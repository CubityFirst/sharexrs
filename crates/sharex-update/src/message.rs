//! `UpdateMessageWindow` (`ShareX.HelpersLib/Presentation/UpdateChecker/`)
//! without its window: the texts it shows, whether it's open, and what Yes
//! does (`UpdateChecker.DownloadUpdateAsync`).

use std::sync::atomic::{AtomicBool, Ordering};

use sharex_strings::helpers_lib as s;

use crate::checker::{GitHubUpdateChecker, UpdateStatus};

/// `ShareX.AvaloniaUI.DialogResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogResult {
    #[default]
    None,
    OK,
    Cancel,
    Yes,
    No,
}

/// `UpdateMessageWindow.IsOpen`: an update message (or the download it
/// started) is showing, so the automatic check skips its turn.
static IS_OPEN: AtomicBool = AtomicBool::new(false);

pub fn is_open() -> bool {
    IS_OPEN.load(Ordering::SeqCst)
}

/// Set when a message opens (`StartAsync`), cleared when it and the
/// download it started are done (its `finally`).
pub fn set_open(open: bool) {
    IS_OPEN.store(open, Ordering::SeqCst);
}

/// `Environment.NewLine`, which `StringBuilder.AppendLine` writes.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// What the window shows for one update.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateMessage {
    /// `Title` (`DialogTitle`).
    pub title: String,
    /// `HeaderText`.
    pub header_text: String,
    /// `MessageText`.
    pub message_text: String,
    /// `ShowChangelog`: not for dev builds.
    pub show_changelog: bool,
    /// The changelog link's target: the release's page (ShareX opens its
    /// website's changelog; ShareXrs's changelog is its release notes).
    pub changelog_url: String,
    pub yes_text: String,
    pub no_text: String,
    /// `ActivateWindow`: the first automatic check (and a check the user
    /// asked for) shows the window; later ones open it minimised and flash
    /// it in the taskbar.
    pub activate_window: bool,
}

impl UpdateMessage {
    /// `new UpdateMessageWindow(updateChecker, activateWindow)`.
    pub fn new(checker: &GitHubUpdateChecker, activate_window: bool) -> Self {
        let (owner, repo) = crate::checker::repository();
        Self {
            title: s::UpdateMessageBox_UpdateMessageBox_update_is_available().to_owned(),
            header_text: s::UpdateMessageWindow_Header().to_owned(),
            message_text: build_message(checker),
            show_changelog: !checker.is_dev,
            changelog_url: checker.release_page().unwrap_or_else(|| format!("https://github.com/{owner}/{repo}/releases")),
            yes_text: s::MyMessageBox_MyMessageBox_Yes().to_owned(),
            no_text: s::MyMessageBox_MyMessageBox_No().to_owned(),
            activate_window,
        }
    }

    /// `StartAsync`'s first test: a window only for an available update.
    pub fn should_show(checker: Option<&GitHubUpdateChecker>) -> bool {
        checker.is_some_and(|c| c.status == UpdateStatus::UpdateAvailable)
    }
}

/// The About window's update panel (`AboutWindow`): shown unless
/// `DisableUpdateCheck`, "Checking for updates..." while the check runs,
/// then the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AboutUpdate {
    /// `UpdatePanel.IsVisible = false`.
    Hidden,
    /// `UpdateStatusText` (with the progress ring while checking).
    Text { text: &'static str, checking: bool },
    /// `UpdateAvailableButton`: opens the update message (`StartAsync`).
    Button(&'static str),
}

impl AboutUpdate {
    /// Before and after `CheckUpdateAsync` (`status` is `None` while it runs).
    pub fn new(disable_update_check: bool, status: Option<UpdateStatus>) -> Self {
        use sharex_strings::sharex as x;
        if disable_update_check {
            return AboutUpdate::Hidden;
        }
        match status {
            None | Some(UpdateStatus::None) => AboutUpdate::Text { text: x::AboutWindow_CheckingForUpdates(), checking: true },
            Some(UpdateStatus::UpdateCheckFailed) => AboutUpdate::Text { text: x::AboutWindow_UpdateCheckFailed(), checking: false },
            Some(UpdateStatus::UpdateAvailable) => AboutUpdate::Button(x::AboutWindow_NewVersionAvailable()),
            Some(UpdateStatus::UpToDate) => AboutUpdate::Text { text: x::AboutWindow_UpToDate(), checking: false },
        }
    }
}

/// `BuildMessage`.
pub fn build_message(checker: &GitHubUpdateChecker) -> String {
    let mut text = String::new();
    text.push_str(if checker.is_portable() {
        s::UpdateMessageBox_UpdateMessageBox_Portable()
    } else {
        s::UpdateMessageBox_UpdateMessageBox_()
    });
    text.push_str(NEW_LINE);
    text.push_str(NEW_LINE);
    text.push_str(s::UpdateMessageBox_UpdateMessageBox_CurrentVersion());
    text.push_str(": ");
    if let Some(v) = &checker.current_version {
        text.push_str(&v.to_string());
    }
    text.push_str(NEW_LINE);
    text.push_str(s::UpdateMessageBox_UpdateMessageBox_LatestVersion());
    text.push_str(": ");
    if let Some(v) = &checker.latest_version {
        text.push_str(&v.to_string());
    }
    if checker.is_dev {
        text.push_str(" Dev");
    }
    if checker.is_pre_release {
        text.push_str(&format!(" ({})", s::UpdateChannel_PreRelease()));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::InstallFlow;
    use crate::version::AppVersion;

    #[test]
    fn builds_the_message() {
        let mut c = GitHubUpdateChecker::new("o", "r", InstallFlow::WindowsSetup);
        c.current_version = Some(AppVersion::parse("22.4.0.0").unwrap());
        c.latest_version = Some(AppVersion::parse("22.5.0").unwrap());
        let nl = NEW_LINE;
        assert_eq!(
            build_message(&c),
            format!("Would you like to download and install it?{nl}{nl}Current version: 22.4.0.0{nl}Latest version: 22.5.0")
        );
        c.flow = InstallFlow::WindowsPortable;
        c.is_dev = true;
        c.is_pre_release = true;
        assert_eq!(
            build_message(&c),
            format!("Would you like to download it?{nl}{nl}Current version: 22.4.0.0{nl}Latest version: 22.5.0 Dev (Pre-release)")
        );
        let m = UpdateMessage::new(&c, true);
        assert!(!m.show_changelog);
        assert_eq!(m.changelog_url, "https://github.com/CubityFirst/sharexrs/releases");
        assert_eq!(m.title, "Update is available");
        assert_eq!(m.header_text, "ShareX update is available");
        assert!(!UpdateMessage::should_show(Some(&c)));
        c.status = UpdateStatus::UpdateAvailable;
        assert!(UpdateMessage::should_show(Some(&c)));
        assert!(!UpdateMessage::should_show(None));
    }

    #[test]
    fn about_panel() {
        assert_eq!(AboutUpdate::new(true, None), AboutUpdate::Hidden);
        assert_eq!(AboutUpdate::new(false, None), AboutUpdate::Text { text: "Checking for updates...", checking: true });
        assert_eq!(
            AboutUpdate::new(false, Some(UpdateStatus::UpdateCheckFailed)),
            AboutUpdate::Text { text: "Update check failed.", checking: false }
        );
        assert_eq!(
            AboutUpdate::new(false, Some(UpdateStatus::UpdateAvailable)),
            AboutUpdate::Button("A newer version of ShareX is available")
        );
    }
}
