//! `UploaderFactory` and the `*UploaderService` classes: for a destination
//! enum value, whether `UploadersConfig` has what it needs (`CheckConfig`)
//! and the uploader it makes (`CreateUploader`), as `WorkerTask.UploadData`
//! uses them.
//!
//! Destinations that aren't ported yet give `None` from `create_*` and
//! [`Ported::No`] from `check_*`.

use crate::api_keys;
use crate::custom::{CustomFileUploader, CustomTextUploader, CustomUploaderItem, CustomUrlSharer, CustomUrlShortener};
use crate::destinations::file_hosts::{IMGFISH_MAX_FILE_ID_LENGTH, IMGFISH_MIN_FILE_ID_LENGTH, ImgFish, LobFile, Puush, Streamable};
use crate::destinations::files::{Lambda, Pomf, Sul, Transfersh, Uguu};
use crate::destinations::images::{Chevereto, Imgur, Vgyme};
use crate::destinations::self_hosted::{Immich, OwnCloud};
use crate::destinations::sharing::{SimpleUrlSharer, url_format_string};
use crate::destinations::shorteners::{Bitly, FirebaseDynamicLinks, Isgd, Kutt, PlainShortener, Polr, QrNet, Turl, Vurl, Yourls, ZeroWidth};
use crate::destinations::text::{Hastebin, OneTimeSecret, Paste2, PasteEe, Pastebin, Pastie, Slexy, SlexySettings, Upaste};
use crate::destinations::amazon_s3::AmazonS3;
use crate::destinations::cloud_storage::{AzureStorage, BackblazeB2};
use crate::destinations::ftp::{FTPAccount, Ftp};
use crate::destinations::sftp::Sftp;
use crate::destinations::mega::{Mega, from_base64_url};
use crate::destinations::email::{Email, EmailSettings, EmailSharer, show_email_window};
use crate::destinations::oauth_services::{
    BoxUploader, Dropbox, Flickr, FlickrSettings, GitHubGist, GoogleCloudStorage, GoogleDrive, OneDrive, Photobucket, PhotobucketAccountInfo, YouTube,
    dropbox_upload_path,
};
use crate::oauth::OAuthInfo;
use crate::destinations::private_bin::{PrivateBin, PrivateBinSettings};
use crate::destinations::services::{
    Hostr, ImageShack, ImageShackOptions, MediaFire, Plik, Pushbullet, PushbulletSettings, PushbulletSharer, Seafile, SendSpace, mediafire_upload_path,
};
use crate::destinations::shared_folder::{LocalhostAccount, SharedFolderUploader};
use crate::enums::{AccountType, FileDestination, ImageDestination, TextDestination, URLSharingServices, UrlShortenerType};
use crate::oauth::OAuth2Info;
use crate::uploader::{GenericUploader, UrlSharer, UrlShortener};
use crate::UploadersConfig;

/// `TaskReferenceHelper`: what a task tells the factory.
#[derive(Debug, Clone, Default)]
pub struct TaskReferenceHelper {
    pub stop_requested: bool,
    pub override_ftp: bool,
    pub ftp_index: i32,
    pub override_custom_uploader: bool,
    pub custom_uploader_index: i32,
    /// `TaskSettings.AdvancedSettings.TextFormat`.
    pub text_format: String,
    /// `DataType`: which FTP / shared folder account an upload uses.
    pub data_type: TaskDataType,
}

/// `EDataType`, as `TaskReferenceHelper.DataType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TaskDataType {
    #[default]
    Default,
    File,
    Image,
    Text,
    Url,
}

/// `CheckConfig`'s answer, or that the destination isn't ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ported {
    Yes(bool),
    No,
}

fn custom_index(config: &UploadersConfig, task: &TaskReferenceHelper, selected: i32) -> i32 {
    if task.override_custom_uploader {
        // BetweenOrDefault(0, Count - 1): out of range gives 0.
        let max = config.CustomUploadersList.0.len() as i32 - 1;
        if task.custom_uploader_index >= 0 && task.custom_uploader_index <= max { task.custom_uploader_index } else { 0 }
    } else {
        selected
    }
}

fn custom_item(config: &UploadersConfig, index: i32) -> Option<CustomUploaderItem> {
    usize::try_from(index).ok().and_then(|i| config.CustomUploadersList.0.get(i)).cloned()
}

fn valid_index(config: &UploadersConfig, index: i32) -> bool {
    usize::try_from(index).is_ok_and(|i| i < config.CustomUploadersList.0.len())
}

fn ne(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|s| !s.is_empty())
}

