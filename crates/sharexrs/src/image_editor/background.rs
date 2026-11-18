//! The Background panel (`BackgroundSettingsPanel.axaml` in the editor's
//! left sidebar, `Border.editor-sidebar.left`): margin, padding, smart
//! padding, rounded corner and shadow radius sliders, the output ratios,
//! and the background (gradient with its presets, colour, transparent,
//! image, wallpaper). While it's open the canvas shows, and the editor
//! exports, the image on its background
//! (`sharex_image_editor::render::background`); the settings live in
//! `ImageEditorOptions` (`MainViewModel.BackgroundState`/`CanvasState`).

use std::path::PathBuf;

use egui::{Color32, CornerRadius, Id, Pos2, Rect, Sense, TextureHandle, Ui, Vec2, pos2, vec2};
use sharex_capture::editor_popups::{ColorDropdown, PopupResult};
use sharex_image_editor::options::ImageEditorOptions;
use sharex_image_editor::render::background::{self as bg, Background, Layout, Mode};
use sharex_skia::Pixmap;
use sharex_strings::image_editor as ie;
use sharex_ui::border::{self, Brush};
use sharex_ui::controls::{Button, ComboBox, ComboItem, Slider, ToggleSwitch};
use sharex_ui::theme::Thickness;
use sharex_ui::keys::app_icons;
use sharex_ui::{Palette, icons, widgets};

/// `Border.editor-sidebar.open`: `Width="282"`, a 1px right border.
pub const WIDTH: f32 = 282.0;
/// The panel's `StackPanel`: `Width="250"`, `Margin="16"`.
const CONTENT: f32 = 250.0;
const MARGIN: f32 = 16.0;

/// `MainViewModel.GradientPreset`s (`BuildGradientPresets`).
pub const PRESETS: [(&str, &str, &str); 60] = [
    ("Sunset Glow", "#FF7E5F", "#FEB47B"),
    ("Ocean Breeze", "#2193B0", "#6DD5ED"),
    ("Skyline", "#00C6FF", "#0072FF"),
    ("Peach Bloom", "#ED4264", "#FFEDBC"),
    ("Mint Fresh", "#11998E", "#38EF7D"),
    ("Royal Blue", "#396AFC", "#2948FF"),
    ("Warm Flame", "#FF9A9E", "#FAD0C4"),
    ("Coral Candy", "#FF9966", "#FF5E62"),
    ("Mango", "#FFE259", "#FFA751"),
    ("Violet Dream", "#4776E6", "#8E54E9"),
    ("Ruby Fire", "#DD2476", "#FF512F"),
    ("Aqua Marine", "#1A2980", "#26D0CE"),
    ("Instagram", "#833AB4", "#FD1D1D"),
    ("Aurora Green", "#00F260", "#0575E6"),
    ("Citrus Pop", "#F7971E", "#FFD200"),
    ("Arctic Sky", "#74EBD5", "#9FACE6"),
    ("Cherry Rush", "#EB3349", "#F45C43"),
    ("Lavender Sky", "#E0C3FC", "#8EC5FC"),
    ("Midnight City", "#232526", "#414345"),
    ("Deep Space", "#000428", "#004E92"),
    ("Lagoon", "#43C6AC", "#191654"),
    ("Rose Gold", "#EECDA3", "#EF629F"),
    ("Emerald Water", "#348F50", "#56B4D3"),
    ("Lemon Lime", "#56AB2F", "#A8E063"),
    ("Pink Haze", "#EC008C", "#FC6767"),
    ("Slate", "#BDC3C7", "#2C3E50"),
    ("Carbon Steel", "#485563", "#29323C"),
    ("Purple Bliss", "#8E2DE2", "#4A00E0"),
    ("Ember", "#CB2D3E", "#EF473A"),
    ("Soft Lilac", "#C471F5", "#FA71CD"),
    ("Blue Hour", "#355C7D", "#6C5B7B"),
    ("Golden Hour", "#F3904F", "#FFD194"),
    ("Watermelon", "#FF5F6D", "#FFC371"),
    ("Sea Foam", "#4CB8C4", "#3CD3AD"),
    ("Twilight Plum", "#41295A", "#2F0743"),
    ("Candy Grape", "#B24592", "#F15F79"),
    ("Fresh Mint", "#76B852", "#8DC26F"),
    ("Polar Night", "#283048", "#859398"),
    ("Tropical Sun", "#F6D365", "#FDA085"),
    ("Blue Raspberry", "#00B4DB", "#0083B0"),
    ("Rosewater", "#E55D87", "#5FC3E4"),
    ("Copper Glow", "#B79891", "#94716B"),
    ("Aurora Violet", "#654EA3", "#EAAFC8"),
    ("Apple Mint", "#5A3F37", "#2C7744"),
    ("Sunrise Peach", "#F857A6", "#FF5858"),
    ("Iceberg", "#C9FFBF", "#FFAFBD"),
    ("Nightfall", "#141E30", "#243B55"),
    ("Electric Cyan", "#0575E6", "#00F260"),
    ("Velvet Rose", "#DA4453", "#89216B"),
    ("Desert Sand", "#BA8B02", "#181818"),
    ("Cosmic Fusion", "#FF00CC", "#333399"),
    ("Deep Ocean", "#360033", "#0B8793"),
    ("Northern Lights", "#43E97B", "#38F9D7"),
    ("Neon Glow", "#4FACFE", "#00F2FE"),
    ("Burning Orange", "#FF416C", "#FF4B2B"),
    ("Mystic Purple", "#9D50BB", "#6E48AA"),
    ("Golden Sunset", "#F12711", "#F5AF19"),
    ("Midnight Dark", "#2C3E50", "#3498DB"),
    ("Cool Sky", "#2980B9", "#6DD5FA"),
    ("Purple Love", "#CC2B5E", "#753A88"),
];

