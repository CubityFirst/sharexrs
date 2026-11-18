//! Account-based HTTP services: `FileUploaders/Hostr.cs`, `Plik.cs`,
//! `Seafile.cs`, `Pushbullet.cs` (and `SharingServices/
//! PushbulletSharingService.cs`), `SendSpace.cs` + `SendSpaceManager.cs`,
//! `MediaFire.cs`, `ImageUploaders/ImageShackUploader.cs`.

use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use md5::Md5;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use sharex_helpers::{NameParserType, mime_types};
use sharex_strings::uploaders_lib as strings;

use super::cloud_storage::name_parse;
use super::{NULL_REFERENCE, deserialize_object, null_or_empty, uploader_base};
use crate::args;
use crate::config::PlikSettings;
use crate::dpapi::Secret;
use crate::enums::{AccountType, HttpMethod};
use crate::http::{self, SharedStream};
use crate::json::{self, get, get_i64, get_str};
use crate::result::UploadResult;
use crate::uploader::{Args, CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig, create_authentication_header, exception};
use crate::url_helpers;

fn null_arg(param: &str) -> UploadError {
    UploadError::Exception(format!("Value cannot be null. (Parameter '{param}')"))
}

fn null_ref() -> UploadError {
    UploadError::Exception(NULL_REFERENCE.into())
}

/// `Task.Delay(duration, cancellationToken)` on an uploader.
pub(crate) fn delay(base: &Uploader, duration: Duration) -> UResult<()> {
    let end = Instant::now() + duration;
    loop {
        if base.stop_requested() {
            return Err(UploadError::Cancelled);
        }
        let now = Instant::now();
        if now >= end {
            return Ok(());
        }
        std::thread::sleep((end - now).min(Duration::from_millis(50)));
    }
}

