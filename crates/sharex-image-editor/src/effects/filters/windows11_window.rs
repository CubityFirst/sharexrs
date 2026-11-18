//! `Windows11ImageEffect` (`Core/ImageEffects/Filters/Windows11ImageEffect.cs`).

use crate::effects::prelude::*;

/// `Windows11ImageEffect`.
#[derive(Clone, Debug)]
pub struct Windows11ImageEffect {
    pub title: String,
    pub corner_radius: i32,
    pub dark_mode: bool,
    pub shadow_size: i32,
    pub padding: i32,
}

impl Default for Windows11ImageEffect {
    fn default() -> Self {
        Windows11ImageEffect {
            title: "".to_string(),
            corner_radius: 8,
            dark_mode: false,
            shadow_size: 16,
            padding: 20,
        }
    }
}

impl EffectImpl for Windows11ImageEffect {
    fn info() -> Info {
        Info::new("windows11_window", "Windows 11", EffectCategory::Filters, "app/monitor")
            .desc("Wraps the image in a Windows 11-style window frame with a title bar and window controls.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            text_param::<Self>("title", "Title", "", |e: &mut Self, v| e.title = v),
            int_slider::<Self>(
                "corner_radius",
                "Corner radius",
                0,
                16,
                8,
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
                16,
                |e: &mut Self, v| e.shadow_size = v,
            ),
            int_slider::<Self>("padding", "Padding", 0, 100, 20, |e: &mut Self, v| {
                e.padding = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const TITLE_BAR: i32 = 32;
        const BW: f32 = 46.0;
        let corner = self.corner_radius.clamp(0, 16) as f32;
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
                shadow as f32 / 2.5,
                &solid(SkColor::rgba(0, 0, 0, 40), true),
            );
        }
        let clip = round_rect_path(window, corner);
        let dark = self.dark_mode;
        let bar = if dark {
            SkColor::rgb(32, 32, 32)
        } else {
            SkColor::rgb(243, 243, 243)
        };
        with_window_clip(&mut canvas, window, corner, &clip, |canvas| {
            sk::draw_rect_xywh(
                canvas,
                wx,
                wy,
                ww as f32,
                TITLE_BAR as f32,
                &solid(bar, true),
                None,
            );
            let border = if dark {
                SkColor::rgb(50, 50, 50)
            } else {
                SkColor::rgb(229, 229, 229)
            };
            let ty = wy + TITLE_BAR as f32;
            sk::draw_line(
                canvas,
                wx,
                ty,
                wx + ww as f32,
                ty,
                &Pen::new(1.0),
                &solid(border, true),
            );
            let text = if dark {
                SkColor::rgb(200, 200, 200)
            } else {
                SkColor::rgb(90, 90, 90)
            };
            let right = wx + ww as f32;
            window_button(
                canvas,
                right - BW,
                wy,
                SkColor::rgb(196, 43, 28),
                SkColor::rgb(255, 255, 255),
                Icon::Close,
            );
            window_button(canvas, right - BW * 2.0, wy, bar, text, Icon::Maximize);
            window_button(canvas, right - BW * 3.0, wy, bar, text, Icon::Minimize);
            if !is_blank(&self.title) {
                let color = if dark {
                    SkColor::rgb(220, 220, 220)
                } else {
                    SkColor::rgb(50, 50, 50)
                };
                let font = Font::new(Typeface::from_family("Segoe UI", 400, false), 12.0);
                let text_y = wy + (TITLE_BAR as f32 / 2.0) + (12.0 / 3.0);
                font.draw(
                    canvas,
                    &self.title,
                    wx + 12.0,
                    text_y,
                    Align::Left,
                    &solid(color, true),
                );
            }
            canvas.draw_bitmap(source, wx, wy + TITLE_BAR as f32, None);
        });
        let wb = if dark {
            SkColor::rgba(60, 60, 60, 160)
        } else {
            SkColor::rgba(200, 200, 200, 160)
        };
        sk::draw_round_rect(
            &mut canvas,
            window,
            corner,
            corner,
            &solid(wb, true),
            Some(&Pen::new(1.0)),
        );
        if !dark {
            let mica = sk::linear_gradient(
                (wx, wy),
                (wx, wy + 2.0),
                &[
                    SkColor::rgba(255, 255, 255, 80),
                    SkColor::rgba(255, 255, 255, 0),
                ],
                None,
                true,
            );
            with_window_clip(&mut canvas, window, corner, &clip, |canvas| {
                sk::draw_rect_xywh(canvas, wx, wy, ww as f32, 2.0, &mica, None);
            });
        }
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

#[derive(Clone, Copy)]
enum Icon {
    Close,
    Maximize,
    Minimize,
}

fn window_button(
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    bg: SkColor,
    icon_color: SkColor,
    icon: Icon,
) {
    sk::draw_rect_xywh(canvas, x, y, 46.0, 32.0, &solid(bg, true), None);
    let (cx, cy) = (x + 46.0 / 2.0, y + 32.0 / 2.0);
    let (size, half) = (10.0f32, 5.0f32);
    let paint = solid(icon_color, true);
    match icon {
        Icon::Close => {
            let pen = Pen::new(1.0).cap(Cap::Round);
            sk::draw_line(
                canvas,
                cx - half,
                cy - half,
                cx + half,
                cy + half,
                &pen,
                &paint,
            );
            sk::draw_line(
                canvas,
                cx + half,
                cy - half,
                cx - half,
                cy + half,
                &pen,
                &paint,
            );
        }
        Icon::Maximize => {
            sk::draw_rect_xywh(
                canvas,
                cx - half,
                cy - half,
                size,
                size,
                &paint,
                Some(&Pen::new(1.0)),
            );
        }
        Icon::Minimize => {
            let pen = Pen::new(1.0).cap(Cap::Round);
            sk::draw_line(canvas, cx - half, cy, cx + half, cy, &pen, &paint);
        }
    }
}
