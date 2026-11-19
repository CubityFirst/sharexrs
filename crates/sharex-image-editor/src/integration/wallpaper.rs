//! `IDesktopWallpaperService`'s macOS and Linux implementations:
//! `MacOSDesktopWallpaperService.cs` (System Events and `defaults`) and
//! `LinuxDesktopWallpaperService.cs` (GNOME, Cinnamon, MATE, Xfce, KDE,
//! LXQt and LXDE settings, Flatpak host paths, JPEG XL converted to PNG).
//!
//! Both are compiled on every platform so their logic is tested
//! everywhere; [`default_service`] picks the one for the running OS as
//! `EditorServices.EnsureDefaultDesktopWallpaperService` does (Windows'
//! lives in the app, beside its registry helpers). The processes and files
//! they read go through [`Host`], so tests can stand in for a desktop.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::render::background::Layout;

/// `DesktopWallpaperInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WallpaperInfo {
    pub path: PathBuf,
    pub layout: Layout,
}

/// `IDesktopWallpaperService`.
pub trait WallpaperService: Send + Sync {
    /// `IsSupported`.
    fn is_supported(&self) -> bool;
    /// `RequiresDesktopWallpaperPrewarm`.
    fn requires_prewarm(&self) -> bool;
    /// `TryGetDesktopWallpaper`.
    fn try_get(&self) -> Option<WallpaperInfo>;
    /// `PrewarmDesktopWallpaper`.
    fn prewarm(&self);
}

/// What the services read from the system: environment variables,
/// processes and files.
pub trait Host: Send + Sync {
    fn env(&self, name: &str) -> Option<String>;
    /// Runs `program` with `args` (no shell), its standard output read to
    /// the end and trimmed; `None` when it couldn't start or hadn't exited
    /// `timeout` after its output ended. The flag is a zero exit code.
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Option<(bool, String)>;
    fn file_exists(&self, path: &Path) -> bool;
    fn dir_exists(&self, path: &Path) -> bool;
    fn read_to_string(&self, path: &Path) -> Option<String>;
    /// The folders directly inside `path`.
    fn subdirs(&self, path: &Path) -> Vec<PathBuf>;
}

/// The real system.
pub struct SystemHost;

impl Host for SystemHost {
    fn env(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Option<(bool, String)> {
        run_process(program, args, timeout, true)
    }

    fn file_exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn dir_exists(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn read_to_string(&self, path: &Path) -> Option<String> {
        std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    fn subdirs(&self, path: &Path) -> Vec<PathBuf> {
        let Ok(rd) = std::fs::read_dir(path) else { return Vec::new() };
        rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect()
    }
}

/// `Process.Start` with redirected output, then `WaitForExit(timeout)`.
/// `read_output`: the output is read to its end first (the `TryReadCommandOutput`
/// helpers); otherwise only the exit is awaited (the converters).
fn run_process(program: &str, args: &[&str], timeout: Duration, read_output: bool) -> Option<(bool, String)> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(if read_output { Stdio::piped() } else { Stdio::null() })
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        out = String::from_utf8_lossy(&bytes).trim().to_owned();
    }
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some((status.success(), out)),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                // `HasExited` false: ShareX kills the converter, and reading
                // `ExitCode` of a running process throws.
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// `EnsureDefaultDesktopWallpaperService` outside Windows: macOS's or
/// Linux's service; `None` elsewhere (Windows' is the app's).
pub fn default_service() -> Option<Box<dyn WallpaperService>> {
    if cfg!(target_os = "macos") {
        Some(Box::new(MacOs::new(SystemHost)))
    } else if cfg!(target_os = "linux") {
        Some(Box::new(Linux::new(SystemHost)))
    } else {
        None
    }
}

/// `EditorServices.StartDesktopWallpaperPrewarm`: once per process, on a
/// thread of its own, when the service wants it (Linux: the JPEG XL
/// conversion cache).
pub fn start_prewarm(service: impl FnOnce() -> Option<Box<dyn WallpaperService>> + Send + 'static) {
    static STARTED: Mutex<bool> = Mutex::new(false);
    let mut started = STARTED.lock().unwrap_or_else(|e| e.into_inner());
    if *started {
        return;
    }
    *started = true;
    drop(started);
    std::thread::spawn(move || {
        if let Some(s) = service()
            && s.is_supported()
            && s.requires_prewarm()
        {
            eprintln!("[ImageEditor:MainViewModel] Starting wallpaper background prewarm.");
            s.prewarm();
            eprintln!("[ImageEditor:MainViewModel] Wallpaper background prewarm completed.");
        }
    });
}

// ----------------------------------------------------------------------
// macOS

/// `MacOSDesktopWallpaperService`.
pub struct MacOs<H: Host> {
    host: H,
}

/// The AppleScript that asks System Events for the wallpaper's path.
pub const MACOS_WALLPAPER_SCRIPT: &str =
    "tell application \"System Events\" to get POSIX path of (picture of current desktop as alias)";

impl<H: Host> MacOs<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }

