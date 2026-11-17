//! Upload tasks end to end, headless: each test starts
//! `tools/uploaders-harness/mock_server.py` on 127.0.0.1 (a free port),
//! points a custom uploader (or, through `http::set_url_rewriter`, a
//! built-in destination) at it, runs the whole task on its own thread as
//! the app does (after capture tasks → upload → after upload tasks),
//! applies its events to a `TaskManager` as the event loop does, and checks
//! the requests the mock received, the task's events and final state, and
//! what it would have copied or opened (`RecordingUi`).
//!
//! Nothing leaves the machine; every key, account and URL is made up.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sharex_settings::{AfterCaptureTasks, AfterUploadTasks, ApplicationConfig, FileDestination, ImageDestination, TaskSettings, TextDestination};
use sharex_settings::{AppPaths, SettingManager, SystemOptions, UrlShortenerType, URLSharingServices};
use sharex_strings::sharex as s;
use sharex_uploaders::UploadersConfig;
use sharex_uploaders::custom::CustomUploaderItem;
use sharex_uploaders::http;

use super::manager::{self, NewTaskContext};
use super::worker::{self, UploadContext, UploadTask};
use super::{StopHandle, TaskUi, UploadersConfigStore};
use crate::capture_job::{ImageTask, Metadata};
use crate::tasks::{TaskEvent, TaskJob, TaskManager, TaskReport, TaskStatus};

// ---------------------------------------------------------------------------
// The mock server.

struct Mock {
    child: Child,
    port: u16,
    _stdout: BufReader<ChildStdout>,
}

