//! `DrawParticlesEffect` (`Core/ImageEffects/Drawings/DrawParticlesEffect.cs`).

use crate::effects::prelude::*;

/// `DrawParticlesEffect`.
#[derive(Clone, Debug)]
pub struct DrawParticlesEffect {
    /// `Math.Clamp(value, 1, 1000)` when set.
    pub image_count: i32,
    pub image_folder: String,
    pub background: bool,
    pub random_size: bool,
    pub random_size_min: i32,
    pub random_size_max: i32,
    pub random_angle: bool,
    pub random_angle_min: i32,
    pub random_angle_max: i32,
    pub random_opacity: bool,
    pub random_opacity_min: i32,
    pub random_opacity_max: i32,
    pub no_overlap: bool,
    pub no_overlap_offset: i32,
    pub edge_overlap: bool,
}

impl Default for DrawParticlesEffect {
    fn default() -> Self {
        DrawParticlesEffect {
            image_count: 1,
            image_folder: "".to_string(),
            background: false,
            random_size: false,
            random_size_min: 64,
            random_size_max: 128,
            random_angle: false,
            random_angle_min: 0,
            random_angle_max: 360,
            random_opacity: false,
            random_opacity_min: 0,
            random_opacity_max: 100,
            no_overlap: false,
            no_overlap_offset: 0,
            edge_overlap: false,
        }
    }
}

impl EffectImpl for DrawParticlesEffect {
    fn info() -> Info {
        Info::new("draw_particles", "Particles", EffectCategory::Drawings, "app/sparkles")
            .desc("Draws particle images on the source image.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            file_param::<Self>("image_folder", "Image folder", "", |e: &mut Self, v| e.image_folder = v),
            int_numeric::<Self>("image_count", "Image count", 1, 1000, 1, |e: &mut Self, v| e.image_count = v.clamp(1, 1000)),
            bool_param::<Self>("background", "Background", false, |e: &mut Self, v| e.background = v),
            bool_param::<Self>("random_size", "Random size", false, |e: &mut Self, v| e.random_size = v),
            int_numeric::<Self>("random_size_min", "Random size min", 1, 10000, 64, |e: &mut Self, v| e.random_size_min = v),
            int_numeric::<Self>("random_size_max", "Random size max", 1, 10000, 128, |e: &mut Self, v| e.random_size_max = v),
            bool_param::<Self>("random_angle", "Random angle", false, |e: &mut Self, v| e.random_angle = v),
            int_numeric::<Self>("random_angle_min", "Random angle min", 0, 360, 0, |e: &mut Self, v| e.random_angle_min = v),
            int_numeric::<Self>("random_angle_max", "Random angle max", 0, 360, 360, |e: &mut Self, v| e.random_angle_max = v),
            bool_param::<Self>("random_opacity", "Random opacity", false, |e: &mut Self, v| e.random_opacity = v),
            int_numeric::<Self>("random_opacity_min", "Random opacity min", 0, 100, 0, |e: &mut Self, v| e.random_opacity_min = v),
            int_numeric::<Self>("random_opacity_max", "Random opacity max", 0, 100, 100, |e: &mut Self, v| e.random_opacity_max = v),
            bool_param::<Self>("no_overlap", "No overlap", false, |e: &mut Self, v| e.no_overlap = v),
            int_numeric::<Self>("no_overlap_offset", "No overlap offset", 0, 1000, 0, |e: &mut Self, v| e.no_overlap_offset = v),
            bool_param::<Self>("edge_overlap", "Edge overlap", false, |e: &mut Self, v| e.edge_overlap = v),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let folder = expand_environment_variables(self.image_folder.trim());
        if is_blank(&folder) || !std::path::Path::new(&folder).is_dir() {
            return Ok(source.clone());
        }
        // Directory.EnumerateFiles: the file system's order (FindFirstFile's on Windows).
        let files: Vec<String> = match std::fs::read_dir(&folder) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .and_then(|x| x.to_str())
                        .map(|x| ["png", "jpg", "jpeg"].iter().any(|e| e.eq_ignore_ascii_case(x)))
                        .unwrap_or(false)
                })
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            Err(_) => Vec::new(),
        };
        if files.is_empty() {
            return Ok(source.clone());
        }
        let mut out = if self.background { Bitmap::new(source.width(), source.height()) } else { source.clone() };
        let mut cache: std::collections::HashMap<String, Option<Bitmap>> = std::collections::HashMap::new();
        let mut rects: Vec<(i32, i32, i32, i32)> = Vec::new();
        {
            let mut canvas = Canvas::new(&mut out);
            for _ in 0..self.image_count {
                let file = files[random::shared(|r| r.next_range(0, files.len() as i32)) as usize].clone();
                let key = file.to_lowercase();
                let particle = cache.entry(key).or_insert_with(|| decode_file(&file));
                let Some(p) = particle.as_ref() else { continue };
                if p.width() == 0 || p.height() == 0 {
                    continue;
                }
                self.draw_particle(&mut canvas, source.width() as i32, source.height() as i32, p, &mut rects);
            }
            if self.background {
                canvas.draw_bitmap(source, 0.0, 0.0, None);
            }
        }
        Ok(out)
    }
}