    /// `TryRunProcess`: a zero exit and some output.
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        match self.host.run(program, args, Duration::from_millis(2000)) {
            Some((true, out)) if !out.trim().is_empty() => Some(out),
            _ => None,
        }
    }

    fn wallpaper_path(&self) -> Option<PathBuf> {
        // `osascript -e "<script>"`, the script escaped for the quotes.
        let out = self.run("osascript", &["-e", MACOS_WALLPAPER_SCRIPT])?;
        let path = PathBuf::from(out.trim());
        (!out.trim().is_empty() && self.host.file_exists(&path)).then_some(path)
    }

    fn wallpaper_layout(&self) -> Option<Layout> {
        let spaces = self.run("defaults", &["read", "com.apple.spaces"])?;
        let uuid = current_space_uuid(&spaces).filter(|u| !u.trim().is_empty())?;
        let desktop = self.run("defaults", &["read", "com.apple.desktop", "Background"])?;
        let block = desktop_settings_block(&desktop, &uuid)?;
        Some(map_macos_layout(assigned_value(&block, "Placement").as_deref(), assigned_value(&block, "ImageScaling").as_deref()))
    }
}

impl<H: Host> WallpaperService for MacOs<H> {
    fn is_supported(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn requires_prewarm(&self) -> bool {
        false
    }

    fn try_get(&self) -> Option<WallpaperInfo> {
        if !self.is_supported() {
            return None;
        }
        self.try_get_any_os()
    }

    fn prewarm(&self) {
        // Apple exposes wallpaper URLs and presentation options directly, so no conversion cache is needed here.
    }
}

impl<H: Host> MacOs<H> {
    /// `TryGetDesktopWallpaper` without the platform check (tests).
    pub fn try_get_any_os(&self) -> Option<WallpaperInfo> {
        let path = self.wallpaper_path()?;
        Some(WallpaperInfo { path, layout: self.wallpaper_layout().unwrap_or(Layout::Fill) })
    }
}

/// `MapDesktopWallpaperLayout`: `Placement`, else `ImageScaling`, else Fill.
pub fn map_macos_layout(placement: Option<&str>, scaling: Option<&str>) -> Layout {
    if let Some(p) = placement.filter(|p| !p.trim().is_empty()) {
        match p.trim() {
            "Center" => return Layout::Center,
            "Tile" => return Layout::Tile,
            "StretchToFillScreen" => return Layout::Stretch,
            "FitToScreen" => return Layout::Fit,
            "FillScreen" => return Layout::Fill,
            _ => {}
        }
    }
    if let Some(s) = scaling.filter(|s| !s.trim().is_empty()) {
        match s.trim() {
            "none" => return Layout::Center,
            "proportionally" | "proportionallyDown" => return Layout::Fit,
            "axesIndependently" => return Layout::Stretch,
            "proportionallyUpOrDown" => return Layout::Fill,
            _ => {}
        }
    }
    Layout::Fill
}

/// `TryGetCurrentSpaceUuid`: the first quoted `uuid =` after the
/// `Current Space` line of `defaults read com.apple.spaces`.
pub fn current_space_uuid(spaces: &str) -> Option<String> {
    let mut in_block = false;
    for raw in spaces.split('\n') {
        let line = raw.trim();
        if line.starts_with("\"Current Space\"") || line.starts_with("Current Space") {
            in_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        if line.starts_with("uuid =")
            && let (Some(first), Some(last)) = (line.find('"'), line.rfind('"'))
            && last > first
        {
            return Some(line[first + 1..last].to_owned());
        }
    }
    None
}

/// `TryExtractDesktopSettingsBlock`: the `default = { ... }` block inside
/// the space's entry of `defaults read com.apple.desktop Background`.
pub fn desktop_settings_block(desktop: &str, uuid: &str) -> Option<String> {
    let count = |s: &str, c: char| s.chars().filter(|&x| x == c).count() as i32;
    let mut block = String::new();
    let (mut in_uuid, mut in_default) = (false, false);
    let (mut depth, mut default_depth) = (0i32, 0i32);
    let key = format!("\"{uuid}\"");
    for raw in desktop.split('\n') {
        let line = raw.trim();
        if !in_uuid {
            if line.starts_with(&key) {
                in_uuid = true;
                depth += count(line, '{') - count(line, '}');
            }
            continue;
        }
        if !in_default && line.starts_with("default = {") {
            in_default = true;
            default_depth = depth + count(line, '{') - count(line, '}');
            block.push_str(line);
            block.push('\n');
            depth = default_depth;
            continue;
        }
        depth += count(line, '{') - count(line, '}');
        if in_default {
            block.push_str(line);
            block.push('\n');
            if depth < default_depth {
                return Some(block);
            }
        }
        if depth <= 0 {
            break;
        }
    }
    None
}

/// `TryReadAssignedValue`: `Key = value;`, unquoted.
pub fn assigned_value(block: &str, key: &str) -> Option<String> {
    let prefix = format!("{key} = ");
    block.split('\n').map(str::trim).find_map(|line| {
        let v = line.strip_prefix(&prefix)?;
        Some(v.trim().trim_end_matches(';').trim().trim_matches('"').to_owned())
    })
}

// ----------------------------------------------------------------------
// Linux

/// `SandboxHostRootPrefixes`: where Flatpak shows the host's files.
const SANDBOX_HOST_ROOTS: [&str; 2] = ["/run/host", "/var/run/host"];
/// `CommonExecutableDirectories`.
const COMMON_EXECUTABLE_DIRS: [&str; 3] = ["/usr/bin", "/bin", "/usr/local/bin"];
/// `WallpaperConversionCacheDirectoryName`.
pub const CONVERSION_CACHE_DIR: &str = "sharex-imageeditor-wallpaper-cache";
/// `WallpaperConverterTimeoutMilliseconds`.
const CONVERTER_TIMEOUT: Duration = Duration::from_millis(15000);
/// `GdkPixbufWallpaperSize`.
const THUMBNAILER_SIZE: &str = "4096";
/// `TryReadCommandOutput`'s `WaitForExit(2000)`.
const COMMAND_TIMEOUT: Duration = Duration::from_millis(2000);

/// `Provider`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Gnome,
    Cinnamon,
    Mate,
    Xfce,
    Kde,
    Lxqt,
    Lxde,
}

/// `LinuxDesktopWallpaperService`.
pub struct Linux<H: Host> {
    host: H,
    /// `WallpaperConversionLocks` (one lock: conversions only serialise).
    conversion: Mutex<()>,
}

impl<H: Host> WallpaperService for Linux<H> {
    fn is_supported(&self) -> bool {
        cfg!(target_os = "linux") && self.is_supported_any_os()
    }

