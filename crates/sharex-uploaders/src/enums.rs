//! `ShareX.UploadersLib/Enums.cs` and the enums declared beside their
//! uploaders. The destination enums live in `sharex-settings` (task
//! settings use them) and are re-exported here.

use sharex_settings::{dotnet_enum, dotnet_flags};

pub use sharex_settings::{FileDestination, ImageDestination, TextDestination, URLSharingServices, UrlShortenerType};

dotnet_enum! {
    /// `HttpMethod` (ShareX's, not System.Net.Http's).
    pub enum HttpMethod { GET, POST, PUT, PATCH, DELETE }
}

dotnet_enum! {
    /// `CustomUploaderBody`.
    pub enum CustomUploaderBody { None, MultipartFormData, FormURLEncoded, JSON, XML, Binary }
}

dotnet_flags! {
    /// `CustomUploaderDestinationType` (`[Flags]`).
    pub struct CustomUploaderDestinationType: u32 {
        const None = 0;
        const ImageUploader = 1;
        const TextUploader = 1 << 1;
        const FileUploader = 1 << 2;
        const URLShortener = 1 << 3;
        const URLSharingService = 1 << 4;
    }
}

dotnet_enum! {
    /// `AccountType`.
    pub enum AccountType { Anonymous, User }
}

dotnet_enum! {
    /// `Privacy`.
    pub enum Privacy { Public, Private }
}

dotnet_enum! {
    /// `FTPProtocol`.
    pub enum FTPProtocol { FTP, FTPS, SFTP }
}

dotnet_enum! {
    /// `FTPSEncryption`.
    pub enum FTPSEncryption { Explicit, Implicit }
}

dotnet_enum! {
    /// `BrowserProtocol`.
    pub enum BrowserProtocol { http, https, ftp, ftps, file, sftp }
}

dotnet_enum! {
    /// `LinkFormatEnum`.
    pub enum LinkFormatEnum {
        URL, ForumImage, HTMLImage, WikiImage, ShortenedURL, ForumLinkedImage, HTMLLinkedImage, WikiLinkedImage,
        ThumbnailURL, LocalFilePath, LocalFilePathUri,
    }
}

dotnet_enum! {
    /// `OAuthLoginStatus`.
    pub enum OAuthLoginStatus { LoginRequired, LoginSuccessful, LoginFailed }
}

dotnet_enum! {
    /// `YouTubeVideoPrivacy`.
    pub enum YouTubeVideoPrivacy { Public, Unlisted, Private }
}

dotnet_enum! {
    /// `BoxShareAccessLevel`.
    pub enum BoxShareAccessLevel { Open, Company, Collaborators }
}

dotnet_enum! {
    /// `ImgurThumbnailType` (ImageUploaders/Imgur.cs).
    pub enum ImgurThumbnailType { Small_Square, Big_Square, Small_Thumbnail, Medium_Thumbnail, Large_Thumbnail, Huge_Thumbnail }
}

dotnet_enum! {
    /// `PastebinPrivacy` (TextUploaders/Pastebin.cs).
    pub enum PastebinPrivacy { Public, Unlisted, Private }
}

dotnet_enum! {
    /// `PastebinExpiration` (TextUploaders/Pastebin.cs).
    pub enum PastebinExpiration { N, M10, H1, D1, W1, W2, M1 }
}

dotnet_enum! {
    /// `AmazonS3StorageClass` (FileUploaders/AmazonS3.cs).
    pub enum AmazonS3StorageClass { STANDARD, STANDARD_IA, ONEZONE_IA, INTELLIGENT_TIERING }
}
