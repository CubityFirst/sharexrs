//! `SFTPRemoteStorageProvider`: an SFTP account through ShareX's `SFTP`
//! class (SSH.NET's `SftpClient`, its async calls), one session kept for
//! the window's life. Paths are relative to the login directory.

use super::*;
use crate::destinations::ftp::FTPAccount;
use crate::destinations::sftp::{Sftp, SftpSession};
use crate::sftp::{DirEntry, SftpError};
use crate::uploader::UploadError;

pub struct SftpRemoteStorageProvider {
    account: FTPAccount,
    client: Sftp,
    allow_upload: bool,
    display_name: String,
    root_display_name: String,
}

/// `NormalizePath`: no slashes at the ends.
fn normalize_path(path: &str) -> String {
    path.trim_matches('/').to_owned()
}

/// `ToRemotePath`: `.` for the login directory.
fn to_remote_path(path: &str) -> String {
    if path.is_empty() { ".".into() } else { path.to_owned() }
}

fn combine_path(directory: &str, name: &str) -> String {
    let d = normalize_path(directory);
    if d.is_empty() { name.to_owned() } else { format!("{d}/{name}") }
}

/// `SftpFile.Name`: after the last slash.
fn file_name(e: &DirEntry) -> &str {
    e.name.rsplit('/').next().unwrap_or(&e.name)
}

fn is_browsable(e: &DirEntry) -> bool {
    file_name(e) != "." && file_name(e) != ".."
}

fn failed(e: SftpError) -> RemoteStorageError {
    RemoteStorageError::Failed(e.to_string())
}

impl SftpRemoteStorageProvider {
    pub fn new(account: FTPAccount, allow_upload: bool) -> Self {
        let name = account_name(&account);
        let client = Sftp::new(account.clone());
        Self { display_name: format!("SFTP - {name}"), root_display_name: name, account, client, allow_upload }
    }

    /// `EnsureConnectedAsync`: no credentials is an error.
    fn session(&mut self) -> RemoteResult<&mut SftpSession> {
        let session = self.client.session().map_err(|e| match e {
            UploadError::Cancelled => RemoteStorageError::Cancelled,
            UploadError::Exception(m) => RemoteStorageError::Failed(m),
        })?;
        session.ok_or_else(|| RemoteStorageError::InvalidOperation("SFTP account does not contain valid authentication credentials.".into()))
    }

    fn list(&mut self, path: &str) -> RemoteResult<Vec<DirEntry>> {
        self.session()?.list_directory_async(path).map_err(failed)
    }

    /// `DeleteDirectoryAsync`: what's in it first (folders the same way),
    /// then the folder.
    fn delete_directory(&mut self, directory: &str, cancel: &CancelToken) -> RemoteResult<()> {
        let entries = self.list(&to_remote_path(directory))?;
        for e in entries.iter().filter(|e| is_browsable(e)) {
            cancel.check()?;
            let child = combine_path(directory, file_name(e));
            if e.attrs.is_dir() && !e.attrs.is_symlink() {
                self.delete_directory(&child, cancel)?;
            } else {
                self.session()?.delete_file_async(&to_remote_path(&child)).map_err(failed)?;
            }
        }
        self.session()?.delete_directory_async(&to_remote_path(directory)).map_err(failed)
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
                name: file_name(e).to_owned(),
                path: combine_path(&directory, file_name(e)),
                is_folder: e.attrs.is_dir() && !e.attrs.is_symlink(),
                size: if e.attrs.is_dir() { None } else { Some(e.attrs.size.map(|s| s as i64).unwrap_or(-1)) },
                modified: e.attrs.mtime.and_then(|t| chrono::DateTime::from_timestamp(i64::from(t), 0)).map(|d| d.fixed_offset()),
            })
            .collect();
        sort_items(&mut items);
        Ok(items)
    }

    fn download(&mut self, item: &RemoteStorageItem, destination: &mut dyn std::io::Write, cancel: &CancelToken) -> RemoteResult<()> {
        validate_file(item)?;
        cancel.check()?;
        let path = to_remote_path(&item.path);
        let buffer_size = self.client.base.buffer_size;
        self.session()?.download_file_async(&path, destination, buffer_size).map_err(failed)
    }

    fn upload(&mut self, directory_path: &str, file_name: &str, source: &mut dyn std::io::Read, cancel: &CancelToken) -> RemoteResult<()> {
        if !self.allow_upload {
            return Err(RemoteStorageError::InvalidOperation("Uploading is disabled.".into()));
        }
        validate_name(file_name)?;
        cancel.check()?;
        let path = to_remote_path(&combine_path(&normalize_path(directory_path), file_name));
        let buffer_size = self.client.base.buffer_size;
        self.session()?.upload_file_async(source, &path, buffer_size).map_err(failed)
    }

    fn create_folder(&mut self, directory_path: &str, folder_name: &str, cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(folder_name)?;
        cancel.check()?;
        let path = to_remote_path(&combine_path(&normalize_path(directory_path), folder_name));
        self.session()?.create_directory_async(&path).map_err(failed)
    }

    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(new_name)?;
        cancel.check()?;
        let parent = self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path());
        let destination = combine_path(&parent, new_name);
        let (from, to) = (to_remote_path(&item.path), to_remote_path(&destination));
        self.session()?.rename_async(&from, &to).map_err(failed)
    }

    fn has_children(&mut self, folder: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<bool> {
        validate_folder(folder)?;
        cancel.check()?;
        let path = to_remote_path(&folder.path);
        self.session()?.directory_has_items_async(&path).map_err(failed)
    }

    fn delete(&mut self, item: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<()> {
        cancel.check()?;
        if item.is_folder {
            self.delete_directory(&normalize_path(&item.path), cancel)
        } else {
            let path = to_remote_path(&item.path);
            self.session()?.delete_file_async(&path).map_err(failed)
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
