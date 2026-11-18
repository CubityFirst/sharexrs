//! `UpdateChecker` and `GitHubUpdateChecker` (`ShareX.HelpersLib/UpdateChecker/`):
//! the latest release from GitHub's API (`/releases/latest`, or the newest
//! of `/releases` on the pre-release channel), the tag read as a version,
//! this build's file picked by the end of its name, and the status from
//! comparing versions.
//!
//! ShareX picks its portable zip or setup exe. ShareXrs also ships for
//! macOS and Linux (docs/RELEASING.md), so [`InstallFlow`] adds, as
//! ShareXrs additions, the files those installs update from: the zipped
//! app (`.app.zip`, swapped whole) or the disk image (`.dmg`, opened in the
//! browser), and the `.deb` or the tarball (`.tar.gz`, unpacked in place).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use sharex_uploaders::http::{self, HttpRequest, RequestOptions};
use sharex_uploaders::HttpMethod;

use crate::release::{GitHubAsset, GitHubRelease, parse_release, parse_releases};
use crate::version::AppVersion;

/// `GitHubUpdateChecker.APIURL`.
pub const API_URL: &str = "https://api.github.com";

/// `UpdateStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    #[default]
    None,
    UpdateCheckFailed,
    UpdateAvailable,
    UpToDate,
}

/// `RuntimeInformation.OSArchitecture`, as far as the asset names care.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
    Other,
}

impl Arch {
    /// The machine's architecture (not the process's: an x64 build running
    /// on Windows on ARM is on `Arm64`, as .NET reports it).
    pub fn current() -> Self {
        #[cfg(windows)]
        if let Some(arch) = windows_native_arch() {
            return arch;
        }
        match std::env::consts::ARCH {
            "x86_64" => Arch::X64,
            "aarch64" => Arch::Arm64,
            _ => Arch::Other,
        }
    }
}

#[cfg(windows)]
fn windows_native_arch() -> Option<Arch> {
    use windows_sys::Win32::System::SystemInformation::{IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2};
    let (mut process, mut native) = (0u16, 0u16);
    // SAFETY: the current process's pseudo handle and two out values.
    if unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) } == 0 {
        return None;
    }
    match native {
        IMAGE_FILE_MACHINE_AMD64 => Some(Arch::X64),
        IMAGE_FILE_MACHINE_ARM64 => Some(Arch::Arm64),
        _ => None,
    }
}

/// How this copy of ShareXrs updates itself, which decides the file it
/// looks for and what happens once it's chosen (`UpdateChecker.DownloadUpdateAsync`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallFlow {
    /// ShareX's portable build: open the portable zip's browser download
    /// URL (`URLHelpers.OpenURL`).
    WindowsPortable,
    /// ShareX's installed build: download the setup exe (the asset's API
    /// URL) in the downloader window and run it with `/UPDATE`.
    WindowsSetup,
    /// Addition: a ShareXrs.app (not translocated, not portable): download
    /// the zipped app and swap the whole bundle (MACOS-LEARNINGS.md §1.6).
    MacApp { app: PathBuf },
    /// Addition: any other macOS copy (a bare program, a translocated or
    /// portable app): open the disk image's browser download URL.
    MacDiskImage,
    /// Addition: installed from the .deb (`/usr/lib/sharexrs`): download the
    /// .deb and hand it to the desktop's package installer.
    LinuxDeb,
    /// Addition: unpacked from the tarball: download it and replace the
    /// files it holds in the folder ShareXrs runs from.
    LinuxTarball { folder: PathBuf },
}

impl InstallFlow {
    /// The flow for this copy: `portable` is `StartupOptions.Portable`.
    pub fn detect(portable: bool) -> Self {
        let exe = crate::install::exe().map(Path::to_path_buf).unwrap_or_default();
        Self::for_platform(std::env::consts::OS, &exe, portable)
    }

