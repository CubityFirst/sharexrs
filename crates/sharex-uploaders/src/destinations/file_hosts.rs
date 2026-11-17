//! File hosts with an account or API key: `FileUploaders/Puush.cs`,
//! `Streamable.cs`, `LobFile.cs`, `ImgFish.cs`.

use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, uploader_base};
use crate::args;
use crate::config::ImgFishSettings;
use crate::enums::HttpMethod;
use crate::http::{self, ProgressManager, SharedStream};
use crate::json::{get, get_bool, get_i64, get_str};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig, create_authentication_header};
use crate::url_helpers;

fn null_ref<T>() -> UResult<T> {
    Err(UploadError::Exception(NULL_REFERENCE.into()))
}

/// `Puush.PuushURL`, its register and reset password pages.
pub const PUUSH_URL: &str = "https://puush.me";
pub const PUUSH_REGISTER_URL: &str = "https://puush.me/register";
pub const PUUSH_RESET_PASSWORD_URL: &str = "https://puush.me/reset_password";

/// `Puush`.
pub struct Puush {
    pub base: Uploader,
    pub api_key: String,
}

uploader_base!(Puush, FileUploader);
impl WritesBackConfig for Puush {}

impl Puush {
    pub fn new(api_key: &str) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned() }
    }

    /// `LoginAsync`: the API key for an account.
    pub fn login(&mut self, email: &str, password: &str) -> UResult<Option<String>> {
        let args = args![("e", email), ("p", password), ("z", http::user_agent())];
        let response = self.base.send_request_multipart("https://puush.me/api/auth", &args, None, HttpMethod::POST)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let values: Vec<&str> = r.split(',').collect();
            if values.len() > 1 && values[0].trim().parse::<i32>().is_ok_and(|s| s >= 0) {
                return Ok(Some(values[1].to_owned()));
            }
        }
        Ok(None)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("k", self.api_key), ("z", http::user_agent())];
        let mut result = self.base.send_request_file_simple("https://puush.me/api/up", data, file_name, "f", &args, None)?;
        if result.is_success() {
            let response = result.response.clone().unwrap_or_default();
            let values: Vec<&str> = response.split(',').collect();
            let status = values[0].trim().parse::<i32>().unwrap_or(-2);
            if status < 0 {
                self.base.errors.add(match status {
                    -1 => strings::Puush_Authentication_failure(),
                    -3 => strings::Puush_Checksum_error(),
                    -4 => strings::Puush_Insufficient_account_storage(),
                    _ => strings::Puush_Connection_error(),
                });
            } else if values.len() > 1 {
                result.url = Some(values[1].to_owned());
            }
        }
        Ok(Some(result))
    }
}

/// `Streamable`: uploads, then polls the transcoding every second
/// (progress 0–100 while it runs).
pub struct Streamable {
    pub base: Uploader,
    pub email: String,
    pub password: String,
    pub use_direct_url: bool,
    /// The wait between status checks (1 s, as ShareX's `Task.Delay(1000)`).
    pub poll_interval: std::time::Duration,
}

uploader_base!(Streamable, FileUploader);
impl WritesBackConfig for Streamable {}

