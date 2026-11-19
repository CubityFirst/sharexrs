use std::collections::{HashMap, HashSet};

use super::*;

/// A desktop made of environment variables, command answers and files.
#[derive(Default)]
struct Fake {
    env: HashMap<String, String>,
    /// `program arg arg` -> (exit 0, output).
    commands: HashMap<String, (bool, String)>,
    files: HashMap<PathBuf, String>,
    dirs: HashSet<PathBuf>,
    ran: Mutex<Vec<String>>,
}

impl Fake {
    fn env(mut self, k: &str, v: &str) -> Self {
        self.env.insert(k.into(), v.into());
        self
    }
    fn cmd(mut self, line: &str, out: &str) -> Self {
        self.commands.insert(line.into(), (true, out.into()));
        self
    }
    fn file(mut self, p: &str, text: &str) -> Self {
        self.files.insert(PathBuf::from(p), text.into());
        let mut d = Path::new(p).parent();
        while let Some(dir) = d {
            self.dirs.insert(dir.to_path_buf());
            d = dir.parent();
        }
        self
    }
    /// `sh -lc "command -v <tool>"` finds it in /usr/bin.
    fn tool(self, name: &str) -> Self {
        let line = format!("sh -lc command -v {name}");
        self.cmd(&line, &format!("/usr/bin/{name}"))
    }
}

impl Host for Fake {
    fn env(&self, name: &str) -> Option<String> {
        self.env.get(name).cloned()
    }
    fn run(&self, program: &str, args: &[&str], _: Duration) -> Option<(bool, String)> {
        let line = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        self.ran.lock().unwrap().push(line.clone());
        Some(self.commands.get(&line).cloned().unwrap_or((false, String::new())))
    }
    fn file_exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }
    fn dir_exists(&self, path: &Path) -> bool {
        self.dirs.contains(path)
    }
    fn read_to_string(&self, path: &Path) -> Option<String> {
        self.files.get(path).cloned()
    }
    fn subdirs(&self, path: &Path) -> Vec<PathBuf> {
        self.dirs.iter().filter(|d| d.parent() == Some(path)).cloned().collect()
    }
}

fn info(path: &str, layout: Layout) -> Option<WallpaperInfo> {
    Some(WallpaperInfo { path: PathBuf::from(path), layout })
}

#[test]
fn desktop_detection_follows_sharex_order() {
    assert_eq!(detect_desktop(Some("ubuntu:GNOME"), None, None), Some(Provider::Gnome));
    assert_eq!(detect_desktop(Some("X-Cinnamon"), None, None), Some(Provider::Cinnamon));
    // GNOME is looked for first, so "GNOME-Flashback:Unity" is GNOME.
    assert_eq!(detect_desktop(Some("Unity:GNOME-Flashback"), None, None), Some(Provider::Gnome));
    assert_eq!(detect_desktop(Some("kde"), None, None), Some(Provider::Kde));
    assert_eq!(detect_desktop(None, Some("TRUE"), Some("xfce")), Some(Provider::Kde));
    assert_eq!(detect_desktop(Some(" "), None, Some("Lubuntu-LXQt")), Some(Provider::Lxqt));
    assert_eq!(detect_desktop(Some("sway"), Some("false"), Some("sway")), None);
}

#[test]
fn providers_put_the_detected_desktop_first() {
    use Provider::*;
    assert_eq!(preferred_providers(None), vec![Gnome, Cinnamon, Mate, Xfce, Kde, Lxqt, Lxde]);
    assert_eq!(preferred_providers(Some(Kde)), vec![Kde, Gnome, Cinnamon, Mate, Xfce, Lxqt, Lxde]);
    assert_eq!(preferred_providers(Some(Cinnamon)), vec![Cinnamon, Gnome, Mate, Xfce, Kde, Lxqt, Lxde]);
}

