//! The update check and download against a mock GitHub API on 127.0.0.1
//! serving canned releases JSON and assets.

mod mock;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mock::{Mock, Response};
use sha2::Digest;
use sharex_update::checker::{Arch, GitHubUpdateChecker, InstallFlow, UpdateStatus};
use sharex_update::downloader_window::{DownloaderWindow, DownloaderWindowStatus};
use sharex_update::install::UpdateHost;
use sharex_update::message::{UpdateMessage, build_message};
use sharex_update::version::AppVersion;

fn asset(mock: &Mock, id: u32, name: &str, tag: &str) -> String {
    format!(
        r#"{{"url":"{}/repos/o/r/releases/assets/{id}","id":{id},"name":"{name}","label":null,"content_type":"application/octet-stream","state":"uploaded","size":3,"download_count":0,"created_at":"2026-11-01T10:00:00Z","updated_at":"2026-11-01T10:00:00Z","browser_download_url":"{}/dl/{tag}/{name}"}}"#,
        mock.base, mock.base
    )
}

fn release(mock: &Mock, tag: &str, prerelease: bool, published: &str, assets: &[&str]) -> String {
    let list: Vec<String> = assets.iter().enumerate().map(|(i, n)| asset(mock, i as u32 + 1, n, tag)).collect();
    format!(
        r#"{{"url":"{b}/repos/o/r/releases/1","html_url":"https://github.com/o/r/releases/tag/{tag}","id":1,"tag_name":"{tag}","name":"{tag}","draft":false,"prerelease":{prerelease},"created_at":"{published}","published_at":"{published}","assets":[{}],"body":"notes"}}"#,
        list.join(","),
        b = mock.base
    )
}

const WINDOWS_ASSETS: &[&str] = &[
    "ShareXrs-22.5.0-setup-x64.exe",
    "ShareXrs-22.5.0-setup-arm64.exe",
    "ShareXrs-22.5.0-portable-x64.zip",
    "ShareXrs-22.5.0-portable-arm64.zip",
    "ShareXrs-macos-aarch64.dmg",
    "ShareXrs-macos-aarch64.app.zip",
    "ShareXrs-22.5.0-linux-x64.deb",
    "ShareXrs-22.5.0-linux-x64.tar.gz",
    "SHA256SUMS",
];

fn checker(mock: &Mock, flow: InstallFlow) -> GitHubUpdateChecker {
    let mut c = GitHubUpdateChecker::new("o", "r", flow);
    c.api_url = mock.base.clone();
    c.arch = Arch::X64;
    c.ignore_revision = true;
    c.current_version = Some(AppVersion::parse("22.4.0.0").unwrap());
    c
}

fn latest(mock: &Mock, tag: &str, prerelease: bool) {
    mock.route("/repos/o/r/releases/latest", Response::json(&release(mock, tag, prerelease, "2026-11-01T10:00:00Z", WINDOWS_ASSETS)));
}

#[test]
fn up_to_date() {
    let mock = Mock::start();
    latest(&mock, "v22.4.0", false);
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpToDate);
    assert_eq!(c.latest_version.as_ref().unwrap().to_string(), "22.4.0");
    // The revision is ignored (`IgnoreRevision`), as for ShareX's own releases.
    latest(&mock, "v22.4.0.7", false);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpToDate);
    c.ignore_revision = false;
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    // An older release is "up to date" too.
    latest(&mock, "v22.3.9", false);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpToDate);
}

