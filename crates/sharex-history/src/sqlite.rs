//! `HistoryManagerSQLite` (with its `HistoryManager` base): History.db.
//!
//! One table, made on open if it isn't there, no indices:
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS History (
//!     Id INTEGER PRIMARY KEY AUTOINCREMENT,
//!     FileName TEXT, FilePath TEXT, DateTime TEXT, Type TEXT, Host TEXT,
//!     URL TEXT, ThumbnailURL TEXT, DeletionURL TEXT, ShortenedURL TEXT,
//!     Tags TEXT
//! );
//! ```
//!
//! Every value is text: `DateTime` is `ToString("o")`, `Tags` Json.NET's
//! JSON of the dictionary, a null string SQL `NULL`. ShareX keeps one
//! connection open for the app's lifetime (`SettingManager.HistoryConnect`)
//! with `PRAGMA busy_timeout = 5000`, writes in `BEGIN IMMEDIATE`
//! transactions (Microsoft.Data.Sqlite's `BeginTransaction`), and retries a
//! busy statement every 150 ms for up to its 30 second command timeout, as
//! Microsoft.Data.Sqlite does. The journal stays SQLite's default
//! (`DELETE`), so the file is all there is to back up.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rusqlite::types::ValueRef;
use rusqlite::{Connection, DropBehavior, ErrorCode, Transaction, TransactionBehavior, params};

use crate::datetime::{DateTime, LocalZone, SystemZone};
use crate::error::HistoryError;
use crate::item::HistoryItem;
use crate::tags::Tags;

/// The `CREATE TABLE` ShareX runs on every open (`EnsureDatabase`),
/// verbatim, with the CRLF line ends of ShareX's source file: SQLite keeps
/// this text in `sqlite_master`.
pub const CREATE_TABLE_SQL: &str = concat!(
    "\r\n",
    "CREATE TABLE IF NOT EXISTS History (\r\n",
    "    Id INTEGER PRIMARY KEY AUTOINCREMENT,\r\n",
    "    FileName TEXT,\r\n",
    "    FilePath TEXT,\r\n",
    "    DateTime TEXT,\r\n",
    "    Type TEXT,\r\n",
    "    Host TEXT,\r\n",
    "    URL TEXT,\r\n",
    "    ThumbnailURL TEXT,\r\n",
    "    DeletionURL TEXT,\r\n",
    "    ShortenedURL TEXT,\r\n",
    "    Tags TEXT\r\n",
    ");\r\n",
);

const INSERT_SQL: &str = "
INSERT INTO History
(FileName, FilePath, DateTime, Type, Host, URL, ThumbnailURL, DeletionURL, ShortenedURL, Tags)
VALUES (@FileName, @FilePath, @DateTime, @Type, @Host, @URL, @ThumbnailURL, @DeletionURL, @ShortenedURL, @Tags);";

const UPDATE_SQL: &str = "
UPDATE History SET
FileName = @FileName,
FilePath = @FilePath,
DateTime = @DateTime,
Type = @Type,
Host = @Host,
URL = @URL,
ThumbnailURL = @ThumbnailURL,
DeletionURL = @DeletionURL,
ShortenedURL = @ShortenedURL,
Tags = @Tags
WHERE Id = @Id;";

/// Microsoft.Data.Sqlite's `SqliteCommand.CommandTimeout` default.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// `HistoryManagerSQLite`.
pub struct HistoryManagerSqlite {
    file_path: PathBuf,
    connection: Connection,
    zone: Arc<dyn LocalZone + Send + Sync>,
}

impl std::fmt::Debug for HistoryManagerSqlite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryManagerSqlite").field("file_path", &self.file_path).finish()
    }
}

fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(e.sqlite_error_code(), Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked))
}

/// Microsoft.Data.Sqlite's busy loop: a statement that finds the database
/// locked (after SQLite's own `busy_timeout` wait) is retried every 150 ms
/// until the command timeout has passed.
fn retry<T>(mut f: impl FnMut() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    let start = Instant::now();
    loop {
        match f() {
            Err(e) if is_busy(&e) && start.elapsed() < COMMAND_TIMEOUT => std::thread::sleep(Duration::from_millis(150)),
            r => return r,
        }
    }
}

/// `reader["Column"].ToString()`: `DBNull` is `""`, numbers their text.
fn value_to_string(v: ValueRef<'_>) -> String {
    match v {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => dotnet_double(f),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(_) => "System.Byte[]".to_string(),
    }
}

