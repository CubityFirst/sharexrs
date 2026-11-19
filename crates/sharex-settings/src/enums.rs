//! The enums ShareX's settings use, named and ordered as in ShareX
//! (`ShareX/Enums.cs`, `ShareX.HelpersLib/Enums.cs`,
//! `ShareX.UploadersLib/Enums.cs`, `ShareX.ScreenCaptureLib/Enums.cs`, ...).
//! Their values matter: Newtonsoft reads numbers too.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{dotnet_enum, dotnet_enum_values, dotnet_flags};

dotnet_enum! {
    /// `ShareX.HotkeyType`: what a hotkey (or a tray click, or a workflow) does.
    pub enum HotkeyType {
        None,
        FileUpload, FolderUpload, ClipboardUpload, ClipboardUploadWithContentViewer, UploadText, UploadURL,
        DragDropUpload, ShortenURL, StopUploads,
        PrintScreen, ActiveWindow, CustomWindow, ActiveMonitor, RectangleRegion, RectangleTransparent,
        CustomRegion, LastRegion, ScrollingCapture, AutoCapture, StartAutoCapture, StopAutoCapture,
        ScreenRecorder, ScreenRecorderActiveWindow, ScreenRecorderCustomRegion, StartScreenRecorder,
        ScreenRecorderGIF, ScreenRecorderGIFActiveWindow, ScreenRecorderGIFCustomRegion, StartScreenRecorderGIF,
        StopScreenRecording, PauseScreenRecording, AbortScreenRecording,
        ColorPicker, ScreenColorPicker, Ruler, ClassicRuler, MouseHighlighter, PinToScreen, PinToScreenFromScreen,
        PinToScreenFromClipboard, PinToScreenFromFile, PinToScreenCloseAll, ImageEditor, PhotoEditor, PixelEditor,
        ImageBeautifier, ImageEffects, ImageViewer, BackgroundRemover, ImageComparer, IconConverter, ImageCombiner,
        ImageSplitter, ImageResizer, ImageConverter, ImageWatermark, ImageThumbnailer, VideoConverter, VideoTrimmer,
        VideoThumbnailer, AnalyzeImage, OCR, QRCode, QRCodeDecodeFromScreen, QRCodeScanRegion, HashCheck, Metadata,
        StripMetadata, IndexFolder, ClipboardViewer, BorderlessWindow, ActiveWindowBorderless, ActiveWindowTopMost,
        InspectWindow, NetworkMonitor, MonitorTest,
        DisableHotkeys, OpenMainWindow, OpenScreenshotsFolder, OpenHistory, OpenImageHistory, ToggleActionsToolbar,
        ToggleTrayMenu, ExitShareX,
        AnimatedGifTrimmer,
    }
}

impl HotkeyType {
    /// `GetLocalizedDescription()`: the `HotkeyType_<name>` string in
    /// ShareX.HelpersLib's resources, or the name when there's none.
    pub fn localized_description(&self) -> String {
        sharex_strings::helpers_lib::lookup(&format!("HotkeyType_{}", self.name()))
            .map(str::to_owned)
            .unwrap_or_else(|| self.name().to_owned())
    }

    /// The `[Category]` of the value (`EnumExtensions.HotkeyType_Category_*`),
    /// none for `None`.
    pub fn category(&self) -> Option<&'static str> {
        use HotkeyType::{
            DisableHotkeys, ExitShareX, OpenHistory, OpenImageHistory, OpenMainWindow, OpenScreenshotsFolder, ToggleActionsToolbar,
            ToggleTrayMenu,
        };
        let v = self.value()?;
        Some(match v {
            0 => return Option::None,
            1..=9 => "Upload",
            10..=21 => "ScreenCapture",
            22..=32 => "ScreenRecord",
            _ if *self == DisableHotkeys
                || *self == OpenMainWindow
                || *self == OpenScreenshotsFolder
                || *self == OpenHistory
                || *self == OpenImageHistory
                || *self == ToggleActionsToolbar
                || *self == ToggleTrayMenu
                || *self == ExitShareX =>
            {
                "Other"
            }
            _ => "Tools",
        })
    }

    /// `EnumInfo.Category` (`GetLocalizedCategory`): the
    /// `HotkeyType_Category_<category>` string.
    pub fn localized_category(&self) -> Option<String> {
        let c = self.category()?;
        Some(localized_description("HotkeyType_Category", c).unwrap_or_else(|| c.to_owned()))
    }
}