impl Mock {
    /// `python mock_server.py 0`: its first line says the port it got.
    fn start() -> Mock {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/uploaders-harness/mock_server.py");
        let candidates: &[&str] = if cfg!(windows) { &["python", "python3", "py"] } else { &["python3", "python"] };
        for exe in candidates {
            let Ok(mut child) = Command::new(exe).arg(&script).arg("0").stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
                continue;
            };
            let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok()
                && let Some(port) = line.trim().rsplit(':').next().and_then(|p| p.parse().ok())
            {
                return Mock { child, port, _stdout: reader };
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        panic!("couldn't start tools/uploaders-harness/mock_server.py: Python 3 is needed (python3 or python on PATH)");
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn control(&self, path: &str, body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("mock server");
        let method = if body.is_empty() { "GET" } else { "POST" };
        write!(
            s,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write");
        let mut out = String::new();
        s.read_to_string(&mut out).expect("read");
        out.split_once("\r\n\r\n").map(|(_, b)| b.to_owned()).unwrap_or_default()
    }

    /// Queues the responses (and the server's options).
    fn reset(&self, spec: Value) {
        self.control("/__mock/reset", &spec.to_string());
    }

    /// The requests since the last reset.
    fn log(&self) -> Vec<Request> {
        let log: Vec<Value> = serde_json::from_str(&self.control("/__mock/log", "")).expect("mock log");
        log.iter().map(Request::from_json).collect()
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A request as the mock received it.
#[derive(Debug)]
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn from_json(v: &Value) -> Self {
        use base64::Engine;
        Request {
            method: v["method"].as_str().unwrap_or_default().to_owned(),
            target: v["target"].as_str().unwrap_or_default().to_owned(),
            headers: v["headers"]
                .as_array()
                .map(|a| a.iter().map(|h| (h[0].as_str().unwrap_or_default().to_owned(), h[1].as_str().unwrap_or_default().to_owned())).collect())
                .unwrap_or_default(),
            body: base64::engine::general_purpose::STANDARD.decode(v["body_base64"].as_str().unwrap_or_default()).unwrap_or_default(),
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn ok(body: &str) -> Value {
    json!({ "status": 200, "reason": "OK", "headers": [["Content-Type", "application/json"]], "body": body })
}

fn text_ok(body: &str) -> Value {
    json!({ "status": 200, "reason": "OK", "headers": [["Content-Type", "text/plain"]], "body": body })
}

fn server_error() -> Value {
    json!({ "status": 500, "reason": "Internal Server Error", "headers": [["Content-Type", "text/plain"]], "body": "made-up failure" })
}

// ---------------------------------------------------------------------------
// What the task did on the "desktop".

#[derive(Debug, Clone, PartialEq)]
enum Ui {
    Copy(String),
    Clear,
    Open(String),
    QrCode(String),
}

#[derive(Default)]
struct RecordingUi {
    actions: Mutex<Vec<Ui>>,
    clipboard: Mutex<Option<String>>,
}

impl TaskUi for RecordingUi {
    fn copy_text(&self, text: &str) {
        self.actions.lock().unwrap().push(Ui::Copy(text.to_owned()));
        *self.clipboard.lock().unwrap() = Some(text.to_owned());
    }

    fn clipboard_contains_text(&self) -> bool {
        self.clipboard.lock().unwrap().is_some()
    }

    fn clear_clipboard(&self) {
        self.actions.lock().unwrap().push(Ui::Clear);
        *self.clipboard.lock().unwrap() = None;
    }

    fn open_url(&self, url: &str) {
        self.actions.lock().unwrap().push(Ui::Open(url.to_owned()));
    }

    fn show_qr_code(&self, text: &str) {
        self.actions.lock().unwrap().push(Ui::QrCode(text.to_owned()));
    }
}

// ---------------------------------------------------------------------------
// Running a task as the app does.

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("sharexrs-upload-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("temp dir");
        TempDir(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Options {
    app: Option<ApplicationConfig>,
    request_timeout: Option<Duration>,
    /// Stop the task at its first progress report.
    stop_at_progress: bool,
}

struct Outcome {
    id: u64,
    report: TaskReport,
    events: Vec<TaskEvent>,
    ui: Vec<Ui>,
    manager: TaskManager,
    config: UploadersConfigStore,
    elapsed: Duration,
}

impl Outcome {
    fn status(&self) -> TaskStatus {
        self.manager.get(self.id).expect("task").status
    }

    fn info(&self) -> &crate::tasks::TaskInfo {
        &self.manager.get(self.id).expect("task").info
    }

    fn progress(&self) -> Vec<(u64, u64)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Progress { progress, .. } => Some((progress.position, progress.length)),
                _ => None,
            })
            .collect()
    }

    fn statuses(&self) -> Vec<(TaskStatus, String)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Status { status, text, .. } => Some((*status, text.clone())),
                _ => None,
            })
            .collect()
    }
}

fn app_settings() -> ApplicationConfig {
    // `MaxUploadFailRetry` 1 and the rest as a fresh profile.
    ApplicationConfig::default()
}

/// `TaskManager.Start` + the task's thread + the event loop applying its
/// events, until `OnTaskCompleted`.
fn run(task: UploadTask, config: UploadersConfig, dir: &Path, options: Options) -> Outcome {
    let store = UploadersConfigStore::loaded(config);
    let ui = Arc::new(RecordingUi::default());
    let (tx, rx) = mpsc::channel::<TaskEvent>();
    let tx = Mutex::new(tx);
    let ctx = UploadContext {
        config: store.clone(),
        ui: ui.clone(),
        events: Arc::new(move |e| {
            let _ = tx.lock().unwrap().send(e);
        }),
        app: options.app.unwrap_or_else(app_settings),
        system_disable_upload: false,
        screenshots_parent: dir.to_path_buf(),
        owner: None,
        retry_delay: Duration::from_millis(100),
        request_timeout: options.request_timeout,
    };
    let mut manager = TaskManager::default();
    let stop = StopHandle::default();
    let id = manager.add_task(task.info(), stop.clone());
    manager.prepare(id);
    let started = Instant::now();
    let thread = {
        let (ctx, stop) = (ctx.clone(), stop.clone());
        std::thread::spawn(move || {
            let report = worker::run(task, id, &ctx, &stop);
            (ctx.events)(TaskEvent::Finished { id, report: Box::new(report) });
        })
    };
    drop(ctx);
    let mut events = Vec::new();
    let mut stopped = false;
    let report = loop {
        let e = rx.recv_timeout(Duration::from_secs(60)).expect("the task finishes");
        if options.stop_at_progress && !stopped && matches!(e, TaskEvent::Progress { .. }) {
            stopped = true;
            manager.stop(id, 10);
        }
        manager.apply_event(&e, false, 10);
        if let TaskEvent::Finished { report, .. } = &e {
            let report = (**report).clone();
            events.push(e);
            break report;
        }
        events.push(e);
    };
    thread.join().expect("task thread");
    let ui = ui.actions.lock().unwrap().clone();
    Outcome { id, report, events, ui, manager, config: store, elapsed: started.elapsed() }
}

fn custom(v: Value) -> CustomUploaderItem {
    CustomUploaderItem::from_value(v).expect("custom uploader")
}

fn config_with(items: Vec<CustomUploaderItem>) -> UploadersConfig {
    let mut c = UploadersConfig::default();
    c.CustomUploadersList.0 = items;
    c
}

/// An image host answering with JSON (`{json:...}`).
fn image_host(mock: &Mock) -> CustomUploaderItem {
    custom(json!({
        "Version": "18.0.0",
        "Name": "Mock image host",
        "DestinationType": "ImageUploader, FileUploader",
        "RequestMethod": "POST",
        "RequestURL": mock.url("/upload"),
        "Body": "MultipartFormData",
        "Arguments": { "key": "made-up-key" },
        "FileFormName": "file",
        "URL": "{json:data.link}",
        "ThumbnailURL": "{json:data.thumb}",
        "DeletionURL": "{json:data.delete}"
    }))
}

const IMAGE_RESPONSE: &str =
    r#"{"data":{"link":"https://i.example.com/abc.png","thumb":"https://i.example.com/abc_t.png","delete":"https://i.example.com/delete/abc"}}"#;

fn task_settings() -> TaskSettings {
    TaskSettings { AfterCaptureJob: AfterCaptureTasks::None, AfterUploadJob: AfterUploadTasks::None, ..Default::default() }
}

fn image_task(task: TaskSettings, dir: &Path) -> UploadTask {
    let image = image::RgbaImage::from_fn(40, 30, |x, y| image::Rgba([x as u8 * 6, y as u8 * 8, 90, 255]));
    let metadata = Metadata { image, window_title: Some("Made-up window".into()), process_name: Some("madeup".into()) };
    let mut app = app_settings();
    let job = ImageTask::new(metadata, task, &mut app, dir, dir.join("ImageEffects"));
    manager::create_image_uploader_task(job)
}

fn new_task_context<'a>(app: &'a mut ApplicationConfig, dir: &Path) -> NewTaskContext<'a> {
    NewTaskContext { app, screenshots_parent: dir.to_path_buf(), image_effects_folder: dir.join("ImageEffects") }
}

/// The part of a multipart body after `name="<form>"; filename="<name>"`'s
/// headers, up to the closing boundary.
fn multipart_file<'a>(body: &'a [u8], form: &str) -> (String, &'a [u8]) {
    let text = String::from_utf8_lossy(body);
    let marker = format!("name=\"{form}\"; filename=\"");
    let start = text.find(&marker).expect("file part") + marker.len();
    let name = text[start..text[start..].find('"').map(|e| start + e).unwrap()].to_owned();
    let head_end = body[start..].windows(4).position(|w| w == b"\r\n\r\n").unwrap() + start + 4;
    let end = body.windows(4).rposition(|w| w == b"\r\n--").unwrap();
    (name, &body[head_end..end])
}

