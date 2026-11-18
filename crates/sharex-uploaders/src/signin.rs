//! Signing in to destinations, without the UI: what the Destination
//! settings window's account cards do (`Presentation/DestinationSettings/
//! DestinationSettingsAccounts.cs`: `BasicOAuth2`, `LoopbackOAuth`,
//! `FlickrOAuth`, `PhotobucketOAuth`) and `UploaderOAuthClientFactory`.
//!
//! Every function reads and writes `UploadersConfig` as ShareX's buttons
//! do; the window only shows the status and opens URLs:
//!
//! | Card | Buttons → functions |
//! |---|---|
//! | Code paste (Imgur, Gist, Dropbox, OneDrive, Box, bit.ly) | Open authorization page → [`open_authorization`] (open the URL it gives), Complete authorization → [`complete_authorization`] with the pasted code, Refresh token → [`refresh_token`], Disconnect → [`disconnect`] |
//! | Loopback (Google Drive, Google Cloud Storage, YouTube) | Connect account → [`connect_loopback`] on a worker thread (opens the browser through the callback, waits for the redirect; cancel with the token), Disconnect → [`disconnect`] |
//! | OAuth 1.0a (Flickr, Photobucket) | Open authorization page → [`open_authorization_oauth1`], Complete authorization → [`complete_authorization_oauth1`] with the verification code, Disconnect → [`disconnect`] |
//!
//! The status line: [`is_connected`] and [`connected_as`].

use crate::api_keys;
use crate::destinations::images::Imgur;
use crate::destinations::oauth_services::{BoxUploader, Dropbox, Flickr, GitHubGist, GoogleCloudStorage, GoogleDrive, OneDrive, Photobucket, PhotobucketAccountInfo, YouTube};
use crate::destinations::shorteners::Bitly;
use crate::http::CancellationToken;
use crate::oauth::{OAuth2Info, OAuthInfo, OAuthUserInfo};
use crate::oauth2::{self, OAuth1Client, OAuth2Client, OAuth2Loopback};
use crate::uploader::{UResult, UploadError};
use crate::UploadersConfig;

/// A destination with an OAuth sign-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInService {
    Imgur,
    Gist,
    Dropbox,
    OneDrive,
    Box,
    Bitly,
    GoogleDrive,
    GoogleCloudStorage,
    YouTube,
    Flickr,
    Photobucket,
}

/// How a service signs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInFlow {
    /// Open the page, paste the code it shows.
    CodePaste,
    /// A local redirect catches the code.
    Loopback,
    /// OAuth 1.0a: open the page, paste the verification code.
    OAuth1,
}

impl SignInService {
    pub const ALL: [SignInService; 11] = [
        Self::Imgur,
        Self::Gist,
        Self::Dropbox,
        Self::OneDrive,
        Self::Box,
        Self::Bitly,
        Self::GoogleDrive,
        Self::GoogleCloudStorage,
        Self::YouTube,
        Self::Flickr,
        Self::Photobucket,
    ];

