//! `GitHubUpdateManager` and `ShareXUpdateManager`: the automatic check, at
//! once and then every hour while `AllowAutoUpdate`
//! (`!SystemOptions.DisableUpdateCheck && AutoCheckUpdate`), skipped while
//! an update message is open, and off for the rest of the run once the
//! user answers No.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use sharex_settings::enums::UpdateChannel;

use crate::checker::{GitHubUpdateChecker, InstallFlow, UpdateStatus, repository};
use crate::message::{self, DialogResult};

/// `UpdateCheckInterval`.
pub const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// `ShareXUpdateManager.CreateUpdateChecker()`: ShareXrs's repository
/// (Cargo.toml's `repository`), the release or pre-release channel, the
/// revision ignored.
pub fn create_update_checker(channel: &UpdateChannel, portable: bool) -> GitHubUpdateChecker {
    let (owner, repo) = repository();
    let mut checker = GitHubUpdateChecker::new(owner, repo, InstallFlow::detect(portable));
    checker.include_pre_release = *channel == UpdateChannel::PreRelease;
    checker.ignore_revision = true;
    checker
}

/// Runs a check (create the checker, `CheckUpdateAsync`): the app's goes to
/// GitHub, a test's returns a canned result.
pub type CheckFn = dyn Fn(&UpdateChannel) -> GitHubUpdateChecker + Send + Sync;
/// Hands an available update to the UI with `activateWindow`; the UI shows
/// the message and calls [`UpdateManager::message_closed`].
pub type UpdateFn = dyn Fn(GitHubUpdateChecker, bool) + Send + Sync;

struct State {
    allow_auto_update: bool,
    auto_update_enabled: bool,
    update_channel: UpdateChannel,
    first_update_check: bool,
    /// Bumped to stop the running timer.
    timer: Option<u64>,
    next_timer: u64,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    check: Box<CheckFn>,
    on_update: Box<UpdateFn>,
    interval: Duration,
}

/// `ShareXUpdateManager`.
#[derive(Clone)]
pub struct UpdateManager {
    shared: Arc<Shared>,
}

impl UpdateManager {
    /// `new ShareXUpdateManager()`: nothing runs until
    /// [`configure_auto_update`](Self::configure_auto_update) allows it.
    pub fn new(check: Box<CheckFn>, on_update: Box<UpdateFn>) -> Self {
        Self::with_interval(check, on_update, UPDATE_CHECK_INTERVAL)
    }

    pub fn with_interval(check: Box<CheckFn>, on_update: Box<UpdateFn>, interval: Duration) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    allow_auto_update: false,
                    auto_update_enabled: true,
                    update_channel: UpdateChannel::Release,
                    first_update_check: true,
                    timer: None,
                    next_timer: 0,
                }),
                wake: Condvar::new(),
                check,
                on_update,
                interval,
            }),
        }
    }

    /// What `ApplicationSettingsRuntime.Apply` sets, then `ConfigureAutoUpdate()`.
    pub fn apply(&self, disable_update_check: bool, auto_check_update: bool, channel: UpdateChannel) {
        {
            let mut s = self.shared.state.lock().expect("update state");
            s.allow_auto_update = !disable_update_check && auto_check_update;
            s.update_channel = channel;
        }
        self.configure_auto_update();
    }

    /// `AutoUpdateEnabled` (false after a No this run).
    pub fn auto_update_enabled(&self) -> bool {
        self.shared.state.lock().expect("update state").auto_update_enabled
    }

    /// Whether the timer is running.
    pub fn is_running(&self) -> bool {
        self.shared.state.lock().expect("update state").timer.is_some()
    }

    pub fn update_channel(&self) -> UpdateChannel {
        self.shared.state.lock().expect("update state").update_channel.clone()
    }

    /// `ConfigureAutoUpdate`: starts the timer (first check at once) or
    /// stops it.
    pub fn configure_auto_update(&self) {
        let mut s = self.shared.state.lock().expect("update state");
        if s.allow_auto_update {
            if s.timer.is_none() {
                let id = s.next_timer;
                s.next_timer += 1;
                s.timer = Some(id);
                let shared = self.shared.clone();
                std::thread::Builder::new()
                    .name("update-timer".into())
                    .spawn(move || timer(shared, id))
                    .expect("update timer thread");
            }
        } else {
            s.timer = None;
            self.shared.wake.notify_all();
        }
    }

    /// What follows `UpdateMessageWindow.StartAsync` in `CheckUpdate`: No
    /// turns the automatic check off for this run, and later messages open
    /// without taking the focus.
    pub fn message_closed(&self, result: DialogResult) {
        let mut s = self.shared.state.lock().expect("update state");
        if result == DialogResult::No {
            s.auto_update_enabled = false;
        }
        s.first_update_check = false;
    }

    /// `Dispose`.
    pub fn stop(&self) {
        let mut s = self.shared.state.lock().expect("update state");
        s.timer = None;
        self.shared.wake.notify_all();
    }

    /// `CheckUpdate` (one timer tick).
    fn tick(shared: &Shared) {
        let channel = {
            let s = shared.state.lock().expect("update state");
            if !s.auto_update_enabled || message::is_open() {
                return;
            }
            s.update_channel.clone()
        };
        let checker = (shared.check)(&channel);
        if checker.status == UpdateStatus::UpdateAvailable && !message::is_open() {
            let activate = shared.state.lock().expect("update state").first_update_check;
            message::set_open(true);
            (shared.on_update)(checker, activate);
        } else {
            // `StartAsync` returns at once without a window.
            shared.state.lock().expect("update state").first_update_check = false;
        }
    }
}