    fn requires_prewarm(&self) -> bool {
        cfg!(target_os = "linux")
    }

    fn try_get(&self) -> Option<WallpaperInfo> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        self.try_get_any_os()
    }

    fn prewarm(&self) {
        let _ = self.try_get();
    }
}

impl<H: Host> Linux<H> {
    pub fn new(host: H) -> Self {
        Self { host, conversion: Mutex::new(()) }
    }

    /// `IsSupported` without the platform check.
    pub fn is_supported_any_os(&self) -> bool {
        self.preferred_providers().into_iter().any(|p| self.is_available(p))
    }

    /// `TryGetDesktopWallpaper` without the platform check.
    pub fn try_get_any_os(&self) -> Option<WallpaperInfo> {
        self.preferred_providers()
            .into_iter()
            .filter(|&p| self.is_available(p))
            .find_map(|p| self.provider_wallpaper(p))
    }

    /// `GetPreferredProviders`: the detected desktop's, then every other in
    /// ShareX's order.
    pub fn preferred_providers(&self) -> Vec<Provider> {
        let detected = detect_desktop(self.host.env("XDG_CURRENT_DESKTOP").as_deref(), self.host.env("KDE_FULL_SESSION").as_deref(), self.host.env("DESKTOP_SESSION").as_deref());
        preferred_providers(detected)
    }

    fn is_available(&self, p: Provider) -> bool {
        match p {
            Provider::Gnome => self.command_exists("gsettings") && self.gsettings_schema_exists("org.gnome.desktop.background"),
            Provider::Cinnamon => self.command_exists("gsettings") && self.gsettings_schema_exists("org.cinnamon.desktop.background"),
            Provider::Mate => self.command_exists("gsettings") && self.gsettings_schema_exists("org.mate.background"),
            Provider::Xfce => self.command_exists("xfconf-query"),
            Provider::Kde => self.kde_config_path().is_some_and(|p| self.host.file_exists(&p)),
            Provider::Lxqt => self.pcmanfm_roots("pcmanfm-qt").iter().any(|r| self.host.dir_exists(r)),
            Provider::Lxde => self.pcmanfm_roots("pcmanfm").iter().any(|r| self.host.dir_exists(r)),
        }
    }

