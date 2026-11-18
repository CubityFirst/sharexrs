//! Destinations that sign in with OAuth: `TextUploaders/GitHubGist.cs`,
//! `FileUploaders/Dropbox.cs`, `OneDrive.cs`, `Box.cs`, `GoogleDrive.cs`,
//! `GoogleCloudStorage.cs`, `YouTube.cs` (OAuth 2), and
//! `ImageUploaders/FlickrUploader.cs`, `Photobucket.cs` (OAuth 1.0a).
//! Their sign-in halves implement [`crate::oauth2`]'s traits; the folder,
//! drive and album listings the Destination settings pickers need are here
//! too.

use std::sync::{Arc, LazyLock, RwLock};

use serde_json::Value;
use sharex_helpers::{NameParserType, file_helpers};
use sharex_strings::uploaders_lib as strings;

use super::cloud_storage::name_parse;
use super::shared_folder::file_name_without_extension;
use super::{NULL_REFERENCE, deserialize_object, uploader_base};
use crate::args;
use crate::enums::{BoxShareAccessLevel, HttpMethod, YouTubeVideoPrivacy};
use crate::http::{self, SharedStream};
use crate::json::{self, get, get_str};
use crate::oauth::{OAuth2Info, OAuthInfo, OAuthUserInfo};
use crate::oauth1;
use crate::oauth2::{self, GoogleOAuth2, OAuth1Client, OAuth2Client, OAuth2Loopback, OAuth2ProofKey, bearer_headers, parse_token};
use crate::result::UploadResult;
use crate::uploader::{CoreResult, Headers, UResult, UploadError, Uploader, WritesBackConfig};
use crate::url_helpers;

/// `Links.Callback`: where code-paste flows redirect (the page shows the code).
pub const LINKS_CALLBACK: &str = "https://getsharex.com/callback/";

fn null_ref() -> UploadError {
    UploadError::Exception(NULL_REFERENCE.into())
}

fn null_arg(param: &str) -> UploadError {
    UploadError::Exception(format!("Value cannot be null. (Parameter '{param}')"))
}

/// `Enumerable.First()` of a JSON object's values (a `Dictionary`).
fn first_value(v: Option<&Value>) -> UResult<&Value> {
    let map = v.filter(|v| !v.is_null()).ok_or_else(|| null_arg("source"))?;
    map.as_object().and_then(|m| m.values().next()).ok_or(UploadError::Exception("Sequence contains no elements".into()))
}

fn client_id(info: &OAuth2Info) -> String {
    info.Client_ID.clone().unwrap_or_default()
}

fn client_secret(info: &OAuth2Info) -> String {
    info.Client_Secret.clone().unwrap_or_default()
}

