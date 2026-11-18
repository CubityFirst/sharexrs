//! The remote storage browser's data side (Tools › Remote storage
//! browser): ShareX's `IRemoteStorageProvider` (`ShareX.Tools`'
//! `RemoteStorageContracts.cs`) and its three providers in `ShareX.exe`
//! (`ShareX/RemoteStorage`): FTP/FTPS, SFTP and Amazon S3 (and
//! S3-compatible services). [`providers`] makes the list
//! `TaskHelpers.OpenRemoteStorageBrowser` makes.
//!
//! Every call blocks (ShareX's are `Task`s run on the thread pool): the
//! window runs them on a worker thread and can cancel them with a
//! [`CancelToken`], checked before and after each call as ShareX's
//! `RunAsync` does.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Offset, TimeZone};

use crate::UploadersConfig;
use crate::enums::FTPProtocol;

mod ftp;
mod s3;
mod sftp;

pub use ftp::FtpRemoteStorageProvider;
pub use s3::AmazonS3RemoteStorageProvider;
pub use sftp::SftpRemoteStorageProvider;

/// `RemoteStorageProviderCapabilities` (flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities(pub u32);

impl Capabilities {
    pub const NONE: Capabilities = Capabilities(0);
    pub const DOWNLOAD: Capabilities = Capabilities(1);
    pub const UPLOAD: Capabilities = Capabilities(2);
    pub const RENAME: Capabilities = Capabilities(4);
    pub const DELETE: Capabilities = Capabilities(8);
    pub const URL: Capabilities = Capabilities(16);
    pub const CREATE_FOLDER: Capabilities = Capabilities(32);

    pub fn contains(self, other: Capabilities) -> bool {
        self.0 & other.0 == other.0
    }

    /// What every provider can do, with upload when it's allowed.
    fn all(allow_upload: bool) -> Self {
        let upload = if allow_upload { Self::UPLOAD.0 } else { 0 };
        Capabilities(Self::DOWNLOAD.0 | upload | Self::CREATE_FOLDER.0 | Self::RENAME.0 | Self::DELETE.0 | Self::URL.0)
    }
}

/// `RemoteStorageItem`: a file or folder of a listing.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteStorageItem {
    pub name: String,
    /// The provider's path of the item (no leading slash; S3 folders end
    /// with one).
    pub path: String,
    pub is_folder: bool,
    pub size: Option<i64>,
    pub modified: Option<DateTime<FixedOffset>>,
}

impl RemoteStorageItem {
    pub fn new(name: &str, path: &str, is_folder: bool, size: Option<i64>) -> Self {
        Self { name: name.to_owned(), path: path.to_owned(), is_folder, size, modified: None }
    }
}

/// What a provider call throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteStorageError {
    /// `ArgumentException(message, paramName)`.
    Argument { message: String, param: String },
    /// `InvalidOperationException`: "Connecting to the server failed.",
    /// "Uploading is disabled.", S3's error texts.
    InvalidOperation(String),
    /// Anything the client library threw (FluentFTP, SSH.NET, HTTP), by
    /// its message.
    Failed(String),
    /// The token was cancelled (`OperationCanceledException`).
    Cancelled,
}

impl std::fmt::Display for RemoteStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteStorageError::Argument { message, param } => write!(f, "{message} (Parameter '{param}')"),
            RemoteStorageError::InvalidOperation(m) | RemoteStorageError::Failed(m) => f.write_str(m),
            RemoteStorageError::Cancelled => f.write_str("The operation was canceled."),
        }
    }
}

impl std::error::Error for RemoteStorageError {}

pub type RemoteResult<T> = Result<T, RemoteStorageError>;

fn argument(message: &str, param: &str) -> RemoteStorageError {
    RemoteStorageError::Argument { message: message.to_owned(), param: param.to_owned() }
}

/// A cancellation token (`CancellationToken`).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    /// `CancellationTokenSource.Cancel()`.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    /// `ThrowIfCancellationRequested`.
    pub fn check(&self) -> RemoteResult<()> {
        if self.is_cancelled() { Err(RemoteStorageError::Cancelled) } else { Ok(()) }
    }
}