/// `BytesToHexadecimal` joined: lower-case hex.
fn hex_lower(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

// ---------------------------------------------------------------------------
// Hostr

/// `Hostr` (Localhostr).
pub struct Hostr {
    pub base: Uploader,
    pub email: String,
    pub password: String,
    pub direct_url: bool,
}

uploader_base!(Hostr, FileUploader);
impl WritesBackConfig for Hostr {}

impl Hostr {
    pub fn new(email: &str, password: &str) -> Self {
        Self { base: Uploader::new(), email: email.to_owned(), password: password.to_owned(), direct_url: false }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.email.is_empty() || self.password.is_empty() {
            return Ok(None);
        }
        let headers = create_authentication_header(&self.email, &self.password);
        let mut result = self.base.send_request_file_simple("https://api.hostr.co/file", data, file_name, "file", &[], Some(&headers))?;
        if result.is_success()
            && let Some(response) = deserialize_object(result.response.as_deref())?
        {
            let direct = get(&response, "direct").filter(|d| !d.is_null());
            if self.direct_url && direct.is_some() {
                result.url = Some(format!(
                    "http://hostr.co/file/{}/{}",
                    get_str(&response, "id").unwrap_or_default(),
                    get_str(&response, "name").unwrap_or_default()
                ));
                result.thumbnail_url = direct.and_then(|d| d.get("150x")).and_then(|v| json::to_dotnet_string(v).ok().flatten());
            } else {
                result.url = get_str(&response, "href");
            }
        }
        Ok(Some(result))
    }
}

// ---------------------------------------------------------------------------
// Plik

/// `Plik`.
pub struct Plik {
    pub base: Uploader,
    pub settings: PlikSettings,
}

uploader_base!(Plik, FileUploader);
impl WritesBackConfig for Plik {}

/// `Plik.GetMultiplyIndex(newUnit, oldUnit)` (0 days, 1 hours, 2 minutes).
pub fn plik_multiply_index(new_unit: i32, old_unit: i32) -> f64 {
    match (new_unit, old_unit) {
        (0, 1) => 1.0 / 24.0,
        (0, 2) => 1.0 / 24.0 / 60.0,
        (1, 0) => 24.0,
        (1, 2) => 1.0 / 60.0,
        (2, 0) => 60.0 * 24.0,
        (2, 1) => 60.0,
        _ => 1.0,
    }
}

impl Plik {
    pub fn new(settings: PlikSettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let base_url = self.settings.URL.clone().unwrap_or_default();
        if base_url.is_empty() {
            return exception(strings::Plik_Host_is_empty());
        }
        let mut headers = Headers::new();
        headers.set("X-PlikToken", self.settings.APIKey.as_str());
        let size = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let ttl = if self.settings.TTLUnit != 3 {
            // Convert.ToInt32(decimal): rounds half to even.
            (plik_multiply_index(2, self.settings.TTLUnit) * self.settings.TTL * 60.0).round_ties_even() as i64
        } else {
            -1
        };
        let s = |v: Option<&str>| json::str_or_null(v);
        let meta = json::object([
            ("ttl", Value::from(ttl)),
            ("removable", Value::Bool(self.settings.Removable)),
            ("oneShot", Value::Bool(self.settings.OneShot)),
            ("comments", if self.settings.HasComment { s(self.settings.Comment.as_deref()) } else { Value::Null }),
            ("login", if self.settings.IsSecured { s(self.settings.Login.as_deref()) } else { Value::Null }),
            ("password", if self.settings.IsSecured { s(self.settings.Password.get()) } else { Value::Null }),
            (
                "files",
                json::object([(
                    "0",
                    json::object([
                        ("fileName", Value::String(file_name.to_owned())),
                        ("fileType", Value::String(mime_types::get_mime_type_from_file_name(file_name))),
                        ("fileSize", Value::from(size as i64)),
                    ]),
                )]),
            ),
        ]);
        let response =
            self.base.send_request_text(HttpMethod::POST, &format!("{base_url}/upload"), &json::to_string(&meta), None, &[], Some(&headers))?;
        let meta_data = deserialize_object(response.as_deref())?.ok_or_else(null_ref)?;
        headers.set("x-uploadtoken", get_str(&meta_data, "uploadToken").unwrap_or_default());
        let files = get(&meta_data, "files").filter(|f| !f.is_null()).ok_or_else(|| null_arg("source"))?;
        let (_, first) = files.as_object().and_then(|f| f.iter().next()).ok_or(UploadError::Exception("Sequence contains no elements".into()))?;
        let id = get_str(&meta_data, "id").unwrap_or_default();
        let file_id = get_str(first, "id").unwrap_or_default();
        let url = format!("{base_url}/file/{id}/{file_id}/{file_name}");
        let file_data = self.base.send_request_file_simple(&url, data, file_name, "file", &[], Some(&headers))?;
        let mut result = UploadResult::with_response(file_data.response, None);
        result.url = Some(format!(
            "{base_url}/file/{id}/{file_id}/{}",
            url_helpers::url_encode(&get_str(first, "fileName").unwrap_or_default(), false)
        ));
        Ok(Some(result))
    }
}

// ---------------------------------------------------------------------------
// Seafile

/// `Seafile`.
pub struct Seafile {
    pub base: Uploader,
    pub api_url: String,
    pub auth_token: String,
    pub repo_id: String,
    pub path: String,
    pub is_library_encrypted: bool,
    pub encrypted_library_password: String,
    pub share_days_to_expire: i32,
    pub share_password: String,
    pub create_shareable_url: bool,
    pub create_shareable_url_raw: bool,
}

uploader_base!(Seafile, FileUploader);
impl WritesBackConfig for Seafile {}

impl Seafile {
    pub fn new(api_url: &str, auth_token: &str, repo_id: &str) -> Self {
        Self {
            base: Uploader::new(),
            api_url: api_url.to_owned(),
            auth_token: auth_token.to_owned(),
            repo_id: repo_id.to_owned(),
            path: String::new(),
            is_library_encrypted: false,
            encrypted_library_password: String::new(),
            share_days_to_expire: 0,
            share_password: String::new(),
            create_shareable_url: true,
            create_shareable_url_raw: false,
        }
    }

    fn api(&self, path: &str) -> String {
        url_helpers::combine_url(&url_helpers::fix_prefix(&self.api_url), path)
    }

    fn token_headers(&self, token: Option<&str>) -> Headers {
        Headers::new().with("Authorization", format!("Token {}", token.unwrap_or(&self.auth_token)))
    }

    /// `GetAuthTokenAsync(username, password)`.
    pub fn get_auth_token(&mut self, username: &str, password: &str) -> UResult<String> {
        let url = self.api("auth-token/?format=json");
        let args = args![("username", username), ("password", password)];
        let response = self.base.send_request_multipart(&url, &args, None, HttpMethod::POST)?;
        if !null_or_empty(response.as_deref()) {
            let r = deserialize_object(response.as_deref())?.ok_or_else(null_ref)?;
            return Ok(get_str(&r, "token").unwrap_or_default());
        }
        Ok(String::new())
    }

    fn ping(&mut self, path: &str, auth: bool) -> UResult<bool> {
        let url = self.api(path);
        let headers = auth.then(|| self.token_headers(None));
        let response = self.base.send_request(HttpMethod::GET, &url, &[], headers.as_ref())?;
        Ok(response.as_deref() == Some("\"pong\""))
    }

    /// `CheckAPIURLAsync`.
    pub fn check_api_url(&mut self) -> UResult<bool> {
        self.ping("ping/?format=json", false)
    }

    /// `CheckAuthTokenAsync`.
    pub fn check_auth_token(&mut self) -> UResult<bool> {
        self.ping("auth/ping/?format=json", true)
    }

    /// `GetAccountInfoAsync`: (usage, total, email).
    pub fn get_account_info(&mut self) -> UResult<Option<(i64, i64, Option<String>)>> {
        let url = self.api("account/info/?format=json");
        let headers = self.token_headers(None);
        let response = self.base.send_request(HttpMethod::GET, &url, &[], Some(&headers))?;
        if null_or_empty(response.as_deref()) {
            return Ok(None);
        }
        Ok(deserialize_object(response.as_deref())?.map(|r| (get_i64(&r, "usage"), get_i64(&r, "total"), get_str(&r, "email"))))
    }

    /// `GetOrMakeDefaultLibraryAsync`.
    pub fn get_or_make_default_library(&mut self, auth_token: Option<&str>) -> UResult<Option<String>> {
        let url = self.api("default-repo/?format=json");
        let headers = self.token_headers(auth_token);
        let response = self.base.send_request(HttpMethod::GET, &url, &[], Some(&headers))?;
        if null_or_empty(response.as_deref()) {
            return Ok(None);
        }
        deserialize_object(response.as_deref())?.ok_or_else(null_ref).map(|r| get_str(&r, "repo_id"))
    }

    /// `GetLibrariesAsync`: the libraries as the server lists them.
    pub fn get_libraries(&mut self) -> UResult<Option<Vec<Value>>> {
        let url = self.api("repos/?format=json");
        let headers = self.token_headers(None);
        let response = self.base.send_request(HttpMethod::GET, &url, &[], Some(&headers))?;
        if null_or_empty(response.as_deref()) {
            return Ok(None);
        }
        match json::deserialize(response.as_deref().unwrap_or("")).map_err(UploadError::Exception)? {
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(_) => exception("Cannot deserialize the current JSON object into a list."),
            None => Ok(None),
        }
    }

    /// `ValidatePathAsync`.
    pub fn validate_path(&mut self, path: &str) -> UResult<bool> {
        let url = self.api(&format!("repos/{}/dir/?p={path}&format=json", self.repo_id));
        let headers = self.token_headers(None);
        Ok(!null_or_empty(self.base.send_request(HttpMethod::GET, &url, &[], Some(&headers))?.as_deref()))
    }

    /// `DecryptLibraryAsync`.
    pub fn decrypt_library(&mut self, library_password: &str) -> UResult<bool> {
        let url = self.api(&format!("repos/{}/?format=json", self.repo_id));
        let headers = self.token_headers(None);
        let args = args![("password", library_password)];
        let response = self.base.send_request_multipart(&url, &args, Some(&headers), HttpMethod::POST)?;
        Ok(response.as_deref() == Some("\"success\""))
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.api_url.is_empty() {
            return exception(strings::Seafile_API_URL_is_empty());
        }
        if self.auth_token.is_empty() {
            return exception(strings::Seafile_Authentication_token_is_empty());
        }
        if self.path.is_empty() {
            self.path = "/".into();
        } else if !self.path.ends_with('/') {
            self.path.push('/');
        }
        let url = self.api(&format!("repos/{}/upload-link/?format=json", self.repo_id));
        let headers = self.token_headers(None);
        let response = self.base.send_request(HttpMethod::GET, &url, &[], Some(&headers))?;
        let response_url = response.ok_or_else(null_ref)?.trim_matches('"').to_owned();
        let args = args![("filename", file_name), ("parent_dir", self.path.clone())];
        let mut result = self.base.send_request_file_simple(&response_url, data, file_name, "file", &args, Some(&headers))?;
        if !self.base.is_error() {
            if self.create_shareable_url && !self.is_library_encrypted {
                self.base.allow_report_progress = false;
                let path = format!("{}{file_name}", self.path);
                result.url = self.share_file(&path)?;
                if self.create_shareable_url_raw {
                    let url = result.url.clone().ok_or_else(|| null_arg("uriString"))?;
                    result.url = Some(raw_share_url(&url));
                }
            } else {
                result.is_url_expected = false;
            }
        }
        Ok(Some(result))
    }

    /// `ShareFileAsync`: the shared link (the `Location` header).
    pub fn share_file(&mut self, path: &str) -> UResult<Option<String>> {
        let url = url_helpers::combine_urls(&[&url_helpers::fix_prefix(&self.api_url), "repos", &self.repo_id, "file/shared-link/"]);
        let mut args: Args = args![("p", path), ("share_type", "download")];
        if !self.share_password.is_empty() {
            args.push(("password".into(), self.share_password.clone()));
        }
        if self.share_days_to_expire > 0 {
            args.push(("expire".into(), self.share_days_to_expire.to_string()));
        }
        let headers = self.token_headers(None);
        self.base.send_request_url_encoded(HttpMethod::PUT, &url, &args, Some(&headers))?;
        Ok(self.base.last_response_info.as_ref().and_then(|r| r.header("Location").map(str::to_owned)))
    }
}

/// Seafile's raw link: the query parsed, `raw=1` set, written back as
/// `scheme://host/path?query` (`UriBuilder` + `HttpValueCollection`).
fn raw_share_url(url: &str) -> String {
    let full = if url_helpers::has_prefix(url) { url.to_owned() } else { format!("http://{url}") };
    let (scheme, rest) = full.split_once("://").unwrap_or(("http", &full));
    let rest = rest.split('#').next().unwrap_or("");
    let (authority, path_query) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = match host.rfind(':') {
        Some(i) if !host.ends_with(']') => &host[..i],
        _ => host,
    };
    let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
    let mut pairs: Vec<(Option<String>, Vec<String>)> = url_helpers::parse_query(query);
    match pairs.iter_mut().find(|(k, _)| k.as_deref().is_some_and(|k| k.eq_ignore_ascii_case("raw"))) {
        Some((_, v)) => *v = vec!["1".into()],
        None => pairs.push((Some("raw".into()), vec!["1".into()])),
    }
    let mut parts = Vec::new();
    for (k, values) in &pairs {
        let key = k.as_deref().map(|k| format!("{}=", url_helpers::http_utility_url_encode(k))).unwrap_or_default();
        if values.is_empty() {
            parts.push(key.clone());
        }
        for v in values {
            parts.push(format!("{key}{}", url_helpers::http_utility_url_encode(v)));
        }
    }
    format!("{scheme}://{}{path}?{}", host.to_ascii_lowercase(), parts.join("&"))
}

// ---------------------------------------------------------------------------
// Pushbullet

/// `PushbulletDevice`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct PushbulletDevice {
    #[serde(default)]
    pub Key: Option<String>,
    #[serde(default)]
    pub Name: Option<String>,
}