    /// The Destination settings page id (`DestinationSettingsAccounts.Build`).
    pub fn page_id(self) -> &'static str {
        match self {
            Self::Imgur => "imgur",
            Self::Gist => "gist",
            Self::Dropbox => "dropbox",
            Self::OneDrive => "onedrive",
            Self::Box => "box",
            Self::Bitly => "bitly",
            Self::GoogleDrive => "google-drive",
            Self::GoogleCloudStorage => "google-cloud-storage",
            Self::YouTube => "youtube",
            Self::Flickr => "flickr",
            Self::Photobucket => "photobucket",
        }
    }

    /// The service of a Destination settings page.
    pub fn from_page_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.page_id() == id)
    }

    pub fn flow(self) -> SignInFlow {
        match self {
            Self::GoogleDrive | Self::GoogleCloudStorage | Self::YouTube => SignInFlow::Loopback,
            Self::Flickr | Self::Photobucket => SignInFlow::OAuth1,
            _ => SignInFlow::CodePaste,
        }
    }

    /// `UploaderOAuthClientFactory.Create*`: the application's client (from
    /// `SHAREXRS_KEY_*` at build time).
    pub fn new_client_info(self) -> OAuth2Info {
        match self {
            Self::Imgur => OAuth2Info::new(api_keys::IMGUR_CLIENT_ID, api_keys::IMGUR_CLIENT_SECRET),
            Self::Gist => OAuth2Info::new(api_keys::GITHUB_ID, api_keys::GITHUB_SECRET),
            Self::Dropbox => OAuth2Info::new(api_keys::DROPBOX_CONSUMER_KEY, api_keys::DROPBOX_CONSUMER_SECRET),
            Self::OneDrive => OneDrive::new_client(api_keys::ONE_DRIVE_CLIENT_ID, api_keys::ONE_DRIVE_CLIENT_SECRET),
            Self::Box => OAuth2Info::new(api_keys::BOX_CLIENT_ID, api_keys::BOX_CLIENT_SECRET),
            Self::Bitly => OAuth2Info::new(api_keys::BITLY_CLIENT_ID, api_keys::BITLY_CLIENT_SECRET),
            Self::GoogleDrive | Self::GoogleCloudStorage | Self::YouTube => OAuth2Info::new(api_keys::GOOGLE_CLIENT_ID, api_keys::GOOGLE_CLIENT_SECRET),
            Self::Flickr | Self::Photobucket => OAuth2Info::default(),
        }
    }

    /// `UploaderOAuthClientFactory.CreateFlickr` / `CreatePhotobucket`.
    pub fn new_oauth1_info(self) -> OAuthInfo {
        let (key, secret) = match self {
            Self::Flickr => (api_keys::FLICKR_KEY, api_keys::FLICKR_SECRET),
            _ => (api_keys::PHOTOBUCKET_CONSUMER_KEY, api_keys::PHOTOBUCKET_CONSUMER_SECRET),
        };
        OAuthInfo { ConsumerKey: Some(key.into()), ConsumerSecret: Some(secret.into()), ..Default::default() }
    }

    /// The OAuth 2 client for this service (code paste and loopback).
    pub fn oauth2_client(self, info: OAuth2Info) -> Option<Box<dyn OAuth2Client>> {
        Some(match self {
            Self::Imgur => Box::new(Imgur::new(info)),
            Self::Gist => Box::new(GitHubGist::new(info)),
            Self::Dropbox => Box::new(Dropbox::new(info)),
            Self::OneDrive => Box::new(OneDrive::new(info)),
            Self::Box => Box::new(BoxUploader::new(info)),
            Self::Bitly => Box::new(Bitly::new(info)),
            Self::GoogleDrive => Box::new(GoogleDrive::new(info)),
            Self::GoogleCloudStorage => Box::new(GoogleCloudStorage::new(info)),
            Self::YouTube => Box::new(YouTube::new(info)),
            Self::Flickr | Self::Photobucket => return None,
        })
    }

    /// The loopback client (Google's services).
    pub fn loopback_client(self, info: OAuth2Info) -> Option<Box<dyn OAuth2Loopback>> {
        Some(match self {
            Self::GoogleDrive => Box::new(GoogleDrive::new(info)),
            Self::GoogleCloudStorage => Box::new(GoogleCloudStorage::new(info)),
            Self::YouTube => Box::new(YouTube::new(info)),
            _ => return None,
        })
    }

    /// The stored OAuth 2 account.
    pub fn oauth2_info(self, config: &UploadersConfig) -> Option<&OAuth2Info> {
        match self {
            Self::Imgur => config.ImgurOAuth2Info.as_ref(),
            Self::Gist => config.GistOAuth2Info.as_ref(),
            Self::Dropbox => config.DropboxOAuth2Info.as_ref(),
            Self::OneDrive => config.OneDriveV2OAuth2Info.as_ref(),
            Self::Box => config.BoxOAuth2Info.as_ref(),
            Self::Bitly => config.BitlyOAuth2Info.as_ref(),
            Self::GoogleDrive => config.GoogleDriveOAuth2Info.as_ref(),
            Self::GoogleCloudStorage => config.GoogleCloudStorageOAuth2Info.as_ref(),
            Self::YouTube => config.YouTubeOAuth2Info.as_ref(),
            Self::Flickr | Self::Photobucket => None,
        }
    }

    fn set_oauth2_info(self, config: &mut UploadersConfig, info: Option<OAuth2Info>) {
        match self {
            Self::Imgur => config.ImgurOAuth2Info = info,
            Self::Gist => config.GistOAuth2Info = info,
            Self::Dropbox => config.DropboxOAuth2Info = info,
            Self::OneDrive => config.OneDriveV2OAuth2Info = info,
            Self::Box => config.BoxOAuth2Info = info,
            Self::Bitly => config.BitlyOAuth2Info = info,
            Self::GoogleDrive => config.GoogleDriveOAuth2Info = info,
            Self::GoogleCloudStorage => config.GoogleCloudStorageOAuth2Info = info,
            Self::YouTube => config.YouTubeOAuth2Info = info,
            Self::Flickr | Self::Photobucket => {}
        }
    }

    fn user_info(self, config: &UploadersConfig) -> Option<&OAuthUserInfo> {
        match self {
            Self::GoogleDrive => config.GoogleDriveUserInfo.as_ref(),
            Self::GoogleCloudStorage => config.GoogleCloudStorageUserInfo.as_ref(),
            Self::YouTube => config.YouTubeUserInfo.as_ref(),
            _ => None,
        }
    }

    fn set_user_info(self, config: &mut UploadersConfig, user: Option<OAuthUserInfo>) {
        match self {
            Self::GoogleDrive => config.GoogleDriveUserInfo = user,
            Self::GoogleCloudStorage => config.GoogleCloudStorageUserInfo = user,
            Self::YouTube => config.YouTubeUserInfo = user,
            _ => {}
        }
    }
}