    /// [`detect`](Self::detect) for any OS (`std::env::consts::OS`) and
    /// executable path, so each platform's rules can be tested anywhere.
    pub fn for_platform(os: &str, exe: &Path, portable: bool) -> Self {
        match os {
            "macos" => match crate::install::app_of(exe) {
                Some(app) if !portable && !crate::install::is_translocated(app) => InstallFlow::MacApp { app: app.to_path_buf() },
                _ => InstallFlow::MacDiskImage,
            },
            "linux" => {
                if exe.starts_with(crate::install::DEB_FOLDER) {
                    InstallFlow::LinuxDeb
                } else {
                    InstallFlow::LinuxTarball { folder: exe.parent().map(Path::to_path_buf).unwrap_or_default() }
                }
            }
            _ => {
                if portable {
                    InstallFlow::WindowsPortable
                } else {
                    InstallFlow::WindowsSetup
                }
            }
        }
    }

    /// `UpdateChecker.IsPortable`: the update opens in the browser instead
    /// of being downloaded and installed (the message says "download"
    /// rather than "download and install").
    pub fn is_portable(&self) -> bool {
        matches!(self, InstallFlow::WindowsPortable | InstallFlow::MacDiskImage)
    }

    /// The asset name endings to look for, in order (`UpdateReleaseInfo`).
    pub fn asset_suffixes(&self, arch: Arch) -> Vec<String> {
        let mut out = Vec::new();
        let arch_name = match arch {
            Arch::Arm64 => "arm64",
            _ => "x64",
        };
        match self {
            InstallFlow::WindowsPortable => {
                if arch == Arch::Arm64 {
                    out.push("portable-arm64.zip".to_owned());
                }
                out.push("portable-x64.zip".to_owned());
                out.push("portable.zip".to_owned());
            }
            InstallFlow::WindowsSetup => {
                if arch == Arch::Arm64 {
                    out.push("setup-arm64.exe".to_owned());
                }
                out.push("setup-x64.exe".to_owned());
                out.push("setup.exe".to_owned());
            }
            // `ShareXrs-macos-aarch64.app.zip`, `.dmg` (Apple silicon only).
            InstallFlow::MacApp { .. } if arch == Arch::Arm64 => out.push("macos-aarch64.app.zip".to_owned()),
            InstallFlow::MacDiskImage if arch == Arch::Arm64 => out.push("macos-aarch64.dmg".to_owned()),
            InstallFlow::MacApp { .. } | InstallFlow::MacDiskImage => {}
            // `ShareXrs-<ver>-linux-x64.deb`, `.tar.gz`.
            InstallFlow::LinuxDeb => out.push(format!("linux-{arch_name}.deb")),
            InstallFlow::LinuxTarball { .. } => out.push(format!("linux-{arch_name}.tar.gz")),
        }
        out
    }
}

/// `GitHubUpdateChecker`, with `UpdateChecker`'s members.
#[derive(Debug, Clone)]
pub struct GitHubUpdateChecker {
    pub owner: String,
    pub repo: String,
    /// `IncludePreRelease`: the newest of `/releases` instead of `/releases/latest`.
    pub include_pre_release: bool,
    /// `IsDev`: the message shows "Dev" after the version and no changelog link.
    pub is_dev: bool,
    pub flow: InstallFlow,
    /// `IgnoreRevision`.
    pub ignore_revision: bool,
    pub arch: Arch,
    /// `APIURL` (a test points it at a local server).
    pub api_url: String,
    /// `ForceUpdate` ("for testing purposes").
    pub force_update: bool,

    pub status: UpdateStatus,
    pub current_version: Option<AppVersion>,
    pub latest_version: Option<AppVersion>,
    /// `IsPreRelease`: the chosen release is marked as a pre-release.
    pub is_pre_release: bool,
    pub download_url: Option<String>,
    file_name: Option<String>,
    /// The chosen release, for its `SHA256SUMS` and page (ShareXrs).
    pub release: Option<GitHubRelease>,
    /// Why the check failed (`DebugHelper.WriteException`'s text).
    pub error: Option<String>,
}