/// `PushbulletSettings` (`UploadersConfig.PushbulletSettings`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct PushbulletSettings {
    #[serde(default)]
    pub UserAPIKey: Secret,
    #[serde(default)]
    pub DeviceList: Option<Vec<PushbulletDevice>>,
    #[serde(default)]
    pub SelectedDevice: i32,
}

impl PushbulletSettings {
    /// Reads `UploadersConfig.PushbulletSettings`.
    pub fn from_value(v: &Value) -> Option<Self> {
        if v.is_null() { None } else { serde_json::from_value(v.clone()).ok() }
    }

    /// `CurrentDevice`.
    pub fn current_device(&self) -> Option<&PushbulletDevice> {
        usize::try_from(self.SelectedDevice).ok().and_then(|i| self.DeviceList.as_ref()?.get(i))
    }

    /// `CheckConfig` of the file uploader and the sharing service.
    pub fn check(settings: Option<&Self>) -> bool {
        settings.is_some_and(|s| !s.UserAPIKey.is_empty() && s.current_device().is_some())
    }
}

const PUSHBULLET_WWW_PUSHES_URL: &str = "https://www.pushbullet.com/pushes";
const PUSHBULLET_API_GET_DEVICES_URL: &str = "https://api.pushbullet.com/v2/devices";
const PUSHBULLET_API_SEND_PUSH_URL: &str = "https://api.pushbullet.com/v2/pushes";
const PUSHBULLET_API_REQUEST_FILE_UPLOAD_URL: &str = "https://api.pushbullet.com/v2/upload-request";