dotnet_flags! {
    /// `ShareX.AfterCaptureTasks`.
    pub struct AfterCaptureTasks: u32 {
        const None = 0;
        const ShowQuickTaskMenu = 1;
        const ShowAfterCaptureWindow = 1 << 1;
        const BeautifyImage = 1 << 2;
        const AddImageEffects = 1 << 3;
        const AnnotateImage = 1 << 4;
        const OpenInPhotoEditor = 1 << 5;
        const CopyImageToClipboard = 1 << 6;
        const PinToScreen = 1 << 7;
        const SendImageToPrinter = 1 << 8;
        const SaveImageToFile = 1 << 9;
        const SaveImageToFileWithDialog = 1 << 10;
        const SaveThumbnailImageToFile = 1 << 11;
        const PerformActions = 1 << 12;
        const CopyFileToClipboard = 1 << 13;
        const CopyFilePathToClipboard = 1 << 14;
        const CopyFolderPathToClipboard = 1 << 15;
        const ShowInExplorer = 1 << 16;
        const AnalyzeImage = 1 << 17;
        const ScanQRCode = 1 << 18;
        const DoOCR = 1 << 19;
        const ShowBeforeUploadWindow = 1 << 20;
        const UploadImageToHost = 1 << 21;
        const DeleteFile = 1 << 22;
    }
}

dotnet_flags! {
    /// `ShareX.AfterUploadTasks`.
    pub struct AfterUploadTasks: u32 {
        const None = 0;
        const ShowAfterUploadWindow = 1;
        const UseURLShortener = 1 << 1;
        const ShareURL = 1 << 2;
        const CopyURLToClipboard = 1 << 3;
        const OpenURL = 1 << 4;
        const ShowQRCode = 1 << 5;
    }
}

dotnet_enum! {
    /// `ShareX.UpdateChannel`.
    pub enum UpdateChannel { Release, PreRelease }
}

dotnet_enum! {
    /// `ShareX.SupportedLanguage`.
    pub enum SupportedLanguage {
        Automatic, Arabic, Czech, Danish, Dutch, English, French, German, Hebrew, Hindi, Hungarian, Indonesian,
        Italian, Japanese, Korean, MexicanSpanish, Persian, Polish, Portuguese, PortugueseBrazil, Romanian, Russian,
        SimplifiedChinese, Spanish, Swedish, Thai, TraditionalChinese, Turkish, Ukrainian, Vietnamese,
    }
}

