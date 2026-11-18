//! `WindStreaksImageEffect` (`Core/ImageEffects/Filters/WindStreaksImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindDirection {
    Left,
    Right,
}

/// `WindStreaksImageEffect`.
#[derive(Clone, Debug)]
pub struct WindStreaksImageEffect {
    pub length: i32,
    pub threshold: i32,
    pub opacity: f32,
    pub direction: WindDirection,
}

impl Default for WindStreaksImageEffect {
    fn default() -> Self {
        WindStreaksImageEffect {
            length: 30,
            threshold: 200,
            opacity: 40.0,
            direction: WindDirection::Right,
        }
    }
}

impl EffectImpl for WindStreaksImageEffect {
    fn info() -> Info {
        Info::new(
            "wind_streaks",
            "Wind streaks",
            EffectCategory::Filters,
            "image-effect/wind",
        )
        .desc("Adds directional wind streak trails to bright areas.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("length", "Streak length", 5, 100, 30, |e: &mut Self, v| {
                e.length = v
            }),
            int_slider::<Self>(
                "threshold",
                "Brightness threshold",
                100,
                254,
                200,
                |e: &mut Self, v| e.threshold = v,
            ),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 40.0, |e: &mut Self, v| {
                e.opacity = v
            }),
            enum_param::<Self, WindDirection>(
                "direction",
                "Direction",
                WindDirection::Right,
                |e: &mut Self, v| e.direction = v,
                &[
                    ("Left", WindDirection::Left),
                    ("Right", WindDirection::Right),
                ],
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (w, h) = (source.width() as i32, source.height() as i32);
        let len = 1.max(self.length);
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        let src = pixels(source);
        let mut dst = src.clone();
        let dir = if self.direction == WindDirection::Right {
            1
        } else {
            -1
        };
        let th = self.threshold as f32;
        for y in 0..h {
            for x in 0..w {
                let c = src[(y * w + x) as usize];
                let lum = 0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32;
                if lum < th {
                    continue;
                }
                let sa = alpha * ((lum - th) / (255.0 - th));
                for s in 1..=len {
                    let tx = x + s * dir;
                    if tx < 0 || tx >= w {
                        break;
                    }
                    let fade = 1.0 - s as f32 / len as f32;
                    let blend = sa * fade;
                    let e = dst[(y * w + tx) as usize];
                    let f =
                        |ev: u8, cv: u8| (ev as f32 + (cv as i32 - ev as i32) as f32 * blend) as u8;
                    dst[(y * w + tx) as usize] =
                        SkColor::rgba(f(e.r, c.r), f(e.g, c.g), f(e.b, c.b), e.a);
                }
            }
        }
        Ok(from_pixels(w as u32, h as u32, &dst))
    }
}