/// `Pushbullet`.
pub struct Pushbullet {
    pub base: Uploader,
    pub config: PushbulletSettings,
}

uploader_base!(Pushbullet, FileUploader);
impl WritesBackConfig for Pushbullet {}

impl Pushbullet {
    pub fn new(config: PushbulletSettings) -> Self {
        Self { base: Uploader::new(), config }
    }

    fn headers(&self) -> Headers {
        create_authentication_header(self.config.UserAPIKey.as_str(), "")
    }

    fn device_key(&self) -> UResult<String> {
        Ok(self.config.current_device().ok_or_else(null_ref)?.Key.clone().unwrap_or_default())
    }

    /// `PushFileAsync`.
    pub fn push_file(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let headers = self.headers();
        let up_args = args![("file_name", file_name)];
        let Some(upload_request) = self.base.send_request_multipart(PUSHBULLET_API_REQUEST_FILE_UPLOAD_URL, &up_args, Some(&headers), HttpMethod::POST)?
        else {
            return Ok(None);
        };
        let Some(file_info) = deserialize_object(Some(&upload_request))? else { return Ok(None) };
        let info_data = get(&file_info, "data").filter(|d| !d.is_null()).ok_or_else(null_ref)?;
        let d = |k: &str| get_str(info_data, k).unwrap_or_default();
        let aws_args = args![
            ("awsaccesskeyid", d("awsaccesskeyid")),
            ("acl", d("acl")),
            ("key", d("key")),
            ("signature", d("signature")),
            ("policy", d("policy")),
            ("content-type", d("content-type")),
        ];
        let upload_url = get_str(&file_info, "upload_url").ok_or_else(|| null_arg("requestUri"))?;
        let mut upload_result = self.base.send_request_file_simple(&upload_url, data, file_name, "file", &aws_args, None)?;
        let mut push_args = up_args;
        push_args.extend(args![
            ("device_iden", self.device_key()?),
            ("type", "file"),
            ("file_url", get_str(&file_info, "file_url").unwrap_or_default()),
            ("body", "Sent via ShareX"),
            ("file_type", get_str(&file_info, "file_type").unwrap_or_default()),
        ]);
        let Some(push_result) = self.base.send_request_multipart(PUSHBULLET_API_SEND_PUSH_URL, &push_args, Some(&headers), HttpMethod::POST)? else {
            return Ok(None);
        };
        if let Some(push) = deserialize_object(Some(&push_result))? {
            upload_result.url = Some(format!("{PUSHBULLET_WWW_PUSHES_URL}?push_iden={}", get_str(&push, "iden").unwrap_or_default()));
        }
        Ok(Some(upload_result))
    }

    fn push(&mut self, push_type: &str, value_type: &str, value: &str, title: &str) -> UResult<Option<String>> {
        let headers = self.headers();
        let mut args = args![("device_iden", self.device_key()?), ("type", push_type), ("title", title), (value_type, value)];
        if value_type != "body" {
            args.push(("body".into(), if push_type == "link" { value.to_owned() } else { "Sent via ShareX".to_owned() }));
        }
        let Some(response) = self.base.send_request_multipart(PUSHBULLET_API_SEND_PUSH_URL, &args, Some(&headers), HttpMethod::POST)? else {
            return Ok(None);
        };
        Ok(deserialize_object(Some(&response))?
            .map(|push| format!("{PUSHBULLET_WWW_PUSHES_URL}?push_iden={}", get_str(&push, "iden").unwrap_or_default())))
    }

    /// `PushNoteAsync`.
    pub fn push_note(&mut self, note: &str, title: &str) -> UResult<Option<String>> {
        self.push("note", "body", note, title)
    }

    /// `PushLinkAsync`.
    pub fn push_link(&mut self, link: &str, title: &str) -> UResult<Option<String>> {
        self.push("link", "url", link, title)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.config.UserAPIKey.is_empty() {
            return exception(strings::Common_API_key_is_missing());
        }
        let Some(device) = self.config.current_device() else { return exception(strings::Pushbullet_No_device_selected()) };
        if device.Key.as_deref().is_none_or(str::is_empty) {
            return exception(strings::Pushbullet_Device_key_is_missing());
        }
        self.push_file(data, file_name)
    }

    /// `GetDeviceListAsync`: the devices that have a nickname, for the
    /// Destination settings' device picker.
    pub fn get_device_list(&mut self) -> UResult<Vec<PushbulletDevice>> {
        let headers = self.headers();
        let response = self.base.send_request(HttpMethod::GET, PUSHBULLET_API_GET_DEVICES_URL, &[], Some(&headers))?;
        let r = deserialize_object(response.as_deref())?;
        let mut list = Vec::new();
        if let Some(Value::Array(devices)) = r.as_ref().and_then(|r| get(r, "devices")) {
            for d in devices {
                let nickname = get_str(d, "nickname");
                if nickname.as_deref().is_some_and(|n| !n.is_empty()) {
                    list.push(PushbulletDevice { Key: get_str(d, "iden"), Name: nickname });
                }
            }
        }
        Ok(list)
    }
}

/// `PushbulletSharer`: pushes the URL as a link to the selected device.
pub struct PushbulletSharer {
    pub base: Uploader,
    pub settings: PushbulletSettings,
}

uploader_base!(PushbulletSharer, UrlSharer);
impl WritesBackConfig for PushbulletSharer {}

