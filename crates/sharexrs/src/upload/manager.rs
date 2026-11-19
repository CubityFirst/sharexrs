//! `ShareX/UploadManager.cs` and `WorkerTask`'s constructors: what the
//! Upload menu, the tray, hotkeys and the clipboard start. Each gives the
//! tasks to start; the app starts them (`TaskManager.Start`).
//!
//! The windows these open in ShareX (the text and URL windows, the
//! clipboard content viewer, the multi upload confirmation) are
//! `crate::dialogs`; the app opens them and calls back in here with what
//! they give.

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
    /// More than 10 files waiting for `MultiUploadConfirmationWindow`
    /// (`IsUploadConfirmed`): the app asks, then uploads them confirmed.
    pub confirm_files: Option<Vec<PathBuf>>,
    /// The files given were confirmed already.
    pub files_confirmed: bool,
}

impl<'a> NewTaskContext<'a> {
    pub fn new(app: &'a mut ApplicationConfig, screenshots_parent: PathBuf, image_effects_folder: PathBuf) -> Self {
        Self { app, screenshots_parent, image_effects_folder, confirm_files: None, files_confirmed: false }
    }
}

/// What the clipboard held (`ClipboardHelpers.ContainsImage/Text/FileDropList`),
/// as `crate::dialogs::clipboard_upload::read_clipboard` reads it.
pub enum ClipboardContent {
    Image(RgbaImage),
    Text(String),
    Files(Vec<PathBuf>),
}

/// The file and folder pickers (`FileDialogHelpers.OpenFiles`,
/// `FileHelpers.BrowseFolder`): the system's, through rfd.
pub mod pickers {
    use super::*;

    /// `FileDialogHelpers.OpenFiles("ShareX - File upload", multiselect: true, initialDirectory)`.
    pub fn open_files(initial_directory: &Path) -> Vec<PathBuf> {
        let title = format!("ShareXrs - {}", s::UploadManager_UploadFile_File_upload());
        rfd::FileDialog::new().set_title(title).set_directory(initial_directory).pick_files().unwrap_or_default()
    }

