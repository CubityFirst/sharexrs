//! `AmazonS3RemoteStorageProvider`: the Amazon S3 settings' bucket through
//! ShareX's `AmazonS3` class. Folders are key prefixes ending with `/`;
//! renaming copies then deletes (each object of a folder).

use super::*;
use crate::config::AmazonS3Settings;
use crate::destinations::amazon_s3::{AmazonS3, AmazonS3ObjectInfo};
use crate::http;
use crate::uploader::UploadError;

pub struct AmazonS3RemoteStorageProvider {
    client: AmazonS3,
    bucket: String,
    allow_upload: bool,
}

/// `NormalizeDirectoryPath`: `\` as `/`, a trailing slash, none in front.
fn normalize_directory_path(path: &str) -> String {
    let p = path.replace('\\', "/").trim_matches('/').to_owned();
    if p.is_empty() { p } else { format!("{p}/") }
}

/// `GetFileParentPath`.
fn file_parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..=i].to_owned(),
        None => String::new(),
    }
}

fn failed(e: UploadError) -> RemoteStorageError {
    match e {
        UploadError::Cancelled => RemoteStorageError::Cancelled,
        UploadError::Exception(m) => RemoteStorageError::Failed(m),
    }
}

impl AmazonS3RemoteStorageProvider {
    pub fn new(settings: AmazonS3Settings, allow_upload: bool) -> Self {
        let bucket = settings.Bucket.clone().unwrap_or_default();
        Self { client: AmazonS3::new(settings), bucket, allow_upload }
    }

    /// The `AmazonS3` client (tests fix its clock).
    pub fn client(&mut self) -> &mut AmazonS3 {
        &mut self.client
    }

    /// `CreateOperationException`: the uploader's errors so far, else
    /// "`action` failed.".
    fn operation_error(&self, action: &str) -> RemoteStorageError {
        let details = self.client.base.to_error_string();
        RemoteStorageError::InvalidOperation(if details.trim().is_empty() { format!("{action} failed.") } else { details })
    }

    fn list(&mut self, prefix: &str, delimiter: Option<&str>, max_keys: Option<i32>, action: &str) -> RemoteResult<Vec<AmazonS3ObjectInfo>> {
        match self.client.list_objects(prefix, delimiter, max_keys).map_err(failed)? {
            Some(objects) => Ok(objects),
            None => Err(self.operation_error(action)),
        }
    }

    fn delete_object(&mut self, key: &str) -> RemoteResult<bool> {
        self.client.delete_object(key).map_err(failed)
    }

    fn copy_object(&mut self, from: &str, to: &str) -> RemoteResult<bool> {
        self.client.copy_object(from, to).map_err(failed)
    }

    /// `RenameFolderAsync`: copies every object under the prefix (undone
    /// if one fails), then deletes the originals.
    fn rename_folder(&mut self, source_prefix: &str, destination_prefix: &str, cancel: &CancelToken) -> RemoteResult<()> {
        let objects = self.list(source_prefix, None, None, "Listing the folder contents")?;
        let sources: Vec<AmazonS3ObjectInfo> = objects.into_iter().filter(|o| !o.is_folder).collect();
        let mut copied: Vec<String> = Vec::new();
        let copy_all = (|| -> RemoteResult<()> {
            for source in &sources {
                cancel.check()?;
                let destination = format!("{destination_prefix}{}", &source.key[source_prefix.len().min(source.key.len())..]);
                if !self.copy_object(&source.key, &destination)? {
                    return Err(self.operation_error(&format!("Renaming '{}'", source.key)));
                }
                copied.push(destination);
            }
            Ok(())
        })();
        if let Err(e) = copy_all {
            for c in &copied {
                let _ = self.client.delete_object(c);
            }
            return Err(e);
        }
        for source in &sources {
            cancel.check()?;
            if !self.delete_object(&source.key)? {
                return Err(self.operation_error(&format!("Removing the original object '{}' after rename", source.key)));
            }
        }
        Ok(())
    }
}

impl RemoteStorageProvider for AmazonS3RemoteStorageProvider {
    fn display_name(&self) -> String {
        "Amazon S3".into()
    }

    fn root_display_name(&self) -> String {
        self.bucket.clone()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::all(self.allow_upload)
    }

    fn get_display_path(&self, path: &str) -> String {
        format!("s3://{}/{}", self.bucket, normalize_directory_path(path))
    }

    fn get_parent_path(&self, path: &str) -> Option<String> {
        let p = normalize_directory_path(path);
        if p.is_empty() {
            return None;
        }
        let without = p.trim_end_matches('/');
        Some(match without.rfind('/') {
            Some(i) => without[..=i].to_owned(),
            None => self.root_path(),
        })
    }

