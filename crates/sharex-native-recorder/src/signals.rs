//! The recorder's wait handles: `stop` and `begin` (manual reset),
//! `changed` (auto reset), and `FrameTimer` (`Native/FrameTimer.cs`: a
//! high-resolution waitable timer, so 30 fps deadlines aren't rounded to
//! the 15.6 ms system tick). [`Waiter::Virtual`] runs the same loop on a
//! [`VirtualTime`] for tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::clock::VirtualTime;
use crate::options::TICKS_PER_MILLISECOND;

/// What ended a wait (`WaitHandle.WaitAny([stop, changed, timer])`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    Stop,
    Changed,
    Timer,
}

#[derive(Default)]
struct Flags {
    stop: bool,
    begin: bool,
    changed: bool,
}

/// The events shared between the worker and its controls.
pub struct Signals {
    flags: Mutex<Flags>,
    cond: Condvar,
    stopped: AtomicBool,
    #[cfg(windows)]
    events: win::Events,
}

impl Signals {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            flags: Mutex::new(Flags::default()),
            cond: Condvar::new(),
            stopped: AtomicBool::new(false),
            #[cfg(windows)]
            events: win::Events::new(),
        })
    }

    fn update(&self, f: impl FnOnce(&mut Flags)) {
        let mut flags = self.flags.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut flags);
        self.cond.notify_all();
    }

    pub fn set_stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.update(|f| f.stop = true);
        #[cfg(windows)]
        self.events.set(0);
    }

    pub fn set_begin(&self) {
        self.update(|f| f.begin = true);
        #[cfg(windows)]
        self.events.set(1);
    }

    pub fn set_changed(&self) {
        self.update(|f| f.changed = true);
        #[cfg(windows)]
        self.events.set(2);
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    fn take_changed(&self) -> bool {
        let mut f = self.flags.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut f.changed)
    }
}

/// How the worker waits: on the real clock, or by moving a virtual one.
pub enum Waiter {
    Real(RealTimer),
    Virtual(Arc<VirtualTime>),
}

impl Waiter {
    pub fn real() -> Result<Self, String> {
        Ok(Self::Real(RealTimer::new()?))
    }

    /// `WaitHandle.WaitAny([stop, begin]) == 0`.
    pub fn wait_begin_or_stop(&mut self, s: &Signals) -> bool {
        match self {
            #[cfg(windows)]
            Self::Real(_) => s.events.wait_begin_or_stop(),
            #[cfg(not(windows))]
            Self::Real(_) => {
                let mut f = s.flags.lock().unwrap_or_else(|e| e.into_inner());
                while !f.stop && !f.begin {
                    f = s.cond.wait(f).unwrap_or_else(|e| e.into_inner());
                }
                f.stop
            }
            Self::Virtual(_) => {
                let mut f = s.flags.lock().unwrap_or_else(|e| e.into_inner());
                while !f.stop && !f.begin {
                    f = s.cond.wait(f).unwrap_or_else(|e| e.into_inner());
                }
                f.stop
            }
        }
    }

    /// `stop.WaitOne(ms)`.
    pub fn stop_wait(&mut self, s: &Signals, ms: i64) -> bool {
        match self {
            Self::Real(_) => {
                let f = s.flags.lock().unwrap_or_else(|e| e.into_inner());
                let (f, _) = s.cond.wait_timeout_while(f, Duration::from_millis(ms as u64), |f| !f.stop).unwrap_or_else(|e| e.into_inner());
                f.stop
            }
            Self::Virtual(t) => {
                if s.is_stopped() {
                    return true;
                }
                t.advance(ms * TICKS_PER_MILLISECOND);
                s.is_stopped()
            }
        }
    }

    /// `timer.Arm(delay); WaitHandle.WaitAny([stop, changed, timer])`.
    pub fn wait(&mut self, s: &Signals, delay_ticks: i64) -> Wake {
        match self {
            Self::Real(timer) => timer.wait(s, delay_ticks),
            Self::Virtual(t) => {
                if s.is_stopped() {
                    return Wake::Stop;
                }
                if s.take_changed() {
                    return Wake::Changed;
                }
                t.advance(delay_ticks.max(1));
                if s.is_stopped() { Wake::Stop } else { Wake::Timer }
            }
        }
    }
}

