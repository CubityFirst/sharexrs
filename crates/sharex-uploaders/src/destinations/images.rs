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

impl Imgur {
    /// `GetAlbumsAsync(maxPage, perPage)` (ShareX: 10 pages of 100): the
    /// signed-in account's albums, page by page until a short one. Destination
    /// settings saves them as `ImgurAlbumList` ("Refresh albums").
    pub fn get_albums(&mut self, max_page: i32, per_page: i32) -> UResult<Vec<ImgurAlbumData>> {
        let mut albums = Vec::new();
        for page in 0..max_page {
            match self.get_albums_page(page, per_page)? {
                Some(p) if !p.is_empty() => {
                    let short = (p.len() as i64) < i64::from(per_page);
                    albums.extend(p);
                    if short {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(albums)
    }

    fn get_albums_page(&mut self, page: i32, per_page: i32) -> UResult<Option<Vec<ImgurAlbumData>>> {
        if !self.check_authorization()? {
            return Ok(None);
        }
        let args = args![("page", page.to_string()), ("perPage", per_page.to_string())];
        let headers = self.auth_headers();
        let response = self.base.send_request(HttpMethod::GET, "https://api.imgur.com/3/account/me/albums", &args, Some(&headers))?;
        self.list_response(response.as_deref(), ImgurAlbumData::from_jtoken)
    }

    /// `GetAlbumImagesAsync(albumID)`: an album's images.
    pub fn get_album_images(&mut self, album_id: &str) -> UResult<Option<Vec<ImgurImageData>>> {
        if !self.check_authorization()? {
            return Ok(None);
        }
        let headers = self.auth_headers();
        let response = self.base.send_request(HttpMethod::GET, &format!("https://api.imgur.com/3/album/{album_id}/images"), &[], Some(&headers))?;
        self.list_response(response.as_deref(), ImgurImageData::from_jtoken)
    }

    /// `DeserializeObject<ImgurResponse>`, then the `data` array as a list,
    /// or `HandleErrors`.
    fn list_response<T>(&mut self, response: Option<&str>, convert: fn(&Value) -> Result<T, String>) -> UResult<Option<Vec<T>>> {
        let Some(response) = deserialize_object(response)? else { return Ok(None) };
        let data = get(&response, "data").cloned().unwrap_or(Value::Null);
        if get_bool(&response, "success") && get_i64(&response, "status") == 200 {
            return match data {
                Value::Array(items) => items.iter().map(convert).collect::<Result<Vec<T>, String>>().map(Some).map_err(UploadError::Exception),
                Value::Null => Err(UploadError::Exception(NULL_REFERENCE.into())),
                Value::Object(_) => Err(UploadError::Exception("Unable to cast object of type 'Newtonsoft.Json.Linq.JObject' to type 'Newtonsoft.Json.Linq.JArray'.".into())),
                _ => Err(UploadError::Exception("Unable to cast object of type 'Newtonsoft.Json.Linq.JValue' to type 'Newtonsoft.Json.Linq.JArray'.".into())),
            };
        }
        if let Some((error, request)) = parse_error(&data)? {
            let status = get_i64(&response, "status");
            self.base.errors.add(
                strings::Imgur_Error_details().replace("{0}", &status.to_string()).replace("{1}", request.as_deref().unwrap_or("")).replace("{2}", &error),
            );
        }
        Ok(None)
    }
}

/// Newtonsoft's conversions of a property for `ToObject<T>()`.
mod newtonsoft {
    use serde_json::Value;

    use crate::json::{get, to_dotnet_string};

    fn kind(v: &Value) -> &'static str {
        match v {
            Value::Null => "Null",
            Value::Bool(_) => "Boolean",
            Value::Number(n) if n.is_f64() => "Float",
            Value::Number(_) => "Integer",
            Value::String(_) => "String",
            Value::Array(_) => "Array",
            Value::Object(_) => "Object",
        }
    }

    pub fn string(o: &Value, name: &str) -> Result<Option<String>, String> {
        match get(o, name) {
            None => Ok(None),
            Some(v) => to_dotnet_string(v),
        }
    }

    fn integer(v: &Value, type_name: &str) -> Result<Option<i64>, String> {
        match v {
            Value::Null => Ok(None),
            Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).map(Some).ok_or_else(|| format!("Error converting value to type '{type_name}'.")),
            Value::String(s) if s.is_empty() => Ok(None),
            Value::String(s) => s.trim().parse().map(Some).map_err(|_| format!("Could not convert string to integer: {s}.")),
            Value::Bool(b) => Ok(Some(i64::from(*b))),
            other => Err(format!("Unexpected character encountered while parsing value: {}.", kind(other))),
        }
    }

    pub fn int(o: &Value, name: &str) -> Result<i32, String> {
        match get(o, name) {
            None => Ok(0),
            Some(v) => match integer(v, "System.Int32")? {
                Some(n) => i32::try_from(n).map_err(|_| "Value was either too large or too small for an Int32.".to_owned()),
                None => Err(format!("Error converting value {{null}} to type 'System.Int32'. Path '{name}'.")),
            },
        }
    }

    pub fn long(o: &Value, name: &str) -> Result<i64, String> {
        match get(o, name) {
            None => Ok(0),
            Some(v) => integer(v, "System.Int64")?.ok_or_else(|| format!("Error converting value {{null}} to type 'System.Int64'. Path '{name}'.")),
        }
    }

    pub fn long_opt(o: &Value, name: &str) -> Result<Option<i64>, String> {
        match get(o, name) {
            None => Ok(None),
            Some(v) => integer(v, "System.Nullable`1[System.Int64]"),
        }
    }

    fn boolean(v: &Value) -> Result<Option<bool>, String> {
        match v {
            Value::Null => Ok(None),
            Value::Bool(b) => Ok(Some(*b)),
            Value::Number(n) => Ok(Some(n.as_f64().unwrap_or(0.0) != 0.0)),
            Value::String(s) if s.is_empty() => Ok(None),
            Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(format!("Could not convert string to boolean: {s}.")),
            },
            other => Err(format!("Unexpected character encountered while parsing value: {}.", kind(other))),
        }
    }

    pub fn bool(o: &Value, name: &str) -> Result<bool, String> {
        match get(o, name) {
            None => Ok(false),
            Some(v) => boolean(v)?.ok_or_else(|| format!("Error converting value {{null}} to type 'System.Boolean'. Path '{name}'.")),
        }
    }

    pub fn bool_opt(o: &Value, name: &str) -> Result<Option<bool>, String> {
        match get(o, name) {
            None => Ok(None),
            Some(v) => boolean(v),
        }
    }
}

/// `ImgurImageData` (an album's images, as `JsonConvert` writes them).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ImgurImageData {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub datetime: i32,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub animated: bool,
    pub width: i32,
    pub height: i32,
    pub size: i32,
    pub views: i32,
    pub bandwidth: i64,
    pub deletehash: Option<String>,
    pub name: Option<String>,
    pub section: Option<String>,
    pub link: Option<String>,
    pub gifv: Option<String>,
    pub mp4: Option<String>,
    pub webm: Option<String>,
    pub looping: bool,
    pub favorite: bool,
    pub nsfw: Option<bool>,
    pub vote: Option<String>,
    pub comment_preview: Option<String>,
}

