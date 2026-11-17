//! `ShareX/UploadManager.cs` and `WorkerTask`'s constructors: what the
//! Upload menu, the tray, hotkeys and the clipboard start. Each gives the
//! tasks to start; the app starts them (`TaskManager.Start`).
//!
//! The pickers and dialogs these open in ShareX (file and folder pickers,
//! the text and URL windows, the clipboard content viewer, the multi upload
//! confirmation) aren't ported: [`dialogs`] stands in for them, so the
//! menu entries call through it and the flows behind them take a path or a
//! text directly.

use std::path::{Path, PathBuf};

use image::RgbaImage;
use sharex_helpers::file_helpers::{check_extension, get_file_name_extension, sanitize_file_name};
use sharex_settings::{AfterCaptureTasks, ApplicationConfig, TaskSettings};
use sharex_strings::sharex as s;
use sharex_uploaders::factory;
use sharex_uploaders::url_helpers;

use super::worker::{Source, UploadTask};
use crate::capture_job::{self, ImageTask, Metadata};
use crate::tasks::{EDataType, TaskJob};

/// What new tasks read and change of the application settings.
pub struct NewTaskContext<'a> {
    /// `ApplicationState.Settings` (the name pattern's counter, the last
    /// upload folder, the multi upload warning).
    pub app: &'a mut ApplicationConfig,
    /// `AppPaths.ScreenshotsParentFolder`.
    pub screenshots_parent: PathBuf,
    /// `AppPaths.ImageEffectsFolder`.
    pub image_effects_folder: PathBuf,
}

/// What the clipboard held (`ClipboardHelpers.ContainsImage/Text/FileDropList`).
/// Nothing reads the clipboard yet ([`dialogs::clipboard_content`]).
#[cfg_attr(not(test), allow(dead_code))]
pub enum ClipboardContent {
    Image(RgbaImage),
    Text(String),
    Files(Vec<PathBuf>),
}

/// ShareX's pickers and dialogs for uploads, not ported yet: each logs and
/// gives nothing (the multi upload confirmation goes ahead), so the menu
/// entries do nothing until their windows are ported.
pub mod dialogs {
    use super::*;

    fn not_ported(what: &str) {
        eprintln!("{what} isn't ported yet.");
    }

    /// `FileDialogHelpers.OpenFiles("ShareX - File upload", multiselect, initialDirectory)`.
    pub fn open_files(_initial_directory: &Path) -> Vec<PathBuf> {
        not_ported("The file upload picker");
        Vec::new()
    }

    /// `FileHelpers.BrowseFolder("ShareX - Folder upload", initialDirectory)`.
    pub fn browse_folder(_initial_directory: &Path) -> Option<PathBuf> {
        not_ported("The folder upload picker");
        None
    }

    /// `TextUploadWindow.ShowAsync()`.
    pub fn text_upload_window() -> Option<String> {
        not_ported("The text upload window");
        None
    }

    /// `URLUploadWindow.ShowAsync(inputText)`.
    pub fn url_upload_window(_input: Option<&str>) -> Option<String> {
        not_ported("The upload from URL window");
        None
    }

    /// `ShortenURLWindow.ShowAsync(inputText)`.
    pub fn shorten_url_window(_input: Option<&str>) -> Option<String> {
        not_ported("The shorten URL window");
        None
    }

    /// `ClipboardHelpers.GetImage/GetText/GetFileDropList`, as
    /// `UploadManager.ClipboardUpload` reads them.
    pub fn clipboard_content() -> Option<ClipboardContent> {
        not_ported("Reading the clipboard for an upload");
        None
    }

    /// `ClipboardUploadWindowIntegration.Show`: the clipboard content viewer.
    pub fn clipboard_upload_window() {
        not_ported("The clipboard content viewer");
    }