impl PushbulletSharer {
    pub fn new(settings: PushbulletSettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn share(&mut self, url: &str) -> CoreResult {
        let result = UploadResult { url: Some(url.to_owned()), is_url_expected: false, ..Default::default() };
        let mut pushbullet = Pushbullet::new(self.settings.clone());
        std::mem::swap(&mut pushbullet.base, &mut self.base);
        let r = pushbullet.push_link(url, "ShareX: URL share");
        std::mem::swap(&mut pushbullet.base, &mut self.base);
        // `new Pushbullet(...)`'s errors stay with that uploader in ShareX.
        self.base.errors = Default::default();
        r?;
        Ok(Some(result))
    }
}

// ---------------------------------------------------------------------------
// SendSpace

const SENDSPACE_API_URL: &str = "http://api.sendspace.com/rest/";
const SENDSPACE_API_VERSION: &str = "1.0";

/// `SendSpace.UploadInfo`.
#[derive(Debug, Clone, Default)]
pub struct SendSpaceUploadInfo {
    pub url: String,
    pub progress_url: String,
    pub max_file_size: String,
    pub upload_identifier: String,
    pub extra_info: String,
}

/// `SendSpaceManager`'s static state, kept for the process as in ShareX.
#[derive(Default)]
struct SendSpaceManager {
    token: Option<String>,
    session_key: Option<String>,
    last_session_key: Option<Instant>,
    upload_info: Option<SendSpaceUploadInfo>,
}

static SENDSPACE_MANAGER: LazyLock<Mutex<SendSpaceManager>> = LazyLock::new(Default::default);

/// `SendSpace`.
pub struct SendSpace {
    pub base: Uploader,
    api_key: String,
    pub account_type: AccountType,
    pub username: String,
    pub password: String,
    pub speed_limit: i32,
    pub app_version: String,
}

uploader_base!(SendSpace, FileUploader);
impl WritesBackConfig for SendSpace {}

/// `XElement.Value`: the text of every descendant.
fn xml_value(node: roxmltree::Node) -> String {
    node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect()
}

/// `GetElementValue`: a child element's value, or "".
fn xml_element_value(node: Option<roxmltree::Node>, name: &str) -> String {
    node.and_then(|n| n.children().find(|c| c.is_element() && c.tag_name().name() == name)).map(xml_value).unwrap_or_default()
}

fn xml_parse(text: &str) -> UResult<roxmltree::Document<'_>> {
    roxmltree::Document::parse(text).map_err(|e| UploadError::Exception(e.to_string()))
}

impl SendSpace {
    pub fn new(api_key: &str) -> Self {
        Self {
            base: Uploader::new(),
            api_key: api_key.to_owned(),
            account_type: AccountType::Anonymous,
            username: String::new(),
            password: String::new(),
            speed_limit: 0,
            app_version: "1.0".into(),
        }
    }

    /// `ParseResponse`: (error, the `result` element's children as text).
    fn parse_response(&mut self, response: &str, read: impl FnOnce(roxmltree::Node) -> Option<SendSpaceUploadInfo>) -> UResult<(bool, Option<SendSpaceUploadInfo>, String)> {
        let doc = xml_parse(response)?;
        let root = doc.root_element();
        let result = (root.tag_name().name() == "result").then_some(root).ok_or_else(null_ref)?;
        let method = result.attribute("method").ok_or_else(null_ref)?.to_owned();
        let status = result.attribute("status").ok_or_else(null_ref)?.to_owned();
        let error = status == "fail";
        if error {
            let e = result.children().find(|c| c.is_element() && c.tag_name().name() == "error").ok_or_else(null_ref)?;
            let code = e.attribute("code").ok_or_else(null_ref)?;
            let text = e.attribute("text").ok_or_else(null_ref)?;
            self.base.errors.add(strings::SendSpace_Error_details().replace("{0}", code).replace("{1}", &method).replace("{2}", text));
            return Ok((true, None, String::new()));
        }
        let token = xml_element_value(Some(result), "token");
        let session = xml_element_value(Some(result), "session_key");
        Ok((false, read(result), if token.is_empty() { session } else { token }))
    }

    fn upload_info(node: roxmltree::Node) -> Option<SendSpaceUploadInfo> {
        let upload = node.children().find(|c| c.is_element() && c.tag_name().name() == "upload");
        let a = |name: &str| upload.and_then(|u| u.attribute(name)).unwrap_or("").to_owned();
        Some(SendSpaceUploadInfo {
            url: a("url"),
            progress_url: a("progress_url"),
            max_file_size: a("max_file_size"),
            upload_identifier: a("upload_identifier"),
            extra_info: a("extra_info"),
        })
    }