    fn provider_wallpaper(&self, p: Provider) -> Option<WallpaperInfo> {
        match p {
            Provider::Gnome => self.gsettings_wallpaper("org.gnome.desktop.background", true),
            Provider::Cinnamon => self.gsettings_wallpaper("org.cinnamon.desktop.background", true),
            Provider::Mate => self.mate_wallpaper(),
            Provider::Xfce => self.xfce_wallpaper(),
            Provider::Kde => self.kde_wallpaper(),
            Provider::Lxqt => self.pcmanfm_wallpaper("pcmanfm-qt", &["lxqt", "default"], &["settings.conf"]),
            Provider::Lxde => self.pcmanfm_wallpaper("pcmanfm", &["LXDE", "default", "lxde"], &["pcmanfm.conf", "desktop-items-0.conf"]),
        }
    }

    /// `TryReadCommandOutput`: a zero exit (the output may be empty).
    fn command_output(&self, program: &str, args: &[&str]) -> Option<String> {
        match self.host.run(program, args, COMMAND_TIMEOUT) {
            Some((true, out)) => Some(out),
            _ => None,
        }
    }

    /// `TryReadGSettingsValue`: `gsettings get <schema> <key>`, unquoted;
    /// empty and `nothing` don't count.
    fn gsettings_value(&self, schema: &str, key: &str) -> Option<String> {
        let out = self.command_output("gsettings", &["get", schema, key])?;
        let v = out.trim().trim_matches(['\'', '"']).to_owned();
        (!v.trim().is_empty() && !v.eq_ignore_ascii_case("nothing")).then_some(v)
    }

    fn gsettings_schema_exists(&self, schema: &str) -> bool {
        self.command_output("gsettings", &["list-schemas"]).is_some_and(|s| s.split('\n').map(str::trim).any(|l| l == schema))
    }

    /// `IsDarkWallpaperPreferred`.
    fn dark_preferred(&self) -> bool {
        self.gsettings_value("org.gnome.desktop.interface", "color-scheme").is_some_and(|v| v.eq_ignore_ascii_case("prefer-dark"))
    }

    /// `TryGetGSettingsWallpaper`.
    fn gsettings_wallpaper(&self, schema: &str, allow_dark: bool) -> Option<WallpaperInfo> {
        let mut picture = None;
        if allow_dark && self.dark_preferred() {
            picture = self.gsettings_value(schema, "picture-uri-dark");
        }
        if picture.as_deref().is_none_or(|p| p.trim().is_empty()) {
            picture = Some(self.gsettings_value(schema, "picture-uri")?);
        }
        let path = self.resolve_accessible(parse_wallpaper_path(picture.as_deref()).as_deref())?;
        let layout = self.gsettings_value(schema, "picture-options").map_or(Layout::Fill, |o| map_gsettings_options(&o));
        Some(WallpaperInfo { path, layout })
    }

    /// `TryGetMateWallpaper`.
    fn mate_wallpaper(&self) -> Option<WallpaperInfo> {
        let picture = self.gsettings_value("org.mate.background", "picture-filename")?;
        let path = self.resolve_accessible(parse_wallpaper_path(Some(&picture)).as_deref())?;
        let layout = self.gsettings_value("org.mate.background", "picture-options").map_or(Layout::Fill, |o| map_gsettings_options(&o));
        Some(WallpaperInfo { path, layout })
    }

    /// `TryGetXfceWallpaper`.
    fn xfce_wallpaper(&self) -> Option<WallpaperInfo> {
        let list = self.command_output("xfconf-query", &["-c", "xfce4-desktop", "-l"])?;
        for property in xfce_candidate_properties(&list) {
            let Some(value) = self.command_output("xfconf-query", &["-c", "xfce4-desktop", "-p", &property]) else { continue };
            let Some(path) = self.resolve_accessible(parse_wallpaper_path(Some(&value)).as_deref()) else { continue };
            let style = xfce_style_property(&property);
            let layout = self.command_output("xfconf-query", &["-c", "xfce4-desktop", "-p", &style]).map_or(Layout::Fill, |v| map_xfce_style(&v));
            return Some(WallpaperInfo { path, layout });
        }
        None
    }

