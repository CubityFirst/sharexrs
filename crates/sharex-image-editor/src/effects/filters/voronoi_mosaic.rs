//! `VoronoiMosaicImageEffect` (`Core/ImageEffects/Filters/VoronoiMosaicImageEffect.cs`).

use crate::effects::prelude::*;

/// `VoronoiMosaicImageEffect`.
#[derive(Clone, Debug)]
pub struct VoronoiMosaicImageEffect {
    pub cell_count: i32,
    pub draw_edges: bool,
    pub edge_thickness: f32,
    pub edge_color: SkColor,
}

impl Default for VoronoiMosaicImageEffect {
    fn default() -> Self {
        VoronoiMosaicImageEffect {
            cell_count: 300,
            draw_edges: true,
            edge_thickness: 1.0,
            edge_color: SkColor::argb(0xFF000000),
        }
    }
}

impl EffectImpl for VoronoiMosaicImageEffect {
    fn info() -> Info {
        Info::new(
            "voronoi_mosaic",
            "Voronoi Mosaic",
            EffectCategory::Filters,
            "image-effect/hexagon",
        )
        .desc("Divides the image into Voronoi cells colored by average region color.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_slider::<Self>(
                "cell_count",
                "Cell Count",
                10,
                2000,
                300,
                |e: &mut Self, v| e.cell_count = v,
            ),
            bool_param::<Self>("draw_edges", "Draw Edges", true, |e: &mut Self, v| {
                e.draw_edges = v
            }),
            float_slider::<Self>(
                "edge_thickness",
                "Edge Thickness",
                0.5,
                5.0,
                1.0,
                |e: &mut Self, v| e.edge_thickness = v,
            )
            .tick(0.5)
            .snap(false)
            .format("{}{0:0.0}"),
            color_param::<Self>(
                "edge_color",
                "Edge Color",
                SkColor::argb(0xFF000000),
                |e: &mut Self, v| e.edge_color = v,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let (width, height) = (source.width() as i32, source.height() as i32);
        let n = self.cell_count.clamp(2, 5000) as usize;
        let mut seeds = Vec::with_capacity(n);
        let mut state: i32 = 42;
        let next = |mut s: i32| {
            s ^= s.wrapping_shl(13);
            s ^= s >> 17;
            s ^= s.wrapping_shl(5);
            s
        };
        for _ in 0..n {
            state = next(state);
            let x = (state & 0x7FFF_FFFF) as f32 / i32::MAX as f32 * width as f32;
            state = next(state);
            let y = (state & 0x7FFF_FFFF) as f32 / i32::MAX as f32 * height as f32;
            seeds.push((x, y));
        }
        let px = pixels(source);
        let mut assign = vec![0usize; px.len()];
        let mut sums = vec![[0i64; 3]; n];
        let mut counts = vec![0i64; n];
        let cell = ((width * height) as f32 / n as f32).sqrt() * 2.0;
        let cols = 1.max((width as f32 / cell).ceil() as i32);
        let rows = 1.max((height as f32 / cell).ceil() as i32);
        let mut grid: Vec<Vec<usize>> = vec![Vec::new(); (cols * rows) as usize];
        for (i, &(sx, sy)) in seeds.iter().enumerate() {
            let gc = ((sx / cell) as i32).clamp(0, cols - 1);
            let gr = ((sy / cell) as i32).clamp(0, rows - 1);
            grid[(gr * cols + gc) as usize].push(i);
        }
        for y in 0..height {
            let gr = ((y as f32 / cell) as i32).clamp(0, rows - 1);
            for x in 0..width {
                let gc = ((x as f32 / cell) as i32).clamp(0, cols - 1);
                let mut best_d = f32::MAX;
                let mut best = 0usize;
                for r in 0.max(gr - 2)..=(rows - 1).min(gr + 2) {
                    for c in 0.max(gc - 2)..=(cols - 1).min(gc + 2) {
                        for &si in &grid[(r * cols + c) as usize] {
                            let dx = x as f32 - seeds[si].0;
                            let dy = y as f32 - seeds[si].1;
                            let d = dx * dx + dy * dy;
                            if d < best_d {
                                best_d = d;
                                best = si;
                            }
                        }
                    }
                }
                let idx = (y * width + x) as usize;
                assign[idx] = best;
                let p = px[idx];
                sums[best][0] += p.r as i64;
                sums[best][1] += p.g as i64;
                sums[best][2] += p.b as i64;
                counts[best] += 1;
            }
        }
        let colors: Vec<SkColor> = (0..n)
            .map(|i| {
                if counts[i] > 0 {
                    SkColor::rgba(
                        (sums[i][0] / counts[i]) as u8,
                        (sums[i][1] / counts[i]) as u8,
                        (sums[i][2] / counts[i]) as u8,
                        255,
                    )
                } else {
                    color::EMPTY
                }
            })
            .collect();
        let mut result: Vec<SkColor> = (0..px.len())
            .map(|i| colors[assign[i]].with_alpha(px[i].a))
            .collect();
        if self.draw_edges {
            let edge_dist = 1.max(self.edge_thickness.ceil() as i32);
            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    let mine = assign[idx];
                    let mut is_edge = false;
                    let mut d = 1;
                    while d <= edge_dist && !is_edge {
                        if x + d < width && assign[idx + d as usize] != mine {
                            is_edge = true;
                        }
                        if y + d < height && assign[idx + (d * width) as usize] != mine {
                            is_edge = true;
                        }
                        d += 1;
                    }
                    if is_edge {
                        result[idx] = self.edge_color;
                    }
                }
            }
        }
        Ok(from_pixels(width as u32, height as u32, &result))
    }
}
