//! `RemoveBackgroundImageEffect` (`Core/ImageEffects/Filters/RemoveBackgroundImageEffect.cs`).

use crate::effects::prelude::*;

/// `RemoveBackgroundImageEffect`.
#[derive(Clone, Debug)]
pub struct RemoveBackgroundImageEffect {
    pub sensitivity: f32,
    pub center_protection: f32,
    pub edge_feather: f32,
}

impl Default for RemoveBackgroundImageEffect {
    fn default() -> Self {
        RemoveBackgroundImageEffect {
            sensitivity: 60.0,
            center_protection: 65.0,
            edge_feather: 4.0,
        }
    }
}

impl EffectImpl for RemoveBackgroundImageEffect {
    fn info() -> Info {
        Info::new("remove_background", "Remove background", EffectCategory::Manipulations, "app/scissors")
            .desc("Removes border-connected background colors and turns them transparent.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            float_slider::<Self>("sensitivity", "Sensitivity (%)", 0.0, 100.0, 60.0, |e: &mut Self, v| e.sensitivity = v),
            float_slider::<Self>("center_protection", "Center protection (%)", 0.0, 100.0, 65.0, |e: &mut Self, v| e.center_protection = v),
            float_slider::<Self>("edge_feather", "Edge feather (px)", 0.0, 24.0, 4.0, |e: &mut Self, v| e.edge_feather = v).tick(0.5).snap(false).format("{}{0:0.#}"),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const THRESHOLD: u8 = 20;
        let (width, height) = (source.width() as i32, source.height() as i32);
        if width <= 0 || height <= 0 {
            return Ok(source.clone());
        }
        let sensitivity = clampf(self.sensitivity, 0.0, 100.0) / 100.0;
        let center_protection = clampf(self.center_protection, 0.0, 100.0) / 100.0;
        let feather = clampf(self.edge_feather, 0.0, 24.0);
        let src = pixels(source);
        if !src.iter().any(|p| p.a > THRESHOLD) {
            return Ok(source.clone());
        }
        let scale = 1f32.min(256.0 / width.max(height) as f32);
        let pw = 1.max((width as f32 * scale).round_ties_even() as i32);
        let ph = 1.max((height as f32 * scale).round_ties_even() as i32);
        let working = if scale < 0.999 { resize(source, pw, ph) } else { source.clone() };
        let wp = pixels(&working);
        let n = (pw * ph) as usize;
        let mut features = Vec::with_capacity(n);
        let mut luma = vec![0f32; n];
        let mut alpha = vec![0u8; n];
        let mut border = Vec::new();
        let mut center = Vec::new();
        let band = ((pw.min(ph) as f32 * 0.08).round_ties_even() as i32).clamp(1, 24);
        let cix = ((pw as f32 * 0.22).round_ties_even() as i32).clamp(band, band.max(pw / 2));
        let ciy = ((ph as f32 * 0.18).round_ties_even() as i32).clamp(band, band.max(ph / 2));
        let (cl, ct) = (cix, ciy);
        let cr = cl.max(pw - cix - 1);
        let cb = ct.max(ph - ciy - 1);
        for y in 0..ph {
            let border_row = y < band || y >= ph - band;
            let center_row = y >= ct && y <= cb;
            for x in 0..pw {
                let i = (y * pw + x) as usize;
                let c = wp[i];
                alpha[i] = c.a;
                let f = Feature::from_color(c);
                features.push(f);
                luma[i] = f.luma;
                if c.a <= THRESHOLD {
                    continue;
                }
                if border_row || x < band || x >= pw - band {
                    border.push(f);
                }
                if center_row && x >= cl && x <= cr {
                    center.push(f);
                }
            }
        }
        if border.len() < 12 {
            return Ok(source.clone());
        }
        let bg_protos = build_prototypes(&border, 4);
        let fg_samples = select_foreground(&center, &bg_protos);
        let fg_protos = build_prototypes(&fg_samples, 3);
        let bg_scale = affinity_scale(&border, &bg_protos, 0.045);
        let fg_scale = if !fg_protos.is_empty() { affinity_scale(&fg_samples, &fg_protos, 0.055) } else { 0.08 };
        let edges = edge_magnitude(&luma, pw, ph);
        let mut likelihood = vec![0f32; n];
        let mut penalty = vec![0f32; n];
        for y in 0..ph {
            for x in 0..pw {
                let i = (y * pw + x) as usize;
                if alpha[i] <= THRESHOLD {
                    likelihood[i] = 1.0;
                    penalty[i] = 0.0;
                    continue;
                }
                let f = features[i];
                let bd = nearest_distance(&f, &bg_protos);
                let fd = if !fg_protos.is_empty() { nearest_distance(&f, &fg_protos) } else { bd + 0.16 };
                let ba = (-bd / bg_scale).exp();
                let fa = if !fg_protos.is_empty() { (-fd / fg_scale).exp() } else { 0.16 };
                let l = ba / (ba + fa + 0.0001);
                likelihood[i] = l;
                let cw = center_weight(x, y, pw, ph);
                let lift = 1.0 - (alpha[i] as f32 / 255.0);
                let p = ((1.0 - l) * 0.72) + (edges[i] * 0.18) + (cw * center_protection * 0.22) - (lift * 0.12);
                penalty[i] = 0f32.max(p);
            }
        }
        let bg_mask = flood(&alpha, &likelihood, &edges, &penalty, pw, ph, sensitivity);
        let small: Vec<SkColor> = bg_mask
            .iter()
            .map(|&b| {
                let a = if b { 0 } else { 255 };
                SkColor::rgba(a, a, a, a)
            })
            .collect();
        let small_bmp = from_pixels(pw as u32, ph as u32, &small);
        let scaled = if pw == width && ph == height { small_bmp } else { resize(&small_bmp, width, height) };
        let final_mask = if feather > 0.05 { transparent_blur(&scaled, feather) } else { scaled };
        let mp = pixels(&final_mask);
        let out: Vec<SkColor> = src
            .iter()
            .zip(&mp)
            .map(|(s, m)| SkColor::rgba(s.r, s.g, s.b, clamp_to_byte(s.a as f32 * (m.a as f32 / 255.0))))
            .collect();
        Ok(from_pixels(width as u32, height as u32, &out))
    }
}

// ---- port ----

#[derive(Clone, Copy, Debug)]
struct Feature {
    luma: f32,
    blue: f32,
    red: f32,
    sat: f32,
}

impl Feature {
    fn from_color(c: SkColor) -> Feature {
        let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
        let luma = (0.2126 * r) + (0.7152 * g) + (0.0722 * b);
        Feature {
            luma,
            blue: (b - luma) * 1.15,
            red: (r - luma) * 1.15,
            sat: r.max(g.max(b)) - r.min(g.min(b)),
        }
    }