    /// `MultiUploadConfirmationWindowIntegration.Show(count)`: (confirmed,
    /// don't show again).
    pub fn multi_upload_confirmation(count: usize) -> (bool, bool) {
        eprintln!("The multi upload confirmation window isn't ported yet; uploading {count} files.");
        (true, false)
    }

    /// `OpenDropWindow`: the drag and drop upload window.
    pub fn drop_window() {
        not_ported("The drag and drop upload window");
    }
}

/// `TaskHelpers.FindDataType`.
pub fn find_data_type(path: &str, task: &TaskSettings) -> EDataType {
    let advanced = task.AdvancedSettings.clone().unwrap_or_default();
    let has = |exts: &[String]| check_extension(path, &exts.iter().map(String::as_str).collect::<Vec<_>>());
    if has(&advanced.ImageExtensions) {
        EDataType::Image
    } else if has(&advanced.TextExtensions) {
        EDataType::Text
    } else {
        EDataType::File
    }
}

/// `TaskHelpers.GetFileName(taskSettings, extension)` (no metadata).
fn name_pattern_file_name(task: &TaskSettings, ctx: &mut NewTaskContext, extension: &str) -> String {
    let metadata = Metadata { image: RgbaImage::new(0, 0), window_title: None, process_name: None };
    let mut counter = ctx.app.NameParserAutoIncrementNumber;
    let name = capture_job::file_name(task, &mut counter, extension, &metadata);
    ctx.app.NameParserAutoIncrementNumber = counter;
    name
}

/// `WorkerTask.CreateFileUploaderTask`: `None` when the file can't be
/// opened (ShareX shows the error).
pub fn create_file_uploader_task(path: &Path, task: TaskSettings, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    let path_str = path.to_string_lossy();
    let data_type = find_data_type(&path_str, &task);
    let mut file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if task.UploadSettings.as_ref().is_some_and(|u| u.FileUploadUseNamePattern) {
        let ext = get_file_name_extension(&path_str);
        file_name = name_pattern_file_name(&task, ctx, &ext);
    }
    if task.AdvancedSettings.as_ref().is_some_and(|a| a.ProcessImagesDuringFileUpload) && data_type == EDataType::Image {
        eprintln!("Processing images during file upload isn't ported yet; uploading the file.");
    }
    // `LoadFileStream`.
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            return None;
        }
    };
    Some(UploadTask {
        job: TaskJob::FileUpload,
        data_type,
        file_name,
        file_path: Some(path.to_path_buf()),
        url: String::new(),
        task,
        source: Source::File(file),
    })
}

/// `WorkerTask.CreateImageUploaderTask`: a capture's image task.
pub fn create_image_uploader_task(image: ImageTask) -> UploadTask {
    UploadTask {
        job: TaskJob::Job,
        data_type: EDataType::Image,
        file_name: image.file_name.clone(),
        file_path: None,
        url: String::new(),
        task: image.task.clone(),
        source: Source::Image(Box::new(image)),
    }
}

/// `WorkerTask.CreateTextUploaderTask`.
pub fn create_text_uploader_task(text: String, task: TaskSettings, ctx: &mut NewTaskContext) -> UploadTask {
    let ext = task.AdvancedSettings.clone().unwrap_or_default().TextFileExtension;
    let file_name = name_pattern_file_name(&task, ctx, &ext);
    UploadTask { job: TaskJob::TextUpload, data_type: EDataType::Text, file_name, file_path: None, url: String::new(), task, source: Source::Text(text) }
}

/// `WorkerTask.CreateURLShortenerTask`.
pub fn create_url_shortener_task(url: String, task: TaskSettings) -> UploadTask {
    let service = factory::service_name("UrlShortenerType", task.URLShortenerDestination.name());
    let file_name = s::UploadTask_CreateURLShortenerTask_Shorten_URL___0__().replace("{0}", &service);
    UploadTask { job: TaskJob::ShortenURL, data_type: EDataType::URL, file_name, file_path: None, url, task, source: Source::Url }
}

