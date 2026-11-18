//! `CrimsonBorderImageEffect` (`Core/ImageEffects/Filters/CrimsonBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CrimsonBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CrimsonBorderImageEffect {
    pub size: i32,
    pub color: SkColor,
    pub glow_strength: f32,
    pub inner_line: bool,
    pub outer_line: bool,
}

impl Default for CrimsonBorderImageEffect {
    fn default() -> Self {
        CrimsonBorderImageEffect {
            size: 12,
            color: SkColor::argb(0xFF990012),
            glow_strength: 50.0,
            inner_line: true,
            outer_line: true,
        }
    }
}

impl EffectImpl for CrimsonBorderImageEffect {
    fn info() -> Info {
        Info::new("crimson_border", "Crimson border", EffectCategory::Filters, "image-effect/sword")
            .desc("Adds a rich crimson/dark red border with an inner glow effect.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("size", "Size", 2, 60, 12, |e: &mut Self, v| e.size = v),
            color_param::<Self>("color", "Color", SkColor::argb(0xFF990012), |e: &mut Self, v| e.color = v),
            float_slider::<Self>("glow_strength", "Glow strength", 0.0, 100.0, 50.0, |e: &mut Self, v| e.glow_strength = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| e.inner_line = v),
            bool_param::<Self>("outer_line", "Outer line", true, |e: &mut Self, v| e.outer_line = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let size = self.size.clamp(2, 60);
        let glow = clampf(self.glow_strength, 0.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + size * 2, ih + size * 2);
        let base = self.color;
        let dark = darken(base, 0.5);
        let light = lighten(base, 0.3);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let (sf, nwf, nhf) = (size as f32, nw as f32, nh as f32);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, nhf, &solid(base, true), None);
        if glow > 0.0 {
            let gw = sf * 0.7;
            let clear = base.with_alpha(0);
            let g = |s: (f32, f32), e: (f32, f32), c: SkColor| sk::linear_gradient(s, e, &[c, clear], None, true);
            let d200 = dark.with_alpha((200.0 * glow) as u8);
            let d180 = dark.with_alpha((180.0 * glow) as u8);
            sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, gw, &g((0.0, 0.0), (0.0, gw), d200), None);
            sk::draw_rect_xywh(&mut canvas, 0.0, nhf - gw, nwf, gw, &g((0.0, nhf), (0.0, nhf - gw), d200), None);
            sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, gw, nhf, &g((0.0, 0.0), (gw, 0.0), d180), None);
            sk::draw_rect_xywh(&mut canvas, nwf - gw, 0.0, gw, nhf, &g((nwf, 0.0), (nwf - gw, 0.0), d180), None);
            let igw = sf * 0.4;
            let l120 = light.with_alpha((120.0 * glow) as u8);
            sk::draw_rect_xywh(&mut canvas, sf, sf - igw, iw as f32, igw, &g((0.0, sf), (0.0, sf - igw), l120), None);
            sk::draw_rect_xywh(&mut canvas, sf, (nh - size) as f32, iw as f32, igw, &g((0.0, (nh - size) as f32), (0.0, (nh - size) as f32 + igw), l120), None);
        }
        canvas.draw_bitmap(source, sf, sf, None);
        if self.inner_line {
            let p = solid(light.with_alpha(180), true);
            sk::draw_rect_xywh(&mut canvas, sf - 0.5, sf - 0.5, (iw + 1) as f32, (ih + 1) as f32, &p, Some(&Pen::new(1.0)));
        }
        if self.outer_line {
            sk::draw_rect_xywh(&mut canvas, 0.5, 0.5, (nw - 1) as f32, (nh - 1) as f32, &solid(dark, true), Some(&Pen::new(1.5)));
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn darken(c: SkColor, f: f32) -> SkColor {
    let d = |v: u8| (v as f32 * (1.0 - f)) as u8;
    SkColor::rgba(d(c.r), d(c.g), d(c.b), c.a)
}

fn lighten(c: SkColor, f: f32) -> SkColor {
    let l = |v: u8| 255f32.min(v as f32 + (255 - v) as f32 * f) as u8;
    SkColor::rgba(l(c.r), l(c.g), l(c.b), c.a)
}
