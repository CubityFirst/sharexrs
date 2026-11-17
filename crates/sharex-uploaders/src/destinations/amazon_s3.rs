//! `FileUploaders/AmazonS3.cs` (S3 and S3-compatible storage, SigV4).

use super::uploader_base;
use crate::config::AmazonS3Settings;
use crate::http::SharedStream;
use crate::uploader::{CoreResult, UploadError, Uploader, WritesBackConfig};

/// `AmazonS3`.
pub struct AmazonS3 {
    pub base: Uploader,
    pub settings: AmazonS3Settings,
}

uploader_base!(AmazonS3, FileUploader);
impl WritesBackConfig for AmazonS3 {}

impl AmazonS3 {
    pub fn new(settings: AmazonS3Settings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    /// `AmazonS3NewFileUploaderService.CheckConfig`.
    pub fn check_config(_settings: &AmazonS3Settings) -> bool {
        false
    }

    fn upload(&mut self, _data: &SharedStream, _file_name: &str) -> CoreResult {
        Err(UploadError::Exception("Amazon S3 isn't ported yet.".into()))
    }
}