#[test]
fn layout_maps_match_sharex() {
    assert_eq!(map_gsettings_options("'wallpaper'"), Layout::Tile);
    assert_eq!(map_gsettings_options("centered"), Layout::Center);
    assert_eq!(map_gsettings_options("'scaled'"), Layout::Fit);
    assert_eq!(map_gsettings_options("stretched"), Layout::Stretch);
    assert_eq!(map_gsettings_options("zoom"), Layout::Fill);
    assert_eq!(map_gsettings_options("spanned"), Layout::Span);
    assert_eq!(map_gsettings_options("none"), Layout::Fill);
    assert_eq!(map_xfce_style("1"), Layout::Center);
    assert_eq!(map_xfce_style("5"), Layout::Fill);
    assert_eq!(map_xfce_style("6"), Layout::Span);
    assert_eq!(map_xfce_style("0"), Layout::Fill);
    assert_eq!(map_xfce_style("x"), Layout::Fill);
    assert_eq!(map_kde_fill_mode(Some("0")), Layout::Stretch);
    assert_eq!(map_kde_fill_mode(Some("1")), Layout::Fit);
    assert_eq!(map_kde_fill_mode(Some("4")), Layout::Tile);
    assert_eq!(map_kde_fill_mode(Some("6")), Layout::Center);
    assert_eq!(map_kde_fill_mode(Some("PreserveAspectFit")), Layout::Fit);
    assert_eq!(map_kde_fill_mode(Some("pad")), Layout::Center);
    assert_eq!(map_kde_fill_mode(None), Layout::Fill);
    assert_eq!(map_pcmanfm_mode(Some("tiled")), Layout::Tile);
    assert_eq!(map_pcmanfm_mode(Some("'fit'")), Layout::Fit);
    assert_eq!(map_pcmanfm_mode(Some("crop")), Layout::Fill);
    assert_eq!(map_macos_layout(Some("Center"), Some("proportionally")), Layout::Center);
    assert_eq!(map_macos_layout(Some("Unknown"), Some("proportionally")), Layout::Fit);
    assert_eq!(map_macos_layout(None, Some("axesIndependently")), Layout::Stretch);
    assert_eq!(map_macos_layout(Some(" "), Some("none")), Layout::Center);
    assert_eq!(map_macos_layout(None, None), Layout::Fill);
}

#[test]
fn wallpaper_paths_parse_as_sharex() {
    assert_eq!(parse_wallpaper_path(Some("'file:///usr/share/backgrounds/a%20b.png'")).as_deref(), Some("/usr/share/backgrounds/a b.png"));
    assert_eq!(parse_wallpaper_path(Some("file://localhost/x.jpg")).as_deref(), Some("/x.jpg"));
    assert_eq!(parse_wallpaper_path(Some("/home/u/w.png")).as_deref(), Some("/home/u/w.png"));
    assert_eq!(parse_wallpaper_path(Some("'none'")), None);
    assert_eq!(parse_wallpaper_path(Some("  ")), None);
    assert_eq!(parse_wallpaper_path(None), None);
    assert_eq!(path_to_file_uri("/home/u/my wall#1.jxl"), "file:///home/u/my%20wall%231.jxl");
}

#[test]
fn flatpak_host_mirrors_are_candidates() {
    assert_eq!(
        accessible_candidates("/usr/share/w.png"),
        vec![PathBuf::from("/usr/share/w.png"), PathBuf::from("/run/host/usr/share/w.png"), PathBuf::from("/var/run/host/usr/share/w.png")]
    );
    assert_eq!(accessible_candidates("/run/host/usr/w.png"), vec![PathBuf::from("/run/host/usr/w.png")]);
    assert_eq!(accessible_candidates("relative.png"), vec![PathBuf::from("relative.png")]);
}

#[test]
fn conversion_cache_names_hash_path_length_and_ticks() {
    let t = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    assert_eq!(dotnet_ticks(t), 621_355_968_000_000_000 + 17_000_000_000_000_000);
    let name = conversion_cache_file_name(Path::new("/w/Sky.jxl"), 1234, t);
    use sha2::{Digest, Sha256};
    let expected: String = Sha256::digest(b"/w/Sky.jxl|1234|638355968000000000").iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(name, format!("Sky-{expected}.png"));
    assert_eq!(temporary_converted_path(Path::new("/tmp/c/Sky-ab.png")), PathBuf::from("/tmp/c/Sky-ab.tmp.png"));
    assert!(requires_conversion(Path::new("/a/b.JXL")));
    assert!(!requires_conversion(Path::new("/a/b.png")));
}