#[test]
fn newer_stable_release() {
    let mock = Mock::start();
    latest(&mock, "v22.5.0", false);
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    assert!(!c.is_pre_release);
    // Installed: the asset's API URL, downloaded with Accept: octet-stream.
    assert_eq!(c.download_url.as_deref(), Some(format!("{}/repos/o/r/releases/assets/1", mock.base).as_str()));
    assert_eq!(c.file_name(), "ShareXrs-22.5.0-setup-x64.exe");
    assert_eq!(c.checksums_url().unwrap(), format!("{}/repos/o/r/releases/assets/9", mock.base));
    let log = mock.log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].method, "GET");
    assert_eq!(log[0].path, "/repos/o/r/releases/latest");
    assert_eq!(log[0].header("User-Agent"), Some(format!("ShareXrs/{}", env!("CARGO_PKG_VERSION")).as_str()));
    assert_eq!(log[0].header("Cache-Control"), Some("no-cache"));
    assert_eq!(log[0].header("Accept"), None);
    let nl = sharex_update::message::NEW_LINE;
    assert_eq!(
        build_message(&c),
        format!("Would you like to download and install it?{nl}{nl}Current version: 22.4.0.0{nl}Latest version: 22.5.0")
    );
    let m = UpdateMessage::new(&c, true);
    assert!(m.show_changelog);
    assert_eq!(m.changelog_url, "https://github.com/o/r/releases/tag/v22.5.0");
}

#[test]
fn picks_each_platforms_file() {
    let mock = Mock::start();
    latest(&mock, "v22.5.0", false);
    let pick = |flow: InstallFlow, arch: Arch| {
        let mut c = checker(&mock, flow);
        c.arch = arch;
        c.check_update();
        (c.status, c.file_name(), c.download_url.clone().unwrap_or_default())
    };
    let (status, name, url) = pick(InstallFlow::WindowsPortable, Arch::X64);
    assert_eq!(status, UpdateStatus::UpdateAvailable);
    assert_eq!(name, "ShareXrs-22.5.0-portable-x64.zip");
    // Portable: the browser download URL.
    assert_eq!(url, format!("{}/dl/v22.5.0/ShareXrs-22.5.0-portable-x64.zip", mock.base));
    assert_eq!(pick(InstallFlow::WindowsPortable, Arch::Arm64).1, "ShareXrs-22.5.0-portable-arm64.zip");
    assert_eq!(pick(InstallFlow::WindowsSetup, Arch::Arm64).1, "ShareXrs-22.5.0-setup-arm64.exe");
    let (_, name, url) = pick(InstallFlow::MacDiskImage, Arch::Arm64);
    assert_eq!(name, "ShareXrs-macos-aarch64.dmg");
    assert!(url.contains("/dl/"));
    let (_, name, url) = pick(InstallFlow::MacApp { app: PathBuf::from("/Applications/ShareXrs.app") }, Arch::Arm64);
    assert_eq!(name, "ShareXrs-macos-aarch64.app.zip");
    assert!(url.contains("/releases/assets/"));
    assert_eq!(pick(InstallFlow::LinuxDeb, Arch::X64).1, "ShareXrs-22.5.0-linux-x64.deb");
    assert_eq!(pick(InstallFlow::LinuxTarball { folder: PathBuf::from("/opt/ShareXrs") }, Arch::X64).1, "ShareXrs-22.5.0-linux-x64.tar.gz");
    // No Intel Mac build: the check fails.
    assert_eq!(pick(InstallFlow::MacDiskImage, Arch::X64).0, UpdateStatus::UpdateCheckFailed);
}

#[test]
fn falls_back_to_shorter_asset_names() {
    let mock = Mock::start();
    mock.route(
        "/repos/o/r/releases/latest",
        Response::json(&release(&mock, "v22.5.0", false, "2026-11-01T10:00:00Z", &["Other.zip", "ShareXrs-portable.zip", "ShareXrs-SETUP.EXE"])),
    );
    let mut c = checker(&mock, InstallFlow::WindowsPortable);
    c.arch = Arch::Arm64;
    c.check_update();
    assert_eq!(c.file_name(), "ShareXrs-portable.zip");
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.check_update();
    // `EndsWith(..., OrdinalIgnoreCase)`.
    assert_eq!(c.file_name(), "ShareXrs-SETUP.EXE");
}