fn refresh_token_of(info: &OAuth2Info) -> String {
    info.Token.as_ref().map(|t| t.refresh_token.as_str().to_owned()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// GitHub Gist

/// `GitHubGist`.
pub struct GitHubGist {
    pub base: Uploader,
    pub auth_info: OAuth2Info,
    pub public_upload: bool,
    pub raw_url: bool,
    pub custom_url_api: String,
}

uploader_base!(GitHubGist, TextUploader);

impl WritesBackConfig for GitHubGist {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.GistOAuth2Info = Some(self.auth_info.clone());
    }
}

impl GitHubGist {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self { base: Uploader::new(), auth_info, public_upload: false, raw_url: false, custom_url_api: String::new() }
    }

    fn upload_text(&mut self, text: &str, file_name: &str) -> CoreResult {
        let mut ur = UploadResult::default();
        if !text.is_empty() && !file_name.is_empty() {
            let api = if self.custom_url_api.is_empty() { "https://api.github.com" } else { self.custom_url_api.as_str() };
            let url = url_helpers::combine_url(api, "gists");
            let mut files = serde_json::Map::new();
            files.insert(file_name.to_owned(), json::object([("content", Value::String(text.to_owned()))]));
            let upload = json::object([("description", Value::String(String::new())), ("public", Value::Bool(self.public_upload)), ("files", Value::Object(files))]);
            let token = self.auth_info.Token.as_ref().ok_or_else(null_ref)?.access_token.as_str().to_owned();
            let headers = Headers::new().with("Authorization", format!("token {token}"));
            let response = self.base.send_request_text(HttpMethod::POST, &url, &json::to_string(&upload), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
            let gist = deserialize_object(response.as_deref())?;
            if response.is_some() {
                let gist = gist.ok_or_else(null_ref)?;
                ur.url = if self.raw_url { get_str(first_value(get(&gist, "files"))?, "raw_url") } else { get_str(&gist, "html_url") };
            }
        }
        Ok(Some(ur))
    }
}

impl OAuth2Client for GitHubGist {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        let args = args![("client_id", client_id(&self.auth_info)), ("redirect_uri", LINKS_CALLBACK), ("scope", "gist")];
        Ok(url_helpers::create_query_string_url("https://github.com/login/oauth/authorize", &args, false))
    }

    fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        let args = args![("client_id", client_id(&self.auth_info)), ("client_secret", client_secret(&self.auth_info)), ("code", code)];
        let headers = Headers::new().with("Accept", http::CONTENT_TYPE_JSON);
        let response = self.base.send_request_multipart("https://github.com/login/oauth/access_token", &args, Some(&headers), HttpMethod::POST)?;
        if let Some(token) = parse_token(response.as_deref()) {
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// Dropbox

const DROPBOX_OAUTH2_AUTHORIZE: &str = "https://www.dropbox.com/oauth2/authorize";
const DROPBOX_OAUTH2_TOKEN: &str = "https://api.dropboxapi.com/oauth2/token";
const DROPBOX_GET_CURRENT_ACCOUNT: &str = "https://api.dropboxapi.com/2/users/get_current_account";
const DROPBOX_UPLOAD: &str = "https://content.dropboxapi.com/2/files/upload";
const DROPBOX_GET_METADATA: &str = "https://api.dropboxapi.com/2/files/get_metadata";
const DROPBOX_CREATE_SHARED_LINK: &str = "https://api.dropboxapi.com/2/sharing/create_shared_link_with_settings";
const DROPBOX_LIST_SHARED_LINKS: &str = "https://api.dropboxapi.com/2/sharing/list_shared_links";
const DROPBOX_COPY: &str = "https://api.dropboxapi.com/2/files/copy";
const DROPBOX_CREATE_FOLDER: &str = "https://api.dropboxapi.com/2/files/create_folder";
const DROPBOX_DELETE: &str = "https://api.dropboxapi.com/2/files/delete";
const DROPBOX_MOVE: &str = "https://api.dropboxapi.com/2/files/move";
const DROPBOX_SHARE_DIRECT: &str = "https://dl.dropboxusercontent.com/scl/fi/";

/// `Dropbox`.
pub struct Dropbox {
    pub base: Uploader,
    pub auth_info: OAuth2Info,
    pub upload_path: String,
    pub auto_create_shareable_link: bool,
    pub use_direct_link: bool,
}

uploader_base!(Dropbox, FileUploader);

impl WritesBackConfig for Dropbox {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.DropboxOAuth2Info = Some(self.auth_info.clone());
    }
}

/// `Dropbox.VerifyPath(path, fileName)`.
pub fn dropbox_verify_path(path: &str, file_name: Option<&str>) -> String {
    if !path.is_empty() {
        let mut path = path.trim().replace('\\', "/").trim_matches('/').to_owned();
        if !path.starts_with('/') {
            path.insert(0, '/');
        }
        if let Some(f) = file_name.filter(|f| !f.is_empty()) {
            path = url_helpers::combine_url(&path, f);
        }
        return path;
    }
    file_name.filter(|f| !f.is_empty()).unwrap_or("").to_owned()
}

/// The `UploadPath` ShareX gives Dropbox: the setting verified, then the
/// name parser.
pub fn dropbox_upload_path(setting: &str) -> String {
    name_parse(NameParserType::Default, &dropbox_verify_path(setting, None))
}

impl Dropbox {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self { base: Uploader::new(), auth_info, upload_path: String::new(), auto_create_shareable_link: false, use_direct_link: false }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let path = self.upload_path.clone();
        self.upload_file(data, &path, file_name, self.auto_create_shareable_link, self.use_direct_link)
    }

    fn post_json(&mut self, url: &str, body: Value) -> UResult<Option<Value>> {
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request_text(HttpMethod::POST, url, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        match response.filter(|r| !r.is_empty()) {
            Some(r) => deserialize_object(Some(&r)),
            None => Ok(None),
        }
    }

    fn signed_in(&self) -> bool {
        OAuth2Info::check(Some(&self.auth_info))
    }

    /// `GetCurrentAccountAsync`: the account as Dropbox describes it.
    pub fn get_current_account(&mut self) -> UResult<Option<Value>> {
        if !self.signed_in() {
            return Ok(None);
        }
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request_text(HttpMethod::POST, DROPBOX_GET_CURRENT_ACCOUNT, "null", Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        match response.filter(|r| !r.is_empty()) {
            Some(r) => deserialize_object(Some(&r)),
            None => Ok(None),
        }
    }

    /// `UploadFileAsync`.
    pub fn upload_file(&mut self, data: &SharedStream, path: &str, file_name: &str, create_shareable_link: bool, use_direct_link: bool) -> CoreResult {
        let length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        if length > 150_000_000 {
            self.base.errors.add(strings::Dropbox_API_upload_limit());
            return Ok(None);
        }
        let arg = json::to_string(&json::object([
            ("path", Value::String(dropbox_verify_path(path, Some(file_name)))),
            ("mode", Value::String("overwrite".into())),
            ("autorename", Value::Bool(false)),
            ("mute", Value::Bool(true)),
        ]));
        let headers = bearer_headers(&self.auth_info);
        let query = args![("arg", arg)];
        let response = self.base.send_request_stream(HttpMethod::POST, DROPBOX_UPLOAD, data, Some(http::CONTENT_TYPE_OCTET_STREAM), &query, Some(&headers))?;
        let mut ur = UploadResult::with_response(response, None);
        if ur.response.as_deref().is_some_and(|r| !r.is_empty())
            && let Some(metadata) = deserialize_object(ur.response.as_deref())?
        {
            if create_shareable_link {
                self.base.allow_report_progress = false;
                ur.url = self.create_shareable_link(&get_str(&metadata, "path_display").unwrap_or_default(), use_direct_link)?;
            } else {
                ur.is_url_expected = false;
            }
        }
        Ok(Some(ur))
    }

    /// `GetMetadataAsync`.
    pub fn get_metadata(&mut self, path: &str) -> UResult<Option<Value>> {
        if !self.signed_in() {
            return Ok(None);
        }
        self.post_json(
            DROPBOX_GET_METADATA,
            json::object([
                ("path", Value::String(dropbox_verify_path(path, None))),
                ("include_media_info", Value::Bool(false)),
                ("include_deleted", Value::Bool(false)),
                ("include_has_explicit_shared_members", Value::Bool(false)),
            ]),
        )
    }

    /// `IsExistsAsync`.
    pub fn is_exists(&mut self, path: &str) -> UResult<bool> {
        Ok(self.get_metadata(path)?.is_some_and(|m| !get_str(&m, ".tag").unwrap_or_default().eq_ignore_ascii_case("deleted")))
    }

    /// `CreateShareableLinkAsync`.
    pub fn create_shareable_link(&mut self, path: &str, direct_link: bool) -> UResult<Option<String>> {
        if path.is_empty() || !self.signed_in() {
            return Ok(None);
        }
        let body = json::object([
            ("path", Value::String(dropbox_verify_path(path, None))),
            ("settings", json::object([("requested_visibility", Value::String("public".into()))])),
        ]);
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request_text(HttpMethod::POST, DROPBOX_CREATE_SHARED_LINK, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        let mut link: Option<Value> = None;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            link = deserialize_object(Some(&r))?;
        } else if self.base.is_error()
            && self.base.errors.errors.last().is_some_and(|e| e.text.contains("\"shared_link_already_exists\""))
            && let Some(result) = self.list_shared_links(path, true)?
            && let Some(Value::Array(links)) = get(&result, "links")
        {
            link = links.first().cloned();
        }
        Ok(link.and_then(|l| {
            let url = get_str(&l, "url").unwrap_or_default();
            if direct_link { direct_shareable_url(&url) } else { Some(url) }
        }))
    }

    /// `ListSharedLinksAsync`.
    pub fn list_shared_links(&mut self, path: &str, direct_only: bool) -> UResult<Option<Value>> {
        if !self.signed_in() {
            return Ok(None);
        }
        self.post_json(DROPBOX_LIST_SHARED_LINKS, json::object([("path", Value::String(dropbox_verify_path(path, None))), ("direct_only", Value::Bool(direct_only))]))
    }

    fn paths_request(&mut self, url: &str, from: &str, to: &str) -> UResult<Option<Value>> {
        if from.is_empty() || to.is_empty() || !self.signed_in() {
            return Ok(None);
        }
        self.post_json(
            url,
            json::object([("from_path", Value::String(dropbox_verify_path(from, None))), ("to_path", Value::String(dropbox_verify_path(to, None)))]),
        )
    }

    /// `CopyAsync`.
    pub fn copy(&mut self, from: &str, to: &str) -> UResult<Option<Value>> {
        self.paths_request(DROPBOX_COPY, from, to)
    }

    /// `MoveAsync`.
    pub fn move_item(&mut self, from: &str, to: &str) -> UResult<Option<Value>> {
        self.paths_request(DROPBOX_MOVE, from, to)
    }

    fn path_only(&mut self, url: &str, path: &str) -> UResult<Option<Value>> {
        if path.is_empty() || !self.signed_in() {
            return Ok(None);
        }
        self.post_json(url, json::object([("path", Value::String(dropbox_verify_path(path, None)))]))
    }

    /// `CreateFolderAsync`.
    pub fn create_folder(&mut self, path: &str) -> UResult<Option<Value>> {
        self.path_only(DROPBOX_CREATE_FOLDER, path)
    }

    /// `DeleteAsync`.
    pub fn delete(&mut self, path: &str) -> UResult<Option<Value>> {
        self.path_only(DROPBOX_DELETE, path)
    }
}

/// `GetDirectShareableURL`.
fn direct_shareable_url(url: &str) -> Option<String> {
    let find = "dropbox.com/scl/fi/";
    let index = url.find(find)?;
    let path = &url[index + find.len()..];
    (!path.is_empty()).then(|| url_helpers::combine_url(DROPBOX_SHARE_DIRECT, path))
}

impl OAuth2Client for Dropbox {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        let args = args![("response_type", "code"), ("client_id", client_id(&self.auth_info))];
        Ok(url_helpers::create_query_string_url(DROPBOX_OAUTH2_AUTHORIZE, &args, false))
    }

    fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        let args = args![
            ("client_id", client_id(&self.auth_info)),
            ("client_secret", client_secret(&self.auth_info)),
            ("grant_type", "authorization_code"),
            ("code", code),
        ];
        let response = self.base.send_request_multipart(DROPBOX_OAUTH2_TOKEN, &args, None, HttpMethod::POST)?;
        if let Some(token) = parse_token(response.as_deref()) {
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// OneDrive

const ONEDRIVE_AUTHORIZATION_ENDPOINT: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const ONEDRIVE_TOKEN_ENDPOINT: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const ONEDRIVE_MAX_SEGMENT_SIZE: u64 = 64 * 1024 * 1024;

/// `OneDriveLinkType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneDriveLinkType {
    Embed,
    Read,
    Edit,
}

/// `OneDrive` (Microsoft Graph).
pub struct OneDrive {
    pub base: Uploader,
    pub auth_info: OAuth2Info,
    pub folder_id: String,
    pub auto_create_shareable_link: bool,
    pub use_direct_link: bool,
    /// `MaxSegmentSize` (64 MiB); tests use smaller segments.
    pub max_segment_size: u64,
}

uploader_base!(OneDrive, FileUploader);

impl WritesBackConfig for OneDrive {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.OneDriveV2OAuth2Info = Some(self.auth_info.clone());
    }
}

/// `UriBuilder.Path = value`: escapes what isn't unreserved or reserved
/// (and `?`, `#`, `%`).
fn uri_builder_escape_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.replace('\\', "/").bytes() {
        if b.is_ascii_alphanumeric() || b"-._~:/[]@!$&'()*+,;=".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl OneDrive {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self {
            base: Uploader::new(),
            auth_info,
            folder_id: String::new(),
            auto_create_shareable_link: true,
            use_direct_link: false,
            max_segment_size: ONEDRIVE_MAX_SEGMENT_SIZE,
        }
    }

    /// `UploaderOAuthClientFactory.CreateOneDrive`: the client with a new
    /// SHA-256 proof key.
    pub fn new_client(client_id: &str, client_secret: &str) -> OAuth2Info {
        let mut info = OAuth2Info::new(client_id, client_secret);
        info.Proof = OAuth2ProofKey::new(oauth2::OAuth2ChallengeMethod::Sha256).to_value();
        info
    }

    fn check_authorization(&mut self) -> UResult<bool> {
        if OAuth2Info::check(Some(&self.auth_info)) {
            let expired = self.auth_info.Token.as_ref().is_none_or(|t| t.is_expired());
            if expired && !self.refresh_access_token()? {
                self.base.errors.add(strings::UploaderErrors_Refresh_access_token_failed());
                return Ok(false);
            }
            Ok(true)
        } else {
            self.base.errors.add(strings::UploaderErrors_Login_is_required());
            Ok(false)
        }
    }

    fn folder_url(folder_id: &str) -> String {
        if folder_id.is_empty() { "me/drive/root".into() } else { url_helpers::combine_url("me/drive/items", folder_id) }
    }

    fn create_session(&mut self, file_name: &str) -> UResult<Option<String>> {
        let mut item = serde_json::Map::new();
        item.insert("@microsoft.graph.conflictBehavior".into(), Value::String("replace".into()));
        let body = json::object([("item", Value::Object(item))]);
        let path = format!("/v1.0/{}:/{file_name}:/createUploadSession", Self::folder_url(&self.folder_id));
        let url = format!("https://graph.microsoft.com{}", uri_builder_escape_path(&path));
        self.base.allow_report_progress = false;
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request_text(HttpMethod::POST, &url, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        self.base.allow_report_progress = true;
        Ok(deserialize_object(response.as_deref())?.and_then(|s| get_str(&s, "uploadUrl")))
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if !self.check_authorization()? {
            return Ok(None);
        }
        let session_url = self.create_session(file_name)?.ok_or_else(|| null_arg("requestUri"))?;
        let length = http::stream_length(data).map_err(|e| UploadError::Exception(e.to_string()))?;
        let mut position = 0u64;
        let mut result;
        loop {
            result = self.base.send_request_file_range(&session_url, data, file_name, position, Some(self.max_segment_size), &[], None, HttpMethod::PUT)?;
            if result.is_success() {
                position += self.max_segment_size;
            } else {
                self.base.send_request(HttpMethod::DELETE, &session_url, &[], None)?;
                break;
            }
            if position >= length {
                break;
            }
        }
        if result.is_success() {
            let info = deserialize_object(result.response.as_deref())?.ok_or_else(null_ref)?;
            if self.auto_create_shareable_link {
                self.base.allow_report_progress = false;
                let link_type = if self.use_direct_link { OneDriveLinkType::Embed } else { OneDriveLinkType::Read };
                result.url = self.create_shareable_link(&get_str(&info, "id").unwrap_or_default(), link_type)?;
            } else {
                result.url = get_str(&info, "webUrl");
            }
        }
        Ok(Some(result))
    }

    /// `CreateShareableLinkAsync`.
    pub fn create_shareable_link(&mut self, id: &str, link_type: OneDriveLinkType) -> UResult<Option<String>> {
        let type_value = match link_type {
            OneDriveLinkType::Embed => "embed",
            OneDriveLinkType::Read => "view",
            OneDriveLinkType::Edit => "edit",
        };
        let body = json::object([("type", Value::String(type_value.into())), ("scope", Value::String("anonymous".into()))]);
        let headers = bearer_headers(&self.auth_info);
        let url = format!("https://graph.microsoft.com/v1.0/me/drive/items/{id}/createLink");
        let response = self.base.send_request_text(HttpMethod::POST, &url, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        let permission = deserialize_object(response.as_deref())?;
        Ok(permission.as_ref().and_then(|p| get(p, "link")).filter(|l| !l.is_null()).and_then(|l| get_str(l, "webUrl")))
    }

    /// `GetPathInfoAsync(id)`: the folder's sub folders (`value`: id, name),
    /// for the Destination settings folder picker.
    pub fn get_path_info(&mut self, id: &str) -> UResult<Option<Value>> {
        if !self.check_authorization()? {
            return Ok(None);
        }
        let args = args![("select", "id,name"), ("filter", "folder ne null")];
        let headers = bearer_headers(&self.auth_info);
        let url = format!("https://graph.microsoft.com/v1.0/{}/children", Self::folder_url(id));
        let response = self.base.send_request(HttpMethod::GET, &url, &args, Some(&headers))?;
        match response {
            Some(r) => deserialize_object(Some(&r)),
            None => Ok(None),
        }
    }
}

impl OAuth2Client for OneDrive {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        let mut args = args![
            ("client_id", client_id(&self.auth_info)),
            ("scope", "offline_access files.readwrite"),
            ("response_type", "code"),
            ("redirect_uri", LINKS_CALLBACK),
        ];
        if let Some(proof) = OAuth2ProofKey::from_value(&self.auth_info.Proof) {
            args.push(("code_challenge".into(), proof.code_challenge.clone()));
            args.push(("code_challenge_method".into(), proof.challenge_method().into()));
        }
        Ok(url_helpers::create_query_string_url(ONEDRIVE_AUTHORIZATION_ENDPOINT, &args, false))
    }

    fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        let mut args = args![
            ("client_id", client_id(&self.auth_info)),
            ("redirect_uri", LINKS_CALLBACK),
            ("client_secret", client_secret(&self.auth_info)),
            ("code", code),
            ("grant_type", "authorization_code"),
        ];
        if let Some(proof) = OAuth2ProofKey::from_value(&self.auth_info.Proof) {
            args.push(("code_verifier".into(), proof.code_verifier));
        }
        let response = self.base.send_request_url_encoded(HttpMethod::POST, ONEDRIVE_TOKEN_ENDPOINT, &args, None)?;
        if let Some(mut token) = parse_token(response.as_deref()) {
            token.update_expire_date();
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    fn refresh_access_token(&mut self) -> UResult<bool> {
        let refresh = refresh_token_of(&self.auth_info);
        if OAuth2Info::check(Some(&self.auth_info)) && !refresh.is_empty() {
            let args = args![
                ("client_id", client_id(&self.auth_info)),
                ("client_secret", client_secret(&self.auth_info)),
                ("refresh_token", refresh.clone()),
                ("grant_type", "refresh_token"),
            ];
            let response = self.base.send_request_url_encoded(HttpMethod::POST, ONEDRIVE_TOKEN_ENDPOINT, &args, None)?;
            if let Some(mut token) = parse_token(response.as_deref()) {
                token.update_expire_date();
                token.refresh_token = crate::dpapi::Secret::new(refresh);
                self.auth_info.Token = Some(token);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn can_refresh(&self) -> bool {
        true
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// Box

/// `Box` (box.com).
pub struct BoxUploader {
    pub base: Uploader,
    pub auth_info: OAuth2Info,
    pub folder_id: String,
    pub share: bool,
    pub share_access_level: BoxShareAccessLevel,
}

uploader_base!(BoxUploader, FileUploader);

impl WritesBackConfig for BoxUploader {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.BoxOAuth2Info = Some(self.auth_info.clone());
    }
}

const BOX_TOKEN_URL: &str = "https://www.box.com/api/oauth2/token";

impl BoxUploader {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self { base: Uploader::new(), auth_info, folder_id: "0".into(), share: true, share_access_level: BoxShareAccessLevel::Open }
    }

    /// `Box.RootFolder`.
    pub fn root_folder() -> Value {
        serde_json::json!({"type": "folder", "id": "0", "sequence_id": null, "etag": null, "name": "Root folder", "shared_link": null, "parent": null})
    }

    fn token_request(&mut self, args: crate::uploader::Args) -> UResult<bool> {
        let response = self.base.send_request_multipart(BOX_TOKEN_URL, &args, None, HttpMethod::POST)?;
        if let Some(mut token) = parse_token(response.as_deref()) {
            token.update_expire_date();
            self.auth_info.Token = Some(token);
            return Ok(true);
        }
        Ok(false)
    }

    fn check_authorization(&mut self) -> UResult<bool> {
        if OAuth2Info::check(Some(&self.auth_info)) {
            let expired = self.auth_info.Token.as_ref().is_none_or(|t| t.is_expired());
            if expired && !self.refresh_access_token()? {
                self.base.errors.add(strings::UploaderErrors_Refresh_access_token_failed());
                return Ok(false);
            }
            Ok(true)
        } else {
            self.base.errors.add(strings::UploaderErrors_Service_login_is_required().replace("{0}", "Box"));
            Ok(false)
        }
    }

    /// `GetFilesAsync(id)`: the folder's items (`entries`), for the folder picker.
    pub fn get_files(&mut self, id: &str) -> UResult<Option<Value>> {
        if !self.check_authorization()? {
            return Ok(None);
        }
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request(HttpMethod::GET, &format!("https://api.box.com/2.0/folders/{id}/items"), &[], Some(&headers))?;
        match response.filter(|r| !r.is_empty()) {
            Some(r) => deserialize_object(Some(&r)),
            None => Ok(None),
        }
    }

    /// `CreateSharedLinkAsync`.
    pub fn create_shared_link(&mut self, id: &str, access_level: &BoxShareAccessLevel) -> UResult<Option<String>> {
        let body = format!("{{\"shared_link\": {{\"access\": \"{}\"}}}}", access_level.name().to_lowercase());
        let headers = bearer_headers(&self.auth_info);
        let response = self.base.send_request_text(HttpMethod::PUT, &format!("https://api.box.com/2.0/files/{id}"), &body, None, &[], Some(&headers))?;
        if let Some(r) = response.filter(|r| !r.is_empty())
            && let Some(entry) = deserialize_object(Some(&r))?
            && let Some(link) = get(&entry, "shared_link").filter(|l| !l.is_null())
        {
            return Ok(get_str(link, "url"));
        }
        Ok(None)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if !self.check_authorization()? {
            return Ok(None);
        }
        if self.folder_id.is_empty() {
            self.folder_id = "0".into();
        }
        let args = args![("parent_id", self.folder_id.clone())];
        let headers = bearer_headers(&self.auth_info);
        let mut result = self.base.send_request_file_simple("https://upload.box.com/api/2.0/files/content", data, file_name, "filename", &args, Some(&headers))?;
        if result.is_success()
            && let Some(info) = deserialize_object(result.response.as_deref())?
            && let Some(Value::Array(entries)) = get(&info, "entries")
            && let Some(entry) = entries.first()
        {
            let id = get_str(entry, "id").unwrap_or_default();
            if self.share {
                self.base.allow_report_progress = false;
                let level = self.share_access_level.clone();
                result.url = self.create_shared_link(&id, &level)?;
            } else {
                let parent = get(entry, "parent").filter(|p| !p.is_null()).ok_or_else(null_ref)?;
                result.url = Some(format!("https://app.box.com/files/0/f/{}/1/f_{id}", get_str(parent, "id").unwrap_or_default()));
            }
        }
        Ok(Some(result))
    }
}

impl OAuth2Client for BoxUploader {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        let args = args![("response_type", "code"), ("client_id", client_id(&self.auth_info))];
        Ok(url_helpers::create_query_string_url("https://www.box.com/api/oauth2/authorize", &args, false))
    }

    fn get_access_token(&mut self, pin: &str) -> UResult<bool> {
        let args = args![
            ("grant_type", "authorization_code"),
            ("code", pin),
            ("client_id", client_id(&self.auth_info)),
            ("client_secret", client_secret(&self.auth_info)),
        ];
        self.token_request(args)
    }

    fn refresh_access_token(&mut self) -> UResult<bool> {
        let refresh = refresh_token_of(&self.auth_info);
        if OAuth2Info::check(Some(&self.auth_info)) && !refresh.is_empty() {
            let args = args![
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
                ("client_id", client_id(&self.auth_info)),
                ("client_secret", client_secret(&self.auth_info)),
            ];
            return self.token_request(args);
        }
        Ok(false)
    }

    fn can_refresh(&self) -> bool {
        true
    }

    fn check_authorization(&mut self) -> UResult<bool> {
        BoxUploader::check_authorization(self)
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// Google: Drive, Cloud Storage, YouTube

macro_rules! google_client {
    ($t:ty) => {
        impl OAuth2Client for $t {
            fn auth_info(&self) -> &OAuth2Info {
                &self.oauth2.auth_info
            }

            fn authorization_url(&mut self) -> UResult<String> {
                Ok(self.oauth2.authorization_url())
            }

            fn get_access_token(&mut self, code: &str) -> UResult<bool> {
                self.oauth2.get_access_token(&mut self.base, code)
            }

            fn refresh_access_token(&mut self) -> UResult<bool> {
                self.oauth2.refresh_access_token(&mut self.base)
            }

            fn can_refresh(&self) -> bool {
                true
            }

            fn uploader(&mut self) -> &mut Uploader {
                &mut self.base
            }
        }

        impl OAuth2Loopback for $t {
            fn set_redirect_uri(&mut self, uri: &str) {
                self.oauth2.redirect_uri = uri.to_owned();
            }

            fn set_state(&mut self, state: &str) {
                self.oauth2.state = state.to_owned();
            }

            fn get_user_info(&mut self) -> UResult<Option<OAuthUserInfo>> {
                self.oauth2.get_user_info(&mut self.base)
            }
        }
    };
}

/// `GoogleDrive`.
pub struct GoogleDrive {
    pub base: Uploader,
    pub oauth2: GoogleOAuth2,
    pub is_public: bool,
    pub direct_link: bool,
    pub folder_id: Option<String>,
    pub drive_id: Option<String>,
}

uploader_base!(GoogleDrive, FileUploader);
google_client!(GoogleDrive);

impl WritesBackConfig for GoogleDrive {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.GoogleDriveOAuth2Info = Some(self.oauth2.auth_info.clone());
    }
}

impl GoogleDrive {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self {
            base: Uploader::new(),
            oauth2: GoogleOAuth2::new(auth_info, "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/userinfo.profile"),
            is_public: false,
            direct_link: false,
            folder_id: None,
            drive_id: None,
        }
    }

    /// `GoogleDrive.MyDrive`.
    pub fn my_drive() -> Value {
        serde_json::json!({"id": "", "name": strings::GoogleDrive_My_drive()})
    }

    fn metadata(name: &str, parent_id: Option<&str>, drive_id: Option<&str>) -> String {
        let parent = parent_id.filter(|p| !p.is_empty()).or(drive_id);
        let v = match parent.filter(|p| !p.is_empty()) {
            Some(p) => json::object([
                ("name", Value::String(name.to_owned())),
                ("driveId", json::str_or_null(drive_id)),
                ("parents", Value::Array(vec![Value::String(p.to_owned())])),
            ]),
            None => json::object([("name", Value::String(name.to_owned()))]),
        };
        json::to_string(&v)
    }

    fn set_permissions(&mut self, file_id: &str, role: &str, kind: &str, allow_file_discovery: bool) -> UResult<()> {
        if !self.oauth2.check_authorization(&mut self.base)? {
            return Ok(());
        }
        let url = format!("https://www.googleapis.com/drive/v3/files/{file_id}/permissions?supportsAllDrives=true");
        let body = json::object([
            ("role", Value::String(role.into())),
            ("type", Value::String(kind.into())),
            ("allowFileDiscovery", Value::String(if allow_file_discovery { "True" } else { "False" }.into())),
        ]);
        let headers = self.oauth2.auth_headers();
        self.base.send_request_text(HttpMethod::POST, &url, &json::to_string(&body), Some(http::CONTENT_TYPE_JSON), &[], Some(&headers))?;
        Ok(())
    }

    fn list_pages(&mut self, url: &str, mut args: crate::uploader::Args, list_name: &str) -> UResult<Option<Vec<Value>>> {
        if !self.oauth2.check_authorization(&mut self.base)? {
            return Ok(None);
        }
        let mut items = Vec::new();
        let mut page_token = String::new();
        loop {
            match args.iter_mut().find(|(k, _)| k == "pageToken") {
                Some((_, v)) => *v = page_token.clone(),
                None => args.push(("pageToken".into(), page_token.clone())),
            }
            let headers = self.oauth2.auth_headers();
            let response = self.base.send_request(HttpMethod::GET, url, &args, Some(&headers))?;
            page_token.clear();
            if let Some(r) = response.filter(|r| !r.is_empty())
                && let Some(list) = deserialize_object(Some(&r))?
            {
                match get(&list, list_name) {
                    Some(Value::Array(a)) => items.extend(a.iter().cloned()),
                    _ => return Err(null_arg("collection")),
                }
                page_token = get_str(&list, "nextPageToken").unwrap_or_default();
            }
            if page_token.is_empty() {
                break;
            }
        }
        Ok(Some(items))
    }

    /// `GetFoldersAsync(driveID, trashed, writer)`: folders (id, name,
    /// description) for the folder picker.
    pub fn get_folders(&mut self, drive_id: &str, trashed: bool, writer: bool) -> UResult<Option<Vec<Value>>> {
        let mut query = "mimeType = 'application/vnd.google-apps.folder'".to_owned();
        if !trashed {
            query.push_str(" and trashed = false");
        }
        if writer && drive_id.is_empty() {
            query.push_str(" and 'me' in writers");
        }
        let mut args = args![("q", query), ("fields", "nextPageToken,files(id,name,description)")];
        if !drive_id.is_empty() {
            args.extend(args![("driveId", drive_id), ("corpora", "drive"), ("supportsAllDrives", "true"), ("includeItemsFromAllDrives", "true")]);
        }
        self.list_pages("https://www.googleapis.com/drive/v3/files", args, "files")
    }

    /// `GetDrivesAsync`: the shared drives (id, name) for the drive picker.
    pub fn get_drives(&mut self) -> UResult<Option<Vec<Value>>> {
        self.list_pages("https://www.googleapis.com/drive/v3/drives", Vec::new(), "drives")
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if !self.oauth2.check_authorization(&mut self.base)? {
            return Ok(None);
        }
        let metadata = Self::metadata(file_name, self.folder_id.as_deref(), self.drive_id.as_deref());
        let headers = self.oauth2.auth_headers();
        let mut result = self.base.send_request_file(
            "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,webViewLink,webContentLink&supportsAllDrives=true",
            data,
            file_name,
            "file",
            &[],
            Some(&headers),
            HttpMethod::POST,
            "multipart/related",
            Some(&metadata),
        )?;
        if result.response.as_deref().is_some_and(|r| !r.is_empty())
            && let Some(upload) = deserialize_object(result.response.as_deref())?
        {
            self.base.allow_report_progress = false;
            if self.is_public {
                self.set_permissions(&get_str(&upload, "id").unwrap_or_default(), "reader", "anyone", false)?;
            }
            if self.direct_link {
                let link = get_str(&upload, "webContentLink").ok_or_else(|| null_arg("uriString"))?;
                result.url = Some(remove_query_parameter(&link, "export"));
            } else {
                result.url = get_str(&upload, "webViewLink");
            }
        }
        Ok(Some(result))
    }
}

/// `Uri.GetLeftPart(Path)` + `?` + the query without `name`
/// (`HttpUtility.ParseQueryString` → `Remove` → `ToString`).
fn remove_query_parameter(url: &str, name: &str) -> String {
    let (left, query) = url.split('#').next().unwrap_or(url).split_once('?').unwrap_or((url, ""));
    let pairs = url_helpers::parse_query(query);
    let mut parts = Vec::new();
    for (k, values) in pairs.iter().filter(|(k, _)| !k.as_deref().is_some_and(|k| k.eq_ignore_ascii_case(name))) {
        let key = k.as_deref().map(|k| format!("{}=", url_helpers::http_utility_url_encode(k))).unwrap_or_default();
        if values.is_empty() {
            parts.push(key.clone());
        }
        for v in values {
            parts.push(format!("{key}{}", url_helpers::http_utility_url_encode(v)));
        }
    }
    format!("{left}?{}", parts.join("&"))
}

/// `GoogleCloudStorage`.
pub struct GoogleCloudStorage {
    pub base: Uploader,
    pub oauth2: GoogleOAuth2,
    pub bucket: String,
    pub domain: String,
    pub prefix: String,
    pub remove_extension_image: bool,
    pub remove_extension_text: bool,
    pub remove_extension_video: bool,
    pub set_public_acl: bool,
}

uploader_base!(GoogleCloudStorage, FileUploader);
google_client!(GoogleCloudStorage);

impl WritesBackConfig for GoogleCloudStorage {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.GoogleCloudStorageOAuth2Info = Some(self.oauth2.auth_info.clone());
    }
}

impl GoogleCloudStorage {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self {
            base: Uploader::new(),
            oauth2: GoogleOAuth2::new(auth_info, "https://www.googleapis.com/auth/devstorage.read_write https://www.googleapis.com/auth/userinfo.profile"),
            bucket: String::new(),
            domain: String::new(),
            prefix: String::new(),
            remove_extension_image: false,
            remove_extension_text: false,
            remove_extension_video: false,
            set_public_acl: true,
        }
    }

    fn upload_path(&self, file_name: &str) -> String {
        let upload_path = name_parse(NameParserType::FilePath, self.prefix.trim_matches('/'));
        let mut name = file_name.to_owned();
        if (self.remove_extension_image && file_helpers::is_image_file(file_name))
            || (self.remove_extension_text && file_helpers::is_text_file(file_name))
            || (self.remove_extension_video && file_helpers::is_video_file(file_name))
        {
            name = file_name_without_extension(file_name);
        }
        url_helpers::combine_url(&upload_path, &name)
    }

    /// `GenerateURL`.
    pub fn generate_url(&mut self, upload_path: &str) -> String {
        if self.bucket.is_empty() {
            return String::new();
        }
        if self.domain.is_empty() {
            self.domain = url_helpers::combine_url("storage.googleapis.com", &self.bucket);
        }
        let path = url_helpers::url_encode_with_options(upload_path, true);
        url_helpers::fix_prefix(&url_helpers::combine_url(&self.domain, &path))
    }

    /// `GetPreviewURL`.
    pub fn preview_url(&mut self) -> String {
        let path = self.upload_path("example.png");
        self.generate_url(&path)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if !self.oauth2.check_authorization(&mut self.base)? {
            return Ok(None);
        }
        let upload_path = self.upload_path(file_name);
        let early = self.generate_url(&upload_path);
        self.base.early_url_copy_requested(&early);
        let acl = if self.set_public_acl {
            Value::Array(vec![json::object([("entity", Value::String("allUsers".into())), ("role", Value::String("READER".into()))])])
        } else {
            Value::Null
        };
        let metadata = json::to_string(&json::object([("name", Value::String(upload_path)), ("acl", acl)]));
        let headers = self.oauth2.auth_headers();
        let url = format!("https://www.googleapis.com/upload/storage/v1/b/{}/o?uploadType=multipart&fields=name", self.bucket);
        let mut result = self.base.send_request_file(&url, data, file_name, "", &[], Some(&headers), HttpMethod::POST, "multipart/related", Some(&metadata))?;
        let response = deserialize_object(result.response.as_deref())?.ok_or_else(null_ref)?;
        let name = get_str(&response, "name").unwrap_or_default();
        result.url = Some(self.generate_url(&name));
        Ok(Some(result))
    }
}

/// What `YouTubeVideoOptionsWindow` returns: title, description, visibility.
pub type YouTubeOptions = (String, String, YouTubeVideoPrivacy);

/// Shows `YouTubeVideoOptionsWindow` (`ShowDialog`): `None` cancels.
pub type YouTubeOptionsUi = Arc<dyn Fn(&str, &str, &YouTubeVideoPrivacy) -> Option<YouTubeOptions> + Send + Sync>;

static YOUTUBE_UI: LazyLock<RwLock<Option<YouTubeOptionsUi>>> = LazyLock::new(|| RwLock::new(None));

/// Sets the window YouTube's "show dialog" option opens. Without one, the
/// upload goes ahead with the file name and the privacy setting.
pub fn set_youtube_options_ui(ui: Option<YouTubeOptionsUi>) {
    *YOUTUBE_UI.write().expect("youtube ui") = ui;
}

/// `YouTube`.
pub struct YouTube {
    pub base: Uploader,
    pub oauth2: GoogleOAuth2,
    pub privacy_type: YouTubeVideoPrivacy,
    pub use_shortened_link: bool,
    pub show_dialog: bool,
}

uploader_base!(YouTube, FileUploader);
google_client!(YouTube);

impl WritesBackConfig for YouTube {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.YouTubeOAuth2Info = Some(self.oauth2.auth_info.clone());
    }
}

impl YouTube {
    pub fn new(auth_info: OAuth2Info) -> Self {
        Self {
            base: Uploader::new(),
            oauth2: GoogleOAuth2::new(auth_info, "https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/userinfo.profile"),
            privacy_type: YouTubeVideoPrivacy::Public,
            use_shortened_link: false,
            show_dialog: false,
        }
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        if !self.oauth2.check_authorization(&mut self.base)? {
            return Ok(None);
        }
        let mut title = file_name_without_extension(file_name);
        let mut description = String::new();
        let mut visibility = self.privacy_type.clone();
        if self.show_dialog {
            let ui = YOUTUBE_UI.read().expect("youtube ui").clone();
            if let Some(ui) = ui {
                match ui(&title, &description, &visibility) {
                    Some((t, d, v)) => {
                        title = t;
                        description = d;
                        visibility = v;
                    }
                    None => return Ok(None),
                }
            }
        }
        // Newtonsoft writes this enum as its number (no StringEnumConverter).
        let privacy = visibility.value().unwrap_or(0);
        let metadata = json::to_string(&json::object([
            ("snippet", json::object([("title", Value::String(title)), ("description", Value::String(description)), ("tags", Value::Null)])),
            ("status", json::object([("privacyStatus", Value::from(privacy))])),
        ]));
        let headers = self.oauth2.auth_headers();
        let mut result = self.base.send_request_file(
            "https://www.googleapis.com/upload/youtube/v3/videos?part=id,snippet,status",
            data,
            file_name,
            "file",
            &[],
            Some(&headers),
            HttpMethod::POST,
            http::CONTENT_TYPE_MULTIPART_FORM_DATA,
            Some(&metadata),
        )?;
        if result.response.as_deref().is_some_and(|r| !r.is_empty())
            && let Some(video) = deserialize_object(result.response.as_deref())?
        {
            let id = get_str(&video, "id").unwrap_or_default();
            result.url = Some(if self.use_shortened_link { format!("https://youtu.be/{id}") } else { format!("https://www.youtube.com/watch?v={id}") });
            let status = get(&video, "status").filter(|s| !s.is_null()).ok_or_else(null_ref)?;
            match get_str(status, "uploadStatus").as_deref() {
                Some("failed") => self.base.errors.add(strings::YouTube_Upload_failed().replace("{0}", &get_str(status, "failureReason").unwrap_or_default())),
                Some("rejected") => {
                    self.base.errors.add(strings::YouTube_Upload_rejected().replace("{0}", &get_str(status, "rejectionReason").unwrap_or_default()))
                }
                _ => {}
            }
        }
        Ok(Some(result))
    }
}

// ---------------------------------------------------------------------------
// Flickr (OAuth 1.0a)

/// `FlickrSettings` (`UploadersConfig.FlickrSettings`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlickrSettings {
    pub direct_link: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub is_public: Option<String>,
    pub is_friend: Option<String>,
    pub is_family: Option<String>,
    pub safety_level: Option<String>,
    pub content_type: Option<String>,
    pub hidden: Option<String>,
}

impl FlickrSettings {
    pub fn from_value(v: &Value) -> Self {
        let s = |k: &str| get_str(v, k);
        Self {
            direct_link: get(v, "DirectLink").and_then(Value::as_bool).unwrap_or(true),
            title: s("Title"),
            description: s("Description"),
            tags: s("Tags"),
            is_public: s("IsPublic"),
            is_friend: s("IsFriend"),
            is_family: s("IsFamily"),
            safety_level: s("SafetyLevel"),
            content_type: s("ContentType"),
            hidden: s("Hidden"),
        }
    }
}

/// `FlickrUploader`.
pub struct Flickr {
    pub base: Uploader,
    pub auth_info: OAuthInfo,
    pub settings: FlickrSettings,
}

uploader_base!(Flickr, FileUploader);

impl WritesBackConfig for Flickr {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.FlickrOAuthInfo = Some(self.auth_info.clone());
    }
}