/// Whether the account is signed in (`OAuth2Info.CheckOAuth` /
/// `OAuthInfo.CheckOAuth`).
pub fn is_connected(service: SignInService, config: &UploadersConfig) -> bool {
    match service {
        SignInService::Flickr => OAuthInfo::check(config.FlickrOAuthInfo.as_ref()),
        SignInService::Photobucket => OAuthInfo::check(config.PhotobucketOAuthInfo.as_ref()),
        s => OAuth2Info::check(s.oauth2_info(config)),
    }
}

/// The Google account's name for "Connected as {0}" (loopback cards).
pub fn connected_as(service: SignInService, config: &UploadersConfig) -> Option<String> {
    service.user_info(config).and_then(|u| u.name.clone()).filter(|n| !n.trim().is_empty())
}

/// "Open authorization page" of a code-paste card: a new client is stored
/// (or cleared when there's no page) and the page to open is returned.
pub fn open_authorization(service: SignInService, config: &mut UploadersConfig) -> UResult<Option<String>> {
    let mut client = service.oauth2_client(service.new_client_info()).ok_or(UploadError::Exception("not an OAuth 2 service".into()))?;
    let url = client.authorization_url()?;
    service.set_oauth2_info(config, if url.is_empty() { None } else { Some(client.auth_info().clone()) });
    Ok((!url.is_empty()).then_some(url))
}

/// "Complete authorization": exchanges the pasted code; the token is saved
/// in the config.
pub fn complete_authorization(service: SignInService, config: &mut UploadersConfig, code: &str) -> UResult<bool> {
    let Some(info) = service.oauth2_info(config).cloned() else { return Ok(false) };
    if code.trim().is_empty() {
        return Ok(false);
    }
    let mut client = service.oauth2_client(info).ok_or(UploadError::Exception("not an OAuth 2 service".into()))?;
    let result = client.get_access_token(code)?;
    service.set_oauth2_info(config, Some(client.auth_info().clone()));
    Ok(result)
}

