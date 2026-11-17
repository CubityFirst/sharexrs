//! ShareX's icon packs (`ShareX.Icons`), from the same SVG files.
//!
//! An icon's key is ShareX's (`app/camera`, the `AppIcons.Camera`
//! constant); its file is the key with `/` as `.` in `assets/icons/modern`
//! or `assets/icons/legacy`. Modern icons are line art in `currentColor`
//! and take the colour they're drawn in, like ShareX's `IconPresenter`
//! `Foreground`; Legacy ones are full colour, with a `-white` variant for
//! dark backgrounds, and fall back to Modern where they have no file
//! (`LegacyIcons.IsModernOnly`, see `ShareX.Icons/README.md`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Color32, ColorImage, Painter, Rect, TextureHandle, TextureOptions, Ui, Vec2, pos2};
use include_dir::{Dir, include_dir};

static ICONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/assets/icons");

/// Application settings › Icon pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IconPack {
    #[default]
    Modern,
    Legacy,
}

static PACK: AtomicU8 = AtomicU8::new(0);

pub fn set_pack(pack: IconPack) {
    PACK.store(pack as u8, Ordering::Relaxed);
}

pub fn pack() -> IconPack {
    if PACK.load(Ordering::Relaxed) == 1 { IconPack::Legacy } else { IconPack::Modern }
}

/// Whether ShareX has an icon with this key.
pub fn exists(key: &str) -> bool {
    ICONS.get_file(format!("modern/{}.svg", key.replace('/', "."))).is_some()
}

/// The SVG for `key` in `pack`, and whether it's tinted (`currentColor`).
fn source(key: &str, pack: IconPack, dark: bool) -> Option<(&'static str, bool)> {
    let file = key.replace('/', ".");
    let modern_only = key.starts_with("app/caption-") || key.starts_with("photo-editor/") || key.starts_with("pixel-editor/");
    if pack == IconPack::Legacy && !modern_only {
        let white = dark.then(|| format!("legacy/{file}-white.svg"));
        for path in white.into_iter().chain([format!("legacy/{file}.svg")]) {
            if let Some(f) = ICONS.get_file(&path) {
                return Some((f.contents_utf8()?, false));
            }
        }
    }
    Some((ICONS.get_file(format!("modern/{file}.svg"))?.contents_utf8()?, true))
}

/// Rasterises an SVG to `side` x `side` pixels. Tinted icons are drawn in
/// white, for egui to multiply by the colour they're shown in.
fn rasterize(svg: &str, tinted: bool, side: u32) -> Option<ColorImage> {
    let svg = if tinted { svg.replace("currentColor", "#FFFFFF") } else { svg.to_string() };
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(side, side)?;
    let size = tree.size();
    let scale = side as f32 / size.width().max(size.height());
    let (dx, dy) = ((side as f32 - size.width() * scale) / 2.0, (side as f32 - size.height() * scale) / 2.0);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy),
        &mut pixmap.as_mut(),
    );
    Some(ColorImage::from_rgba_premultiplied([side as usize, side as usize], pixmap.data()))
}

type Cache = Arc<Mutex<HashMap<(String, u32, IconPack, bool), Option<(TextureHandle, bool)>>>>;

/// The texture for `key` at `side` physical pixels, and whether it's tinted.
fn texture(ctx: &egui::Context, key: &str, side: u32) -> Option<(TextureHandle, bool)> {
    let pack = pack();
    let dark = ctx.theme() == egui::Theme::Dark;
    let cache: Cache = ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(egui::Id::new("sharex-icons")).clone());
    let mut cache = cache.lock().unwrap();
    cache
        .entry((key.to_string(), side, pack, dark))
        .or_insert_with(|| {
            let (svg, tinted) = source(key, pack, dark)?;
            let image = rasterize(svg, tinted, side)?;
            let tex = ctx.load_texture(format!("icon:{key}@{side}"), image, TextureOptions::LINEAR);
            Some((tex, tinted))
        })
        .clone()
}

