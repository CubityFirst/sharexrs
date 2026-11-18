//! ShareX's update check in the app: `ApplicationState.UpdateManager`
//! (`ShareXUpdateManager`, set up by `ApplicationSettingsRuntime.Apply`),
//! the update message it shows, and what Yes leads to. The check and the
//! windows' logic are `sharex-update`; the windows are `dialogs::update`.
//!
//! The releases are ShareXrs's own (Cargo.toml's `repository`). With the
//! `DisableUpdateCheck` policy or "Automatically check for updates" off,
//! nothing is ever requested.

use std::path::Path;
use std::sync::Arc;

use sharex_update::install::UpdateHost;
use sharex_update::{DialogResult, GitHubUpdateChecker, InstallFlow, UpdateManager, create_update_checker, message};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};

use crate::dialogs::update::{DownloaderDialog, UpdateMessageWindow, UpdateOrigin};
use crate::{App, UserEvent};

/// What the update check tells the event loop.
#[derive(Debug)]
pub enum UpdateEvent {
    /// The hourly check found a newer release: show the message
    /// (`activateWindow`: the first check of the run takes the focus).
    Available(Box<GitHubUpdateChecker>, bool),
}

/// The system as the update flow uses it: the browser setting
/// (`URLHelpers.OpenURL`), the shell, `ShellExecute` for the installer.
pub struct AppHost;

impl UpdateHost for AppHost {
    fn open_url(&self, url: &str) {
        crate::upload::open_url(url);
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        if crate::shell::open_file(path) { Ok(()) } else { Err(format!("couldn't open {}", path.display())) }
    }

    fn run_installer(&self, path: &Path, arguments: &str, elevate: bool) -> Result<(), String> {
        sharex_update::install::shell_execute(path, arguments, elevate)
    }

    fn restart(&self, exe: &Path) -> Result<(), String> {
        std::process::Command::new(exe).spawn().map(drop).map_err(|e| format!("couldn't start ShareXrs again: {e}"))
    }

    /// The app exits through `Effect::UpdateFinished` instead.
    fn shutdown(&self) {}
}

/// `ApplicationState.UpdateManager` and the host its downloads use.
pub struct Updates {
    pub manager: UpdateManager,
    pub host: Arc<dyn UpdateHost>,
}

impl Updates {
    /// `new ShareXUpdateManager()`: nothing runs until the settings allow it.
    pub fn new(proxy: EventLoopProxy<UserEvent>, portable: bool) -> Self {
        let check = move |channel: &sharex_settings::enums::UpdateChannel| {
            let mut checker = create_update_checker(channel, portable);
            checker.check_update();
            if let Some(e) = &checker.error {
                eprintln!("GitHub update check failed. {e}");
            }
            checker
        };
        let on_update = move |checker: GitHubUpdateChecker, activate: bool| {
            let _ = proxy.send_event(UserEvent::Update(UpdateEvent::Available(Box::new(checker), activate)));
        };
        Self { manager: UpdateManager::new(Box::new(check), Box::new(on_update)), host: Arc::new(AppHost) }
    }
}

impl App {
    /// `ApplicationSettingsRuntime.Apply`'s part:
    /// `AllowAutoUpdate = !SystemOptions.DisableUpdateCheck && AutoCheckUpdate`,
    /// the channel, then `ConfigureAutoUpdate()`.
    pub(crate) fn apply_update_settings(&mut self) {
        let s = &self.settings.settings;
        self.updates.manager.apply(self.settings.system.disable_update_check, s.AutoCheckUpdate, s.UpdateChannel.clone());
    }

    pub(crate) fn update_event(&mut self, event_loop: &ActiveEventLoop, event: UpdateEvent) {
        match event {
            UpdateEvent::Available(checker, activate) => {
                self.open_dialog_window(event_loop, Box::new(UpdateMessageWindow::new(checker, activate, UpdateOrigin::Timer)));
            }
        }
    }

    /// `UpdateChecker.DownloadUpdateAsync`: the portable build opens the
    /// download in the browser; the others download it in the downloader
    /// window.
    pub(crate) fn download_update(&mut self, event_loop: &ActiveEventLoop, checker: GitHubUpdateChecker, origin: UpdateOrigin) {
        eprintln!(
            "Updating ShareXrs from version {} to {}",
            checker.current_version.as_ref().map(ToString::to_string).unwrap_or_default(),
            checker.latest_version.as_ref().map(ToString::to_string).unwrap_or_default()
        );
        if checker.is_portable() || checker.flow == InstallFlow::WindowsPortable {
            if let Some(url) = &checker.download_url {
                self.updates.host.open_url(url);
            }
            self.update_finished(event_loop, DialogResult::Yes, origin, false);
            return;
        }
        let dialog = DownloaderDialog::new(&checker, self.updates.host.clone(), origin);
        self.open_dialog_window(event_loop, Box::new(dialog));
    }

    /// `StartAsync` returned: `IsOpen` clears; for the hourly check, No
    /// turns it off for this run and later messages don't take the focus;
    /// ShareXrs exits when an installer (or the replaced program) takes over
    /// (`AvaloniaBootstrapper.Shutdown()`).
    pub(crate) fn update_finished(&mut self, event_loop: &ActiveEventLoop, result: DialogResult, origin: UpdateOrigin, shutdown: bool) {
        message::set_open(false);
        if origin == UpdateOrigin::Timer {
            self.updates.manager.message_closed(result);
        }
        if shutdown {
            self.exit(event_loop);
        }
    }
}
