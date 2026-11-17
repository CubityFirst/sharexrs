//! Image destinations: `ImageUploaders/Imgur.cs`, `Chevereto.cs`,
//! `VgymeUploader.cs`.

use serde_json::Value;
use sharex_helpers::file_helpers;
use sharex_strings::uploaders_lib as strings;

use super::{NULL_REFERENCE, deserialize_object, null_or_empty, uploader_base};
use crate::args;
use crate::config::CheveretoUploader;
use crate::enums::{AccountType, HttpMethod, ImgurThumbnailType};
use crate::http::SharedStream;
use crate::json::{self, get, get_bool, get_i64, get_str};
use crate::oauth::{OAuth2Info, OAuth2Token};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

const IMGUR_TOKEN_URL: &str = "https://api.imgur.com/oauth2/token";

/// `Imgur`: anonymous uploads with the application's client ID, or to the
/// signed-in account (and an album), refreshing its token when needed.
pub struct Imgur {
    pub base: Uploader,
    pub upload_method: AccountType,
    pub auth_info: OAuth2Info,
    pub thumbnail_type: ImgurThumbnailType,
    pub upload_album_id: Option<String>,
    pub direct_link: bool,
    pub use_gifv: bool,
}

uploader_base!(Imgur, FileUploader);

impl WritesBackConfig for Imgur {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.ImgurOAuth2Info = Some(self.auth_info.clone());
    }
}

impl Imgur {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self {
            base: Uploader::new(),
            upload_method: AccountType::Anonymous,
            auth_info,
            thumbnail_type: ImgurThumbnailType::Medium_Thumbnail,
            upload_album_id: None,
            direct_link: true,
            use_gifv: true,
        }
    }

    /// `GetAuthorizationURLAsync`: the PIN sign-in page.
    pub fn authorization_url(&self) -> String {
        let args = args![("client_id", self.auth_info.Client_ID.clone().unwrap_or_default()), ("response_type", "pin")];
        url_helpers::create_query_string_url("https://api.imgur.com/oauth2/authorize", &args, false)
    }

    fn token_request(&mut self, args: Vec<(String, String)>) -> UResult<bool> {
        let response = self.base.send_request_multipart(IMGUR_TOKEN_URL, &args, None, HttpMethod::POST)?;
        if let Some(response) = response.filter(|r| !r.is_empty())
            && let Some(mut token) = OAuth2Token::from_response(&response)
            && !token.access_token.is_empty()
        {
            token.update_expire_date();
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    /// `GetAccessTokenAsync(pin)`.
    pub fn get_access_token(&mut self, pin: &str) -> UResult<bool> {
        let args = args![
            ("client_id", self.auth_info.Client_ID.clone().unwrap_or_default()),
            ("client_secret", self.auth_info.Client_Secret.clone().unwrap_or_default()),
            ("grant_type", "pin"),
            ("pin", pin),
        ];
        self.token_request(args)
    }

    /// `RefreshAccessTokenAsync`.
    pub fn refresh_access_token(&mut self) -> UResult<bool> {
        let refresh = self.auth_info.Token.as_ref().map(|t| t.refresh_token.as_str().to_owned()).unwrap_or_default();
        if OAuth2Info::check(Some(&self.auth_info)) && !refresh.is_empty() {
            let args = args![
                ("refresh_token", refresh),
                ("client_id", self.auth_info.Client_ID.clone().unwrap_or_default()),
                ("client_secret", self.auth_info.Client_Secret.clone().unwrap_or_default()),
                ("grant_type", "refresh_token"),
            ];
            return self.token_request(args);
        }
        Ok(false)
    }

    fn auth_headers(&self) -> Headers {
        Headers::new().with("Authorization", format!("Bearer {}", self.auth_info.access_token()))
    }

    /// `CheckAuthorizationAsync`.
    pub fn check_authorization(&mut self) -> UResult<bool> {
        if OAuth2Info::check(Some(&self.auth_info)) {
            let expired = self.auth_info.Token.as_ref().is_none_or(|t| t.is_expired());
            if expired && !self.refresh_access_token()? {
                self.base.errors.add(strings::UploaderErrors_Refresh_access_token_failed());
                return Ok(false);
            }
        } else {
            self.base.errors.add(strings::UploaderErrors_Service_login_is_required().replace("{0}", "Imgur"));
            return Ok(false);
        }
        Ok(true)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        self.internal_upload(data, file_name, true)
    }

    fn internal_upload(&mut self, data: &SharedStream, file_name: &str, refresh_token_on_error: bool) -> CoreResult {
        let mut args: Vec<(String, String)> = Vec::new();
        let headers = if self.upload_method == AccountType::User {
            if !self.check_authorization()? {
                return Ok(None);
            }
            if let Some(album) = self.upload_album_id.as_deref().filter(|a| !a.is_empty()) {
                args.push(("album".into(), album.into()));
            }
            self.auth_headers()
        } else {
            Headers::new().with("Authorization", format!("Client-ID {}", self.auth_info.Client_ID.as_deref().unwrap_or("")))
        };
        self.base.return_response_on_error = true;
        let form_name = if file_helpers::is_video_file(file_name) { "video" } else { "image" };
        let mut result = self.base.send_request_file_simple("https://api.imgur.com/3/upload", data, file_name, form_name, &args, Some(&headers))?;
        if null_or_empty(result.response.as_deref()) {
            return Ok(Some(result));
        }
        let Some(response) = deserialize_object(result.response.as_deref())? else { return Ok(Some(result)) };
        let success = get_bool(&response, "success");
        let status = get_i64(&response, "status");
        let data_value = get(&response, "data").cloned().unwrap_or(Value::Null);
        if success && status == 200 {
            // ((JObject)data).ToObject<ImgurImageData>()
            if !data_value.is_object() && !data_value.is_null() {
                return Err(UploadError::Exception("Unable to cast object to type 'Newtonsoft.Json.Linq.JObject'.".into()));
            }
            let link = get_str(&data_value, "link");
            if !data_value.is_null() && !null_or_empty(link.as_deref()) {
                let id = get_str(&data_value, "id").unwrap_or_default();
                let link = link.expect("checked");
                if self.direct_link {
                    match get_str(&data_value, "gifv").filter(|g| self.use_gifv && !g.is_empty()) {
                        Some(gifv) => result.url = Some(gifv),
                        // webm uploads return a link with a dot at the end
                        None => result.url = Some(link.trim_end_matches('.').to_owned()),
                    }
                } else {
                    result.url = Some(format!("https://imgur.com/{id}"));
                }
                let thumbnail = match self.thumbnail_type {
                    ImgurThumbnailType::Small_Square => "s",
                    ImgurThumbnailType::Big_Square => "b",
                    ImgurThumbnailType::Small_Thumbnail => "t",
                    ImgurThumbnailType::Medium_Thumbnail => "m",
                    ImgurThumbnailType::Large_Thumbnail => "l",
                    ImgurThumbnailType::Huge_Thumbnail => "h",
                    ImgurThumbnailType::Unknown(_) => "",
                };
                // Imgur thumbnails are always jpg
                result.thumbnail_url = Some(format!("https://i.imgur.com/{id}{thumbnail}.jpg"));
                result.deletion_url = Some(format!("https://imgur.com/delete/{}", get_str(&data_value, "deletehash").unwrap_or_default()));
            }
        } else if let Some((error, _request)) = parse_error(&data_value)? {
            if self.upload_method == AccountType::User
                && refresh_token_on_error
                && error.eq_ignore_ascii_case("The access token provided is invalid.")
                && self.refresh_access_token()?
            {
                return self.internal_upload(data, file_name, false);
            }
            self.base.errors.add_first(strings::Imgur_Upload_failed().replace("{0}", &status.to_string()).replace("{1}", &error));
        }
        Ok(Some(result))
    }
}

/// `ParseError`: the error text (a string, or an object's `message`) and
/// the request.
fn parse_error(data: &Value) -> UResult<Option<(String, Option<String>)>> {
    if data.is_null() {
        return Ok(None);
    }
    if !data.is_object() {
        return Err(UploadError::Exception("Unable to cast object to type 'Newtonsoft.Json.Linq.JObject'.".into()));
    }
    let request = get_str(data, "request");
    let error = match get(data, "error") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(_)) => get(data, "error").and_then(|e| get_str(e, "message")).unwrap_or_default(),
        Some(Value::Null) | None => return Err(UploadError::Exception(NULL_REFERENCE.into())),
        Some(_) => return Err(UploadError::Exception("Unable to cast object to type 'Newtonsoft.Json.Linq.JObject'.".into())),
    };
    Ok(Some((error, request)))
}

