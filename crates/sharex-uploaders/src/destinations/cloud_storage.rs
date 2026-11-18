//! Cloud storage with their own request signing: `FileUploaders/AzureStorage.cs`
//! (SharedKey, HMAC-SHA256) and `BackblazeB2.cs` (B2 native API v1).

use hmac::{Hmac, Mac};
use serde_json::Value;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use sharex_helpers::{NameParser, NameParserType, mime_types};
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, uploader_base};
use crate::enums::HttpMethod;
use crate::http::{self, ResponseInfo, SharedStream};
use crate::json::{self, get, get_str};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig, create_authentication_header};
use crate::url_helpers;

/// `NameParser.Parse(type, pattern)`.
pub(crate) fn name_parse(kind: NameParserType, pattern: &str) -> String {
    NameParser::new(kind).parse(pattern)
}

fn base64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ---------------------------------------------------------------------------
// Azure Storage

const AZURE_API_VERSION: &str = "2016-05-31";

/// `AzureStorage`: a block blob PUT signed with the account's shared key.
pub struct AzureStorage {
    pub base: Uploader,
    pub account_name: String,
    pub account_access_key: String,
    pub container: String,
    pub environment: String,
    pub custom_domain: String,
    pub upload_path: String,
    pub cache_control: String,
}

uploader_base!(AzureStorage, FileUploader);
impl WritesBackConfig for AzureStorage {}

