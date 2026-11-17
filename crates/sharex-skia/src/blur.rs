//! Skia m119's CPU Gaussian blur (`SkBlurImageFilter.cpp`'s `cpu_blur`):
//! three box passes in one sweep (`GaussPass`), or two (`TentPass`) for
//! very large sigmas, on premultiplied N32 pixels, transparent beyond the
//! image (decal).

use crate::pixmap::Pixmap;

/// `calculate_window`.
pub fn calculate_window(sigma: f64) -> i32 {
    let possible = (sigma * 3.0 * (2.0 * std::f64::consts::PI).sqrt() / 4.0 + 0.5).floor() as i32;
    possible.max(1)
}

/// `skvx::ScaledDividerU32`.
struct Divider {
    factor: u32,
    half: u32,
}

impl Divider {
    fn new(divisor: u32) -> Self {
        Self {
            factor: ((1.0 / divisor as f64) * (1u64 << 32) as f64).round() as u32,
            half: (divisor + 1) >> 1,
        }
    }

    fn divide(&self, n: u32) -> u32 {
        ((n as u64 * self.factor as u64) >> 32) as u32
    }
}

/// A `GaussPass` or `TentPass` over one row or column of 4 channels.
struct Pass {
    gauss: bool,
    border: i32,
    divider: Divider,
    window: usize,
    buf0: Vec<[u32; 4]>,
    buf1: Vec<[u32; 4]>,
    buf2: Vec<[u32; 4]>,
    c0: usize,
    c1: usize,
    c2: usize,
    sum0: [u32; 4],
    sum1: [u32; 4],
    sum2: [u32; 4],
}

impl Pass {
    fn make(sigma: f64) -> Pass {
        let window = calculate_window(sigma);
        if window < 255 {
            let border = if window & 1 == 1 {
                3 * ((window - 1) / 2)
            } else {
                3 * (window / 2) - 1
            };
            let w2 = window * window;
            let w3 = w2 * window;
            let divisor = if window & 1 == 1 { w3 } else { w3 + w2 };
            let pass = (window - 1) as usize;
            let last = if window & 1 == 1 { pass } else { pass + 1 };
            Pass {
                gauss: true,
                border,
                divider: Divider::new(divisor as u32),
                window: window as usize,
                buf0: vec![[0; 4]; pass],
                buf1: vec![[0; 4]; pass],
                buf2: vec![[0; 4]; last],
                c0: 0,
                c1: 0,
                c2: 0,
                sum0: [0; 4],
                sum1: [0; 4],
                sum2: [0; 4],
            }
        } else {
            let tent = 3 * window / 2;
            let pass = (tent - 1) as usize;
            Pass {
                gauss: false,
                border: tent - 1,
                divider: Divider::new((tent * tent) as u32),
                window: tent as usize,
                buf0: vec![[0; 4]; pass],
                buf1: vec![[0; 4]; pass],
                buf2: Vec::new(),
                c0: 0,
                c1: 0,
                c2: 0,
                sum0: [0; 4],
                sum1: [0; 4],
                sum2: [0; 4],
            }
        }
    }

    fn start(&mut self) {
        let half = self.divider.half;
        self.sum0 = [0; 4];
        if self.gauss {
            self.sum1 = [0; 4];
            self.sum2 = [half; 4];
        } else {
            self.sum1 = [half; 4];
        }
        for b in [&mut self.buf0, &mut self.buf1, &mut self.buf2] {
            b.iter_mut().for_each(|v| *v = [0; 4]);
        }
        self.c0 = 0;
        self.c1 = 0;
        self.c2 = 0;
    }

    fn process(&mut self, leading: [u32; 4]) -> [u8; 4] {
        let mut out = [0u8; 4];
        // Buffers of length 0 (a window of 1) never hold anything.
        let wrap = |c: &mut usize, len: usize| {
            *c = if *c + 1 < len { *c + 1 } else { 0 };
        };
        if self.gauss {
            for i in 0..4 {
                self.sum0[i] = self.sum0[i].wrapping_add(leading[i]);
                self.sum1[i] = self.sum1[i].wrapping_add(self.sum0[i]);
                self.sum2[i] = self.sum2[i].wrapping_add(self.sum1[i]);
                out[i] = self.divider.divide(self.sum2[i]) as u8;
            }
            if !self.buf2.is_empty() {
                for i in 0..4 {
                    self.sum2[i] = self.sum2[i].wrapping_sub(self.buf2[self.c2][i]);
                }
                self.buf2[self.c2] = self.sum1;
                wrap(&mut self.c2, self.buf2.len());
            }
            if !self.buf1.is_empty() {
                for i in 0..4 {
                    self.sum1[i] = self.sum1[i].wrapping_sub(self.buf1[self.c1][i]);
                }
                self.buf1[self.c1] = self.sum0;
                wrap(&mut self.c1, self.buf1.len());
                for i in 0..4 {
                    self.sum0[i] = self.sum0[i].wrapping_sub(self.buf0[self.c0][i]);
                }
                self.buf0[self.c0] = leading;
                wrap(&mut self.c0, self.buf0.len());
            }
        } else {
            for i in 0..4 {
                self.sum0[i] = self.sum0[i].wrapping_add(leading[i]);
                self.sum1[i] = self.sum1[i].wrapping_add(self.sum0[i]);
                out[i] = self.divider.divide(self.sum1[i]) as u8;
            }
            for i in 0..4 {
                self.sum1[i] = self.sum1[i].wrapping_sub(self.buf1[self.c1][i]);
            }
            self.buf1[self.c1] = self.sum0;
            wrap(&mut self.c1, self.buf1.len());
            for i in 0..4 {
                self.sum0[i] = self.sum0[i].wrapping_sub(self.buf0[self.c0][i]);
            }
            self.buf0[self.c0] = leading;
            wrap(&mut self.c0, self.buf0.len());
        }
        out
    }