impl SupportedLanguage {
    /// `LanguageHelper.GetCultureName`: English (`en-US`) for Automatic too.
    pub fn culture_name(&self) -> &'static str {
        use SupportedLanguage::*;
        match self {
            Arabic => "ar-YE",
            Czech => "cs-CZ",
            Danish => "da-DK",
            Dutch => "nl-NL",
            French => "fr-FR",
            German => "de-DE",
            Hebrew => "he-IL",
            Hindi => "hi-IN",
            Hungarian => "hu-HU",
            Indonesian => "id-ID",
            Italian => "it-IT",
            Japanese => "ja-JP",
            Korean => "ko-KR",
            MexicanSpanish => "es-MX",
            Persian => "fa-IR",
            Polish => "pl-PL",
            Portuguese => "pt-PT",
            PortugueseBrazil => "pt-BR",
            Romanian => "ro-RO",
            Russian => "ru-RU",
            SimplifiedChinese => "zh-CN",
            Spanish => "es-ES",
            Swedish => "sv-SE",
            Thai => "th-TH",
            TraditionalChinese => "zh-TW",
            Turkish => "tr-TR",
            Ukrainian => "uk-UA",
            Vietnamese => "vi-VN",
            _ => "en-US",
        }
    }

    /// The `[Description]`s in `ShareX/Enums.cs`: each language in itself.
    fn description(&self) -> &str {
        use SupportedLanguage::*;
        match self {
            Arabic => "العربية (Arabic)",
            Czech => "Čeština (Czech)",
            Danish => "Dansk (Danish)",
            Dutch => "Nederlands (Dutch)",
            English => "English",
            French => "Français (French)",
            German => "Deutsch (German)",
            Hebrew => "עִברִית (Hebrew)",
            Hindi => "हिन्दी (Hindi)",
            Hungarian => "Magyar (Hungarian)",
            Indonesian => "Bahasa Indonesia (Indonesian)",
            Italian => "Italiano (Italian)",
            Japanese => "日本語 (Japanese)",
            Korean => "한국어 (Korean)",
            MexicanSpanish => "Español mexicano (Mexican Spanish)",
            Persian => "فارسی (Persian)",
            Polish => "Polski (Polish)",
            Portuguese => "Português (Portuguese)",
            PortugueseBrazil => "Português-Brasil (Portuguese-Brazil)",
            Romanian => "Română (Romanian)",
            Russian => "Русский (Russian)",
            SimplifiedChinese => "简体中文 (Simplified Chinese)",
            Spanish => "Español (Spanish)",
            Swedish => "Svenska (Swedish)",
            Thai => "ไทย (Thai)",
            TraditionalChinese => "繁體中文 (Traditional Chinese)",
            Turkish => "Türkçe (Turkish)",
            Ukrainian => "Українська (Ukrainian)",
            Vietnamese => "Tiếng Việt (Vietnamese)",
            other => other.name(),
        }
    }

    /// `GetLocalizedDescription()`: "Automatic" is translated, the others
    /// name themselves.
    pub fn localized_description(&self) -> String {
        localized_description("SupportedLanguage", self.name()).unwrap_or_else(|| self.description().to_owned())
    }

    /// The flag `LanguageFlags` shows: the culture's country code
    /// (`us` for English), none for Automatic.
    pub fn flag_code(&self) -> Option<String> {
        (*self != SupportedLanguage::Automatic)
            .then(|| self.culture_name().split('-').nth(1).unwrap_or("us").to_ascii_lowercase())
    }
}

/// `EnumExtensions.GetLocalizedDescription`: the `<Type>_<Value>` string in
/// ShareX.HelpersLib's resources, if there's one.
pub fn localized_description(type_name: &str, value: &str) -> Option<String> {
    sharex_strings::helpers_lib::lookup(&format!("{type_name}_{value}")).filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Enums shown in settings lists by their localized description.
pub trait Described: Sized + Clone + PartialEq + 'static {
    const TYPE: &'static str;
    fn all() -> &'static [Self];
    fn variant_name(&self) -> &str;
    fn localized_description(&self) -> String {
        localized_description(Self::TYPE, self.variant_name()).unwrap_or_else(|| self.variant_name().to_owned())
    }
}

macro_rules! described {
    ($($t:ident),*) => {$(
        impl Described for $t {
            const TYPE: &'static str = stringify!($t);
            fn all() -> &'static [Self] { $t::ALL }
            fn variant_name(&self) -> &str { self.name() }
        }
    )*};
}
described!(
    UpdateChannel,
    TaskViewMode,
    ImagePreviewVisibility,
    ImagePreviewLocation,
    ThumbnailTitleLocation,
    ThumbnailViewClickAction,
    ProxyMethod,
    ToastClickAction,
    ToastWindowMonitor,
    FileExistAction,
    ImageContentAlignment,
    EImageFormat,
    PNGBitDepth,
    BMPBitDepth,
    GIFQuality,
    RegionCaptureAction,
    RegionCaptureMagnifierScrollAction
);

dotnet_enum! {
    /// `ShareX.HelpersLib.ProxyMethod`.
    pub enum ProxyMethod { None, Manual, Automatic }
}

dotnet_enum! {
    /// `ShareX.TaskViewMode`.
    pub enum TaskViewMode { ListView, ThumbnailView }
}

dotnet_enum! {
    /// `ShareX.ImagePreviewVisibility`.
    pub enum ImagePreviewVisibility { Show, Hide, Automatic }
}

dotnet_enum! {
    /// `ShareX.ImagePreviewLocation`.
    pub enum ImagePreviewLocation { Side, Bottom }
}

dotnet_enum! {
    /// `ShareX.ThumbnailTitleLocation`.
    pub enum ThumbnailTitleLocation { Top, Bottom }
}

dotnet_enum! {
    /// `ShareX.ThumbnailViewClickAction`.
    pub enum ThumbnailViewClickAction { Default, Select, OpenImageViewer, OpenFile, OpenFolder, OpenURL, EditImage }
}

