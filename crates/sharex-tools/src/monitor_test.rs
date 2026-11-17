//! Monitor test: the patterns `MonitorTestPatternControl` draws (solid gray
//! or RGB, a two-stop gradient, line and checkerboard patterns, moving bars)
//! as images, and the motion offset's animation (`OnAnimationTick`).

use sharex_skia::raster::gradient::LinearGradient;
use sharex_skia::raster::{Canvas, Matrix, Paint, Rect, Shader};

use crate::imaging::{self, Bitmap, Rgba};

/// `MonitorTestMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Grayscale,
    Rgb,
    Gradient,
    Pattern,
    Motion,
}

/// `MonitorGradientDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    ForwardDiagonal,
    BackwardDiagonal,
}

/// `MonitorPattern`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    HorizontalLines,
    VerticalLines,
    Checkerboard,
}

/// `MonitorMotionDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionDirection {
    VerticalBars,
    HorizontalBars,
}

/// The control's properties, with `MonitorTestViewModel`'s defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorTest {
    pub mode: Mode,
    pub gray: f64,
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub gradient_start: Rgba,
    pub gradient_end: Rgba,
    pub gradient_direction: GradientDirection,
    pub pattern: Pattern,
    pub pattern_size: f64,
    pub motion_direction: MotionDirection,
    pub motion_speed: f64,
}

impl Default for MonitorTest {
    fn default() -> Self {
        MonitorTest {
            mode: Mode::Grayscale,
            gray: 255.0,
            red: 255.0,
            green: 0.0,
            blue: 0.0,
            gradient_start: imaging::WHITE,
            gradient_end: imaging::BLACK,
            gradient_direction: GradientDirection::Vertical,
            pattern: Pattern::HorizontalLines,
            pattern_size: 5.0,
            motion_direction: MotionDirection::VerticalBars,
            motion_speed: 500.0,
        }
    }
}

/// `ToByte`: rounded and clamped.
pub fn to_byte(v: f64) -> u8 {
    v.clamp(0.0, 255.0).round_ties_even() as u8
}

/// `RgbText` / `GradientStartText`: `#RRGGBB`.
pub fn hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// `OnAnimationTick`: the offset after `elapsed` seconds since the last tick.
pub fn advance_motion(offset: f64, elapsed_seconds: f64, speed: f64) -> f64 {
    offset + elapsed_seconds * speed.clamp(100.0, 2000.0)
}

impl MonitorTest {
    /// `Render` into a `width` x `height` image, with the motion offset the
    /// animation has reached.
    pub fn render(&self, width: u32, height: u32, motion_offset: f64) -> Bitmap {
        let mut out = imaging::create_bitmap(width, height);
        let (w, h) = (width as f32, height as f32);
        {
            let mut c = Canvas::new(&mut out);
            let full = Rect::xywh(0.0, 0.0, w, h);
            match self.mode {
                Mode::Grayscale => {
                    let g = to_byte(self.gray);
                    c.draw_rect(full, &Paint::fill([g, g, g, 255]));
                }
                Mode::Rgb => c.draw_rect(full, &Paint::fill([to_byte(self.red), to_byte(self.green), to_byte(self.blue), 255])),
                Mode::Gradient => {
                    let (start, end) = match self.gradient_direction {
                        GradientDirection::Horizontal => ((0.0, h * 0.5), (w, h * 0.5)),
                        GradientDirection::ForwardDiagonal => ((0.0, 0.0), (w, h)),
                        GradientDirection::BackwardDiagonal => ((w, 0.0), (0.0, h)),
                        GradientDirection::Vertical => ((w * 0.5, 0.0), (w * 0.5, h)),
                    };
                    let mut paint = Paint::shader(Shader::Gradient(LinearGradient {
                        start,
                        end,
                        colors: vec![self.gradient_start, self.gradient_end],
                        positions: Some(vec![0.0, 1.0]),
                        local: Matrix::IDENTITY,
                    }));
                    paint.anti_alias = true;
                    c.draw_rect(full, &paint);
                }
                Mode::Pattern => {
                    c.draw_rect(full, &Paint::fill(imaging::WHITE));
                }
                Mode::Motion => {
                    c.draw_rect(full, &Paint::fill(imaging::WHITE));
                    const BAR: f64 = 50.0;
                    let start = motion_offset % (BAR * 2.0) - BAR;
                    let black = Paint::fill(imaging::BLACK);
                    let mut p = start;
                    match self.motion_direction {
                        MotionDirection::VerticalBars => {
                            while p < w as f64 {
                                c.draw_rect(Rect::xywh(p as f32, 0.0, BAR as f32, h), &black);
                                p += BAR * 2.0;
                            }
                        }
                        MotionDirection::HorizontalBars => {
                            while p < h as f64 {
                                c.draw_rect(Rect::xywh(0.0, p as f32, w, BAR as f32), &black);
                                p += BAR * 2.0;
                            }
                        }
                    }
                }
            }
        }
        if self.mode == Mode::Pattern {
            // The tiled DrawingBrush: a period x period tile of whole pixels.
            let size = self.pattern_size.round_ties_even().max(1.0) as u32;
            let period = size * 2;
            for y in 0..height {
                for x in 0..width {
                    let (tx, ty) = (x % period, y % period);
                    let black = match self.pattern {
                        Pattern::HorizontalLines => ty < size,
                        Pattern::VerticalLines => tx < size,
                        Pattern::Checkerboard => (tx < size) == (ty < size),
                    };
                    if black {
                        out.set_pixel(x, y, imaging::BLACK);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        let t = MonitorTest { mode: Mode::Pattern, pattern: Pattern::Checkerboard, pattern_size: 2.0, ..Default::default() };
        let b = t.render(8, 8, 0.0);
        assert_eq!(b.pixel(0, 0), imaging::BLACK);
        assert_eq!(b.pixel(2, 0), imaging::WHITE);
        assert_eq!(b.pixel(2, 2), imaging::BLACK);
        assert_eq!(hex([255, 0, 16, 255]), "#FF0010");
        assert_eq!(advance_motion(0.0, 0.5, 5000.0), 1000.0);
    }
}
