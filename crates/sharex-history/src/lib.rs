//! `ShareX.HistoryLib` as data (see `docs/HISTORY.md`): History.db read and
//! written with ShareX's schema and values ([`HistoryManagerSqlite`]), the
//! legacy History.json/History.xml files and the startup migration
//! ([`legacy`]), the History and Image history windows' queries
//! ([`query`]) and actions ([`actions`]), their settings ([`settings`]),
//! and the stats ([`stats`]).
//!
//! Dates are .NET `DateTime`s ([`datetime`]): History.db keeps the text
//! `ToString("o")` writes, and reading it back gives local time, so the
//! machine's time zone matters exactly as it does for ShareX.

pub mod actions;
/// The .NET string comparisons, shared with the other tools (`sharex_helpers::culture`).
pub use sharex_helpers::culture;
pub mod datetime;
pub mod error;
pub mod item;
pub mod legacy;
pub mod paths;
pub mod query;
pub mod settings;
pub mod sqlite;
pub mod stats;
pub mod tags;

pub use datetime::{DateTime, DateTimeKind, LocalZone, SystemZone};
pub use error::HistoryError;
pub use item::HistoryItem;
pub use legacy::migrate_history_file;
pub use query::{HistoryFilter, ImageHistoryFilter};
pub use settings::{HistoryColumnLayout, HistorySettings, ImageHistorySettings};
pub use sqlite::HistoryManagerSqlite;
pub use stats::HistoryStats;
pub use tags::Tags;

/// `AppPaths.HistoryFileNameOld`.
pub const HISTORY_FILE_NAME_OLD: &str = "History.json";
/// `AppPaths.HistoryFileName`.
pub const HISTORY_FILE_NAME: &str = "History.db";
