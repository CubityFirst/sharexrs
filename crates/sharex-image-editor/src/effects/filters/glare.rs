//! `GlareImageEffect` (`Core/ImageEffects/Filters/GlareImageEffect.cs`).

use crate::effects::prelude::*;

/// `GlareImageEffect`.
#[derive(Clone, Debug)]
pub struct GlareImageEffect {
    pub position_x: f32,
    pub position_y: f32,
    pub radius: f32,
    pub intensity: f32,
    pub ray_count: i32,
    pub ray_length: f32,
    pub color: SkColor,
}

impl Default for GlareImageEffect {
    fn default() -> Self {
        GlareImageEffect {
            position_x: 50.0,
            position_y: 30.0,
            radius: 30.0,
            intensity: 80.0,
            ray_count: 6,
            ray_length: 50.0,
            color: SkColor::argb(0xFFFFFFFF),
        }
    }
}

impl EffectImpl for GlareImageEffect {
    fn info() -> Info {
        Info::new(
            "glare",
            "Glare",
            EffectCategory::Filters,
            "image-effect/sparkle",
        )
        .desc("Adds a bright glare/light flare at a specified position.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>(
                "position_x",
                "Position X (%)",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.position_x = v,
            ),
            float_slider::<Self>(
                "position_y",
                "Position Y (%)",
                0.0,
                100.0,
                30.0,
                |e: &mut Self, v| e.position_y = v,
            ),
            float_slider::<Self>("radius", "Radius", 5.0, 100.0, 30.0, |e: &mut Self, v| {
                e.radius = v
            }),
            float_slider::<Self>(
                "intensity",
                "Intensity",
                0.0,
                100.0,
                80.0,
                |e: &mut Self, v| e.intensity = v,
            ),
            int_slider::<Self>("ray_count", "Ray count", 0, 12, 6, |e: &mut Self, v| {
                e.ray_count = v
            }),
            float_slider::<Self>(
                "ray_length",
                "Ray length",
                0.0,
                100.0,
                50.0,
                |e: &mut Self, v| e.ray_length = v,
            ),
            color_param::<Self>(
                "color",
                "Color",
                SkColor::argb(0xFFFFFFFF),
                |e: &mut Self, v| e.color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let intensity = clampf(self.intensity, 0.0, 100.0) / 100.0;
        if intensity <= 0.0 {
            return Ok(source.clone());
        }
        let (w, h) = (source.width() as i32, source.height() as i32);
        let cx = w as f32 * clampf(self.position_x, 0.0, 100.0) / 100.0;
        let cy = h as f32 * clampf(self.position_y, 0.0, 100.0) / 100.0;
        let diagonal = ((w * w + h * h) as f32).sqrt();
        let radius_px = diagonal * clampf(self.radius, 5.0, 100.0) / 200.0;
        let ray_len = diagonal * clampf(self.ray_length, 0.0, 100.0) / 200.0;
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let ga = (255.0 * intensity) as u8;
        let c = self.color;
        let center = SkColor::rgba(c.r, c.g, c.b, ga);
        let edge = SkColor::rgba(c.r, c.g, c.b, 0);
        let mid = SkColor::rgba(c.r, c.g, c.b, (ga as f32 * 0.4) as u8);
        let mut glare = sk::radial_gradient(
            (cx, cy),
            radius_px,
            &[center, mid, edge],
            Some(&[0.0, 0.4, 1.0]),
            true,
        );
        glare.blend = Blend::Screen;
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, w as f32, h as f32, &glare, None);
        let rays = self.ray_count.clamp(0, 12);
        if rays > 0 && ray_len > 1.0 {
            let step = 360.0 / rays as f32;
            let ray_w = radius_px * 0.15;
            for i in 0..rays {
                let angle = i as f32 * step * std::f32::consts::PI / 180.0;
                let ex = cx + angle.cos() * ray_len;
                let ey = cy + angle.sin() * ray_len;
                let mut paint = sk::linear_gradient(
                    (cx, cy),
                    (ex, ey),
                    &[center, edge],
                    Some(&[0.0, 1.0]),
                    true,
                );
                paint.blend = Blend::Screen;
                let pen = Pen::new(ray_w).cap(Cap::Round);
                sk::draw_line(&mut canvas, cx, cy, ex, ey, &pen, &paint);
            }
        }
        drop(canvas);
        Ok(out)
    }
}
