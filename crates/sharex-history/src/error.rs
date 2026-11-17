//! What can go wrong reading or writing history: ShareX's exceptions
//! (`SqliteException`, `FormatException`, `JsonException`, `XmlException`,
//! `IOException`).

use std::fmt;

#[derive(Debug)]
pub enum HistoryError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
    /// A date that doesn't parse, a missing column, a value of the wrong kind.
    Format(String),
    Xml(String),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "SQLite: {e}"),
            Self::Io(e) => write!(f, "{e}"),
            Self::Json(e) => write!(f, "JSON: {e}"),
            Self::Format(e) => write!(f, "{e}"),
            Self::Xml(e) => write!(f, "XML: {e}"),
        }
    }
}

impl std::error::Error for HistoryError {}

impl From<rusqlite::Error> for HistoryError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<std::io::Error> for HistoryError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for HistoryError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<crate::datetime::ParseError> for HistoryError {
    fn from(e: crate::datetime::ParseError) -> Self {
        Self::Format(e.to_string())
    }
}