dotnet_enum! {
    /// `ShareX.ToastClickAction`.
    pub enum ToastClickAction {
        CloseNotification, AnnotateImage, CopyImageToClipboard, CopyFile, CopyFilePath, CopyUrl, OpenFile,
        OpenFolder, OpenUrl, Upload, PinToScreen, DeleteFile,
    }
}

dotnet_enum! {
    /// `ShareX.ToastWindowMonitor`.
    pub enum ToastWindowMonitor { ActiveMonitor, PrimaryMonitor, SpecificMonitor }
}

dotnet_enum! {
    /// `ShareX.FileExistAction`.
    pub enum FileExistAction { Ask, Overwrite, UniqueName, Cancel }
}

dotnet_enum_values! {
    /// `ShareX.HelpersLib.ImageContentAlignment`.
    pub enum ImageContentAlignment {
        TopLeft = 1, TopCenter = 2, TopRight = 4,
        MiddleLeft = 16, MiddleCenter = 32, MiddleRight = 64,
        BottomLeft = 256, BottomCenter = 512, BottomRight = 1024,
    }
}

dotnet_enum! {
    /// `ShareX.HelpersLib.EImageFormat`.
    pub enum EImageFormat { PNG, JPEG, WEBP, GIF, BMP }
}

dotnet_enum! {
    /// `ShareX.HelpersLib.PNGBitDepth`.
    pub enum PNGBitDepth { Default, Automatic, Bit32, Bit24 }
}

dotnet_enum! {
    /// `ShareX.HelpersLib.BMPBitDepth`.
    pub enum BMPBitDepth { Automatic, Bit32, Bit24 }
}

dotnet_enum! {
    /// `ShareX.HelpersLib.GIFQuality`.
    pub enum GIFQuality { Default, Bit8, Bit4, Grayscale, Adaptive }
}