/// `MainViewModel`'s output ratio keys (the 3x3 grid, in order).
const RATIOS: [&str; 9] = ["Auto", "1:1", "4:3", "3:2", "16:9", "5:3", "9:16", "3:4", "2:3"];

/// Which colour a popup edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Gradient1,
    Gradient2,
    Color,
}

/// What the panel asked the window for.
#[derive(Default)]
pub struct PanelResult {
    /// The settings changed (the canvas and export follow).
    pub changed: bool,
    /// The screen colour picker for this colour.
    pub pick_screen_color: Option<ColorTarget>,
}

/// The panel's state (`IsSettingsPanelOpen`, `SelectedOutputRatio`, the
/// sidebar's scroll, the colour popups) and its caches.
#[derive(Default)]
pub struct Panel {
    /// `IsSettingsPanelOpen`.
    pub open: bool,
    /// `SelectedOutputRatio` (not saved: Auto each time the editor opens).
    pub ratio: usize,
    scroll: f32,
    popup: Option<ColorTarget>,
    dropdown: ColorDropdown,
    popup_anchor: Option<Rect>,
    /// The smart padding insets of the image at a version
    /// (`EnsureSmartPaddingCache`).
    insets: Option<(u64, (i32, i32, i32, i32))>,
    /// Background images, by path (`LoadBitmapFromPath`).
    images: Vec<(PathBuf, Option<Pixmap>)>,
    /// The wallpaper service's answer (`None`: not supported), asked once.
    wallpaper: Option<Option<Option<(PathBuf, Layout)>>>,
    /// Gradient swatch textures: the preview and the presets.
    swatches: Vec<(String, TextureHandle)>,
    /// Where the panel sat last frame.
    pub rect: Option<Rect>,
}

/// `IDesktopWallpaperService` for ShareXrs: Windows' wallpaper
/// (`SPI_GETDESKWALLPAPER`, then the registry's `TranscodedImageCache`),
/// always laid out as Fill, as ShareX's `WindowsDesktopWallpaperService`
/// reports it. `SHAREXRS_TEST_WALLPAPER` (a file, optionally
/// `;Fit|Stretch|Center|Tile|Span`) stands in for it in tests.
pub fn desktop_wallpaper() -> Option<Option<(PathBuf, Layout)>> {
    if let Ok(v) = std::env::var("SHAREXRS_TEST_WALLPAPER") {
        let mut parts = v.splitn(2, ';');
        let path = PathBuf::from(parts.next().unwrap_or_default());
        let layout = match parts.next().unwrap_or("Fill") {
            "Fit" => Layout::Fit,
            "Stretch" => Layout::Stretch,
            "Center" => Layout::Center,
            "Tile" => Layout::Tile,
            "Span" => Layout::Span,
            _ => Layout::Fill,
        };
        return Some(path.exists().then_some((path, layout)));
    }
    #[cfg(windows)]
    {
        Some(windows_wallpaper().map(|p| (p, Layout::Fill)))
    }
    #[cfg(not(windows))]
    {
        // ShareX's macOS and Linux services (System Events, the desktop
        // environments' settings) aren't ported: no Wallpaper option.
        None
    }
}

