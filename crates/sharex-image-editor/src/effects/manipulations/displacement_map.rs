//! `DisplacementMapImageEffect` (`Core/ImageEffects/Manipulations/DisplacementMapImageEffect.cs`).

use crate::effects::prelude::*;

/// `DisplacementMapImageEffect`.
#[derive(Clone, Debug)]
pub struct DisplacementMapImageEffect {
    pub map_file_path: String,
    pub amount_x: f32,
    pub amount_y: f32,
}

impl Default for DisplacementMapImageEffect {
    fn default() -> Self {
        DisplacementMapImageEffect {
            map_file_path: "".to_string(),
            amount_x: 20.0,
            amount_y: 20.0,
        }
    }
}

impl EffectImpl for DisplacementMapImageEffect {
    fn info() -> Info {
        Info::new(
            "displacement_map",
            "Displacement map",
            EffectCategory::Manipulations,
            "image-effect/map",
        )
        .desc("Displaces pixels using a map image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("map_file_path", "Map file path", "", |e: &mut Self, v| {
                e.map_file_path = v
            }),
            float_slider::<Self>(
                "amount_x",
                "Amount X",
                -200.0,
                200.0,
                20.0,
                |e: &mut Self, v| e.amount_x = v,
            ),
            float_slider::<Self>(
                "amount_y",
                "Amount Y",
                -200.0,
                200.0,
                20.0,
                |e: &mut Self, v| e.amount_y = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        if self.amount_x.abs() < 0.0001 && self.amount_y.abs() < 0.0001 {
            return Ok(source.clone());
        }
        let src = pixels(source);
        let map_path = expand_environment_variables(&self.map_file_path);
        let map = if is_blank(&map_path) || !file_exists(&map_path) {
            None
        } else {
            decode_file(&map_path).filter(|m| !m.is_empty())
        };
        let map_pixels = map.as_ref().map(pixels);
        let (mp, mw, mh) = match (&map_pixels, &map) {
            (Some(p), Some(m)) => (p.as_slice(), m.width() as i32, m.height() as i32),
            _ => (src.as_slice(), width, height),
        };
        let scale = |c: i32, sl: i32, tl: i32| -> i32 {
            if tl <= 1 || sl <= 1 {
                return 0;
            }
            let scaled = c as f32 * (tl - 1) as f32 / (sl - 1) as f32;
            clamp_int(scaled.round_ties_even() as i32, 0, tl - 1)
        };
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let m = mp[(scale(y, height, mh) * mw + scale(x, width, mw)) as usize];
                let dx = ((m.r as f32 / 255.0) - 0.5) * 2.0 * self.amount_x;
                let dy = ((m.g as f32 / 255.0) - 0.5) * 2.0 * self.amount_y;
                let sx = clamp_int((x as f32 + dx).round_ties_even() as i32, 0, right);
                let sy = clamp_int((y as f32 + dy).round_ties_even() as i32, 0, bottom);
                dst[(row + x) as usize] = src[(sy * width + sx) as usize];
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