// ---------------------------------------------------------------------------
// Flows.

#[test]
fn a_capture_is_uploaded_then_its_after_upload_tasks_run() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE)] }));
    let dir = TempDir::new("image");
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::SaveImageToFile | AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard | AfterUploadTasks::OpenURL | AfterUploadTasks::ShowQRCode;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    // `UploadInfoParser.ForumLinkedImage`.
    task.AdvancedSettings.get_or_insert_default().ClipboardContentFormat = "[url=$url][img]$thumbnailurl[/img][/url]".into();
    let out = run(image_task(task, &dir.0), config_with(vec![image_host(&mock)]), &dir.0, Options::default());

    // One request: the PNG the task saved, as multipart, with ShareXrs's user agent.
    let log = mock.log();
    assert_eq!(log.len(), 1, "{log:?}");
    let r = &log[0];
    assert_eq!((r.method.as_str(), r.target.as_str()), ("POST", "/upload"));
    assert_eq!(r.header("User-Agent"), Some(http::default_user_agent().as_str()));
    assert!(r.header("User-Agent").unwrap().starts_with("ShareXrs/"));
    assert!(r.body_text().contains("name=\"key\"\r\n\r\nmade-up-key\r\n"));
    let (name, png) = multipart_file(&r.body, "file");
    let saved = out.report.file_path.clone().expect("saved file");
    assert_eq!(saved.file_name().unwrap().to_string_lossy(), name);
    assert!(name.starts_with("madeup_") && name.ends_with(".png"), "{name}");
    assert_eq!(std::fs::read(&saved).unwrap(), png, "the upload is the saved file");
    assert!(r.body_text().contains("Content-Type: image/png"));

    // The task: preparing, uploading with progress, done.
    assert_eq!(out.statuses(), [(TaskStatus::Working, s::UploadTask_DoUploadJob_Uploading().to_owned())]);
    assert!(out.events.iter().any(|e| matches!(e, TaskEvent::UploadStarted { .. })));
    let (position, length) = *out.progress().last().expect("progress reported");
    assert_eq!((position, length), (png.len() as u64, png.len() as u64), "progress counts the file's bytes");
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().status, s::UploadTask_OnUploadCompleted_Done());
    assert_eq!(out.info().result.url, "https://i.example.com/abc.png");
    assert_eq!(out.info().result.thumbnail_url, "https://i.example.com/abc_t.png");
    assert_eq!(out.info().result.deletion_url, "https://i.example.com/delete/abc");
    assert_eq!(out.info().file_name, name);
    assert!(out.info().result.errors.is_empty());
    assert_eq!(out.manager.recent.last().map(|r| r.url.as_str()), Some("https://i.example.com/abc.png"));

    // The history item (`TaskInfo.GetHistoryItem`): the upload's URLs and
    // host as ShareX records them.
    let h = out.report.history.clone().expect("history item");
    assert_eq!(h.file_name.as_deref(), Some(name.as_str()));
    assert_eq!(h.file_path.as_deref(), Some(saved.to_string_lossy().as_ref()));
    assert_eq!(h.r#type.as_deref(), Some("Image"));
    assert_eq!(h.host.as_deref(), sharex_strings::helpers_lib::lookup("ImageDestination_CustomImageUploader"));
    assert_eq!(h.url.as_deref(), Some("https://i.example.com/abc.png"));
    assert_eq!(h.thumbnail_url.as_deref(), Some("https://i.example.com/abc_t.png"));
    assert_eq!(h.deletion_url.as_deref(), Some("https://i.example.com/delete/abc"));
    assert_eq!(h.shortened_url, None);
    assert!(h.is_valid());

    // Copy (with the clipboard format), open, QR code.
    assert_eq!(
        out.ui,
        [
            Ui::Copy("[url=https://i.example.com/abc.png][img]https://i.example.com/abc_t.png[/img][/url]".into()),
            Ui::Open("https://i.example.com/abc.png".into()),
            Ui::QrCode("https://i.example.com/abc.png".into()),
        ]
    );
}