/// `Chevereto`.
pub struct Chevereto {
    pub base: Uploader,
    pub uploader: CheveretoUploader,
    pub direct_url: bool,
}

uploader_base!(Chevereto, FileUploader);
impl WritesBackConfig for Chevereto {}

impl Chevereto {
    pub fn new(uploader: CheveretoUploader) -> Self {
        Self { base: Uploader::new(), uploader, direct_url: true }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let args = args![("key", self.uploader.APIKey.as_str()), ("format", "json")];
        let url = url_helpers::fix_prefix(self.uploader.UploadURL.as_deref().unwrap_or(""));
        let mut result = self.base.send_request_file_simple(&url, data, file_name, "source", &args, None)?;
        if result.is_success()
            && let Some(response) = deserialize_object(result.response.as_deref())?
            && let Some(image) = get(&response, "Image").filter(|i| !i.is_null())
        {
            result.url = if self.direct_url { get_str(image, "URL") } else { get_str(image, "URL_Viewer") };
            if let Some(thumb) = get(image, "Thumb").filter(|t| !t.is_null()) {
                result.thumbnail_url = get_str(thumb, "URL");
            }
        }
        Ok(Some(result))
    }
}

/// `VgymeUploader`.
pub struct Vgyme {
    pub base: Uploader,
    pub user_key: String,
}

uploader_base!(Vgyme, FileUploader);
impl WritesBackConfig for Vgyme {}

impl Vgyme {
    pub fn new(user_key: &str) -> Self {
        Self { base: Uploader::new(), user_key: user_key.to_owned() }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let mut args = Vec::new();
        if !self.user_key.is_empty() {
            args.push(("userkey".to_owned(), self.user_key.clone()));
        }
        let mut result = self.base.send_request_file_simple("https://vgy.me/upload", data, file_name, "file", &args, None)?;
        if result.is_success()
            && let Some(response) = deserialize_object(result.response.as_deref())?
            && !get_bool(&response, "Error")
        {
            result.url = get_str(&response, "Image");
            result.deletion_url = get_str(&response, "Delete");
        }
        Ok(Some(result))
    }
}

/// Keeps `json` used for the doc links above.
#[allow(dead_code)]
fn _uses(_: &UploadResult) -> Option<String> {
    json::to_dotnet_string(&Value::Null).ok().flatten()
}