// ---- port ----

fn next_int(min: i32, max: i32) -> i32 {
    if min == max {
        return min;
    }
    let (lo, hi) = if min > max { (max, min) } else { (min, max) };
    random::shared(|r| r.next_range(lo, hi))
}

impl DrawParticlesEffect {
    fn draw_particle(&self, canvas: &mut Canvas, tw: i32, th: i32, particle: &Bitmap, rects: &mut Vec<(i32, i32, i32, i32)>) {
        let (pw, ph) = (particle.width() as i32, particle.height() as i32);
        let (width, height) = if self.random_size {
            let size = next_int(self.random_size_min.min(self.random_size_max), self.random_size_min.max(self.random_size_max));
            let (mut w, mut h) = (size, size);
            if pw > ph {
                h = (size as f64 * (ph as f64 / pw as f64)).round_ties_even() as i32;
            } else if pw < ph {
                w = (size as f64 * (pw as f64 / ph as f64)).round_ties_even() as i32;
            }
            (w, h)
        } else {
            (pw, ph)
        };
        if width < 1 || height < 1 {
            return;
        }
        let min_x = if self.edge_overlap { -width + 1 } else { 0 };
        let min_y = if self.edge_overlap { -height + 1 } else { 0 };
        let max_x = tw - if self.edge_overlap { 0 } else { width } - 1;
        let max_y = th - if self.edge_overlap { 0 } else { height } - 1;
        let mut rect;
        let mut attempts = 0;
        loop {
            attempts += 1;
            if attempts > 1000 {
                return;
            }
            let x = next_int(min_x.min(max_x), min_x.max(max_x));
            let y = next_int(min_y.min(max_y), min_y.max(max_y));
            rect = (x, y, x + width, y + height);
            let o = self.no_overlap_offset;
            let inflated = (rect.0 - o, rect.1 - o, rect.2 + o, rect.3 + o);
            let hit = rects
                .iter()
                .any(|r| r.0 < inflated.2 && r.2 > inflated.0 && r.1 < inflated.3 && r.3 > inflated.1);
            if !(self.no_overlap && hit) {
                break;
            }
        }
        rects.push(rect);
        let mut alpha = 255;
        if self.random_opacity {
            let o = next_int(
                self.random_opacity_min.min(self.random_opacity_max),
                self.random_opacity_min.max(self.random_opacity_max),
            )
            .clamp(0, 100);
            alpha = (o as f32 / 100.0 * 255.0).round_ties_even() as i32;
        }
        let saved = canvas.matrix;
        if self.random_angle {
            let mx = rect.0 as f32 + (rect.2 - rect.0) as f32 / 2.0;
            let my = rect.1 as f32 + (rect.3 - rect.1) as f32 / 2.0;
            let rot = next_int(
                self.random_angle_min.min(self.random_angle_max),
                self.random_angle_min.max(self.random_angle_max),
            );
            canvas.translate(mx, my);
            canvas.rotate(rot as f32);
            canvas.translate(-mx, -my);
        }
        let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
        paint.anti_alias = true;
        if alpha < 255 {
            paint.modulate = Some([255, 255, 255, alpha as u8]);
        }
        canvas.draw_image_rect(
            particle,
            None,
            RRect::ltrb(rect.0 as f32, rect.1 as f32, rect.2 as f32, rect.3 as f32),
            Sampling::CatmullRom,
            Some(&paint),
        );
        canvas.matrix = saved;
    }
}