    /// `GetKdeWallpaperConfigPath`: `~/.config/plasma-org.kde.plasma.desktop-appletsrc`.
    fn kde_config_path(&self) -> Option<PathBuf> {
        let home = self.host.env("HOME")?;
        Some(ujoin(&ujoin(Path::new(&home), ".config"), "plasma-org.kde.plasma.desktop-appletsrc"))
    }

    /// `TryGetKdeWallpaper`.
    fn kde_wallpaper(&self) -> Option<WallpaperInfo> {
        let config = self.kde_config_path()?;
        if !self.host.file_exists(&config) {
            return None;
        }
        let text = self.host.read_to_string(&config)?;
        let (path, fill_mode) = parse_kde_config(&text, |raw| self.resolve_accessible(parse_wallpaper_path(Some(raw)).as_deref()), |p| self.host.file_exists(p));
        let path = self.resolve_accessible(path.as_ref().and_then(|p| p.to_str()))?;
        Some(WallpaperInfo { path, layout: map_kde_fill_mode(fill_mode.as_deref()) })
    }

    /// `EnumeratePcmanfmRoots`.
    pub fn pcmanfm_roots(&self, app: &str) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        // `Environment.SpecialFolder.ApplicationData`: $XDG_CONFIG_HOME or ~/.config.
        let config = self
            .host
            .env("XDG_CONFIG_HOME")
            .filter(|c| c.starts_with('/'))
            .map(PathBuf::from)
            .or_else(|| self.host.env("HOME").map(|h| ujoin(Path::new(&h), ".config")));
        if let Some(c) = config {
            roots.push(ujoin(&c, app));
        }
        for prefix in ["/etc/xdg", "/run/host/etc/xdg", "/var/run/host/etc/xdg"] {
            roots.push(ujoin(Path::new(prefix), app));
        }
        roots
    }

    /// `EnumeratePcmanfmConfigFiles`.
    pub fn pcmanfm_config_files(&self, app: &str, profiles: &[&str], names: &[&str]) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let add = |f: PathBuf, out: &mut Vec<PathBuf>| {
            if self.host.file_exists(&f) && !out.contains(&f) {
                out.push(f);
            }
        };
        for root in self.pcmanfm_roots(app) {
            if !self.host.dir_exists(&root) {
                continue;
            }
            for profile in profiles {
                for name in names {
                    add(ujoin(&ujoin(&root, profile), name), &mut out);
                }
            }
            let mut dirs = self.host.subdirs(&root);
            dirs.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
            for dir in dirs {
                for name in names {
                    add(ujoin(&dir, name), &mut out);
                }
            }
        }
        out
    }

    /// `TryGetPcmanfmWallpaper`.
    fn pcmanfm_wallpaper(&self, app: &str, profiles: &[&str], names: &[&str]) -> Option<WallpaperInfo> {
        self.pcmanfm_config_files(app, profiles, names).into_iter().find_map(|file| {
            let text = self.host.read_to_string(&file)?;
            let (raw_path, raw_layout) = parse_pcmanfm_config(&text);
            let path = self.resolve_accessible(parse_wallpaper_path(raw_path.as_deref()).as_deref())?;
            Some(WallpaperInfo { path, layout: map_pcmanfm_mode(raw_layout.as_deref()) })
        })
    }

    /// `ResolveAccessibleWallpaperPath`: the path, or its Flatpak host
    /// mirror, that exists; JPEG XL converted to a cached PNG.
    fn resolve_accessible(&self, path: Option<&str>) -> Option<PathBuf> {
        let path = path.filter(|p| !p.trim().is_empty())?;
        let found = accessible_candidates(path).into_iter().find(|c| self.host.file_exists(c))?;
        if !requires_conversion(&found) {
            return Some(found);
        }
        self.convert(&found)
    }

    /// `TryResolveCommandPath`: `sh -lc "command -v <name>"`, then `PATH`,
    /// then the usual folders.
    pub fn resolve_command(&self, command: &str) -> Option<PathBuf> {
        if command.trim().is_empty() {
            return None;
        }
        if command.starts_with('/') {
            let p = PathBuf::from(command);
            return self.host.file_exists(&p).then_some(p);
        }
        if let Some(out) = self.command_output("sh", &["-lc", &format!("command -v {command}")])
            && let Some(first) = out.split('\n').map(str::trim).find(|l| !l.is_empty())
        {
            return Some(PathBuf::from(first));
        }
        if let Some(path) = self.host.env("PATH") {
            for dir in path.split(':').map(str::trim).filter(|d| !d.is_empty()) {
                let c = ujoin(Path::new(dir), command);
                if self.host.file_exists(&c) {
                    return Some(c);
                }
            }
        }
        COMMON_EXECUTABLE_DIRS.iter().map(|d| ujoin(Path::new(d), command)).find(|c| self.host.file_exists(c))
    }

    fn command_exists(&self, command: &str) -> bool {
        self.resolve_command(command).is_some()
    }

    /// `TryConvertWallpaper`: the cached PNG, else FFmpeg, glycin's
    /// thumbnailer or GdkPixbuf's, into the cache.
    fn convert(&self, source: &Path) -> Option<PathBuf> {
        let cache = conversion_cache_path(source)?;
        let _guard = self.conversion.lock().unwrap_or_else(|e| e.into_inner());
        if cache.is_file() {
            return Some(cache);
        }
        let tmp = temporary_converted_path(&cache);
        for (tool, args) in converter_commands(source, &tmp) {
            let Some(exe) = self.resolve_command(tool) else { continue };
            if run_converter(&exe, &args, &tmp, &cache) {
                return Some(cache);
            }
        }
        None
    }
}