#[test]
fn text_is_saved_uploaded_and_its_url_copied() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [text_ok("https://paste.example.com/raw/xyz")] }));
    let dir = TempDir::new("text");
    let paste = custom(json!({
        "Version": "18.0.0",
        "Name": "Mock paste",
        "DestinationType": "TextUploader",
        "RequestMethod": "POST",
        "RequestURL": mock.url("/paste"),
        "Body": "JSON",
        "Data": "{\"content\":\"{input}\",\"name\":\"{filename}\"}",
        "URL": "{response}"
    }));
    let mut task = task_settings();
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.TextDestination = TextDestination::CustomTextUploader;
    let mut app = app_settings();
    let text = "Made-up \"text\"\nwith ünïcode";
    let t = manager::upload_text(text, &task, false, &mut new_task_context(&mut app, &dir.0)).unwrap();
    assert_eq!(t.job, TaskJob::TextUpload);
    let file_name = t.file_name.clone();
    assert!(file_name.ends_with(".txt"));
    let out = run(t, config_with(vec![paste]), &dir.0, Options::default());

    let log = mock.log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].header("Content-Type"), Some("application/json"));
    let sent: Value = serde_json::from_slice(&log[0].body).unwrap();
    assert_eq!(sent, json!({ "content": text, "name": file_name }));
    // `TextTaskSaveAsFile`: UTF-8 with a byte order mark, in the screenshots folder.
    let saved = out.report.file_path.clone().unwrap();
    assert!(saved.starts_with(&dir.0));
    assert_eq!(std::fs::read(&saved).unwrap(), [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat());
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().result.url, "https://paste.example.com/raw/xyz");
    assert_eq!(out.ui, [Ui::Copy("https://paste.example.com/raw/xyz".into())]);
}

#[test]
fn a_file_upload_reports_progress_into_the_task_list() {
    let mock = Mock::start();
    // Read slowly: ~0.7 s for the file, so ProgressManager reports a few times.
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE)], "read_chunk": 131072, "read_delay_ms": 40 }));
    let dir = TempDir::new("file");
    let path = dir.0.join("made-up archive.zip");
    let data: Vec<u8> = (0..2_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    let mut task = task_settings();
    task.FileDestination = FileDestination::CustomFileUploader;
    let mut app = app_settings();
    let tasks = manager::upload_file(&path, &task, &mut new_task_context(&mut app, &dir.0));
    assert_eq!(tasks.len(), 1);
    let out = run(tasks.into_iter().next().unwrap(), config_with(vec![image_host(&mock)]), &dir.0, Options::default());

    let log = mock.log();
    let (name, body) = multipart_file(&log[0].body, "file");
    assert_eq!(name, "made-up archive.zip");
    assert_eq!(body, &data[..]);
    let progress = out.progress();
    // Several reports while the mock reads slowly, on Windows. Linux's
    // socket buffers can take the whole file in one write, so there the
    // upload may report just once; the rest still holds.
    let min_reports = if cfg!(windows) { 2 } else { 1 };
    assert!(progress.len() >= min_reports, "progress reported {} times", progress.len());
    assert!(progress.windows(2).all(|w| w[0].0 <= w[1].0), "progress goes forward: {progress:?}");
    let total = progress[0].1;
    assert_eq!(total, data.len() as u64, "progress counts the file's bytes");
    assert_eq!(progress.last().unwrap().0, total);
    // The list view's transfer columns keep the last report.
    let task = out.manager.get(out.id).unwrap();
    assert_eq!(task.transfer[0], format!("{0} / {0}", crate::tasks::size_string(total, false)));
    assert!(task.transfer[1].ends_with("/s"));
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().file_path.as_deref(), Some(path.as_path()));
    assert!(out.ui.is_empty(), "no after upload tasks: {:?}", out.ui);
}

