//! `SFTPRemoteStorageProvider`: an SFTP account through ShareX's `SFTP`
//! class (SSH.NET's `SftpClient`), one session kept for the window's life.

use super::*;
use crate::destinations::ftp::FTPAccount;
use crate::destinations::sftp::Sftp;
use crate::sftp::{DirEntry, SftpError};
use crate::uploader::UploadError;

pub struct SftpRemoteStorageProvider {
    account: FTPAccount,
    client: Sftp,
    allow_upload: bool,
    display_name: String,
    root_display_name: String,
}

/// `NormalizePath`: no slashes at the ends (backslashes kept).
fn normalize_path(path: &str) -> String {
    path.trim_matches('/').to_owned()
}

/// `ToRemotePath`: relative to the login directory, `.` for it.
fn to_remote_path(path: &str) -> String {
    if path.is_empty() { ".".into() } else { path.to_owned() }
}

fn combine_path(directory: &str, name: &str) -> String {
    let d = normalize_path(directory);
    if d.is_empty() { name.to_owned() } else { format!("{d}/{name}") }
}

fn is_browsable(e: &DirEntry) -> bool {
    e.name != "." && e.name != ".."
}

fn failed(e: SftpError) -> RemoteStorageError {
    RemoteStorageError::Failed(e.to_string())
}

fn upload_failed(e: UploadError) -> RemoteStorageError {
    match e {
        UploadError::Cancelled => RemoteStorageError::Cancelled,
        UploadError::Exception(m) => RemoteStorageError::Failed(m),
    }
}

impl SftpRemoteStorageProvider {
    pub fn new(account: FTPAccount, allow_upload: bool) -> Self {
        let name = account_name(&account);
        let client = Sftp::new(account.clone());
        Self { display_name: format!("SFTP - {name}"), root_display_name: name, account, client, allow_upload }
    }

    /// `EnsureConnectedAsync`: no credentials is an error.
    fn session(&mut self) -> RemoteResult<&mut crate::destinations::sftp::SftpSession> {
        match self.client.session().map_err(upload_failed)? {
            Some(s) => Ok(s),
            None => Err(RemoteStorageError::InvalidOperation("SFTP account does not contain valid authentication credentials.".into())),
        }
    }

    fn list(&mut self, path: &str) -> RemoteResult<Vec<DirEntry>> {
        self.session()?.list_directory(path).map_err(failed)
    }

    /// `DeleteDirectoryAsync` of the provider: what's in it first.
    fn delete_directory(&mut self, directory: &str, cancel: &CancelToken) -> RemoteResult<()> {
        let entries = self.list(&to_remote_path(directory))?;
        for e in entries.iter().filter(|e| is_browsable(e)) {
            cancel.check()?;
            let child = combine_path(directory, &e.name);
            if e.attrs.is_dir() && !e.attrs.is_symlink() {
                self.delete_directory(&child, cancel)?;
            } else {
                self.session()?.delete_file(&to_remote_path(&child)).map_err(failed)?;
            }
        }
        self.session()?.delete_directory(&to_remote_path(directory)).map_err(failed)
    }
}

impl RemoteStorageProvider for SftpRemoteStorageProvider {
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
        cancel.check()?;
        let directory = normalize_path(path);
        let entries = self.list(&to_remote_path(&directory))?;
        let mut items: Vec<RemoteStorageItem> = entries
            .iter()
            .filter(|e| is_browsable(e))
            .map(|e| RemoteStorageItem {
                name: e.name.clone(),
                path: combine_path(&directory, &e.name),
                is_folder: e.attrs.is_dir() && !e.attrs.is_symlink(),
                size: if e.attrs.is_dir() { None } else { Some(e.attrs.size.unwrap_or(0) as i64) },
                modified: e.attrs.mtime.and_then(|t| chrono::DateTime::from_timestamp(i64::from(t), 0)).map(|d| d.fixed_offset()),
            })
            .collect();
        sort_items(&mut items);
        Ok(items)
    }

    fn download(&mut self, item: &RemoteStorageItem, destination: &mut dyn std::io::Write, _cancel: &CancelToken) -> RemoteResult<()> {
        validate_file(item)?;
        let path = to_remote_path(&item.path);
        self.session()?.download_file(&path, destination).map_err(failed)
    }

    fn upload(&mut self, directory_path: &str, file_name: &str, source: &mut dyn std::io::Read, _cancel: &CancelToken) -> RemoteResult<()> {
        if !self.allow_upload {
            return Err(RemoteStorageError::InvalidOperation("Uploading is disabled.".into()));
        }
        validate_name(file_name)?;
        let path = to_remote_path(&combine_path(&normalize_path(directory_path), file_name));
        let buffer_size = self.client.base.buffer_size;
        self.session()?.upload_stream(source, &path, buffer_size).map_err(failed)
    }

    fn create_folder(&mut self, directory_path: &str, folder_name: &str, _cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(folder_name)?;
        let path = to_remote_path(&combine_path(&normalize_path(directory_path), folder_name));
        self.session()?.create_directory(&path).map_err(failed)
    }

    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, _cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(new_name)?;
        let parent = self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path());
        let destination = combine_path(&parent, new_name);
        let (from, to) = (to_remote_path(&item.path), to_remote_path(&destination));
        self.session()?.rename(&from, &to).map_err(failed)
    }

    fn has_children(&mut self, folder: &RemoteStorageItem, _cancel: &CancelToken) -> RemoteResult<bool> {
        validate_folder(folder)?;
        let path = to_remote_path(&folder.path);
        Ok(self.list(&path)?.iter().any(is_browsable))
    }

    fn delete(&mut self, item: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<()> {
        if item.is_folder {
            self.delete_directory(&normalize_path(&item.path), cancel)
        } else {
            let path = to_remote_path(&item.path);
            self.session()?.delete_file(&path).map_err(failed)
        }
    }

    fn get_url(&self, item: &RemoteStorageItem) -> Option<String> {
        if item.is_folder {
            return None;
        }
        let parent = self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path());
        Some(self.account.get_uri_path(&item.name, Some(&parent)))
    }

    fn dispose(&mut self) {
        self.client.disconnect();
    }
}

impl Drop for SftpRemoteStorageProvider {
    fn drop(&mut self) {
        self.dispose();
    }
}
