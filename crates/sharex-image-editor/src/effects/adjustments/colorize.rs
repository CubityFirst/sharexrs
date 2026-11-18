//! `ColorizeImageEffect` (`Core/ImageEffects/Adjustments/ColorizeImageEffect.cs`).

use crate::effects::prelude::*;

/// `ColorizeImageEffect`.
#[derive(Clone, Debug)]
pub struct ColorizeImageEffect {
    pub color: SkColor,
    pub strength: f32,
}

impl Default for ColorizeImageEffect {
    fn default() -> Self {
        ColorizeImageEffect {
            color: SkColor::argb(0xFFFF0000),
            strength: 50.0,
        }
    }
}

impl EffectImpl for ColorizeImageEffect {
    fn info() -> Info {
        Info::new(
            "colorize",
            "Colorize",
            EffectCategory::Adjustments,
            "app/palette",
        )
        .desc("Colorizes the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFFFFA500),
                |e: &mut Self, v| e.color = v,
            ),
            float_slider::<Self>(
                "strength",
                "Strength",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.strength = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        use sharex_skia::raster as r;
        let mut strength = self.strength;
        if strength <= 0.0 {
            return Ok(source.clone());
        }
        if strength > 100.0 {
            strength = 100.0;
        }
        const GRAY: [f32; 20] = [
            0.2126, 0.7152, 0.0722, 0.0, 0.0, //
            0.2126, 0.7152, 0.0722, 0.0, 0.0, //
            0.2126, 0.7152, 0.0722, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        // SkColorFilters::Blend(color, Modulate): the colour premultiplied in float.
        let tint = r::premul_f(r::load(self.color.rgba_array()));
        // A sprite through SkRasterPipelineSpriteBlitter (highp): load_8888, the
        // paint's alpha, the grayscale matrix then the modulate filter, src-over.
        let alpha = (strength < 100.0).then(|| r::from_byte((255.0 * (strength / 100.0)) as u8));
        let n = (source.width() * source.height()) as usize;
        let mut out = if strength >= 100.0 {
            Bitmap::new(source.width(), source.height())
        } else {
            source.clone()
        };
        for i in 0..n {
            let mut c = r::load(source.px(i));
            if let Some(a) = alpha {
                c = c.map(|v| v * a);
            }
            c = r::color_filter(&GRAY, c);
            c = [0, 1, 2, 3].map(|k| tint[k] * c[k]);
            let c = r::clamp_01(c);
            let d = out.px(i);
            let inv = 1.0 - c[3];
            out.set_px(
                i,
                [0, 1, 2, 3]
                    .map(|k| r::to_unorm(r::mad(d[k] as f32, inv, c[k] * 255.0), 1.0, 255.0)),
            );
        }
        Ok(out)
    }
}