/// Draws icon `key` filling `rect` (square), in `color` if it's a Modern
/// icon. Missing icons draw nothing.
pub fn paint(painter: &Painter, rect: Rect, key: &str, color: Color32) {
    let ppp = painter.ctx().pixels_per_point();
    let side = (rect.width().max(rect.height()) * ppp).round().max(1.0) as u32;
    // On whole physical pixels, so 1px strokes stay sharp.
    let min = pos2((rect.min.x * ppp).round() / ppp, (rect.min.y * ppp).round() / ppp);
    let rect = Rect::from_min_size(min, Vec2::splat(side as f32 / ppp));
    if let Some((tex, tinted)) = texture(painter.ctx(), key, side) {
        let tint = if tinted { color } else { Color32::WHITE.gamma_multiply(color.a() as f32 / 255.0) };
        painter.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
    }
}

/// [`paint`] with a render transform: `offset` (points) and `angle`
/// (radians, clockwise) about the icon's centre, as XAML's
/// `RenderTransform="translateY(..) rotate(..)"` with a 50%,50% origin.
pub fn paint_transformed(painter: &Painter, rect: Rect, key: &str, color: Color32, offset: Vec2, angle: f32) {
    if angle == 0.0 {
        return paint(painter, rect.translate(offset), key, color);
    }
    let ppp = painter.ctx().pixels_per_point();
    let side = (rect.width().max(rect.height()) * ppp).round().max(1.0) as u32;
    let rect = Rect::from_center_size(rect.center() + offset, Vec2::splat(side as f32 / ppp));
    if let Some((tex, tinted)) = texture(painter.ctx(), key, side) {
        let tint = if tinted { color } else { Color32::WHITE.gamma_multiply(color.a() as f32 / 255.0) };
        let mut mesh = egui::Mesh::with_texture(tex.id());
        mesh.add_rect_with_uv(rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
        mesh.rotate(egui::emath::Rot2::from_angle(angle), rect.center());
        painter.add(mesh);
    }
}

/// Draws a tinted (`currentColor`) SVG given inline, stretched uniformly
/// into `rect` like a XAML `Path` with `Stretch="Uniform"`: for the few
/// shapes ShareX takes from a control template rather than its icon packs
/// (a menu item's submenu chevron). `key` names it in the texture cache.
pub fn paint_svg(painter: &Painter, rect: Rect, key: &str, svg: &str, color: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let (w, h) = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
    let min = pos2((rect.min.x * ppp).round() / ppp, (rect.min.y * ppp).round() / ppp);
    let rect = Rect::from_min_size(min, egui::vec2(w as f32 / ppp, h as f32 / ppp));
    type SvgCache = Arc<Mutex<HashMap<(String, u32, u32), Option<TextureHandle>>>>;
    let cache: SvgCache = ctx.data_mut(|d| d.get_temp_mut_or_default::<SvgCache>(egui::Id::new("sharex-svgs")).clone());
    let tex = cache
        .lock()
        .unwrap()
        .entry((key.to_string(), w, h))
        .or_insert_with(|| {
            let svg = svg.replace("currentColor", "#FFFFFF");
            let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()?;
            let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
            let size = tree.size();
            let scale = (w as f32 / size.width()).min(h as f32 / size.height());
            let (dx, dy) = ((w as f32 - size.width() * scale) / 2.0, (h as f32 - size.height() * scale) / 2.0);
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy),
                &mut pixmap.as_mut(),
            );
            let image = ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data());
            Some(ctx.load_texture(format!("svg:{key}@{w}x{h}"), image, TextureOptions::LINEAR))
        })
        .clone();
    if let Some(tex) = tex {
        painter.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
    }
}

/// An `IconPresenter` of `size` (its `FontSize`), allocated in `ui`.
pub fn show(ui: &mut Ui, key: &str, size: f32, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint(ui.painter(), rect, key, color);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_modern_icon_rasterizes() {
        let dir = ICONS.get_dir("modern").unwrap();
        let mut failed = Vec::new();
        for f in dir.files() {
            let svg = f.contents_utf8().unwrap();
            if rasterize(svg, true, 16).is_none() {
                failed.push(f.path().display().to_string());
            }
        }
        assert!(failed.is_empty(), "{failed:?}");
    }

    #[test]
    fn legacy_falls_back_to_modern() {
        assert!(source("app/caption-close", IconPack::Legacy, true).unwrap().1);
        assert!(source("app/camera", IconPack::Modern, true).unwrap().1);
    }
}