    /// `AuthCreateTokenAsync`.
    pub fn auth_create_token(&mut self) -> UResult<Option<String>> {
        let args = args![
            ("method", "auth.createToken"),
            ("api_key", self.api_key.clone()),
            ("api_version", SENDSPACE_API_VERSION),
            ("app_version", self.app_version.clone()),
            ("response_format", "xml"),
        ];
        let response = self.base.send_request_multipart(SENDSPACE_API_URL, &args, None, HttpMethod::POST)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let (error, _, token) = self.parse_response(&r, |_| None)?;
            if !error {
                return Ok(Some(token));
            }
        }
        Ok(None)
    }

    /// `AuthLoginAsync`: the session key.
    pub fn auth_login(&mut self, token: &str, username: &str, password: &str) -> UResult<Option<String>> {
        let password_hash = hex_lower(&Md5::digest(password.as_bytes()));
        let args = args![
            ("method", "auth.login"),
            ("token", token),
            ("user_name", username),
            ("tokened_password", hex_lower(&Md5::digest(format!("{token}{password_hash}").as_bytes()))),
        ];
        let response = self.base.send_request_multipart(SENDSPACE_API_URL, &args, None, HttpMethod::POST)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let doc_text = r.clone();
            let (error, _, _) = self.parse_response(&r, |_| None)?;
            if !error {
                let doc = xml_parse(&doc_text)?;
                return Ok(Some(xml_element_value(Some(doc.root_element()), "session_key")));
            }
        }
        Ok(None)
    }

    fn get_info(&mut self, args: Args) -> UResult<Option<SendSpaceUploadInfo>> {
        let response = self.base.send_request(HttpMethod::GET, SENDSPACE_API_URL, &args, None)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let (error, info, _) = self.parse_response(&r, Self::upload_info)?;
            if !error {
                return Ok(info);
            }
        }
        Ok(None)
    }

    /// `UploadGetInfoAsync`.
    pub fn upload_get_info(&mut self, session_key: &str) -> UResult<Option<SendSpaceUploadInfo>> {
        self.get_info(args![("method", "upload.getInfo"), ("session_key", session_key), ("speed_limit", self.speed_limit)])
    }

    /// `AnonymousUploadGetInfoAsync`.
    pub fn anonymous_upload_get_info(&mut self) -> UResult<Option<SendSpaceUploadInfo>> {
        self.get_info(args![
            ("method", "anonymous.uploadGetInfo"),
            ("speed_limit", self.speed_limit),
            ("api_key", self.api_key.clone()),
            ("api_version", SENDSPACE_API_VERSION),
            ("app_version", self.app_version.clone()),
        ])
    }

    /// `SendSpaceManager.PrepareUploadInfoAsync`: through a second
    /// `SendSpace` whose errors are dropped, as ShareX's.
    fn prepare_upload_info(&mut self, username: Option<&str>, password: Option<&str>) -> UResult<()> {
        let mut helper = SendSpace::new(&self.api_key);
        helper.base = self.base.fork();
        let mut m = SENDSPACE_MANAGER.lock().expect("sendspace manager");
        let r: UResult<()> = (|| {
            match (username.filter(|u| !u.is_empty()), password.filter(|p| !p.is_empty())) {
                (Some(username), Some(password)) => {
                    if m.token.as_deref().is_none_or(str::is_empty) {
                        m.token = helper.auth_create_token()?;
                        if m.token.as_deref().is_none_or(str::is_empty) {
                            return exception(strings::SendSpace_Token_is_missing());
                        }
                    }
                    if m.session_key.as_deref().is_none_or(str::is_empty) || m.last_session_key.is_none_or(|t| t.elapsed() > Duration::from_secs(30 * 60)) {
                        let token = m.token.clone().unwrap_or_default();
                        m.session_key = helper.auth_login(&token, username, password)?;
                        if m.session_key.as_deref().is_none_or(str::is_empty) {
                            return exception(strings::SendSpace_Session_key_is_missing());
                        }
                        m.last_session_key = Some(Instant::now());
                    }
                    let key = m.session_key.clone().unwrap_or_default();
                    m.upload_info = helper.upload_get_info(&key)?;
                }
                _ => m.upload_info = helper.anonymous_upload_get_info()?,
            }
            if m.upload_info.is_none() {
                return exception(strings::SendSpace_Upload_information_is_missing());
            }
            Ok(())
        })();
        match r {
            Err(UploadError::Cancelled) => Err(UploadError::Cancelled),
            _ => Ok(()),
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if self.account_type == AccountType::User {
            let (u, p) = (self.username.clone(), self.password.clone());
            self.prepare_upload_info(Some(&u), Some(&p))?;
        } else {
            self.prepare_upload_info(None, None)?;
        }
        let info = SENDSPACE_MANAGER.lock().expect("sendspace manager").upload_info.clone();
        self.upload_with(data, file_name, info)
    }

    /// `UploadAsync(stream, fileName, uploadInfo)`.
    pub fn upload_with(&mut self, data: &SharedStream, file_name: &str, info: Option<SendSpaceUploadInfo>) -> CoreResult {
        let Some(info) = info else { return Ok(None) };
        let args = args![("MAX_FILE_SIZE", info.max_file_size), ("UPLOAD_IDENTIFIER", info.upload_identifier), ("extra_info", info.extra_info)];
        let mut result = self.base.send_request_file_simple(&info.url, data, file_name, "userfile", &args, None)?;
        if result.is_success() {
            let response = result.response.clone().unwrap_or_default();
            if response.starts_with("upload_status=ok") {
                let id = fancy_regex::Regex::new(r"file_id=(\w+)")
                    .ok()
                    .and_then(|re| re.captures(&response).ok().flatten())
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
                    .unwrap_or_default();
                result.url = Some(format!("http://www.sendspace.com/file/{id}"));
            } else {
                let doc = xml_parse(&response)?;
                let root = doc.root_element();
                if xml_element_value(Some(root), "status") == "ok" {
                    result.url = Some(xml_element_value(Some(root), "download_url"));
                    result.deletion_url = Some(xml_element_value(Some(root), "delete_url"));
                } else {
                    return Err(null_ref());
                }
            }
        }
        Ok(Some(result))
    }
}

/// Clears `SendSpaceManager`'s static token and session (tests).
pub fn reset_sendspace_manager() {
    *SENDSPACE_MANAGER.lock().expect("sendspace manager") = SendSpaceManager::default();
}

// ---------------------------------------------------------------------------
// MediaFire

const MEDIAFIRE_API_URL: &str = "https://www.mediafire.com/api/";

/// `MediaFire`.
pub struct MediaFire {
    pub base: Uploader,
    pub upload_path: String,
    pub use_long_link: bool,
    app_id: String,
    api_key: String,
    user: String,
    pasw: String,
    session_token: String,
    signature_time: String,
    signature_key: i32,
    /// The wait between polls (1 s in ShareX).
    pub poll_interval: Duration,
}

uploader_base!(MediaFire, FileUploader);
impl WritesBackConfig for MediaFire {}