#[test]
fn converters_run_in_sharex_order_with_its_arguments() {
    let c = converter_commands(Path::new("/w/a b.jxl"), Path::new("/tmp/x.tmp.png"));
    assert_eq!(c[0].0, "ffmpeg");
    assert_eq!(c[0].1, ["-hide_banner", "-loglevel", "error", "-y", "-i", "/w/a b.jxl", "-frames:v", "1", "/tmp/x.tmp.png"]);
    assert_eq!(c[1].0, "glycin-thumbnailer");
    assert_eq!(c[1].1, ["--input", "file:///w/a%20b.jxl", "--output", "/tmp/x.tmp.png", "--size", "4096"]);
    assert_eq!(c[2].0, "gdk-pixbuf-thumbnailer");
    assert_eq!(c[2].1, ["-s", "4096", "/w/a b.jxl", "/tmp/x.tmp.png"]);
}

#[test]
fn gnome_reads_the_dark_picture_when_preferred() {
    let fake = Fake::default()
        .env("XDG_CURRENT_DESKTOP", "GNOME")
        .tool("gsettings")
        .cmd("gsettings list-schemas", "org.gnome.desktop.interface\norg.gnome.desktop.background\n")
        .cmd("gsettings get org.gnome.desktop.interface color-scheme", "'prefer-dark'")
        .cmd("gsettings get org.gnome.desktop.background picture-uri-dark", "'file:///usr/share/backgrounds/d.png'")
        .cmd("gsettings get org.gnome.desktop.background picture-uri", "'file:///usr/share/backgrounds/l.png'")
        .cmd("gsettings get org.gnome.desktop.background picture-options", "'spanned'")
        .file("/usr/share/backgrounds/d.png", "")
        .file("/usr/share/backgrounds/l.png", "");
    let s = Linux::new(fake);
    assert!(s.is_supported_any_os());
    assert_eq!(s.try_get_any_os(), info("/usr/share/backgrounds/d.png", Layout::Span));
}

#[test]
fn gnome_light_picture_through_the_flatpak_mirror() {
    let fake = Fake::default()
        .tool("gsettings")
        .cmd("gsettings list-schemas", "org.gnome.desktop.background")
        .cmd("gsettings get org.gnome.desktop.interface color-scheme", "'default'")
        .cmd("gsettings get org.gnome.desktop.background picture-uri", "'/usr/share/w.jpg'")
        .file("/run/host/usr/share/w.jpg", "");
    assert_eq!(Linux::new(fake).try_get_any_os(), info("/run/host/usr/share/w.jpg", Layout::Fill));
}

#[test]
fn nothing_found_is_unsupported() {
    let fake = Fake::default().env("HOME", "/home/u").env("PATH", "/usr/bin");
    let s = Linux::new(fake);
    assert!(!s.is_supported_any_os());
    assert_eq!(s.try_get_any_os(), None);
}

#[test]
fn commands_are_found_on_path_then_in_the_usual_folders() {
    let fake = Fake::default().env("PATH", "/opt/a:/opt/b").file("/opt/b/xfconf-query", "").file("/usr/local/bin/gsettings", "");
    let s = Linux::new(fake);
    assert_eq!(s.resolve_command("xfconf-query"), Some(PathBuf::from("/opt/b/xfconf-query")));
    assert_eq!(s.resolve_command("gsettings"), Some(PathBuf::from("/usr/local/bin/gsettings")));
    assert_eq!(s.resolve_command("nope"), None);
}