/// `FrameTimer`.
pub struct RealTimer {
    #[cfg(windows)]
    timer: win::Timer,
}

impl RealTimer {
    fn new() -> Result<Self, String> {
        Ok(Self {
            #[cfg(windows)]
            timer: win::Timer::new()?,
        })
    }

    #[cfg(windows)]
    fn wait(&mut self, s: &Signals, delay_ticks: i64) -> Wake {
        self.timer.arm(delay_ticks);
        match s.events.wait_stop_changed_timer(&self.timer) {
            0 => Wake::Stop,
            1 => {
                s.take_changed();
                Wake::Changed
            }
            _ => Wake::Timer,
        }
    }

    #[cfg(not(windows))]
    fn wait(&mut self, s: &Signals, delay_ticks: i64) -> Wake {
        let f = s.flags.lock().unwrap_or_else(|e| e.into_inner());
        let timeout = Duration::from_nanos(delay_ticks.max(1) as u64 * 100);
        let (mut f, _) = s.cond.wait_timeout_while(f, timeout, |f| !f.stop && !f.changed).unwrap_or_else(|e| e.into_inner());
        if f.stop {
            Wake::Stop
        } else if std::mem::take(&mut f.changed) {
            Wake::Changed
        } else {
            Wake::Timer
        }
    }
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{CreateEventW, CreateWaitableTimerExW, INFINITE, SetEvent, SetWaitableTimerEx, WaitForMultipleObjects};

    /// stop (manual), begin (manual), changed (auto).
    pub struct Events([HANDLE; 3]);

    // SAFETY: event handles may be used from any thread.
    unsafe impl Send for Events {}
    unsafe impl Sync for Events {}

    impl Events {
        pub fn new() -> Self {
            // SAFETY: unnamed events; a failure leaves a null handle, which waits fail on.
            unsafe {
                Self([
                    CreateEventW(None, true, false, None).unwrap_or_default(),
                    CreateEventW(None, true, false, None).unwrap_or_default(),
                    CreateEventW(None, false, false, None).unwrap_or_default(),
                ])
            }
        }
        pub fn set(&self, i: usize) {
            // SAFETY: our own event.
            unsafe {
                let _ = SetEvent(self.0[i]);
            }
        }
        pub fn wait_begin_or_stop(&self) -> bool {
            // SAFETY: valid handles.
            let r = unsafe { WaitForMultipleObjects(&[self.0[0], self.0[1]], false, INFINITE) };
            r == WAIT_OBJECT_0
        }
        pub fn wait_stop_changed_timer(&self, timer: &Timer) -> u32 {
            // SAFETY: valid handles.
            let r = unsafe { WaitForMultipleObjects(&[self.0[0], self.0[2], timer.0], false, INFINITE) };
            r.0.wrapping_sub(WAIT_OBJECT_0.0)
        }
    }

    impl Drop for Events {
        fn drop(&mut self) {
            for h in self.0 {
                if !h.is_invalid() {
                    // SAFETY: our own handles.
                    unsafe {
                        let _ = CloseHandle(h);
                    }
                }
            }
        }
    }

    /// A high-resolution waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`).
    pub struct Timer(HANDLE);

    impl Timer {
        pub fn new() -> Result<Self, String> {
            // SAFETY: an unnamed timer, TIMER_ALL_ACCESS.
            unsafe { CreateWaitableTimerExW(None, None, 2, 0x001F_0003) }.map(Self).map_err(|e| e.message())
        }
        pub fn arm(&self, delay_ticks: i64) {
            // A relative 100 ns interval.
            let due = -delay_ticks.max(1);
            // SAFETY: our own timer.
            unsafe {
                let _ = SetWaitableTimerEx(self.0, &due, 0, None, None, None, 0);
            }
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            // SAFETY: our own handle.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
