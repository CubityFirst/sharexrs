//! `NeonGlowBorderImageEffect` (`Core/ImageEffects/Filters/NeonGlowBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `NeonGlowBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct NeonGlowBorderImageEffect {
    pub border_size: i32,
    pub neon_color: SkColor,
    pub glow_intensity: f32,
    pub tube_width: f32,
    pub bg_color: SkColor,
    pub double_tube: bool,
}

impl Default for NeonGlowBorderImageEffect {
    fn default() -> Self {
        NeonGlowBorderImageEffect {
            border_size: 50,
            neon_color: SkColor::argb(0xFF00FFC8),
            glow_intensity: 75.0,
            tube_width: 4.0,
            bg_color: SkColor::argb(0xFF0A0A12),
            double_tube: false,
        }
    }
}

impl EffectImpl for NeonGlowBorderImageEffect {
    fn info() -> Info {
        Info::new("neon_glow_border", "Neon glow border", EffectCategory::Filters, "app/zap")
            .desc("Adds a vibrant neon light tube border with a diffused outer glow on a dark background.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>("border_size", "Border size", 10, 200, 50, |e: &mut Self, v| e.border_size = v),
            color_param::<Self>("neon_color", "Neon color", SkColor::argb(0xFF00FFC8), |e: &mut Self, v| e.neon_color = v),
            float_slider::<Self>("glow_intensity", "Glow intensity", 10.0, 100.0, 75.0, |e: &mut Self, v| e.glow_intensity = v),
            float_slider::<Self>("tube_width", "Tube width", 1.0, 20.0, 4.0, |e: &mut Self, v| e.tube_width = v),
            color_param::<Self>("bg_color", "Background color", SkColor::argb(0xFF0A0A12), |e: &mut Self, v| e.bg_color = v),
            bool_param::<Self>("double_tube", "Double tube", false, |e: &mut Self, v| e.double_tube = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(10, 200);
        let glow = clampf(self.glow_intensity, 10.0, 100.0) / 100.0;
        let tube = clampf(self.tube_width, 1.0, 20.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.bg_color.rgba_array());
        let bf = border as f32;
        canvas.draw_bitmap(source, bf, bf, None);
        let off = bf * 0.5;
        let tube_rect = RRect::ltrb(off, off, nw as f32 - off, nh as f32 - off);
        let sigma = bf * 0.35 * glow;
        let nc = self.neon_color;
        let a = |v: f32| (v * glow) as u8;
        sk::draw_rect_blur(&mut canvas, tube_rect, &solid(nc.with_alpha(a(120.0)), true), Some(&Pen::new(tube * 5.0)), sigma);
        sk::draw_rect_blur(&mut canvas, tube_rect, &solid(nc.with_alpha(a(180.0)), true), Some(&Pen::new(tube * 3.0)), sigma * 0.35);
        sk::draw_rect(&mut canvas, tube_rect, &solid(SkColor::rgba(255, 255, 255, a(220.0)), true), Some(&Pen::new(tube)));
        sk::draw_rect(&mut canvas, tube_rect, &solid(nc.with_alpha(a(240.0)), true), Some(&Pen::new(tube * 1.4)));
        if self.double_tube {
            let io = bf * 0.78;
            let inner = RRect::ltrb(io, io, nw as f32 - io, nh as f32 - io);
            sk::draw_rect_blur(&mut canvas, inner, &solid(nc.with_alpha(a(100.0)), true), Some(&Pen::new(tube * 2.5)), sigma * 0.25);
            sk::draw_rect(&mut canvas, inner, &solid(nc.with_alpha(a(200.0)), true), Some(&Pen::new(tube * 0.8)));
        }
        drop(canvas);
        Ok(out)
    }
}
