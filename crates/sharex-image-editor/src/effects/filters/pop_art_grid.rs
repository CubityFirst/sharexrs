//! `PopArtGridImageEffect` (`Core/ImageEffects/Filters/PopArtGridImageEffect.cs`).

use crate::effects::prelude::*;

/// `PopArtGridImageEffect`.
#[derive(Clone, Debug)]
pub struct PopArtGridImageEffect {
    pub color1: SkColor,
    pub color2: SkColor,
    pub color3: SkColor,
    pub color4: SkColor,
    pub strength: f32,
}

impl Default for PopArtGridImageEffect {
    fn default() -> Self {
        PopArtGridImageEffect {
            color1: SkColor::argb(0xFFFF3232),
            color2: SkColor::argb(0xFF32C832),
            color3: SkColor::argb(0xFF3232FF),
            color4: SkColor::argb(0xFFFFC832),
            strength: 60.0,
        }
    }
}

impl EffectImpl for PopArtGridImageEffect {
    fn info() -> Info {
        Info::new(
            "pop_art_grid",
            "Pop art grid",
            EffectCategory::Filters,
            "app/split",
        )
        .desc("Creates an Andy Warhol-style 2×2 pop art grid with color variations.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>(
                "color1",
                "Color 1",
                SkColor::argb(0xFFFF3232),
                |e: &mut Self, v| e.color1 = v,
            ),
            color_param::<Self>(
                "color2",
                "Color 2",
                SkColor::argb(0xFF32C832),
                |e: &mut Self, v| e.color2 = v,
            ),
            color_param::<Self>(
                "color3",
                "Color 3",
                SkColor::argb(0xFF3232FF),
                |e: &mut Self, v| e.color3 = v,
            ),
            color_param::<Self>(
                "color4",
                "Color 4",
                SkColor::argb(0xFFFFC832),
                |e: &mut Self, v| e.color4 = v,
            ),
            float_slider::<Self>(
                "strength",
                "Color strength",
                0.0,
                100.0,
                60.0,
                |e: &mut Self, v| e.strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (w, h) = (source.width() as i32, source.height() as i32);
        let (hw, hh) = (w / 2, h / 2);
        let s = clampf(self.strength / 100.0, 0.0, 1.0);
        let mut gray = Bitmap::new(source.width(), source.height());
        {
            let mut gc = Canvas::new(&mut gray);
            let mut gp = Paint::shader(Shader::Color([0, 0, 0, 255]));
            gp.color_filter = Some([
                0.33, 0.33, 0.33, 0.0, 0.0, //
                0.33, 0.33, 0.33, 0.0, 0.0, //
                0.33, 0.33, 0.33, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ]);
            gc.draw_bitmap(source, 0.0, 0.0, Some(&gp));
        }
        let mut out = Bitmap::new(source.width(), source.height());
        let mut canvas = Canvas::new(&mut out);
        let tints = [self.color1, self.color2, self.color3, self.color4];
        let (wf, hf, hwf, hhf) = (w as f32, h as f32, hw as f32, hh as f32);
        let cells = [
            RRect::ltrb(0.0, 0.0, hwf, hhf),
            RRect::ltrb(hwf, 0.0, wf, hhf),
            RRect::ltrb(0.0, hhf, hwf, hf),
            RRect::ltrb(hwf, hhf, wf, hf),
        ];
        for i in 0..4 {
            let cell = cells[i];
            let tint = tints[i];
            sk::with_clip_rect(&mut canvas, cell, |canvas| {
                canvas.draw_image_rect(
                    &gray,
                    Some(RRect::ltrb(0.0, 0.0, wf, hf)),
                    cell,
                    Sampling::Nearest,
                    None,
                );
                let mut tp = solid(tint.with_alpha((255.0 * s) as u8), false);
                tp.blend = Blend::Modulate;
                sk::draw_rect(canvas, cell, &tp, None);
            });
        }
        drop(canvas);
        Ok(out)
    }
}
