//! `ConfettiImageEffect` (`Core/ImageEffects/Drawings/ConfettiImageEffect.cs`).

use crate::effects::prelude::*;

/// `ConfettiImageEffect`.
#[derive(Clone, Debug)]
pub struct ConfettiImageEffect {
    pub count: i32,
    pub min_size: i32,
    pub max_size: i32,
    pub opacity: f32,
    pub background: bool,
}

impl Default for ConfettiImageEffect {
    fn default() -> Self {
        ConfettiImageEffect {
            count: 150,
            min_size: 4,
            max_size: 12,
            opacity: 90.0,
            background: false,
        }
    }
}

impl EffectImpl for ConfettiImageEffect {
    fn info() -> Info {
        Info::new("confetti", "Confetti", EffectCategory::Drawings, "image-effect/party-popper")
            .desc("Scatters colorful confetti pieces over the image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Count", 10, 1000, 150, |e: &mut Self, v| e.count = v),
            int_slider::<Self>("min_size", "Min size", 2, 40, 4, |e: &mut Self, v| e.min_size = v),
            int_slider::<Self>("max_size", "Max size", 4, 80, 12, |e: &mut Self, v| e.max_size = v),
            float_slider::<Self>("opacity", "Opacity", 0.0, 100.0, 90.0, |e: &mut Self, v| e.opacity = v),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const COLORS: [(u8, u8, u8); 10] = [
            (255, 56, 96),
            (255, 165, 0),
            (255, 215, 0),
            (0, 200, 83),
            (0, 176, 255),
            (156, 39, 176),
            (255, 64, 129),
            (0, 230, 118),
            (255, 234, 0),
            (29, 233, 182),
        ];
        if self.count <= 0 {
            return Ok(source.clone());
        }
        let alpha = clampf(self.opacity / 100.0, 0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(source.clone());
        }
        let min_size = 2.max(self.min_size.min(self.max_size));
        let max_size = min_size.max(self.max_size);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        if self.background {
            canvas.clear([255, 255, 255, 0]);
        }
        let (sw, sh) = (source.width() as i32, source.height() as i32);
        for _ in 0..self.count {
            let (x, y, w, h, angle, ci, shape) = random::shared(|r| {
                let x = r.next_range(0, sw) as f32;
                let y = r.next_range(0, sh) as f32;
                let w = r.next_range(min_size, max_size + 1) as f32;
                let h = r.next_range(min_size, max_size + 1) as f32;
                let angle = r.next_range(0, 360) as f32;
                let ci = r.next_max(COLORS.len() as i32) as usize;
                let shape = r.next_max(3);
                (x, y, w, h, angle, ci, shape)
            });
            let (cr, cg, cb) = COLORS[ci];
            let paint = solid(SkColor::rgba(cr, cg, cb, (255.0 * alpha) as u8), true);
            let saved = canvas.matrix;
            canvas.translate(x, y);
            canvas.rotate(angle);
            match shape {
                0 => sk::draw_rect_xywh(&mut canvas, -w / 2.0, -h / 2.0, w, h, &paint, None),
                1 => sk::draw_oval(&mut canvas, RRect::ltrb(0.0 - w / 2.0, 0.0 - h / 2.0, 0.0 + w / 2.0, 0.0 + h / 2.0), &paint, None),
                _ => {
                    let mut path = SkPath::new();
                    path.move_to(0.0, -h / 2.0);
                    path.line_to(w / 2.0, h / 2.0);
                    path.line_to(-w / 2.0, h / 2.0);
                    path.close();
                    canvas.fill_sk_path(&path, &paint);
                }
            }
            canvas.matrix = saved;
        }
        if self.background {
            canvas.draw_bitmap(source, 0.0, 0.0, None);
        }
        drop(canvas);
        Ok(out)
    }
}
