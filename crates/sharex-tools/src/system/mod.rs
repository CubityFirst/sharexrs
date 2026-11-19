//! The System tools' data side (`ShareX.Tools/Tools/{ClipboardViewer,
//! InspectWindow, NetworkMonitor, BorderlessWindow}`): everything their view
//! models compute, without the windows and without touching the clipboard
//! or other apps' windows. The monitor test is [`crate::monitor_test`].

pub mod borderless_window;
pub mod clipboard_viewer;
pub mod inspect_window;
pub mod network_monitor;
pub mod unix_ping;

/// `Environment.NewLine`.
#[cfg(windows)]
pub const NEW_LINE: &str = "\r\n";
#[cfg(not(windows))]
pub const NEW_LINE: &str = "\n";
