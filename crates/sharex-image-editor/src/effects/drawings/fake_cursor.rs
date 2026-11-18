//! `FakeCursorImageEffect` (`Core/ImageEffects/Drawings/FakeCursorImageEffect.cs`).

use crate::effects::prelude::*;

/// `FakeCursorImageEffect`.
#[derive(Clone, Debug)]
pub struct FakeCursorImageEffect {
    pub count: i32,
    pub scale: f32,
    pub random_size: bool,
    pub random_size_min: f32,
    pub random_size_max: f32,
    pub random_angle: bool,
    pub random_angle_min: i32,
    pub random_angle_max: i32,
    pub fill_color: SkColor,
    pub border_color: SkColor,
}

impl Default for FakeCursorImageEffect {
    fn default() -> Self {
        FakeCursorImageEffect {
            count: 1,
            scale: 1.0,
            random_size: false,
            random_size_min: 0.5,
            random_size_max: 2.0,
            random_angle: false,
            random_angle_min: 0,
            random_angle_max: 360,
            fill_color: SkColor::argb(0xFFFFFFFF),
            border_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for FakeCursorImageEffect {
    fn info() -> Info {
        Info::new("fake_cursor", "Fake cursor", EffectCategory::Drawings, "image-editor/cursor")
            .desc("Draws fake mouse cursors at random positions on the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 1, 200, 1, |e: &mut Self, v| e.count = v),
            float_slider::<Self>("scale", "Scale", 0.5, 5.0, 1.0, |e: &mut Self, v| e.scale = v).tick(0.5),
            bool_param::<Self>("random_size", "Random size", false, |e: &mut Self, v| e.random_size = v),
            float_slider::<Self>("random_size_min", "Random size min", 0.5, 5.0, 0.5, |e: &mut Self, v| e.random_size_min = v).tick(0.5),
            float_slider::<Self>("random_size_max", "Random size max", 0.5, 5.0, 2.0, |e: &mut Self, v| e.random_size_max = v).tick(0.5),
            bool_param::<Self>("random_angle", "Random angle", false, |e: &mut Self, v| e.random_angle = v),
            int_numeric::<Self>("random_angle_min", "Random angle min", 0, 360, 0, |e: &mut Self, v| e.random_angle_min = v),
            int_numeric::<Self>("random_angle_max", "Random angle max", 0, 360, 360, |e: &mut Self, v| e.random_angle_max = v),
            color_param::<Self>("fill_color", "Fill color", SkColor::argb(0xFFFFFFFF), |e: &mut Self, v| e.fill_color = v),
            color_param::<Self>("border_color", "Border color", SkColor::argb(0xFF000000), |e: &mut Self, v| e.border_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const OUTLINE: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 20.4), (5.7, 15.6), (9.6, 23.4), (12.6, 22.2), (8.7, 14.4), (15.3, 14.4)];
        let count = self.count.clamp(1, 200);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        for _ in 0..count {
            let mut scale = if self.random_size {
                let n = random::shared(|r| r.next_single());
                self.random_size_min + n * (self.random_size_min.max(self.random_size_max) - self.random_size_min)
            } else {
                self.scale
            };
            scale = clampf(scale, 0.5, 5.0);
            let cw = 15.3 * scale;
            let ch = 23.4 * scale;
            let x = random::shared(|r| r.next_range(0, 1.max(sw - cw as i32))) as f32;
            let y = random::shared(|r| r.next_range(0, 1.max(sh - ch as i32))) as f32;
            let saved = canvas.matrix;
            canvas.translate(x, y);
            if self.random_angle {
                let min_a = self.random_angle_min.min(self.random_angle_max);
                let max_a = self.random_angle_min.max(self.random_angle_max);
                let angle = if min_a == max_a { min_a } else { random::shared(|r| r.next_range(min_a, max_a)) };
                let (px, py) = (cw / 2.0, ch / 2.0);
                canvas.translate(px, py);
                canvas.rotate(angle as f32);
                canvas.translate(-px, -py);
            }
            canvas.scale(scale, scale);
            let mut path = SkPath::new();
            path.move_to(OUTLINE[0].0, OUTLINE[0].1);
            for p in &OUTLINE[1..] {
                path.line_to(p.0, p.1);
            }
            path.close();
            let pen = Pen::new(1.5).join(Join::Round);
            sk::stroke_path(&mut canvas, &path, &pen, &solid(self.border_color, true));
            canvas.fill_sk_path(&path, &solid(self.fill_color, true));
            canvas.matrix = saved;
        }
        drop(canvas);
        Ok(out)
    }
}
