//! `UploadersConfig.cs` (`UploadersConfig.json`): every destination's
//! account settings, in ShareX's property order with its defaults.
//!
//! Settings of destinations that aren't ported yet are kept as JSON
//! (`Value`), exactly as read. `[JsonEncrypt]` strings are [`Secret`]s
//! (DPAPI on Windows, as ShareX with `SupportDPAPIEncryption`). Properties
//! this version doesn't know are kept in `extra`. A ShareX profile reads and
//! writes back byte for byte.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use sharex_settings::settings;
use sharex_settings::settings_base::SettingsFile;

use crate::custom::{CustomUploaderItem, SerializeMode};
use crate::dpapi::Secret;
use crate::enums::{AccountType, AmazonS3StorageClass, BoxShareAccessLevel, ImgurThumbnailType, PastebinExpiration, PastebinPrivacy, YouTubeVideoPrivacy};
use crate::oauth::{OAuth2Info, OAuthInfo, OAuthUserInfo};

settings! {
    /// `CheveretoUploader`.
    pub struct CheveretoUploader {
        UploadURL: Option<String> = None,
        APIKey: Secret = Secret::null(),
    }
}

settings! {
    /// `PastebinSettings`.
    pub struct PastebinSettings {
        Username: Option<String> = None,
        Password: Secret = Secret::null(),
        Exposure: PastebinPrivacy = PastebinPrivacy::Unlisted,
        Expiration: PastebinExpiration = PastebinExpiration::N,
        Title: Option<String> = None,
        TextFormat: Option<String> = Some("text".into()),
        UserKey: Secret = Secret::null(),
        RawURL: bool = false,
    }
}

settings! {
    /// `AmazonS3Settings`.
    pub struct AmazonS3Settings {
        AccessKeyID: Option<String> = None,
        SecretAccessKey: Secret = Secret::null(),
        Endpoint: Option<String> = None,
        Region: Option<String> = None,
        UsePathStyle: bool = false,
        Bucket: Option<String> = None,
        ObjectPrefix: Option<String> = None,
        UseCustomCNAME: bool = false,
        CustomDomain: Option<String> = None,
        StorageClass: AmazonS3StorageClass = AmazonS3StorageClass::STANDARD,
        SetPublicACL: bool = true,
        SignedPayload: bool = false,
        UseMultipartUpload: bool = false,
        MultipartConcurrency: i32 = 4,
        RemoveExtensionImage: bool = false,
        RemoveExtensionVideo: bool = false,
        RemoveExtensionText: bool = false,
    }
}

settings! {
    /// `LambdaSettings`.
    pub struct LambdaSettings {
        UserAPIKey: Secret = Secret::new(""),
        UploadURL: Option<String> = Some("https://lbda.net/".into()),
    }
}

settings! {
    /// `ImgFishSettings`.
    pub struct ImgFishSettings {
        APIKey: Secret = Secret::new(""),
        FileIDLength: i32 = 8,
    }
}

settings! {
    /// `LobFileSettings` (kept under its old name, `LithiioSettings`).
    pub struct LobFileSettings {
        UserAPIKey: Secret = Secret::new(""),
    }
}

settings! {
    /// `PomfUploader`.
    pub struct PomfUploader {
        UploadURL: Option<String> = None,
        ResultURL: Option<String> = None,
    }
}

settings! {
    /// `PlikSettings`.
    pub struct PlikSettings {
        URL: Option<String> = Some(String::new()),
        APIKey: Secret = Secret::new(""),
        IsSecured: bool = false,
        Login: Option<String> = Some(String::new()),
        Password: Secret = Secret::new(""),
        Removable: bool = false,
        OneShot: bool = false,
        TTLUnit: i32 = 2,
        TTL: f64 = 30.0,
        HasComment: bool = false,
        Comment: Option<String> = Some(String::new()),
    }
}

settings! {
    /// `KuttSettings`.
    pub struct KuttSettings {
        Host: Option<String> = Some("https://kutt.it".into()),
        APIKey: Secret = Secret::null(),
        Password: Secret = Secret::null(),
        Reuse: bool = false,
        Domain: Option<String> = None,
    }
}

/// `List<CustomUploaderItem>`, written as `UploadersConfig.json` holds it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CustomUploaderList(pub Vec<CustomUploaderItem>);

impl Serialize for CustomUploaderList {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let values: Vec<Value> = self.0.iter().map(|i| i.to_value(SerializeMode::Config)).collect();
        values.serialize(s)
    }
}

impl<'de> Deserialize<'de> for CustomUploaderList {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = Vec::<Value>::deserialize(d)?;
        Ok(CustomUploaderList(values.into_iter().filter_map(CustomUploaderItem::from_value).collect()))
    }
}