#[test]
fn an_uploader_filter_picks_the_destination_by_extension() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE)] }));
    let dir = TempDir::new("filter");
    let path = dir.0.join("notes.md");
    std::fs::write(&path, "# made up").unwrap();
    let mut task = task_settings();
    // Dropbox isn't ported: only the filter's destination can upload this.
    task.FileDestination = FileDestination::Dropbox;
    task.UploadSettings.get_or_insert_default().UploaderFilters = vec![json!({ "Uploader": "customfileuploader", "Extensions": [".md"] })];
    let mut app = app_settings();
    let t = manager::upload_file(&path, &task, &mut new_task_context(&mut app, &dir.0)).pop().unwrap();
    let out = run(t, config_with(vec![image_host(&mock)]), &dir.0, Options::default());
    assert_eq!(mock.log().len(), 1);
    assert_eq!(out.status(), TaskStatus::Completed, "{:?}", out.info().result.errors);
    assert_eq!(out.info().result.url, "https://i.example.com/abc.png");
}

fn shortener(mock: &Mock) -> CustomUploaderItem {
    custom(json!({
        "Version": "18.0.0",
        "Name": "Mock shortener",
        "DestinationType": "URLShortener",
        "RequestMethod": "GET",
        "RequestURL": mock.url("/shorten"),
        "Parameters": { "url": "{input}" },
        "URL": "{response}"
    }))
}

#[test]
fn a_url_is_shortened_and_the_short_url_copied() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [text_ok("https://s.example.com/x1")] }));
    let dir = TempDir::new("shorten");
    let mut task = task_settings();
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.URLShortenerDestination = UrlShortenerType::CustomURLShortener;
    let long = "https://example.com/a/very/long/path?q=made up&x=1";
    let t = manager::shorten_url(long, &task).unwrap();
    assert_eq!(t.file_name, s::UploadTask_CreateURLShortenerTask_Shorten_URL___0__().replace("{0}", &sharex_uploaders::factory::service_name("UrlShortenerType", "CustomURLShortener")));
    let out = run(t, config_with(vec![shortener(&mock)]), &dir.0, Options::default());

    let log = mock.log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].method, "GET");
    let query = log[0].target.strip_prefix("/shorten?url=").expect("url parameter");
    assert_eq!(sharex_uploaders::url_helpers::url_decode(query, 1), long);
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().result.url, long);
    assert_eq!(out.info().result.shortened_url, "https://s.example.com/x1");
    assert_eq!(out.info().text(), "https://s.example.com/x1");
    assert_eq!(out.ui, [Ui::Copy("https://s.example.com/x1".into())]);
}

#[test]
fn an_upload_is_shortened_and_shared_after_upload() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE), text_ok("https://s.example.com/img"), text_ok("shared")] }));
    let dir = TempDir::new("shorten-share");
    let sharer = custom(json!({
        "Version": "18.0.0",
        "Name": "Mock sharer",
        "DestinationType": "URLSharingService",
        "RequestMethod": "POST",
        "RequestURL": mock.url("/share"),
        "Body": "FormURLEncoded",
        "Arguments": { "link": "{input}" }
    }));
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::UseURLShortener | AfterUploadTasks::ShareURL | AfterUploadTasks::CopyURLToClipboard;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    task.URLShortenerDestination = UrlShortenerType::CustomURLShortener;
    task.URLSharingServiceDestination = URLSharingServices::CustomURLSharingService;
    let mut config = config_with(vec![image_host(&mock), shortener(&mock), sharer]);
    config.CustomURLShortenerSelected = 1;
    config.CustomURLSharingServiceSelected = 2;
    let out = run(image_task(task, &dir.0), config, &dir.0, Options::default());

    let log = mock.log();
    let targets: Vec<&str> = log.iter().map(|r| r.target.split('?').next().unwrap()).collect();
    assert_eq!(targets, ["/upload", "/shorten", "/share"]);
    // `ShareURL(Info.Result.ToString())`: the short URL.
    assert_eq!(log[2].body_text(), "link=https%3a%2f%2fs.example.com%2fimg");
    assert_eq!(out.status(), TaskStatus::Completed);
    assert!(out.report.file_path.is_none(), "not saved");
    assert_eq!(out.info().result.shortened_url, "https://s.example.com/img");
    assert_eq!(out.ui, [Ui::Copy("https://s.example.com/img".into())]);
}