/// `TryRunWallpaperConverter`: into the temporary file, moved over the
/// cache file on a zero exit; the temporary file never stays.
fn run_converter(exe: &Path, args: &[String], tmp: &Path, out: &Path) -> bool {
    let ok = (|| {
        std::fs::create_dir_all(out.parent()?).ok()?;
        if tmp.is_file() {
            std::fs::remove_file(tmp).ok()?;
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let (success, _) = run_process(exe.to_str()?, &args, CONVERTER_TIMEOUT, false)?;
        if !success || !tmp.is_file() {
            return None;
        }
        std::fs::rename(tmp, out).ok()
    })()
    .is_some();
    if tmp.is_file() {
        let _ = std::fs::remove_file(tmp);
    }
    ok
}

/// The converters ShareX tries, in order, with their arguments.
pub fn converter_commands(source: &Path, tmp: &Path) -> Vec<(&'static str, Vec<String>)> {
    let (src, tmp) = (source.to_string_lossy().into_owned(), tmp.to_string_lossy().into_owned());
    vec![
        ("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", "-i", &src, "-frames:v", "1", &tmp].map(String::from).to_vec()),
        ("glycin-thumbnailer", vec!["--input".into(), path_to_file_uri(&src), "--output".into(), tmp.clone(), "--size".into(), THUMBNAILER_SIZE.into()]),
        ("gdk-pixbuf-thumbnailer", ["-s", THUMBNAILER_SIZE, &src, &tmp].map(String::from).to_vec()),
    ]
}

/// `DetectDesktopEnvironment`: `XDG_CURRENT_DESKTOP`, `KDE_FULL_SESSION`,
/// then `DESKTOP_SESSION`.
pub fn detect_desktop(current_desktop: Option<&str>, kde_full_session: Option<&str>, desktop_session: Option<&str>) -> Option<Provider> {
    const TOKENS: [(&str, Provider); 7] = [
        ("GNOME", Provider::Gnome),
        ("CINNAMON", Provider::Cinnamon),
        ("MATE", Provider::Mate),
        ("XFCE", Provider::Xfce),
        ("KDE", Provider::Kde),
        ("LXQT", Provider::Lxqt),
        ("LXDE", Provider::Lxde),
    ];
    let find = |v: Option<&str>| {
        let v = v.filter(|v| !v.trim().is_empty())?.to_uppercase();
        TOKENS.iter().find(|(t, _)| v.contains(t)).map(|&(_, p)| p)
    };
    find(current_desktop)
        .or_else(|| kde_full_session.filter(|v| v.eq_ignore_ascii_case("true")).map(|_| Provider::Kde))
        .or_else(|| find(desktop_session))
}