#[cfg(windows)]
fn windows_wallpaper() -> Option<PathBuf> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETDESKWALLPAPER, SystemParametersInfoW};
    let mut buf = vec![0u16; 32768];
    // SAFETY: the buffer holds `buf.len()` UTF-16 units, as the call is told.
    let ok = unsafe { SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, buf.as_mut_ptr().cast(), 0) };
    if ok != 0 {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let path = String::from_utf16_lossy(&buf[..end]);
        let p = PathBuf::from(path.trim_end_matches('\0'));
        if !path.trim().is_empty() {
            return p.is_file().then_some(p);
        }
    }
    // TranscodedImageCache: the path is the first `X:` or `\` in it.
    let bytes = registry_binary(r"Control Panel\Desktop", "TranscodedImageCache")?;
    if bytes.is_empty() {
        return None;
    }
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let text = String::from_utf16_lossy(&units);
    let chars: Vec<char> = text.chars().collect();
    let start = (0..chars.len().saturating_sub(2)).find(|&i| {
        (chars[i].is_alphabetic() && chars[i + 1] == ':' && chars[i + 2] == '\\') || (chars[i] == '\\' && chars[i + 1] == '\\')
    })?;
    let rest: String = chars[start..].iter().take_while(|&&c| c != '\0').collect();
    let p = PathBuf::from(rest.trim());
    p.is_file().then_some(p)
}

/// A binary value under `HKEY_CURRENT_USER`.
#[cfg(windows)]
fn registry_binary(key: &str, name: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_BINARY, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (k, n) = (wide(key), wide(name));
    let mut size = 0u32;
    // SAFETY: the strings are NUL-terminated; a null buffer asks for the size.
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), n.as_ptr(), RRF_RT_REG_BINARY, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) };
    if r != 0 || size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    // SAFETY: the buffer holds `size` bytes, as the call is told.
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), n.as_ptr(), RRF_RT_REG_BINARY, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    if r != 0 {
        return None;
    }
    buf.truncate(size as usize);
    Some(buf)
}

/// The wallpaper option exists where the service is supported
/// (`BuildBackgroundModeOptions`).
fn modes(wallpaper: bool) -> Vec<Mode> {
    let mut m = vec![Mode::Gradient, Mode::Color, Mode::Transparent, Mode::Image];
    if wallpaper {
        m.push(Mode::Wallpaper);
    }
    m
}

fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Gradient => ie::MainViewModel_BackgroundModeGradient(),
        Mode::Color => ie::MainViewModel_BackgroundModeColor(),
        Mode::Transparent => ie::MainViewModel_BackgroundModeTransparent(),
        Mode::Image => ie::MainViewModel_BackgroundModeImage(),
        Mode::Wallpaper => ie::MainViewModel_BackgroundModeWallpaper(),
    }
}

fn color32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

fn hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

impl Panel {
    /// The wallpaper service's answer (`None`: not supported).
    fn wallpaper(&mut self) -> Option<Option<(PathBuf, Layout)>> {
        self.wallpaper.get_or_insert_with(desktop_wallpaper).clone()
    }

    fn wallpaper_supported(&mut self) -> bool {
        self.wallpaper().is_some()
    }

    /// The selected mode as the combo box shows it (a Wallpaper setting
    /// with no wallpaper service falls back to the first option).
    pub fn mode(&mut self, o: &ImageEditorOptions) -> Mode {
        let m = Mode::parse(&o.BackgroundType);
        if m == Mode::Wallpaper && !self.wallpaper_supported() { Mode::Gradient } else { m }
    }

    fn load_image(&mut self, path: &std::path::Path) -> Option<Pixmap> {
        if let Some((_, pm)) = self.images.iter().find(|(p, _)| p == path) {
            return pm.clone();
        }
        let pm = crate::thumbnail::load(path).map(|img| super::pixmap_of(&img));
        if pm.is_none() {
            eprintln!("Image editor: couldn't load background image {}", path.display());
        }
        self.images.push((path.to_path_buf(), pm.clone()));
        pm
    }

    /// The background as it applies now.
    pub fn background(&mut self, o: &ImageEditorOptions) -> Background {
        let ratio = bg::parse_ratio(RATIOS[self.ratio]);
        let wallpaper = self.wallpaper();
        let path = match Mode::parse(&o.BackgroundType) {
            Mode::Image if !o.BackgroundImagePath.trim().is_empty() => Some(PathBuf::from(&o.BackgroundImagePath)),
            Mode::Wallpaper => wallpaper.clone().flatten().map(|(p, _)| p),
            _ => None,
        };
        let pm = path.and_then(|p| self.load_image(&p));
        bg::from_options(o, ratio, wallpaper, |_| pm.clone())
    }

    /// Whether the smart padding insets of the image `key` are unknown.
    pub fn needs_insets(&self, key: u64) -> bool {
        self.open && self.insets.is_none_or(|(k, _)| k != key)
    }

    /// `EnsureSmartPaddingCache`'s result for the image `key`; turns smart
    /// padding off when there's none (`DisableBackgroundSmartPaddingIfUnavailable`).
    pub fn set_insets(&mut self, key: u64, insets: (i32, i32, i32, i32), o: &mut ImageEditorOptions) {
        self.insets = Some((key, insets));
        if !bg::has_smart_padding(insets) {
            o.BackgroundSmartPadding = false;
        }
    }

