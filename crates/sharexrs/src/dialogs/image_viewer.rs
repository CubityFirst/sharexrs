//! `ShareX.HelpersLib/Presentation/ImageViewer`: `ImageViewerWindow` and
//! its view model. Full screen on the active monitor, borderless, topmost:
//! the image centred on the checkered background (shrunk to fit, never
//! enlarged), its status (`n / count | name | width × height`) at the top,
//! the previous/next buttons at the sides. Ctrl+wheel zooms about the
//! pointer, the wheel and the arrow keys go through the folder's images,
//! a left or right click (or Escape, Enter, Space) closes it, a middle click
//! resets the zoom; it closes when it loses the focus.

use std::path::{Path, PathBuf};

use egui::{Color32, Rect, Sense, Vec2, pos2, vec2};
use image::RgbaImage;
use sharex_strings::helpers_lib as hs;
use sharex_ui::controls::Button;
use sharex_ui::keys::app_icons as icon;
use sharex_ui::theme::palette;
use sharex_ui::widgets::{center_y, font, galley, line_height, text_width};
use sharex_ui::icons;

use super::{Cx, Dialog, Effect, Spec};
use crate::history_ui::common::tool_panel;

/// `MinZoom`, `MaxZoom`, `ZoomFactor`.
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 10.0;
const ZOOM_FACTOR: f32 = 1.2;
/// `ShareX.Brush.Checkered`: 15px cells.
const CHECKER: f32 = 15.0;

/// `ImageViewerViewModel`: the images and the one shown.
#[derive(Default)]
pub struct Images {
    /// `_images`: the files to go through (none for an encoded image).
    pub files: Vec<PathBuf>,
    pub index: usize,
    /// `CurrentImage`.
    pub image: Option<RgbaImage>,
    /// `CurrentImageFilePath`: a file, or an encoded image's display name.
    pub name: Option<String>,
    /// `SupportWrap`.
    pub wrap: bool,
}

impl Images {
    /// `LoadFile`: the image and the other images in its folder.
    pub fn load_file(&mut self, path: &Path) -> bool {
        if !path.is_file() || !sharex_helpers::file_helpers::is_image_file(&path.to_string_lossy()) {
            return false;
        }
        let mut files: Vec<PathBuf> = path
            .parent()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_file() && sharex_helpers::file_helpers::is_image_file(&p.to_string_lossy())).collect())
            .unwrap_or_default();
        // `Directory.GetFiles`: NTFS's order, by name.
        files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default());
        let index = files.iter().position(|p| p.to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy()));
        match index {
            Some(i) => {
                self.files = files;
                self.index = i;
            }
            None => {
                self.files = vec![path.to_path_buf()];
                self.index = 0;
            }
        }
        self.load_current()
    }

    /// `LoadFiles`: the image files among `paths`, `selected` shown.
    pub fn load_files(&mut self, paths: &[PathBuf], selected: usize) -> bool {
        let mut files = Vec::new();
        let mut filtered = None;
        for (i, p) in paths.iter().enumerate() {
            if !p.is_file() || !sharex_helpers::file_helpers::is_image_file(&p.to_string_lossy()) {
                continue;
            }
            if i == selected {
                filtered = Some(files.len());
            }
            files.push(p.clone());
        }
        if files.is_empty() {
            return false;
        }
        self.index = filtered.unwrap_or(selected.min(files.len() - 1));
        self.files = files;
        self.load_current()
    }

    /// `LoadEncodedImage`: one image, no folder.
    pub fn load_image(&mut self, image: RgbaImage, name: Option<String>) -> bool {
        self.files.clear();
        self.index = 0;
        self.image = Some(image);
        self.name = name;
        true
    }

    fn load_current(&mut self) -> bool {
        let Some(path) = self.files.get(self.index).cloned() else { return false };
        self.name = Some(path.to_string_lossy().into_owned());
        match crate::thumbnail::load(&path) {
            Some(img) => {
                self.image = Some(img);
                true
            }
            None => {
                eprintln!("Failed to load image '{}'.", path.display());
                false
            }
        }
    }

    pub fn can_navigate(&self) -> bool {
        self.files.len() > 1
    }

    pub fn can_navigate_left(&self) -> bool {
        self.can_navigate() && (self.wrap || self.index > 0)
    }

    pub fn can_navigate_right(&self) -> bool {
        self.can_navigate() && (self.wrap || self.index + 1 < self.files.len())
    }

    /// `Navigate(offset)`; whether another image is shown.
    pub fn navigate(&mut self, offset: i32) -> bool {
        if !self.can_navigate() {
            return false;
        }
        let n = self.files.len() as i32;
        let mut next = self.index as i32 + offset;
        next = if self.wrap { (next + n) % n } else { next.clamp(0, n - 1) };
        if next as usize != self.index {
            self.index = next as usize;
            return self.load_current();
        }
        false
    }

    /// `UpdateStatus`.
    pub fn status(&self) -> String {
        let mut parts = Vec::new();
        if self.can_navigate() {
            parts.push(format!("{} / {}", self.index + 1, self.files.len()));
        }
        if let Some(name) = self.name.as_deref().filter(|n| !n.trim().is_empty()) {
            let file_name = Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_owned());
            let chars: Vec<char> = file_name.chars().collect();
            parts.push(if chars.len() > 128 { format!("{}...", chars[..125].iter().collect::<String>()) } else { file_name });
        }
        if let Some(img) = &self.image {
            parts.push(format!("{} × {}", img.width(), img.height()));
        }
        parts.join("  |  ")
    }
}

