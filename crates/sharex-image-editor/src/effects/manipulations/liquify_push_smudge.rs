//! `LiquifyPushSmudgeImageEffect` (`Core/ImageEffects/Manipulations/LiquifyPushSmudgeImageEffect.cs`).

use crate::effects::prelude::*;

/// `LiquifyPushSmudgeImageEffect`.
#[derive(Clone, Debug)]
pub struct LiquifyPushSmudgeImageEffect {
    pub angle: f32,
    pub distance: f32,
    pub radius_percentage: f32,
    pub smudge: f32,
    pub center_x_percentage: f32,
    pub center_y_percentage: f32,
}

impl Default for LiquifyPushSmudgeImageEffect {
    fn default() -> Self {
        LiquifyPushSmudgeImageEffect {
            angle: 0.0,
            distance: 40.0,
            radius_percentage: 25.0,
            smudge: 35.0,
            center_x_percentage: 50.0,
            center_y_percentage: 50.0,
        }
    }
}

impl EffectImpl for LiquifyPushSmudgeImageEffect {
    fn info() -> Info {
        Info::new(
            "liquify_push_smudge",
            "Liquify push / smudge",
            EffectCategory::Manipulations,
            "image-effect/droplet",
        )
        .desc("Pushes and smudges pixels in a direction.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("angle", "Angle", 0.0, 360.0, 0.0, |e: &mut Self, v| {
                e.angle = v
            }),
            float_slider::<Self>(
                "distance",
                "Distance",
                -200.0,
                200.0,
                40.0,
                |e: &mut Self, v| e.distance = v,
            ),
            float_slider::<Self>(
                "radius_percentage",
                "Radius percentage",
                1.0,
                100.0,
                25.0,
                |e: &mut Self, v| e.radius_percentage = v,
            ),
            float_slider::<Self>("smudge", "Smudge", 0.0, 100.0, 35.0, |e: &mut Self, v| {
                e.smudge = v
            }),
            float_slider::<Self>(
                "center_x_percentage",
                "Center X percentage",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_x_percentage = v,
            ),
            float_slider::<Self>(
                "center_y_percentage",
                "Center Y percentage",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.center_y_percentage = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let distance = clampf(self.distance, -200.0, 200.0);
        if distance.abs() <= 0.01 {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let radius =
            1f32.max(width.min(height) as f32 * clampf(self.radius_percentage, 1.0, 100.0) / 100.0);
        let smudge01 = clampf(self.smudge, 0.0, 100.0) / 100.0;
        let samples = 1 + (smudge01 * 5.0).round_ties_even() as i32;
        let angle = self.angle * (std::f32::consts::PI / 180.0);
        let (dir_x, dir_y) = (angle.cos(), angle.sin());
        let cx = percentage_to_x(width, self.center_x_percentage);
        let cy = percentage_to_y(height, self.center_y_percentage);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let i = (row + x) as usize;
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let pd = ((dx * dx) + (dy * dy)).sqrt();
                if pd >= radius {
                    dst[i] = src[i];
                    continue;
                }
                let normalized = pd / radius;
                let mut influence = 1.0 - smooth_step(0.0, 1.0, normalized);
                influence *= influence;
                let push = distance * influence;
                if samples <= 1 {
                    dst[i] = bilinear_sample(
                        &src,
                        width,
                        height,
                        x as f32 - (dir_x * push),
                        y as f32 - (dir_y * push),
                    );
                    continue;
                }
                let (mut sr, mut sg, mut sb, mut sa) = (0f32, 0f32, 0f32, 0f32);
                for k in 0..samples {
                    let t = if samples == 1 {
                        0.0
                    } else {
                        k as f32 / (samples - 1) as f32
                    };
                    let offset = push * (1.0 - (t * smudge01));
                    let s = bilinear_sample(
                        &src,
                        width,
                        height,
                        x as f32 - (dir_x * offset),
                        y as f32 - (dir_y * offset),
                    );
                    sr += s.r as f32;
                    sg += s.g as f32;
                    sb += s.b as f32;
                    sa += s.a as f32;
                }
                let inv = 1.0 / samples as f32;
                dst[i] = SkColor::rgba(
                    clamp_to_byte(sr * inv),
                    clamp_to_byte(sg * inv),
                    clamp_to_byte(sb * inv),
                    clamp_to_byte(sa * inv),
                );
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