#[test]
fn xfce_prefers_workspace_zero() {
    let list = "/backdrop/screen0/monitorDP-1/workspace1/last-image\n/backdrop/screen0/monitorDP-1/workspace0/last-image\n/backdrop/screen0/monitorDP-1/workspace0/image-style\n/backdrop/screen0/monitor0/image-path";
    assert_eq!(
        xfce_candidate_properties(list),
        vec![
            "/backdrop/screen0/monitorDP-1/workspace0/last-image".to_string(),
            "/backdrop/screen0/monitor0/image-path".into(),
            "/backdrop/screen0/monitorDP-1/workspace1/last-image".into(),
        ]
    );
    assert_eq!(xfce_style_property("/a/workspace0/last-image"), "/a/workspace0/image-style");
    let fake = Fake::default()
        .env("XDG_CURRENT_DESKTOP", "XFCE")
        .tool("xfconf-query")
        .cmd("xfconf-query -c xfce4-desktop -l", list)
        .cmd("xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitorDP-1/workspace0/last-image", "/w/x.png")
        .cmd("xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitorDP-1/workspace0/image-style", "3")
        .file("/w/x.png", "");
    assert_eq!(Linux::new(fake).try_get_any_os(), info("/w/x.png", Layout::Stretch));
}

#[test]
fn kde_reads_the_plasma_applets_file() {
    let config = "[Containments][1]\nwallpaperplugin=org.kde.image\n\n[Containments][1][Wallpaper][org.kde.image][General]\nImage=file:///home/u/Pictures/k.png\nFillMode=1\n\n[Containments][2][Wallpaper][org.kde.image][General]\nImage=/home/u/other.png\n";
    let fake = Fake::default()
        .env("XDG_CURRENT_DESKTOP", "KDE")
        .env("HOME", "/home/u")
        .file("/home/u/.config/plasma-org.kde.plasma.desktop-appletsrc", config)
        .file("/home/u/Pictures/k.png", "")
        .file("/home/u/other.png", "");
    assert_eq!(Linux::new(fake).try_get_any_os(), info("/home/u/Pictures/k.png", Layout::Fit));
}

#[test]
fn kde_keeps_looking_past_a_missing_image() {
    let config = "[A][Wallpaper][org.kde.image][General]\nImage=/gone.png\nFillMode=6\n[B][Wallpaper][org.kde.image][General]\nImage=/here.png\n";
    let (path, mode) = parse_kde_config(config, |v| (v == "/here.png").then(|| PathBuf::from(v)), |p| p == Path::new("/here.png"));
    // FillMode carries over from the earlier section, as in ShareX.
    assert_eq!((path, mode.as_deref()), (Some(PathBuf::from("/here.png")), Some("6")));
}

#[test]
fn lxqt_reads_pcmanfm_qt_settings() {
    let fake = Fake::default()
        .env("XDG_CURRENT_DESKTOP", "LXQt")
        .env("HOME", "/home/u")
        .file("/home/u/.config/pcmanfm-qt/lxqt/settings.conf", "[Desktop]\nWallpaper=/usr/share/lxqt/w.png\nWallpaperMode=zoom\n");
    let fake = fake.file("/usr/share/lxqt/w.png", "");
    assert_eq!(Linux::new(fake).try_get_any_os(), info("/usr/share/lxqt/w.png", Layout::Fill));
    assert_eq!(parse_pcmanfm_config("; c\nwallpaper_mode=center\nwallpaper=/a.png\n"), (Some("/a.png".into()), Some("center".into())));
}

#[test]
fn lxde_profiles_then_other_folders() {
    let fake = Fake::default()
        .env("HOME", "/home/u")
        .env("XDG_CONFIG_HOME", "/cfg")
        .file("/cfg/pcmanfm/zeta/pcmanfm.conf", "wallpaper=/z.png")
        .file("/cfg/pcmanfm/LXDE/desktop-items-0.conf", "wallpaper=/l.png")
        .file("/etc/xdg/pcmanfm/default/pcmanfm.conf", "wallpaper=/d.png");
    let s = Linux::new(fake);
    assert_eq!(
        s.pcmanfm_config_files("pcmanfm", &["LXDE", "default", "lxde"], &["pcmanfm.conf", "desktop-items-0.conf"]),
        vec![
            PathBuf::from("/cfg/pcmanfm/LXDE/desktop-items-0.conf"),
            PathBuf::from("/cfg/pcmanfm/zeta/pcmanfm.conf"),
            PathBuf::from("/etc/xdg/pcmanfm/default/pcmanfm.conf"),
        ]
    );
}