    fn get_items(&mut self, path: &str, cancel: &CancelToken) -> RemoteResult<Vec<RemoteStorageItem>> {
        let prefix = normalize_directory_path(path);
        let objects = self.list(&prefix, Some("/"), None, "Listing objects")?;
        let mut items: Vec<RemoteStorageItem> = Vec::new();
        for object in objects {
            cancel.check()?;
            if !object.key.starts_with(&prefix) || object.key == prefix {
                continue;
            }
            let relative = &object.key[prefix.len()..];
            let name = relative.trim_end_matches('/');
            if name.is_empty() || name.contains('/') {
                continue;
            }
            let is_folder = object.is_folder || object.key.ends_with('/');
            let object_path = if is_folder { normalize_directory_path(&object.key) } else { object.key.clone() };
            let item = RemoteStorageItem {
                name: name.to_owned(),
                path: object_path.clone(),
                is_folder,
                size: if is_folder { None } else { object.size },
                modified: if is_folder { None } else { object.last_modified },
            };
            // A dictionary keyed by path: a later duplicate replaces an
            // earlier one only when it has a date and the earlier hasn't.
            match items.iter_mut().find(|i| i.path == object_path) {
                Some(existing) => {
                    if existing.modified.is_none() && item.modified.is_some() {
                        *existing = item;
                    }
                }
                None => items.push(item),
            }
        }
        sort_items(&mut items);
        Ok(items)
    }

    fn download(&mut self, item: &RemoteStorageItem, destination: &mut dyn std::io::Write, _cancel: &CancelToken) -> RemoteResult<()> {
        validate_file(item)?;
        if !self.client.download_object(&item.path, destination).map_err(failed)? {
            return Err(self.operation_error("Downloading the object"));
        }
        Ok(())
    }

    fn upload(&mut self, directory_path: &str, file_name: &str, source: &mut dyn std::io::Read, _cancel: &CancelToken) -> RemoteResult<()> {
        if !self.allow_upload {
            return Err(RemoteStorageError::InvalidOperation("Uploading is disabled.".into()));
        }
        validate_name(file_name)?;
        let key = format!("{}{file_name}", normalize_directory_path(directory_path));
        let mut bytes = Vec::new();
        source.read_to_end(&mut bytes).map_err(|e| RemoteStorageError::Failed(e.to_string()))?;
        let data = http::shared_stream(std::io::Cursor::new(bytes));
        if !self.client.upload_object(&data, &key).map_err(failed)? {
            return Err(self.operation_error("Uploading the object"));
        }
        Ok(())
    }

    fn create_folder(&mut self, directory_path: &str, folder_name: &str, _cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(folder_name)?;
        let key = format!("{}{folder_name}/", normalize_directory_path(directory_path));
        let data = http::shared_stream(std::io::Cursor::new(Vec::new()));
        if !self.client.upload_object(&data, &key).map_err(failed)? {
            return Err(self.operation_error("Creating the folder"));
        }
        Ok(())
    }

    fn rename(&mut self, item: &RemoteStorageItem, new_name: &str, cancel: &CancelToken) -> RemoteResult<()> {
        validate_name(new_name)?;
        let parent = if item.is_folder { self.get_parent_path(&item.path).unwrap_or_else(|| self.root_path()) } else { file_parent_path(&item.path) };
        if item.is_folder {
            let source = normalize_directory_path(&item.path);
            let destination = normalize_directory_path(&format!("{parent}{new_name}"));
            return self.rename_folder(&source, &destination, cancel);
        }
        let destination = format!("{parent}{new_name}");
        if !self.copy_object(&item.path, &destination)? {
            return Err(self.operation_error("Renaming the object"));
        }
        if !self.delete_object(&item.path)? {
            let _ = self.client.delete_object(&destination);
            return Err(self.operation_error("Removing the original object after rename"));
        }
        Ok(())
    }

    fn has_children(&mut self, folder: &RemoteStorageItem, _cancel: &CancelToken) -> RemoteResult<bool> {
        validate_folder(folder)?;
        let prefix = normalize_directory_path(&folder.path);
        let objects = self.list(&prefix, None, Some(2), "Checking the folder contents")?;
        Ok(objects.iter().any(|o| o.key != prefix))
    }

    fn delete(&mut self, item: &RemoteStorageItem, cancel: &CancelToken) -> RemoteResult<()> {
        if !item.is_folder {
            if !self.delete_object(&item.path)? {
                return Err(self.operation_error("Deleting the object"));
            }
            return Ok(());
        }
        let objects = self.list(&normalize_directory_path(&item.path), None, None, "Listing the folder contents")?;
        for child in objects.iter().filter(|o| !o.is_folder) {
            cancel.check()?;
            if !self.delete_object(&child.key)? {
                return Err(self.operation_error(&format!("Deleting '{}'", child.key)));
            }
        }
        Ok(())
    }

    fn get_url(&self, item: &RemoteStorageItem) -> Option<String> {
        if item.is_folder { None } else { Some(self.client.generate_url(&item.path)) }
    }
}
