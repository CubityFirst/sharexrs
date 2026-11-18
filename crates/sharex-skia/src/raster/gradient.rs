//! `SKShader.CreateLinearGradient` (clamp), as `SkLinearGradient` + `SkGradientBaseShader`
//! run it: colours interpolated unpremultiplied, then premultiplied unless all opaque;
//! two evenly spaced stops use `evenly_spaced_2_stop_gradient`, others the `gradient`
//! stage's search.

use super::Matrix;

/// A linear gradient between two points.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub start: (f32, f32),
    pub end: (f32, f32),
    /// Straight RGBA colours.
    pub colors: Vec<[u8; 4]>,
    /// Stop positions in `[0, 1]` (`None`: evenly spaced).
    pub positions: Option<Vec<f32>>,
    /// The shader's local matrix.
    pub local: Matrix,
}

enum Fill {
    /// `f`, `b`: colour = t * f + b.
    TwoStop { f: [f32; 4], b: [f32; 4] },
    /// `ts` (search thresholds), per-interval `fs`/`bs`.
    Stops {
        ts: Vec<f32>,
        fs: Vec<[f32; 4]>,
        bs: Vec<[f32; 4]>,
        evenly: bool,
    },
    /// Degenerate: a solid colour (straight floats).
    Solid([f32; 4]),
}

pub(crate) struct GradientSampler {
    m: Matrix,
    fill: Fill,
    clamp_t: bool,
    premul: bool,
    /// `xy_to_radius` after the matrix (a radial gradient).
    radial: bool,
}

fn color4f(c: [u8; 4]) -> [f32; 4] {
    c.map(super::from_byte)
}

impl LinearGradient {
    pub(crate) fn sampler(&self, ctm: &Matrix) -> Option<GradientSampler> {
        let n = self.colors.len();
        if n == 0 {
            return None;
        }
        let colors: Vec<[f32; 4]> = self.colors.iter().map(|&c| color4f(c)).collect();
        let opaque = colors.iter().all(|c| c[3] == 1.0);
        let (dx, dy) = (self.end.0 - self.start.0, self.end.1 - self.start.1);
        let mag = (dx * dx + dy * dy).sqrt();
        if n == 1 || mag <= 1.0 / 32768.0 {
            let last = *colors.last().unwrap();
            return Some(GradientSampler {
                m: Matrix::IDENTITY,
                fill: Fill::Solid(last),
                clamp_t: true,
                premul: !opaque,
                radial: false,
            });
        }
        // pts_to_unit_matrix.
        let inv = 1.0 / mag;
        let (vx, vy) = (dx * inv, dy * inv);
        let (sin, cos) = (-vy, vx);
        let (px, py) = self.start;
        let omc = 1.0 - cos;
        let mut p2u = Matrix([
            cos,
            -sin,
            sin * py + omc * px,
            sin,
            cos,
            -sin * px + omc * py,
            0.0,
            0.0,
            1.0,
        ]);
        // postTranslate(-p0): translation added.
        p2u.0[2] += -px;
        p2u.0[5] += -py;
        // postScale(inv, inv): Concat(scale, p2u).
        let p2u = Matrix::concat(&Matrix::scale_translate(inv, inv, 0.0, 0.0), &p2u);
        let total_inv = Matrix::concat(ctm, &self.local).invert()?;
        let m = Matrix::concat(&p2u, &total_inv);

        Some(build_sampler(
            colors,
            self.positions.clone(),
            m,
            false,
            opaque,
        ))
    }
}

/// A radial gradient (`SKShader.CreateRadialGradient`, clamp): `SkRadialGradient`.
#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    pub center: (f32, f32),
    pub radius: f32,
    /// Straight RGBA colours.
    pub colors: Vec<[u8; 4]>,
    /// Stop positions in `[0, 1]` (`None`: evenly spaced).
    pub positions: Option<Vec<f32>>,
    /// The shader's local matrix.
    pub local: Matrix,
}