    /// `Pass::blur` over a line: `src` is the source line (indices
    /// `src_left..src_right` relative to the destination's start), `dst`
    /// gets `dst_right` values.
    fn blur(
        &mut self,
        src_left: i32,
        src_right: i32,
        dst_right: i32,
        src: &[u32],
        dst: &mut [u32],
    ) {
        self.start();
        let src_start = src_left - self.border;
        let src_end = src_right - self.border;
        let dst_end = dst_right;
        let mut src_idx = src_start;
        let mut dst_idx = 0;
        let mut sc = 0usize;
        let load = |p: u32| [p & 0xFF, (p >> 8) & 0xFF, (p >> 16) & 0xFF, p >> 24];
        let store = |v: [u8; 4]| {
            v[0] as u32 | (v[1] as u32) << 8 | (v[2] as u32) << 16 | (v[3] as u32) << 24
        };
        if dst_idx < src_idx {
            let common_end = src_idx.min(dst_end);
            while dst_idx < common_end {
                dst[dst_idx as usize] = 0;
                dst_idx += 1;
            }
        } else if src_idx < dst_idx {
            let common_end = dst_idx.min(src_end);
            if src_idx < common_end {
                let n = common_end - src_idx;
                for _ in 0..n {
                    self.process(load(src[sc]));
                    sc += 1;
                }
                src_idx += n;
            }
            if src_idx < dst_idx {
                let n = dst_idx - src_idx;
                for _ in 0..n {
                    self.process([0; 4]);
                }
                src_idx += n;
            }
        }
        let common_end = dst_end.min(src_end);
        if dst_idx < common_end {
            let n = common_end - dst_idx;
            for _ in 0..n {
                let v = self.process(load(src[sc]));
                dst[dst_idx as usize] = store(v);
                sc += 1;
                dst_idx += 1;
            }
            src_idx += n;
        }
        let _ = src_idx;
        while dst_idx < dst_end {
            let v = self.process([0; 4]);
            dst[dst_idx as usize] = store(v);
            dst_idx += 1;
        }
        let _ = self.window;
    }
}

/// `SkImageFilters::Blur(sigma_x, sigma_y)` applied to `src`, output over
/// the same bounds (what a filter evaluated with the image's bounds as its
/// desired output gives).
pub fn blur(src: &Pixmap, sigma_x: f32, sigma_y: f32) -> Pixmap {
    let sigma_x = sigma_x.min(532.0);
    let sigma_y = sigma_y.min(532.0);
    let sx = if !sigma_x.is_finite() || calculate_window(sigma_x as f64) <= 1 {
        0.0
    } else {
        sigma_x
    };
    let sy = if !sigma_y.is_finite() || calculate_window(sigma_y as f64) <= 1 {
        0.0
    } else {
        sigma_y
    };
    if sx == 0.0 && sy == 0.0 {
        return src.clone();
    }
    let (w, h) = (src.width() as i32, src.height() as i32);
    let mut pass_x = Pass::make(sx as f64);
    let mut pass_y = Pass::make(sy as f64);

    // srcBounds = dstBounds = (0, 0, w, h)
    if pass_x.window > 1 && sx != 0.0 {
        let y_pad = (3.0 * sy).ceil() as i32;
        let dh = h + 2 * y_pad;
        // X pass into a buffer outset by y_pad rows (rows outside src stay 0).
        let mut tmp = vec![0u32; (w * dh) as usize];
        for y in 0..h {
            let row = &src.pixels()[(y * w) as usize..((y + 1) * w) as usize];
            let dst = &mut tmp[((y + y_pad) * w) as usize..((y + y_pad + 1) * w) as usize];
            pass_x.blur(0, w, w, row, dst);
        }
        let mut out = Pixmap::new(w as u32, h as u32);
        if pass_y.window > 1 && sy != 0.0 {
            let mut col = vec![0u32; dh as usize];
            let mut dcol = vec![0u32; h as usize];
            for x in 0..w {
                for y in 0..dh {
                    col[y as usize] = tmp[(y * w + x) as usize];
                }
                // src rows -y_pad..h+y_pad, dst rows 0..h
                pass_y.blur(-y_pad, h + y_pad, h, &col, &mut dcol);
                for y in 0..h {
                    out.pixels_mut()[(y * w + x) as usize] = dcol[y as usize];
                }
            }
        } else {
            for y in 0..h {
                let s = ((y + y_pad) * w) as usize;
                out.pixels_mut()[(y * w) as usize..((y + 1) * w) as usize]
                    .copy_from_slice(&tmp[s..s + w as usize]);
            }
        }
        out
    } else {
        let mut out = Pixmap::new(w as u32, h as u32);
        let mut col = vec![0u32; h as usize];
        let mut dcol = vec![0u32; h as usize];
        for x in 0..w {
            for y in 0..h {
                col[y as usize] = src.pixel(x, y);
            }
            pass_y.blur(0, h, h, &col, &mut dcol);
            for y in 0..h {
                out.pixels_mut()[(y * w + x) as usize] = dcol[y as usize];
            }
        }
        out
    }
}
