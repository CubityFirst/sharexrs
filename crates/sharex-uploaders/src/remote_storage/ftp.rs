//! `FTPRemoteStorageProvider`: an FTP/FTPS account through ShareX's `FTP`
//! class (FluentFTP), one connection kept for the window's life.

use super::*;
use crate::destinations::ftp::{FTPAccount, Ftp, FtpError};
use crate::destinations::ftp_list::{FtpListItem, FtpObjectType};
use crate::http;
use crate::uploader::UploadError;
use crate::url_helpers;

pub struct FtpRemoteStorageProvider {
    account: FTPAccount,
    client: Ftp,
    allow_upload: bool,
    display_name: String,
    root_display_name: String,
}

fn failed(e: FtpError) -> RemoteStorageError {
    RemoteStorageError::Failed(e.to_string())
}

fn upload_failed(e: UploadError) -> RemoteStorageError {
    match e {
        UploadError::Cancelled => RemoteStorageError::Cancelled,
        UploadError::Exception(m) => RemoteStorageError::Failed(m),
    }
}

/// `NormalizePath`: `\` as `/`, no slashes at the ends.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_matches('/').to_owned()
}

/// `ToRemotePath`: absolute.
fn to_remote_path(path: &str) -> String {
    let p = normalize_path(path);
    if p.is_empty() { "/".into() } else { format!("/{p}") }
}

/// `CombinePath`.
fn combine_path(directory: &str, name: &str) -> String {
    let d = normalize_path(directory);
    if d.is_empty() { name.to_owned() } else { format!("{d}/{name}") }
}

/// `IsBrowsableItem`: files and folders, not `.`/`..`.
fn is_browsable(item: &FtpListItem) -> bool {
    let name = item.name();
    name.as_deref() != Some(".") && name.as_deref() != Some("..") && matches!(item.kind, FtpObjectType::File | FtpObjectType::Directory)
}

impl FtpRemoteStorageProvider {
    /// `new FTPRemoteStorageProvider(account, allowUpload)`: FTP and FTPS
    /// accounts only.
    pub fn new(account: FTPAccount, allow_upload: bool) -> RemoteResult<Self> {
        if !matches!(account.Protocol, FTPProtocol::FTP | FTPProtocol::FTPS) {
            return Err(argument("The account must use FTP or FTPS.", "account"));
        }
        let protocol = if account.Protocol == FTPProtocol::FTPS { "FTPS" } else { "FTP" };
        let name = account_name(&account);
        let client = Ftp::new(account.clone());
        Ok(Self { display_name: format!("{protocol} - {name}"), root_display_name: name, account, client, allow_upload })
    }

    /// `EnsureConnected`: ShareX's `FTP.Connect()`.
    fn ensure_connected(&mut self) -> RemoteResult<()> {
        let c = self.client.client();
        if !c.is_connected() {
            c.connect().map_err(failed)?;
        }
        if !c.is_connected() {
            return Err(RemoteStorageError::InvalidOperation("Connecting to the server failed.".into()));
        }
        Ok(())
    }

    /// `RunAsync`: the token before and after.
    fn run<T>(&mut self, cancel: &CancelToken, f: impl FnOnce(&mut Self) -> RemoteResult<T>) -> RemoteResult<T> {
        cancel.check()?;
        let r = f(self)?;
        cancel.check()?;
        Ok(r)
    }

    /// ShareX's `FTP.DeleteDirectory`: what's listed (its folders the same
    /// way, its files one by one), then FluentFTP's own recursive delete.
    fn delete_directory(&mut self, remote_path: &str) -> RemoteResult<()> {
        let name = url_helpers::get_file_name(remote_path);
        if name == "." || name == ".." {
            return Ok(());
        }
        let files = self.client.client().get_listing(remote_path).map_err(failed)?;
        for file in files {
            if file.name().is_none_or(|n| n.is_empty()) {
                continue;
            }
            let full = file.full_name.clone().unwrap_or_default();
            match file.kind {
                FtpObjectType::Directory => self.delete_directory(&full)?,
                FtpObjectType::File => self.client.client().delete_file(&full).map_err(failed)?,
                FtpObjectType::Link => {}
            }
        }
        self.client.client().delete_directory(remote_path).map_err(failed)
    }
}

impl RemoteStorageProvider for FtpRemoteStorageProvider {
    fn display_name(&self) -> String {
        self.display_name.clone()
    }

    fn root_display_name(&self) -> String {
        self.root_display_name.clone()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::all(self.allow_upload)
    }