pub struct ImageViewerWindow {
    pub images: Images,
    /// The active monitor (physical pixels) and its scale.
    screen: sharex_capture::Rect,
    scale: f32,
    texture: Option<egui::TextureHandle>,
    /// `_previewTransform`: zoom and offset (points).
    zoom: f32,
    offset: Vec2,
    /// `_closeOnDeactivate`.
    close_on_deactivate: bool,
    /// `PlayAnimation`: the current file's animation, at full size, until
    /// another image is shown (`_imageVersion`).
    animation: Option<(PathBuf, crate::animation::Player)>,
}

impl ImageViewerWindow {
    pub fn new(images: Images) -> Self {
        let screens = sharex_capture::screenshot::screens();
        let cursor = sharex_capture::screenshot::cursor_position();
        // `CaptureHelpers.GetActiveScreenBounds()`.
        let screen = cursor
            .and_then(|(x, y)| screens.iter().find(|s| s.bounds.contains(x, y)))
            .or(screens.iter().find(|s| s.primary))
            .or(screens.first());
        let (bounds, scale) = screen.map_or((sharex_capture::Rect::new(0, 0, 1280, 720), 1.0), |s| (s.bounds, s.scale as f32));
        let close_on_deactivate = images.image.is_some();
        Self { images, screen: bounds, scale, texture: None, zoom: 1.0, offset: Vec2::ZERO, close_on_deactivate, animation: None }
    }

    /// For `--render`: a screen of this size (points).
    pub fn with_size(mut self, w: i32, h: i32) -> Self {
        self.screen = sharex_capture::Rect::new(0, 0, w, h);
        self.scale = 1.0;
        self
    }

    /// `PlayAnimation(CurrentImageFilePath)` and `ShowFrame`: frames replace
    /// the image but keep the zoom.
    fn play(&mut self, ctx: &egui::Context) {
        let current = self.images.files.get(self.images.index).cloned();
        if self.animation.as_ref().map(|(p, _)| p) != current.as_ref() {
            self.animation = current
                .filter(|p| sharex_tools::animated_preview::can_be_animated(&p.to_string_lossy()))
                .map(|p| (p.clone(), crate::animation::Player::start(p, u32::MAX, Box::new(crate::animation::straight))));
        }
        let Some((_, player)) = &mut self.animation else { return };
        match player.poll(std::time::Instant::now()) {
            crate::animation::Poll::Frame(frame) => {
                let ci = egui::ColorImage::from_rgba_unmultiplied([frame.width() as usize, frame.height() as usize], frame.as_raw());
                match &mut self.texture {
                    Some(t) if t.size() == ci.size => t.set(ci, egui::TextureOptions::LINEAR),
                    t => *t = Some(ctx.load_texture("image-viewer", ci, egui::TextureOptions::LINEAR)),
                }
                self.images.image = Some(frame);
                ctx.request_repaint();
            }
            crate::animation::Poll::Wait(d) => ctx.request_repaint_after(d),
            crate::animation::Poll::NotAnimated | crate::animation::Poll::Ended => {}
        }
    }