impl GitHubUpdateChecker {
    /// `new GitHubUpdateChecker(owner, repo)`.
    pub fn new(owner: &str, repo: &str, flow: InstallFlow) -> Self {
        Self {
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            include_pre_release: false,
            is_dev: false,
            flow,
            ignore_revision: false,
            arch: Arch::current(),
            api_url: API_URL.to_owned(),
            force_update: false,
            status: UpdateStatus::None,
            current_version: None,
            latest_version: None,
            is_pre_release: false,
            download_url: None,
            file_name: None,
            release: None,
            error: None,
        }
    }

    /// `IsPortable`.
    pub fn is_portable(&self) -> bool {
        self.flow.is_portable()
    }

    fn releases_url(&self) -> String {
        format!("{}/repos/{}/{}/releases", self.api_url, self.owner, self.repo)
    }

    /// `FileName`: the asset's name, else the download URL's last segment
    /// (`HttpUtility.UrlDecode`).
    pub fn file_name(&self) -> String {
        match self.file_name.as_deref().filter(|n| !n.is_empty()) {
            Some(name) => name.to_owned(),
            None => {
                let url = self.download_url.as_deref().unwrap_or("");
                url_decode(&url[url.rfind('/').map_or(0, |i| i + 1)..])
            }
        }
    }

    /// The chosen asset's name as stored (`fileName`), before
    /// [`file_name`](Self::file_name)'s fallback.
    pub fn asset_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    pub fn set_file_name(&mut self, name: Option<String>) {
        self.file_name = name;
    }

    /// `CheckUpdateAsync`.
    pub fn check_update(&mut self) {
        match self.get_latest_release(self.include_pre_release) {
            Ok(release) => {
                let browser = self.is_portable();
                match self.update_release_info(release.as_ref(), browser) {
                    Ok(true) => {
                        self.release = release;
                        self.refresh_status();
                        return;
                    }
                    Ok(false) => self.error = Some("GitHub update check failed: no release or asset found.".to_owned()),
                    Err(e) => self.error = Some(e),
                }
            }
            Err(e) => self.error = Some(e),
        }
        self.status = UpdateStatus::UpdateCheckFailed;
    }

    /// `RefreshStatus`.
    pub fn refresh_status(&mut self) {
        if self.current_version.is_none() {
            self.current_version = Some(AppVersion::current());
        }
        let newer = match (&self.current_version, &self.latest_version) {
            (Some(current), Some(latest)) => {
                self.force_update || current.compare(latest, self.ignore_revision) == Ordering::Less
            }
            _ => false,
        };
        self.status = if self.status != UpdateStatus::UpdateCheckFailed
            && self.download_url.as_deref().is_some_and(|u| !u.is_empty())
            && newer
        {
            UpdateStatus::UpdateAvailable
        } else {
            UpdateStatus::UpToDate
        };
    }

    /// `GetLatestDownloadURL(isBrowserDownloadURL)`.
    pub fn get_latest_download_url(&mut self, browser: bool) -> Option<String> {
        let release = self.get_latest_release(self.include_pre_release).ok()?;
        match self.update_release_info(release.as_ref(), browser) {
            Ok(true) => self.download_url.clone(),
            _ => None,
        }
    }

    /// `GetLatestRelease(includePreRelease)`.
    pub fn get_latest_release(&self, include_pre_release: bool) -> Result<Option<GitHubRelease>, String> {
        if include_pre_release {
            Ok(self.get_releases()?.and_then(|r| r.into_iter().next()).flatten())
        } else {
            match download_string(&format!("{}/latest", self.releases_url()))? {
                Some(text) if !text.is_empty() => parse_release(&text),
                _ => Ok(None),
            }
        }
    }