#[test]
fn a_url_is_downloaded_then_uploaded() {
    let mock = Mock::start();
    let head = json!({ "status": 200, "reason": "OK", "headers": [["Content-Disposition", "attachment; filename=\"made-up report.txt\""]], "body": "" });
    let file = json!({ "status": 200, "reason": "OK", "headers": [["Content-Type", "text/plain"]], "body": "made-up downloaded text" });
    mock.reset(json!({ "responses": [head, file, ok(IMAGE_RESPONSE)] }));
    let dir = TempDir::new("download");
    let mut task = task_settings();
    task.TextFileDestination = FileDestination::CustomFileUploader;
    task.TextDestination = TextDestination::FileUploader;
    let mut app = app_settings();
    let t = manager::download_and_upload_file(&mock.url("/files/r.txt"), &task, &mut new_task_context(&mut app, &dir.0)).unwrap();
    let out = run(t, config_with(vec![image_host(&mock)]), &dir.0, Options::default());

    let log = mock.log();
    let methods: Vec<(&str, &str)> = log.iter().map(|r| (r.method.as_str(), r.target.as_str())).collect();
    assert_eq!(methods, [("HEAD", "/files/r.txt"), ("GET", "/files/r.txt"), ("POST", "/upload")]);
    let (name, body) = multipart_file(&log[2].body, "file");
    assert_eq!((name.as_str(), body), ("made-up report.txt", &b"made-up downloaded text"[..]));
    let saved = out.report.file_path.clone().unwrap();
    assert_eq!(saved.file_name().unwrap(), "made-up report.txt");
    assert_eq!(std::fs::read(&saved).unwrap(), b"made-up downloaded text");
    assert!(out.statuses().iter().any(|(_, t)| t == s::UploadTask_DownloadAndUpload_Downloading()));
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().result.url, "https://i.example.com/abc.png");
}

// ---------------------------------------------------------------------------
// Failures, stopping and retrying.

fn failed_image_upload(mock: &Mock, options: Options) -> (Outcome, TempDir) {
    let dir = TempDir::new(&format!("fail-{}", mock.port));
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    let out = run(image_task(task, &dir.0), config_with(vec![image_host(mock)]), &dir.0, options);
    (out, dir)
}

fn assert_failed(out: &Outcome) {
    assert_eq!(out.status(), TaskStatus::Failed, "{:?}", out.info().result.errors);
    assert_eq!(out.info().status, s::TaskManager_task_UploadCompleted_Error());
    assert!(out.info().result.is_error());
    assert!(out.info().result.url.is_empty());
    assert!(out.ui.is_empty(), "nothing copied: {:?}", out.ui);
    // `WorkerTask_ErrorTitle` with the service's name.
    let service = sharex_uploaders::factory::service_name("ImageDestination", "CustomImageUploader");
    assert_eq!(out.info().result.error_title, s::WorkerTask_ErrorTitle().replace("{0}", &service));
}

#[test]
fn an_http_500_fails_the_task_after_its_retry() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [server_error(), server_error()] }));
    let (out, _dir) = failed_image_upload(&mock, Options::default());
    // `MaxUploadFailRetry` 1: two attempts.
    assert_eq!(mock.log().len(), 2);
    assert_failed(&out);
    let errors = &out.info().result.errors;
    assert_eq!(errors.len(), 1, "the last attempt's errors: {errors:?}");
    assert!(errors[0].contains("Response status code does not indicate success: 500 (Internal Server Error)."), "{}", errors[0]);
    assert!(errors[0].contains("made-up failure"));
}

#[test]
fn a_timeout_fails_the_task() {
    let mock = Mock::start();
    let slow = json!({ "status": 200, "reason": "OK", "body": IMAGE_RESPONSE, "delay_ms": 3000 });
    mock.reset(json!({ "responses": [slow.clone(), slow] }));
    let options = Options { request_timeout: Some(Duration::from_millis(300)), ..Default::default() };
    let (out, _dir) = failed_image_upload(&mock, options);
    assert_failed(&out);
    assert!(out.info().result.errors[0].contains("The request timed out after 00:00:00.3000000."), "{}", out.info().result.errors[0]);
    assert!(out.elapsed < Duration::from_secs(3), "{:?}", out.elapsed);
}

#[test]
fn a_refused_connection_fails_the_task() {
    let mock = Mock::start();
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let dir = TempDir::new("refused");
    let mut host = image_host(&mock);
    host.request_url = Some(format!("http://127.0.0.1:{closed}/upload"));
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    let out = run(image_task(task, &dir.0), config_with(vec![host]), &dir.0, Options::default());
    assert_failed(&out);
    let error = &out.info().result.errors[0];
    assert!(error.starts_with("Error message:"), "{error}");
    assert!(error.contains(&format!("http://127.0.0.1:{closed}/upload")), "{error}");
    assert!(mock.log().is_empty());
}

#[test]
fn missing_settings_fail_and_ask_for_destination_settings() {
    let dir = TempDir::new("invalid");
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    // No custom uploader at all.
    let out = run(image_task(task, &dir.0), UploadersConfig::default(), &dir.0, Options::default());
    assert_eq!(out.status(), TaskStatus::Failed);
    let service = sharex_uploaders::factory::service_name("ImageDestination", "CustomImageUploader");
    let message = s::WorkerTask_GetInvalidConfigResult__0__configuration_is_invalid_or_missing__Please_check__Destination_settings__window_to_configure_it_()
        .replace("{0}", &service);
    assert_eq!(out.info().result.errors, [message]);
    let requests = out.events.iter().filter(|e| matches!(e, TaskEvent::DestinationSettingsRequested { .. })).count();
    assert_eq!(requests, 2, "asked on each attempt");
}

