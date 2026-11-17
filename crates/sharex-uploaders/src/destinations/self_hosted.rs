//! Self-hosted servers: `FileUploaders/Immich.cs`, `OwnCloud.cs`
//! (ownCloud / Nextcloud).

use serde_json::Value;
use sharex_helpers::{file_helpers, mime_types};
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, uploader_base};
use crate::args;
use crate::enums::HttpMethod;
use crate::http::{self, SharedStream};
use crate::json::{self, get, get_i64, get_str};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig, create_authentication_header};
use crate::url_helpers;

fn ends_with_ignore_case(s: &str, suffix: &str) -> bool {
    s.len() >= suffix.len() && s.as_bytes()[s.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
}

/// `Immich`.
pub struct Immich {
    pub base: Uploader,
    pub url: String,
    pub api_key: String,
    pub auto_create_shareable_link: bool,
    pub show_metadata: bool,
}

uploader_base!(Immich, FileUploader);
impl WritesBackConfig for Immich {}

impl Immich {
    pub fn new(url: &str, api_key: &str) -> Self {
        Self { base: Uploader::new(), url: url.to_owned(), api_key: api_key.to_owned(), auto_create_shareable_link: true, show_metadata: false }
    }

    /// `ImmichFileUploaderService.CheckConfig`: an absolute http(s) URL and a key.
    pub fn check_config(url: &str, api_key: &str) -> bool {
        url_helpers::uri_host(url).is_some()
            && (url.trim().to_ascii_lowercase().starts_with("http://") || url.trim().to_ascii_lowercase().starts_with("https://"))
            && !api_key.trim().is_empty()
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        // DateTimeOffset.UtcNow.ToString("O")
        let timestamp = json::dotnet_round_trip(chrono::Utc::now(), "+00:00");
        let guid = http::create_boundary().trim_start_matches('-').to_owned();
        let args = args![
            ("deviceAssetId", format!("ShareX-{guid}")),
            ("deviceId", "ShareX"),
            ("fileCreatedAt", timestamp),
            ("fileModifiedAt", timestamp),
            ("isFavorite", "false"),
            ("filename", file_name),
        ];
        let headers = Headers::new().with("Accept", "application/json").with("x-api-key", self.api_key.clone());
        let mut result = self.base.send_request_file_simple(&upload_url(&self.url), data, file_name, "assetData", &args, Some(&headers))?;
        if result.is_success() {
            let response = deserialize_object(result.response.as_deref())?;
            match response.as_ref().and_then(|r| get_str(r, "Id")).filter(|id| !id.trim().is_empty()) {
                Some(id) => {
                    if self.auto_create_shareable_link {
                        result.url = self.create_shareable_link(&id, &headers)?;
                        result.success = result.url.as_deref().is_some_and(|u| !u.trim().is_empty());
                    } else {
                        result.url = Some(url_helpers::combine_urls(&[&web_url(&self.url), "photos", &id]));
                    }
                }
                None => result.success = false,
            }
        }
        Ok(Some(result))
    }

    fn create_shareable_link(&mut self, asset_id: &str, headers: &Headers) -> UResult<Option<String>> {
        let body = json::object([
            ("type", Value::String("INDIVIDUAL".into())),
            ("assetIds", Value::Array(vec![Value::String(asset_id.into())])),
            ("allowUpload", Value::Bool(false)),
            ("allowDownload", Value::Bool(true)),
            ("showMetadata", Value::Bool(self.show_metadata)),
        ]);
        let response = self.base.send_request_text(
            HttpMethod::POST,
            &shared_links_url(&self.url),
            &json::to_string(&body),
            Some(http::CONTENT_TYPE_JSON),
            &[],
            Some(headers),
        )?;
        let response = deserialize_object(response.as_deref())?;
        Ok(response
            .as_ref()
            .and_then(|r| get_str(r, "Key"))
            .filter(|k| !k.trim().is_empty())
            .map(|k| url_helpers::combine_urls(&[&web_url(&self.url), "share", &k])))
    }
}

/// `Immich.GetUploadURL`.
fn upload_url(url: &str) -> String {
    let n = url.trim_end_matches('/');
    if ends_with_ignore_case(n, "/api/assets") {
        n.to_owned()
    } else if ends_with_ignore_case(n, "/api") {
        url_helpers::combine_url(n, "assets")
    } else {
        url_helpers::combine_urls(&[n, "api", "assets"])
    }
}

/// `Immich.GetWebURL`.
fn web_url(url: &str) -> String {
    let n = url.trim_end_matches('/');
    if ends_with_ignore_case(n, "/api/assets") {
        n[..n.len() - 11].to_owned()
    } else if ends_with_ignore_case(n, "/api") {
        n[..n.len() - 4].to_owned()
    } else {
        n.to_owned()
    }
}

/// `Immich.GetSharedLinksURL`.
fn shared_links_url(url: &str) -> String {
    url_helpers::combine_urls(&[&web_url(url), "api", "shared-links"])
}

/// `OwnCloud` (ownCloud / Nextcloud): a WebDAV PUT, then a public share
/// through the OCS API.
pub struct OwnCloud {
    pub base: Uploader,
    pub host: String,
    pub username: String,
    pub password: String,
    pub path: String,
    pub auto_expire_time: i32,
    pub create_share: bool,
    pub append_file_name_to_url: bool,
    pub direct_link: bool,
    pub preview_link: bool,
    pub is_compatibility_81: bool,
    pub auto_expire: bool,
}

uploader_base!(OwnCloud, FileUploader);
impl WritesBackConfig for OwnCloud {}

impl OwnCloud {
    pub fn new(host: &str, username: &str, password: &str) -> Self {
        Self {
            base: Uploader::new(),
            host: host.to_owned(),
            username: username.to_owned(),
            password: password.to_owned(),
            path: "/".to_owned(),
            auto_expire_time: 7,
            create_share: true,
            append_file_name_to_url: false,
            direct_link: false,
            preview_link: false,
            is_compatibility_81: true,
            auto_expire: false,
        }
    }

    fn headers(&self) -> Headers {
        let mut headers = create_authentication_header(&self.username, &self.password);
        headers.set("OCS-APIREQUEST", "true");
        headers
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.host.is_empty() {
            return Err(UploadError::Exception(strings::OwnCloud_Host_is_empty().into()));
        }
        if self.username.is_empty() || self.password.is_empty() {
            return Err(UploadError::Exception(strings::OwnCloud_Credentials_are_empty().into()));
        }
        if self.path.is_empty() {
            self.path = "/".into();
        }
        let path = url_helpers::combine_url(&self.path, file_name);
        let encoded_path = url_helpers::combine_url(&self.path, &url_helpers::url_encode(file_name, false));
        let url = url_helpers::fix_prefix(&url_helpers::combine_urls(&[&self.host, "remote.php/webdav", &encoded_path]));
        let headers = self.headers();
        let mime = mime_types::get_mime_type_from_file_name(file_name);
        let response = self.base.send_request_stream(HttpMethod::PUT, &url, data, Some(&mime), &[], Some(&headers))?;
        let mut result = UploadResult::with_response(response, None);
        if !self.base.is_error() {
            if self.create_share {
                self.base.allow_report_progress = false;
                result.url = self.share_file(&path, file_name)?;
            } else {
                result.is_url_expected = false;
            }
        }
        Ok(Some(result))
    }

    /// `ShareFileAsync`: a public, read-only link (expiring if set).
    pub fn share_file(&mut self, path: &str, file_name: &str) -> UResult<Option<String>> {
        let mut args = args![("path", path), ("shareType", "3"), ("permissions", "1")];
        if self.auto_expire {
            if self.auto_expire_time == 0 {
                return Err(UploadError::Exception(strings::OwnCloud_Auto_expire_time_is_invalid().into()));
            }
            let expire = chrono::Utc::now() + chrono::Duration::days(self.auto_expire_time as i64);
            use chrono::Datelike;
            args.push(("expireDate".into(), format!("{}-{}-{}", expire.year(), expire.month(), expire.day())));
        }
        let url = url_helpers::fix_prefix(&url_helpers::combine_url(&self.host, "ocs/v1.php/apps/files_sharing/api/v1/shares?format=json"));
        let headers = self.headers();
        let response = self.base.send_request_multipart(&url, &args, Some(&headers), HttpMethod::POST)?;
        if let Some(response) = response.filter(|r| !r.is_empty())
            && let Some(result) = deserialize_object(Some(&response))?
            && let Some(ocs) = get(&result, "ocs").filter(|o| !o.is_null())
            && let Some(meta) = get(ocs, "meta").filter(|m| !m.is_null())
        {
            let data = get(ocs, "data").filter(|d| !d.is_null());
            let statuscode = get_i64(meta, "statuscode");
            if let Some(data) = data.filter(|_| statuscode == 100) {
                if !data.is_object() {
                    return Err(UploadError::Exception("Unable to cast object to type 'Newtonsoft.Json.Linq.JObject'.".into()));
                }
                let mut link = get_str(data, "url").ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
                if self.preview_link && file_helpers::is_image_file(path) {
                    link.push_str("/preview");
                } else if self.direct_link {
                    link.push_str(if self.is_compatibility_81 { "/download" } else { "&download" });
                    if self.append_file_name_to_url {
                        link = url_helpers::combine_url(&link, &url_helpers::url_encode(file_name, false));
                    }
                }
                return Ok(Some(link));
            }
            self.base.errors.add(
                strings::OwnCloud_Error_details()
                    .replace("{0}", &get_str(meta, "status").unwrap_or_default())
                    .replace("{1}", &statuscode.to_string())
                    .replace("{2}", &get_str(meta, "message").unwrap_or_default()),
            );
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immich_urls() {
        assert_eq!(upload_url("https://photos.example.com/"), "https://photos.example.com/api/assets");
        assert_eq!(upload_url("https://photos.example.com/API"), "https://photos.example.com/API/assets");
        assert_eq!(web_url("https://photos.example.com/api/assets/"), "https://photos.example.com");
        assert_eq!(shared_links_url("https://p.example.com/api"), "https://p.example.com/api/shared-links");
        assert!(Immich::check_config("https://p.example.com", "k"));
        assert!(!Immich::check_config("p.example.com", "k"));
    }
}