    /// `FileHelpers.BrowseFolder("ShareX - Folder upload", initialDirectory)`.
    pub fn browse_folder(initial_directory: &Path) -> Option<PathBuf> {
        let title = format!("ShareXrs - {}", s::UploadManager_UploadFolder_Folder_upload());
        rfd::FileDialog::new().set_title(title).set_directory(initial_directory).pick_folder()
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
        // `task.Info.Job = TaskJob.Job; task.Image = SkiaImageHelpers.LoadImage(...)`:
        // the image goes through the after capture tasks like a capture,
        // under the file's name (or the name pattern's), its path the
        // file's until the image is saved elsewhere.
        let Some(bmp) = sharex_tools::imaging::load_image(path) else {
            // ShareX's task has no image then, and nothing to upload.
            eprintln!("{}: not an image ShareXrs can load.", path.display());
            return None;
        };
        let image = RgbaImage::from_raw(bmp.width(), bmp.height(), bmp.to_straight()).unwrap_or_default();
        let metadata = Metadata { image, window_title: None, process_name: None };
        // No name from the pattern here (the counter stays as it is).
        let counter = ctx.app.NameParserAutoIncrementNumber;
        let mut image_task = ImageTask::new(metadata, task, ctx.app, &ctx.screenshots_parent.clone(), ctx.image_effects_folder.clone());
        ctx.app.NameParserAutoIncrementNumber = counter;
        image_task.file_name = file_name;
        let mut upload = create_image_uploader_task(image_task);
        upload.file_path = Some(path.to_path_buf());
        return Some(upload);
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

/// `WorkerTask.CreateFileJobTask`: a file a capture made (a screen
/// recording) going through the after capture tasks; the file is opened
/// only when the task uploads (`LoadFileStream`), `None` when that fails.
pub fn create_file_job_task(path: &Path, task: TaskSettings, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    let path_str = path.to_string_lossy();
    let data_type = find_data_type(&path_str, &task);
    let mut file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if task.UploadSettings.as_ref().is_some_and(|u| u.FileUploadUseNamePattern) {
        let ext = get_file_name_extension(&path_str);
        file_name = name_pattern_file_name(&task, ctx, &ext);
    }
    let source = if task.AfterCaptureJob.contains(AfterCaptureTasks::UploadImageToHost) {
        match std::fs::File::open(path) {
            Ok(f) => Source::File(f),
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                return None;
            }
        }
    } else {
        Source::None
    };
    Some(UploadTask { job: TaskJob::Job, data_type, file_name, file_path: Some(path.to_path_buf()), url: String::new(), task, source })
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

/// `UploadManager.IndexFolder(folderPath, taskSettings)`: the folder
/// indexed with the task's `IndexerSettings` (and the application's
/// `BinaryUnits`) and uploaded as text named with the output's extension.
/// ShareX indexes on a thread pool task; this indexes before returning.
pub fn index_folder(folder: &str, task: &TaskSettings, ctx: &mut NewTaskContext) -> Option<UploadTask> {
    if folder.is_empty() || !Path::new(folder).is_dir() {
        return None;
    }
    let tools = task.ToolsSettings.clone().unwrap_or_default();
    let settings = sharex_tools::indexer::IndexerSettings::from_json(&tools.IndexerSettings, ctx.app.BinaryUnits);
    let source = sharex_tools::indexer::index(folder, &settings);
    if source.is_empty() {
        return None;
    }
    Some(index_upload_task(source, settings.output, task.clone(), ctx))
}

/// `WorkerTask.CreateTextUploaderTask(source, taskSettings)` with
/// `Path.ChangeExtension(FileName, output)` (`OpenDirectoryIndexer`'s upload).
pub fn index_upload_task(source: String, output: sharex_tools::indexer::IndexerOutput, task: TaskSettings, ctx: &mut NewTaskContext) -> UploadTask {
    let mut t = create_text_uploader_task(source, task, ctx);
    t.file_name = change_extension(&t.file_name, output.extension());
    t
}

/// `Path.ChangeExtension(path, extension)`.
pub fn change_extension(path: &str, extension: &str) -> String {
    let name_start = path.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let stem = match path[name_start..].rfind('.') {
        Some(dot) => &path[..name_start + dot],
        None => path,
    };
    format!("{stem}.{}", extension.trim_start_matches('.'))
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
    if files.len() > 10 && ctx.app.ShowMultiUploadWarning && !std::mem::take(&mut ctx.files_confirmed) {
        // The window answers later; the first files to ask wait for it.
        ctx.confirm_files.get_or_insert_with(|| files.to_vec());
        return Vec::new();
    }
    ctx.files_confirmed = false;
    files.iter().flat_map(|f| upload_file(f, task, ctx)).collect()
}

/// `UploadManager.UploadFile(taskSettings)`: the file picker, from the last
/// upload folder (or the desktop).
pub fn upload_file_with_picker(task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    let files = pickers::open_files(&initial_directory(ctx.app));
    if let Some(dir) = files.first().and_then(|f| f.parent()) {
        ctx.app.FileUploadDefaultDirectory = dir.to_string_lossy().into_owned();
    }
    upload_files(&files, task, ctx)
}

/// `UploadManager.UploadFolder`.
pub fn upload_folder_with_picker(task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    let Some(folder) = pickers::browse_folder(&initial_directory(ctx.app)) else { return Vec::new() };
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
        return index_folder(text, task, ctx).into_iter().collect();
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

/// `UploadManager.ClipboardUpload`: an image, text or files on the clipboard
/// (the app goes through `crate::dialogs::UploadRequest::clipboard`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn clipboard_upload(content: ClipboardContent, task: &TaskSettings, ctx: &mut NewTaskContext) -> Vec<UploadTask> {
    match content {
        ClipboardContent::Image(image) => vec![process_image_upload(image, task.clone(), ctx)],
        ClipboardContent::Text(text) => process_text_upload(&text, task, ctx),
        ClipboardContent::Files(files) => upload_files(&files, task, ctx),
    }
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
        let mut ctx = NewTaskContext::new(&mut app, PathBuf::new(), PathBuf::new());
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
        let mut ctx = NewTaskContext::new(&mut app, dir.clone(), dir.clone());
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
        let mut ctx = NewTaskContext::new(&mut app, PathBuf::new(), PathBuf::new());
        let mut task = TaskSettings::default();
        task.UploadSettings.get_or_insert_default().ClipboardUploadShortenURL = true;
        let tasks = process_text_upload(" https://example.com/long ", &task, &mut ctx);
        assert_eq!((tasks[0].job, tasks[0].url.as_str()), (TaskJob::ShortenURL, "https://example.com/long"));
        let tasks = process_text_upload("just text", &task, &mut ctx);
        assert_eq!(tasks[0].job, TaskJob::TextUpload);
    }
}