/// `WorkerTask.CreateShareURLTask`.
pub fn create_share_url_task(url: String, task: TaskSettings) -> UploadTask {
    let service = factory::service_name("URLSharingServices", task.URLSharingServiceDestination.name());
    let file_name = s::UploadTask_CreateShareURLTask_Share_URL___0__().replace("{0}", &service);
    UploadTask { job: TaskJob::ShareURL, data_type: EDataType::URL, file_name, file_path: None, url, task, source: Source::Url }
}

/// `WorkerTask.CreateDownloadTask`: the file name from the URL (or the
/// name pattern); `None` without one.
pub fn create_download_task(url: &str, upload: bool, task: TaskSettings, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    let mut file_name = url_helpers::url_decode(url, 10);
    file_name = url_helpers::get_file_name(&file_name);
    file_name = sanitize_file_name(&file_name);
    if task.UploadSettings.as_ref().is_some_and(|u| u.FileUploadUseNamePattern) {
        let ext = get_file_name_extension(&file_name);
        file_name = name_pattern_file_name(&task, ctx, &ext);
    }
    if file_name.is_empty() {
        return None;
    }
    let data_type = find_data_type(&file_name, &task);
    Some(UploadTask {
        job: if upload { TaskJob::DownloadUpload } else { TaskJob::Download },
        data_type,
        file_name,
        file_path: None,
        url: url.to_owned(),
        task,
        source: Source::Url,
    })
}

/// `UploadManager.UploadFile(filePath)`: a file, or every file in a folder
/// and its sub folders.
pub fn upload_file(path: &Path, task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    if path.as_os_str().is_empty() {
        return Vec::new();
    }
    if path.is_file() {
        create_file_uploader_task(path, task.clone(), ctx).into_iter().collect()
    } else if path.is_dir() {
        let mut files = Vec::new();
        collect_files(path, &mut files);
        upload_files(&files, task, ctx)
    } else {
        Vec::new()
    }
}

/// `Directory.GetFiles(path, "*.*", SearchOption.AllDirectories)`.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    let (dirs, files): (Vec<_>, Vec<_>) = entries.into_iter().partition(|p| p.is_dir());
    out.extend(files);
    for d in dirs {
        collect_files(&d, out);
    }
}

/// `UploadManager.UploadFile(files)`: more than 10 files need confirming
/// (`ShowMultiUploadWarning`).
pub fn upload_files(files: &[PathBuf], task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    if files.is_empty() {
        return Vec::new();
    }
    if files.len() > 10 && ctx.app.ShowMultiUploadWarning {
        let (confirmed, dont_show_again) = dialogs::multi_upload_confirmation(files.len());
        ctx.app.ShowMultiUploadWarning = !dont_show_again;
        if !confirmed {
            return Vec::new();
        }
    }
    files.iter().flat_map(|f| upload_file(f, task, ctx)).collect()
}

/// `UploadManager.UploadFile(taskSettings)`: the file picker, from the last
/// upload folder (or the desktop).
pub fn upload_file_with_picker(task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    let files = dialogs::open_files(&initial_directory(ctx.app));
    if let Some(dir) = files.first().and_then(|f| f.parent()) {
        ctx.app.FileUploadDefaultDirectory = dir.to_string_lossy().into_owned();
    }
    upload_files(&files, task, ctx)
}

/// `UploadManager.UploadFolder`.
pub fn upload_folder_with_picker(task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    let Some(folder) = dialogs::browse_folder(&initial_directory(ctx.app)) else { return Vec::new() };
    ctx.app.FileUploadDefaultDirectory = folder.to_string_lossy().into_owned();
    upload_file(&folder, task, ctx)
}

fn initial_directory(app: &ApplicationConfig) -> PathBuf {
    let dir = PathBuf::from(&app.FileUploadDefaultDirectory);
    if !app.FileUploadDefaultDirectory.is_empty() && dir.is_dir() { dir } else { dirs::desktop_dir().unwrap_or_default() }
}