fn releases(mock: &Mock) {
    // Out of order on purpose: the newest published comes first after sorting.
    let list = [
        release(mock, "v22.5.0", false, "2026-11-01T10:00:00Z", WINDOWS_ASSETS),
        release(mock, "v22.6.0-beta.1", true, "2026-11-10T10:00:00Z", WINDOWS_ASSETS),
        release(mock, "v22.4.0", false, "2026-10-01T10:00:00Z", WINDOWS_ASSETS),
    ];
    mock.route("/repos/o/r/releases", Response::json(&format!("[{}]", list.join(","))));
    latest(mock, "v22.5.0", false);
}

#[test]
fn pre_release_only_on_the_pre_release_channel() {
    let mock = Mock::start();
    releases(&mock);
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.check_update();
    assert_eq!(c.latest_version.as_ref().unwrap().to_string(), "22.5.0");
    assert!(!c.is_pre_release);
    assert_eq!(mock.log().last().unwrap().path, "/repos/o/r/releases/latest");

    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.include_pre_release = true;
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    assert_eq!(c.latest_version.as_ref().unwrap().to_string(), "22.6.0-beta.1");
    assert!(c.is_pre_release);
    assert_eq!(mock.log().last().unwrap().path, "/repos/o/r/releases");
    assert!(build_message(&c).ends_with("Latest version: 22.6.0-beta.1 (Pre-release)"));
}

#[test]
fn a_pre_release_build_updates_to_its_release() {
    let mock = Mock::start();
    latest(&mock, "v22.5.0", false);
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.current_version = Some(AppVersion::parse("22.5.0.0-rc.1").unwrap());
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    latest(&mock, "v22.5.0-rc.2", true);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    latest(&mock, "v22.5.0-beta.9", true);
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpToDate);
}

#[test]
fn dev_build() {
    let mock = Mock::start();
    // ShareX's `DownloadDevBuild`: a dev checker keeps the revision.
    latest(&mock, "v22.4.0.1", false);
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.is_dev = true;
    c.ignore_revision = false;
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    assert!(build_message(&c).ends_with("Latest version: 22.4.0.1 Dev"));
    assert!(!UpdateMessage::new(&c, true).show_changelog);
}

