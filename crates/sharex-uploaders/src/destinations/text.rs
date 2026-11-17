//! Text destinations: `TextUploaders/Pastebin.cs`, `Paste_ee.cs`,
//! `Hastebin.cs`, `Upaste.cs`, `OneTimeSecret.cs`, `Pastie.cs`, `Paste2.cs`,
//! `Slexy.cs`.

use serde_json::Value;
use sharex_helpers::file_helpers;
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, null_or_empty, uploader_base};
use crate::args;
use crate::config::PastebinSettings;
use crate::dpapi::Secret;
use crate::enums::{HttpMethod, PastebinExpiration, PastebinPrivacy, Privacy};
use crate::http;
use crate::json::{self, get, get_str};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig, create_authentication_header};
use crate::url_helpers;

/// `Pastebin`.
pub struct Pastebin {
    pub base: Uploader,
    api_key: String,
    pub settings: PastebinSettings,
}

uploader_base!(Pastebin, TextUploader);

impl WritesBackConfig for Pastebin {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.PastebinSettings = self.settings.clone();
    }
}

impl Pastebin {
    pub fn new(api_key: &str, settings: PastebinSettings) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned(), settings }
    }

    /// `LoginAsync`: gets a user key for the account.
    pub fn login(&mut self) -> UResult<bool> {
        let username = self.settings.Username.clone().unwrap_or_default();
        if !username.is_empty() && !self.settings.Password.is_empty() {
            let args = args![("api_dev_key", self.api_key), ("api_user_name", username), ("api_user_password", self.settings.Password.as_str())];
            let response = self.base.send_request_multipart("https://pastebin.com/api/api_login.php", &args, None, HttpMethod::POST)?;
            if let Some(r) = response.filter(|r| !r.is_empty() && !r.starts_with("Bad API request")) {
                self.settings.UserKey = Secret::new(r);
                return Ok(true);
            }
        }
        self.settings.UserKey = Secret::null();
        self.base.errors.add(strings::Pastebin_Login_failed());
        Ok(false)
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let s = &self.settings;
            let mut args = args![
                ("api_dev_key", self.api_key),
                ("api_option", "paste"),
                ("api_paste_code", text),
                ("api_paste_name", s.Title.clone().unwrap_or_default()),
                ("api_paste_format", s.TextFormat.clone().unwrap_or_default()),
                ("api_paste_private", privacy(&s.Exposure)),
                ("api_paste_expire_date", expiration(&s.Expiration)),
            ];
            if !s.UserKey.is_empty() {
                args.push(("api_user_key".into(), s.UserKey.as_str().into()));
            }
            ur.response = self.base.send_request_multipart("https://pastebin.com/api/api_post.php", &args, None, HttpMethod::POST)?;
            let response = ur.response.clone().unwrap_or_default();
            if url_helpers::is_valid_url(&response) {
                if self.settings.RawURL {
                    ur.url = Some(format!("https://pastebin.com/raw/{}", url_helpers::get_file_name(&response)));
                } else {
                    ur.url = Some(response);
                }
            } else {
                self.base.errors.add(response);
            }
        }
        Ok(Some(ur))
    }
}

fn privacy(p: &PastebinPrivacy) -> &'static str {
    match p {
        PastebinPrivacy::Public => "0",
        PastebinPrivacy::Private => "2",
        _ => "1",
    }
}

fn expiration(e: &PastebinExpiration) -> &'static str {
    match e {
        PastebinExpiration::M10 => "10M",
        PastebinExpiration::H1 => "1H",
        PastebinExpiration::D1 => "1D",
        PastebinExpiration::W1 => "1W",
        PastebinExpiration::W2 => "2W",
        PastebinExpiration::M1 => "1M",
        _ => "N",
    }
}

/// `Paste_ee`.
pub struct PasteEe {
    pub base: Uploader,
    pub api_key: String,
    pub encrypt_paste: bool,
}

uploader_base!(PasteEe, TextUploader);
impl WritesBackConfig for PasteEe {}

