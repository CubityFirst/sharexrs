//! `MacOSWindowImageEffect` (`Core/ImageEffects/Filters/MacOSWindowImageEffect.cs`).

use crate::effects::prelude::*;

/// `MacOSWindowImageEffect`.
#[derive(Clone, Debug)]
pub struct MacOSWindowImageEffect {
    pub title: String,
    pub corner_radius: i32,
    pub dark_mode: bool,
    pub shadow_size: i32,
    pub padding: i32,
}

impl Default for MacOSWindowImageEffect {
    fn default() -> Self {
        MacOSWindowImageEffect {
            title: "".to_string(),
            corner_radius: 10,
            dark_mode: false,
            shadow_size: 20,
            padding: 20,
        }
    }
}

impl EffectImpl for MacOSWindowImageEffect {
    fn info() -> Info {
        Info::new("macos_window", "macOS", EffectCategory::Filters, "app/monitor")
            .desc("Wraps the image in a macOS-style window frame with a title bar and traffic light buttons.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("title", "Title", "", |e: &mut Self, v| e.title = v),
            int_slider::<Self>(
                "corner_radius",
                "Corner radius",
                0,
                30,
                10,
                |e: &mut Self, v| e.corner_radius = v,
            ),
            bool_param::<Self>("dark_mode", "Dark mode", false, |e: &mut Self, v| {
                e.dark_mode = v
            }),
            int_slider::<Self>(
                "shadow_size",
                "Shadow size",
                0,
                60,
                20,
                |e: &mut Self, v| e.shadow_size = v,
            ),
            int_slider::<Self>("padding", "Padding", 0, 100, 20, |e: &mut Self, v| {
                e.padding = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const TITLE_BAR: i32 = 38;
        const BUTTON_RADIUS: f32 = 6.0;
        const BUTTON_SPACING: f32 = 20.0;
        const BUTTON_LEFT: f32 = 14.0;
        let corner = self.corner_radius.clamp(0, 30) as f32;
        let shadow = self.shadow_size.clamp(0, 60);
        let padding = self.padding.clamp(0, 100);
        let (ww, wh) = (source.width() as i32, TITLE_BAR + source.height() as i32);
        let tw = ww + padding * 2 + shadow * 2;
        let th = wh + padding * 2 + shadow * 2;
        let mut out = Bitmap::new(tw.max(0) as u32, th.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let wx = (padding + shadow) as f32;
        let wy = (padding + shadow) as f32;
        let window = RRect::ltrb(wx, wy, wx + ww as f32, wy + wh as f32);
        if shadow > 0 {
            canvas.draw_round_rect_mask_blur(
                window,
                corner,
                corner,
                shadow as f32 / 2.0,
                &solid(SkColor::rgba(0, 0, 0, 60), true),
            );
        }
        let clip = round_rect_path(window, corner);
        let dark = self.dark_mode;
        with_window_clip(&mut canvas, window, corner, &clip, |canvas| {
            let bar = if dark {
                SkColor::rgb(56, 55, 57)
            } else {
                SkColor::rgb(232, 232, 232)
            };
            let bar_border = if dark {
                SkColor::rgb(45, 44, 46)
            } else {
                SkColor::rgb(208, 208, 208)
            };
            sk::draw_rect_xywh(
                canvas,
                wx,
                wy,
                ww as f32,
                TITLE_BAR as f32,
                &solid(bar, true),
                None,
            );
            let ty = wy + TITLE_BAR as f32;
            sk::draw_line(
                canvas,
                wx,
                ty,
                wx + ww as f32,
                ty,
                &Pen::new(1.0),
                &solid(bar_border, true),
            );
            let by = wy + TITLE_BAR as f32 / 2.0;
            let bx = wx + BUTTON_LEFT + BUTTON_RADIUS;
            traffic_button(canvas, bx, by, SkColor::rgb(255, 95, 87));
            traffic_button(canvas, bx + BUTTON_SPACING, by, SkColor::rgb(255, 189, 46));
            traffic_button(
                canvas,
                bx + BUTTON_SPACING * 2.0,
                by,
                SkColor::rgb(39, 201, 63),
            );
            if !is_blank(&self.title) {
                let color = if dark {
                    SkColor::rgb(220, 220, 220)
                } else {
                    SkColor::rgb(74, 74, 74)
                };
                let font = Font::new(Typeface::from_family("San Francisco", 500, false), 13.0);
                let text_y = wy + (TITLE_BAR as f32 / 2.0) + (13.0 / 3.0);
                font.draw(
                    canvas,
                    &self.title,
                    wx + ww as f32 / 2.0,
                    text_y,
                    Align::Center,
                    &solid(color, true),
                );
            }
            canvas.draw_bitmap(source, wx, wy + TITLE_BAR as f32, None);
        });
        let border = if dark {
            SkColor::rgba(70, 70, 70, 180)
        } else {
            SkColor::rgba(180, 180, 180, 180)
        };
        sk::draw_round_rect(
            &mut canvas,
            window,
            corner,
            corner,
            &solid(border, true),
            Some(&Pen::new(1.0)),
        );
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

/// `ClipRoundRect(SKRoundRect(rect, r, r), Intersect, antialias)`: a square
/// round rect is `clipRect`, and an anti-aliased rect on whole pixels is an
/// aliased clip (`SkRasterClip::op`); otherwise the round rect's path, AA.
fn with_window_clip(
    canvas: &mut Canvas,
    rect: RRect,
    r: f32,
    path: &SkPath,
    draw: impl FnOnce(&mut Canvas),
) {
    let whole = |v: f32| (v - v.round()).abs() <= 1.0 / 4096.0;
    if r <= 0.0 && whole(rect.left) && whole(rect.top) && whole(rect.right) && whole(rect.bottom) {
        sk::with_clip_rect(canvas, rect, draw);
    } else {
        sk::with_clip_path_aa(canvas, path, draw);
    }
}

/// The path `ClipRoundRect(SKRoundRect(rect, r, r))` clips to.
fn round_rect_path(rect: RRect, r: f32) -> SkPath {
    let g = sharex_skia::Rect::from_ltrb(rect.left, rect.top, rect.right, rect.bottom);
    let mut p = SkPath::new();
    if r > 0.0 {
        let (w, h) = (rect.width(), rect.height());
        let (mut rx, mut ry) = (r, r);
        if w < rx + rx || h < ry + ry {
            let s = (w / (rx + rx)).min(h / (ry + ry));
            rx *= s;
            ry *= s;
        }
        if rx >= w * 0.5 && ry >= h * 0.5 {
            p.add_oval(g, sharex_skia::Direction::Cw);
        } else {
            p.add_round_rect(g, rx, ry, sharex_skia::Direction::Cw);
        }
    } else {
        p.add_rect(g, sharex_skia::Direction::Cw, 0);
    }
    p
}

fn traffic_button(canvas: &mut Canvas, x: f32, y: f32, color: SkColor) {
    sk::draw_circle(canvas, x, y, 6.0, &solid(color, true), None);
    let s = |v: u8| (v as f32 * 0.8) as u8;
    let stroke = solid(SkColor::rgba(s(color.r), s(color.g), s(color.b), 200), true);
    sk::draw_circle(canvas, x, y, 6.0, &stroke, Some(&Pen::new(0.5)));
}