    /// `GetReleases`: newest `published_at` first (`List.Sort`'s introsort,
    /// so releases published at the same moment stay where .NET leaves them).
    pub fn get_releases(&self) -> Result<Option<Vec<Option<GitHubRelease>>>, String> {
        let Some(text) = download_string(&self.releases_url())?.filter(|t| !t.is_empty()) else {
            return Ok(None);
        };
        let mut releases = parse_releases(&text)?;
        if let Some(list) = releases.as_mut().filter(|l| !l.is_empty()) {
            // A null element throws in the comparison, which `Sort` wraps.
            let mut failed = false;
            sharex_helpers::dotnet_sort::intro_sort(list, &mut |x, y| match (x, y) {
                (Some(x), Some(y)) => y.published_at.cmp(&x.published_at),
                _ => {
                    failed = true;
                    Ordering::Equal
                }
            });
            if failed {
                return Err("Failed to compare two elements in the array. (Object reference not set to an instance of an object.)".to_owned());
            }
        }
        Ok(releases)
    }

    /// `UpdateReleaseInfo(release, isPortable, isBrowserDownloadURL)`.
    /// An error is an exception (`new Version` on a bad tag).
    pub fn update_release_info(&mut self, release: Option<&GitHubRelease>, browser: bool) -> Result<bool, String> {
        let Some(release) = release else { return Ok(false) };
        let Some(tag) = release.tag_name.as_deref().filter(|t| t.chars().count() > 1 && t.starts_with('v')) else {
            return Ok(false);
        };
        self.latest_version = Some(AppVersion::parse(&tag[1..])?);
        let asset = self.flow.asset_suffixes(self.arch).iter().find_map(|suffix| find_asset(release, suffix));
        let Some(asset) = asset else { return Ok(false) };
        self.file_name = asset.name.clone();
        self.download_url = if browser { asset.browser_download_url.clone() } else { asset.url.clone() };
        self.is_pre_release = release.prerelease;
        Ok(true)
    }

    /// The release's `SHA256SUMS` (ShareXrs's release workflow publishes
    /// one; ShareX's doesn't), by the same kind of URL as the download.
    pub fn checksums_url(&self) -> Option<String> {
        let release = self.release.as_ref()?;
        let asset = release.assets.as_ref()?.iter().flatten().find(|a| a.name.as_deref() == Some("SHA256SUMS"))?;
        if self.is_portable() { asset.browser_download_url.clone() } else { asset.url.clone() }
    }

    /// The release's page, for the changelog link (ShareX opens its
    /// website's changelog; ShareXrs's changelog is the release notes).
    pub fn release_page(&self) -> Option<String> {
        self.release.as_ref()?.html_url.clone()
    }
}

/// `FindAsset(release, endsWith)`: the first asset whose name ends with it
/// (`StringComparison.OrdinalIgnoreCase`).
fn find_asset<'a>(release: &'a GitHubRelease, ends_with: &str) -> Option<&'a GitHubAsset> {
    release
        .assets
        .as_ref()?
        .iter()
        .flatten()
        .find(|a| a.name.as_deref().is_some_and(|n| !n.is_empty() && ends_with_ignore_case(n, ends_with)))
}

/// `string.EndsWith(value, OrdinalIgnoreCase)`: UTF-16 code units compared
/// after simple upper-casing.
fn ends_with_ignore_case(text: &str, suffix: &str) -> bool {
    let upper = |c: char| {
        let mut u = c.to_uppercase();
        match (u.next(), u.next()) {
            (Some(x), None) => x,
            _ => c,
        }
    };
    let t: Vec<char> = text.chars().collect();
    let s: Vec<char> = suffix.chars().collect();
    s.len() <= t.len() && t[t.len() - s.len()..].iter().zip(&s).all(|(a, b)| upper(*a) == upper(*b))
}

/// `HttpUtility.UrlDecode`: `%XX` and `+`, as UTF-8.
fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() && bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit() => {
                let hex = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(bytes[i + 1]) * 16 + hex(bytes[i + 2]));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `WebHelpers.DownloadStringAsync(url)`: the body of a 2xx response,
/// `None` for any other status, an error when there's no response.
pub fn download_string(url: &str) -> Result<Option<String>, String> {
    let request = HttpRequest { method: HttpMethod::GET, url: url.to_owned(), headers: Vec::new(), content: None };
    // `HttpClientFactory.Create()`'s client: 100 s timeout.
    let options = RequestOptions { timeout: Some(std::time::Duration::from_secs(100)), ..Default::default() };
    let response = http::send(request, options).map_err(|e| e.to_string())?;
    Ok(if response.is_success() { response.response_text } else { None })
}