const SPACES: &str = r#"{
    SpacesDisplayConfiguration =     {
        "Management Data" =         {
            Monitors =             (
                                {
                    "Current Space" =                     {
                        Collapsed = 0;
                        "Display Identifier" = Main;
                        ManagedSpaceID = 3;
                        id64 = 3;
                        type = 0;
                        uuid = "6C1A1D5B-6B5E-4D7B-9A3D-1C0B7E5F0A11";
                        wsid = 3;
                    };
"#;

const DESKTOP: &str = r#"{
    "6C1A1D5B-6B5E-4D7B-9A3D-1C0B7E5F0A11" =     {
        Main =         {
            ImageFilePath = "/Users/u/w.heic";
        };
        default =         {
            BackgroundColor =             (
                0,
                0
            );
            ImageFilePath = "/Users/u/w.heic";
            Placement = FitToScreen;
            ImageScaling = proportionallyUpOrDown;
        };
    };
    default =     {
        Placement = Tile;
    };
}"#;

/// `defaults read` pads `key =` with spaces to line the braces up, so
/// ShareX's `StartsWith("default = {")` doesn't see the padded
/// `default =         {`: the block is found only in unpadded output.
#[test]
fn macos_space_and_layout_parse_from_defaults() {
    assert_eq!(current_space_uuid(SPACES).as_deref(), Some("6C1A1D5B-6B5E-4D7B-9A3D-1C0B7E5F0A11"));
    assert_eq!(current_space_uuid("uuid = \"x\";"), None);
    assert_eq!(desktop_settings_block(DESKTOP, "6C1A1D5B-6B5E-4D7B-9A3D-1C0B7E5F0A11"), None);
    let unpadded = DESKTOP.replace("default =         {", "default = {");
    // The space's own `default = {` block, not the global one after it.
    let block = desktop_settings_block(&unpadded, "6C1A1D5B-6B5E-4D7B-9A3D-1C0B7E5F0A11").expect("block");
    assert_eq!(assigned_value(&block, "Placement").as_deref(), Some("FitToScreen"));
    assert_eq!(assigned_value(&block, "ImageScaling").as_deref(), Some("proportionallyUpOrDown"));
    assert_eq!(assigned_value(&block, "ImageFilePath").as_deref(), Some("/Users/u/w.heic"));
    assert_eq!(desktop_settings_block(&unpadded, "missing"), None);
}

#[test]
fn macos_service_asks_system_events_then_defaults() {
    let script = format!("osascript -e {MACOS_WALLPAPER_SCRIPT}");
    let fake = |desktop: &str| {
        Fake::default()
            .cmd(&script, "/Users/u/w.heic
")
            .cmd("defaults read com.apple.spaces", SPACES)
            .cmd("defaults read com.apple.desktop Background", desktop)
            .file("/Users/u/w.heic", "")
    };
    assert_eq!(MacOs::new(fake(&DESKTOP.replace("default =         {", "default = {"))).try_get_any_os(), info("/Users/u/w.heic", Layout::Fit));
    // Padded output, as `defaults` prints it: Fill.
    assert_eq!(MacOs::new(fake(DESKTOP)).try_get_any_os(), info("/Users/u/w.heic", Layout::Fill));
    // No Background settings: Fill.
    let fake = Fake::default().cmd(&script, "/Users/u/w.heic").file("/Users/u/w.heic", "");
    assert_eq!(MacOs::new(fake).try_get_any_os(), info("/Users/u/w.heic", Layout::Fill));
    // A path that doesn't exist: no wallpaper.
    let fake = Fake::default().cmd(&script, "/gone.heic");
    assert_eq!(MacOs::new(fake).try_get_any_os(), None);
}