/// "Refresh token" (`IOAuth2.RefreshAccessTokenAsync`; `false` for services
/// that can't).
pub fn refresh_token(service: SignInService, config: &mut UploadersConfig) -> UResult<bool> {
    let Some(info) = service.oauth2_info(config).cloned() else { return Ok(false) };
    let mut client = service.oauth2_client(info).ok_or(UploadError::Exception("not an OAuth 2 service".into()))?;
    if !client.can_refresh() {
        return Ok(false);
    }
    let result = client.refresh_access_token()?;
    service.set_oauth2_info(config, Some(client.auth_info().clone()));
    Ok(result)
}

/// "Disconnect": forgets the account.
pub fn disconnect(service: SignInService, config: &mut UploadersConfig) {
    match service {
        SignInService::Flickr => config.FlickrOAuthInfo = None,
        SignInService::Photobucket => {
            config.PhotobucketOAuthInfo = None;
            config.PhotobucketAccountInfo = serde_json::Value::Null;
        }
        s => {
            s.set_oauth2_info(config, None);
            s.set_user_info(config, None);
        }
    }
}

/// "Connect account" of a loopback card (`OAuthListenerWindow`): opens the
/// page through `open_url`, waits for the redirect (stop with `cancel`),
/// then saves the token and the user. `true` when connected.
pub fn connect_loopback(service: SignInService, config: &mut UploadersConfig, open_url: &dyn Fn(&str), cancel: &CancellationToken) -> UResult<bool> {
    let mut client = service.loopback_client(service.new_client_info()).ok_or(UploadError::Exception("not a loopback service".into()))?;
    match oauth2::connect(client.as_mut(), open_url, cancel)? {
        Some(user) => {
            service.set_oauth2_info(config, Some(client.auth_info().clone()));
            service.set_user_info(config, user);
            Ok(true)
        }
        None => {
            service.set_oauth2_info(config, None);
            service.set_user_info(config, None);
            Ok(false)
        }
    }
}

/// "Open authorization page" of the Flickr and Photobucket cards: gets a
/// request token; the account is stored and the page returned.
pub fn open_authorization_oauth1(service: SignInService, config: &mut UploadersConfig) -> UResult<Option<String>> {
    let info = service.new_oauth1_info();
    let (url, info) = match service {
        SignInService::Flickr => {
            let mut f = Flickr::new(info, Default::default());
            (f.authorization_url()?, f.auth_info)
        }
        SignInService::Photobucket => {
            let mut p = Photobucket::new(info, PhotobucketAccountInfo::default());
            (p.authorization_url()?, p.auth_info)
        }
        _ => return Err(UploadError::Exception("not an OAuth 1 service".into())),
    };
    let url = url.filter(|u| !u.is_empty());
    if url.is_some() {
        match service {
            SignInService::Flickr => config.FlickrOAuthInfo = Some(info),
            _ => config.PhotobucketOAuthInfo = Some(info),
        }
    }
    Ok(url)
}

/// "Complete authorization" of the Flickr and Photobucket cards.
pub fn complete_authorization_oauth1(service: SignInService, config: &mut UploadersConfig, code: &str) -> UResult<bool> {
    match service {
        SignInService::Flickr => {
            let Some(info) = config.FlickrOAuthInfo.clone() else { return Ok(false) };
            let mut f = Flickr::new(info, Default::default());
            let r = f.get_access_token(code)?;
            config.FlickrOAuthInfo = Some(f.auth_info);
            Ok(r)
        }
        SignInService::Photobucket => {
            let Some(info) = config.PhotobucketOAuthInfo.clone() else { return Ok(false) };
            let mut p = Photobucket::new(info, PhotobucketAccountInfo::default());
            let r = p.get_access_token(code)?;
            config.PhotobucketOAuthInfo = Some(p.auth_info.clone());
            if r {
                config.PhotobucketAccountInfo = serde_json::Value::Null;
                p.account_info.write_to(&mut config.PhotobucketAccountInfo);
            }
            Ok(r)
        }
        _ => Err(UploadError::Exception("not an OAuth 1 service".into())),
    }
}

