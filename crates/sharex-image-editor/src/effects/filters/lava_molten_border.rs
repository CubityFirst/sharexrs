//! `LavaMoltenBorderImageEffect` (`Core/ImageEffects/Filters/LavaMoltenBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `LavaMoltenBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct LavaMoltenBorderImageEffect {
    pub border_size: i32,
    pub rock_color: SkColor,
    pub lava_color: SkColor,
    pub crack_density: f32,
    pub glow_intensity: f32,
    pub ember_sparks: bool,
}

impl Default for LavaMoltenBorderImageEffect {
    fn default() -> Self {
        LavaMoltenBorderImageEffect {
            border_size: 56,
            rock_color: SkColor::argb(0xFF1E1916),
            lava_color: SkColor::argb(0xFFFF500A),
            crack_density: 60.0,
            glow_intensity: 70.0,
            ember_sparks: true,
        }
    }
}

impl EffectImpl for LavaMoltenBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "lava_molten_border",
            "Lava molten border",
            EffectCategory::Filters,
            "image-effect/flame",
        )
        .desc("Adds a volcanic border with dark cracked rock and glowing molten lava seams.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                12,
                300,
                56,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "rock_color",
                "Rock color",
                SkColor::argb(0xFF1E1916),
                |e: &mut Self, v| e.rock_color = v,
            ),
            color_param::<Self>(
                "lava_color",
                "Lava color",
                SkColor::argb(0xFFFF500A),
                |e: &mut Self, v| e.lava_color = v,
            ),
            float_slider::<Self>(
                "crack_density",
                "Crack density",
                10.0,
                100.0,
                60.0,
                |e: &mut Self, v| e.crack_density = v,
            ),
            float_slider::<Self>(
                "glow_intensity",
                "Glow intensity",
                10.0,
                100.0,
                70.0,
                |e: &mut Self, v| e.glow_intensity = v,
            ),
            bool_param::<Self>("ember_sparks", "Ember sparks", true, |e: &mut Self, v| {
                e.ember_sparks = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 8371;
        let border = self.border_size.clamp(12, 300);
        let density = clampf(self.crack_density, 10.0, 100.0) / 100.0;
        let glow = clampf(self.glow_intensity, 10.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut dst = vec![color::EMPTY; (nw * nh) as usize];
        let ns = 0.04 + density * 0.06;
        let (rc, lc) = (self.rock_color, self.lava_color);
        for y in 0..nh {
            let top = y < border;
            let bottom = y >= border + ih;
            for x in 0..nw {
                let left = x < border;
                let right = x >= border + iw;
                if !top && !bottom && !left && !right {
                    continue;
                }
                let n1 = fractal_noise(x as f32 * ns, y as f32 * ns, 4, 2.0, 0.5, SEED);
                let n2 = fractal_noise(
                    x as f32 * ns * 1.7 + 100.0,
                    y as f32 * ns * 1.7 + 200.0,
                    3,
                    2.0,
                    0.5,
                    SEED ^ 0x55,
                );
                let cw = 0.08 + density * 0.12;
                let c1 = 1.0 - smooth_step(0.0, cw, n1.abs());
                let c2 = 1.0 - smooth_step(0.0, cw, n2.abs());
                let crack = c1.max(c2);
                let lb = crack * glow;
                let rn = hash01(x, y, SEED ^ 0x77) * 0.15 - 0.075;
                let mut rr = (rc.r as f32 / 255.0 + rn) * (1.0 - lb) + (lc.r as f32 / 255.0) * lb;
                let mut rg = (rc.g as f32 / 255.0 + rn) * (1.0 - lb) + (lc.g as f32 / 255.0) * lb;
                let mut rb = (rc.b as f32 / 255.0 + rn) * (1.0 - lb) + (lc.b as f32 / 255.0) * lb;
                if crack > 0.7 {
                    let boost = (crack - 0.7) / 0.3 * glow;
                    rr += boost * 0.6;
                    rg += boost * 0.3;
                    rb += boost * 0.05;
                }
                dst[(y * nw + x) as usize] = SkColor::rgba(
                    clamp_to_byte(rr * 255.0),
                    clamp_to_byte(rg * 255.0),
                    clamp_to_byte(rb * 255.0),
                    255,
                );
            }
        }
        let mut out = from_pixels(nw as u32, nh as u32, &dst);
        let mut canvas = Canvas::new(&mut out);
        if self.ember_sparks {
            let count = (nw + nh) / 8;
            for i in 0..count {
                let sx = hash01(i, 0, SEED ^ 0xAB) * nw as f32;
                let sy = hash01(0, i, SEED ^ 0xCD) * nh as f32;
                let bf = border as f32;
                let in_border =
                    sx < bf || sx >= (border + iw) as f32 || sy < bf || sy >= (border + ih) as f32;
                if !in_border {
                    continue;
                }
                let local = fractal_noise(sx * ns, sy * ns, 4, 2.0, 0.5, SEED).abs();
                if local > 0.15 {
                    continue;
                }
                let br = hash01(i, i, SEED ^ 0xEF);
                let c = SkColor::rgba(
                    clamp_to_byte(255.0 * (0.8 + br * 0.2)),
                    clamp_to_byte(180.0 * br),
                    clamp_to_byte(40.0 * br),
                    clamp_to_byte(150.0 + br * 100.0),
                );
                sk::draw_circle_blur(
                    &mut canvas,
                    sx,
                    sy,
                    1.0 + br * 2.0,
                    &solid(c, true),
                    None,
                    2.0,
                );
            }
        }
        let glow_paint = solid(SkColor::rgba(lc.r, lc.g, lc.b, (60.0 * glow) as u8), true);
        let bf = border as f32;
        sk::draw_rect_blur(
            &mut canvas,
            RRect::xywh(bf, bf, iw as f32, ih as f32),
            &glow_paint,
            Some(&Pen::new(4.0)),
            6.0,
        );
        canvas.draw_bitmap(source, bf, bf, None);
        drop(canvas);
        Ok(out)
    }
}