impl PasteEe {
    pub fn new(api_key: &str) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned(), encrypt_paste: false }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        if self.api_key.is_empty() {
            return Err(UploadError::Exception(strings::Common_API_key_is_missing().into()));
        }
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let body = json::object([
                ("encrypted", Value::Bool(self.encrypt_paste)),
                ("description", Value::String(String::new())),
                ("expiration", Value::String("never".into())),
                (
                    "sections",
                    Value::Array(vec![json::object([
                        ("name", Value::String(String::new())),
                        ("syntax", Value::String("autodetect".into())),
                        ("contents", Value::String(text.into())),
                    ])]),
                ),
            ]);
            let headers = Headers::new().with("X-Auth-Token", self.api_key.clone());
            ur.response = self.base.send_request_text(
                HttpMethod::POST,
                "https://api.paste.ee/v1/pastes",
                &json::to_string(&body),
                Some(http::CONTENT_TYPE_JSON),
                &[],
                Some(&headers),
            )?;
            if !null_or_empty(ur.response.as_deref()) {
                let response = deserialize_object(ur.response.as_deref())?.ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
                ur.url = get_str(&response, "link");
            }
        }
        Ok(Some(ur))
    }
}

/// `Hastebin`.
pub struct Hastebin {
    pub base: Uploader,
    pub custom_domain: String,
    pub syntax_highlighting: String,
    pub use_file_extension: bool,
}

uploader_base!(Hastebin, TextUploader);
impl WritesBackConfig for Hastebin {}

impl Hastebin {
    pub fn new() -> Self {
        Self { base: Uploader::new(), custom_domain: String::new(), syntax_highlighting: String::new(), use_file_extension: true }
    }

    fn upload_text(&mut self, text: &str, file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let domain = if self.custom_domain.is_empty() { "https://hastebin.com".to_owned() } else { self.custom_domain.clone() };
            ur.response = self.base.send_request_text(HttpMethod::POST, &url_helpers::combine_url(&domain, "documents"), text, None, &[], None)?;
            if !null_or_empty(ur.response.as_deref())
                && let Some(response) = deserialize_object(ur.response.as_deref())?
                && let Some(key) = get_str(&response, "Key").filter(|k| !k.is_empty())
            {
                let mut url = url_helpers::combine_url(&domain, &key);
                let mut syntax = self.syntax_highlighting.clone();
                if self.use_file_extension {
                    let ext = file_helpers::get_file_name_extension(file_name);
                    if !ext.is_empty() && !ext.eq_ignore_ascii_case("txt") {
                        syntax = ext.to_lowercase();
                    }
                }
                if !syntax.is_empty() {
                    url.push('.');
                    url.push_str(&syntax);
                }
                ur.url = Some(url);
            }
        }
        Ok(Some(ur))
    }
}

impl Default for Hastebin {
    fn default() -> Self {
        Self::new()
    }
}

/// `Upaste`.
pub struct Upaste {
    pub base: Uploader,
    pub user_key: String,
    pub is_public: bool,
}

uploader_base!(Upaste, TextUploader);
impl WritesBackConfig for Upaste {}

impl Upaste {
    pub fn new(user_key: &str) -> Self {
        Self { base: Uploader::new(), user_key: user_key.to_owned(), is_public: false }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let mut headers = Headers::new();
            if !self.user_key.is_empty() {
                headers.add("Authorization", format!("Bearer {}", self.user_key));
            }
            let args = args![("paste", text), ("privacy", if self.is_public { "0" } else { "1" }), ("expire", "0")];
            ur.response = self.base.send_request_multipart("https://upaste.me/api/v2/paste", &args, Some(&headers), HttpMethod::POST)?;
            if !null_or_empty(ur.response.as_deref())
                && let Some(response) = deserialize_object(ur.response.as_deref())?
            {
                let status = get_str(&response, "status").ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
                if status.eq_ignore_ascii_case("success") {
                    let paste = get(&response, "paste").filter(|p| !p.is_null()).ok_or(UploadError::Exception(NULL_REFERENCE.into()))?;
                    ur.url = get_str(paste, "link");
                } else {
                    self.base.errors.add(get_str(&response, "error").unwrap_or_default());
                }
            }
        }
        Ok(Some(ur))
    }
}

/// `OneTimeSecret`.
pub struct OneTimeSecret {
    pub base: Uploader,
    pub api_key: String,
    pub api_username: String,
}

uploader_base!(OneTimeSecret, TextUploader);
impl WritesBackConfig for OneTimeSecret {}