impl RadialGradient {
    pub(crate) fn sampler(&self, ctm: &Matrix) -> Option<GradientSampler> {
        let n = self.colors.len();
        if n == 0 || self.radius < 0.0 {
            return None;
        }
        let colors: Vec<[f32; 4]> = self.colors.iter().map(|&c| color4f(c)).collect();
        let opaque = colors.iter().all(|c| c[3] == 1.0);
        if n == 1 || self.radius.abs() <= 1.0 / 32768.0 {
            // A colour shader; MakeDegenerateGradient (clamp): the last colour.
            let last = *colors.last().unwrap();
            return Some(GradientSampler {
                m: Matrix::IDENTITY,
                fill: Fill::Solid(last),
                clamp_t: true,
                premul: !opaque,
                radial: false,
            });
        }
        // rad_to_unit_matrix: setTranslate(-center), postScale(1/radius).
        let inv = 1.0 / self.radius;
        let (cx, cy) = self.center;
        let r2u = Matrix([inv, 0.0, inv * -cx, 0.0, inv, inv * -cy, 0.0, 0.0, 1.0]);
        let total_inv = Matrix::concat(ctm, &self.local).invert()?;
        let m = Matrix::concat(&r2u, &total_inv);
        Some(build_sampler(
            colors,
            self.positions.clone(),
            m,
            true,
            opaque,
        ))
    }
}

/// `SkGradientBaseShader`: the colour stops (`ColorStopOptimizer`, implicit
/// end stops, uniform positions dropped) and the stages that interpolate them.
fn build_sampler(
    colors: Vec<[f32; 4]>,
    positions: Option<Vec<f32>>,
    m: Matrix,
    radial: bool,
    opaque: bool,
) -> GradientSampler {
    // ColorStopOptimizer (3 stops at 0,0,1 or 0,1,1) and the constructor's implicit stops.
    let mut cols = colors.clone();
    let mut pos = positions;
    if let Some(p) = pos.as_ref().filter(|_| cols.len() == 3) {
        {
            let near = |a: f32, b: f32| (a - b).abs() <= 1.0 / 4096.0;
            if near(p[0], 0.0) && near(p[1], 0.0) && near(p[2], 1.0) && cols[0] == cols[1] {
                cols.remove(0);
                pos = Some(p[1..].to_vec());
            } else if near(p[0], 0.0) && near(p[1], 1.0) && near(p[2], 1.0) && cols[1] == cols[2] {
                cols.pop();
                pos = Some(p[..2].to_vec());
            }
        }
    }
    let mut positions = None;
    if let Some(p) = &pos {
        let first_implicit = p[0] != 0.0;
        let last_implicit = *p.last().unwrap() != 1.0;
        let count = cols.len();
        let mut c2 = Vec::new();
        if first_implicit {
            c2.push(cols[0]);
        }
        c2.extend_from_slice(&cols);
        if last_implicit {
            c2.push(cols[count - 1]);
        }
        let mut out = vec![0.0f32];
        let mut prev = 0.0f32;
        let start = if first_implicit { 0 } else { 1 };
        let end = count + last_implicit as usize;
        let mut uniform = true;
        let step = p[start.min(count - 1)] - prev;
        for i in start..end {
            let curr = p
                .get(i)
                .filter(|_| i != count)
                .map_or(1.0, |v| v.clamp(prev, 1.0));
            uniform &= ((step) - (curr - prev)).abs() <= 1.0 / 4096.0;
            out.push(curr);
            prev = curr;
        }
        cols = c2;
        if !uniform {
            positions = Some(out);
        }
    }

    let fill = if cols.len() == 2 && positions.is_none() {
        let (l, r) = (cols[0], cols[1]);
        Fill::TwoStop {
            f: [0, 1, 2, 3].map(|i| r[i] - l[i]),
            b: l,
        }
    } else if let Some(pos) = positions {
        let count = cols.len();
        let first = if count > 2 && cols[0] == cols[1] {
            1
        } else {
            0
        };
        let last = if count > 2 {
            if cols[count - 2] != cols[count - 1] {
                count - 1
            } else {
                count - 2
            }
        } else {
            1
        };
        let mut ts = Vec::new();
        let mut fs = Vec::new();
        let mut bs = Vec::new();
        let mut t_l = pos[first];
        let mut c_l = cols[first];
        ts.push(0.0);
        fs.push([0.0; 4]);
        bs.push(c_l);
        for i in first..last {
            let t_r = pos[i + 1];
            let c_r = cols[i + 1];
            if t_l < t_r {
                let f = [0, 1, 2, 3].map(|k| (c_r[k] - c_l[k]) / (t_r - t_l));
                let b = [0, 1, 2, 3].map(|k| c_l[k] - f[k] * t_l);
                ts.push(t_l);
                fs.push(f);
                bs.push(b);
            }
            t_l = t_r;
            c_l = c_r;
        }
        ts.push(t_l);
        fs.push([0.0; 4]);
        bs.push(c_l);
        Fill::Stops {
            ts,
            fs,
            bs,
            evenly: false,
        }
    } else {
        let stop_count = cols.len();
        let gap = (stop_count - 1) as f32;
        let mut fs = Vec::new();
        let mut bs = Vec::new();
        for i in 0..stop_count - 1 {
            let (l, r) = (cols[i], cols[i + 1]);
            let f = [0, 1, 2, 3].map(|k| (r[k] - l[k]) * gap);
            let b = [0, 1, 2, 3].map(|k| l[k] - f[k] * (i as f32 / gap));
            fs.push(f);
            bs.push(b);
        }
        fs.push([0.0; 4]);
        bs.push(cols[stop_count - 1]);
        Fill::Stops {
            ts: Vec::new(),
            fs,
            bs,
            evenly: true,
        }
    };
    let clamp_t = !matches!(fill, Fill::Stops { evenly: false, .. });
    GradientSampler {
        m,
        fill,
        clamp_t,
        premul: !opaque,
        radial,
    }
}