impl ImgurImageData {
    /// `ToObject<ImgurImageData>()`.
    pub fn from_jtoken(v: &Value) -> Result<Self, String> {
        use newtonsoft::*;
        if !v.is_object() {
            return Err("Error converting value to type 'ShareX.UploadersLib.ImageUploaders.ImgurImageData'.".into());
        }
        Ok(Self {
            id: string(v, "id")?,
            title: string(v, "title")?,
            description: string(v, "description")?,
            datetime: int(v, "datetime")?,
            kind: string(v, "type")?,
            animated: bool(v, "animated")?,
            width: int(v, "width")?,
            height: int(v, "height")?,
            size: int(v, "size")?,
            views: int(v, "views")?,
            bandwidth: long(v, "bandwidth")?,
            deletehash: string(v, "deletehash")?,
            name: string(v, "name")?,
            section: string(v, "section")?,
            link: string(v, "link")?,
            gifv: string(v, "gifv")?,
            mp4: string(v, "mp4")?,
            webm: string(v, "webm")?,
            looping: bool(v, "looping")?,
            favorite: bool(v, "favorite")?,
            nsfw: bool_opt(v, "nsfw")?,
            vote: string(v, "vote")?,
            comment_preview: string(v, "comment_preview")?,
        })
    }
}

/// `ImgurAlbumData`: an album of the account (`ImgurAlbumList`,
/// `ImgurSelectedAlbum` in `UploadersConfig`).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ImgurAlbumData {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub datetime: i32,
    pub cover: Option<String>,
    pub cover_width: Option<String>,
    pub cover_height: Option<String>,
    pub account_url: Option<String>,
    pub account_id: Option<i64>,
    pub privacy: Option<String>,
    pub layout: Option<String>,
    pub views: i32,
    pub link: Option<String>,
    pub favorite: bool,
    pub nsfw: Option<bool>,
    pub section: Option<String>,
    pub order: i32,
    pub deletehash: Option<String>,
    pub images_count: i32,
    pub images: Option<Vec<ImgurImageData>>,
}

impl ImgurAlbumData {
    /// `ToObject<ImgurAlbumData>()`.
    pub fn from_jtoken(v: &Value) -> Result<Self, String> {
        use newtonsoft::*;
        if !v.is_object() {
            return Err("Error converting value to type 'ShareX.UploadersLib.ImageUploaders.ImgurAlbumData'.".into());
        }
        let images = match json::get(v, "images") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) => Some(a.iter().map(ImgurImageData::from_jtoken).collect::<Result<Vec<_>, String>>()?),
            Some(_) => return Err("Cannot deserialize the current JSON object into type 'ShareX.UploadersLib.ImageUploaders.ImgurImageData[]'.".into()),
        };
        Ok(Self {
            id: string(v, "id")?,
            title: string(v, "title")?,
            description: string(v, "description")?,
            datetime: int(v, "datetime")?,
            cover: string(v, "cover")?,
            cover_width: string(v, "cover_width")?,
            cover_height: string(v, "cover_height")?,
            account_url: string(v, "account_url")?,
            account_id: long_opt(v, "account_id")?,
            privacy: string(v, "privacy")?,
            layout: string(v, "layout")?,
            views: int(v, "views")?,
            link: string(v, "link")?,
            favorite: bool(v, "favorite")?,
            nsfw: bool_opt(v, "nsfw")?,
            section: string(v, "section")?,
            order: int(v, "order")?,
            deletehash: string(v, "deletehash")?,
            images_count: int(v, "images_count")?,
            images,
        })
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