/// `XDocument.Parse(text).Element(root)` (`None` for another root).
fn xml_root<'a>(doc: &'a roxmltree::Document, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    let root = doc.root_element();
    (root.tag_name().name() == name).then_some(root)
}

fn xml_child<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> {
    node.children().find(|c| c.is_element() && c.tag_name().name() == name)
}

fn xml_value(node: roxmltree::Node) -> String {
    node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect()
}

impl Flickr {
    pub fn new(auth_info: OAuthInfo, settings: FlickrSettings) -> Self {
        Self { base: Uploader::new(), auth_info, settings }
    }

    /// `PhotosGetSizesAsync`: the photo's sizes as Flickr lists them.
    pub fn photos_get_sizes(&mut self, photo_id: &str) -> UResult<Option<Value>> {
        let args = args![("nojsoncallback", "1"), ("format", "json"), ("method", "flickr.photos.getSizes"), ("photo_id", photo_id)];
        let (query, _) = oauth1::generate_query("https://api.flickr.com/services/rest", &args, HttpMethod::POST, &self.auth_info)?;
        let response = self.base.send_request(HttpMethod::GET, &query, &[], None)?;
        deserialize_object(response.as_deref())
    }

    /// `ParseResponse(response, field)`: the field's value, or the error added.
    fn parse_response(&mut self, response: &str, field: &str) -> UResult<Option<String>> {
        if response.is_empty() {
            return Ok(None);
        }
        let doc = roxmltree::Document::parse(response).map_err(|e| UploadError::Exception(e.to_string()))?;
        if let Some(rsp) = xml_root(&doc, "rsp") {
            let status = rsp.attribute("status").filter(|s| !s.is_empty()).or_else(|| rsp.attribute("stat")).unwrap_or("");
            match status {
                "ok" => return Ok(xml_child(rsp, field).map(xml_value)),
                "fail" => {
                    let err = xml_child(rsp, "err").ok_or_else(null_ref)?;
                    self.base.errors.add(err.attribute("msg").unwrap_or(""));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let url = "https://up.flickr.com/services/upload/";
        let s = &self.settings;
        let mut args = crate::uploader::Args::new();
        for (k, v) in [
            ("title", &s.title),
            ("description", &s.description),
            ("tags", &s.tags),
            ("is_public", &s.is_public),
            ("is_friend", &s.is_friend),
            ("is_family", &s.is_family),
            ("safety_level", &s.safety_level),
            ("content_type", &s.content_type),
            ("hidden", &s.hidden),
        ] {
            if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
                args.push((k.to_owned(), v.to_owned()));
            }
        }
        let (_, parameters) = oauth1::generate_query(url, &args, HttpMethod::POST, &self.auth_info)?;
        let mut result = self.base.send_request_file_simple(url, data, file_name, "photo", &parameters, None)?;
        if result.is_success() {
            let response = result.response.clone().unwrap_or_default();
            if let Some(photo_id) = self.parse_response(&response, "photoid")? {
                let photos = self.photos_get_sizes(&photo_id)?;
                if let Some(Value::Array(sizes)) = photos.as_ref().and_then(|p| get(p, "sizes")).filter(|s| !s.is_null()).and_then(|s| get(s, "size"))
                    && let Some(photo) = sizes.last()
                {
                    result.url = get_str(photo, if self.settings.direct_link { "source" } else { "url" });
                }
            }
        }
        Ok(Some(result))
    }
}

impl OAuth1Client for Flickr {
    fn auth_info(&self) -> &OAuthInfo {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<Option<String>> {
        let args = args![("oauth_callback", LINKS_CALLBACK)];
        let url = oauth1::request_authorization_url(
            &mut self.base,
            "https://www.flickr.com/services/oauth/request_token",
            "https://www.flickr.com/services/oauth/authorize",
            &mut self.auth_info,
            &args,
            HttpMethod::GET,
        )?;
        Ok(Some(format!("{}&perms=write", url.unwrap_or_default())))
    }

    fn get_access_token(&mut self, verification_code: &str) -> UResult<bool> {
        self.auth_info.AuthVerifier.set(Some(verification_code.to_owned()));
        Ok(oauth1::request_access_token(&mut self.base, "https://www.flickr.com/services/oauth/access_token", &mut self.auth_info, HttpMethod::GET)?.is_some())
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// Photobucket (OAuth 1.0a)

/// `PhotobucketAccountInfo` (`UploadersConfig.PhotobucketAccountInfo`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PhotobucketAccountInfo {
    pub subdomain: Option<String>,
    pub album_id: Option<String>,
    pub album_list: Vec<String>,
    pub active_album_id: i32,
}

impl PhotobucketAccountInfo {
    pub fn from_value(v: &Value) -> Option<Self> {
        if !v.is_object() {
            return None;
        }
        Some(Self {
            subdomain: get_str(v, "Subdomain"),
            album_id: get_str(v, "AlbumID"),
            album_list: match get(v, "AlbumList") {
                Some(Value::Array(a)) => a.iter().map(|i| json::to_dotnet_string(i).ok().flatten().unwrap_or_default()).collect(),
                _ => Vec::new(),
            },
            active_album_id: json::get_i64(v, "ActiveAlbumID") as i32,
        })
    }

    /// Writes the account into `UploadersConfig.PhotobucketAccountInfo`,
    /// keeping properties this version doesn't know.
    pub fn write_to(&self, v: &mut Value) {
        if !v.is_object() {
            *v = serde_json::json!({"AlbumList": [], "ActiveAlbumID": 0});
        }
        let o = v.as_object_mut().expect("object");
        o.insert("Subdomain".into(), json::str_or_null(self.subdomain.as_deref()));
        o.insert("AlbumID".into(), json::str_or_null(self.album_id.as_deref()));
        o.insert("AlbumList".into(), Value::Array(self.album_list.iter().map(|a| Value::String(a.clone())).collect()));
        o.insert("ActiveAlbumID".into(), Value::from(self.active_album_id));
    }

    /// `ActiveAlbumPath`.
    pub fn active_album_path(&self) -> UResult<String> {
        usize::try_from(self.active_album_id).ok().and_then(|i| self.album_list.get(i)).cloned().ok_or(UploadError::Exception(
            "Index was out of range. Must be non-negative and less than the size of the collection. (Parameter 'index')".into(),
        ))
    }
}

const PHOTOBUCKET_URL_REQUEST_TOKEN: &str = "http://api.photobucket.com/login/request";
const PHOTOBUCKET_URL_AUTHORIZE: &str = "http://photobucket.com/apilogin/login";
const PHOTOBUCKET_URL_ACCESS_TOKEN: &str = "http://api.photobucket.com/login/access";

/// `Photobucket`.
pub struct Photobucket {
    pub base: Uploader,
    pub auth_info: OAuthInfo,
    pub account_info: PhotobucketAccountInfo,
}

uploader_base!(Photobucket, FileUploader);

impl WritesBackConfig for Photobucket {
    fn write_back(&self, config: &mut crate::UploadersConfig) {
        config.PhotobucketOAuthInfo = Some(self.auth_info.clone());
        self.account_info.write_to(&mut config.PhotobucketAccountInfo);
    }
}

impl Photobucket {
    pub fn new(auth_info: OAuthInfo, account_info: PhotobucketAccountInfo) -> Self {
        Self { base: Uploader::new(), auth_info, account_info }
    }

    fn fix_url(&self, url: &str) -> String {
        url.replace("api.photobucket.com", self.account_info.subdomain.as_deref().unwrap_or(""))
    }

    fn upload(&mut self, data: &SharedStream, file_name: &str) -> CoreResult {
        let album = self.account_info.active_album_path()?;
        self.upload_media(data, file_name, &album)
    }

    /// `UploadMediaAsync`.
    pub fn upload_media(&mut self, data: &SharedStream, file_name: &str, album_id: &str) -> CoreResult {
        let args = args![("id", album_id), ("type", "image")];
        let (query, _) = oauth1::generate_query("http://api.photobucket.com/album/!/upload", &args, HttpMethod::POST, &self.auth_info)?;
        let query = self.fix_url(&query);
        let mut result = self.base.send_request_file_simple(&query, data, file_name, "uploadfile", &[], None)?;
        if result.is_success() {
            let response = result.response.clone().unwrap_or_default();
            let doc = roxmltree::Document::parse(&response).map_err(|e| UploadError::Exception(e.to_string()))?;
            if let Some(content) = xml_root(&doc, "response").and_then(|r| xml_child(r, "content")) {
                result.url = Some(xml_child(content, "url").map(xml_value).unwrap_or_default());
                result.thumbnail_url = Some(xml_child(content, "thumb").map(xml_value).unwrap_or_default());
            }
        }
        Ok(Some(result))
    }

    /// `CreateAlbumAsync`.
    pub fn create_album(&mut self, album_id: &str, album_name: &str) -> UResult<bool> {
        let args = args![("id", album_id), ("name", album_name)];
        let (query, _) = oauth1::generate_query("http://api.photobucket.com/album/!", &args, HttpMethod::POST, &self.auth_info)?;
        let query = self.fix_url(&query);
        let response = self.base.send_request_multipart(&query, &args, None, HttpMethod::POST)?;
        if let Some(r) = response.filter(|r| !r.is_empty()) {
            let doc = roxmltree::Document::parse(&r).map_err(|e| UploadError::Exception(e.to_string()))?;
            if let Some(resp) = xml_root(&doc, "response") {
                return Ok(xml_child(resp, "status").map(xml_value).as_deref() == Some("OK"));
            }
        }
        Ok(false)
    }
}

impl OAuth1Client for Photobucket {
    fn auth_info(&self) -> &OAuthInfo {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<Option<String>> {
        oauth1::request_authorization_url(&mut self.base, PHOTOBUCKET_URL_REQUEST_TOKEN, PHOTOBUCKET_URL_AUTHORIZE, &mut self.auth_info, &[], HttpMethod::POST)
    }

    fn get_access_token(&mut self, verification_code: &str) -> UResult<bool> {
        self.auth_info.AuthVerifier.set(Some(verification_code.to_owned()));
        let values = oauth1::request_access_token(&mut self.base, PHOTOBUCKET_URL_ACCESS_TOKEN, &mut self.auth_info, HttpMethod::POST)?;
        if let Some(values) = values {
            self.account_info.subdomain = oauth1::response_value(&values, "subdomain");
            self.account_info.album_id = oauth1::response_value(&values, "username");
            return Ok(self.account_info.subdomain.as_deref().is_some_and(|s| !s.is_empty()));
        }
        Ok(false)
    }

    fn uploader(&mut self) -> &mut Uploader {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_links() {
        assert_eq!(dropbox_verify_path(" \\ShareX\\%y/ ", Some("a.png")), "/ShareX/%y/a.png");
        assert_eq!(dropbox_verify_path("", Some("a.png")), "a.png");
        assert_eq!(direct_shareable_url("https://www.dropbox.com/scl/fi/abc/a.png?rlkey=x&dl=0").as_deref(), Some("https://dl.dropboxusercontent.com/scl/fi/abc/a.png?rlkey=x&dl=0"));
        assert_eq!(remove_query_parameter("https://drive.google.com/uc?id=abc&export=download", "export"), "https://drive.google.com/uc?id=abc");
        assert_eq!(uri_builder_escape_path("/v1.0/me/drive/root:/my shot#1.png:/x"), "/v1.0/me/drive/root:/my%20shot%231.png:/x");
    }
}