#[test]
fn an_unported_destination_fails_saying_so() {
    let dir = TempDir::new("unported");
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.ImageDestination = ImageDestination::Flickr;
    let out = run(image_task(task, &dir.0), UploadersConfig::default(), &dir.0, Options::default());
    assert_eq!(out.status(), TaskStatus::Failed);
    assert_eq!(out.info().result.errors, ["Flickr isn't ported to ShareXrs yet."]);
}

#[test]
fn stopping_mid_upload_stops_the_task() {
    let mock = Mock::start();
    // ~6 s to read the file: the stop comes at the first progress report.
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE), ok(IMAGE_RESPONSE)], "read_chunk": 16384, "read_delay_ms": 50 }));
    let dir = TempDir::new("stop");
    let path = dir.0.join("big.bin");
    std::fs::write(&path, vec![3u8; 2_000_000]).unwrap();
    let mut task = task_settings();
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.FileDestination = FileDestination::CustomFileUploader;
    let mut app = app_settings();
    let t = manager::upload_file(&path, &task, &mut new_task_context(&mut app, &dir.0)).pop().unwrap();
    let out = run(t, config_with(vec![image_host(&mock)]), &dir.0, Options { stop_at_progress: true, ..Default::default() });
    assert!(out.elapsed < Duration::from_secs(4), "stopped after {:?}", out.elapsed);
    assert_eq!(out.status(), TaskStatus::Stopped);
    assert_eq!(out.info().status, s::UploadTask_OnUploadCompleted_Stopped());
    assert!(out.report.stop_requested);
    assert!(out.info().result.errors.is_empty(), "a stop isn't an error: {:?}", out.info().result.errors);
    assert!(out.ui.is_empty());
    // No retry after a stop.
    assert!(mock.log().len() <= 1);
}

#[test]
fn a_retry_after_a_failure_completes_the_task() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [server_error(), ok(IMAGE_RESPONSE)] }));
    let (out, _dir) = failed_image_upload(&mock, Options::default());
    assert_eq!(mock.log().len(), 2);
    assert_eq!(out.status(), TaskStatus::Completed);
    assert_eq!(out.info().result.url, "https://i.example.com/abc.png");
    // `Info.Result` is the retry's: the first attempt's error is gone.
    assert!(out.info().result.errors.is_empty());
    assert_eq!(out.ui, [Ui::Copy("https://i.example.com/abc.png".into())]);
}

#[test]
fn no_retry_when_max_upload_fail_retry_is_zero() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [server_error(), ok(IMAGE_RESPONSE)] }));
    let mut app = app_settings();
    app.MaxUploadFailRetry = 0;
    let (out, _dir) = failed_image_upload(&mock, Options { app: Some(app), ..Default::default() });
    assert_eq!(mock.log().len(), 1);
    assert_failed(&out);
}

// ---------------------------------------------------------------------------
// Built-in destinations, through the URL rewriter (one test at a time).

static REWRITER: Mutex<()> = Mutex::new(());

/// Sends every URL but 127.0.0.1's to the mock as
/// `/__/<scheme>/<host><path>`, as the harness does.
fn rewrite_to(mock: &Mock) -> std::sync::MutexGuard<'static, ()> {
    let guard = REWRITER.lock().unwrap_or_else(|e| e.into_inner());
    let base = mock.url("");
    let back_base = format!("{base}/__/");
    http::set_url_rewriter(Some(http::UrlRewriter {
        forward: Arc::new(move |u: &str| {
            if u.starts_with("http://127.0.0.1") {
                return u.to_owned();
            }
            let (scheme, rest) = u.split_once("://").unwrap_or(("https", u));
            format!("{base}/__/{scheme}/{rest}")
        }),
        back: Arc::new(move |u: &str| match u.strip_prefix(&back_base).and_then(|r| r.split_once('/')) {
            Some((scheme, rest)) => format!("{scheme}://{rest}"),
            None => u.to_owned(),
        }),
    }));
    guard
}

