//! The built-in destinations ported so far, by kind. Each follows its
//! ShareX class line for line (named in its doc comment); the factory
//! ([`crate::factory`]) builds them from `UploadersConfig` as ShareX's
//! `*UploaderService.CreateUploader` does.

pub mod amazon_s3;
pub mod cloud_storage;
pub mod email;
pub mod file_hosts;
pub mod files;
pub mod ftp;
pub mod ftp_list;
pub mod mega;
pub mod oauth_services;
pub mod images;
pub mod private_bin;
pub mod self_hosted;
pub mod shared_folder;
pub mod services;
pub mod sftp;
pub mod sftp_keys;
pub mod sharing;
pub mod shorteners;
pub mod text;

use serde_json::Value;

use crate::uploader::{UResult, UploadError};

/// `NullReferenceException.Message`, for the places ShareX's code would
/// dereference null.
pub(crate) const NULL_REFERENCE: &str = "Object reference not set to an instance of an object.";

/// `JsonConvert.DeserializeObject<T>(text)` for a class `T`: `None` for an
/// empty text or `null`; an exception for a null text (`ArgumentNullException`),
/// invalid JSON or a value that isn't an object.
pub(crate) fn deserialize_object(text: Option<&str>) -> UResult<Option<Value>> {
    let Some(text) = text else { return Err(UploadError::Exception("Value cannot be null. (Parameter 'value')".into())) };
    match crate::json::deserialize(text) {
        Ok(None) => Ok(None),
        Ok(Some(v)) if v.is_object() => Ok(Some(v)),
        Ok(Some(_)) => Err(UploadError::Exception("Cannot deserialize the current JSON value into an object.".into())),
        Err(e) => Err(UploadError::Exception(e)),
    }
}

/// `string.IsNullOrEmpty`.
pub(crate) fn null_or_empty(s: Option<&str>) -> bool {
    s.is_none_or(str::is_empty)
}

/// Implements the `uploader()` accessor of the uploader traits for a type
/// with a `base: Uploader` field.
macro_rules! uploader_base {
    ($t:ty, $trait:ident) => {
        impl $crate::uploader::$trait for $t {
            fn uploader(&mut self) -> &mut $crate::uploader::Uploader {
                &mut self.base
            }
            uploader_base!(@core $trait);
        }
    };
    (@core FileUploader) => {
        fn upload_core(&mut self, data: &$crate::http::SharedStream, file_name: &str) -> $crate::uploader::CoreResult {
            self.upload(data, file_name)
        }
    };
    (@core TextUploader) => {
        fn upload_text_core(&mut self, text: &str, file_name: &str) -> $crate::uploader::CoreResult {
            self.upload_text(text, file_name)
        }
    };
    (@core UrlShortener) => {
        fn shorten_url_core(&mut self, url: &str) -> $crate::uploader::CoreResult {
            self.shorten(url)
        }
    };
    (@core UrlSharer) => {
        fn share_url_core(&mut self, url: &str) -> $crate::uploader::CoreResult {
            self.share(url)
        }
    };
}
pub(crate) use uploader_base;
