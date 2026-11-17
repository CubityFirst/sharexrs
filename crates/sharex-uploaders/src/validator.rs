//! `UploadersConfigValidator`: whether a destination has what it needs in
//! `UploadersConfig` (its service's `CheckConfig`), for every destination,
//! ported or not: the ported ones answer through [`factory`], the others
//! check their settings as ShareX's services do, so lists filtered by this
//! (the before upload window's) show what ShareX's show.

use serde_json::Value;

use crate::UploadersConfig;
use crate::enums::{AccountType, FileDestination, ImageDestination, TextDestination, URLSharingServices, UrlShortenerType};
use crate::factory::{self, Ported};
use crate::oauth::{OAuth2Info, OAuthInfo};

fn ne(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

fn not_blank(s: &str) -> bool {
    !s.trim().is_empty()
}

/// A JSON string property that's set and not empty.
fn json_ne(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_str).is_some_and(|s| !s.is_empty())
}

/// `list.IsValidIndex(index)` on a JSON array.
fn valid_index(list: &Value, index: i64) -> bool {
    list.as_array().is_some_and(|a| index >= 0 && (index as usize) < a.len())
}

/// `Validate<ImageDestination>`.
pub fn image(dest: &ImageDestination, config: &UploadersConfig) -> bool {
    if *dest == ImageDestination::FileUploader {
        return true;
    }
    if let Ported::Yes(ok) = factory::check_image(dest, config) {
        return ok;
    }
    match dest {
        ImageDestination::ImageShack => json_ne(&config.ImageShackSettings, "Auth_token"),
        ImageDestination::Flickr => OAuthInfo::check(config.FlickrOAuthInfo.as_ref()),
        ImageDestination::Photobucket => !config.PhotobucketAccountInfo.is_null() && OAuthInfo::check(config.PhotobucketOAuthInfo.as_ref()),
        _ => true,
    }
}

/// `Validate<TextDestination>`.
pub fn text(dest: &TextDestination, config: &UploadersConfig) -> bool {
    if *dest == TextDestination::FileUploader {
        return true;
    }
    if let Ported::Yes(ok) = factory::check_text(dest, config) {
        return ok;
    }
    match dest {
        TextDestination::Gist => OAuth2Info::check(config.GistOAuth2Info.as_ref()),
        // PrivateBin needs nothing.
        _ => true,
    }
}

/// `Validate<FileDestination>`.
pub fn file(dest: &FileDestination, config: &UploadersConfig) -> bool {
    if let Ported::Yes(ok) = factory::check_file(dest, config) {
        return ok;
    }
    let c = config;
    match dest {
        FileDestination::Dropbox => OAuth2Info::check(c.DropboxOAuth2Info.as_ref()),
        FileDestination::FTP => valid_index(&c.FTPAccountList, c.FTPSelectedFile as i64),
        FileDestination::OneDrive => OAuth2Info::check(c.OneDriveV2OAuth2Info.as_ref()),
        FileDestination::GoogleDrive => OAuth2Info::check(c.GoogleDriveOAuth2Info.as_ref()),
        FileDestination::Box => OAuth2Info::check(c.BoxOAuth2Info.as_ref()),
        FileDestination::Mega => not_blank(c.MegaSessionID.as_str()) && not_blank(c.MegaMasterKey.as_str()),
        FileDestination::GoogleCloudStorage => OAuth2Info::check(c.GoogleCloudStorageOAuth2Info.as_ref()) && ne(&c.GoogleCloudStorageBucket),
        FileDestination::AzureStorage => {
            ne(&c.AzureStorageAccountName) && !c.AzureStorageAccountAccessKey.is_empty() && ne(&c.AzureStorageContainer)
        }
        FileDestination::BackblazeB2 => c.B2ApplicationKeyId.as_deref().is_some_and(not_blank) && not_blank(c.B2ApplicationKey.as_str()),
        FileDestination::MediaFire => ne(&c.MediaFireUsername) && !c.MediaFirePassword.is_empty(),
        FileDestination::Pushbullet => pushbullet(c),
        FileDestination::SendSpace => {
            c.SendSpaceAccountType == AccountType::Anonymous || (ne(&c.SendSpaceUsername) && !c.SendSpacePassword.is_empty())
        }
        FileDestination::Localhostr => ne(&c.LocalhostrEmail) && !c.LocalhostrPassword.is_empty(),
        FileDestination::Seafile => ne(&c.SeafileAPIURL) && !c.SeafileAuthToken.is_empty() && ne(&c.SeafileRepoID),
        FileDestination::Plik => ne(&c.PlikSettings.URL) && !c.PlikSettings.APIKey.is_empty(),
        FileDestination::YouTube => OAuth2Info::check(c.YouTubeOAuth2Info.as_ref()),
        FileDestination::SharedFolder => valid_index(&c.LocalhostAccountList, c.LocalhostSelectedFiles as i64),
        FileDestination::Email => email(c),
        _ => true,
    }
}

/// `PushbulletSettings`: a key and a valid selected device.
fn pushbullet(c: &UploadersConfig) -> bool {
    let p = &c.PushbulletSettings;
    json_ne(p, "UserAPIKey")
        && p.get("DeviceList").is_some_and(|l| valid_index(l, p.get("SelectedDevice").and_then(Value::as_i64).unwrap_or(0)))
}

/// The SMTP server, port, sender and password.
fn email(c: &UploadersConfig) -> bool {
    ne(&c.EmailSmtpServer) && c.EmailSmtpPort > 0 && ne(&c.EmailFrom) && !c.EmailPassword.is_empty()
}

/// `Validate<UrlShortenerType>`.
pub fn url_shortener(dest: &UrlShortenerType, config: &UploadersConfig) -> bool {
    match factory::check_url_shortener(dest, config) {
        Ported::Yes(ok) => ok,
        Ported::No => true,
    }
}

/// `Validate<URLSharingServices>`.
pub fn url_sharing(dest: &URLSharingServices, config: &UploadersConfig) -> bool {
    if let Ported::Yes(ok) = factory::check_url_sharing(dest, config) {
        return ok;
    }
    match dest {
        URLSharingServices::Email => email(config),
        URLSharingServices::Pushbullet => pushbullet(config),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_configs_validate_as_sharex() {
        let c = UploadersConfig::default();
        // Anonymous Imgur, vgy.me; Chevereto needs a URL and key.
        assert!(image(&ImageDestination::Imgur, &c));
        assert!(image(&ImageDestination::Vgyme, &c));
        assert!(!image(&ImageDestination::Chevereto, &c));
        assert!(!image(&ImageDestination::Flickr, &c));
        assert!(!image(&ImageDestination::CustomImageUploader, &c));
        assert!(text(&TextDestination::Pastebin, &c));
        assert!(text(&TextDestination::PrivateBin, &c));
        assert!(!text(&TextDestination::Gist, &c));
        assert!(file(&FileDestination::Uguu, &c));
        assert!(file(&FileDestination::SendSpace, &c));
        assert!(!file(&FileDestination::Dropbox, &c));
        assert!(!file(&FileDestination::Email, &c));
        assert!(!file(&FileDestination::FTP, &c));
        assert!(url_shortener(&UrlShortenerType::ISGD, &c));
        assert!(!url_shortener(&UrlShortenerType::BITLY, &c));
        assert!(url_sharing(&URLSharingServices::Facebook, &c));
        assert!(!url_sharing(&URLSharingServices::Pushbullet, &c));
    }
}
