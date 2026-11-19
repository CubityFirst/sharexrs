//! The tray icon: `ShareX/Presentation/MainWindow/TrayIconService.cs`
//! (`DesktopTrayIconService`) and `ShareX.HelpersLib/Native/WindowsTrayIcon.cs`.
//!
//! On Windows the icon reports every mouse button and ShareX shows its own
//! menu (`MainWindow.ShowTrayMenu`, ported in `tray_menu.rs`). Elsewhere
//! ShareX gives Avalonia's `TrayIcon` a native menu built from the same
//! entries (`BuildNativeMenu`), and a click on the icon is a left click:
//! the macOS menu bar item and the Linux StatusNotifierItem do the same
//! here. Platform code follows snapr's `src/tray/`.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::Tray;
#[cfg(target_os = "macos")]
pub use macos::{Tray, set_dock_icon};
#[cfg(windows)]
pub use windows::{Tray, double_click_time};

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
compile_error!("the tray icon is ported for Windows, macOS and Linux");

use std::sync::Arc;

use image::RgbaImage;

/// `InputMouseButton`, as the tray icon reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

/// What the tray icon tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum TrayEvent {
    /// `MouseDown` (Windows only).
    Down(Button),
    /// `MouseUp`; elsewhere `Clicked` is a left button's.
    Up(Button),
    /// A native menu's entry was chosen (macOS, Linux): its index in
    /// [`TrayMenuEntries::commands`].
    #[cfg_attr(windows, allow(dead_code))]
    Command(usize),
}

pub type Callback = Arc<dyn Fn(TrayEvent) + Send + Sync>;

/// A native menu's entry, flattened from `MainMenuEntry`s
/// (`BuildNativeMenu`): headers only, checks, submenus.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeEntry {
    pub header: String,
    pub separator: bool,
    pub enabled: bool,
    pub checked: bool,
    /// The index [`TrayEvent::Command`] reports.
    pub command: Option<usize>,
    pub children: Option<Vec<NativeEntry>>,
}

/// The tray menu as a native menu, and the command behind each index
/// [`TrayEvent::Command`] reports.
#[derive(Clone, Debug, Default)]
pub struct TrayMenuEntries {
    pub native: Vec<NativeEntry>,
    pub commands: Vec<crate::main_menu::MenuCommand>,
}

impl TrayMenuEntries {
    /// `BuildNativeMenu`: visible entries only; a tool grid becomes a
    /// submenu per category.
    pub fn new(entries: &[sharex_ui::menu::MenuEntry<crate::main_menu::MenuCommand>]) -> Self {
        let mut me = Self::default();
        me.native = me.build(entries);
        me
    }

    fn build(&mut self, entries: &[sharex_ui::menu::MenuEntry<crate::main_menu::MenuCommand>]) -> Vec<NativeEntry> {
        let mut out = Vec::new();
        for e in entries.iter().filter(|e| e.visible) {
            if e.separator {
                out.push(NativeEntry { header: String::new(), separator: true, enabled: false, checked: false, command: None, children: None });
                continue;
            }
            let children = if let Some(children) = &e.children {
                Some(self.build(children))
            } else {
                e.categories.as_ref().map(|categories| {
                    categories
                        .iter()
                        .map(|c| NativeEntry {
                            header: c.header.clone(),
                            separator: false,
                            enabled: true,
                            checked: false,
                            command: None,
                            children: Some(self.build(&c.entries)),
                        })
                        .collect()
                })
            };
            let command = match (&children, &e.command) {
                (None, Some(c)) => {
                    self.commands.push(c.clone());
                    Some(self.commands.len() - 1)
                }
                _ => None,
            };
            out.push(NativeEntry { header: e.header.clone(), separator: false, enabled: e.enabled, checked: e.checked, command, children });
        }
        out
    }
}