impl OAuth2Client for Imgur {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        Ok(Imgur::authorization_url(self))
    }

    fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        Imgur::get_access_token(self, code)
    }

    fn refresh_access_token(&mut self) -> UResult<bool> {
        Imgur::refresh_access_token(self)
    }

    fn check_authorization(&mut self) -> UResult<bool> {
        Imgur::check_authorization(self)
    }

    fn can_refresh(&self) -> bool {
        true
    }

    fn uploader(&mut self) -> &mut crate::uploader::Uploader {
        &mut self.base
    }
}

impl OAuth2Client for Bitly {
    fn auth_info(&self) -> &OAuth2Info {
        &self.auth_info
    }

    fn authorization_url(&mut self) -> UResult<String> {
        Ok(Bitly::authorization_url(self))
    }

    fn get_access_token(&mut self, code: &str) -> UResult<bool> {
        Bitly::get_access_token(self, code)
    }

    fn uploader(&mut self) -> &mut crate::uploader::Uploader {
        &mut self.base
    }
}

// ---------------------------------------------------------------------------
// The other account cards and the pickers (`DestinationSettingsAccounts`,
// `DestinationSettingsPageBuilder`, `UploaderConfigurationClient`): what
// their buttons do to `UploadersConfig`, without the window.

/// Paste.ee's "Get user key" page (`GetPasteEeAuthorizationURL`).
pub fn paste_ee_authorization_url() -> String {
    format!("https://paste.ee/account/api/authorize/{}", api_keys::PASTE_EE_APPLICATION_KEY)
}

/// ImageShack's "Log in" (`LoginImageShackAsync`): the auth token is saved
/// in `ImageShackSettings`. `false` is "Login failed".
pub fn login_image_shack(config: &mut UploadersConfig) -> UResult<bool> {
    use crate::destinations::services::{ImageShack, ImageShackOptions};
    use crate::uploader::WritesBackConfig;
    let Some(options) = ImageShackOptions::from_value(&config.ImageShackSettings) else {
        return Err(UploadError::Exception(crate::destinations::NULL_REFERENCE.into()));
    };
    let mut u = ImageShack::new(api_keys::IMAGE_SHACK_KEY, options);
    let ok = u.get_access_token()?;
    u.write_back(config);
    Ok(ok)
}

/// Pastebin's "Log in" (`LoginPastebinAsync`): the user key is saved (or
/// cleared) in `PastebinSettings`.
pub fn login_pastebin(config: &mut UploadersConfig) -> UResult<bool> {
    use crate::destinations::text::Pastebin;
    let mut u = Pastebin::new(api_keys::PASTEBIN_KEY, config.PastebinSettings.clone());
    let ok = u.login()?;
    config.PastebinSettings.UserKey = u.settings.UserKey.clone();
    Ok(ok)
}

/// MEGA's "Log in" with `MegaEmail` and `MegaPassword`: the session is saved
/// and the password field cleared. `MegaError::Api(-26)` asks for the
/// two-factor code: ask it (`DestinationSettings_Enter_current_two_factor_authentication_code`)
/// and call again with it.
pub fn mega_login(config: &mut UploadersConfig, two_factor_code: Option<&str>) -> Result<(), crate::destinations::mega::MegaError> {
    use crate::destinations::mega::Mega;
    let mut mega = Mega::new(config.MegaEmail.as_deref().unwrap_or(""), config.MegaPassword.as_str());
    let session = mega.login(two_factor_code)?;
    config.MegaPassword = crate::dpapi::Secret::new("");
    config.MegaSessionID = crate::dpapi::Secret::new(session.session_id);
    config.MegaMasterKey = crate::dpapi::Secret::new(session.master_key);
    Ok(())
}

