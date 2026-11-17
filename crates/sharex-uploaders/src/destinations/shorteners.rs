//! URL shorteners (`URLShorteners/*.cs`): bit.ly (with a saved token),
//! is.gd, v.gd, TinyURL, turl.ca, yourls, qr.net, vurl, 2.gp, Polr, Firebase
//! Dynamic Links, Kutt, Zero Width Shortener.

use serde_json::Value;
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, null_or_empty, uploader_base};
use crate::args;
use crate::config::KuttSettings;
use crate::enums::HttpMethod;
use crate::http;
use crate::json::{self, get_str, str_or_null};
use crate::oauth::{OAuth2Info, OAuth2Token};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

fn shortened(url: &str) -> UploadResult {
    UploadResult { url: Some(url.to_owned()), ..Default::default() }
}

fn null_ref<T>() -> UResult<T> {
    Err(UploadError::Exception(NULL_REFERENCE.into()))
}

/// `BitlyURLShortener`.
pub struct Bitly {
    pub base: Uploader,
    pub auth_info: OAuth2Info,
    pub domain: String,
}

uploader_base!(Bitly, UrlShortener);

impl WritesBackConfig for Bitly {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.BitlyOAuth2Info = Some(self.auth_info.clone());
    }
}

/// `Links.Callback`: the OAuth redirect page ShareX registers.
pub const OAUTH_CALLBACK: &str = "https://getsharex.com/callback/";

impl Bitly {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self { base: Uploader::new(), auth_info, domain: String::new() }
    }

    /// `GetAuthorizationURLAsync`.
    pub fn authorization_url(&self) -> String {
        let args = args![("client_id", self.auth_info.Client_ID.clone().unwrap_or_default()), ("redirect_uri", OAUTH_CALLBACK)];
        url_helpers::create_query_string_url("https://bitly.com/oauth/authorize", &args, false)
    }

    /// `GetAccessTokenAsync(code)`.
    pub fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        let args = args![
            ("client_id", self.auth_info.Client_ID.clone().unwrap_or_default()),
            ("client_secret", self.auth_info.Client_Secret.clone().unwrap_or_default()),
            ("code", code),
            ("redirect_uri", OAUTH_CALLBACK),
        ];
        let response = self.base.send_request_url_encoded(HttpMethod::POST, "https://api-ssl.bitly.com/oauth/access_token", &args, None)?;
        if let Some(response) = response.filter(|r| !r.is_empty())
            && let Some(token) = url_helpers::parse_query(&response)
                .into_iter()
                .find(|(k, _)| k.as_deref().is_some_and(|k| k.eq_ignore_ascii_case("access_token")))
                .map(|(_, v)| v.join(","))
                .filter(|t| !t.is_empty())
        {
            self.auth_info.Token = Some(OAuth2Token { access_token: crate::dpapi::Secret::new(token), ..Default::default() });
            return Ok(true);
        }
        Ok(false)
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        if !url.is_empty() {
            let mut body = json::object([("long_url", Value::String(url.into())), ("domain", Value::String("bit.ly".into()))]);
            if !self.domain.is_empty() {
                body["domain"] = Value::String(self.domain.clone());
            }
            let headers = Headers::new().with("Authorization", format!("Bearer {}", self.auth_info.access_token()));
            result.response = self.base.send_request_text(
                HttpMethod::POST,
                "https://api-ssl.bitly.com/v4/shorten",
                &json::to_string(&body),
                Some(http::CONTENT_TYPE_JSON),
                &[],
                Some(&headers),
            )?;
            if let Some(response) = deserialize_object(result.response.as_deref())?
                && let Some(link) = get_str(&response, "link").filter(|l| !l.is_empty())
            {
                result.shortened_url = Some(link);
            }
        }
        Ok(Some(result))
    }
}

/// `IsgdURLShortener` / `VgdURLShortener` (same API, other host).
pub struct Isgd {
    pub base: Uploader,
    pub api_url: &'static str,
}

uploader_base!(Isgd, UrlShortener);
impl WritesBackConfig for Isgd {}

impl Isgd {
    pub fn for_isgd() -> Self {
        Self { base: Uploader::new(), api_url: "http://is.gd/create.php" }
    }

    pub fn for_vgd() -> Self {
        Self { base: Uploader::new(), api_url: "http://v.gd/create.php" }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        if !url.is_empty() {
            let args = args![("format", "simple"), ("url", url)];
            result.response = self.base.send_request(HttpMethod::GET, self.api_url, &args, None)?;
            let Some(response) = result.response.clone() else { return null_ref() };
            if !(response.len() >= 6 && response[..6].eq_ignore_ascii_case("Error:")) {
                result.shortened_url = Some(response);
            }
        }
        Ok(Some(result))
    }
}

/// A shortener whose response is the short URL: `TinyURLShortener`, `NlcmURLShortener`.
pub struct PlainShortener {
    pub base: Uploader,
    pub api_url: &'static str,
}

uploader_base!(PlainShortener, UrlShortener);
impl WritesBackConfig for PlainShortener {}