/// `IRemoteStorageProvider`.
pub trait RemoteStorageProvider: Send {
    /// `DisplayName`: the provider list's entry.
    fn display_name(&self) -> String;
    /// `RootDisplayName`: the tree's root.
    fn root_display_name(&self) -> String;
    /// `RootPath` (empty for all three).
    fn root_path(&self) -> String {
        String::new()
    }
    fn capabilities(&self) -> Capabilities;
    /// `GetDisplayPath`: the address bar's text for a path.
    fn get_display_path(&self, path: &str) -> String;
    /// `GetParentPath`: `None` at the root.
    fn get_parent_path(&self, path: &str) -> Option<String>;
    /// `GetItemsAsync`: folders first, then files, by name (current
    /// culture, ignoring case).
    fn get_items(&mut self, path: &str, cancel: &CancelToken) -> RemoteResult<Vec<RemoteStorageItem>>;
    /// `DownloadAsync`: the file's bytes into `destination`.
    fn download(&mut self, item: &RemoteStorageItem, destination: &mut dyn std::io::Write, cancel: &CancelToken) -> RemoteResult<()>;
    /// `UploadAsync`: `source` as `file_name` in the folder.
    fn upload(&mut self, directory_path: &str, file_name: &str, source: &mut dyn std::io::Read, cancel: &CancelToken) -> RemoteResult<()>;
    /// `CreateFolderAsync`.
    fn create_folder(&mut self, directory_path: &str, folder_name: &str, cancel: &CancelToken) -> RemoteResult<()>;
    /// `RenameAsync` (in the same folder).
    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, cancel: &CancelToken) -> RemoteResult<()>;
    /// `HasChildrenAsync`: whether the tree shows an expander.
    fn has_children(&mut self, folder: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<bool>;
    /// `DeleteAsync` (folders with what's in them).
    fn delete(&mut self, item: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<()>;
    /// `GetUrl`: the file's public URL (`None` for folders).
    fn get_url(&self, item: &RemoteStorageItem) -> Option<String>;
    /// `Dispose()` (FTP: `QUIT`; SFTP: the session ends). Also on drop.
    fn dispose(&mut self) {}
}

/// `TaskHelpers.OpenRemoteStorageBrowser`'s providers: Amazon S3 when its
/// settings are complete, then each FTP account that's usable
/// (`UploadersConfigValidator.IsValidRemoteStorageAccount`). Empty: ShareX
/// says `TaskHelpers_RemoteStorageNotConfigured` instead of opening the
/// window. `allow_upload` is `!SystemOptions.DisableUpload`.
pub fn providers(config: &UploadersConfig, allow_upload: bool) -> Vec<Box<dyn RemoteStorageProvider>> {
    let mut list: Vec<Box<dyn RemoteStorageProvider>> = Vec::new();
    if crate::validator::file(&crate::enums::FileDestination::AmazonS3, config) {
        list.push(Box::new(AmazonS3RemoteStorageProvider::new(config.AmazonS3Settings.clone(), allow_upload)));
    }
    let accounts = crate::destinations::ftp::FTPAccount::list_from_value(&config.FTPAccountList).unwrap_or_default();
    for account in accounts {
        if !crate::validator::is_valid_remote_storage_account(Some(&account)) {
            continue;
        }
        if matches!(account.Protocol, FTPProtocol::FTP | FTPProtocol::FTPS) {
            if let Ok(p) = FtpRemoteStorageProvider::new(account, allow_upload) {
                list.push(Box::new(p));
            }
        } else {
            list.push(Box::new(SftpRemoteStorageProvider::new(account, allow_upload)));
        }
    }
    list
}

/// `ValidateName`: not blank, not `.`/`..`, no slashes.
fn validate_name(name: &str) -> RemoteResult<()> {
    if name.trim().is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(argument("The name is not valid.", "name"));
    }
    Ok(())
}

/// `ValidateFile`.
fn validate_file(item: &RemoteStorageItem) -> RemoteResult<()> {
    if item.is_folder {
        return Err(argument("The operation requires a file.", "item"));
    }
    Ok(())
}

/// `HasChildrenAsync`'s check.
fn validate_folder(folder: &RemoteStorageItem) -> RemoteResult<()> {
    if !folder.is_folder {
        return Err(argument("The operation requires a folder.", "folder"));
    }
    Ok(())
}

/// The account's name in the provider list: `Name (Host:Port)`, or
/// `Host:Port` without a name.
fn account_name(account: &crate::destinations::ftp::FTPAccount) -> String {
    let host = account.Host.as_deref().unwrap_or("");
    match account.Name.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(n) => format!("{n} ({host}:{})", account.Port),
        None => format!("{host}:{}", account.Port),
    }
}

/// `OrderByDescending(IsFolder).ThenBy(Name, CurrentCultureIgnoreCase)`
/// (stable).
fn sort_items(items: &mut [RemoteStorageItem]) {
    items.sort_by(|a, b| b.is_folder.cmp(&a.is_folder).then_with(|| sharex_helpers::culture::current_culture_ignore_case_cmp(&a.name, &b.name)));
}

/// `new DateTimeOffset(dateTime)`: a UTC date at +00:00, a local one at
/// this machine's offset then.
fn date_time_offset(value: NaiveDateTime, utc: bool) -> DateTime<FixedOffset> {
    if utc {
        return value.and_utc().fixed_offset();
    }
    match Local.from_local_datetime(&value) {
        chrono::LocalResult::Single(d) => d.fixed_offset(),
        // Ambiguous (the hour the clocks go back): the standard offset.
        chrono::LocalResult::Ambiguous(a, b) => {
            let (a, b) = (a.fixed_offset(), b.fixed_offset());
            if a.offset().local_minus_utc() <= b.offset().local_minus_utc() { a } else { b }
        }
        // Skipped (the clocks go forward): the offset before the change.
        chrono::LocalResult::None => {
            let before = value - chrono::Duration::hours(1);
            let offset = Local.from_local_datetime(&before).earliest().map(|d| d.offset().fix()).unwrap_or_else(|| Local::now().offset().fix());
            DateTime::from_naive_utc_and_offset(value - chrono::Duration::seconds(i64::from(offset.local_minus_utc())), offset)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_errors() {
        assert!(validate_name("a.txt").is_ok());
        for bad in ["", " ", ".", "..", "a/b", "a\\b"] {
            assert_eq!(validate_name(bad).unwrap_err().to_string(), "The name is not valid. (Parameter 'name')");
        }
        assert_eq!(Capabilities::all(true).0, 63);
        assert_eq!(Capabilities::all(false).0, 61);
    }

    #[test]
    fn sorted_folders_first() {
        let mut items = vec![
            RemoteStorageItem::new("Zeta.txt", "Zeta.txt", false, None),
            RemoteStorageItem::new("docs", "docs", true, None),
            RemoteStorageItem::new("alpha.txt", "alpha.txt", false, None),
            RemoteStorageItem::new("Empty", "Empty", true, None),
        ];
        sort_items(&mut items);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["docs", "Empty", "alpha.txt", "Zeta.txt"]);
    }
}