/// The owner and repository in Cargo.toml's `repository`
/// (`https://github.com/CubityFirst/sharexrs`): ShareXrs's own releases,
/// never ShareX's.
pub fn repository() -> (&'static str, &'static str) {
    let path = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/").trim_end_matches('/');
    path.split_once('/').unwrap_or((path, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_at_sharexrs() {
        assert_eq!(repository(), ("CubityFirst", "sharexrs"));
    }

    #[test]
    fn picks_assets_by_suffix() {
        let suffixes = |flow: InstallFlow, arch| flow.asset_suffixes(arch);
        assert_eq!(suffixes(InstallFlow::WindowsSetup, Arch::X64), ["setup-x64.exe", "setup.exe"]);
        assert_eq!(suffixes(InstallFlow::WindowsSetup, Arch::Arm64), ["setup-arm64.exe", "setup-x64.exe", "setup.exe"]);
        assert_eq!(suffixes(InstallFlow::WindowsPortable, Arch::Arm64), ["portable-arm64.zip", "portable-x64.zip", "portable.zip"]);
        assert_eq!(suffixes(InstallFlow::MacDiskImage, Arch::Arm64), ["macos-aarch64.dmg"]);
        assert!(suffixes(InstallFlow::MacDiskImage, Arch::X64).is_empty());
        assert_eq!(suffixes(InstallFlow::LinuxDeb, Arch::X64), ["linux-x64.deb"]);
        assert_eq!(suffixes(InstallFlow::LinuxTarball { folder: PathBuf::new() }, Arch::X64), ["linux-x64.tar.gz"]);
        assert!(ends_with_ignore_case("ShareXrs-22.5.0-SETUP-X64.EXE", "setup-x64.exe"));
        assert!(!ends_with_ignore_case("x64.exe", "setup-x64.exe"));
    }

    #[test]
    fn detects_the_flow() {
        let f = |os, exe: &str, portable| InstallFlow::for_platform(os, Path::new(exe), portable);
        assert_eq!(f("windows", r"C:\Program Files\ShareXrs\sharexrs.exe", false), InstallFlow::WindowsSetup);
        assert_eq!(f("windows", r"D:\ShareXrs\sharexrs.exe", true), InstallFlow::WindowsPortable);
        assert_eq!(
            f("macos", "/Applications/ShareXrs.app/Contents/MacOS/sharexrs", false),
            InstallFlow::MacApp { app: PathBuf::from("/Applications/ShareXrs.app") }
        );
        assert_eq!(f("macos", "/Applications/ShareXrs.app/Contents/MacOS/sharexrs", true), InstallFlow::MacDiskImage);
        assert_eq!(
            f("macos", "/private/var/folders/x/AppTranslocation/1234/d/ShareXrs.app/Contents/MacOS/sharexrs", false),
            InstallFlow::MacDiskImage
        );
        assert_eq!(f("macos", "/usr/local/bin/sharexrs", false), InstallFlow::MacDiskImage);
        assert_eq!(f("linux", "/usr/lib/sharexrs/sharexrs", false), InstallFlow::LinuxDeb);
        assert_eq!(
            f("linux", "/home/u/ShareXrs/sharexrs", false),
            InstallFlow::LinuxTarball { folder: PathBuf::from("/home/u/ShareXrs") }
        );
        assert!(f("windows", "x", true).is_portable());
        assert!(!f("linux", "/usr/lib/sharexrs/sharexrs", false).is_portable());
    }

    #[test]
    fn names_the_file() {
        let mut c = GitHubUpdateChecker::new("o", "r", InstallFlow::WindowsSetup);
        c.download_url = Some("https://x/dl/ShareXrs%20setup+1.exe".into());
        assert_eq!(c.file_name(), "ShareXrs setup 1.exe");
        c.set_file_name(Some("a.exe".into()));
        assert_eq!(c.file_name(), "a.exe");
    }
}