#[test]
fn a_refreshed_oauth_token_is_written_back() {
    use sharex_uploaders::dpapi::Secret;
    use sharex_uploaders::oauth::{OAuth2Info, OAuth2Token};
    let mock = Mock::start();
    let token = r#"{"access_token":"made-up-new-access","refresh_token":"made-up-new-refresh","expires_in":3600,"token_type":"bearer"}"#;
    let upload = r#"{"success":true,"status":200,"data":{"id":"mAdEuP","link":"https://i.imgur.com/mAdEuP.png","deletehash":"made-up-hash"}}"#;
    mock.reset(json!({ "responses": [ok(token), ok(upload)] }));
    let _guard = rewrite_to(&mock);
    let dir = TempDir::new("oauth");
    let imgur = Some(OAuth2Info {
        Client_ID: Some("made-up-client".into()),
        Client_Secret: Some("made-up-secret".into()),
        Token: Some(OAuth2Token {
            access_token: Secret::new("made-up-old-access"),
            refresh_token: Secret::new("made-up-refresh"),
            expires_in: 3600,
            ExpireDate: Value::String("2020-01-01T00:00:00Z".into()),
            ..Default::default()
        }),
        ..Default::default()
    });
    let config = UploadersConfig { ImgurAccountType: sharex_uploaders::AccountType::User, ImgurOAuth2Info: imgur, ..Default::default() };
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::CopyURLToClipboard;
    task.ImageDestination = ImageDestination::Imgur;
    let out = run(image_task(task, &dir.0), config, &dir.0, Options::default());
    http::set_url_rewriter(None);

    let log = mock.log();
    let targets: Vec<&str> = log.iter().map(|r| r.target.as_str()).collect();
    assert_eq!(targets, ["/__/https/api.imgur.com/oauth2/token", "/__/https/api.imgur.com/3/upload"]);
    assert!(log[0].body_text().contains("made-up-refresh"));
    assert_eq!(log[1].header("Authorization"), Some("Bearer made-up-new-access"));
    assert_eq!(out.status(), TaskStatus::Completed, "{:?}", out.info().result.errors);
    assert_eq!(out.info().result.url, "https://i.imgur.com/mAdEuP.png");
    // The refreshed token is in the shared config, to be saved with it.
    out.config.with(|c| {
        let t = c.ImgurOAuth2Info.as_ref().unwrap().Token.as_ref().unwrap();
        assert_eq!(t.access_token.as_str(), "made-up-new-access");
        assert_eq!(t.refresh_token.as_str(), "made-up-new-refresh");
        assert!(!t.is_expired());
    });
}

#[test]
fn a_built_in_shortener_shortens_after_upload() {
    let mock = Mock::start();
    mock.reset(json!({ "responses": [ok(IMAGE_RESPONSE), text_ok("https://is.gd/mAdEuP")] }));
    let _guard = rewrite_to(&mock);
    let dir = TempDir::new("isgd");
    let mut task = task_settings();
    task.AfterCaptureJob = AfterCaptureTasks::UploadImageToHost;
    task.AfterUploadJob = AfterUploadTasks::UseURLShortener | AfterUploadTasks::CopyURLToClipboard;
    task.ImageDestination = ImageDestination::CustomImageUploader;
    task.URLShortenerDestination = UrlShortenerType::ISGD;
    let out = run(image_task(task, &dir.0), config_with(vec![image_host(&mock)]), &dir.0, Options::default());
    http::set_url_rewriter(None);

    let log = mock.log();
    assert_eq!(log.len(), 2);
    // is.gd's API is plain HTTP in ShareX.
    assert!(log[1].target.starts_with("/__/http/is.gd/create.php?format=simple&url=https%3a%2f%2fi.example.com%2fabc.png"), "{}", log[1].target);
    assert_eq!(out.info().result.shortened_url, "https://is.gd/mAdEuP");
    assert_eq!(out.ui, [Ui::Copy("https://is.gd/mAdEuP".into())]);
}

// ---------------------------------------------------------------------------
// UploadersConfig.json.

#[test]
fn the_uploaders_config_loads_and_saves_with_the_other_settings() {
    let dir = TempDir::new("config");
    let paths = AppPaths::in_folder(&dir.0);
    let mut settings = SettingManager::load(paths.clone(), SystemOptions::default());
    let path = sharex_uploaders::config::uploaders_config_file_path(&paths, Some(&settings.settings)).unwrap();
    assert_eq!(path, dir.0.join("UploadersConfig.json"));
    let store = UploadersConfigStore::load_in_background(Some(path.clone()), paths.backup_folder());
    store.with(|c| {
        assert!(c.CustomUploadersList.0.is_empty(), "a new config");
        c.CustomUploadersList.0.push(custom(json!({ "Version": "18.0.0", "Name": "Made-up", "RequestURL": "https://example.com/u" })));
    });
    for _ in 0..2 {
        let json = store.to_json().unwrap();
        settings.save_file(json, path.clone(), true).unwrap();
    }
    assert!(paths.backup_folder().join("UploadersConfig.json").is_file(), "the previous file is backed up");
    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["ApplicationVersion"], sharex_settings::settings_base::application_version());
    let reloaded = UploadersConfigStore::load_in_background(Some(path), paths.backup_folder());
    reloaded.with(|c| assert_eq!(c.CustomUploadersList.0[0].name.as_deref(), Some("Made-up")));
}
