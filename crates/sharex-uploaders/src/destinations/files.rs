//! File destinations that need at most an API key: `FileUploaders/Pomf.cs`,
//! `Uguu.cs`, `Transfersh.cs`, `Sul.cs`, `Lambda.cs`.

use serde_json::Value;
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, uploader_base};
use crate::args;
use crate::config::{LambdaSettings, PomfUploader};
use crate::enums::HttpMethod;
use crate::http::{self, SharedStream};
use crate::json::{get, get_bool, get_str};
use crate::uploader::{CoreResult, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

fn null_ref<T>() -> UResult<T> {
    Err(UploadError::Exception(NULL_REFERENCE.into()))
}

/// `Pomf`: Pomf-compatible hosts (`files[]` form field).
pub struct Pomf {
    pub base: Uploader,
    pub uploader: PomfUploader,
}

uploader_base!(Pomf, FileUploader);
impl WritesBackConfig for Pomf {}

impl Pomf {
    pub fn new(uploader: PomfUploader) -> Self {
        Self { base: Uploader::new(), uploader }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let url = self.uploader.UploadURL.clone().unwrap_or_default();
        let mut result = self.base.send_request_file_simple(&url, data, file_name, "files[]", &[], None)?;
        if result.is_success() {
            let Some(response) = deserialize_object(result.response.as_deref())? else { return null_ref() };
            let files = get(&response, "files").and_then(Value::as_array);
            if get_bool(&response, "success")
                && let Some(first) = files.and_then(|f| f.first())
            {
                let mut url = get_str(first, "url").unwrap_or_default();
                let result_url = self.uploader.ResultURL.clone().unwrap_or_default();
                if !url_helpers::has_prefix(&url) && !result_url.is_empty() {
                    url = url_helpers::combine_url(&url_helpers::fix_prefix(&result_url), &url);
                }
                result.url = Some(url);
            }
        }
        Ok(Some(result))
    }
}

/// `Uguu`.
pub struct Uguu {
    pub base: Uploader,
}

uploader_base!(Uguu, FileUploader);
impl WritesBackConfig for Uguu {}

impl Uguu {
    pub fn new() -> Self {
        Self { base: Uploader::new() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let mut result = self.base.send_request_file_simple("https://uguu.se/upload.php?output=text", data, file_name, "files[]", &[], None)?;
        if result.is_success() {
            result.url = result.response.clone();
        }
        Ok(Some(result))
    }
}

impl Default for Uguu {
    fn default() -> Self {
        Self::new()
    }
}

/// `Transfersh`.
pub struct Transfersh {
    pub base: Uploader,
}

uploader_base!(Transfersh, FileUploader);
impl WritesBackConfig for Transfersh {}

impl Transfersh {
    pub fn new() -> Self {
        Self { base: Uploader::new() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let mut result = self.base.send_request_file_simple("https://transfer.sh", data, file_name, "file", &[], None)?;
        if result.is_success() {
            result.url = result.response.as_deref().map(|r| r.trim().to_owned());
        }
        Ok(Some(result))
    }
}

impl Default for Transfersh {
    fn default() -> Self {
        Self::new()
    }
}

/// `SulUploader` (s-ul.eu).
pub struct Sul {
    pub base: Uploader,
    api_key: String,
}

uploader_base!(Sul, FileUploader);
impl WritesBackConfig for Sul {}

impl Sul {
    pub fn new(api_key: &str) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("wizard", "true"), ("key", self.api_key), ("client", "sharex-native")];
        let url = "https://s-ul.eu";
        let upload_url = url_helpers::combine_url(url, "api/v1/upload");
        let mut result = self.base.send_request_file_simple(&upload_url, data, file_name, "file", &args, None)?;
        if result.is_success() {
            let json = crate::json::parse(result.response.as_deref().unwrap_or("")).map_err(UploadError::Exception)?;
            let token = |name: &str| -> UResult<String> {
                match crate::json::select_token(&json, name).map_err(UploadError::Exception)? {
                    Some(t) => Ok(crate::json::to_dotnet_string(t).map_err(UploadError::Exception)?.unwrap_or_default()),
                    None => Ok(String::new()),
                }
            };
            let (protocol, domain, file, extension, error) = if json.is_null() {
                Default::default()
            } else {
                (token("protocol")?, token("domain")?, token("filename")?, token("extension")?, token("error")?)
            };
            if !error.is_empty() || protocol.is_empty() {
                if error.is_empty() {
                    self.base.errors.add(strings::Sul_Generic_error_contact_support());
                } else {
                    self.base.errors.add(error);
                }
            } else {
                result.url = Some(format!("{protocol}{domain}/{file}{extension}"));
                result.deletion_url = Some(url_helpers::combine_url(url, &format!("delete.php?key={}&file={file}", self.api_key)));
            }
        }
        Ok(Some(result))
    }
}

/// `Lambda`.
pub struct Lambda {
    pub base: Uploader,
    pub config: LambdaSettings,
}

uploader_base!(Lambda, FileUploader);

impl WritesBackConfig for Lambda {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.LambdaSettings = self.config.clone();
    }
}

/// `Lambda.UploadURLs`.
pub const LAMBDA_UPLOAD_URLS: [&str; 2] = ["https://lbda.net/", "https://lambda.sx/"];

impl Lambda {
    pub fn new(config: LambdaSettings) -> Self {
        Self { base: Uploader::new(), config }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("api_key", self.config.UserAPIKey.as_str())];
        let mut result = self.base.send_request_file(
            "https://lbda.net/api/upload",
            data,
            file_name,
            "file",
            &args,
            None,
            HttpMethod::PUT,
            http::CONTENT_TYPE_MULTIPART_FORM_DATA,
            None,
        )?;
        let Some(response_text) = result.response.clone() else {
            self.base.errors.add(strings::Lambda_Upload_failed_check_API_key());
            return Ok(Some(result));
        };
        let response = deserialize_object(Some(&response_text))?;
        if result.is_success() {
            let Some(response) = response else { return null_ref() };
            result.url = Some(format!("{}{}", self.config.UploadURL.clone().unwrap_or_default(), get_str(&response, "url").unwrap_or_default()));
        } else {
            let Some(response) = response else { return null_ref() };
            let Some(errors) = get(&response, "errors").and_then(Value::as_array) else { return null_ref() };
            for e in errors {
                self.base.errors.add(crate::json::to_dotnet_string(e).ok().flatten().unwrap_or_default());
            }
        }
        Ok(Some(result))
    }
}