/// `ImageUploaderService.CheckConfig`.
pub fn check_image(dest: &ImageDestination, config: &UploadersConfig) -> Ported {
    Ported::Yes(match dest {
        ImageDestination::Imgur => config.ImgurAccountType == AccountType::Anonymous || OAuth2Info::check(config.ImgurOAuth2Info.as_ref()),
        ImageDestination::Chevereto => ne(&config.CheveretoUploader.UploadURL) && !config.CheveretoUploader.APIKey.is_empty(),
        ImageDestination::Vgyme => true,
        ImageDestination::ImageShack => {
            ImageShackOptions::from_value(&config.ImageShackSettings).is_some_and(|o| o.Auth_token.as_deref().is_some_and(|t| !t.is_empty()))
        }
        ImageDestination::Flickr => OAuthInfo::check(config.FlickrOAuthInfo.as_ref()),
        ImageDestination::Photobucket => !config.PhotobucketAccountInfo.is_null() && OAuthInfo::check(config.PhotobucketOAuthInfo.as_ref()),
        ImageDestination::CustomImageUploader => valid_index(config, config.CustomImageUploaderSelected),
        _ => return Ported::No,
    })
}

/// `ImageUploaderService.CreateUploader`.
pub fn create_image(dest: &ImageDestination, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<GenericUploader> {
    Some(match dest {
        ImageDestination::Imgur => {
            if config.ImgurOAuth2Info.is_none() {
                config.ImgurOAuth2Info = Some(OAuth2Info::new(api_keys::IMGUR_CLIENT_ID, api_keys::IMGUR_CLIENT_SECRET));
            }
            let album = if config.ImgurUploadSelectedAlbum { config.ImgurSelectedAlbum.get("id").and_then(|v| v.as_str()).map(str::to_owned) } else { None };
            let mut u = Imgur::new(config.ImgurOAuth2Info.clone().expect("set above"));
            u.upload_method = config.ImgurAccountType.clone();
            u.direct_link = config.ImgurDirectLink;
            u.thumbnail_type = config.ImgurThumbnailType.clone();
            u.use_gifv = config.ImgurUseGIFV;
            u.upload_album_id = album;
            GenericUploader::File(Box::new(u))
        }
        ImageDestination::Chevereto => {
            let mut u = Chevereto::new(config.CheveretoUploader.clone());
            u.direct_url = config.CheveretoDirectURL;
            GenericUploader::File(Box::new(u))
        }
        ImageDestination::Vgyme => GenericUploader::File(Box::new(Vgyme::new(config.VgymeUserKey.as_str()))),
        ImageDestination::ImageShack => {
            let options = ImageShackOptions::from_value(&config.ImageShackSettings)?;
            GenericUploader::File(Box::new(ImageShack::new(api_keys::IMAGE_SHACK_KEY, options)))
        }
        ImageDestination::Flickr => {
            GenericUploader::File(Box::new(Flickr::new(config.FlickrOAuthInfo.clone()?, FlickrSettings::from_value(&config.FlickrSettings))))
        }
        ImageDestination::Photobucket => GenericUploader::File(Box::new(Photobucket::new(
            config.PhotobucketOAuthInfo.clone()?,
            PhotobucketAccountInfo::from_value(&config.PhotobucketAccountInfo)?,
        ))),
        ImageDestination::CustomImageUploader => {
            let index = custom_index(config, task, config.CustomImageUploaderSelected);
            GenericUploader::File(Box::new(CustomFileUploader::new(custom_item(config, index)?)))
        }
        _ => return None,
    })
}

/// `TextUploaderService.CheckConfig`.
pub fn check_text(dest: &TextDestination, config: &UploadersConfig) -> Ported {
    Ported::Yes(match dest {
        TextDestination::Pastebin
        | TextDestination::Paste2
        | TextDestination::Slexy
        | TextDestination::Paste_ee
        | TextDestination::Upaste
        | TextDestination::Hastebin
        | TextDestination::OneTimeSecret
        | TextDestination::Pastie
        | TextDestination::PrivateBin => true,
        TextDestination::Gist => OAuth2Info::check(config.GistOAuth2Info.as_ref()),
        TextDestination::CustomTextUploader => valid_index(config, config.CustomTextUploaderSelected),
        _ => return Ported::No,
    })
}

/// `TextUploaderService.CreateUploader`.
pub fn create_text(dest: &TextDestination, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<GenericUploader> {
    Some(match dest {
        TextDestination::Pastebin => {
            if config.PastebinSettings.TextFormat.as_deref().is_none_or(str::is_empty) {
                config.PastebinSettings.TextFormat = Some(task.text_format.clone());
            }
            GenericUploader::Text(Box::new(Pastebin::new(api_keys::PASTEBIN_KEY, config.PastebinSettings.clone())))
        }
        TextDestination::Paste2 => GenericUploader::Text(Box::new(Paste2::new(&task.text_format))),
        TextDestination::Slexy => {
            GenericUploader::Text(Box::new(Slexy::new(SlexySettings { text_format: task.text_format.clone(), ..Default::default() })))
        }
        TextDestination::Paste_ee => {
            let key = if config.Paste_eeUserKey.is_empty() { api_keys::PASTE_EE_APPLICATION_KEY } else { config.Paste_eeUserKey.as_str() };
            let mut u = PasteEe::new(key);
            u.encrypt_paste = config.Paste_eeEncryptPaste;
            GenericUploader::Text(Box::new(u))
        }
        TextDestination::Upaste => {
            let mut u = Upaste::new(config.UpasteUserKey.as_str());
            u.is_public = config.UpasteIsPublic;
            GenericUploader::Text(Box::new(u))
        }
        TextDestination::Hastebin => {
            let mut u = Hastebin::new();
            u.custom_domain = config.HastebinCustomDomain.clone().unwrap_or_default();
            u.syntax_highlighting = config.HastebinSyntaxHighlighting.clone().unwrap_or_default();
            u.use_file_extension = config.HastebinUseFileExtension;
            GenericUploader::Text(Box::new(u))
        }
        TextDestination::OneTimeSecret => GenericUploader::Text(Box::new(OneTimeSecret::new(
            config.OneTimeSecretAPIUsername.as_deref().unwrap_or(""),
            config.OneTimeSecretAPIKey.as_str(),
        ))),
        TextDestination::Pastie => GenericUploader::Text(Box::new(Pastie::new(config.PastieIsPublic))),
        TextDestination::Gist => {
            let mut u = GitHubGist::new(config.GistOAuth2Info.clone()?);
            u.public_upload = config.GistPublishPublic;
            u.raw_url = config.GistRawURL;
            u.custom_url_api = config.GistCustomURL.clone().unwrap_or_default();
            GenericUploader::Text(Box::new(u))
        }
        TextDestination::PrivateBin => GenericUploader::Text(Box::new(PrivateBin::new(PrivateBinSettings::from_value(&config.PrivateBinSettings)))),
        TextDestination::CustomTextUploader => {
            let index = custom_index(config, task, config.CustomTextUploaderSelected);
            GenericUploader::Text(Box::new(CustomTextUploader::new(custom_item(config, index)?)))
        }
        _ => return None,
    })
}

/// `FileUploaderService.CheckConfig`.
pub fn check_file(dest: &FileDestination, config: &UploadersConfig) -> Ported {
    Ported::Yes(match dest {
        FileDestination::Pomf => ne(&config.PomfUploader.UploadURL),
        FileDestination::Uguu | FileDestination::Transfersh => true,
        FileDestination::Sul => !config.SulAPIKey.is_empty(),
        FileDestination::Lambda => !config.LambdaSettings.UserAPIKey.is_empty(),
        FileDestination::AmazonS3 => AmazonS3::check_config(&config.AmazonS3Settings),
        FileDestination::Puush => !config.PuushAPIKey.is_empty(),
        FileDestination::Immich => Immich::check_config(config.ImmichURL.as_deref().unwrap_or(""), config.ImmichAPIKey.as_str()),
        FileDestination::OwnCloud => ne(&config.OwnCloudHost) && ne(&config.OwnCloudUsername) && !config.OwnCloudPassword.is_empty(),
        FileDestination::Streamable => ne(&config.StreamableUsername) && !config.StreamablePassword.is_empty(),
        FileDestination::Lithiio => !config.LithiioSettings.UserAPIKey.is_empty(),
        FileDestination::ImgFish => (IMGFISH_MIN_FILE_ID_LENGTH..=IMGFISH_MAX_FILE_ID_LENGTH).contains(&config.ImgFishSettings.FileIDLength),
        FileDestination::FTP => {
            FTPAccount::list_from_value(&config.FTPAccountList).is_some_and(|l| usize::try_from(config.FTPSelectedFile).is_ok_and(|i| i < l.len()))
        }
        FileDestination::Email => email_configured(config),
        FileDestination::Mega => !config.MegaSessionID.as_str().trim().is_empty() && !config.MegaMasterKey.as_str().trim().is_empty(),
        FileDestination::Dropbox => OAuth2Info::check(config.DropboxOAuth2Info.as_ref()),
        FileDestination::OneDrive => OAuth2Info::check(config.OneDriveV2OAuth2Info.as_ref()),
        FileDestination::Box => OAuth2Info::check(config.BoxOAuth2Info.as_ref()),
        FileDestination::GoogleDrive => OAuth2Info::check(config.GoogleDriveOAuth2Info.as_ref()),
        FileDestination::GoogleCloudStorage => OAuth2Info::check(config.GoogleCloudStorageOAuth2Info.as_ref()) && ne(&config.GoogleCloudStorageBucket),
        FileDestination::YouTube => OAuth2Info::check(config.YouTubeOAuth2Info.as_ref()),
        FileDestination::AzureStorage => {
            ne(&config.AzureStorageAccountName) && !config.AzureStorageAccountAccessKey.is_empty() && ne(&config.AzureStorageContainer)
        }
        FileDestination::BackblazeB2 => {
            config.B2ApplicationKeyId.as_deref().is_some_and(|s| !s.trim().is_empty()) && !config.B2ApplicationKey.as_str().trim().is_empty()
        }
        FileDestination::Localhostr => ne(&config.LocalhostrEmail) && !config.LocalhostrPassword.is_empty(),
        FileDestination::Plik => ne(&config.PlikSettings.URL) && !config.PlikSettings.APIKey.is_empty(),
        FileDestination::Seafile => ne(&config.SeafileAPIURL) && !config.SeafileAuthToken.is_empty() && ne(&config.SeafileRepoID),
        FileDestination::Pushbullet => PushbulletSettings::check(PushbulletSettings::from_value(&config.PushbulletSettings).as_ref()),
        FileDestination::SendSpace => {
            config.SendSpaceAccountType == AccountType::Anonymous || (ne(&config.SendSpaceUsername) && !config.SendSpacePassword.is_empty())
        }
        FileDestination::MediaFire => ne(&config.MediaFireUsername) && !config.MediaFirePassword.is_empty(),
        FileDestination::SharedFolder => LocalhostAccount::list_from_value(&config.LocalhostAccountList)
            .is_some_and(|l| usize::try_from(config.LocalhostSelectedFiles).is_ok_and(|i| i < l.len())),
        FileDestination::CustomFileUploader => valid_index(config, config.CustomFileUploaderSelected),
        _ => return Ported::No,
    })
}

/// `FileUploaderService.CreateUploader`.
pub fn create_file(dest: &FileDestination, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<GenericUploader> {
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    Some(match dest {
        FileDestination::Pomf => GenericUploader::File(Box::new(Pomf::new(config.PomfUploader.clone()))),
        FileDestination::Uguu => GenericUploader::File(Box::new(Uguu::new())),
        FileDestination::Transfersh => GenericUploader::File(Box::new(Transfersh::new())),
        FileDestination::Sul => GenericUploader::File(Box::new(Sul::new(config.SulAPIKey.as_str()))),
        FileDestination::Lambda => {
            // Correct old URLs
            if config.LambdaSettings.UploadURL.as_deref() == Some("https://λ.pw/") {
                config.LambdaSettings.UploadURL = Some("https://lbda.net/".into());
            }
            GenericUploader::File(Box::new(Lambda::new(config.LambdaSettings.clone())))
        }
        FileDestination::AmazonS3 => GenericUploader::File(Box::new(AmazonS3::new(config.AmazonS3Settings.clone()))),
        FileDestination::Puush => GenericUploader::File(Box::new(Puush::new(config.PuushAPIKey.as_str()))),
        FileDestination::Immich => {
            let mut u = Immich::new(config.ImmichURL.as_deref().unwrap_or(""), config.ImmichAPIKey.as_str());
            u.auto_create_shareable_link = config.ImmichAutoCreateShareableLink;
            u.show_metadata = config.ImmichShowMetadata;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::OwnCloud => {
            let s = |o: &Option<String>| o.clone().unwrap_or_default();
            let mut u = OwnCloud::new(&s(&config.OwnCloudHost), &s(&config.OwnCloudUsername), config.OwnCloudPassword.as_str());
            u.path = s(&config.OwnCloudPath);
            u.create_share = config.OwnCloudCreateShare;
            u.direct_link = config.OwnCloudDirectLink;
            u.preview_link = config.OwnCloudUsePreviewLinks;
            u.append_file_name_to_url = config.OwnCloudAppendFileNameToURL;
            u.is_compatibility_81 = config.OwnCloud81Compatibility;
            u.auto_expire_time = config.OwnCloudExpiryTime;
            u.auto_expire = config.OwnCloudAutoExpire;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Streamable => {
            let mut u = Streamable::new(config.StreamableUsername.as_deref().unwrap_or(""), config.StreamablePassword.as_str());
            u.use_direct_url = config.StreamableUseDirectURL;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Lithiio => GenericUploader::File(Box::new(LobFile::new(config.LithiioSettings.UserAPIKey.as_str()))),
        FileDestination::ImgFish => GenericUploader::File(Box::new(ImgFish::new(config.ImgFishSettings.clone()))),
        FileDestination::FTP => {
            let accounts = FTPAccount::list_from_value(&config.FTPAccountList)?;
            let index = if task.override_ftp {
                // BetweenOrDefault(0, Count - 1)
                let max = accounts.len() as i32 - 1;
                if task.ftp_index >= 0 && task.ftp_index <= max { task.ftp_index } else { 0 }
            } else {
                match task.data_type {
                    TaskDataType::Image => config.FTPSelectedImage,
                    TaskDataType::Text => config.FTPSelectedText,
                    _ => config.FTPSelectedFile,
                }
            };
            let account = accounts.into_iter().nth(usize::try_from(index).ok()?)?;
            match account.Protocol {
                crate::enums::FTPProtocol::FTP | crate::enums::FTPProtocol::FTPS => GenericUploader::File(Box::new(Ftp::new(account))),
                crate::enums::FTPProtocol::SFTP => GenericUploader::File(Box::new(Sftp::new(account))),
                _ => return None,
            }
        }
        FileDestination::Mega => {
            let mut u = Mega::with_session(config.MegaSessionID.as_str(), from_base64_url(config.MegaMasterKey.as_str()).unwrap_or_default());
            u.folder_id = config.MegaSelectedFolder.get("ID").and_then(|v| v.as_str()).map(str::to_owned);
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Email => {
            let settings = email_settings(config);
            let automatic_to = s(&config.EmailAutomaticSendTo);
            if config.EmailAutomaticSend && !automatic_to.is_empty() {
                GenericUploader::File(Box::new(Email::new(settings, &automatic_to, &s(&config.EmailDefaultSubject), &s(&config.EmailDefaultBody))))
            } else {
                let first = if config.EmailRememberLastTo { s(&config.EmailLastTo) } else { String::new() };
                // `EmailWindowIntegration.Show`; cancelled: no uploader (ShareX stops the task).
                let (to, subject, body) = show_email_window(&first, &s(&config.EmailDefaultSubject), &s(&config.EmailDefaultBody))?;
                if config.EmailRememberLastTo {
                    config.EmailLastTo = Some(to.clone());
                }
                GenericUploader::File(Box::new(Email::new(settings, &to, &subject, &body)))
            }
        }
        FileDestination::Dropbox => {
            let mut u = Dropbox::new(config.DropboxOAuth2Info.clone()?);
            u.upload_path = dropbox_upload_path(&s(&config.DropboxUploadPath));
            u.auto_create_shareable_link = config.DropboxAutoCreateShareableLink;
            u.use_direct_link = config.DropboxUseDirectLink;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::OneDrive => {
            let mut u = OneDrive::new(config.OneDriveV2OAuth2Info.clone()?);
            u.folder_id = config.OneDriveV2SelectedFolder.get("id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            u.auto_create_shareable_link = config.OneDriveAutoCreateShareableLink;
            u.use_direct_link = config.OneDriveUseDirectLink;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Box => {
            let mut u = BoxUploader::new(config.BoxOAuth2Info.clone()?);
            u.folder_id = config.BoxSelectedFolder.get("id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            u.share = config.BoxShare;
            u.share_access_level = config.BoxShareAccessLevel.clone();
            GenericUploader::File(Box::new(u))
        }
        FileDestination::GoogleDrive => {
            let mut u = GoogleDrive::new(config.GoogleDriveOAuth2Info.clone()?);
            u.is_public = config.GoogleDriveIsPublic;
            u.direct_link = config.GoogleDriveDirectLink;
            u.folder_id = if config.GoogleDriveUseFolder { config.GoogleDriveFolderID.clone() } else { None };
            u.drive_id = config.GoogleDriveSelectedDrive.get("id").and_then(|v| v.as_str()).map(str::to_owned);
            GenericUploader::File(Box::new(u))
        }
        FileDestination::GoogleCloudStorage => {
            let mut u = GoogleCloudStorage::new(config.GoogleCloudStorageOAuth2Info.clone()?);
            u.bucket = s(&config.GoogleCloudStorageBucket);
            u.domain = s(&config.GoogleCloudStorageDomain);
            u.prefix = s(&config.GoogleCloudStorageObjectPrefix);
            u.remove_extension_image = config.GoogleCloudStorageRemoveExtensionImage;
            u.remove_extension_text = config.GoogleCloudStorageRemoveExtensionText;
            u.remove_extension_video = config.GoogleCloudStorageRemoveExtensionVideo;
            u.set_public_acl = config.GoogleCloudStorageSetPublicACL;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::YouTube => {
            let mut u = YouTube::new(config.YouTubeOAuth2Info.clone()?);
            u.privacy_type = config.YouTubePrivacyType.clone();
            u.use_shortened_link = config.YouTubeUseShortenedLink;
            u.show_dialog = config.YouTubeShowDialog;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::AzureStorage => GenericUploader::File(Box::new(AzureStorage::new(
            &s(&config.AzureStorageAccountName),
            config.AzureStorageAccountAccessKey.as_str(),
            &s(&config.AzureStorageContainer),
            &s(&config.AzureStorageEnvironment),
            &s(&config.AzureStorageCustomDomain),
            &s(&config.AzureStorageUploadPath),
            &s(&config.AzureStorageCacheControl),
        ))),
        FileDestination::BackblazeB2 => GenericUploader::File(Box::new(BackblazeB2::new(
            &s(&config.B2ApplicationKeyId),
            config.B2ApplicationKey.as_str(),
            &s(&config.B2BucketName),
            &s(&config.B2UploadPath),
            config.B2UseCustomUrl,
            &s(&config.B2CustomUrl),
        ))),
        FileDestination::Localhostr => {
            let mut u = Hostr::new(&s(&config.LocalhostrEmail), config.LocalhostrPassword.as_str());
            u.direct_url = config.LocalhostrDirectURL;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Plik => GenericUploader::File(Box::new(Plik::new(config.PlikSettings.clone()))),
        FileDestination::Seafile => {
            let mut u = Seafile::new(&s(&config.SeafileAPIURL), config.SeafileAuthToken.as_str(), &s(&config.SeafileRepoID));
            u.path = s(&config.SeafilePath);
            u.is_library_encrypted = config.SeafileIsLibraryEncrypted;
            u.encrypted_library_password = config.SeafileEncryptedLibraryPassword.as_str().to_owned();
            u.share_days_to_expire = config.SeafileShareDaysToExpire;
            u.share_password = config.SeafileSharePassword.as_str().to_owned();
            u.create_shareable_url = config.SeafileCreateShareableURL;
            u.create_shareable_url_raw = config.SeafileCreateShareableURLRaw;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::Pushbullet => GenericUploader::File(Box::new(Pushbullet::new(PushbulletSettings::from_value(&config.PushbulletSettings)?))),
        FileDestination::SendSpace => {
            let mut u = SendSpace::new(api_keys::SEND_SPACE_KEY);
            u.account_type = config.SendSpaceAccountType.clone();
            u.username = s(&config.SendSpaceUsername);
            u.password = config.SendSpacePassword.as_str().to_owned();
            GenericUploader::File(Box::new(u))
        }
        FileDestination::MediaFire => {
            let mut u = MediaFire::new(api_keys::MEDIA_FIRE_APP_ID, api_keys::MEDIA_FIRE_API_KEY, &s(&config.MediaFireUsername), config.MediaFirePassword.as_str());
            u.upload_path = mediafire_upload_path(&s(&config.MediaFirePath));
            u.use_long_link = config.MediaFireUseLongLink;
            GenericUploader::File(Box::new(u))
        }
        FileDestination::SharedFolder => {
            let index = match task.data_type {
                TaskDataType::Image => config.LocalhostSelectedImages,
                TaskDataType::Text => config.LocalhostSelectedText,
                _ => config.LocalhostSelectedFiles,
            };
            let index = usize::try_from(index).ok()?;
            let account = LocalhostAccount::list_from_value(&config.LocalhostAccountList)?.into_iter().nth(index)?;
            GenericUploader::File(Box::new(SharedFolderUploader::new(account, index)))
        }
        FileDestination::CustomFileUploader => {
            let index = custom_index(config, task, config.CustomFileUploaderSelected);
            GenericUploader::File(Box::new(CustomFileUploader::new(custom_item(config, index)?)))
        }
        _ => return None,
    })
}

/// `URLShortenerService.CheckConfig`.
pub fn check_url_shortener(dest: &UrlShortenerType, config: &UploadersConfig) -> Ported {
    Ported::Yes(match dest {
        UrlShortenerType::BITLY => OAuth2Info::check(config.BitlyOAuth2Info.as_ref()),
        UrlShortenerType::ISGD
        | UrlShortenerType::VGD
        | UrlShortenerType::TINYURL
        | UrlShortenerType::TURL
        | UrlShortenerType::QRnet
        | UrlShortenerType::VURL
        | UrlShortenerType::TwoGP
        | UrlShortenerType::ZeroWidthShortener => true,
        UrlShortenerType::YOURLS => {
            ne(&config.YourlsAPIURL) && (!config.YourlsSignature.is_empty() || (ne(&config.YourlsUsername) && !config.YourlsPassword.is_empty()))
        }
        UrlShortenerType::Polr => ne(&config.PolrAPIHostname) && !config.PolrAPIKey.is_empty(),
        UrlShortenerType::FirebaseDynamicLinks => !config.FirebaseWebAPIKey.is_empty() && ne(&config.FirebaseDynamicLinkDomain),
        UrlShortenerType::Kutt => !config.KuttSettings.APIKey.is_empty(),
        UrlShortenerType::CustomURLShortener => valid_index(config, config.CustomURLShortenerSelected),
        _ => return Ported::No,
    })
}

/// `URLShortenerService.CreateShortener`.
pub fn create_url_shortener(dest: &UrlShortenerType, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<Box<dyn UrlShortener>> {
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    Some(match dest {
        UrlShortenerType::BITLY => {
            if config.BitlyOAuth2Info.is_none() {
                config.BitlyOAuth2Info = Some(OAuth2Info::new(api_keys::BITLY_CLIENT_ID, api_keys::BITLY_CLIENT_SECRET));
            }
            let mut u = Bitly::new(config.BitlyOAuth2Info.clone().expect("set above"));
            u.domain = s(&config.BitlyDomain);
            Box::new(u)
        }
        UrlShortenerType::ISGD => Box::new(Isgd::for_isgd()),
        UrlShortenerType::VGD => Box::new(Isgd::for_vgd()),
        UrlShortenerType::TINYURL => Box::new(PlainShortener::tinyurl()),
        UrlShortenerType::TURL => Box::new(Turl::new()),
        UrlShortenerType::QRnet => Box::new(QrNet::for_qrnet()),
        UrlShortenerType::TwoGP => Box::new(QrNet::for_twogp()),
        UrlShortenerType::VURL => Box::new(Vurl::new()),
        UrlShortenerType::YOURLS => Box::new(Yourls::new(
            &s(&config.YourlsAPIURL),
            config.YourlsSignature.as_str(),
            &s(&config.YourlsUsername),
            config.YourlsPassword.as_str(),
        )),
        UrlShortenerType::Polr => {
            let mut u = Polr::new(&s(&config.PolrAPIHostname), config.PolrAPIKey.as_str());
            u.is_secret = config.PolrIsSecret;
            u.use_api_v1 = config.PolrUseAPIv1;
            Box::new(u)
        }
        UrlShortenerType::FirebaseDynamicLinks => {
            Box::new(FirebaseDynamicLinks::new(config.FirebaseWebAPIKey.as_str(), &s(&config.FirebaseDynamicLinkDomain), config.FirebaseIsShort))
        }
        UrlShortenerType::Kutt => Box::new(Kutt::new(config.KuttSettings.clone())),
        UrlShortenerType::ZeroWidthShortener => Box::new(ZeroWidth::new(&s(&config.ZeroWidthShortenerURL), &s(&config.ZeroWidthShortenerToken))),
        UrlShortenerType::CustomURLShortener => {
            let index = custom_index(config, task, config.CustomURLShortenerSelected);
            Box::new(CustomUrlShortener::new(custom_item(config, index)?))
        }
        _ => return None,
    })
}

/// `URLSharingService.CheckConfig`.
pub fn check_url_sharing(dest: &URLSharingServices, config: &UploadersConfig) -> Ported {
    if url_format_string(dest).is_some() {
        return Ported::Yes(true);
    }
    match dest {
        URLSharingServices::CustomURLSharingService => Ported::Yes(valid_index(config, config.CustomURLSharingServiceSelected)),
        URLSharingServices::Email => Ported::Yes(email_configured(config)),
        URLSharingServices::Pushbullet => Ported::Yes(PushbulletSettings::check(PushbulletSettings::from_value(&config.PushbulletSettings).as_ref())),
        _ => Ported::No,
    }
}

/// `URLSharingService.CreateSharer`.
pub fn create_url_sharing(dest: &URLSharingServices, config: &mut UploadersConfig, task: &TaskReferenceHelper) -> Option<Box<dyn UrlSharer>> {
    if let Some(format) = url_format_string(dest) {
        return Some(Box::new(SimpleUrlSharer::new(format)));
    }
    match dest {
        URLSharingServices::CustomURLSharingService => {
            let index = custom_index(config, task, config.CustomURLSharingServiceSelected);
            Some(Box::new(CustomUrlSharer::new(custom_item(config, index)?)))
        }
        URLSharingServices::Email => {
            let mut u = EmailSharer::new(email_settings(config), config.EmailDefaultSubject.as_deref().unwrap_or(""));
            let to = config.EmailAutomaticSendTo.clone().unwrap_or_default();
            u.automatic_to = (config.EmailAutomaticSend && !to.is_empty()).then_some(to);
            u.last_to = config.EmailLastTo.clone().unwrap_or_default();
            u.remember_last_to = config.EmailRememberLastTo;
            Some(Box::new(u))
        }
        URLSharingServices::Pushbullet => Some(Box::new(PushbulletSharer::new(PushbulletSettings::from_value(&config.PushbulletSettings)?))),
        _ => None,
    }
}

/// `EmailFileUploaderService.CheckConfig` (and the sharing service's).
fn email_configured(config: &UploadersConfig) -> bool {
    ne(&config.EmailSmtpServer) && config.EmailSmtpPort > 0 && ne(&config.EmailFrom) && !config.EmailPassword.is_empty()
}

fn email_settings(config: &UploadersConfig) -> EmailSettings {
    EmailSettings {
        smtp_server: config.EmailSmtpServer.clone().unwrap_or_default(),
        smtp_port: config.EmailSmtpPort,
        from_email: config.EmailFrom.clone().unwrap_or_default(),
        password: config.EmailPassword.as_str().to_owned(),
    }
}

/// `UploaderService.ServiceName`: the destination's name as ShareX shows it
/// (its `[Description]`, or the localized name for ShareX's own kinds).
pub fn service_name(enum_name: &str, value: &str) -> String {
    if let Some(s) = sharex_strings::uploaders_lib::lookup(&format!("{enum_name}_{value}")) {
        return s.to_owned();
    }
    enum_description(enum_name, value)
}

/// `EnumExtensions.GetDescription` of a destination enum value: its
/// `[Description]`, else its name.
pub fn enum_description(enum_name: &str, value: &str) -> String {
    DESCRIPTIONS.iter().find(|(e, v, _)| *e == enum_name && *v == value).map(|(_, _, d)| (*d).to_owned()).unwrap_or_else(|| value.to_owned())
}

/// The `[Description]` attributes of the destination enums (`Enums.cs`).
static DESCRIPTIONS: &[(&str, &str, &str)] = &[
    ("ImageDestination", "Imgur", "Imgur"),
    ("ImageDestination", "ImageShack", "ImageShack"),
    ("ImageDestination", "Flickr", "Flickr"),
    ("ImageDestination", "Photobucket", "Photobucket"),
    ("ImageDestination", "Chevereto", "Chevereto"),
    ("ImageDestination", "Vgyme", "vgy.me"),
    ("TextDestination", "Pastebin", "Pastebin"),
    ("TextDestination", "Paste2", "Paste2"),
    ("TextDestination", "Slexy", "Slexy"),
    ("TextDestination", "Paste_ee", "Paste.ee"),
    ("TextDestination", "Gist", "GitHub Gist"),
    ("TextDestination", "Upaste", "uPaste"),
    ("TextDestination", "Hastebin", "Hastebin"),
    ("TextDestination", "OneTimeSecret", "OneTimeSecret"),
    ("TextDestination", "Pastie", "Pastie"),
    ("TextDestination", "PrivateBin", "PrivateBin"),
    ("FileDestination", "Dropbox", "Dropbox"),
    ("FileDestination", "FTP", "FTP"),
    ("FileDestination", "OneDrive", "OneDrive"),
    ("FileDestination", "GoogleDrive", "Google Drive"),
    ("FileDestination", "Puush", "puush"),
    ("FileDestination", "Box", "Box"),
    ("FileDestination", "Mega", "MEGA"),
    ("FileDestination", "AmazonS3", "Amazon S3"),
    ("FileDestination", "GoogleCloudStorage", "Google Cloud Storage"),
    ("FileDestination", "AzureStorage", "Azure Storage"),
    ("FileDestination", "BackblazeB2", "Backblaze B2"),
    ("FileDestination", "OwnCloud", "ownCloud / Nextcloud"),
    ("FileDestination", "Immich", "Immich"),
    ("FileDestination", "MediaFire", "MediaFire"),
    ("FileDestination", "Pushbullet", "Pushbullet"),
    ("FileDestination", "SendSpace", "SendSpace"),
    ("FileDestination", "Localhostr", "Hostr"),
    ("FileDestination", "Lambda", "Lambda"),
    ("FileDestination", "ImgFish", "img.fish"),
    ("FileDestination", "Pomf", "Pomf"),
    ("FileDestination", "Uguu", "Uguu"),
    ("FileDestination", "Seafile", "Seafile"),
    ("FileDestination", "Streamable", "Streamable"),
    ("FileDestination", "Sul", "s-ul"),
    ("FileDestination", "Lithiio", "LobFile"),
    ("FileDestination", "Transfersh", "transfer.sh"),
    ("FileDestination", "Plik", "Plik"),
    ("FileDestination", "YouTube", "YouTube"),
    ("UrlShortenerType", "BITLY", "bit.ly"),
    ("UrlShortenerType", "ISGD", "is.gd"),
    ("UrlShortenerType", "VGD", "v.gd"),
    ("UrlShortenerType", "TINYURL", "tinyurl.com"),
    ("UrlShortenerType", "TURL", "turl.ca"),
    ("UrlShortenerType", "YOURLS", "yourls.org"),
    ("UrlShortenerType", "QRnet", "qr.net"),
    ("UrlShortenerType", "VURL", "vurl.com"),
    ("UrlShortenerType", "TwoGP", "2.gp"),
    ("UrlShortenerType", "Polr", "Polr"),
    ("UrlShortenerType", "FirebaseDynamicLinks", "Firebase Dynamic Links"),
    ("UrlShortenerType", "Kutt", "Kutt"),
    ("UrlShortenerType", "ZeroWidthShortener", "Zero Width Shortener"),
    ("URLSharingServices", "Facebook", "Facebook"),
    ("URLSharingServices", "Reddit", "Reddit"),
    ("URLSharingServices", "Pinterest", "Pinterest"),
    ("URLSharingServices", "Tumblr", "Tumblr"),
    ("URLSharingServices", "LinkedIn", "LinkedIn"),
    ("URLSharingServices", "StumbleUpon", "StumbleUpon"),
    ("URLSharingServices", "Delicious", "Delicious"),
    ("URLSharingServices", "VK", "VK"),
    ("URLSharingServices", "Pushbullet", "Pushbullet"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every destination ShareX has is ported (the `FileUploader` entries
    /// of the image and text enums mean "use the file destination").
    #[test]
    fn every_destination_is_ported() {
        let config = UploadersConfig::default();
        for d in ImageDestination::ALL.iter().filter(|d| d.name() != "FileUploader") {
            assert_ne!(check_image(d, &config), Ported::No, "{}", d.name());
        }
        for d in TextDestination::ALL.iter().filter(|d| d.name() != "FileUploader") {
            assert_ne!(check_text(d, &config), Ported::No, "{}", d.name());
        }
        for d in FileDestination::ALL {
            assert_ne!(check_file(d, &config), Ported::No, "{}", d.name());
        }
        for d in UrlShortenerType::ALL {
            assert_ne!(check_url_shortener(d, &config), Ported::No, "{}", d.name());
        }
        for d in URLSharingServices::ALL {
            assert_ne!(check_url_sharing(d, &config), Ported::No, "{}", d.name());
        }
    }
}
