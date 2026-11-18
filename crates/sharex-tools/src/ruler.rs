//! The Ruler's (`Tools › Ruler`, "ShareX - Measure") pixel work:
//! `ShareX.Tools/Tools/Ruler/ScreenPixelBuffer.cs`. The screen is captured
//! once when the ruler opens; lines are runs of similar colour through a
//! pixel and rectangles shrink to the content inside a dragged area.

/// A rectangle in desktop pixels (`System.Drawing.Rectangle`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// `Rectangle.FromLTRB`.
    pub const fn from_ltrb(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self { x: l, y: t, w: r - l, h: b - t }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// `Rectangle.IsEmpty`: all four zero.
    pub const fn is_empty(&self) -> bool {
        self.x == 0 && self.y == 0 && self.w == 0 && self.h == 0
    }

    /// `PixelRect.Contains` / `Rectangle.Contains(x, y)`.
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    /// `Rectangle.Offset`.
    pub fn offset(&mut self, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
    }
}

/// `ScreenPixelBuffer`: the desktop as 0xAARRGGBB pixels.
pub struct ScreenPixelBuffer {
    pub bounds: Rect,
    pixels: Vec<u32>,
}

impl ScreenPixelBuffer {
    /// From straight RGBA rows covering `bounds`.
    pub fn from_rgba(bounds: Rect, rgba: &[u8]) -> Self {
        let pixels = rgba.chunks_exact(4).map(|p| (p[3] as u32) << 24 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32).collect();
        Self { bounds, pixels }
    }

    fn pixel(&self, sx: i32, sy: i32) -> u32 {
        let (x, y) = (sx - self.bounds.x, sy - self.bounds.y);
        self.pixels[(x + y * self.bounds.w) as usize]
    }

    /// `FindColorRun`: the run of colours within `tolerance` of the pixel
    /// at `point`, along a row or a column (one pixel thick).
    pub fn find_color_run(&self, (px, py): (i32, i32), horizontal: bool, tolerance: i32) -> Rect {
        if !self.bounds.contains(px, py) {
            return Rect::default();
        }
        let reference = self.pixel(px, py);
        let b = self.bounds;
        if horizontal {
            let mut left = px;
            while left > b.x && colors_are_close(reference, self.pixel(left - 1, py), tolerance) {
                left -= 1;
            }
            let mut right = px + 1;
            while right < b.right() && colors_are_close(reference, self.pixel(right, py), tolerance) {
                right += 1;
            }
            return Rect::new(left, py, right - left, 1);
        }
        let mut top = py;
        while top > b.y && colors_are_close(reference, self.pixel(px, top - 1), tolerance) {
            top -= 1;
        }
        let mut bottom = py + 1;
        while bottom < b.bottom() && colors_are_close(reference, self.pixel(px, bottom), tolerance) {
            bottom += 1;
        }
        Rect::new(px, top, 1, bottom - top)
    }

    /// `FindContentBounds`: `selection` shrunk to what differs from its
    /// top-left (left and top edges) and bottom-right (right and bottom
    /// edges) colours.
    pub fn find_content_bounds(&self, selection: Rect, tolerance: i32) -> Rect {
        let s = self.clamp(selection);
        if s.w <= 1 || s.h <= 1 {
            return s;
        }
        let top_left = self.pixel(s.x, s.y);
        let bottom_right = self.pixel(s.right() - 1, s.bottom() - 1);
        let differs = |bg: u32, x: i32, y: i32| !colors_are_close(bg, self.pixel(x, y), tolerance);
        let first_col = (s.x..s.right()).find(|&x| (s.y..s.bottom()).any(|y| differs(top_left, x, y)));
        let Some(left) = first_col else { return s };
        let top = (s.y..s.bottom()).find(|&y| (s.x..s.right()).any(|x| differs(top_left, x, y)));
        let right = (s.x..s.right()).rev().find(|&x| (s.y..s.bottom()).any(|y| differs(bottom_right, x, y)));
        let bottom = (s.y..s.bottom()).rev().find(|&y| (s.x..s.right()).any(|x| differs(bottom_right, x, y)));
        match (top, right, bottom) {
            (Some(top), Some(right), Some(bottom)) if right >= left && bottom >= top => {
                Rect::from_ltrb(left, top, right + 1, bottom + 1)
            }
            _ => s,
        }
    }

    /// `Clamp(Rectangle)`.
    pub fn clamp(&self, r: Rect) -> Rect {
        let b = self.bounds;
        let l = r.x.clamp(b.x, b.right());
        let t = r.y.clamp(b.y, b.bottom());
        let rr = r.right().clamp(b.x, b.right());
        let bb = r.bottom().clamp(b.y, b.bottom());
        Rect::from_ltrb(l.min(rr), t.min(bb), l.max(rr), t.max(bb))
    }

    /// `ClampPreservingSize`.
    pub fn clamp_preserving_size(&self, r: Rect) -> Rect {
        let b = self.bounds;
        let w = r.w.min(b.w);
        let h = r.h.min(b.h);
        Rect::new(r.x.clamp(b.x, b.right() - w), r.y.clamp(b.y, b.bottom() - h), w, h)
    }

    /// `Clamp(PixelPoint)`.
    pub fn clamp_point(&self, (x, y): (i32, i32)) -> (i32, i32) {
        let b = self.bounds;
        (x.clamp(b.x, b.right() - 1), y.clamp(b.y, b.bottom() - 1))
    }
}

/// `ColorsAreClose`: the sum of the channel differences.
fn colors_are_close(a: u32, b: u32, tolerance: i32) -> bool {
    let d = |s: u32| (((a >> s) & 0xFF) as i32 - ((b >> s) & 0xFF) as i32).abs();
    d(16) + d(8) + d(0) <= tolerance
}

/// `RulerOverlayControl.CreateRectangle`: both corners included.
pub fn create_rectangle((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> Rect {
    Rect::new(ax.min(bx), ay.min(by), (bx - ax).abs() + 1, (by - ay).abs() + 1)
}

/// `RulerOverlayControl.ColorTolerance`: the percentage of 255.
pub fn color_tolerance(percentage: i32) -> i32 {
    (percentage as f64 * 255.0 / 100.0).round_ties_even() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer() -> ScreenPixelBuffer {
        // 10x6 white with a black 4x2 block at (3, 2).
        let (w, h) = (10, 6);
        let mut rgba = vec![255u8; w * h * 4];
        for y in 2..4 {
            for x in 3..7 {
                let i = (y * w + x) * 4;
                rgba[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        ScreenPixelBuffer::from_rgba(Rect::new(100, 50, w as i32, h as i32), &rgba)
    }

    #[test]
    fn runs_and_content() {
        let b = buffer();
        assert_eq!(b.find_color_run((104, 52), true, 0), Rect::new(103, 52, 4, 1));
        assert_eq!(b.find_color_run((101, 52), true, 0), Rect::new(100, 52, 3, 1));
        assert_eq!(b.find_color_run((104, 52), false, 0), Rect::new(104, 52, 1, 2));
        assert_eq!(b.find_content_bounds(Rect::new(100, 50, 10, 6), 0), Rect::new(103, 52, 4, 2));
        assert_eq!(color_tolerance(50), 128);
    }
}
