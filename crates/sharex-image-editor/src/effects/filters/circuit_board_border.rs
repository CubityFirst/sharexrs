//! `CircuitBoardBorderImageEffect` (`Core/ImageEffects/Filters/CircuitBoardBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `CircuitBoardBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct CircuitBoardBorderImageEffect {
    pub border_size: i32,
    pub board_color: SkColor,
    pub trace_color: SkColor,
    pub trace_density: f32,
    pub glow: bool,
    pub inner_line: bool,
}

impl Default for CircuitBoardBorderImageEffect {
    fn default() -> Self {
        CircuitBoardBorderImageEffect {
            border_size: 64,
            board_color: SkColor::argb(0xFF0C3A20),
            trace_color: SkColor::argb(0xFFB87333),
            trace_density: 55.0,
            glow: true,
            inner_line: true,
        }
    }
}

impl EffectImpl for CircuitBoardBorderImageEffect {
    fn info() -> Info {
        Info::new("circuit_board_border", "Circuit board border", EffectCategory::Filters, "image-effect/circuit-board")
            .desc("Adds a PCB circuit board border with copper traces, vias, and pads on a dark substrate.")
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                20,
                300,
                64,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "board_color",
                "Board color",
                SkColor::argb(0xFF0C3A20),
                |e: &mut Self, v| e.board_color = v,
            ),
            color_param::<Self>(
                "trace_color",
                "Trace color",
                SkColor::argb(0xFFB87333),
                |e: &mut Self, v| e.trace_color = v,
            ),
            float_slider::<Self>(
                "trace_density",
                "Trace density",
                10.0,
                100.0,
                55.0,
                |e: &mut Self, v| e.trace_density = v,
            ),
            bool_param::<Self>("glow", "Trace glow", true, |e: &mut Self, v| e.glow = v),
            bool_param::<Self>("inner_line", "Inner line", true, |e: &mut Self, v| {
                e.inner_line = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        const SEED: i32 = 8157;
        let border = self.border_size.clamp(20, 300);
        let density = clampf(self.trace_density, 10.0, 100.0) / 100.0;
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        let (bf, nwf) = (border as f32, nw as f32);
        let board = solid(self.board_color, false);
        sk::draw_rect_xywh(&mut canvas, 0.0, 0.0, nwf, bf, &board, None);
        sk::draw_rect_xywh(
            &mut canvas,
            0.0,
            (nh - border) as f32,
            nwf,
            bf,
            &board,
            None,
        );
        sk::draw_rect_xywh(&mut canvas, 0.0, bf, bf, ih as f32, &board, None);
        sk::draw_rect_xywh(
            &mut canvas,
            (nw - border) as f32,
            bf,
            bf,
            ih as f32,
            &board,
            None,
        );
        let trace_w = 1.5f32.max(bf * 0.045);
        let pad_r = trace_w * 1.8;
        let grid = 6f32.max(bf * 0.22) as i32;
        let tc = self.trace_color;
        let p = Paints {
            glow: self.glow.then(|| {
                (
                    solid(SkColor::rgba(tc.r, tc.g, tc.b, 45), true),
                    Pen::new(trace_w * 3.5),
                    trace_w * 1.5,
                )
            }),
            trace: solid(tc, true),
            trace_pen: Pen::new(trace_w).cap(Cap::Round).join(Join::Round),
            pad: solid(tc, true),
            hole: solid(self.board_color, true),
        };
        traces(
            &mut canvas,
            &p,
            0.0,
            0.0,
            nwf,
            bf,
            true,
            grid,
            density,
            pad_r,
            SEED ^ 0x1A,
        );
        traces(
            &mut canvas,
            &p,
            0.0,
            (nh - border) as f32,
            nwf,
            bf,
            true,
            grid,
            density,
            pad_r,
            SEED ^ 0x2B,
        );
        traces(
            &mut canvas,
            &p,
            0.0,
            bf,
            bf,
            ih as f32,
            false,
            grid,
            density,
            pad_r,
            SEED ^ 0x3C,
        );
        traces(
            &mut canvas,
            &p,
            (nw - border) as f32,
            bf,
            bf,
            ih as f32,
            false,
            grid,
            density,
            pad_r,
            SEED ^ 0x4D,
        );
        canvas.draw_bitmap(source, bf, bf, None);
        if self.inner_line {
            let lp = solid(tc.with_alpha(200), true);
            sk::draw_rect_xywh(
                &mut canvas,
                bf - trace_w * 0.4,
                bf - trace_w * 0.4,
                iw as f32 + trace_w * 0.8,
                ih as f32 + trace_w * 0.8,
                &lp,
                Some(&Pen::new(trace_w * 0.8)),
            );
        }
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

struct Paints {
    glow: Option<(Paint<'static>, Pen, f32)>,
    trace: Paint<'static>,
    trace_pen: Pen,
    pad: Paint<'static>,
    hole: Paint<'static>,
}

#[allow(clippy::too_many_arguments)]
fn traces(
    canvas: &mut Canvas,
    p: &Paints,
    rx: f32,
    ry: f32,
    rw: f32,
    rh: f32,
    horizontal: bool,
    grid: i32,
    density: f32,
    pad_r: f32,
    seed: i32,
) {
    sk::with_clip_rect(canvas, RRect::ltrb(rx, ry, rx + rw, ry + rh), |canvas| {
        let primary = if horizontal { rw } else { rh };
        let cross = if horizontal { rh } else { rw };
        let n = (primary / grid as f32 * density) as i32 + 1;
        for i in 0..n {
            let h1 = hash01(i, seed, seed ^ 0x7F);
            let h2 = hash01(i, seed ^ 0x13, seed);
            let h3 = hash01(i, seed ^ 0x31, seed ^ 0x55);
            let along = h1 * primary;
            let across = h2 * cross * 0.85 + cross * 0.05;
            let t_len = (0.15 + h3 * 0.65) * primary * 0.5;
            let t_end = (along + t_len).min(primary - 1.0);
            let (x1, y1, x2, y2) = if horizontal {
                (rx + along, ry + across, rx + t_end, ry + across)
            } else {
                (rx + across, ry + along, rx + across, ry + t_end)
            };
            if let Some((gp, pen, sigma)) = &p.glow {
                sk::draw_line_blur(canvas, x1, y1, x2, y2, pen, gp, *sigma);
            }
            sk::draw_line(canvas, x1, y1, x2, y2, &p.trace_pen, &p.trace);
            sk::draw_circle(canvas, x1, y1, pad_r, &p.pad, None);
            let via = pad_r * 0.85;
            sk::draw_circle(canvas, x2, y2, via * 1.5, &p.pad, None);
            sk::draw_circle(canvas, x2, y2, via * 0.65, &p.hole, None);
            let h4 = hash01(i, seed ^ 0x9A, seed ^ 0xCF);
            if h4 < 0.4 && t_len > grid as f32 * 1.5 {
                let at = along + t_len * 0.5;
                let ec =
                    across + if h4 < 0.2 { -1.0 } else { 1.0 } * grid as f32 * (0.5 + h3 * 0.8);
                let (ex, ey, ex2, ey2) = if horizontal {
                    (rx + at, ry + across, rx + at, ry + ec)
                } else {
                    (rx + across, ry + at, rx + ec, ry + at)
                };
                sk::draw_line(canvas, ex, ey, ex2, ey2, &p.trace_pen, &p.trace);
                sk::draw_circle(canvas, ex2, ey2, pad_r * 0.9, &p.pad, None);
            }
        }
    });
}