/// `UploadManager.ProcessImageUpload` + `RunImageTask`: the image through
/// the after capture tasks, or only uploaded unless
/// `ProcessImagesDuringClipboardUpload`. (The quick task menu and the after
/// capture window aren't ported.)
pub fn process_image_upload(image: RgbaImage, mut task: TaskSettings, ctx: &mut NewTaskContext) -> UploadTask {
    if !task.AdvancedSettings.as_ref().is_some_and(|a| a.ProcessImagesDuringClipboardUpload) {
        task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    }
    let metadata = Metadata { image, window_title: None, process_name: None };
    let image_task = ImageTask::new(metadata, task, ctx.app, &ctx.screenshots_parent.clone(), ctx.image_effects_folder.clone());
    create_image_uploader_task(image_task)
}

/// `UploadManager.ProcessTextUpload`: a URL is downloaded and uploaded,
/// shortened or shared when the task says so; anything else is uploaded
/// as text.
pub fn process_text_upload(text: &str, task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    if text.is_empty() {
        return Vec::new();
    }
    let url = text.trim();
    let upload = task.UploadSettings.clone().unwrap_or_default();
    if url_helpers::is_valid_url(url) {
        if upload.ClipboardUploadURLContents {
            return download_and_upload_file(url, task, ctx).into_iter().collect();
        }
        if upload.ClipboardUploadShortenURL {
            return vec![create_url_shortener_task(url.to_owned(), task.clone())];
        }
        if upload.ClipboardUploadShareURL {
            return vec![create_share_url_task(url.to_owned(), task.clone())];
        }
    }
    if upload.ClipboardUploadAutoIndexFolder && text.chars().count() <= 260 && Path::new(text).is_dir() {
        eprintln!("Index folder isn't ported yet; uploading the text.");
    }
    upload_text(text, task, true, ctx).into_iter().collect()
}

/// `UploadManager.UploadText`: with `allow_custom_text`, the task's
/// `TextCustom` (`%input` replaced by the text, HTML-encoded with
/// `TextCustomEncodeInput`).
pub fn upload_text(text: &str, task: &TaskSettings, allow_custom_text: bool, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    if text.is_empty() {
        return None;
    }
    let mut text = text.to_owned();
    if allow_custom_text {
        let advanced = task.AdvancedSettings.clone().unwrap_or_default();
        if !advanced.TextCustom.is_empty() {
            if advanced.TextCustomEncodeInput {
                text = html_encode(&text);
            }
            text = advanced.TextCustom.replace("%input", &text);
        }
    }
    Some(create_text_uploader_task(text, task.clone(), ctx))
}

/// `UploadManager.DownloadAndUploadFile`.
pub fn download_and_upload_file(url: &str, task: &TaskSettings, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    if url.is_empty() {
        return None;
    }
    create_download_task(url, true, task.clone(), ctx)
}

/// `UploadManager.ShortenURL`.
pub fn shorten_url(url: &str, task: &TaskSettings) -> Option<UploadTask> {
    (!url.is_empty()).then(|| create_url_shortener_task(url.to_owned(), task.clone()))
}

/// `UploadManager.ShareURL`.
pub fn share_url(url: &str, task: &TaskSettings) -> Option<UploadTask> {
    (!url.is_empty()).then(|| create_share_url_task(url.to_owned(), task.clone()))
}

/// `UploadManager.ClipboardUpload`: an image, text or files on the clipboard.
pub fn clipboard_upload(content: ClipboardContent, task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    match content {
        ClipboardContent::Image(image) => vec![process_image_upload(image, task.clone(), ctx)],
        ClipboardContent::Text(text) => process_text_upload(&text, task, ctx),
        ClipboardContent::Files(files) => upload_files(&files, task, ctx),
    }
}