/// `double.ToString()`: the shortest round-trip digits, `E+XX` past 15 digits.
fn dotnet_double(f: f64) -> String {
    if f.is_finite() && f != 0.0 && !(1e-5..1e15).contains(&f.abs()) {
        let s = format!("{f:E}");
        let (m, e) = s.split_once('E').unwrap();
        let e: i32 = e.parse().unwrap();
        format!("{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
    } else {
        format!("{f}")
    }
}

/// Commits by hand so a busy `COMMIT` is retried; an error before this
/// rolls back (`SqliteTransaction.Dispose`).
fn commit(mut tx: Transaction<'_>) -> Result<(), HistoryError> {
    retry(|| tx.execute_batch("COMMIT;"))?;
    tx.set_drop_behavior(DropBehavior::Ignore);
    Ok(())
}

impl HistoryManagerSqlite {
    /// `connection.BeginTransaction()`: `BEGIN IMMEDIATE;`.
    fn begin(&self) -> rusqlite::Result<Transaction<'_>> {
        retry(|| Transaction::new_unchecked(&self.connection, TransactionBehavior::Immediate))
    }

    /// `new HistoryManagerSQLite(filePath)`: makes the folder, opens (or
    /// creates) the database with a 5 s busy timeout, and makes the table.
    pub fn open(file_path: impl AsRef<Path>) -> Result<Self, HistoryError> {
        Self::open_with_zone(file_path, Arc::new(SystemZone))
    }

    /// As [`open`](Self::open), with dates read and written in `zone`.
    pub fn open_with_zone(file_path: impl AsRef<Path>, zone: Arc<dyn LocalZone + Send + Sync>) -> Result<Self, HistoryError> {
        let file_path = file_path.as_ref().to_path_buf();
        if let Some(dir) = file_path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let connection = Connection::open(&file_path)?;
        connection.execute_batch("PRAGMA busy_timeout = 5000;")?;
        retry(|| connection.execute_batch(CREATE_TABLE_SQL))?;
        Ok(Self { file_path, connection, zone })
    }

    /// `FilePath`.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn zone(&self) -> &(dyn LocalZone + Send + Sync) {
        &*self.zone
    }

    /// The open connection, for tools.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// `GetHistoryItems` without the error box: every row in table order
    /// (`SELECT * FROM History;`, so oldest id first). A row whose date
    /// doesn't parse (a `NULL` one included) fails the whole load, as
    /// `DateTime.Parse` throwing does in ShareX.
    pub fn load(&self) -> Result<Vec<HistoryItem>, HistoryError> {
        let mut stmt = retry(|| self.connection.prepare("SELECT * FROM History;"))?;
        let names: Vec<String> = stmt.column_names().map(str::to_string).collect();
        let col = |name: &str| -> Result<usize, HistoryError> {
            names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(name))
                .ok_or_else(|| HistoryError::Format(format!("Could not find specified column in results: {name}")))
        };
        let c = [
            col("Id")?,
            col("FileName")?,
            col("FilePath")?,
            col("DateTime")?,
            col("Type")?,
            col("Host")?,
            col("URL")?,
            col("ThumbnailURL")?,
            col("DeletionURL")?,
            col("ShortenedURL")?,
            col("Tags")?,
        ];
        let mut rows = stmt.query([])?;
        let mut items = Vec::new();
        // A read only waits on a writer's exclusive lock: busy_timeout covers it.
        while let Some(row) = rows.next()? {
            let text = |i: usize| -> rusqlite::Result<Option<String>> { Ok(Some(value_to_string(row.get_ref(i)?))) };
            let date_text = value_to_string(row.get_ref(c[3])?);
            let tags_text = value_to_string(row.get_ref(c[10])?);
            items.push(HistoryItem {
                id: row.get(c[0])?,
                file_name: text(c[1])?,
                file_path: text(c[2])?,
                date_time: DateTime::parse_in(&date_text, &*self.zone)?,
                r#type: text(c[4])?,
                host: text(c[5])?,
                url: text(c[6])?,
                thumbnail_url: text(c[7])?,
                deletion_url: text(c[8])?,
                shortened_url: text(c[9])?,
                tags: Tags::from_json(&tags_text)?,
            });
        }
        Ok(items)
    }

    /// The items newest first, as both history windows list them
    /// (`GetHistoryItemsAsync` then `Reverse`).
    pub fn load_newest_first(&self) -> Result<Vec<HistoryItem>, HistoryError> {
        let mut items = self.load()?;
        items.reverse();
        Ok(items)
    }

    /// `AppendHistoryItem`.
    pub fn append_history_item(&mut self, item: &mut HistoryItem) -> bool {
        self.append_history_items(std::slice::from_mut(item))
    }

    /// `AppendHistoryItems`: the valid items (`IsValidHistoryItem`) in one
    /// transaction, each getting its new id; false (logged) on an error.
    pub fn append_history_items(&mut self, items: &mut [HistoryItem]) -> bool {
        let mut valid: Vec<&mut HistoryItem> = items.iter_mut().filter(|i| i.is_valid()).collect();
        match self.append_refs(&mut valid) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("history: couldn't add items to {}: {e}", self.file_path.display());
                false
            }
        }
    }

    /// The protected `Append`: every item, valid or not (the JSON migration
    /// uses it as is).
    pub fn append(&mut self, items: &mut [HistoryItem]) -> Result<(), HistoryError> {
        let mut all: Vec<&mut HistoryItem> = items.iter_mut().collect();
        self.append_refs(&mut all)
    }

    fn append_refs(&mut self, items: &mut [&mut HistoryItem]) -> Result<(), HistoryError> {
        let zone = self.zone.clone();
        let tx = self.begin()?;
        {
            let mut stmt = tx.prepare(INSERT_SQL)?;
            for item in items.iter_mut() {
                let date = item.date_time.to_string_o_in(&*zone);
                let tags = item.tags.as_ref().map(Tags::to_json);
                retry(|| {
                    stmt.execute(params![
                        item.file_name,
                        item.file_path,
                        date,
                        item.r#type,
                        item.host,
                        item.url,
                        item.thumbnail_url,
                        item.deletion_url,
                        item.shortened_url,
                        tags
                    ])
                })?;
                item.id = tx.last_insert_rowid();
            }
        }
        commit(tx)
    }

    /// `Edit`: writes every field of the row with the item's id.
    pub fn edit(&mut self, item: &HistoryItem) -> Result<(), HistoryError> {
        let date = item.date_time.to_string_o_in(&*self.zone);
        let tags = item.tags.as_ref().map(Tags::to_json);
        let tx = self.begin()?;
        retry(|| {
            tx.execute(
                UPDATE_SQL,
                params![
                    item.file_name,
                    item.file_path,
                    date,
                    item.r#type,
                    item.host,
                    item.url,
                    item.thumbnail_url,
                    item.deletion_url,
                    item.shortened_url,
                    tags,
                    item.id
                ],
            )
        })?;
        commit(tx)
    }

    /// `Delete`: the rows with these items' ids, in one transaction.
    pub fn delete(&mut self, items: &[HistoryItem]) -> Result<(), HistoryError> {
        if items.is_empty() {
            return Ok(());
        }
        let tx = self.begin()?;
        {
            let mut stmt = tx.prepare("DELETE FROM History WHERE Id = @Id;")?;
            for item in items {
                retry(|| stmt.execute([item.id]))?;
            }
        }
        commit(tx)
    }

    /// `MigrateFromJSON`: the JSON file's items appended as they are.
    pub fn migrate_from_json(&mut self, json_file_path: &Path) -> Result<(), HistoryError> {
        let mut items = crate::legacy::load_json(json_file_path, &*self.zone)?;
        if !items.is_empty() {
            self.append(&mut items)?;
        }
        Ok(())
    }

    /// The XML history's items appended as they are. ShareX 22.4 has the
    /// reader (`HistoryManagerXML`) but no step that migrates History.xml;
    /// this is that step, for a ShareXrs tool to offer.
    pub fn migrate_from_xml(&mut self, xml_file_path: &Path) -> Result<(), HistoryError> {
        let mut items = crate::legacy::load_xml(xml_file_path, &*self.zone)?;
        if !items.is_empty() {
            self.append(&mut items)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datetime::{DateTimeKind, RuleZone};

    fn zone() -> Arc<dyn LocalZone + Send + Sync> {
        Arc::new(RuleZone { standard: 0, eu_dst: true })
    }

    fn item(name: &str) -> HistoryItem {
        HistoryItem {
            file_name: Some(name.into()),
            file_path: Some(format!(r"C:\Shots\{name}")),
            date_time: DateTime::parse_in("2026-07-01T10:00:00.1234567+01:00", &*zone()).unwrap(),
            r#type: Some("Image".into()),
            ..Default::default()
        }
    }

    #[test]
    fn write_read_edit_delete() {
        let dir = std::env::temp_dir().join(format!("sharex-history-{}", std::process::id()));
        let db = dir.join("sub").join("History.db");
        let _ = std::fs::remove_dir_all(&dir);
        let mut m = HistoryManagerSqlite::open_with_zone(&db, zone()).unwrap();
        let mut items = vec![item("a.png"), HistoryItem { file_name: None, ..item("bad.png") }, item("b.png")];
        assert!(m.append_history_items(&mut items));
        assert_eq!((items[0].id, items[1].id, items[2].id), (1, 0, 2));
        let raw: (Option<String>, String, Option<String>) = m
            .connection()
            .query_row("SELECT Host, DateTime, Tags FROM History WHERE Id = 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!(raw, (None, "2026-07-01T10:00:00.1234567+01:00".into(), None));
        let mut loaded = m.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].host.as_deref(), Some(""));
        assert_eq!(loaded[0].date_time.kind(), DateTimeKind::Local);
        loaded[0].set_favorite(true);
        m.edit(&loaded[0]).unwrap();
        let tags: String = m.connection().query_row("SELECT Tags FROM History WHERE Id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(tags, r#"{"Favorite":null}"#);
        m.delete(&loaded[1..]).unwrap();
        assert_eq!(m.load().unwrap().len(), 1);
        drop(m);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn doubles_as_dotnet_prints_them() {
        assert_eq!(dotnet_double(1.5), "1.5");
        assert_eq!(dotnet_double(1e20), "1E+20");
        assert_eq!(dotnet_double(42.0), "42");
    }
}