#[test]
fn failures() {
    let mock = Mock::start();
    let check = |body: Response| {
        mock.route("/repos/o/r/releases/latest", body);
        let mut c = checker(&mock, InstallFlow::WindowsSetup);
        c.check_update();
        c
    };
    // Missing asset: the version is read, but there's nothing to download.
    let c = check(Response::json(&release(&mock, "v22.5.0", false, "2026-11-01T10:00:00Z", &["ShareXrs-macos-aarch64.dmg"])));
    assert_eq!(c.status, UpdateStatus::UpdateCheckFailed);
    assert_eq!(c.latest_version.as_ref().unwrap().to_string(), "22.5.0");
    assert_eq!(c.download_url, None);
    for body in [
        Response::json("{ not json"),
        Response::json("[]"),
        Response::json("null"),
        Response::json(""),
        Response::json(r#"{"tag_name":"22.5.0","assets":[]}"#),
        Response::json(r#"{"tag_name":"v","assets":[]}"#),
        Response::json(r#"{"tag_name":"vNext","assets":[]}"#),
        Response::json(r#"{"tag_name":"v22","assets":[]}"#),
        Response::json(r#"{"tag_name":"v22.5.0","published_at":null}"#),
        Response::status(404, "Not Found", r#"{"message":"Not Found"}"#),
        Response::status(403, "Forbidden", r#"{"message":"API rate limit exceeded"}"#),
        Response::status(500, "Internal Server Error", ""),
    ] {
        let c = check(body);
        assert_eq!(c.status, UpdateStatus::UpdateCheckFailed);
        assert!(c.error.is_some());
    }
    // Nothing listening.
    let mut c = checker(&mock, InstallFlow::WindowsSetup);
    c.api_url = "http://127.0.0.1:9".into();
    c.check_update();
    assert_eq!(c.status, UpdateStatus::UpdateCheckFailed);
}

// ---------------------------------------------------------------------------
// Downloading and installing.

#[derive(Default)]
struct FakeHost {
    calls: Mutex<Vec<String>>,
}

impl UpdateHost for FakeHost {
    fn open_url(&self, url: &str) {
        self.calls.lock().unwrap().push(format!("open_url {url}"));
    }
    fn open_file(&self, path: &Path) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("open_file {}", path.display()));
        Ok(())
    }
    fn run_installer(&self, path: &Path, arguments: &str, _elevate: bool) -> Result<(), String> {
        let name = path.file_name().unwrap().to_string_lossy();
        self.calls.lock().unwrap().push(format!("run_installer {name} {arguments}"));
        Ok(())
    }
    fn restart(&self, exe: &Path) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("restart {}", exe.file_name().unwrap().to_string_lossy()));
        Ok(())
    }
    fn shutdown(&self) {
        self.calls.lock().unwrap().push("shutdown".into());
    }
}

impl FakeHost {
    fn wait_for(&self, prefix: &str) -> String {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if let Some(c) = self.calls.lock().unwrap().iter().find(|c| c.starts_with(prefix)) {
                return c.clone();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no {prefix} call: {:?}", self.calls.lock().unwrap());
    }
}

fn pump(window: &mut DownloaderWindow, until: impl Fn(&DownloaderWindow) -> bool) {
    let start = Instant::now();
    while !until(window) {
        window.poll();
        assert!(start.elapsed() < Duration::from_secs(20), "timed out: {}", window.status_text);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn sha256(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

/// A release whose assets the mock serves, with `SHA256SUMS` for them.
fn release_with_files(mock: &Mock, files: &[(&str, Vec<u8>)], sums: Option<String>, flow: InstallFlow) -> GitHubUpdateChecker {
    let mut names: Vec<&str> = files.iter().map(|(n, _)| *n).collect();
    if sums.is_some() {
        names.push("SHA256SUMS");
    }
    mock.route("/repos/o/r/releases/latest", Response::json(&release(mock, "v22.5.0", false, "2026-11-01T10:00:00Z", &names)));
    for (i, (_, data)) in files.iter().enumerate() {
        mock.route(&format!("/repos/o/r/releases/assets/{}", i + 1), Response::ok(data.clone()));
    }
    if let Some(sums) = sums {
        mock.route(&format!("/repos/o/r/releases/assets/{}", files.len() + 1), Response::ok(sums));
    }
    let mut c = checker(mock, flow);
    c.check_update();
    c
}

#[test]
fn downloads_checks_the_signature_and_runs_the_setup() {
    let mock = Mock::start();
    let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let name = "ShareXrs-22.5.0-dl1-setup-x64.exe";
    let c = release_with_files(&mock, &[(name, data.clone())], Some(format!("{}  {name}\n", sha256(&data))), InstallFlow::WindowsSetup);
    assert_eq!(c.status, UpdateStatus::UpdateAvailable);
    // Served slowly so progress events come in.
    let mut slow = Response::ok(data.clone());
    slow.chunk = 20_000;
    slow.chunk_delay = Duration::from_millis(30);
    mock.route("/repos/o/r/releases/assets/1", slow);

    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    assert_eq!(window.file_name_text, format!("Filename: {name}"));
    assert_eq!(window.status_text, "Status: Waiting...");
    window.opened();
    assert_eq!(window.status, DownloaderWindowStatus::DownloadStarted);
    assert_eq!(window.action_text, "Cancel");
    pump(&mut window, |w| w.progress_text.starts_with("Progress:") && w.progress_value > 0.0 && w.progress_value < 100.0);
    assert_eq!(window.status_text, "Status: Downloading...");
    // The setup isn't signed: ShareX asks before running it.
    pump(&mut window, |w| w.signature_prompt);
    assert_eq!(window.status, DownloaderWindowStatus::DownloadCompleted);
    assert_eq!(window.status_text, "Status: Download completed.");
    assert_eq!(window.action_text, "Install");
    assert!(window.progress_text.starts_with("Progress: 100.0%\r\n"), "{}", window.progress_text);
    assert_eq!(std::fs::read(&window.download_location).unwrap(), data);
    let asset_request = mock.log().into_iter().find(|r| r.path == "/repos/o/r/releases/assets/1").unwrap();
    assert_eq!(asset_request.header("Accept"), Some("application/octet-stream"));
    window.confirm_signature(true);
    assert_eq!(window.status, DownloaderWindowStatus::InstallStarted);
    assert!(window.close_requested && window.shutdown_after);
    assert_eq!(host.wait_for("run_installer"), format!("run_installer {name} /UPDATE /SILENT"));
    let _ = std::fs::remove_file(&window.download_location);
}

#[test]
fn no_to_the_signature_question_cancels() {
    let mock = Mock::start();
    let name = "ShareXrs-22.5.0-dl2-setup-x64.exe";
    let c = release_with_files(&mock, &[(name, b"abc".to_vec())], None, InstallFlow::WindowsSetup);
    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.signature_prompt);
    window.confirm_signature(false);
    assert_eq!(window.result, sharex_update::DialogResult::Cancel);
    assert!(window.close_requested && !window.shutdown_after);
    std::thread::sleep(Duration::from_millis(1200));
    assert!(host.calls.lock().unwrap().is_empty());
    let _ = std::fs::remove_file(&window.download_location);
}

#[test]
fn cancelling_deletes_the_partial_download() {
    let mock = Mock::start();
    let name = "ShareXrs-22.5.0-dl3-setup-x64.exe";
    let data = vec![7u8; 400_000];
    let c = release_with_files(&mock, &[(name, data.clone())], None, InstallFlow::WindowsSetup);
    let mut slow = Response::ok(data);
    slow.chunk = 10_000;
    slow.chunk_delay = Duration::from_millis(40);
    mock.route("/repos/o/r/releases/assets/1", slow);
    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.progress_value > 0.0);
    window.action_click();
    assert_eq!(window.result, sharex_update::DialogResult::Cancel);
    assert!(window.close_requested && window.is_closed());
    let location = window.download_location.clone();
    let start = Instant::now();
    while location.exists() && start.elapsed() < Duration::from_secs(10) {
        window.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!location.exists());
    std::thread::sleep(Duration::from_millis(300));
    window.poll();
    // ShareX would go on to Install(); ShareXrs stops.
    assert!(!window.signature_prompt);
    assert_ne!(window.status, DownloaderWindowStatus::InstallStarted);
    assert!(host.calls.lock().unwrap().is_empty());
}

#[test]
fn a_checksum_mismatch_stops_the_install() {
    let mock = Mock::start();
    let name = "ShareXrs-22.5.0-dl4-setup-x64.exe";
    let c = release_with_files(&mock, &[(name, b"tampered".to_vec())], Some(format!("{}  {name}\n", sha256(b"original"))), InstallFlow::WindowsSetup);
    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.status_text.contains("checksum"));
    assert_eq!(window.status_text, format!("Status: {name} doesn't match its checksum"));
    assert!(!window.signature_prompt);
    assert!(!window.download_location.exists());
}

#[test]
fn download_errors_show_in_the_status() {
    let mock = Mock::start();
    let name = "ShareXrs-22.5.0-dl5-setup-x64.exe";
    let c = release_with_files(&mock, &[(name, b"abc".to_vec())], None, InstallFlow::WindowsSetup);
    mock.route("/repos/o/r/releases/assets/1", Response::status(404, "Not Found", "{}"));
    let mut window = DownloaderWindow::for_update(&c, Arc::new(FakeHost::default()));
    window.opened();
    pump(&mut window, |w| w.status_text.contains("404"));
    assert_eq!(window.status_text, "Status: Response status code does not indicate success: 404 (Not Found).");
    // ShareX leaves the button on Cancel, which closes the window.
    assert_eq!(window.action_text, "Cancel");
    window.action_click();
    assert!(window.close_requested);

    // No Content-Length: `StartDownload` gives up quietly.
    let mut no_length = Response::ok(b"abc".to_vec());
    no_length.no_length = true;
    mock.route("/repos/o/r/releases/assets/1", no_length);
    let mut window = DownloaderWindow::for_update(&c, Arc::new(FakeHost::default()));
    window.opened();
    pump(&mut window, |w| w.status_text == "Status: Downloading...");
    std::thread::sleep(Duration::from_millis(300));
    window.poll();
    assert_eq!(window.status, DownloaderWindowStatus::DownloadStarted);
    assert!(!window.signature_prompt);
}

/// The Linux tarball (an addition) updates in place on any OS's file system.
#[test]
fn linux_tarball_replaces_files_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("app");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("sharexrs"), b"old").unwrap();
    let mut tar_data = Vec::new();
    {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(&mut tar_data, flate2::Compression::fast()));
        for (path, data) in [("ShareXrs/sharexrs", &b"new"[..]), ("ShareXrs/ffmpeg", &b"ff"[..])] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, path, data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();
    }
    let name = "ShareXrs-22.5.0-dl6-linux-x64.tar.gz";
    let flow = InstallFlow::LinuxTarball { folder: folder.clone() };

    // Without SHA256SUMS an in-place update refuses.
    let mock = Mock::start();
    let c = release_with_files(&mock, &[(name, tar_data.clone())], None, flow.clone());
    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.status_text.contains("SHA256SUMS"));
    assert_eq!(std::fs::read(folder.join("sharexrs")).unwrap(), b"old");

    let mock = Mock::start();
    let c = release_with_files(&mock, &[(name, tar_data.clone())], Some(format!("{} *{name}\n", sha256(&tar_data))), flow);
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.status == DownloaderWindowStatus::InstallStarted);
    assert!(!window.signature_prompt);
    assert_eq!(std::fs::read(folder.join("sharexrs")).unwrap(), b"new");
    assert_eq!(std::fs::read(folder.join("ffmpeg")).unwrap(), b"ff");
    assert!(window.shutdown_after);
    assert_eq!(host.wait_for("restart"), "restart sharexrs");
}