/// ShareXrs's icon (`assets/ShareXrs.iconset`) where ShareX shows its own
/// (`ShareXResources.IconBytes`), at `size` pixels square:
/// `SkiaImageHelpers.DecodeIcon` takes the smallest picture that covers the
/// size and scales it to it.
pub fn app_icon(size: u32) -> RgbaImage {
    const SIZES: [(u32, &[u8]); 7] = [
        (16, include_bytes!("../../../../assets/ShareXrs.iconset/icon_16x16.png")),
        (32, include_bytes!("../../../../assets/ShareXrs.iconset/icon_32x32.png")),
        (64, include_bytes!("../../../../assets/ShareXrs.iconset/icon_32x32@2x.png")),
        (128, include_bytes!("../../../../assets/ShareXrs.iconset/icon_128x128.png")),
        (256, include_bytes!("../../../../assets/ShareXrs.iconset/icon_256x256.png")),
        (512, include_bytes!("../../../../assets/ShareXrs.iconset/icon_512x512.png")),
        (1024, include_bytes!("../../../../assets/ShareXrs.iconset/icon_512x512@2x.png")),
    ];
    let size = size.max(1);
    let (_, png) = SIZES.iter().find(|(s, _)| *s >= size).unwrap_or(&SIZES[SIZES.len() - 1]);
    let image = image::load_from_memory(png).map(|i| i.to_rgba8()).unwrap_or_else(|_| RgbaImage::new(size, size));
    if image.width() == size && image.height() == size {
        return image;
    }
    image::imageops::resize(&image, size, size, image::imageops::FilterType::Triangle)
}

/// `TrayIconImage`'s and `Helpers.GetProgressIconBytes`' size: the small
/// icon size (`SM_CXSMICON`) on Windows, 16 pixels elsewhere.
pub fn glyph_icon_size() -> u32 {
    #[cfg(windows)]
    return Tray::icon_size();
    #[cfg(not(windows))]
    16
}

/// The OS's light/dark mode (`PlatformSettings.GetColorValues().ThemeVariant`),
/// kept up to date by the app.
static SYSTEM_LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The OS switched light/dark mode.
pub fn set_system_light(light: bool) {
    SYSTEM_LIGHT.store(light, std::sync::atomic::Ordering::Relaxed);
}

/// `TrayIconImage.GetThemeIconColor`: black on a light taskbar (Windows)
/// or in the OS's light mode (elsewhere), else white.
pub fn glyph_color() -> egui::Color32 {
    let light = if cfg!(windows) { sharex_capture::platform::light_taskbar() } else { SYSTEM_LIGHT.load(std::sync::atomic::Ordering::Relaxed) };
    if light { egui::Color32::BLACK } else { egui::Color32::WHITE }
}

/// Whether the app's icon is the white one (`ShareXResources.UseWhiteIcon`,
/// from `UseWhiteShareXIcon`).
static WHITE_ICON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `MainForm.ApplyIconSetting`: `ShareXResources.UseWhiteIcon`.
pub fn set_white_icon(white: bool) {
    WHITE_ICON.store(white, std::sync::atomic::Ordering::Relaxed);
}

/// `ShareXResources.IconBytes`: the white icon with `UseWhiteShareXIcon`,
/// else the app's, at `size` pixels square.
pub fn current_icon(size: u32) -> RgbaImage {
    let icon = app_icon(size);
    if WHITE_ICON.load(std::sync::atomic::Ordering::Relaxed) { white_icon(&icon) } else { icon }
}

/// ShareX's white icon (`ShareX_Icon_White.ico`) is its logo's silhouette in
/// white. ShareXrs's logo is a blue square with a white "S": its white icon
/// is the square in white with the "S" cut out. Each pixel of the colour
/// icon is a blend of the blue (#2563EB) and white; the white icon keeps the
/// blue part's share as its alpha.
pub fn white_icon(icon: &RgbaImage) -> RgbaImage {
    const BLUE_R: f32 = 0x25 as f32;
    let mut out = icon.clone();
    for p in out.pixels_mut() {
        // How much of the pixel is blue (0 for the white "S").
        let blue = ((255.0 - p[0] as f32) / (255.0 - BLUE_R)).clamp(0.0, 1.0);
        let a = (p[3] as f32 * blue).round() as u8;
        *p = image::Rgba([255, 255, 255, a]);
    }
    out
}