impl OneTimeSecret {
    pub fn new(api_username: &str, api_key: &str) -> Self {
        Self { base: Uploader::new(), api_key: api_key.to_owned(), api_username: api_username.to_owned() }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut result = UploadResult::default();
        if !text.is_empty() {
            let args = args![("secret", text)];
            let headers = (!self.api_username.is_empty() && !self.api_key.is_empty())
                .then(|| create_authentication_header(&self.api_username, &self.api_key));
            result.response = self.base.send_request_multipart("https://onetimesecret.com/api/v1/share", &args, headers.as_ref(), HttpMethod::POST)?;
            if !null_or_empty(result.response.as_deref())
                && let Some(response) = deserialize_object(result.response.as_deref())?
            {
                result.url = Some(url_helpers::combine_url("https://onetimesecret.com/secret/", &get_str(&response, "secret_key").unwrap_or_default()));
            }
        }
        Ok(Some(result))
    }
}

/// The URL a form post ended on (`LastResponseInfo.ResponseURL`), as
/// Pastie, Paste2 and Slexy report it.
fn last_response_url(base: &Uploader) -> UResult<Option<String>> {
    base.last_response_info.as_ref().map(|r| r.response_url.clone()).ok_or(UploadError::Exception(NULL_REFERENCE.into()))
}

/// `Pastie`.
pub struct Pastie {
    pub base: Uploader,
    pub is_public: bool,
}

uploader_base!(Pastie, TextUploader);
impl WritesBackConfig for Pastie {}

impl Pastie {
    pub fn new(is_public: bool) -> Self {
        Self { base: Uploader::new(), is_public }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let args = args![
                ("paste[body]", text),
                ("paste[restricted]", if self.is_public { "0" } else { "1" }),
                ("paste[authorization]", "burger"),
            ];
            self.base.send_request_url_encoded(HttpMethod::POST, "http://pastie.org/pastes", &args, None)?;
            ur.url = last_response_url(&self.base)?;
        }
        Ok(Some(ur))
    }
}

/// `Paste2`.
pub struct Paste2 {
    pub base: Uploader,
    pub text_format: String,
    pub description: String,
}

uploader_base!(Paste2, TextUploader);
impl WritesBackConfig for Paste2 {}

impl Paste2 {
    pub fn new(text_format: &str) -> Self {
        Self { base: Uploader::new(), text_format: text_format.to_owned(), description: String::new() }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let args = args![("code", text), ("lang", self.text_format), ("description", self.description), ("parent", "")];
            self.base.send_request_multipart("https://paste2.org/", &args, None, HttpMethod::POST)?;
            ur.url = last_response_url(&self.base)?;
        }
        Ok(Some(ur))
    }
}

/// `SlexySettings`.
#[derive(Debug, Clone)]
pub struct SlexySettings {
    pub text_format: String,
    pub author: String,
    pub visibility: Privacy,
    pub description: String,
    pub line_numbers: bool,
    pub expiration: String,
}

impl Default for SlexySettings {
    fn default() -> Self {
        Self {
            text_format: "text".into(),
            author: String::new(),
            visibility: Privacy::Private,
            description: String::new(),
            line_numbers: true,
            expiration: "2592000".into(),
        }
    }
}

/// `Slexy`.
pub struct Slexy {
    pub base: Uploader,
    pub settings: SlexySettings,
}

uploader_base!(Slexy, TextUploader);
impl WritesBackConfig for Slexy {}

impl Slexy {
    pub fn new(settings: SlexySettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn upload_text(&mut self, text: &str, _file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() {
            let s = &self.settings;
            let args = args![
                ("raw_paste", text),
                ("author", s.author),
                ("comment", ""),
                ("desc", s.description),
                ("expire", s.expiration),
                ("language", s.text_format),
                ("linenumbers", if s.line_numbers { "1" } else { "0" }),
                ("permissions", if s.visibility == Privacy::Private { "1" } else { "0" }),
                ("submit", "Submit Paste"),
                ("tabbing", "true"),
                ("tabtype", "real"),
            ];
            self.base.send_request_multipart("http://slexy.org/index.php/submit", &args, None, HttpMethod::POST)?;
            ur.url = last_response_url(&self.base)?;
        }
        Ok(Some(ur))
    }
}