/// MEGA's "Disconnect" (also when the email or password changes): the
/// session and master key cleared, the folder back to the root.
pub fn mega_disconnect(config: &mut UploadersConfig) {
    config.MegaSessionID = crate::dpapi::Secret::new("");
    config.MegaMasterKey = crate::dpapi::Secret::new("");
    config.MegaSelectedFolder = serde_json::to_value(crate::destinations::mega::MegaFolderInfo::root()).expect("folder");
}

/// The MEGA folder picker's children of `parent` (`None`: the Cloud Drive
/// root), with the saved session. Not connected: `Err` with
/// `DestinationSettings_Not_connected`.
pub fn mega_folders(
    config: &UploadersConfig,
    parent: Option<&crate::destinations::mega::MegaFolderInfo>,
) -> Result<Vec<crate::destinations::mega::MegaFolderInfo>, crate::destinations::mega::MegaError> {
    use crate::destinations::mega::{Mega, MegaError, MegaFolderInfo, from_base64_url};
    if config.MegaSessionID.as_str().trim().is_empty() || config.MegaMasterKey.as_str().trim().is_empty() {
        return Err(MegaError::Other(sharex_strings::uploaders_lib::DestinationSettings_Not_connected().into()));
    }
    let key = from_base64_url(config.MegaMasterKey.as_str())?;
    let mut mega = Mega::with_session(config.MegaSessionID.as_str(), key);
    let root = MegaFolderInfo::root();
    mega.get_folders(Some(parent.unwrap_or(&root)))
}

/// Imgur's "Refresh albums": `None` when the account isn't connected
/// (`DestinationSettings_Connect_Imgur_account_first`), else the albums,
/// saved as `ImgurAlbumList` (a refreshed token is saved too).
pub fn refresh_imgur_albums(config: &mut UploadersConfig) -> UResult<Option<Vec<crate::destinations::images::ImgurAlbumData>>> {
    let Some(info) = config.ImgurOAuth2Info.clone().filter(|i| OAuth2Info::check(Some(i))) else { return Ok(None) };
    let mut imgur = Imgur::new(info);
    let albums = imgur.get_albums(10, 100);
    config.ImgurOAuth2Info = Some(imgur.auth_info.clone());
    let albums = albums?;
    config.ImgurAlbumList = serde_json::to_value(&albums).expect("albums");
    Ok(Some(albums))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_helpers_without_network() {
        let mut config = UploadersConfig::default();
        assert!(paste_ee_authorization_url().starts_with("https://paste.ee/account/api/authorize/"));
        assert!(refresh_imgur_albums(&mut config).unwrap().is_none());
        assert!(mega_folders(&config, None).is_err());
        config.MegaSessionID = crate::dpapi::Secret::new("s");
        mega_disconnect(&mut config);
        assert!(config.MegaSessionID.is_empty());
        assert_eq!(config.MegaSelectedFolder, serde_json::json!({"ID": "", "Name": ""}));
    }

    #[test]
    fn pages_and_flows() {
        for s in SignInService::ALL {
            assert_eq!(SignInService::from_page_id(s.page_id()), Some(s));
        }
        assert_eq!(SignInService::YouTube.flow(), SignInFlow::Loopback);
        assert_eq!(SignInService::Flickr.flow(), SignInFlow::OAuth1);
        assert_eq!(SignInService::OneDrive.flow(), SignInFlow::CodePaste);
        assert!(SignInService::OneDrive.new_client_info().Proof.is_object(), "OneDrive signs in with PKCE");
        let mut config = UploadersConfig::default();
        assert!(!is_connected(SignInService::Dropbox, &config));
        disconnect(SignInService::Photobucket, &mut config);
        assert!(config.PhotobucketAccountInfo.is_null());
    }
}