/// `UploadManager.ClipboardUploadMainWindow`: the clipboard content viewer
/// first when `ShowClipboardContentViewer`.
pub fn clipboard_upload_main_window(task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    if ctx.app.ShowClipboardContentViewer {
        dialogs::clipboard_upload_window();
        return Vec::new();
    }
    dialogs::clipboard_content().map(|c| clipboard_upload(c, task, ctx)).unwrap_or_default()
}

/// `HttpUtility.HtmlEncode`: `<>&"'` and the Latin-1 supplement as entities.
pub fn html_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\u{A0}'..='\u{FF}' => out.push_str(&format!("&#{};", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_encoding_as_dotnet() {
        assert_eq!(html_encode("<a href=\"x\">'é' & ü</a>"), "&lt;a href=&quot;x&quot;&gt;&#39;&#233;&#39; &amp; &#252;&lt;/a&gt;");
    }

    #[test]
    fn data_types_by_extension() {
        let task = TaskSettings::default();
        assert_eq!(find_data_type("a.PNG", &task), EDataType::Image);
        assert_eq!(find_data_type("a.txt", &task), EDataType::Text);
        assert_eq!(find_data_type("a.zip", &task), EDataType::File);
    }

    #[test]
    fn download_tasks_are_named_from_the_url() {
        let mut app = ApplicationConfig::default();
        let mut ctx = NewTaskContext { app: &mut app, screenshots_parent: PathBuf::new(), image_effects_folder: PathBuf::new() };
        let t = create_download_task("https://example.com/a/My%20File.png?x=1", true, TaskSettings::default(), &mut ctx).unwrap();
        assert_eq!((t.file_name.as_str(), t.data_type, t.job), ("My File.png", EDataType::Image, TaskJob::DownloadUpload));
        assert!(create_download_task("https://example.com/", false, TaskSettings::default(), &mut ctx).is_none());
    }

    #[test]
    fn clipboard_contents_make_their_tasks() {
        let dir = std::env::temp_dir().join(format!("sharexrs-clipboard-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.zip");
        std::fs::write(&file, b"made up").unwrap();
        let mut app = ApplicationConfig::default();
        let mut ctx = NewTaskContext { app: &mut app, screenshots_parent: dir.clone(), image_effects_folder: dir.clone() };
        let task = TaskSettings::default();
        let image = clipboard_upload(ClipboardContent::Image(RgbaImage::new(2, 2)), &task, &mut ctx);
        // `ProcessImageUpload`: only uploaded, unless images are processed.
        assert_eq!((image[0].job, image[0].task.AfterCaptureJob), (TaskJob::Job, AfterCaptureTasks::UploadImageToHost));
        let text = clipboard_upload(ClipboardContent::Text("made up".into()), &task, &mut ctx);
        assert_eq!(text[0].job, TaskJob::TextUpload);
        let files = clipboard_upload(ClipboardContent::Files(vec![file.clone(), dir.join("missing.zip")]), &task, &mut ctx);
        assert_eq!(files.len(), 1);
        assert_eq!((files[0].job, files[0].data_type), (TaskJob::FileUpload, EDataType::File));
        drop(files);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn urls_on_the_clipboard_follow_the_task() {
        let mut app = ApplicationConfig::default();
        let mut ctx = NewTaskContext { app: &mut app, screenshots_parent: PathBuf::new(), image_effects_folder: PathBuf::new() };
        let mut task = TaskSettings::default();
        task.UploadSettings.get_or_insert_default().ClipboardUploadShortenURL = true;
        let tasks = process_text_upload(" https://example.com/long ", &task, &mut ctx);
        assert_eq!((tasks[0].job, tasks[0].url.as_str()), (TaskJob::ShortenURL, "https://example.com/long"));
        let tasks = process_text_upload("just text", &task, &mut ctx);
        assert_eq!(tasks[0].job, TaskJob::TextUpload);
    }
}