    /// The sidebar's scroll (`--render`).
    pub fn set_scroll(&mut self, y: f32) {
        self.scroll = y;
    }

    /// `CanUseBackgroundSmartPadding`.
    fn can_use_smart_padding(&self) -> bool {
        self.insets.is_none_or(|(_, i)| bg::has_smart_padding(i))
    }

    /// `ToggleSettingsPanel`.
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.insets = None;
        self.close_popup();
    }

    pub fn close_popup(&mut self) {
        self.popup = None;
        self.dropdown.close();
    }

    /// Whether `p` is on the panel or its popup.
    pub fn contains(&self, p: Pos2) -> bool {
        (self.open && self.rect.is_some_and(|r| r.contains(p))) || self.dropdown.contains(p)
    }

    /// The content's height (for the scroll viewer).
    fn content_height(&self, mode: Mode, path_lines: usize) -> f32 {
        // Margin 16, the heading (20px text, 16 below it, 8 more), four
        // slider blocks, the smart padding row, the ratio label and grid,
        // the background label and combo box: up to 655.
        let base = 655.0;
        let tail = match mode {
            Mode::Gradient => 40.0 + 4.0 + 8.0 + (PRESETS.len() as f32 / 4.0).ceil() * 44.0,
            Mode::Color => 40.0 + 4.0,
            Mode::Image => 40.0 + if path_lines > 0 { 6.0 + 15.0 + 2.0 + 15.0 * path_lines as f32 } else { 0.0 },
            _ => 0.0,
        };
        let tail = if tail > 0.0 { tail } else { -12.0 + 12.0 };
        base + tail + MARGIN
    }

    /// Draws the sidebar at the left of `view` and handles its input.
    pub fn show(&mut self, ui: &mut Ui, p: &Palette, view: Rect, o: &mut ImageEditorOptions, modal: bool) -> PanelResult {
        let mut out = PanelResult::default();
        let rect = Rect::from_min_size(view.min, vec2(WIDTH, view.height()));
        self.rect = Some(rect);
        // `Background.Panel`, the 1px right border.
        ui.painter().rect_filled(rect, 0.0, p.background_panel);
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.max.x - 1.0, rect.min.y), rect.max), 0.0, p.border);
        let viewport = Rect::from_min_max(rect.min, pos2(rect.max.x - 1.0, rect.max.y));
        let mode = self.mode(o);
        let path_lines = if mode == Mode::Image && !o.BackgroundImagePath.trim().is_empty() {
            sharex_ui::controls::wrap_lines(ui, &o.BackgroundImagePath, &widgets::font(11.0, 400), CONTENT).len()
        } else {
            0
        };
        let extent = self.content_height(mode, path_lines);
        let max_scroll = (extent - viewport.height()).max(0.0);
        let hovered = !modal && ui.rect_contains_pointer(viewport);
        if hovered && !ComboBox::is_open(ui.ctx(), Id::new("bg-mode")) {
            let d = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            self.scroll -= d;
        }
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(viewport));
        child.set_clip_rect(viewport.intersect(ui.clip_rect()));
        if modal {
            child.disable();
        }
        let ui = &mut child;
        let x = viewport.min.x + MARGIN;
        let top = (viewport.min.y + MARGIN - self.scroll).round();
        let painter = ui.painter().clone();
        let label_font = widgets::font(13.0, 600);
        let lh = widgets::line_height(ui, &label_font);

        // The heading: the icon and "Background" (20px, Normal, accent text).
        let head_font = widgets::font(20.0, 400);
        let head_h = widgets::line_height(ui, &head_font);
        let icon_r = Rect::from_min_size(pos2(x, top + ((head_h + 16.0 - 2.0 - 20.0) / 2.0).round_ties_even() + 2.0), vec2(20.0, 20.0));
        icons::paint(&painter, icon_r, app_icons::Wallpaper, p.text_secondary);
        let g = widgets::galley(ui, ie::EditorView_Background(), head_font.clone(), p.accent_text, f32::INFINITY);
        painter.galley(pos2(x + 20.0 + 8.0, top), g, p.accent_text);
        let mut y = top + head_h + 16.0 + 8.0;

        // The slider blocks: a label with its value capsule, the slider.
        let slider = |ui: &mut Ui, y: &mut f32, label: &str, id: &str, value: &mut f64, max: f32| -> bool {
            let row = Rect::from_min_size(pos2(x, *y), vec2(CONTENT, 24.0));
            let g = widgets::galley(ui, label, label_font.clone(), p.text_secondary, f32::INFINITY);
            ui.painter().galley(pos2(x, *y), g, p.text_secondary);
            capsule(ui, p, row, &format!("{} px", value.round_ties_even() as i64));
            let r = Rect::from_min_size(pos2(x, *y + 24.0), vec2(CONTENT, 50.0));
            let mut v = *value as f32;
            let s = Slider { minimum: 0.0, maximum: max, step: 0.0, enabled: true };
            let changed = s.show_with_track(ui, r, Id::new(("bg-slider", id)), &mut v, Some(p.control_background_disabled));
            if changed {
                *value = v as f64;
            }
            *y += 24.0 + 50.0;
            changed
        };
        out.changed |= slider(ui, &mut y, ie::EditorView_Margin(), "margin", &mut o.BackgroundMargin, 200.0);
        out.changed |= slider(ui, &mut y, ie::EditorView_Padding(), "padding", &mut o.BackgroundPadding, 200.0);
        // `LabeledToggleSwitch`: the label and the switch (its box's right
        // edge 12px past the row), 32 high, 16 below.
        {
            let row = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 32.0));
            let enabled = self.can_use_smart_padding();
            let opacity = if enabled { 1.0 } else { 0.35 };
            let resp = ui.interact(row, Id::new("bg-smart"), if enabled { Sense::click() } else { Sense::hover() });
            let mut faded = ui.new_child(egui::UiBuilder::new().max_rect(row));
            faded.set_clip_rect(ui.clip_rect());
            faded.multiply_opacity(opacity);
            let ty = widgets::center_y(row, lh);
            let g = widgets::galley(&faded, ie::EditorView_SmartPadding(), label_font.clone(), p.text_secondary, f32::INFINITY);
            faded.painter().galley(pos2(x, ty), g, p.text_secondary);
            let mut on = o.BackgroundSmartPadding;
            let sw = ToggleSwitch { enabled };
            let _ = sw.show(&faded, pos2(row.max.x + 12.0 - ToggleSwitch::SIZE.x, row.min.y), Id::new("bg-smart-switch"), &mut on.clone());
            if enabled && resp.clicked() {
                on = !on;
                o.BackgroundSmartPadding = on;
                self.insets = None;
                out.changed = true;
            }
            y += 32.0 + 16.0;
        }
        out.changed |= slider(ui, &mut y, ie::EditorView_RoundedCorner(), "corner", &mut o.BackgroundRoundedCorner, 50.0);
        out.changed |= slider(ui, &mut y, ie::EditorView_ShadowRadius(), "shadow", &mut o.BackgroundShadowRadius, 100.0);

        // Ratio: the label (16 below), the 3x3 grid of 32px buttons.
        let g = widgets::galley(ui, ie::EditorView_Ratio(), label_font.clone(), p.text_secondary, f32::INFINITY);
        painter.galley(pos2(x, y), g, p.text_secondary);
        y += lh + 16.0;
        // Star columns, layout-rounded: 83, 84, 83.
        let cols = [0.0, 83.0, 167.0, 250.0];
        for (i, key) in RATIOS.iter().enumerate() {
            let (c, r) = (i % 3, i / 3);
            let cell = Rect::from_min_max(pos2(x + cols[c], y + r as f32 * 36.0), pos2(x + cols[c + 1], y + r as f32 * 36.0 + 36.0));
            let b = cell.shrink(2.0);
            let text = if i == 0 { ie::EditorView_Auto() } else { key };
            let clicked = if self.ratio == i {
                active_button(ui, p, b, Id::new(("bg-ratio", i)), text)
            } else {
                Button::default().show(ui, b, Id::new(("bg-ratio", i)), text, true).clicked()
            };
            if clicked && self.ratio != i {
                self.ratio = i;
                out.changed = true;
            }
        }
        y += 108.0;

        // Background: the label (16 above and below), the combo box.
        y += 16.0;
        let g = widgets::galley(ui, ie::EditorView_Background(), label_font.clone(), p.text_secondary, f32::INFINITY);
        painter.galley(pos2(x, y), g, p.text_secondary);
        y += lh + 16.0;
        let supported = self.wallpaper_supported();
        let list = modes(supported);
        let items: Vec<ComboItem> = list.iter().map(|m| ComboItem::text(mode_name(*m))).collect();
        let sel = list.iter().position(|m| *m == mode);
        let combo = ComboBox { padding: Thickness::hv(8.0, 6.0), enabled: true };
        if let Some(i) = combo.show(ui, Rect::from_min_size(pos2(x, y), vec2(CONTENT, 40.0)), Id::new("bg-mode"), &items, sel) {
            o.BackgroundType = list[i].name().to_string();
            self.close_popup();
            out.changed = true;
        }
        y += 40.0 + 12.0;

        match mode {
            Mode::Gradient => {
                let preview = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 40.0));
                let c1 = bg::rgba(&bg::normalize_hex(&o.BackgroundGradientColor1Hex, bg::GRADIENT1_FALLBACK));
                let c2 = bg::rgba(&bg::normalize_hex(&o.BackgroundGradientColor2Hex, bg::GRADIENT2_FALLBACK));
                self.swatch(ui, p, preview, 5.0, c1, c2);
                // The two halves open their colour's popup.
                for (k, target) in [ColorTarget::Gradient1, ColorTarget::Gradient2].into_iter().enumerate() {
                    let half = if k == 0 {
                        Rect::from_min_max(preview.min, pos2(preview.center().x, preview.max.y))
                    } else {
                        Rect::from_min_max(pos2(preview.center().x, preview.min.y), preview.max)
                    };
                    let resp = ui.interact(half, Id::new(("bg-half", k)), Sense::click());
                    overlay(ui, half, if k == 0 { CornerRadius { nw: 5, sw: 5, ne: 0, se: 0 } } else { CornerRadius { nw: 0, sw: 0, ne: 5, se: 5 } }, &resp);
                    let tip = if k == 0 { ie::EditorView_Editgradientcolor1() } else { ie::EditorView_Editgradientcolor2() };
                    let resp = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.is_pointer_button_down_on() && ui.ctx().input(|i| i.pointer.primary_pressed()) {
                        self.open_popup(target, preview, color32(if k == 0 { c1 } else { c2 }));
                    }
                }
                y += 40.0 + 4.0 + 8.0;
                // The presets: a 4-column `UniformGrid` of 62px cells, each
                // a 46x36 swatch 2px in.
                for (i, (name, a, b)) in PRESETS.iter().enumerate() {
                    let (c, r) = (i % 4, i / 4);
                    let cell = Rect::from_min_size(pos2(x + c as f32 * 62.0 + 2.0, y + r as f32 * 44.0 + 2.0), vec2(46.0, 40.0));
                    let sw = Rect::from_min_size(pos2(cell.min.x, cell.min.y + 2.0), vec2(46.0, 36.0));
                    if sw.max.y < viewport.min.y || sw.min.y > viewport.max.y {
                        continue;
                    }
                    let (pa, pb) = (bg::rgba(a), bg::rgba(b));
                    self.swatch(ui, p, sw, 5.0, pa, pb);
                    let resp = ui.interact(cell, Id::new(("bg-preset", i)), Sense::click()).on_hover_text(*name);
                    if resp.clicked() {
                        // `ApplyGradientPreset`: both colours, then Gradient.
                        o.BackgroundGradientColor1Hex = bg::normalize_hex(a, bg::GRADIENT1_FALLBACK);
                        o.BackgroundGradientColor2Hex = bg::normalize_hex(b, bg::GRADIENT2_FALLBACK);
                        o.BackgroundType = Mode::Gradient.name().to_string();
                        out.changed = true;
                    }
                }
            }
            Mode::Color => {
                let preview = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 40.0));
                let c = bg::rgba(&bg::normalize_hex(&o.BackgroundColorHex, bg::COLOR_FALLBACK));
                small_checkered(&painter, preview.shrink(1.0), p);
                border::border(&painter, preview, 5.0, 1.0, Some(Brush::Solid(color32(c))), Some(p.border));
                let resp = ui.interact(preview, Id::new("bg-color"), Sense::click());
                overlay(ui, preview, CornerRadius::same(5), &resp);
                let resp = resp.on_hover_text(ie::EditorView_Editbackgroundcolor()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.is_pointer_button_down_on() && ui.ctx().input(|i| i.pointer.primary_pressed()) {
                    self.open_popup(ColorTarget::Color, preview, color32(c));
                }
            }
            Mode::Image => {
                let b = Rect::from_min_size(pos2(x, y), vec2(CONTENT, 40.0));
                if Button::default().show(ui, b, Id::new("bg-browse"), ie::EditorView_BrowsebackgroundimageEllipsis(), true).clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_title(ie::EditorView_SelectBackgroundImage())
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp", "ico", "tif", "tiff"])
                        .pick_file()
                {
                    // `SetBackgroundImagePath`: the same path loads again.
                    self.images.retain(|(q, _)| *q != path);
                    o.BackgroundImagePath = path.display().to_string();
                    out.changed = true;
                }
                if path_lines > 0 {
                    let small = widgets::font(11.0, 400);
                    let mut ty = y + 40.0 + 6.0;
                    let g = widgets::galley(ui, ie::EditorView_Backgroundimage(), small.clone(), p.text_secondary, f32::INFINITY);
                    painter.galley(pos2(x, ty), g, p.text_secondary);
                    ty += widgets::line_height(ui, &small) + 2.0;
                    for line in sharex_ui::controls::wrap_lines(ui, &o.BackgroundImagePath, &small, CONTENT) {
                        let g = widgets::galley(ui, &line, small.clone(), p.text_secondary, f32::INFINITY);
                        painter.galley(pos2(x, ty), g, p.text_secondary);
                        ty += widgets::line_height(ui, &small);
                    }
                }
            }
            _ => {}
        }

        // The scroll viewer's collapsed vertical bar.
        if max_scroll > 0.0 {
            let thumb = sharex_ui::theme::fluent(ui.ctx()).base_medium;
            let track = (viewport.min.y + 16.0, viewport.max.y - 16.0);
            let len = track.1 - track.0;
            let th = (len * viewport.height() / extent).max(16.0);
            let ty = track.0 + (len - th) * self.scroll / max_scroll;
            ui.painter().rect_filled(Rect::from_min_size(pos2(viewport.max.x - 4.0, ty), vec2(2.0, th)), CornerRadius::same(1), thumb);
        }
        out
    }

    fn open_popup(&mut self, target: ColorTarget, anchor: Rect, color: Color32) {
        if self.popup == Some(target) && self.dropdown.open {
            self.close_popup();
            return;
        }
        self.dropdown.close();
        self.popup = Some(target);
        self.popup_anchor = Some(anchor);
        self.dropdown.toggle(color);
    }

    /// The open colour popup (over everything, within `bounds`).
    pub fn show_popup(&mut self, ui: &Ui, p: &Palette, bounds: Rect, o: &mut ImageEditorOptions) -> PanelResult {
        let mut out = PanelResult::default();
        let Some(target) = self.popup else { return out };
        if !self.dropdown.open {
            self.popup = None;
            return out;
        }
        let (field, fallback) = match target {
            ColorTarget::Gradient1 => (&mut o.BackgroundGradientColor1Hex, bg::GRADIENT1_FALLBACK),
            ColorTarget::Gradient2 => (&mut o.BackgroundGradientColor2Hex, bg::GRADIENT2_FALLBACK),
            ColorTarget::Color => (&mut o.BackgroundColorHex, bg::COLOR_FALLBACK),
        };
        let mut c = color32(bg::rgba(&bg::normalize_hex(field, fallback)));
        let anchor = self.popup_anchor.unwrap_or(Rect::NOTHING);
        let r: PopupResult = self.dropdown.show(ui, p, anchor, bounds, "bg-color", &mut c);
        let new = hex(c);
        if new != *field {
            *field = new;
            out.changed = true;
        }
        if r.pick_screen_color {
            out.pick_screen_color = Some(target);
        }
        if !self.dropdown.open {
            self.popup = None;
        }
        out
    }

    /// A gradient swatch: the brush (rendered by Skia as Avalonia fills it)
    /// in its rounded border.
    fn swatch(&mut self, ui: &Ui, p: &Palette, rect: Rect, radius: f32, a: [u8; 4], b: [u8; 4]) {
        let key = format!("{}x{}:{:?}{:?}", rect.width(), rect.height(), a, b);
        let ppp = ui.ctx().pixels_per_point();
        let tex = match self.swatches.iter().find(|(k, _)| *k == key) {
            Some((_, t)) => t.clone(),
            None => {
                let (w, h) = ((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32);
                let img = gradient_fill(w, h, (radius * ppp).max(0.0), a, b, ppp);
                let t = ui.ctx().load_texture(format!("bg-swatch-{key}"), img, egui::TextureOptions::NEAREST);
                self.swatches.push((key, t.clone()));
                t
            }
        };
        ui.painter().image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        border::border(ui.painter(), rect, radius, 1.0, None, Some(p.border));
    }
}