impl AzureStorage {
    pub fn new(account_name: &str, access_key: &str, container: &str, environment: &str, custom_domain: &str, upload_path: &str, cache_control: &str) -> Self {
        Self {
            base: Uploader::new(),
            account_name: account_name.to_owned(),
            account_access_key: access_key.to_owned(),
            container: container.to_owned(),
            environment: if environment.is_empty() { "blob.core.windows.net".to_owned() } else { environment.to_owned() },
            custom_domain: custom_domain.to_owned(),
            upload_path: upload_path.to_owned(),
            cache_control: cache_control.to_owned(),
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.account_name.is_empty() {
            self.base.errors.add(strings::AzureStorage_Account_name_must_not_be_empty());
        }
        if self.account_access_key.is_empty() {
            self.base.errors.add(strings::AzureStorage_Access_key_must_not_be_empty());
        }
        if self.container.is_empty() {
            self.base.errors.add(strings::AzureStorage_Container_must_not_be_empty());
        }
        if self.base.is_error() {
            return Ok(None);
        }
        // DateTime.UtcNow.ToString("R")
        let date = chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let upload_path = self.get_upload_path(file_name);
        let request_url = self.generate_url(&upload_path, true);
        let result_url = self.generate_url(&upload_path, false);
        self.base.early_url_copy_requested(&result_url);
        let content_type = mime_types::get_mime_type_from_file_name(file_name);
        let mut headers = Headers::new();
        headers.set("x-ms-date", date.clone());
        headers.set("x-ms-version", AZURE_API_VERSION);
        headers.set("x-ms-blob-type", "BlockBlob");
        let mut canonicalized_headers = format!("x-ms-blob-type:BlockBlob\nx-ms-date:{date}\nx-ms-version:{AZURE_API_VERSION}\n");
        if !self.cache_control.is_empty() {
            headers.set("x-ms-blob-cache-control", self.cache_control.clone());
            canonicalized_headers = format!("x-ms-blob-cache-control:{}\n{canonicalized_headers}", self.cache_control);
        }
        let canonicalized_resource = format!("/{}/{}/{}", self.account_name, self.container, upload_path);
        let length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let string_to_sign = self.generate_string_to_sign(&canonicalized_headers, &canonicalized_resource, &length.to_string(), &content_type)?;
        headers.set("Authorization", format!("SharedKey {}:{}", self.account_name, string_to_sign));
        self.base.send_request_stream(HttpMethod::PUT, &request_url, data, Some(&content_type), &[], Some(&headers))?;
        if self.base.last_response_info.as_ref().is_some_and(ResponseInfo::is_success) {
            return Ok(Some(UploadResult { success: true, url: Some(result_url), ..Default::default() }));
        }
        self.base.errors.add(strings::UploaderErrors_Upload_failed());
        Ok(None)
    }

    fn generate_string_to_sign(&self, canonicalized_headers: &str, canonicalized_resource: &str, content_length: &str, content_type: &str) -> UResult<String> {
        let string_to_sign =
            format!("PUT\n\n\n{content_length}\n\n{content_type}\n\n\n\n\n\n\n{canonicalized_headers}{canonicalized_resource}");
        self.hash_request(&string_to_sign)
    }

    fn hash_request(&self, string_to_sign: &str) -> UResult<String> {
        use base64::Engine;
        let key = base64::engine::general_purpose::STANDARD
            .decode(self.account_access_key.trim())
            .map_err(|_| UploadError::Exception("The input is not a valid Base-64 string as it contains a non-base 64 character, more than two padding characters, or an illegal character among the padding characters.".into()))?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("HMAC takes any key");
        mac.update(string_to_sign.as_bytes());
        Ok(base64(&mac.finalize().into_bytes()))
    }

    fn get_upload_path(&self, file_name: &str) -> String {
        let upload_path = if !self.upload_path.is_empty() {
            let path = name_parse(NameParserType::FilePath, self.upload_path.trim_matches('/'));
            url_helpers::combine_url(&path, file_name)
        } else {
            file_name.to_owned()
        };
        uri_escape_data_string(&upload_path)
    }

    /// `GenerateURL`.
    pub fn generate_url(&self, upload_path: &str, is_request: bool) -> String {
        if !is_request && !self.custom_domain.is_empty() {
            url_helpers::fix_prefix(&url_helpers::combine_url(&self.custom_domain, upload_path))
        } else if !is_request && self.container == "$root" {
            format!("https://{}.{}/{}", self.account_name, self.environment, upload_path)
        } else {
            format!("https://{}.{}/{}/{}", self.account_name, self.environment, self.container, upload_path)
        }
    }

    /// `GetPreviewURL`: the URL an upload of `example.png` gets.
    pub fn preview_url(&self) -> String {
        self.generate_url(&self.get_upload_path("example.png"), false)
    }
}

/// `Uri.EscapeDataString`: everything but `A-Z a-z 0-9 - . _ ~` as UTF-8
/// `%XX` (upper case).
pub fn uri_escape_data_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Backblaze B2

const B2_AUTHORIZE_ACCOUNT_URL: &str = "https://api.backblazeb2.com/b2api/v1/b2_authorize_account";
const B2_GET_UPLOAD_URL_PATH: &str = "/b2api/v1/b2_get_upload_url";
const B2_LIST_BUCKETS_PATH: &str = "/b2api/v1/b2_list_buckets";
const B2_APPLICATION_JSON: &str = "application/json; charset=utf-8";

/// `BackblazeB2`.
pub struct BackblazeB2 {
    pub base: Uploader,
    pub application_key_id: String,
    pub application_key: String,
    pub bucket_name: String,
    pub upload_path: String,
    pub use_custom_url: bool,
    pub custom_url: String,
    /// The waits between attempts (ShareX: 2^(try-1) seconds); tests shorten them.
    pub backoff: fn(u32) -> std::time::Duration,
}

uploader_base!(BackblazeB2, FileUploader);
impl WritesBackConfig for BackblazeB2 {}

struct B2Auth {
    account_id: Option<String>,
    api_url: Option<String>,
    authorization_token: Option<String>,
    download_url: Option<String>,
    allowed: Option<Value>,
}

struct B2UploadUrl {
    upload_url: Option<String>,
    authorization_token: Option<String>,
}

struct B2Error {
    status: i64,
    code: Option<String>,
    message: Option<String>,
}

enum B2UploadOutcome {
    ConnectionFailed,
    Error(u16, Option<B2Error>),
    Ok(Option<Value>),
}

fn b2_backoff(tries: u32) -> std::time::Duration {
    std::time::Duration::from_millis(2u64.pow(tries - 1) * 1000)
}

impl BackblazeB2 {
    pub fn new(application_key_id: &str, application_key: &str, bucket_name: &str, upload_path: &str, use_custom_url: bool, custom_url: &str) -> Self {
        Self {
            base: Uploader::new(),
            application_key_id: application_key_id.to_owned(),
            application_key: application_key.to_owned(),
            bucket_name: bucket_name.to_owned(),
            upload_path: upload_path.to_owned(),
            use_custom_url,
            custom_url: custom_url.to_owned(),
            backoff: b2_backoff,
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let parsed_upload_path = name_parse(NameParserType::FilePath, &self.upload_path);
        let destination_path = url_helpers::combine_url(&parsed_upload_path, file_name);
        let auth = match self.authorize()? {
            Ok(a) => a,
            Err(e) => {
                self.base.errors.add(strings::BackblazeB2_Could_not_authenticate().replace("{0}", &e));
                return Ok(None);
            }
        };
        let mut bucket_id = auth.allowed.as_ref().and_then(|a| get_str(a, "bucketId"));
        if bucket_id.is_none() {
            match self.get_bucket_id(&auth)? {
                Ok(id) => bucket_id = Some(id),
                Err(e) => {
                    self.base.errors.add(strings::BackblazeB2_Upload_failed_with_error().replace("{0}", &e));
                    return Ok(None);
                }
            }
        }
        let bucket_id = bucket_id.unwrap_or_default();
        if let Err(e) = is_authorized_for_upload(&auth, &bucket_id, &destination_path) {
            self.base.errors.add(strings::BackblazeB2_Upload_failed_with_error().replace("{0}", e));
            return Ok(None);
        }
        const MAX_TRIES: u32 = 5;
        let mut url: Option<B2UploadUrl> = None;
        for tries in 1..=MAX_TRIES {
            if tries > 1 {
                self.sleep((self.backoff)(tries))?;
            }
            if url.is_none() {
                match self.get_upload_url(&auth, &bucket_id)? {
                    Ok(u) => url = Some(u),
                    Err(e) => {
                        self.base.errors.add(strings::BackblazeB2_Could_not_get_upload_URL().replace("{0}", &e));
                        return Ok(None);
                    }
                }
            }
            let outcome = self.upload_file(url.as_ref().expect("set above"), &destination_path, data)?;
            match outcome {
                B2UploadOutcome::ConnectionFailed => {
                    url = None;
                    continue;
                }
                B2UploadOutcome::Error(401, Some(e)) if matches!(e.code.as_deref(), Some("expired_auth_token" | "bad_auth_token")) => {
                    url = None;
                    continue;
                }
                B2UploadOutcome::Error(408 | 429, _) => continue,
                B2UploadOutcome::Error(503, _) => {
                    url = None;
                    continue;
                }
                B2UploadOutcome::Error(_, _) => {
                    self.base.errors.add(strings::BackblazeB2_Unknown_upload_error());
                    return Ok(None);
                }
                B2UploadOutcome::Ok(upload) => {
                    let uploaded_name = upload.as_ref().and_then(|u| get_str(u, "fileName"));
                    let Some(uploaded_name) = uploaded_name else { return Err(UploadError::Exception(NULL_REFERENCE.into())) };
                    let encoded_file_name = url_helpers::url_encode(&uploaded_name, true);
                    let mut remote_location = url_helpers::combine_urls(&[
                        auth.download_url.as_deref().unwrap_or(""),
                        "file",
                        &url_helpers::url_encode(&self.bucket_name, false),
                        &encoded_file_name,
                    ]);
                    if self.use_custom_url {
                        remote_location = url_helpers::fix_prefix(&url_helpers::combine_url(&self.custom_url, &encoded_file_name));
                    }
                    return Ok(Some(UploadResult { success: true, url: Some(remote_location), ..Default::default() }));
                }
            }
        }
        self.base.errors.add(strings::BackblazeB2_Upload_failed_after_attempts().replace("{0}", &MAX_TRIES.to_string()));
        Ok(None)
    }

    /// `Task.Delay(delay, cancellationToken)`.
    fn sleep(&self, duration: std::time::Duration) -> UResult<()> {
        let end = std::time::Instant::now() + duration;
        while std::time::Instant::now() < end {
            if self.base.stop_requested() {
                return Err(UploadError::Cancelled);
            }
            std::thread::sleep((end - std::time::Instant::now()).min(std::time::Duration::from_millis(50)));
        }
        Ok(())
    }

    fn authorize(&mut self) -> UResult<Result<B2Auth, String>> {
        let headers = create_authentication_header(&self.application_key_id, &self.application_key);
        let Some(res) = self.base.get_response(HttpMethod::GET, B2_AUTHORIZE_ACCOUNT_URL, None, &[], Some(&headers), &[], true)? else {
            return Ok(Err("Connection failed.".into()));
        };
        if res.status_code != 200 {
            return Ok(Err(stringify_b2_error(&res)));
        }
        let v = super::deserialize_object(res.response_text.as_deref())?;
        let Some(v) = v else { return Err(UploadError::Exception(NULL_REFERENCE.into())) };
        Ok(Ok(B2Auth {
            account_id: get_str(&v, "accountId"),
            api_url: get_str(&v, "apiUrl"),
            authorization_token: get_str(&v, "authorizationToken"),
            download_url: get_str(&v, "downloadUrl"),
            allowed: get(&v, "allowed").filter(|a| a.is_object()).cloned(),
        }))
    }

    fn json_body(&self, props: Vec<(&str, Option<String>)>) -> SharedStream {
        let mut m = serde_json::Map::new();
        for (k, v) in props {
            m.insert(k.to_owned(), json::str_or_null(v.as_deref()));
        }
        let body = json::to_string(&Value::Object(m));
        http::shared_stream(std::io::Cursor::new(body.into_bytes()))
    }

    fn get_bucket_id(&mut self, auth: &B2Auth) -> UResult<Result<String, String>> {
        let mut headers = Headers::new();
        headers.set("Authorization", auth.authorization_token.clone().unwrap_or_default());
        let data = self.json_body(vec![("accountId", auth.account_id.clone()), ("bucketName", Some(self.bucket_name.clone()))]);
        let url = format!("{}{}", auth.api_url.as_deref().unwrap_or(""), B2_LIST_BUCKETS_PATH);
        let Some(res) = self.base.get_response_stream(HttpMethod::POST, &url, Some(&data), Some(B2_APPLICATION_JSON), &[], Some(&headers), true)? else {
            return Ok(Err("Connection failed.".into()));
        };
        if res.status_code != 200 {
            return Ok(Err(stringify_b2_error(&res)));
        }
        let Ok(json) = json::parse(res.response_text.as_deref().unwrap_or("")).and_then(|v| if v.is_object() { Ok(v) } else { Err(String::new()) }) else {
            return Ok(Err("B2 upload failed: Couldn't parse b2_list_buckets response.".into()));
        };
        let mut bucket_id = String::new();
        if let Some(Value::Array(buckets)) = json.get("buckets") {
            for b in buckets {
                let name = b.get("bucketName").ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
                if json_token_to_string(name) == self.bucket_name {
                    bucket_id = b.get("bucketId").map(json_token_to_string).unwrap_or_default();
                    break;
                }
            }
        }
        Ok(if !bucket_id.trim().is_empty() {
            Ok(bucket_id)
        } else {
            Err(format!("B2 upload failed: Couldn't find bucket {}.", self.bucket_name))
        })
    }

    fn get_upload_url(&mut self, auth: &B2Auth, bucket_id: &str) -> UResult<Result<B2UploadUrl, String>> {
        let mut headers = Headers::new();
        headers.set("Authorization", auth.authorization_token.clone().unwrap_or_default());
        let data = self.json_body(vec![("bucketId", Some(bucket_id.to_owned()))]);
        let url = format!("{}{}", auth.api_url.as_deref().unwrap_or(""), B2_GET_UPLOAD_URL_PATH);
        let Some(res) = self.base.get_response_stream(HttpMethod::POST, &url, Some(&data), Some(B2_APPLICATION_JSON), &[], Some(&headers), true)? else {
            return Ok(Err("Connection failed.".into()));
        };
        if res.status_code != 200 {
            return Ok(Err(stringify_b2_error(&res)));
        }
        let v = super::deserialize_object(res.response_text.as_deref())?;
        let Some(v) = v else { return Err(UploadError::Exception(NULL_REFERENCE.into())) };
        Ok(Ok(B2UploadUrl { upload_url: get_str(&v, "uploadUrl"), authorization_token: get_str(&v, "authorizationToken") }))
    }

    fn upload_file(&mut self, upload_url: &B2UploadUrl, destination_path: &str, data: &SharedStream) -> UResult<B2UploadOutcome> {
        let disposition = content_disposition("inline", &url_helpers::get_file_name(destination_path));
        let (sha1, length) = {
            use std::io::{Read, Seek, SeekFrom};
            let mut s = data.lock().expect("upload stream");
            s.seek(SeekFrom::Start(0)).map_err(|e| UploadError::Exception(e.to_string()))?;
            let mut hasher = Sha1::new();
            let mut buf = [0u8; 8192];
            let mut total = 0u64;
            loop {
                let n = s.read(&mut buf).map_err(|e| UploadError::Exception(e.to_string()))?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                hasher.update(&buf[..n]);
            }
            s.seek(SeekFrom::Start(0)).map_err(|e| UploadError::Exception(e.to_string()))?;
            (hex::encode(hasher.finalize()), total)
        };
        let mut headers = Headers::new();
        headers.set("Authorization", upload_url.authorization_token.clone().unwrap_or_default());
        headers.set("X-Bz-File-Name", url_helpers::url_encode(destination_path, false));
        headers.set("Content-Length", length.to_string());
        headers.set("X-Bz-Content-Sha1", sha1);
        headers.set("X-Bz-Info-src_last_modified_millis", chrono::Utc::now().timestamp_millis().to_string());
        headers.set("X-Bz-Info-b2-content-disposition", url_helpers::url_encode(&disposition, false));
        let content_type = mime_types::get_mime_type_from_file_name(destination_path);
        let url = upload_url.upload_url.clone().unwrap_or_default();
        let Some(res) = self.base.get_response_stream(HttpMethod::POST, &url, Some(data), Some(&content_type), &[], Some(&headers), true)? else {
            return Ok(B2UploadOutcome::ConnectionFailed);
        };
        if res.status_code != 200 {
            return Ok(B2UploadOutcome::Error(res.status_code, parse_b2_error(&res)));
        }
        Ok(B2UploadOutcome::Ok(super::deserialize_object(res.response_text.as_deref())?))
    }
}

/// `JToken.ToString()` of a value token: a string's text, other values as JSON.
fn json_token_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => json::to_dotnet_string(other).ok().flatten().unwrap_or_else(|| json::to_string(other)),
    }
}