impl PlainShortener {
    /// `TinyURLShortener`.
    pub fn tinyurl() -> Self {
        Self { base: Uploader::new(), api_url: "http://tinyurl.com/api-create.php" }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        if !url.is_empty() {
            let args = args![("url", url)];
            result.response = self.base.send_request(HttpMethod::GET, self.api_url, &args, None)?;
            result.shortened_url = result.response.clone();
        }
        Ok(Some(result))
    }
}

/// `TurlURLShortener`.
pub struct Turl {
    pub base: Uploader,
}

uploader_base!(Turl, UrlShortener);
impl WritesBackConfig for Turl {}

impl Turl {
    pub fn new() -> Self {
        Self { base: Uploader::new() }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        if !url.is_empty() {
            let args = args![("url", url)];
            result.response = self.base.send_request(HttpMethod::GET, "http://turl.ca/api.php", &args, None)?;
            if let Some(r) = result.response.clone().filter(|r| !r.is_empty()) {
                if let Some(rest) = r.strip_prefix("SUCCESS:") {
                    result.shortened_url = Some(format!("http://turl.ca/{rest}"));
                }
                if let Some(rest) = r.strip_prefix("ERROR:") {
                    self.base.errors.add(rest);
                }
            }
        }
        Ok(Some(result))
    }
}

impl Default for Turl {
    fn default() -> Self {
        Self::new()
    }
}

/// `QRnetURLShortener` and `TwoGPURLShortener` (same API).
pub struct QrNet {
    pub base: Uploader,
    pub api_url: &'static str,
}

uploader_base!(QrNet, UrlShortener);
impl WritesBackConfig for QrNet {}

impl QrNet {
    pub fn for_qrnet() -> Self {
        Self { base: Uploader::new(), api_url: "http://qr.net/api/short" }
    }

    pub fn for_twogp() -> Self {
        Self { base: Uploader::new(), api_url: "http://2.gp/api/short" }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        let args = args![("longurl", url)];
        let response = self.base.send_request(HttpMethod::GET, self.api_url, &args, None)?;
        if !null_or_empty(response.as_deref())
            && let Some(json) = deserialize_object(response.as_deref())?
        {
            result.shortened_url = get_str(&json, "url");
        }
        Ok(Some(result))
    }
}

/// `VURLShortener`.
pub struct Vurl {
    pub base: Uploader,
}

uploader_base!(Vurl, UrlShortener);
impl WritesBackConfig for Vurl {}

impl Vurl {
    pub fn new() -> Self {
        Self { base: Uploader::new() }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        let args = args![("url", url)];
        let response = self.base.send_request(HttpMethod::GET, "http://vurl.com/api.php", &args, None)?;
        if let Some(r) = response.filter(|r| !r.is_empty() && r != "Invalid URL") {
            result.shortened_url = Some(r);
        }
        Ok(Some(result))
    }
}

impl Default for Vurl {
    fn default() -> Self {
        Self::new()
    }
}

/// `YourlsURLShortener`.
pub struct Yourls {
    pub base: Uploader,
    pub api_url: String,
    pub signature: String,
    pub username: String,
    pub password: String,
}

uploader_base!(Yourls, UrlShortener);
impl WritesBackConfig for Yourls {}

impl Yourls {
    pub fn new(api_url: &str, signature: &str, username: &str, password: &str) -> Self {
        Self { base: Uploader::new(), api_url: api_url.into(), signature: signature.into(), username: username.into(), password: password.into() }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        if !url.is_empty() {
            let mut args = Vec::new();
            if !self.signature.is_empty() {
                args.push(("signature".to_owned(), self.signature.clone()));
            } else if !self.username.is_empty() && !self.password.is_empty() {
                args.push(("username".to_owned(), self.username.clone()));
                args.push(("password".to_owned(), self.password.clone()));
            } else {
                return Err(UploadError::Exception(strings::Yourls_Credentials_missing().into()));
            }
            args.extend(args![("action", "shorturl"), ("url", url), ("format", "simple")]);
            result.response = self.base.send_request_multipart(&self.api_url.clone(), &args, None, HttpMethod::POST)?;
            result.shortened_url = result.response.clone();
        }
        Ok(Some(result))
    }
}

/// `PolrURLShortener`.
pub struct Polr {
    pub base: Uploader,
    pub host: String,
    pub key: String,
    pub is_secret: bool,
    pub use_api_v1: bool,
}

uploader_base!(Polr, UrlShortener);
impl WritesBackConfig for Polr {}

impl Polr {
    pub fn new(host: &str, key: &str) -> Self {
        Self { base: Uploader::new(), host: host.into(), key: key.into(), is_secret: false, use_api_v1: false }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        self.host = url_helpers::fix_prefix(&self.host);
        let mut args = Vec::new();
        if !self.key.is_empty() {
            args.push((if self.use_api_v1 { "apikey" } else { "key" }.to_owned(), self.key.clone()));
        }
        if self.use_api_v1 {
            args.push(("action".to_owned(), "shorten".to_owned()));
        }
        args.push(("url".to_owned(), url.to_owned()));
        if self.is_secret && !self.use_api_v1 {
            args.push(("is_secret".to_owned(), "true".to_owned()));
        }
        let response = self.base.send_request(HttpMethod::GET, &self.host.clone(), &args, None)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            result.shortened_url = Some(r);
        }
        Ok(Some(result))
    }
}