impl Streamable {
    pub fn new(email: &str, password: &str) -> Self {
        Self {
            base: Uploader::new(),
            email: email.to_owned(),
            password: password.to_owned(),
            use_direct_url: false,
            poll_interval: std::time::Duration::from_secs(1),
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let headers = (!self.email.is_empty() && !self.password.is_empty()).then(|| create_authentication_header(&self.email, &self.password));
        let url = url_helpers::combine_url("https://api.streamable.com", "upload");
        let mut result = self.base.send_request_file_simple(&url, data, file_name, "file", &[], headers.as_ref())?;
        self.transcode_file(&mut result)?;
        Ok(Some(result))
    }

    fn report(&self, progress: &ProgressManager) {
        if let Some(cb) = self.base.progress_callback() {
            cb(progress);
        }
    }

    /// `TranscodeFileAsync`.
    fn transcode_file(&mut self, result: &mut UploadResult) -> UResult<()> {
        let Some(transcode) = deserialize_object(result.response.as_deref())? else { return null_ref() };
        let Some(shortcode) = get_str(&transcode, "Shortcode").filter(|s| !s.is_empty()) else {
            self.base.errors.add(strings::Streamable_Could_not_create_video());
            result.success = false;
            return Ok(());
        };
        let mut progress = ProgressManager::new(100, 0);
        self.report(&progress);
        while !self.base.stop_requested() {
            let url = url_helpers::combine_urls(&["https://api.streamable.com", "videos", &shortcode]);
            let status_json = self.base.send_request(HttpMethod::GET, &url, &[], None)?;
            let Some(response) = deserialize_object(status_json.as_deref())? else { return null_ref() };
            let status = get_i64(&response, "status");
            if status > 2 {
                self.base.errors.add(get_str(&response, "message").unwrap_or_default());
                result.success = false;
                break;
            } else if status == 2 {
                progress.update_progress(100 - progress.position);
                self.report(&progress);
                result.success = true;
                let mp4 = get(&response, "files").and_then(|f| get(f, "mp4")).and_then(|m| get_str(m, "url")).filter(|u| !u.is_empty());
                let url = match mp4.filter(|_| self.use_direct_url) {
                    Some(u) => u,
                    None => get_str(&response, "url").unwrap_or_default(),
                };
                result.url = Some(url_helpers::force_prefix(&url, "https://"));
                break;
            }
            let percent = get_i64(&response, "percent").max(0) as u64;
            progress.update_progress(percent.saturating_sub(progress.position));
            self.report(&progress);
            let until = std::time::Instant::now() + self.poll_interval;
            while std::time::Instant::now() < until {
                if self.base.stop_requested() {
                    return Err(UploadError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(20).min(self.poll_interval));
            }
        }
        Ok(())
    }
}

/// `LobFile` (formerly Lithiio).
pub struct LobFile {
    pub base: Uploader,
    pub api_key: String,
}

uploader_base!(LobFile, FileUploader);
impl WritesBackConfig for LobFile {}

impl LobFile {
    pub fn new(api_key: &str) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("api_key", self.api_key)];
        let mut result = self.base.send_request_file_simple("https://lobfile.com/api/v3/upload", data, file_name, "file", &args, None)?;
        if result.is_success() {
            let Some(response) = deserialize_object(result.response.as_deref())? else { return null_ref() };
            if get_bool(&response, "Success") {
                result.url = get_str(&response, "URL");
            } else {
                self.base.errors.add(get_str(&response, "Error").unwrap_or_default());
            }
        }
        Ok(Some(result))
    }

    /// `FetchAPIKeyAsync`.
    pub fn fetch_api_key(&mut self, email: &str, password: &str) -> UResult<Option<String>> {
        let args = args![("email", email), ("password", password)];
        let response = self.base.send_request_multipart("https://lobfile.com/api/v3/fetch-api-key", &args, None, HttpMethod::POST)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let Some(json) = deserialize_object(Some(&r))? else { return null_ref() };
            if get_bool(&json, "Success") {
                return Ok(get_str(&json, "API_Key"));
            }
            return Err(UploadError::Exception(get_str(&json, "Error").unwrap_or_default()));
        }
        Ok(None)
    }
}

/// `ImgFishSettings.MinFileIDLength` / `MaxFileIDLength`.
pub const IMGFISH_MIN_FILE_ID_LENGTH: i32 = 8;
pub const IMGFISH_MAX_FILE_ID_LENGTH: i32 = 32;

/// `ImgFish` (img.fish).
pub struct ImgFish {
    pub base: Uploader,
    pub settings: ImgFishSettings,
}

uploader_base!(ImgFish, FileUploader);
impl WritesBackConfig for ImgFish {}

impl ImgFish {
    pub fn new(settings: ImgFishSettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("length", self.settings.FileIDLength)];
        let mut headers = Headers::new().with("Accept", "application/json");
        if !self.settings.APIKey.as_str().trim().is_empty() {
            headers.set("x-api-key", self.settings.APIKey.as_str());
        }
        let mut result = self.base.send_request_file_simple("https://img.fish/up", data, file_name, "file", &args, Some(&headers))?;
        if result.is_success() {
            match deserialize_object(result.response.as_deref()) {
                Ok(response) => {
                    let error = response.as_ref().and_then(|r| get_str(r, "error")).filter(|e| !e.is_empty());
                    let link = response.as_ref().and_then(|r| get_str(r, "link")).filter(|l| !l.is_empty());
                    if let Some(error) = error {
                        self.base.errors.add(error);
                        result.success = false;
                    } else if let Some(link) = link {
                        result.url = Some(link);
                        result.deletion_url = response.as_ref().and_then(|r| get_str(r, "destroy"));
                    } else {
                        self.base.errors.add(strings::Common_Unknown_error());
                        result.success = false;
                    }
                }
                // catch (JsonException)
                Err(_) => {
                    self.base.errors.add(strings::Common_Unknown_error());
                    result.success = false;
                }
            }
        }
        Ok(Some(result))
    }
}