/// `GetPreferredProviders` from the detected desktop (Cinnamon also
/// tries GNOME's schema next).
pub fn preferred_providers(detected: Option<Provider>) -> Vec<Provider> {
    let mut out = Vec::new();
    match detected {
        Some(Provider::Cinnamon) => out.extend([Provider::Cinnamon, Provider::Gnome]),
        Some(p) => out.push(p),
        None => {}
    }
    for p in [Provider::Gnome, Provider::Cinnamon, Provider::Mate, Provider::Xfce, Provider::Kde, Provider::Lxqt, Provider::Lxde] {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// `MapGSettingsPictureOptions`.
pub fn map_gsettings_options(option: &str) -> Layout {
    match option.trim().trim_matches(['\'', '"']).to_lowercase().as_str() {
        "wallpaper" => Layout::Tile,
        "centered" => Layout::Center,
        "scaled" => Layout::Fit,
        "stretched" => Layout::Stretch,
        "spanned" => Layout::Span,
        _ => Layout::Fill,
    }
}

/// `MapXfceStyle`.
pub fn map_xfce_style(style: &str) -> Layout {
    match style.trim().parse::<i32>() {
        Ok(1) => Layout::Center,
        Ok(2) => Layout::Tile,
        Ok(3) => Layout::Stretch,
        Ok(4) => Layout::Fit,
        Ok(6) => Layout::Span,
        _ => Layout::Fill,
    }
}

/// `MapKdeFillMode`: Plasma's numbers, or the names.
pub fn map_kde_fill_mode(mode: Option<&str>) -> Layout {
    let Some(mode) = mode else { return Layout::Fill };
    if let Ok(n) = mode.trim().parse::<i32>() {
        return match n {
            0 => Layout::Stretch,
            1 => Layout::Fit,
            3..=5 => Layout::Tile,
            6 => Layout::Center,
            _ => Layout::Fill,
        };
    }
    match mode.trim().to_lowercase().as_str() {
        "stretch" => Layout::Stretch,
        "preserveaspectfit" => Layout::Fit,
        "tile" => Layout::Tile,
        "pad" => Layout::Center,
        _ => Layout::Fill,
    }
}

/// `MapPcmanfmWallpaperMode`.
pub fn map_pcmanfm_mode(mode: Option<&str>) -> Layout {
    match mode.unwrap_or_default().trim().trim_matches(['\'', '"']).to_lowercase().as_str() {
        "center" | "centered" => Layout::Center,
        "tile" | "tiled" => Layout::Tile,
        "stretch" | "stretched" => Layout::Stretch,
        "fit" | "scaled" => Layout::Fit,
        _ => Layout::Fill,
    }
}

/// `ParseWallpaperPath`: unquoted; `none` is no wallpaper; a `file://`
/// URI becomes its local path.
pub fn parse_wallpaper_path(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim().trim_matches(['\'', '"']);
    if value.trim().is_empty() || value.eq_ignore_ascii_case("none") {
        return None;
    }
    if value.len() >= 7 && value[..7].eq_ignore_ascii_case("file://")
        && let Some(local) = file_uri_local_path(value)
    {
        return Some(local);
    }
    Some(value.to_owned())
}

/// `Uri.LocalPath` of a `file://` URI on Unix: the path after the host
/// (`localhost` or none), percent-decoded.
fn file_uri_local_path(uri: &str) -> Option<String> {
    let rest = &uri[7..];
    let path = if rest.starts_with('/') {
        rest
    } else {
        let slash = rest.find('/')?;
        let host = &rest[..slash];
        if !host.eq_ignore_ascii_case("localhost") {
            return None;
        }
        &rest[slash..]
    };
    // A query or fragment isn't part of the path.
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// `PathToFileUri`: `new Uri(path).AbsoluteUri`.
pub fn path_to_file_uri(path: &str) -> String {
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~!$&'()*+,;=:@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `GetAccessiblePathCandidates`: the path, then (for a rooted path that
/// isn't one already) its mirrors under the Flatpak host roots.
pub fn accessible_candidates(path: &str) -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(path)];
    if path.trim().is_empty() || !path.starts_with('/') || is_sandbox_host_mirror(path) {
        return out;
    }
    let relative = path.trim_start_matches('/');
    out.extend(SANDBOX_HOST_ROOTS.iter().map(|r| ujoin(Path::new(r), relative)));
    out
}

/// `Path.Combine` on Unix: `a/b` (the Linux paths are built the same on
/// every OS, so the tests see what Linux sees).
fn ujoin(a: &Path, b: &str) -> PathBuf {
    let a = a.to_string_lossy();
    if a.is_empty() {
        PathBuf::from(b)
    } else if a.ends_with('/') {
        PathBuf::from(format!("{a}{b}"))
    } else {
        PathBuf::from(format!("{a}/{b}"))
    }
}

/// `IsSandboxHostMirrorPath`.
fn is_sandbox_host_mirror(path: &str) -> bool {
    SANDBOX_HOST_ROOTS.iter().any(|r| path == *r || path.starts_with(&format!("{r}/")))
}

/// `RequiresWallpaperConversion`: JPEG XL.
pub fn requires_conversion(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("jxl"))
}

/// `GetWallpaperConversionCachePath`: `<temp>/sharex-imageeditor-wallpaper-cache/<name>-<sha256>.png`,
/// the hash of `path|length|last write ticks`. Creates the folder.
pub fn conversion_cache_path(source: &Path) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(CONVERSION_CACHE_DIR);
    std::fs::create_dir_all(&dir).ok()?;
    let meta = std::fs::metadata(source).ok()?;
    let modified = meta.modified().ok()?;
    Some(dir.join(conversion_cache_file_name(source, meta.len(), modified)))
}