/// `new Timer(TimerCallback, null, TimeSpan.Zero, UpdateCheckInterval)`.
fn timer(shared: Arc<Shared>, id: u64) {
    loop {
        if shared.state.lock().expect("update state").timer != Some(id) {
            return;
        }
        UpdateManager::tick(&shared);
        let s = shared.state.lock().expect("update state");
        let (s, _) = shared.wake.wait_timeout_while(s, shared.interval, |s| s.timer == Some(id)).expect("update state");
        if s.timer != Some(id) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    fn available() -> GitHubUpdateChecker {
        let mut c = GitHubUpdateChecker::new("o", "r", InstallFlow::WindowsSetup);
        c.status = UpdateStatus::UpdateAvailable;
        c
    }

    /// The manager tests share `UpdateMessageWindow.IsOpen`, so they run
    /// one at a time.
    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn checks_at_once_then_every_interval_and_stops_after_no() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        message::set_open(false);
        let checks = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let counter = checks.clone();
        let manager = UpdateManager::with_interval(
            Box::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                available()
            }),
            Box::new(move |_, activate| tx.send(activate).unwrap()),
            Duration::from_millis(40),
        );
        manager.apply(false, true, UpdateChannel::Release);
        // The first check activates the window.
        assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        // While it's open, later ticks skip.
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(checks.load(Ordering::SeqCst), 1);
        manager.message_closed(DialogResult::Yes);
        message::set_open(false);
        // The next one doesn't take the focus.
        assert!(!rx.recv_timeout(Duration::from_secs(5)).unwrap());
        manager.message_closed(DialogResult::No);
        message::set_open(false);
        assert!(!manager.auto_update_enabled());
        let after_no = checks.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(checks.load(Ordering::SeqCst), after_no);
        manager.stop();
    }

    #[test]
    fn disable_update_check_or_auto_check_off_never_checks() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        message::set_open(false);
        let checks = Arc::new(AtomicUsize::new(0));
        let counter = checks.clone();
        let manager = UpdateManager::with_interval(
            Box::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                available()
            }),
            Box::new(|_, _| {}),
            Duration::from_millis(20),
        );
        manager.apply(true, true, UpdateChannel::Release);
        assert!(!manager.is_running());
        manager.apply(false, false, UpdateChannel::Release);
        assert!(!manager.is_running());
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(checks.load(Ordering::SeqCst), 0);
        // Turned on, then off again: the timer stops.
        manager.apply(false, true, UpdateChannel::PreRelease);
        assert!(manager.is_running());
        assert_eq!(manager.update_channel(), UpdateChannel::PreRelease);
        manager.apply(false, false, UpdateChannel::PreRelease);
        assert!(!manager.is_running());
        message::set_open(false);
    }

    #[test]
    fn up_to_date_shows_nothing() {
        let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        message::set_open(false);
        let (tx, rx) = mpsc::channel::<bool>();
        let (checked_tx, checked_rx) = mpsc::channel();
        let manager = UpdateManager::with_interval(
            Box::new(move |_| {
                let _ = checked_tx.send(());
                let mut c = available();
                c.status = UpdateStatus::UpToDate;
                c
            }),
            Box::new(move |_, a| tx.send(a).unwrap()),
            Duration::from_secs(60),
        );
        manager.apply(false, true, UpdateChannel::Release);
        checked_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
        assert!(!message::is_open());
        manager.stop();
    }
}
