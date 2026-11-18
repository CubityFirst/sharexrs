//! `CylinderWrapImageEffect` (`Core/ImageEffects/Manipulations/CylinderWrapImageEffect.cs`).

use crate::effects::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CylinderWrapOrientation {
    Vertical,
    Horizontal,
}

/// `CylinderWrapImageEffect`.
#[derive(Clone, Debug)]
pub struct CylinderWrapImageEffect {
    pub orientation: CylinderWrapOrientation,
    pub curvature: f32,
    pub edge_shading: f32,
}

impl Default for CylinderWrapImageEffect {
    fn default() -> Self {
        CylinderWrapImageEffect {
            orientation: CylinderWrapOrientation::Vertical,
            curvature: 65.0,
            edge_shading: 35.0,
        }
    }
}

impl EffectImpl for CylinderWrapImageEffect {
    fn info() -> Info {
        Info::new(
            "cylinder_wrap",
            "Cylinder wrap",
            EffectCategory::Manipulations,
            "image-effect/cylinder",
        )
        .desc("Wraps the image around a cylindrical surface.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            enum_param::<Self, CylinderWrapOrientation>(
                "orientation",
                "Orientation",
                CylinderWrapOrientation::Vertical,
                |e: &mut Self, v| e.orientation = v,
                &[
                    ("Vertical", CylinderWrapOrientation::Vertical),
                    ("Horizontal", CylinderWrapOrientation::Horizontal),
                ],
            ),
            float_slider::<Self>(
                "curvature",
                "Curvature",
                0.0,
                100.0,
                65.0,
                |e: &mut Self, v| e.curvature = v,
            ),
            float_slider::<Self>(
                "edge_shading",
                "Edge shading",
                0.0,
                100.0,
                35.0,
                |e: &mut Self, v| e.edge_shading = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let curvature01 = clampf(self.curvature, 0.0, 100.0) / 100.0;
        if curvature01 <= 0.0 {
            return Ok(source.clone());
        }
        let shading01 = clampf(self.edge_shading, 0.0, 100.0) / 100.0;
        let max_angle = 0.12 + (curvature01 * 1.28);
        let sin_max = max_angle.sin();
        let (width, height) = (source.width() as i32, source.height() as i32);
        let width_range = 1f32.max((width - 1) as f32);
        let height_range = 1f32.max((height - 1) as f32);
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (sx, sy, shade);
                if self.orientation == CylinderWrapOrientation::Vertical {
                    let projected = ((x as f32 / width_range) * 2.0) - 1.0;
                    let angle = clampf(projected * sin_max, -1.0, 1.0).asin();
                    let u = ((angle / max_angle) + 1.0) * 0.5;
                    sx = u * width_range;
                    sy = y as f32;
                    shade = 1.0 - (shading01 * (1.0 - angle.cos()) * 0.65);
                } else {
                    let projected = ((y as f32 / height_range) * 2.0) - 1.0;
                    let angle = clampf(projected * sin_max, -1.0, 1.0).asin();
                    let v = ((angle / max_angle) + 1.0) * 0.5;
                    sx = x as f32;
                    sy = v * height_range;
                    shade = 1.0 - (shading01 * (1.0 - angle.cos()) * 0.65);
                }
                let sampled = bilinear_sample(&src, width, height, sx, sy);
                dst[(row + x) as usize] = multiply_rgb(sampled, shade);
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}