    fn distance_squared(&self, o: &Feature) -> f32 {
        let l = self.luma - o.luma;
        let b = self.blue - o.blue;
        let r = self.red - o.red;
        let s = self.sat - o.sat;
        (l * l * 0.52) + (b * b * 0.94) + (r * r * 0.94) + (s * s * 0.35)
    }
}

fn nearest_index(f: &Feature, protos: &[Feature]) -> usize {
    let mut best = 0usize;
    let mut best_d = f32::MAX;
    for (i, p) in protos.iter().enumerate() {
        let d = f.distance_squared(p);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

fn nearest_distance(f: &Feature, protos: &[Feature]) -> f32 {
    if protos.is_empty() {
        return 1.0;
    }
    let mut best = f32::MAX;
    for p in protos {
        let d = f.distance_squared(p);
        if d < best {
            best = d;
        }
    }
    best.sqrt()
}

fn select_by_cutoff(c: &[Feature], d: &[f32], cutoff: f32) -> Vec<Feature> {
    c.iter().zip(d).filter(|(_, d)| **d >= cutoff).map(|(f, _)| *f).collect()
}

fn select_foreground(candidates: &[Feature], bg: &[Feature]) -> Vec<Feature> {
    if candidates.is_empty() || bg.is_empty() {
        return candidates.to_vec();
    }
    let mut distances = Vec::with_capacity(candidates.len());
    let (mut sum, mut max) = (0f32, 0f32);
    for c in candidates {
        let d = nearest_distance(c, bg);
        distances.push(d);
        sum += d;
        if d > max {
            max = d;
        }
    }
    let average = sum / 1.max(candidates.len()) as f32;
    let cutoff = average + ((max - average) * 0.18);
    let selected = select_by_cutoff(candidates, &distances, cutoff);
    if selected.len() >= 16.max(candidates.len() / 8) {
        return selected;
    }
    let selected = select_by_cutoff(candidates, &distances, average);
    if selected.len() >= 8 { selected } else { candidates.to_vec() }
}

fn build_prototypes(samples: &[Feature], max_count: usize) -> Vec<Feature> {
    if samples.is_empty() || max_count == 0 {
        return Vec::new();
    }
    let count = max_count.min(samples.len());
    let mut protos = vec![samples[0]; count];
    for i in 1..count {
        let (mut far_i, mut far_d) = (0usize, -1f32);
        for (si, s) in samples.iter().enumerate() {
            let d = nearest_distance(s, &protos[..i]);
            if d > far_d {
                far_d = d;
                far_i = si;
            }
        }
        protos[i] = samples[far_i];
    }
    for _ in 0..5 {
        let mut sl = vec![0f32; count];
        let mut sb = vec![0f32; count];
        let mut sr = vec![0f32; count];
        let mut ss = vec![0f32; count];
        let mut counts = vec![0i32; count];
        for s in samples {
            let k = nearest_index(s, &protos);
            sl[k] += s.luma;
            sb[k] += s.blue;
            sr[k] += s.red;
            ss[k] += s.sat;
            counts[k] += 1;
        }
        for k in 0..count {
            if counts[k] == 0 {
                protos[k] = samples[(k * samples.len()) / count];
                continue;
            }
            let inv = 1.0 / counts[k] as f32;
            protos[k] = Feature {
                luma: sl[k] * inv,
                blue: sb[k] * inv,
                red: sr[k] * inv,
                sat: ss[k] * inv,
            };
        }
    }
    protos
}

fn affinity_scale(samples: &[Feature], protos: &[Feature], fallback: f32) -> f32 {
    if samples.is_empty() || protos.is_empty() {
        return fallback;
    }
    let mut sum = 0f32;
    for s in samples {
        sum += nearest_distance(s, protos);
    }
    let average = sum / samples.len() as f32;
    fallback.max((average * 2.8) + (fallback * 0.4))
}

fn edge_magnitude(luma: &[f32], width: i32, height: i32) -> Vec<f32> {
    let mut edges = vec![0f32; luma.len()];
    let (right, bottom) = (width - 1, height - 1);
    let at = |x: i32, y: i32| luma[(y * width + x) as usize];
    for y in 0..height {
        let y0 = if y > 0 { y - 1 } else { 0 };
        let y2 = if y < bottom { y + 1 } else { bottom };
        for x in 0..width {
            let x0 = if x > 0 { x - 1 } else { 0 };
            let x2 = if x < right { x + 1 } else { right };
            let (tl, tc, tr) = (at(x0, y0), at(x, y0), at(x2, y0));
            let (ml, mr) = (at(x0, y), at(x2, y));
            let (bl, bc, br) = (at(x0, y2), at(x, y2), at(x2, y2));
            let gx = (-tl + tr) + (-2.0 * ml + (2.0 * mr)) + (-bl + br);
            let gy = (-tl - (2.0 * tc) - tr) + (bl + (2.0 * bc) + br);
            let m = ((gx * gx) + (gy * gy)).sqrt();
            edges[(y * width + x) as usize] = clampf(m / 1.35, 0.0, 1.0);
        }
    }
    edges
}

#[allow(clippy::too_many_arguments)]
fn flood(alpha: &[u8], likelihood: &[f32], edges: &[f32], penalty: &[f32], width: i32, height: i32, sensitivity: f32) -> Vec<bool> {
    const THRESHOLD: u8 = 20;
    let mut visited = vec![false; penalty.len()];
    let mut queue = std::collections::VecDeque::new();
    let base = 0.19 + (sensitivity * 0.28);
    let seed = base + 0.04;
    let boost = 0.04 + (sensitivity * 0.06);
    let mut try_seed = |i: usize, visited: &mut Vec<bool>, queue: &mut std::collections::VecDeque<usize>| {
        if visited[i] {
            return;
        }
        if alpha[i] <= THRESHOLD || penalty[i] <= seed || likelihood[i] >= 0.82 {
            visited[i] = true;
            queue.push_back(i);
        }
    };
    for x in 0..width {
        try_seed(x as usize, &mut visited, &mut queue);
        try_seed(((height - 1) * width + x) as usize, &mut visited, &mut queue);
    }
    for y in 1..height - 1 {
        try_seed((y * width) as usize, &mut visited, &mut queue);
        try_seed((y * width + (width - 1)) as usize, &mut visited, &mut queue);
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i as i32 % width, i as i32 / width);
        for oy in -1..=1 {
            let ny = y + oy;
            if ny < 0 || ny >= height {
                continue;
            }
            for ox in -1..=1 {
                if ox == 0 && oy == 0 {
                    continue;
                }
                let nx = x + ox;
                if nx < 0 || nx >= width {
                    continue;
                }
                let ni = (ny * width + nx) as usize;
                if visited[ni] {
                    continue;
                }
                if alpha[ni] <= THRESHOLD {
                    visited[ni] = true;
                    queue.push_back(ni);
                    continue;
                }
                let local = base + ((1.0 - edges[ni]) * 0.05) + (likelihood[ni] * boost);
                if penalty[ni] <= local {
                    visited[ni] = true;
                    queue.push_back(ni);
                }
            }
        }
    }
    visited
}

fn center_weight(x: i32, y: i32, width: i32, height: i32) -> f32 {
    let cx = (width - 1) as f32 * 0.5;
    let cy = (height - 1) as f32 * 0.5;
    let dx = (x as f32 - cx) / 1f32.max(width as f32 * 0.5);
    let dy = (y as f32 - cy) / 1f32.max(height as f32 * 0.5);
    let d = ((dx * dx) + (dy * dy)).sqrt();
    1.0 - smooth_step(0.22, 1.0, d)
}

/// `ResizeBitmap`: an anti-aliased Catmull-Rom draw onto a cleared bitmap.
fn resize(source: &Bitmap, width: i32, height: i32) -> Bitmap {
    if source.width() as i32 == width && source.height() as i32 == height {
        return source.clone();
    }
    let mut out = Bitmap::new(width as u32, height as u32);
    let mut paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
    paint.anti_alias = true;
    Canvas::new(&mut out).draw_image_rect(source, None, RRect::ltrb(0.0, 0.0, width as f32, height as f32), Sampling::CatmullRom, Some(&paint));
    out
}

/// `ApplyTransparentBlur`.
fn transparent_blur(source: &Bitmap, radius: f32) -> Bitmap {
    if radius <= 0.01 {
        return source.clone();
    }
    let padding = 2.max((radius * 2.0).ceil() as i32);
    blur_transparent_padded(source, padding, 0.001f32.max(radius / 3.0))
}