    fn get_display_path(&self, path: &str) -> String {
        let address = self.account.ftp_address();
        let address = address.trim_end_matches('/');
        let p = normalize_path(path);
        if p.is_empty() { format!("{address}/") } else { format!("{address}/{p}") }
    }

    fn get_parent_path(&self, path: &str) -> Option<String> {
        let p = normalize_path(path);
        if p.is_empty() {
            return None;
        }
        Some(match p.rfind('/') {
            Some(i) => p[..i].to_owned(),
            None => self.root_path(),
        })
    }

    fn get_items(&mut self, path: &str, cancel: &CancelToken) -> RemoteResult<Vec<RemoteStorageItem>> {
        let directory = normalize_path(path);
        self.run(cancel, |s| {
            s.ensure_connected()?;
            let listing = s.client.client().get_listing(&to_remote_path(&directory)).map_err(failed)?;
            let mut items: Vec<RemoteStorageItem> = listing
                .iter()
                .filter(|f| is_browsable(f))
                .map(|f| {
                    let name = f.name().unwrap_or_default();
                    RemoteStorageItem {
                        path: combine_path(&directory, &name),
                        name,
                        is_folder: f.kind == FtpObjectType::Directory,
                        size: (f.kind == FtpObjectType::File && f.size >= 0).then_some(f.size),
                        modified: f.modified.map(|m| date_time_offset(m, f.modified_utc)),
                    }
                })
                .collect();
            sort_items(&mut items);
            Ok(items)
        })
    }

    fn download(&mut self, item: &RemoteStorageItem, destination: &mut dyn std::io::Write, cancel: &CancelToken) -> RemoteResult<()> {
        validate_file(item)?;
        let path = to_remote_path(&item.path);
        self.run(cancel, |s| {
            s.ensure_connected()?;
            s.client.client().download(&path, destination).map_err(failed)
        })
    }

    fn upload(&mut self, directory_path: &str, file_name: &str, source: &mut dyn std::io::Read, cancel: &CancelToken) -> RemoteResult<()> {
        if !self.allow_upload {
            return Err(RemoteStorageError::InvalidOperation("Uploading is disabled.".into()));
        }
        validate_name(file_name)?;
        let remote_path = to_remote_path(&combine_path(&normalize_path(directory_path), file_name));
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes).map_err(|e| RemoteStorageError::Failed(e.to_string()))?;
        self.run(cancel, |s| {
            s.ensure_connected()?;
            let data = http::shared_stream(std::io::Cursor::new(bytes));
            if !s.client.upload_data(&data, &remote_path).map_err(upload_failed)? {
                return Err(RemoteStorageError::InvalidOperation("Uploading the file failed.".into()));
            }
            Ok(())
        })
    }

    fn create_folder(&mut self, directory_path: &str, folder_name: &str, cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(folder_name)?;
        let remote_path = to_remote_path(&combine_path(&normalize_path(directory_path), folder_name));
        self.run(cancel, |s| {
            s.ensure_connected()?;
            if !s.client.create_directory(&remote_path) {
                return Err(RemoteStorageError::InvalidOperation("Creating the folder failed.".into()));
            }
            Ok(())
        })
    }

    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(new_name)?;
        let parent = self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path());
        let destination = combine_path(&parent, new_name);
        let (from, to) = (to_remote_path(&item.path), to_remote_path(&destination));
        self.run(cancel, |s| {
            s.ensure_connected()?;
            s.client.client().rename(&from, &to).map_err(failed)
        })
    }

    fn has_children(&mut self, folder: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<bool> {
        validate_folder(folder)?;
        let path = to_remote_path(&folder.path);
        self.run(cancel, |s| {
            s.ensure_connected()?;
            Ok(s.client.client().get_listing(&path).map_err(failed)?.iter().any(is_browsable))
        })
    }

    fn delete(&mut self, item: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<()> {
        let remote_path = to_remote_path(&item.path);
        let is_folder = item.is_folder;
        self.run(cancel, |s| {
            s.ensure_connected()?;
            if is_folder { s.delete_directory(&remote_path) } else { s.client.client().delete_file(&remote_path).map_err(failed) }
        })
    }

    fn get_url(&self, item: &RemoteStorageItem) -> Option<String> {
        if item.is_folder {
            return None;
        }
        let parent = self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path());
        Some(self.account.get_uri_path(&item.name, Some(&parent)))
    }

    fn dispose(&mut self) {
        self.client.client().disconnect();
    }
}

impl Drop for FtpRemoteStorageProvider {
    fn drop(&mut self) {
        self.dispose();
    }
}