/// The cache file's name for a source of `length` bytes written at `modified`.
pub fn conversion_cache_file_name(source: &Path, length: u64, modified: std::time::SystemTime) -> String {
    use sha2::{Digest, Sha256};
    let key = format!("{}|{}|{}", source.to_string_lossy(), length, dotnet_ticks(modified));
    let hash: String = Sha256::digest(key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "wallpaper".into());
    format!("{stem}-{hash}.png")
}

/// `DateTime.Ticks` of a UTC time: 100 ns since 0001-01-01.
pub fn dotnet_ticks(t: std::time::SystemTime) -> i64 {
    const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => UNIX_EPOCH_TICKS + (d.as_nanos() / 100) as i64,
        Err(e) => UNIX_EPOCH_TICKS - (e.duration().as_nanos() / 100) as i64,
    }
}

/// `GetTemporaryConvertedWallpaperPath`: `<name>.tmp.png` beside it.
pub fn temporary_converted_path(out: &Path) -> PathBuf {
    let dir = out.parent().filter(|d| !d.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(std::env::temp_dir);
    let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "wallpaper".into());
    dir.join(format!("{stem}.tmp.png"))
}

/// The `xfconf-query -l` properties that name an image, the first
/// workspace's first, then in ordinal order.
pub fn xfce_candidate_properties(list: &str) -> Vec<String> {
    let mut props: Vec<String> = list
        .split('\n')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| l.ends_with("/last-image") || l.ends_with("/image-path"))
        .map(str::to_owned)
        .collect();
    props.sort_by(|a, b| {
        let w = |s: &str| if s.to_lowercase().contains("workspace0") { 0 } else { 1 };
        w(a).cmp(&w(b)).then_with(|| a.as_bytes().cmp(b.as_bytes()))
    });
    props
}

/// The `image-style` property beside an image property.
pub fn xfce_style_property(property: &str) -> String {
    let base = property.strip_suffix("/last-image").or_else(|| property.strip_suffix("/image-path")).unwrap_or(property);
    format!("{base}/image-style")
}

/// The KDE loop of `TryGetKdeWallpaper`: `Image` and `FillMode` from
/// `[...][Wallpaper][org.kde.image][General]` sections, stopping at the
/// next section once an existing image was found. `resolve` is
/// `ResolveAccessibleWallpaperPath(ParseWallpaperPath(value))`.
pub fn parse_kde_config(
    text: &str,
    resolve: impl Fn(&str) -> Option<PathBuf>,
    exists: impl Fn(&Path) -> bool,
) -> (Option<PathBuf>, Option<String>) {
    let (mut path, mut fill_mode): (Option<PathBuf>, Option<String>) = (None, None);
    let mut in_section = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if path.as_ref().is_some_and(|p| exists(p)) {
                break;
            }
            in_section = line.contains("Wallpaper][org.kde.image][General]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(eq) = line.find('=').filter(|&i| i > 0) else { continue };
        let (key, value) = (line[..eq].trim(), line[eq + 1..].trim());
        if key == "Image" {
            if let Some(c) = resolve(value) {
                path = Some(c);
            }
        } else if key == "FillMode" {
            fill_mode = Some(value.to_owned());
        }
    }
    (path, fill_mode)
}

/// `TryReadPcmanfmWallpaper`'s reading: the last `wallpaper` and
/// `wallpaper_mode`/`wallpapermode` values, sections ignored.
pub fn parse_pcmanfm_config(text: &str) -> (Option<String>, Option<String>) {
    let (mut path, mut mode) = (None, None);
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(['#', ';', '[']) {
            continue;
        }
        let Some(eq) = line.find('=').filter(|&i| i > 0) else { continue };
        let (key, value) = (line[..eq].trim(), line[eq + 1..].trim());
        if key.eq_ignore_ascii_case("wallpaper") {
            path = Some(value.to_owned());
        } else if key.eq_ignore_ascii_case("wallpaper_mode") || key.eq_ignore_ascii_case("wallpapermode") {
            mode = Some(value.to_owned());
        }
    }
    (path, mode)
}

#[cfg(test)]
mod tests;
