//! `ShareX.HelpersLib/Input/WindowsHotkeyHost.cs`: `RegisterHotKey`.
//!
//! ShareX registers its hotkeys on a hidden window of its UI thread. winit
//! owns ShareXrs's UI thread's message loop, so here a thread of its own
//! registers them (`RegisterHotKey` with no window ties a hotkey to the
//! calling thread's queue) and reports `WM_HOTKEY` through the callback,
//! which posts to the winit loop. Registering and unregistering are sent
//! to that thread and wait for its answer, so the status is known at once,
//! as in ShareX.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use sharex_settings::{HotkeyInfo, HotkeyStatus};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromRect};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowRect, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    WM_APP, WM_HOTKEY, WM_QUIT, WM_USER,
};

use super::{HotkeyHost, OnPress};

/// Asks the hotkey thread to run its queued requests.
const WM_REQUEST: u32 = WM_APP + 1;

enum Request {
    Register { id: u16, modifiers: u32, key: u32 },
    Unregister { id: u16 },
}

pub struct WindowsHotkeyHost {
    thread_id: u32,
    requests: mpsc::Sender<(Request, mpsc::Sender<bool>)>,
    join: Option<thread::JoinHandle<()>>,
    /// The next id to register under (`GlobalAddAtom` in ShareX; any id in
    /// 0x0000-0xBFFF is an application's own).
    next_id: u16,
}

impl WindowsHotkeyHost {
    pub fn new(on_press: OnPress) -> Self {
        let (requests, rx) = mpsc::channel::<(Request, mpsc::Sender<bool>)>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || {
                let mut msg: MSG = unsafe { std::mem::zeroed() };
                // Makes the thread's message queue, so posts to it aren't lost.
                unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE) };
                let _ = ready_tx.send(unsafe { GetCurrentThreadId() });
                let mut registered: Vec<u16> = Vec::new();
                while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                    match msg.message {
                        WM_HOTKEY => on_press(msg.wParam as u16),
                        WM_REQUEST => {
                            while let Ok((request, reply)) = rx.try_recv() {
                                let ok = match request {
                                    Request::Register { id, modifiers, key } => {
                                        let ok = unsafe {
                                            RegisterHotKey(std::ptr::null_mut(), id as i32, modifiers as HOT_KEY_MODIFIERS, key)
                                        } != 0;
                                        if ok {
                                            registered.push(id);
                                        }
                                        ok
                                    }
                                    Request::Unregister { id } => {
                                        registered.retain(|&r| r != id);
                                        unsafe { UnregisterHotKey(std::ptr::null_mut(), id as i32) != 0 }
                                    }
                                };
                                let _ = reply.send(ok);
                            }
                        }
                        _ => {}
                    }
                }
                // `Close()`: what's still registered goes with the host.
                for id in registered {
                    unsafe { UnregisterHotKey(std::ptr::null_mut(), id as i32) };
                }
            })
            .expect("hotkey thread");
        let thread_id = ready_rx.recv().expect("hotkey thread started");
        Self { thread_id, requests, join: Some(join), next_id: 1 }
    }

    fn send(&self, request: Request) -> bool {
        let (tx, rx) = mpsc::channel();
        if self.requests.send((request, tx)).is_err() {
            return false;
        }
        unsafe { PostThreadMessageW(self.thread_id, WM_REQUEST, 0, 0) };
        rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
    }
}

impl HotkeyHost for WindowsHotkeyHost {
    fn register(&mut self, info: &mut HotkeyInfo) {
        if info.status == HotkeyStatus::Registered {
            return;
        }
        if !info.is_valid_hotkey() {
            info.status = HotkeyStatus::NotConfigured;
            return;
        }
        if info.id == 0 {
            info.id = self.next_id;
            self.next_id = if self.next_id >= 0xBFFF { 1 } else { self.next_id + 1 };
        }
        let request = Request::Register { id: info.id, modifiers: info.modifiers_enum(), key: info.key_code().0 };
        if self.send(request) {
            info.status = HotkeyStatus::Registered;
        } else {
            eprintln!("Unable to register hotkey: {info}");
            info.id = 0;
            info.status = HotkeyStatus::Failed;
        }
    }

    fn unregister(&mut self, info: &mut HotkeyInfo) -> bool {
        if info.id > 0 && self.send(Request::Unregister { id: info.id }) {
            info.id = 0;
            info.status = HotkeyStatus::NotConfigured;
            return true;
        }
        info.status = HotkeyStatus::Failed;
        false
    }
}

impl Drop for WindowsHotkeyHost {
    fn drop(&mut self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// `CaptureHelpers.IsActiveWindowFullscreen()`.
pub fn is_active_window_fullscreen() -> bool {
    let hwnd: HWND = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return false;
    }
    let mut class = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
    let class = String::from_utf16_lossy(&class[..len.max(0) as usize]);
    if ["Progman", "WorkerW"].iter().any(|c| class.eq_ignore_ascii_case(c)) {
        return false;
    }
    let mut window: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut window) } == 0 {
        return false;
    }
    let monitor = unsafe { MonitorFromRect(&window, MONITOR_DEFAULTTONEAREST) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return false;
    }
    let m = info.rcMonitor;
    // `windowRectangle.Contains(monitorRectangle)`.
    window.left <= m.left && window.top <= m.top && window.right >= m.right && window.bottom >= m.bottom
}
