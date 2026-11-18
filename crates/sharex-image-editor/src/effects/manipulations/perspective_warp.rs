//! `PerspectiveWarpImageEffect` (`Core/ImageEffects/Manipulations/PerspectiveWarpImageEffect.cs`).

use crate::effects::prelude::*;

/// `PerspectiveWarpImageEffect`.
#[derive(Clone, Debug)]
pub struct PerspectiveWarpImageEffect {
    pub top_left_x: f32,
    pub top_left_y: f32,
    pub top_right_x: f32,
    pub top_right_y: f32,
    pub bottom_right_x: f32,
    pub bottom_right_y: f32,
    pub bottom_left_x: f32,
    pub bottom_left_y: f32,
}

impl Default for PerspectiveWarpImageEffect {
    fn default() -> Self {
        PerspectiveWarpImageEffect {
            top_left_x: 0.0,
            top_left_y: 0.0,
            top_right_x: 0.0,
            top_right_y: 0.0,
            bottom_right_x: 0.0,
            bottom_right_y: 0.0,
            bottom_left_x: 0.0,
            bottom_left_y: 0.0,
        }
    }
}

impl EffectImpl for PerspectiveWarpImageEffect {
    fn info() -> Info {
        Info::new(
            "perspective_warp",
            "Perspective warp",
            EffectCategory::Manipulations,
            "image-effect/move-diagonal",
        )
        .desc("Applies a perspective warp transformation.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "top_left_x",
                "Top left X",
                -500,
                500,
                0,
                |e: &mut Self, v| e.top_left_x = v as f32,
            ),
            int_numeric::<Self>(
                "top_left_y",
                "Top left Y",
                -500,
                500,
                0,
                |e: &mut Self, v| e.top_left_y = v as f32,
            ),
            int_numeric::<Self>(
                "top_right_x",
                "Top right X",
                -500,
                500,
                0,
                |e: &mut Self, v| e.top_right_x = v as f32,
            ),
            int_numeric::<Self>(
                "top_right_y",
                "Top right Y",
                -500,
                500,
                0,
                |e: &mut Self, v| e.top_right_y = v as f32,
            ),
            int_numeric::<Self>(
                "bottom_right_x",
                "Bottom right X",
                -500,
                500,
                0,
                |e: &mut Self, v| e.bottom_right_x = v as f32,
            ),
            int_numeric::<Self>(
                "bottom_right_y",
                "Bottom right Y",
                -500,
                500,
                0,
                |e: &mut Self, v| e.bottom_right_y = v as f32,
            ),
            int_numeric::<Self>(
                "bottom_left_x",
                "Bottom left X",
                -500,
                500,
                0,
                |e: &mut Self, v| e.bottom_left_x = v as f32,
            ),
            int_numeric::<Self>(
                "bottom_left_y",
                "Bottom left Y",
                -500,
                500,
                0,
                |e: &mut Self, v| e.bottom_left_y = v as f32,
            ),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        if self.top_left_x == 0.0
            && self.top_left_y == 0.0
            && self.top_right_x == 0.0
            && self.top_right_y == 0.0
            && self.bottom_right_x == 0.0
            && self.bottom_right_y == 0.0
            && self.bottom_left_x == 0.0
            && self.bottom_left_y == 0.0
        {
            return Ok(source.clone());
        }
        let (width, height) = (source.width() as i32, source.height() as i32);
        let (right, bottom) = (width - 1, height - 1);
        let (r, b) = (right as f32, bottom as f32);
        let src_quad = [(0.0, 0.0), (r, 0.0), (r, b), (0.0, b)];
        let dst_quad = [
            (self.top_left_x, self.top_left_y),
            (r + self.top_right_x, self.top_right_y),
            (r + self.bottom_right_x, b + self.bottom_right_y),
            (self.bottom_left_x, b + self.bottom_left_y),
        ];
        let Some(inv) = compute_homography(&dst_quad, &src_quad) else {
            return Ok(source.clone());
        };
        let src = pixels(source);
        let mut dst = vec![SkColor::default(); src.len()];
        for y in 0..height {
            let row = y * width;
            for x in 0..width {
                let (xd, yd) = (x as f64, y as f64);
                let w = inv[6] * xd + inv[7] * yd + inv[8];
                let i = (row + x) as usize;
                if w.abs() < 1e-8 {
                    dst[i] = color::TRANSPARENT;
                    continue;
                }
                let sx = ((inv[0] * xd + inv[1] * yd + inv[2]) / w) as f32;
                let sy = ((inv[3] * xd + inv[4] * yd + inv[5]) / w) as f32;
                if sx < 0.0 || sy < 0.0 || sx > r || sy > b {
                    dst[i] = color::TRANSPARENT;
                    continue;
                }
                let px = clamp_int((sx as f64).round_ties_even() as i32, 0, right);
                let py = clamp_int((sy as f64).round_ties_even() as i32, 0, bottom);
                dst[i] = src[(py * width + px) as usize];
            }
        }
        Ok(from_pixels(width as u32, height as u32, &dst))
    }
}

// ---- port ----

fn compute_homography(src: &[(f32, f32); 4], dst: &[(f32, f32); 4]) -> Option<[f64; 9]> {
    let mut a = [[0f64; 8]; 8];
    let mut b = [0f64; 8];
    for i in 0..4 {
        let (x, y) = (src[i].0 as f64, src[i].1 as f64);
        let (u, v) = (dst[i].0 as f64, dst[i].1 as f64);
        let (r0, r1) = (i * 2, i * 2 + 1);
        a[r0] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y];
        b[r0] = u;
        a[r1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y];
        b[r1] = v;
    }
    let s = solve_linear_system(&a, &b)?;
    let mut h = [0f64; 9];
    h[..8].copy_from_slice(&s);
    h[8] = 1.0;
    Some(h)
}

// Gaussian elimination as ShareX writes it, by index.
#[allow(clippy::needless_range_loop)]
fn solve_linear_system(a: &[[f64; 8]; 8], b: &[f64; 8]) -> Option<[f64; 8]> {
    const N: usize = 8;
    let mut aug = [[0f64; N + 1]; N];
    for i in 0..N {
        aug[i][..N].copy_from_slice(&a[i]);
        aug[i][N] = b[i];
    }
    for col in 0..N {
        let mut pivot = col;
        let mut max = aug[pivot][col].abs();
        for row in col + 1..N {
            let v = aug[row][col].abs();
            if v > max {
                max = v;
                pivot = row;
            }
        }
        if max < 1e-12 {
            return None;
        }
        if pivot != col {
            for j in col..=N {
                let t = aug[col][j];
                aug[col][j] = aug[pivot][j];
                aug[pivot][j] = t;
            }
        }
        let div = aug[col][col];
        for j in col..=N {
            aug[col][j] /= div;
        }
        for row in 0..N {
            if row == col {
                continue;
            }
            let factor = aug[row][col];
            if factor.abs() < 1e-12 {
                continue;
            }
            for j in col..=N {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }
    let mut x = [0f64; N];
    for i in 0..N {
        x[i] = aug[i][N];
    }
    Some(x)
}