impl MediaFire {
    pub fn new(app_id: &str, api_key: &str, user: &str, pasw: &str) -> Self {
        Self {
            base: Uploader::new(),
            upload_path: String::new(),
            use_long_link: false,
            app_id: app_id.to_owned(),
            api_key: api_key.to_owned(),
            user: user.to_owned(),
            pasw: pasw.to_owned(),
            session_token: String::new(),
            signature_time: String::new(),
            signature_key: 0,
            poll_interval: Duration::from_millis(1000),
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        self.base.allow_report_progress = false;
        self.get_session_token()?;
        self.base.allow_report_progress = true;
        let key = self.simple_upload(data, file_name)?;
        self.base.allow_report_progress = false;
        let url = loop {
            if let Some(url) = self.poll_upload(&key, file_name)? {
                break url;
            }
            delay(&self.base, self.poll_interval)?;
        };
        Ok(Some(UploadResult { success: true, url: Some(url), ..Default::default() }))
    }

    /// `DeserializeResponse<T>`: the `response` object (missing: null).
    fn response(text: Option<&str>) -> UResult<Value> {
        let text = text.ok_or_else(|| null_arg("value"))?;
        let v = json::deserialize(text).map_err(UploadError::Exception)?;
        v.as_ref().and_then(|v| get(v, "response")).filter(|r| r.is_object()).cloned().ok_or_else(null_ref)
    }

    fn io_error(message: impl Into<String>) -> UploadError {
        UploadError::Exception(message.into())
    }

    fn could_not_upload(message: Option<String>) -> UploadError {
        Self::io_error(strings::MediaFire_Could_not_upload_file().replace("{0}", &message.unwrap_or_else(|| strings::Common_Unknown_error().to_owned())))
    }

    fn ensure_success(&mut self, resp: &Value) -> UResult<()> {
        if get_str(resp, "result").as_deref() != Some("Success") {
            return Err(Self::could_not_upload(get_str(resp, "message")));
        }
        if get_str(resp, "new_key").as_deref() == Some("yes") {
            self.signature_key = ((self.signature_key as i64 * 16807) % 2147483647) as i32;
        }
        Ok(())
    }

    fn get_session_token(&mut self) -> UResult<()> {
        let init_signature = hex_lower(&Sha1::digest(format!("{}{}{}{}", self.user, self.pasw, self.app_id, self.api_key).as_bytes()));
        let args = args![
            ("email", self.user.clone()),
            ("password", self.pasw.clone()),
            ("application_id", self.app_id.clone()),
            ("token_version", "2"),
            ("response_format", "json"),
            ("signature", init_signature),
        ];
        let resp_str = self.base.send_request_multipart(&format!("{MEDIAFIRE_API_URL}user/get_session_token.php"), &args, None, HttpMethod::POST)?;
        let resp = Self::response(resp_str.as_deref())?;
        self.ensure_success(&resp)?;
        let token = get_str(&resp, "session_token");
        let time = get_str(&resp, "time");
        let secret = get(&resp, "secret_key").filter(|v| !v.is_null());
        let (Some(token), Some(time), Some(secret)) = (token, time, secret) else {
            return Err(Self::io_error(strings::MediaFire_Invalid_response()));
        };
        self.session_token = token;
        self.signature_time = time;
        self.signature_key = secret.as_i64().or_else(|| secret.as_str().and_then(|s| s.parse().ok())).unwrap_or(0) as i32;
        Ok(())
    }

    fn signature(&self, url_suffix: &str, args: &Args) -> String {
        let key = self.signature_key % 256;
        let query = args.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
        let url = if args.is_empty() { format!("/api/{url_suffix}") } else { format!("/api/{url_suffix}?{query}") };
        hex_lower(&Md5::digest(format!("{key}{}{url}", self.signature_time).as_bytes()))
    }

    fn simple_upload(&mut self, data: &SharedStream, file_name: &str) -> UResult<String> {
        let mut args = args![("session_token", self.session_token.clone()), ("path", self.upload_path.clone()), ("response_format", "json")];
        let signature = self.signature("upload/simple.php", &args);
        args.push(("signature".into(), signature));
        let url = url_helpers::create_query_string_url(&format!("{MEDIAFIRE_API_URL}upload/simple.php"), &args, false);
        let res = self.base.send_request_file_simple(&url, data, file_name, "Filedata", &[], None)?;
        if !res.is_success() {
            return Err(Self::io_error(res.errors_to_string().unwrap_or_else(|| "I/O error occurred.".into())));
        }
        let resp = Self::response(res.response.as_deref())?;
        self.ensure_success(&resp)?;
        let doupload = get(&resp, "doupload").filter(|d| !d.is_null()).ok_or_else(null_ref)?;
        let result = get(doupload, "result").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())));
        let key = get_str(doupload, "key");
        match (result, key) {
            (Some(0), Some(key)) => Ok(key),
            _ => Err(Self::io_error(strings::MediaFire_Invalid_response())),
        }
    }

    fn poll_upload(&mut self, upload_key: &str, file_name: &str) -> UResult<Option<String>> {
        let mut args =
            args![("session_token", self.session_token.clone()), ("key", upload_key), ("filename", file_name), ("response_format", "json")];
        let signature = self.signature("upload/poll_upload.php", &args);
        args.push(("signature".into(), signature));
        let resp_str = self.base.send_request_multipart(&format!("{MEDIAFIRE_API_URL}upload/poll_upload.php"), &args, None, HttpMethod::POST)?;
        let resp = Self::response(resp_str.as_deref())?;
        self.ensure_success(&resp)?;
        let doupload = get(&resp, "doupload").filter(|d| !d.is_null()).ok_or_else(null_ref)?;
        let int = |k: &str| get(doupload, k).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));
        let (Some(result), Some(status)) = (int("result"), int("status")) else {
            return Err(Self::io_error(strings::MediaFire_Invalid_response()));
        };
        if result != 0 || int("fileerror").is_some() {
            return Err(Self::could_not_upload(get_str(doupload, "description")));
        }
        if status == 99 {
            let quickkey = get_str(doupload, "quickkey").ok_or_else(|| Self::io_error(strings::MediaFire_Invalid_response()))?;
            let mut url = url_helpers::combine_url("http://www.mediafire.com/view", &quickkey);
            if self.use_long_link {
                url = url_helpers::combine_url(&url, &url_helpers::url_encode(&get_str(doupload, "filename").unwrap_or_default(), false));
            }
            return Ok(Some(url));
        }
        Ok(None)
    }
}