/// `FirebaseDynamicLinksURLShortener`.
pub struct FirebaseDynamicLinks {
    pub base: Uploader,
    pub web_api_key: String,
    pub dynamic_link_domain: String,
    pub is_short: bool,
}

uploader_base!(FirebaseDynamicLinks, UrlShortener);
impl WritesBackConfig for FirebaseDynamicLinks {}

impl FirebaseDynamicLinks {
    pub fn new(web_api_key: &str, dynamic_link_domain: &str, is_short: bool) -> Self {
        Self { base: Uploader::new(), web_api_key: web_api_key.into(), dynamic_link_domain: dynamic_link_domain.into(), is_short }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        let suffix = if self.is_short { json::object([("option", Value::String("SHORT".into()))]) } else { Value::Null };
        let request = json::object([
            (
                "dynamicLinkInfo",
                json::object([
                    ("dynamicLinkDomain", Value::String(url_helpers::remove_prefixes(&self.dynamic_link_domain))),
                    ("link", Value::String(url.into())),
                ]),
            ),
            ("suffix", suffix),
        ]);
        let args = args![("key", self.web_api_key), ("fields", "shortLink")];
        result.response = self.base.send_request_text(
            HttpMethod::POST,
            "https://firebasedynamiclinks.googleapis.com/v1/shortLinks",
            &json::to_string(&request),
            Some(http::CONTENT_TYPE_JSON),
            &args,
            None,
        )?;
        if let Some(response) = deserialize_object(result.response.as_deref())? {
            result.shortened_url = get_str(&response, "shortLink");
        }
        Ok(Some(result))
    }
}

/// `KuttURLShortener`.
pub struct Kutt {
    pub base: Uploader,
    pub settings: KuttSettings,
}

uploader_base!(Kutt, UrlShortener);

impl WritesBackConfig for Kutt {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.KuttSettings = self.settings.clone();
    }
}

impl Kutt {
    pub fn new(settings: KuttSettings) -> Self {
        Self { base: Uploader::new(), settings }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        result.shortened_url = self.submit(url)?;
        Ok(Some(result))
    }

    /// `SubmitAsync`.
    pub fn submit(&mut self, url: &str) -> UResult<Option<String>> {
        let host = self.settings.Host.clone().unwrap_or_default();
        self.settings.Host = Some(if host.is_empty() { "https://kutt.it".to_owned() } else { url_helpers::fix_prefix(&host) });
        let request_url = url_helpers::combine_url(self.settings.Host.as_deref().unwrap_or(""), "/api/v2/links");
        let body = json::object([
            ("target", Value::String(url.into())),
            ("password", str_or_null(self.settings.Password.get())),
            ("customurl", Value::Null),
            ("reuse", Value::Bool(self.settings.Reuse)),
            ("domain", str_or_null(self.settings.Domain.as_deref())),
        ]);
        let headers = Headers::new().with("X-API-KEY", self.settings.APIKey.get().unwrap_or("").to_owned());
        let response = self.base.send_request_text(HttpMethod::POST, &request_url, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        if !null_or_empty(response.as_deref())
            && let Some(r) = deserialize_object(response.as_deref())?
        {
            return Ok(get_str(&r, "link"));
        }
        Ok(None)
    }
}

/// `ZeroWidthURLShortener`.
pub struct ZeroWidth {
    pub base: Uploader,
    pub request_url: String,
    pub token: String,
}

uploader_base!(ZeroWidth, UrlShortener);
impl WritesBackConfig for ZeroWidth {}

impl ZeroWidth {
    pub fn new(request_url: &str, token: &str) -> Self {
        Self { base: Uploader::new(), request_url: request_url.into(), token: token.into() }
    }

    fn shorten(&mut self, url: &str) -> CoreResult {
        let mut result = shortened(url);
        let body = json::object([("url", Value::String(url.into()))]);
        if self.request_url.is_empty() {
            self.request_url = "https://api.zws.im".into();
        }
        let headers = (!self.token.is_empty()).then(|| Headers::new().with("Authorization", format!("Bearer {}", self.token)));
        let response = self.base.send_request_text(
            HttpMethod::POST,
            &self.request_url.clone(),
            &json::to_string(&body),
            Some(http::CONTENT_TYPE_JSON),
            &[],
            headers.as_ref(),
        )?;
        if !null_or_empty(response.as_deref())
            && let Some(r) = deserialize_object(response.as_deref())?
        {
            match get_str(&r, "URL").filter(|u| !u.is_empty()) {
                Some(u) => result.shortened_url = Some(u),
                None => result.shortened_url = Some(url_helpers::combine_url("https://zws.im", &get_str(&r, "Short").unwrap_or_default())),
            }
        }
        Ok(Some(result))
    }
}