    fn reset_view(&mut self) {
        self.texture = None;
        self.zoom = 1.0;
        self.offset = Vec2::ZERO;
    }

    /// A previous/next `Button` (72x96, a 38px chevron).
    fn nav_button(ui: &egui::Ui, rect: Rect, id: egui::Id, key: &str) -> egui::Response {
        let p = palette(ui.ctx());
        let b = Button::default();
        let r = b.show(ui, rect, id, "", true);
        let c = b.foreground(ui, &r, true, p.background_main);
        icons::paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(38.0)), key, c);
        r
    }
}

impl Dialog for ImageViewerWindow {
    fn spec(&self) -> Spec {
        let (w, h) = (self.screen.w as f32 / self.scale, self.screen.h as f32 / self.scale);
        Spec { topmost: true, maximizable: false, resizable: false, borderless: true, ..Spec::sized(hs::ImageViewerWindow_Title(), w, h, 0.0, 0.0) }
    }

    fn position(&self, _work_area: sharex_capture::Rect, _size: (u32, u32)) -> Option<(i32, i32)> {
        Some((self.screen.x, self.screen.y))
    }

    fn deactivated(&mut self) -> Vec<Effect> {
        if self.close_on_deactivate { vec![Effect::Close] } else { Vec::new() }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx) -> Vec<Effect> {
        let p = palette(ui.ctx());
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.background_main);
        // The checkered background.
        let painter = ui.painter().with_clip_rect(full);
        painter.rect_filled(full, 0.0, p.checkered[0]);
        let (cols, rows) = ((full.width() / CHECKER).ceil() as i32, (full.height() / CHECKER).ceil() as i32);
        for row in 0..rows {
            for col in 0..cols {
                if (row + col) % 2 == 0 {
                    let r = Rect::from_min_size(pos2(full.min.x + col as f32 * CHECKER, full.min.y + row as f32 * CHECKER), Vec2::splat(CHECKER));
                    painter.rect_filled(r, 0.0, p.checkered[1]);
                }
            }
        }
        let mut out = Vec::new();