/// `MediaFire`'s `UploadPath` from settings (`NameParser.Parse(URL, path)`).
pub fn mediafire_upload_path(path: &str) -> String {
    name_parse(NameParserType::Url, path)
}

// ---------------------------------------------------------------------------
// ImageShack

/// `ImageShackOptions` (`UploadersConfig.ImageShackSettings`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[allow(non_snake_case)]
pub struct ImageShackOptions {
    #[serde(default)]
    pub Username: Option<String>,
    #[serde(default)]
    pub Password: Secret,
    #[serde(default)]
    pub IsPublic: bool,
    #[serde(default)]
    pub Auth_token: Option<String>,
    #[serde(default = "default_thumbnail_width")]
    pub ThumbnailWidth: i32,
    #[serde(default)]
    pub ThumbnailHeight: i32,
}

fn default_thumbnail_width() -> i32 {
    256
}

impl ImageShackOptions {
    pub fn from_value(v: &Value) -> Option<Self> {
        if v.is_null() { None } else { serde_json::from_value(v.clone()).ok() }
    }
}

const IMAGESHACK_URL_ACCESS_TOKEN: &str = "https://api.imageshack.com/v2/user/login";
const IMAGESHACK_URL_UPLOAD: &str = "https://api.imageshack.com/v2/images";

/// `ImageShackUploader`.
pub struct ImageShack {
    pub base: Uploader,
    pub config: ImageShackOptions,
    api_key: String,
    token_changed: bool,
}

uploader_base!(ImageShack, FileUploader);

impl WritesBackConfig for ImageShack {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        if self.token_changed
            && let Some(o) = config.ImageShackSettings.as_object_mut()
        {
            o.insert("Auth_token".into(), json::str_or_null(self.config.Auth_token.as_deref()));
        }
    }
}

impl ImageShack {
    pub fn new(developer_key: &str, config: ImageShackOptions) -> Self {
        Self { base: Uploader::new(), config, api_key: developer_key.to_owned(), token_changed: false }
    }

    /// `GetAccessTokenAsync`: logs in with the username and password and
    /// keeps the auth token (written back with [`WritesBackConfig`]).
    pub fn get_access_token(&mut self) -> UResult<bool> {
        let (Some(user), Some(password)) = (self.config.Username.clone().filter(|u| !u.is_empty()), self.config.Password.get().filter(|p| !p.is_empty()))
        else {
            return Ok(false);
        };
        let args = args![("user", user), ("password", password)];
        let response = self.base.send_request_multipart(IMAGESHACK_URL_ACCESS_TOKEN, &args, None, HttpMethod::POST)?;
        if !null_or_empty(response.as_deref())
            && let Some(r) = deserialize_object(response.as_deref())?
            && let Some(token) = get(&r, "result").and_then(|res| get_str(res, "auth_token")).filter(|t| !t.is_empty())
        {
            self.config.Auth_token = Some(token);
            self.token_changed = true;
            return Ok(true);
        }
        Ok(false)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![
            ("api_key", self.api_key.clone()),
            ("auth_token", self.config.Auth_token.clone().unwrap_or_default()),
            ("public", if self.config.IsPublic { "y" } else { "n" }),
        ];
        let mut result = self.base.send_request_file_simple(IMAGESHACK_URL_UPLOAD, data, file_name, "file", &args, None)?;
        if let Some(response) = result.response.clone().filter(|r| !r.is_empty()) {
            let v = json::parse(&response).map_err(UploadError::Exception)?;
            let success = match v.get("success") {
                Some(Value::Bool(b)) => *b,
                Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
                _ => return Err(null_arg("value")),
            };
            if success {
                if let Some(Value::Array(images)) = v.get("result").and_then(|r| get(r, "images"))
                    && let Some(image) = images.first()
                {
                    let i = |k: &str| get_i64(image, k);
                    result.url = Some(format!(
                        "https://imagizer.imageshack.com/a/img{}/{}/{}",
                        i("server"),
                        i("bucket"),
                        get_str(image, "filename").unwrap_or_default()
                    ));
                    result.thumbnail_url = Some(format!(
                        "https://imagizer.imageshack.us/v2/{}x{}q90/{}/{}",
                        self.config.ThumbnailWidth,
                        self.config.ThumbnailHeight,
                        i("server"),
                        get_str(image, "filename").unwrap_or_default()
                    ));
                }
            } else {
                let e = v.get("error").filter(|e| !e.is_null()).ok_or_else(null_ref)?;
                self.base.errors.add(format!("Error message: {}\r\nError code: {}", get_str(e, "error_message").unwrap_or_default(), get_i64(e, "error_code")));
            }
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plik_ttl_and_seafile_raw() {
        assert_eq!(plik_multiply_index(2, 0), 1440.0);
        assert_eq!(plik_multiply_index(2, 2), 1.0);
        assert_eq!(raw_share_url("https://sea.example.com/f/abc/?dl=0"), "https://sea.example.com/f/abc/?dl=0&raw=1");
        assert_eq!(raw_share_url("https://Sea.example.com:8000/f/abc/"), "https://sea.example.com/f/abc/?raw=1");
    }
}