/// `TaskManager.UpdateTrayIcon`'s colour for the progress.
const PROGRESS_COLOR: [u8; 3] = [16, 116, 193];

/// `Helpers.GetProgressIconBytes(percentage, color)`: a dark square filled
/// from the bottom up to the percentage, a lighter line on the fill's top
/// edge, the percentage (at most 99) in white Arial 10pt in the middle, and
/// the four corner pixels cleared.
pub fn progress_icon(size: u32, percentage: i32) -> RgbaImage {
    progress_icon_in(size, percentage, PROGRESS_COLOR)
}

/// [`progress_icon`] filled with `color` (the recording frame's encoding
/// progress is #8C0024).
pub fn progress_icon_in(size: u32, percentage: i32, color: [u8; 3]) -> RgbaImage {
    let percentage = percentage.clamp(0, 100);
    let s = size.max(1);
    let mut img = RgbaImage::from_pixel(s, s, image::Rgba([39, 39, 39, 255]));
    let height = (s as f32 * (percentage as f32 / 100.0)) as u32;
    let [r, g, b] = color;
    for y in s - height..s {
        for x in 0..s {
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }
    if height > 0 && height < s {
        // `ColorHelpers.LighterColor(color, 0.3f)`, a 1px antialiased line
        // from (0, y) to (size - 1, y): half on each row it straddles.
        let lighter = color.map(|c| (c as f32 + (255.0 - c as f32) * 0.3) as u8);
        let y = s - height;
        for row in [y - 1, y] {
            for x in 0..s - 1 {
                let p = img.get_pixel_mut(x, row);
                for c in 0..3 {
                    p[c] = (p[c] as u16 + lighter[c] as u16).div_ceil(2) as u8;
                }
            }
        }
    }
    draw_label(&mut img, &percentage.min(99).to_string());
    for (x, y) in [(0, 0), (s - 1, 0), (0, s - 1), (s - 1, s - 1)] {
        img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
    }
    img
}

/// Arial (`ImageFont("Arial", 10)`: 10pt, 13.33px), else the bundled Inter.
fn label_font() -> Option<ab_glyph::FontVec> {
    use ab_glyph::FontVec;
    let system = if cfg!(windows) {
        std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts").join("arial.ttf"))
    } else if cfg!(target_os = "macos") {
        Some("/System/Library/Fonts/Supplemental/Arial.ttf".into())
    } else {
        Some("/usr/share/fonts/truetype/msttcorefonts/Arial.ttf".into())
    };
    system
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|d| FontVec::try_from_vec(d).ok())
        .or_else(|| FontVec::try_from_vec(sharex_ui::fonts::INTER_REGULAR.to_vec()).ok())
}

