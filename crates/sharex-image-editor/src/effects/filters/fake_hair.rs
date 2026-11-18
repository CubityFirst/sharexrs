//! `FakeHairImageEffect` (`Core/ImageEffects/Filters/FakeHairImageEffect.cs`).

use crate::effects::prelude::*;

/// `FakeHairImageEffect`.
#[derive(Clone, Debug)]
pub struct FakeHairImageEffect {
    pub count: i32,
    pub length: f32,
    pub thickness: f32,
    pub curliness: f32,
    pub hair_color: SkColor,
}

impl Default for FakeHairImageEffect {
    fn default() -> Self {
        FakeHairImageEffect {
            count: 1,
            length: 180.0,
            thickness: 1.5,
            curliness: 30.0,
            hair_color: SkColor::argb(0xDC140F0A),
        }
    }
}

impl EffectImpl for FakeHairImageEffect {
    fn info() -> Info {
        Info::new("fake_hair", "Fake Hair", EffectCategory::Drawings, "app/scissors")
            .desc("Scatters realistic hair strands over the image to annoy your friends.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>("count", "Hair Count", 1, 100, 1, |e: &mut Self, v| e.count = v),
            float_slider::<Self>("length", "Length", 20.0, 600.0, 180.0, |e: &mut Self, v| e.length = v),
            float_slider::<Self>("thickness", "Thickness", 0.5, 4.0, 1.0, |e: &mut Self, v| e.thickness = v).tick(0.1).snap(false).format("{}{0:0.0}"),
            float_slider::<Self>("curliness", "Curliness", 0.0, 100.0, 30.0, |e: &mut Self, v| e.curliness = v),
            color_param::<Self>("hair_color", "Hair Color", SkColor::argb(0xDC140F0A), |e: &mut Self, v| e.hair_color = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let count = self.count.clamp(0, 1000);
        if count == 0 {
            return Ok(source.clone());
        }
        let length = clampf(self.length, 5.0, 2000.0);
        let thickness = clampf(self.thickness, 0.2, 20.0);
        let curliness = clampf(self.curliness, 0.0, 100.0) / 100.0;
        let (width, height) = (source.width() as f32, source.height() as f32);
        let mut out = source.clone();
        let mut canvas = Canvas::new(&mut out);
        // Environment.TickCount: ShareX seeds from the clock, so no two runs match.
        let mut rng = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i32)
            .unwrap_or(1);
        if rng == 0 {
            rng = 1;
        }
        let hc = self.hair_color;
        for _ in 0..count {
            let start_x = next_float(&mut rng) * width;
            let start_y = next_float(&mut rng) * height;
            let base_angle = next_float(&mut rng) * std::f32::consts::PI * 2.0;
            let var = (next_float(&mut rng) - 0.5) * 0.3;
            let hr = to_byte(hc.r as f32 + hc.r as f32 * var);
            let hg = to_byte(hc.g as f32 + hc.g as f32 * var);
            let hb = to_byte(hc.b as f32 + hc.b as f32 * var);
            let ha = to_byte(180.0 + next_float(&mut rng) * 75.0);
            let hair_t = thickness * (0.6 + next_float(&mut rng) * 0.8);
            let hair_l = length * (0.5 + next_float(&mut rng) * 0.7);
            let segments = 8.max((hair_l / 6.0) as i32);
            let seg_len = hair_l / segments as f32;
            let max_turn = curliness * 0.35;
            let mut path = SkPath::new();
            let (mut px, mut py) = (start_x, start_y);
            path.move_to(px, py);
            let mut angle = base_angle;
            for _ in 1..=segments {
                angle += (next_float(&mut rng) - 0.5) * 2.0 * max_turn;
                px += angle.cos() * seg_len;
                py += angle.sin() * seg_len;
                path.line_to(px, py);
            }
            let pen = Pen::new(hair_t).cap(Cap::Round).join(Join::Round);
            sk::stroke_path(&mut canvas, &path, &pen, &solid(SkColor::rgba(hr, hg, hb, ha), true));
            if hair_t > 0.8 {
                let sheen = solid(
                    SkColor::rgba(to_byte(hr as f32 + 60.0), to_byte(hg as f32 + 50.0), to_byte(hb as f32 + 40.0), ha / 4),
                    true,
                );
                let pen = Pen::new(hair_t * 0.3).cap(Cap::Round).join(Join::Round);
                let saved = canvas.matrix;
                canvas.translate(0.3, -0.3);
                sk::stroke_path(&mut canvas, &path, &pen, &sheen);
                canvas.matrix = saved;
            }
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

fn next_float(state: &mut i32) -> f32 {
    *state ^= state.wrapping_shl(13);
    *state ^= *state >> 17;
    *state ^= state.wrapping_shl(5);
    (*state & 0x7FFF_FFFF) as f32 / i32::MAX as f32
}

fn to_byte(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v >= 255.0 {
        255
    } else {
        v.round_ties_even() as u8
    }
}