/// `LinearGradientBrush` (0,0)-(1,1) filling a `Border` with a 1px border:
/// the fill to the middle of the border, anti-aliased.
fn gradient_fill(w: u32, h: u32, radius: f32, a: [u8; 4], b: [u8; 4], ppp: f32) -> egui::ColorImage {
    use sharex_skia::raster::gradient::LinearGradient;
    use sharex_skia::raster::{Bitmap, Canvas, Matrix, Paint, Shader};
    let mut bmp = Bitmap::new(w.max(1), h.max(1));
    {
        let mut c = Canvas::new(&mut bmp);
        let mut paint = Paint::shader(Shader::Gradient(LinearGradient {
            start: (0.0, 0.0),
            end: (w as f32, h as f32),
            colors: vec![a, b],
            positions: Some(vec![0.0, 1.0]),
            local: Matrix::IDENTITY,
        }));
        paint.anti_alias = true;
        let half = 0.5 * ppp;
        let mut path = sharex_skia::Path::new();
        path.add_round_rect(
            sharex_skia::Rect::from_ltrb(half, half, w as f32 - half, h as f32 - half),
            (radius - half).max(0.0),
            (radius - half).max(0.0),
            sharex_skia::Direction::Cw,
        );
        c.fill_sk_path(&path, &paint);
    }
    egui::ColorImage::from_rgba_premultiplied([bmp.width() as usize, bmp.height() as usize], bmp.data())
}