#[test]
fn the_deb_goes_to_the_package_installer() {
    let mock = Mock::start();
    let name = "ShareXrs-22.5.0-dl7-linux-x64.deb";
    let data = b"!<arch>\n".to_vec();
    let c = release_with_files(&mock, &[(name, data.clone())], Some(format!("{}  {name}\n", sha256(&data))), InstallFlow::LinuxDeb);
    let host = Arc::new(FakeHost::default());
    let mut window = DownloaderWindow::for_update(&c, host.clone());
    window.opened();
    pump(&mut window, |w| w.status == DownloaderWindowStatus::InstallStarted);
    assert!(host.wait_for("open_file").ends_with(name));
    assert!(!window.shutdown_after);
    let _ = std::fs::remove_file(&window.download_location);
}

/// macOS: the zipped app replaces the whole bundle (needs `ditto`).
#[cfg(target_os = "macos")]
#[test]
fn mac_app_is_swapped_whole() {
    let dir = tempfile::tempdir().unwrap();
    let make_app = |root: &Path, marker: &[u8]| {
        let app = root.join("ShareXrs.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(app.join("Contents/MacOS/sharexrs"), marker).unwrap();
        app
    };
    let installed = make_app(&dir.path().join("Applications"), b"old");
    let new_root = dir.path().join("build");
    make_app(&new_root, b"new");
    let zip = dir.path().join("ShareXrs-macos-aarch64.app.zip");
    let status = std::process::Command::new("ditto")
        .args(["-c", "-k", "--keepParent"])
        .arg(new_root.join("ShareXrs.app"))
        .arg(&zip)
        .status()
        .unwrap();
    assert!(status.success());
    sharex_update::install::replace_app(&installed, &zip).unwrap();
    assert_eq!(std::fs::read(installed.join("Contents/MacOS/sharexrs")).unwrap(), b"new");
    assert!(!sharex_update::install::sibling(&installed, "old").exists());
}