dotnet_enum! {
    /// `ShareX.Icons.IconPack`.
    pub enum IconPack { Modern, Legacy }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.RegionCaptureAction`.
    pub enum RegionCaptureAction {
        None, CancelCapture, RemoveShapeCancelCapture, RemoveShape, SwapToolType, CaptureFullscreen,
        CaptureActiveMonitor, CaptureLastRegion, CaptureSelection,
    }
}

dotnet_enum! {
    /// `ShareX.ScreenCaptureLib.RegionCaptureMagnifierScrollAction`.
    pub enum RegionCaptureMagnifierScrollAction { Resize, Zoom }
}

dotnet_enum! {
    /// `ShareX.ScreenRecordingLib.CameraCaptureResolution`.
    pub enum CameraCaptureResolution { Size640x480, Size1280x720, Size1920x1080 }
}

dotnet_enum! {
    /// `ShareX.ScreenRecordingLib.CameraOverlayPosition`.
    pub enum CameraOverlayPosition { TopLeft, TopRight, BottomLeft, BottomRight }
}

dotnet_enum! {
    /// `ShareX.ScreenRecordingLib.CameraOverlayShape`.
    pub enum CameraOverlayShape { Rectangle, Circle }
}

dotnet_enum! {
    /// `ShareX.UploadersLib.ImageDestination`.
    pub enum ImageDestination {
        Imgur, ImageShack, Flickr, Photobucket, Chevereto, Vgyme, CustomImageUploader, FileUploader,
    }
}

dotnet_enum! {
    /// `ShareX.UploadersLib.TextDestination`.
    pub enum TextDestination {
        Pastebin, Paste2, Slexy, Paste_ee, Gist, Upaste, Hastebin, OneTimeSecret, Pastie, PrivateBin,
        CustomTextUploader, FileUploader,
    }
}

dotnet_enum! {
    /// `ShareX.UploadersLib.FileDestination`.
    pub enum FileDestination {
        Dropbox, FTP, OneDrive, GoogleDrive, Puush, Box, Mega, AmazonS3, GoogleCloudStorage, AzureStorage,
        BackblazeB2, OwnCloud, Immich, MediaFire, Pushbullet, SendSpace, Localhostr, Lambda, ImgFish, Pomf, Uguu,
        Seafile, Streamable, Sul, Lithiio, Transfersh, Plik, YouTube, SharedFolder, Email, CustomFileUploader,
    }
}

dotnet_enum! {
    /// `ShareX.UploadersLib.UrlShortenerType`.
    pub enum UrlShortenerType {
        BITLY, ISGD, VGD, TINYURL, TURL, YOURLS, QRnet, VURL, TwoGP, Polr, FirebaseDynamicLinks, Kutt,
        ZeroWidthShortener, CustomURLShortener,
    }
}

dotnet_enum! {
    /// `ShareX.UploadersLib.URLSharingServices`.
    pub enum URLSharingServices {
        Email, Facebook, Reddit, Pinterest, Tumblr, LinkedIn, StumbleUpon, Delicious, VK, Pushbullet,
        GoogleImageSearch, BingVisualSearch, CustomURLSharingService,
    }
}

/// An enum from a library ShareXrs doesn't port as such (SkiaSharp's
/// `SKPngEncoderFilterFlags`, `SKJpegEncoderDownsample`), kept as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawEnum(pub serde_json::Value);

impl RawEnum {
    pub fn name(name: &str) -> Self {
        Self(serde_json::Value::String(name.to_owned()))
    }
}

impl Serialize for RawEnum {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for RawEnum {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(d)? {
            v @ (serde_json::Value::String(_) | serde_json::Value::Number(_)) => Ok(Self(v)),
            _ => Err(serde::de::Error::custom("not an enum value")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_by_name_number_and_unknown() {
        assert_eq!(serde_json::to_string(&HotkeyType::RectangleRegion).unwrap(), r#""RectangleRegion""#);
        assert_eq!(serde_json::from_str::<HotkeyType>(r#""rectangleregion""#).unwrap(), HotkeyType::RectangleRegion);
        assert_eq!(serde_json::from_str::<HotkeyType>("14").unwrap(), HotkeyType::RectangleRegion);
        assert_eq!(HotkeyType::AnimatedGifTrimmer.value(), Some(85));
        let unknown: HotkeyType = serde_json::from_str(r#""SomethingNew""#).unwrap();
        assert_eq!(unknown, HotkeyType::Unknown("SomethingNew".into()));
        assert_eq!(serde_json::to_string(&unknown).unwrap(), r#""SomethingNew""#);
        assert_eq!(serde_json::to_string(&ImageContentAlignment::BottomRight).unwrap(), r#""BottomRight""#);
        assert_eq!(serde_json::from_str::<ImageContentAlignment>("1024").unwrap(), ImageContentAlignment::BottomRight);
    }

    #[test]
    fn flags_like_dotnet() {
        let t = AfterCaptureTasks::CopyImageToClipboard | AfterCaptureTasks::SaveImageToFile;
        assert_eq!(serde_json::to_string(&t).unwrap(), r#""CopyImageToClipboard, SaveImageToFile""#);
        assert_eq!(serde_json::to_string(&AfterCaptureTasks::None).unwrap(), r#""None""#);
        assert_eq!(serde_json::from_str::<AfterCaptureTasks>(r#""SaveImageToFile,copyimagetoclipboard""#).unwrap(), t);
        assert!(serde_json::from_str::<AfterCaptureTasks>(r#""NoSuchTask""#).is_err());
        assert_eq!(serde_json::to_string(&AfterCaptureTasks(1 << 30)).unwrap(), "1073741824");
    }

    #[test]
    fn hotkey_categories() {
        assert_eq!(HotkeyType::None.category(), None);
        assert_eq!(HotkeyType::StopUploads.category(), Some("Upload"));
        assert_eq!(HotkeyType::PrintScreen.category(), Some("ScreenCapture"));
        assert_eq!(HotkeyType::AbortScreenRecording.category(), Some("ScreenRecord"));
        assert_eq!(HotkeyType::MonitorTest.category(), Some("Tools"));
        assert_eq!(HotkeyType::ExitShareX.category(), Some("Other"));
        assert_eq!(HotkeyType::AnimatedGifTrimmer.category(), Some("Tools"));
        assert_eq!(HotkeyType::ScreenRecorder.localized_category().as_deref(), Some("Screen record"));
    }

    #[test]
    fn localized_descriptions() {
        assert_eq!(HotkeyType::RectangleRegion.localized_description(), "Capture region");
        assert_eq!(HotkeyType::ScreenRecorder.localized_description(), "Start/Stop screen recording");
    }
}