/// `ShareX.Brush.Checkered.Small`: 8px tiles of two 4px squares.
fn small_checkered(painter: &egui::Painter, rect: Rect, p: &Palette) {
    painter.rect_filled(rect, 4.0, p.checkered[0]);
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let mut j = 0;
    let mut y = rect.min.y;
    while y < rect.max.y {
        let mut i = 0;
        let mut x = rect.min.x;
        while x < rect.max.x {
            if (i + j) % 2 == 1 {
                clip.rect_filled(Rect::from_min_size(pos2(x, y), Vec2::splat(4.0)), 0.0, p.checkered[1]);
            }
            x += 4.0;
            i += 1;
        }
        y += 4.0;
        j += 1;
    }
}

/// `gradient-preview-half` / `color-preview-overlay`: transparent at rest,
/// `#66FFFFFF` with a `#CCFFFFFF` border under the pointer, `#55000000`
/// with `#DD000000` pressed.
fn overlay(ui: &Ui, rect: Rect, radius: CornerRadius, resp: &egui::Response) {
    let (fill, stroke) = if resp.is_pointer_button_down_on() {
        (Color32::from_rgba_unmultiplied(0, 0, 0, 0x55), Color32::from_rgba_unmultiplied(0, 0, 0, 0xDD))
    } else if resp.hovered() {
        (Color32::from_rgba_unmultiplied(255, 255, 255, 0x66), Color32::from_rgba_unmultiplied(255, 255, 255, 0xCC))
    } else {
        return;
    };
    ui.painter().rect_filled(rect, radius, fill);
    ui.painter().rect_stroke(rect, radius, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
}

/// `Border.capsule` at the right of `row`: `Padding="6,3"`, 4px corners,
/// the accent fill, its text in the accent foreground.
pub(super) fn capsule(ui: &Ui, p: &Palette, row: Rect, text: &str) {
    // (ShareX's shot shows it Regular.)
    let font = widgets::font(13.0, 400);
    let w = widgets::text_width(ui, text, font.clone()).ceil() + 12.0;
    let h = widgets::line_height(ui, &font) + 6.0;
    let r = Rect::from_min_size(pos2(row.max.x - w, row.min.y), vec2(w, h));
    border::border(ui.painter(), r, 4.0, 0.0, Some(Brush::accent(ui.ctx())), None);
    let g = widgets::galley(ui, text, font, p.accent_foreground, f32::INFINITY);
    ui.painter().galley(pos2(r.min.x + 6.0, r.min.y + 3.0), g, p.accent_foreground);
}

/// A `Button.active` (`ratio-button`): the accent fill in every state, the
/// accent foreground.
pub(super) fn active_button(ui: &Ui, p: &Palette, rect: Rect, id: Id, text: &str) -> bool {
    let r = ui.interact(rect, id, Sense::click());
    border::border(ui.painter(), rect, 4.0, 1.0, Some(Brush::accent(ui.ctx())), Some(p.border));
    let font = sharex_ui::controls::content_font(13.0, 400);
    let w = widgets::text_width(ui, text, font.clone());
    let inner = rect.shrink(1.0);
    let x = inner.min.x + 10.0 + ((inner.width() - 20.0 - w) / 2.0).round_ties_even().max(0.0);
    let g = widgets::galley(ui, text, font.clone(), p.accent_foreground, f32::INFINITY);
    ui.painter().galley(pos2(x, widgets::center_y(inner, widgets::line_height(ui, &font))), g, p.accent_foreground);
    r.clicked()
}

/// What the canvas shows and the editor exports while the panel is open.
pub struct Frame {
    pub frame: bg::Frame,
    pub background: Background,
    pub insets: Option<(i32, i32, i32, i32)>,
}

impl Frame {
    /// Where the image's (0, 0) sits in the frame.
    pub fn image_offset(&self) -> (i32, i32) {
        (self.frame.viewport.0 - self.frame.crop.0, self.frame.viewport.1 - self.frame.crop.1)
    }

    /// The export: `content` (the image with its annotations) on the
    /// background; `source` is the image alone (the padding colour).
    pub fn render(&self, content: &Pixmap, source: &Pixmap) -> Pixmap {
        bg::render(content, source.pixel(0, 0), self.insets, &self.background)
    }
}

impl Panel {
    /// The frame for an image of `size`, when the panel is open.
    pub fn frame(&mut self, size: (u32, u32), o: &ImageEditorOptions) -> Option<Frame> {
        if !self.open {
            return None;
        }
        let insets = self.insets.map(|(_, i)| i).unwrap_or((0, 0, 0, 0));
        let background = self.background(o);
        let active = background.smart_padding && bg::has_smart_padding(insets);
        let insets = active.then_some(insets);
        let frame = bg::frame(size, insets, &background);
        Some(Frame { frame, background, insets })
    }
}