/// Draws `label` in white, its ink centred: `MeasureText`'s bounds, then
/// `DrawText` at `(size - width) / 2 - left`, `(size - height) / 2 - top`.
fn draw_label(img: &mut RgbaImage, label: &str) {
    use ab_glyph::{Font, ScaleFont, point};
    let Some(font) = label_font() else { return };
    let px = 10.0 * 96.0 / 72.0;
    let scaled = font.as_scaled(px);
    let (sx, sy) = (scaled.h_scale_factor(), scaled.v_scale_factor());
    // Ink bounds, y down, with the baseline at 0.
    let mut pen = 0.0;
    let mut glyphs = Vec::new();
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for c in label.chars() {
        let id = font.glyph_id(c);
        if let Some(o) = font.outline(id) {
            left = left.min(pen + o.bounds.min.x * sx);
            right = right.max(pen + o.bounds.max.x * sx);
            top = top.min(-o.bounds.max.y * sy);
            bottom = bottom.max(-o.bounds.min.y * sy);
        }
        glyphs.push((id, pen));
        pen += scaled.h_advance(id);
    }
    if left > right {
        return;
    }
    let size = img.width() as f32;
    let x = (size - (right - left)) / 2.0 - left;
    let y = (size - (bottom - top)) / 2.0 - top;
    for (id, at) in glyphs {
        let glyph = id.with_scale_and_position(px, point(x + at, y));
        let Some(outlined) = font.outline_glyph(glyph) else { continue };
        let b = outlined.px_bounds();
        outlined.draw(|gx, gy, coverage| {
            let (tx, ty) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
            if tx < 0 || ty < 0 || tx as u32 >= img.width() || ty as u32 >= img.height() {
                return;
            }
            let p = img.get_pixel_mut(tx as u32, ty as u32);
            let a = coverage.clamp(0.0, 1.0);
            for c in 0..3 {
                p[c] = (p[c] as f32 * (1.0 - a) + 255.0 * a).round() as u8;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_native_menu_has_the_tray_menus_entries() {
        let state = crate::main_menu::MenuState::default();
        let entries = TrayMenuEntries::new(&crate::main_menu::tray_menu(&state));
        let headers: Vec<&str> = entries.native.iter().filter(|e| !e.separator).map(|e| e.header.as_str()).collect();
        assert_eq!(headers.first(), Some(&"Capture"));
        assert_eq!(headers.last(), Some(&"Exit"));
        // Recent items is hidden with no recent tasks.
        assert!(!headers.contains(&"Recent items"));
        // Tools: a submenu per category.
        let tools = entries.native.iter().find(|e| e.header == "Tools").unwrap();
        assert_eq!(tools.children.as_ref().unwrap()[0].header, "Screen & color");
        let exit = entries.native.iter().find(|e| e.header == "Exit").unwrap().command.unwrap();
        assert_eq!(entries.commands[exit], crate::main_menu::MenuCommand::Exit);
    }

    #[test]
    fn the_app_icon_comes_at_any_size() {
        assert_eq!(app_icon(16).dimensions(), (16, 16));
        assert_eq!(app_icon(20).dimensions(), (20, 20));
        assert_eq!(app_icon(24).dimensions(), (24, 24));
    }

    #[test]
    fn the_white_icon_is_the_square_without_the_s() {
        let icon = app_icon(64);
        let white = white_icon(&icon);
        // The corner stays transparent, the blue becomes opaque white, the
        // middle of the "S" stroke goes.
        assert_eq!(white.get_pixel(0, 0)[3], icon.get_pixel(0, 0)[3]);
        let blue = (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).find(|&(x, y)| icon.get_pixel(x, y)[0] < 0x40 && icon.get_pixel(x, y)[3] == 255).unwrap();
        assert_eq!(white.get_pixel(blue.0, blue.1), &image::Rgba([255, 255, 255, 255]));
        let s = (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).find(|&(x, y)| icon.get_pixel(x, y)[0] > 0xF0 && icon.get_pixel(x, y)[3] == 255).unwrap();
        assert!(white.get_pixel(s.0, s.1)[3] < 16);
    }

    #[test]
    fn progress_fills_from_the_bottom() {
        let img = progress_icon(16, 50);
        // Dark above, blue below, the corners cleared.
        assert_eq!(img.get_pixel(1, 1), &image::Rgba([39, 39, 39, 255]));
        assert_eq!(img.get_pixel(1, 14), &image::Rgba([16, 116, 193, 255]));
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert_eq!(img.get_pixel(15, 15)[3], 0);
        // The label is white somewhere in the middle.
        assert!((4..12).any(|x| (4..12).any(|y| img.get_pixel(x, y)[0] > 200)));
        assert_eq!(progress_icon(16, 0).get_pixel(1, 14), &image::Rgba([39, 39, 39, 255]));
    }
}