        self.play(ui.ctx());
        // The image: `Stretch="Uniform" StretchDirection="DownOnly"`.
        let mut image_rect = None;
        if let Some(img) = &self.images.image {
            if self.texture.is_none() {
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                self.texture = Some(ui.ctx().load_texture("image-viewer", ci, egui::TextureOptions::LINEAR));
            }
            let ppp = ui.ctx().pixels_per_point();
            let (iw, ih) = (img.width() as f32 / ppp, img.height() as f32 / ppp);
            let fit = (full.width() / iw).min(full.height() / ih).min(1.0);
            let size = vec2(iw * fit, ih * fit);
            let base = Rect::from_min_size(pos2(full.min.x + ((full.width() - size.x) / 2.0).round(), full.min.y + ((full.height() - size.y) / 2.0).round()), size);
            let shown = Rect::from_min_size(base.min + self.offset, size * self.zoom);
            if let Some(t) = &self.texture {
                painter.image(t.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
            image_rect = Some(base);
        }

        // Ctrl+wheel zooms about the pointer; the wheel goes through the images.
        // The wheel in notches (`PointerWheelEventArgs.Delta.Y`).
        let (wheel, ctrl) = ui.input(|i| {
            let notches: f32 = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, .. } => Some(delta.y / 50.0),
                    egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Page, delta, .. } => Some(delta.y * 3.0),
                    egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                    _ => None,
                })
                .sum();
            (notches, i.modifiers.ctrl)
        });
        if wheel != 0.0 && ui.rect_contains_pointer(full) {
            if ctrl {
                if let (Some(base), Some(pos)) = (image_rect, ui.input(|i| i.pointer.hover_pos())) {
                    let zoom = (self.zoom * ZOOM_FACTOR.powf(wheel)).clamp(MIN_ZOOM, MAX_ZOOM);
                    let local = (pos - base.min - self.offset) / self.zoom;
                    self.offset += local * (self.zoom - zoom);
                    self.zoom = zoom;
                }
            } else if wheel > 0.0 && self.images.can_navigate_left() {
                if self.images.navigate(-1) {
                    self.reset_view();
                }
            } else if wheel < 0.0 && self.images.can_navigate_right() && self.images.navigate(1) {
                self.reset_view();
            }
        }

        // The status.
        if self.images.image.is_some() {
            let status = self.images.status();
            let tf = font(13.0, 400);
            let w = text_width(ui, &status, tf.clone()) + 20.0 + 2.0;
            let h = line_height(ui, &tf) + 14.0 + 2.0;
            let panel = Rect::from_min_size(pos2(full.min.x + ((full.width() - w) / 2.0).round_ties_even(), full.min.y + 16.0), vec2(w, h));
            tool_panel(ui, panel);
            ui.painter().galley(pos2(panel.min.x + 11.0, panel.min.y + 8.0), galley(ui, &status, tf, p.text_secondary, f32::INFINITY), p.text_secondary);
        }

        // Previous and next.
        let mut nav_rects = Vec::new();
        let mid = Rect::from_min_max(full.min, full.max);
        if self.images.can_navigate_left() {
            let r = Rect::from_min_size(pos2(full.min.x + 20.0, center_y(mid, 96.0)), vec2(72.0, 96.0));
            nav_rects.push(r);
            if Self::nav_button(ui, r, ui.id().with("previous"), icon::ChevronLeft).clicked() && self.images.navigate(-1) {
                self.reset_view();
            }
        }
        if self.images.can_navigate_right() {
            let r = Rect::from_min_size(pos2(full.max.x - 20.0 - 72.0, center_y(mid, 96.0)), vec2(72.0, 96.0));
            nav_rects.push(r);
            if Self::nav_button(ui, r, ui.id().with("next"), icon::ChevronRight).clicked() && self.images.navigate(1) {
                self.reset_view();
            }
        }

        // Clicks on the preview (`OnPreviewPointerReleased`).
        let over_nav = ui.input(|i| i.pointer.interact_pos()).is_some_and(|pos| nav_rects.iter().any(|r| r.contains(pos)));
        let bg = ui.interact(full, ui.id().with("background"), Sense::click());
        let _ = &bg;
        if !over_nav {
            let released = ui.input(|i| {
                (i.pointer.button_released(egui::PointerButton::Primary) || i.pointer.button_released(egui::PointerButton::Secondary), i.pointer.button_released(egui::PointerButton::Middle))
            });
            if released.0 {
                out.push(Effect::Close);
                return out;
            }
            if released.1 {
                self.zoom = 1.0;
                self.offset = Vec2::ZERO;
            }
        }

        // Keys.
        let (left, right, close) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space),
            )
        });
        if close {
            out.push(Effect::Close);
        } else if left && self.images.navigate(-1) || right && self.images.navigate(1) {
            self.reset_view();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_navigation() {
        let dir = std::env::temp_dir().join(format!("sharexrs-viewer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["b.png", "a.png", "c.txt"] {
            RgbaImage::new(3, 2).save_with_format(dir.join(n), image::ImageFormat::Png).ok();
        }
        std::fs::write(dir.join("c.txt"), "x").unwrap();
        let mut images = Images::default();
        assert!(images.load_file(&dir.join("b.png")));
        assert_eq!(images.files.len(), 2);
        assert_eq!(images.index, 1);
        assert_eq!(images.status(), "2 / 2  |  b.png  |  3 × 2");
        assert!(images.can_navigate_left() && !images.can_navigate_right());
        assert!(images.navigate(-1));
        assert_eq!(images.status(), "1 / 2  |  a.png  |  3 × 2");
        assert!(!images.navigate(-1));
        let mut one = Images::default();
        one.load_image(RgbaImage::new(5, 4), None);
        assert_eq!(one.status(), "5 × 4");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
