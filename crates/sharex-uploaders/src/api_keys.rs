//! `APIKeys/APIKeys.cs`: the services' application keys. ShareX's source
//! has them empty; official builds fill them in at build time. ShareXrs
//! takes them from `SHAREXRS_KEY_*` environment variables when it's built
//! (empty otherwise, as in a ShareX built from source).

/// `APIKeys.ImgurClientID` (`SHAREXRS_KEY_IMGUR_CLIENT_ID`).
pub const IMGUR_CLIENT_ID: &str = match option_env!("SHAREXRS_KEY_IMGUR_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.ImgurClientSecret` (`SHAREXRS_KEY_IMGUR_CLIENT_SECRET`).
pub const IMGUR_CLIENT_SECRET: &str = match option_env!("SHAREXRS_KEY_IMGUR_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.ImageShackKey` (`SHAREXRS_KEY_IMAGE_SHACK_KEY`).
pub const IMAGE_SHACK_KEY: &str = match option_env!("SHAREXRS_KEY_IMAGE_SHACK_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.FlickrKey` (`SHAREXRS_KEY_FLICKR_KEY`).
pub const FLICKR_KEY: &str = match option_env!("SHAREXRS_KEY_FLICKR_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.FlickrSecret` (`SHAREXRS_KEY_FLICKR_SECRET`).
pub const FLICKR_SECRET: &str = match option_env!("SHAREXRS_KEY_FLICKR_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.PhotobucketConsumerKey` (`SHAREXRS_KEY_PHOTOBUCKET_CONSUMER_KEY`).
pub const PHOTOBUCKET_CONSUMER_KEY: &str = match option_env!("SHAREXRS_KEY_PHOTOBUCKET_CONSUMER_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.PhotobucketConsumerSecret` (`SHAREXRS_KEY_PHOTOBUCKET_CONSUMER_SECRET`).
pub const PHOTOBUCKET_CONSUMER_SECRET: &str = match option_env!("SHAREXRS_KEY_PHOTOBUCKET_CONSUMER_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.PastebinKey` (`SHAREXRS_KEY_PASTEBIN_KEY`).
pub const PASTEBIN_KEY: &str = match option_env!("SHAREXRS_KEY_PASTEBIN_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.GitHubID` (`SHAREXRS_KEY_GITHUB_ID`).
pub const GITHUB_ID: &str = match option_env!("SHAREXRS_KEY_GITHUB_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.GitHubSecret` (`SHAREXRS_KEY_GITHUB_SECRET`).
pub const GITHUB_SECRET: &str = match option_env!("SHAREXRS_KEY_GITHUB_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.Paste_eeApplicationKey` (`SHAREXRS_KEY_PASTE_EE_APPLICATION_KEY`).
pub const PASTE_EE_APPLICATION_KEY: &str = match option_env!("SHAREXRS_KEY_PASTE_EE_APPLICATION_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.DropboxConsumerKey` (`SHAREXRS_KEY_DROPBOX_CONSUMER_KEY`).
pub const DROPBOX_CONSUMER_KEY: &str = match option_env!("SHAREXRS_KEY_DROPBOX_CONSUMER_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.DropboxConsumerSecret` (`SHAREXRS_KEY_DROPBOX_CONSUMER_SECRET`).
pub const DROPBOX_CONSUMER_SECRET: &str = match option_env!("SHAREXRS_KEY_DROPBOX_CONSUMER_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.BoxClientID` (`SHAREXRS_KEY_BOX_CLIENT_ID`).
pub const BOX_CLIENT_ID: &str = match option_env!("SHAREXRS_KEY_BOX_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.BoxClientSecret` (`SHAREXRS_KEY_BOX_CLIENT_SECRET`).
pub const BOX_CLIENT_SECRET: &str = match option_env!("SHAREXRS_KEY_BOX_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.SendSpaceKey` (`SHAREXRS_KEY_SEND_SPACE_KEY`).
pub const SEND_SPACE_KEY: &str = match option_env!("SHAREXRS_KEY_SEND_SPACE_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.MediaFireAppId` (`SHAREXRS_KEY_MEDIA_FIRE_APP_ID`).
pub const MEDIA_FIRE_APP_ID: &str = match option_env!("SHAREXRS_KEY_MEDIA_FIRE_APP_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.MediaFireApiKey` (`SHAREXRS_KEY_MEDIA_FIRE_API_KEY`).
pub const MEDIA_FIRE_API_KEY: &str = match option_env!("SHAREXRS_KEY_MEDIA_FIRE_API_KEY") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.OneDriveClientID` (`SHAREXRS_KEY_ONE_DRIVE_CLIENT_ID`).
pub const ONE_DRIVE_CLIENT_ID: &str = match option_env!("SHAREXRS_KEY_ONE_DRIVE_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.OneDriveClientSecret` (`SHAREXRS_KEY_ONE_DRIVE_CLIENT_SECRET`).
pub const ONE_DRIVE_CLIENT_SECRET: &str = match option_env!("SHAREXRS_KEY_ONE_DRIVE_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.BitlyClientID` (`SHAREXRS_KEY_BITLY_CLIENT_ID`).
pub const BITLY_CLIENT_ID: &str = match option_env!("SHAREXRS_KEY_BITLY_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.BitlyClientSecret` (`SHAREXRS_KEY_BITLY_CLIENT_SECRET`).
pub const BITLY_CLIENT_SECRET: &str = match option_env!("SHAREXRS_KEY_BITLY_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.GoogleClientID` (`SHAREXRS_KEY_GOOGLE_CLIENT_ID`).
pub const GOOGLE_CLIENT_ID: &str = match option_env!("SHAREXRS_KEY_GOOGLE_CLIENT_ID") {
    Some(v) => v,
    None => "",
};
/// `APIKeys.GoogleClientSecret` (`SHAREXRS_KEY_GOOGLE_CLIENT_SECRET`).
pub const GOOGLE_CLIENT_SECRET: &str = match option_env!("SHAREXRS_KEY_GOOGLE_CLIENT_SECRET") {
    Some(v) => v,
    None => "",
};