fn is_authorized_for_upload(auth: &B2Auth, bucket_id: &str, destination_path: &str) -> Result<(), &'static str> {
    let allowed = auth.allowed.as_ref();
    if let Some(allowed_bucket) = allowed.and_then(|a| get_str(a, "bucketId"))
        && bucket_id != allowed_bucket
    {
        return Err("No permission to upload to this bucket. Are you using the right application key?");
    }
    if let Some(prefix) = allowed.and_then(|a| get_str(a, "namePrefix"))
        && !destination_path.starts_with(&prefix)
    {
        return Err("Your upload path conflicts with the key's name prefix setting.");
    }
    if let Some(Value::Array(caps)) = allowed.and_then(|a| get(a, "capabilities"))
        && !caps.iter().any(|c| c.as_str() == Some("writeFiles"))
    {
        return Err("Your key does not allow uploading to this bucket.");
    }
    Ok(())
}

fn parse_b2_error(res: &ResponseInfo) -> Option<B2Error> {
    if res.is_success() {
        return None;
    }
    let v = json::deserialize(res.response_text.as_deref().unwrap_or("")).ok()??;
    if !v.is_object() {
        return None;
    }
    Some(B2Error { status: json::get_i64(&v, "status"), code: get_str(&v, "code"), message: get_str(&v, "message") })
}

