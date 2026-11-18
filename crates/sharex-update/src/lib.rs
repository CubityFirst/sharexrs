//! ShareX's update check, against ShareXrs's own GitHub releases
//! (`https://github.com/CubityFirst/sharexrs`, Cargo.toml's `repository`):
//!
//! - [`version`] — `System.Version` and `Helpers.CompareVersion`, plus SemVer
//!   pre-release tags (addition);
//! - [`release`] — GitHub's release JSON as Newtonsoft reads it;
//! - [`checker`] — `UpdateChecker`/`GitHubUpdateChecker` (asset by name
//!   ending, status), with the macOS and Linux files as additions;
//! - [`downloader`] — `FileDownloader`;
//! - [`downloader_window`] and [`message`] — the update windows' logic
//!   (`DownloaderWindow`, `UpdateMessageWindow`), without their UI;
//! - [`install`] — running the setup (`/UPDATE`), the signature check, and
//!   the macOS/Linux in-place updates (additions);
//! - [`manager`] — `GitHubUpdateManager`/`ShareXUpdateManager`'s hourly check.
//!
//! The requests go through `sharex-uploaders`' HTTP layer: ShareXrs's user
//! agent and proxy.

pub mod checker;
pub mod downloader;
pub mod downloader_window;
pub mod install;
pub mod manager;
pub mod message;
pub mod release;
pub mod text;
pub mod version;

pub use checker::{Arch, GitHubUpdateChecker, InstallFlow, UpdateStatus};
pub use downloader_window::{DownloaderWindow, DownloaderWindowStatus};
pub use install::{SystemHost, UpdateHost};
pub use manager::{UpdateManager, create_update_checker};
pub use message::{DialogResult, UpdateMessage};
pub use version::{AppVersion, DotNetVersion};