settings! {
    /// `ShareX.UploadersLib.UploadersConfig`.
    pub struct UploadersConfig {
        // Imgur
        ImgurAccountType: AccountType = AccountType::Anonymous,
        ImgurDirectLink: bool = true,
        ImgurThumbnailType: ImgurThumbnailType = ImgurThumbnailType::Medium_Thumbnail,
        ImgurUseGIFV: bool = true,
        ImgurOAuth2Info: Option<OAuth2Info> = None,
        ImgurUploadSelectedAlbum: bool = false,
        /// `ImgurAlbumData`.
        ImgurSelectedAlbum: Value = Value::Null,
        ImgurAlbumList: Value = Value::Null,
        // ImageShack
        ImageShackSettings: Value = json!({"Username": null, "Password": null, "IsPublic": false, "Auth_token": null, "ThumbnailWidth": 256, "ThumbnailHeight": 0}),
        // Flickr
        FlickrOAuthInfo: Option<OAuthInfo> = None,
        FlickrSettings: Value = json!({"DirectLink": true, "Title": null, "Description": null, "Tags": null, "IsPublic": null, "IsFriend": null, "IsFamily": null, "SafetyLevel": null, "ContentType": null, "Hidden": null}),
        // Photobucket
        PhotobucketOAuthInfo: Option<OAuthInfo> = None,
        PhotobucketAccountInfo: Value = Value::Null,
        // Chevereto
        CheveretoUploader: CheveretoUploader = CheveretoUploader::default(),
        CheveretoDirectURL: bool = true,
        // vgy.me
        VgymeUserKey: Secret = Secret::new(""),
        // Pastebin
        PastebinSettings: PastebinSettings = PastebinSettings::default(),
        // Paste.ee
        Paste_eeUserKey: Secret = Secret::new(""),
        Paste_eeEncryptPaste: bool = false,
        // Gist
        GistOAuth2Info: Option<OAuth2Info> = None,
        GistPublishPublic: bool = false,
        GistRawURL: bool = false,
        GistCustomURL: Option<String> = Some(String::new()),
        // uPaste
        UpasteUserKey: Secret = Secret::new(""),
        UpasteIsPublic: bool = false,
        // Hastebin
        HastebinCustomDomain: Option<String> = Some("https://hastebin.com".into()),
        HastebinSyntaxHighlighting: Option<String> = Some("hs".into()),
        HastebinUseFileExtension: bool = true,
        // OneTimeSecret
        OneTimeSecretAPIUsername: Option<String> = Some(String::new()),
        OneTimeSecretAPIKey: Secret = Secret::new(""),
        // Pastie
        PastieIsPublic: bool = false,
        // PrivateBin
        PrivateBinSettings: Value = json!({"Username": null, "Password": null, "BurnAfterReading": false, "Expiration": "W1", "Format": "PlainText", "CustomUrl": "https://privatebin.net/", "PastePassword": ""}),
        // Dropbox
        DropboxOAuth2Info: Option<OAuth2Info> = None,
        DropboxUploadPath: Option<String> = Some("ShareX/%y/%mo".into()),
        DropboxAutoCreateShareableLink: bool = true,
        DropboxUseDirectLink: bool = false,
        // FTP
        /// `List<FTPAccount>`.
        FTPAccountList: Value = json!([]),
        FTPSelectedImage: i32 = 0,
        FTPSelectedText: i32 = 0,
        FTPSelectedFile: i32 = 0,
        // OneDrive
        OneDriveV2OAuth2Info: Option<OAuth2Info> = None,
        OneDriveV2SelectedFolder: Value = json!({"id": "", "name": "Root folder", "webUrl": null}),
        OneDriveAutoCreateShareableLink: bool = true,
        OneDriveUseDirectLink: bool = false,
        // Google Drive
        GoogleDriveOAuth2Info: Option<OAuth2Info> = None,
        GoogleDriveUserInfo: Option<OAuthUserInfo> = None,
        GoogleDriveIsPublic: bool = true,
        GoogleDriveDirectLink: bool = false,
        GoogleDriveUseFolder: bool = false,
        GoogleDriveFolderID: Option<String> = Some(String::new()),
        GoogleDriveSelectedDrive: Value = json!({"id": "", "name": "My drive"}),
        // puush
        PuushAPIKey: Secret = Secret::new(""),
        // SendSpace
        SendSpaceAccountType: AccountType = AccountType::Anonymous,
        SendSpaceUsername: Option<String> = Some(String::new()),
        SendSpacePassword: Secret = Secret::new(""),
        // Box
        BoxOAuth2Info: Option<OAuth2Info> = None,
        BoxSelectedFolder: Value = json!({"type": "folder", "id": "0", "sequence_id": null, "etag": null, "name": "Root folder", "shared_link": null, "parent": null}),
        BoxShare: bool = true,
        BoxShareAccessLevel: BoxShareAccessLevel = BoxShareAccessLevel::Open,
        // MEGA
        MegaEmail: Option<String> = Some(String::new()),
        MegaPassword: Secret = Secret::new(""),
        MegaSessionID: Secret = Secret::new(""),
        MegaMasterKey: Secret = Secret::new(""),
        MegaSelectedFolder: Value = json!({"ID": "", "Name": ""}),
        // Localhostr
        LocalhostrEmail: Option<String> = Some(String::new()),
        LocalhostrPassword: Secret = Secret::new(""),
        LocalhostrDirectURL: bool = true,
        // Shared folder
        /// `List<LocalhostAccount>`.
        LocalhostAccountList: Value = json!([]),
        LocalhostSelectedImages: i32 = 0,
        LocalhostSelectedText: i32 = 0,
        LocalhostSelectedFiles: i32 = 0,
        // Email
        EmailSmtpServer: Option<String> = Some("smtp.gmail.com".into()),
        EmailSmtpPort: i32 = 587,
        EmailFrom: Option<String> = Some("...@gmail.com".into()),
        EmailPassword: Secret = Secret::new(""),
        EmailRememberLastTo: bool = true,
        EmailLastTo: Option<String> = Some(String::new()),
        EmailDefaultSubject: Option<String> = Some("Sending email from ShareX".into()),
        EmailDefaultBody: Option<String> = Some("Screenshot is attached.".into()),
        EmailAutomaticSend: bool = false,
        EmailAutomaticSendTo: Option<String> = Some(String::new()),
        // Amazon S3
        AmazonS3Settings: AmazonS3Settings = AmazonS3Settings { ObjectPrefix: Some("ShareX/%y/%mo".into()), ..AmazonS3Settings::default() },
        // ownCloud / Nextcloud
        OwnCloudHost: Option<String> = Some(String::new()),
        OwnCloudUsername: Option<String> = Some(String::new()),
        OwnCloudPassword: Secret = Secret::new(""),
        OwnCloudPath: Option<String> = Some("/".into()),
        OwnCloudExpiryTime: i32 = 7,
        OwnCloudCreateShare: bool = true,
        OwnCloudDirectLink: bool = false,
        OwnCloud81Compatibility: bool = true,
        OwnCloudUsePreviewLinks: bool = false,
        OwnCloudAppendFileNameToURL: bool = false,
        OwnCloudAutoExpire: bool = false,
        // MediaFire
        MediaFireUsername: Option<String> = Some(String::new()),
        MediaFirePassword: Secret = Secret::new(""),
        MediaFirePath: Option<String> = Some(String::new()),
        MediaFireUseLongLink: bool = false,
        // Pushbullet
        PushbulletSettings: Value = json!({"UserAPIKey": "", "DeviceList": [], "SelectedDevice": 0}),
        // Lambda
        LambdaSettings: LambdaSettings = LambdaSettings::default(),
        // img.fish
        ImgFishSettings: ImgFishSettings = ImgFishSettings::default(),
        // LobFile
        LithiioSettings: LobFileSettings = LobFileSettings::default(),
        // Pomf
        PomfUploader: PomfUploader = PomfUploader::default(),
        // s-ul
        SulAPIKey: Secret = Secret::new(""),
        // Seafile
        SeafileAPIURL: Option<String> = Some(String::new()),
        SeafileAuthToken: Secret = Secret::new(""),
        SeafileRepoID: Option<String> = Some(String::new()),
        SeafilePath: Option<String> = Some("/".into()),
        SeafileIsLibraryEncrypted: bool = false,
        SeafileEncryptedLibraryPassword: Secret = Secret::new(""),
        SeafileCreateShareableURL: bool = true,
        SeafileCreateShareableURLRaw: bool = false,
        SeafileShareDaysToExpire: i32 = 0,
        SeafileSharePassword: Secret = Secret::new(""),
        SeafileAccInfoEmail: Option<String> = Some(String::new()),
        SeafileAccInfoUsage: Option<String> = Some(String::new()),
        // Streamable
        StreamableUsername: Option<String> = Some(String::new()),
        StreamablePassword: Secret = Secret::new(""),
        StreamableUseDirectURL: bool = false,
        // Azure Storage
        AzureStorageAccountName: Option<String> = Some(String::new()),
        AzureStorageAccountAccessKey: Secret = Secret::new(""),
        AzureStorageContainer: Option<String> = Some(String::new()),
        AzureStorageEnvironment: Option<String> = Some("blob.core.windows.net".into()),
        AzureStorageCustomDomain: Option<String> = Some(String::new()),
        AzureStorageUploadPath: Option<String> = Some(String::new()),
        AzureStorageCacheControl: Option<String> = Some(String::new()),
        // Backblaze B2
        B2ApplicationKeyId: Option<String> = Some(String::new()),
        B2ApplicationKey: Secret = Secret::new(""),
        B2BucketName: Option<String> = Some(String::new()),
        B2UploadPath: Option<String> = Some("ShareX/%y/%mo".into()),
        B2UseCustomUrl: bool = false,
        B2CustomUrl: Option<String> = Some("https://example.com".into()),
        // Plik
        PlikSettings: PlikSettings = PlikSettings::default(),
        // YouTube
        YouTubeOAuth2Info: Option<OAuth2Info> = None,
        YouTubeUserInfo: Option<OAuthUserInfo> = None,
        YouTubePrivacyType: YouTubeVideoPrivacy = YouTubeVideoPrivacy::Public,
        YouTubeUseShortenedLink: bool = false,
        YouTubeShowDialog: bool = false,
        // Google Cloud Storage
        GoogleCloudStorageOAuth2Info: Option<OAuth2Info> = None,
        GoogleCloudStorageUserInfo: Option<OAuthUserInfo> = None,
        GoogleCloudStorageBucket: Option<String> = Some(String::new()),
        GoogleCloudStorageDomain: Option<String> = Some(String::new()),
        GoogleCloudStorageObjectPrefix: Option<String> = Some("ShareX/%y/%mo".into()),
        GoogleCloudStorageRemoveExtensionImage: bool = false,
        GoogleCloudStorageRemoveExtensionVideo: bool = false,
        GoogleCloudStorageRemoveExtensionText: bool = false,
        GoogleCloudStorageSetPublicACL: bool = true,
        // Immich
        ImmichURL: Option<String> = Some(String::new()),
        ImmichAPIKey: Secret = Secret::new(""),
        ImmichAutoCreateShareableLink: bool = true,
        ImmichShowMetadata: bool = false,
        // bit.ly
        BitlyOAuth2Info: Option<OAuth2Info> = None,
        BitlyDomain: Option<String> = Some(String::new()),
        // yourls.org
        YourlsAPIURL: Option<String> = Some("http://yoursite.com/yourls-api.php".into()),
        YourlsSignature: Secret = Secret::new(""),
        YourlsUsername: Option<String> = Some(String::new()),
        YourlsPassword: Secret = Secret::new(""),
        // polr
        PolrAPIHostname: Option<String> = Some(String::new()),
        PolrAPIKey: Secret = Secret::new(""),
        PolrIsSecret: bool = false,
        PolrUseAPIv1: bool = false,
        // Firebase Dynamic Links
        FirebaseWebAPIKey: Secret = Secret::new(""),
        FirebaseDynamicLinkDomain: Option<String> = Some(String::new()),
        FirebaseIsShort: bool = false,
        // Kutt
        KuttSettings: KuttSettings = KuttSettings::default(),
        // Zero Width Shortener
        ZeroWidthShortenerURL: Option<String> = Some("https://api.zws.im".into()),
        ZeroWidthShortenerToken: Option<String> = Some(String::new()),
        // Custom uploaders
        CustomUploadersList: CustomUploaderList = CustomUploaderList::default(),
        CustomImageUploaderSelected: i32 = 0,
        CustomTextUploaderSelected: i32 = 0,
        CustomFileUploaderSelected: i32 = 0,
        CustomURLShortenerSelected: i32 = 0,
        CustomURLSharingServiceSelected: i32 = 0,
        // SettingsBase
        ApplicationVersion: Option<String> = None,
    }
}

impl SettingsFile for UploadersConfig {
    fn application_version(&self) -> Option<&str> {
        self.ApplicationVersion.as_deref()
    }

    fn set_application_version(&mut self, version: String) {
        self.ApplicationVersion = Some(version);
    }
}

impl UploadersConfig {
    /// `SettingManager.UploadersConfigBackwardCompatibilityTasks`: every
    /// custom uploader through `CheckBackwardCompatibility` (failures
    /// ignored, as in ShareX).
    pub fn backward_compatibility_tasks(&mut self) {
        for item in &mut self.CustomUploadersList.0 {
            let _ = item.check_backward_compatibility();
        }
    }
}