fn stringify_b2_error(res: &ResponseInfo) -> String {
    match parse_b2_error(res) {
        None => format!("Status {}, unknown error.", http::status_code_name(res.status_code)),
        Some(err) => {
            let message = err.message.unwrap_or_default();
            let colon_space = if message.trim().is_empty() { "" } else { ": " };
            format!("Got status {} ({}){colon_space}{message}", err.status, err.code.unwrap_or_default())
        }
    }
}

/// `new ContentDisposition(type) { FileName = name }.ToString()`: a token
/// as is, other ASCII quoted, anything else MIME-encoded (`=?utf-8?B?...?=`).
pub fn content_disposition(disposition_type: &str, file_name: &str) -> String {
    format!("{disposition_type}; filename={}", mime_parameter_value(file_name))
}

/// `ContentDisposition.EncodeToBuffer`.
pub(crate) fn mime_parameter_value(value: &str) -> String {
    let ascii = value.chars().all(|c| (c as u32) < 128 && c != '\r' && c != '\n');
    if !ascii {
        return format!("\"=?utf-8?B?{}?=\"", base64(value.as_bytes()));
    }
    let is_token = !value.is_empty() && value.bytes().all(|b| (33..=126).contains(&b) && !b"()<>@,;:\\\"/[]?=".contains(&b));
    if is_token {
        value.to_owned()
    } else {
        let mut s = String::from("\"");
        for c in value.chars() {
            if c == '\\' || c == '"' {
                s.push('\\');
            }
            s.push(c);
        }
        s.push('"');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azure_urls() {
        let a = AzureStorage::new("acc", "a2V5", "box", "", "", "", "");
        assert_eq!(a.generate_url("a%2Fb.png", true), "https://acc.blob.core.windows.net/box/a%2Fb.png");
        let mut root = AzureStorage::new("acc", "a2V5", "$root", "", "", "", "");
        assert_eq!(root.preview_url(), "https://acc.blob.core.windows.net/example.png");
        root.custom_domain = "cdn.example.com".into();
        assert_eq!(root.preview_url(), "https://cdn.example.com/example.png");
        assert_eq!(uri_escape_data_string("a b/é~"), "a%20b%2F%C3%A9~");
    }

    #[test]
    fn content_dispositions() {
        assert_eq!(content_disposition("inline", "a.png"), "inline; filename=a.png");
        assert_eq!(content_disposition("inline", "my shot.png"), "inline; filename=\"my shot.png\"");
        assert_eq!(content_disposition("inline", "é.png"), "inline; filename=\"=?utf-8?B?w6kucG5n?=\"");
        assert_eq!(http::status_code_name(404), "NotFound");
        assert_eq!(http::status_code_name(499), "499");
    }
}