impl GradientSampler {
    pub(crate) fn lowp_capable(&self) -> bool {
        true
    }

    fn t(&self, x: i32, y: i32) -> f32 {
        let mad_ = super::mad;
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        let m = &self.m;
        let t = if self.radial {
            // append_matrix's stage for the matrix's type, then xy_to_radius.
            let (x, y) = if m.is_identity() {
                (fx, fy)
            } else if m.is_translate() {
                (fx + m.tx(), fy + m.ty())
            } else if m.is_scale_translate() {
                (mad_(fx, m.sx(), m.tx()), mad_(fy, m.sy(), m.ty()))
            } else {
                (
                    mad_(fx, m.sx(), mad_(fy, m.kx(), m.tx())),
                    mad_(fx, m.ky(), mad_(fy, m.sy(), m.ty())),
                )
            };
            let (x2, y2) = (x * x, y * y);
            (x2 + y2).sqrt()
        } else if m.is_identity() {
            fx
        } else if m.is_translate() {
            fx + m.tx()
        } else if m.is_scale_translate() {
            mad_(fx, m.sx(), m.tx())
        } else {
            mad_(fx, m.sx(), mad_(fy, m.kx(), m.tx()))
        };
        if self.clamp_t { t.clamp(0.0, 1.0) } else { t }
    }

    fn color(&self, t: f32) -> [f32; 4] {
        match &self.fill {
            Fill::Solid(c) => *c,
            Fill::TwoStop { f, b } => [0, 1, 2, 3].map(|i| super::mad(t, f[i], b[i])),
            Fill::Stops { ts, fs, bs, evenly } => {
                let idx = if *evenly {
                    (t * (fs.len() - 1) as f32) as usize
                } else {
                    (1..ts.len()).filter(|&i| t >= ts[i]).count()
                };
                let idx = idx.min(fs.len() - 1);
                [0, 1, 2, 3].map(|i| super::mad(t, fs[idx][i], bs[idx][i]))
            }
        }
    }

    pub(crate) fn shade_highp(&self, x: i32, y: i32) -> [f32; 4] {
        let c = self.color(self.t(x, y));
        if self.premul { super::premul_f(c) } else { c }
    }

    pub(crate) fn shade_lowp(&self, x: i32, y: i32) -> [u32; 4] {
        let c = self.color(self.t(x, y));
        let round = |v: f32| (v * 255.0 + 0.5) as u32;
        let mut out = [
            round(c[0].clamp(0.0, 1.0)),
            round(c[1].clamp(0.0, 1.0)),
            round(c[2].clamp(0.0, 1.0)),
            round(c[3]),
        ];
        if self.premul {
            let a = out[3];
            for v in &mut out[..3] {
                *v = super::div255_accurate(*v * a);
            }
        }
        out
    }
}
