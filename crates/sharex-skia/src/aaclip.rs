//! Anti-aliased clips: `SkAAClip` (`src/core/SkAAClip.cpp`), what
//! `SkRasterClip` turns into when a path is clipped with anti-aliasing
//! (`clipPath(path, kIntersect, true)` on a rectangular clip:
//! `SkAAClip::setPath`), and `SkAAClipBlitter`, which modulates every blit
//! by the clip's coverage. The clip is stored as Skia stores it: rows of
//! `[count, alpha]` runs, consecutive equal rows merged, then trimmed; the
//! runs decide which calls the blitter beneath receives.

use crate::blitter::Blitter;
use crate::geom::IRect;
use crate::path::Path;

/// `SkMulDiv255Round`.
fn mul_div_255_round(a: u32, b: u32) -> u32 {
    let prod = a * b + 128;
    (prod + (prod >> 8)) >> 8
}

#[derive(Clone, Debug)]
struct Row {
    /// The last y (relative to `bounds.top`) this row stands for.
    y: i32,
    /// `[count, alpha]` pairs covering the bounds' width.
    data: Vec<u8>,
}

/// `SkAAClip`.
#[derive(Clone, Debug, Default)]
pub struct AAClip {
    bounds: IRect,
    rows: Vec<Row>,
}

impl AAClip {
    /// `SkAAClip::setPath(path, clip, doAA = true)`: the anti-aliased
    /// coverage of `path` (device space) inside `clip`.
    pub fn from_path(path: &Path, clip: IRect) -> AAClip {
        if clip.is_empty() {
            return AAClip::default();
        }
        // (Inverse fills aren't used here.)
        let mut ibounds = path.bounds().round_out();
        if ibounds.is_empty() || !ibounds.intersect(&clip) {
            return AAClip::default();
        }
        let mut builder = Builder::new(ibounds);
        {
            let mut blitter = BuilderBlitter::new(&mut builder);
            crate::scan::anti_fill_path_rle(path, ibounds, &mut blitter, true);
            blitter.finish();
        }
        builder.finish()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn bounds(&self) -> IRect {
        self.bounds
    }

    /// The clip's coverage of a pixel.
    pub fn alpha_at(&self, x: i32, y: i32) -> u8 {
        if self.is_empty()
            || x < self.bounds.left
            || x >= self.bounds.right
            || y < self.bounds.top
            || y >= self.bounds.bottom
        {
            return 0;
        }
        let (row, _) = self.find_row(y);
        let (off, _) = self.find_x(row, x);
        self.rows[row].data[off + 1]
    }

    /// `findRow`: the row holding `y` (inside the bounds) and its last y.
    fn find_row(&self, y: i32) -> (usize, i32) {
        let y = y - self.bounds.top;
        let mut i = 0;
        while self.rows[i].y < y {
            i += 1;
        }
        (i, self.bounds.top + self.rows[i].y)
    }

    /// `findX`: the offset of the run holding `x` (inside the bounds) and
    /// how many of its pixels are left from `x`.
    fn find_x(&self, row: usize, x: i32) -> (usize, i32) {
        let data = &self.rows[row].data;
        let mut x = x - self.bounds.left;
        let mut off = 0;
        loop {
            let n = data[off] as i32;
            if x < n {
                return (off, n - x);
            }
            off += 2;
            x -= n;
        }
    }

    /// `quickContains`.
    pub fn quick_contains(&self, left: i32, top: i32, right: i32, bottom: i32) -> bool {
        if self.is_empty() {
            return false;
        }
        let r = IRect::from_ltrb(left, top, right, bottom);
        // SkIRect::contains
        if r.is_empty()
            || self.bounds.left > left
            || self.bounds.top > top
            || self.bounds.right < right
            || self.bounds.bottom < bottom
        {
            return false;
        }
        let (row, last_y) = self.find_row(top);
        if last_y < bottom {
            return false;
        }
        let (mut off, mut count) = self.find_x(row, left);
        let data = &self.rows[row].data;
        let mut rect_width = right - left;
        while off + 1 < data.len() && data[off + 1] == 0xFF {
            if count >= rect_width {
                return true;
            }
            rect_width -= count;
            off += 2;
            if off >= data.len() {
                return false;
            }
            count = data[off] as i32;
        }
        false
    }

    /// `trimBounds`.
    fn trim_bounds(&mut self) {
        if self.is_empty() {
            return;
        }
        let last = self.rows.last().unwrap().y;
        self.bounds.bottom = self.bounds.top + last + 1;
        if self.trim_top_bottom() {
            self.trim_left_right();
        }
    }

    fn set_empty(&mut self) -> bool {
        self.bounds = IRect::default();
        self.rows.clear();
        false
    }

    /// `trimTopBottom`.
    fn trim_top_bottom(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        let width = self.bounds.width();
        let mut skip = 0;
        while skip < self.rows.len() && row_is_all_zeros(&self.rows[skip].data, width) {
            skip += 1;
        }
        if skip == self.rows.len() {
            return self.set_empty();
        }
        if skip > 0 {
            let dy = self.rows[skip - 1].y + 1;
            for r in &mut self.rows[skip..] {
                r.y -= dy;
            }
            self.rows.drain(..skip);
            self.bounds.top += dy;
        }
        let mut k = self.rows.len() - 1;
        while row_is_all_zeros(&self.rows[k].data, width) {
            k -= 1;
        }
        if k + 1 < self.rows.len() {
            self.rows.truncate(k + 1);
            self.bounds.bottom = self.bounds.top + self.rows[k].y + 1;
        }
        true
    }

    /// `trimLeftRight`.
    fn trim_left_right(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        let width = self.bounds.width();
        let mut left_zeros = width;
        let mut rite_zeros = width;
        for r in &self.rows {
            let (l, rz) = count_left_right_zeros(&r.data, width);
            left_zeros = left_zeros.min(l);
            rite_zeros = rite_zeros.min(rz);
            if (left_zeros | rite_zeros) == 0 {
                return true;
            }
        }
        if width == left_zeros {
            return self.set_empty();
        }
        self.bounds.left += left_zeros;
        self.bounds.right -= rite_zeros;
        for r in &mut self.rows {
            trim_row_left_right(&mut r.data, width, left_zeros, rite_zeros);
        }
        true
    }
}

fn row_is_all_zeros(row: &[u8], mut width: i32) -> bool {
    let mut off = 0;
    loop {
        if row[off + 1] != 0 {
            return false;
        }
        width -= row[off] as i32;
        off += 2;
        if width <= 0 {
            return true;
        }
    }
}

fn count_left_right_zeros(row: &[u8], mut width: i32) -> (i32, i32) {
    let mut off = 0;
    let mut zeros = 0;
    loop {
        if row[off + 1] != 0 {
            break;
        }
        let n = row[off] as i32;
        zeros += n;
        off += 2;
        width -= n;
        if width <= 0 {
            break;
        }
    }
    let left = zeros;
    if width == 0 {
        // this line is completely empty
        return (left, left);
    }
    zeros = 0;
    while width > 0 {
        let n = row[off] as i32;
        if row[off + 1] == 0 {
            zeros += n;
        } else {
            zeros = 0;
        }
        off += 2;
        width -= n;
    }
    (left, zeros)
}

/// `trim_row_left_right`: drops the zero runs (or parts of them) the trim
/// removes.
fn trim_row_left_right(row: &mut Vec<u8>, mut width: i32, mut left_z: i32, mut rite_z: i32) {
    let mut start = 0usize;
    let mut off = 0usize;
    while left_z > 0 {
        let n = row[off] as i32;
        width -= n;
        off += 2;
        if n > left_z {
            row[off - 2] = (n - left_z) as u8;
            break;
        }
        start += 2;
        left_z -= n;
    }
    let mut end = row.len();
    if rite_z != 0 {
        while width > 0 {
            let n = row[off] as i32;
            width -= n;
            off += 2;
        }
        loop {
            off -= 2;
            let n = row[off] as i32;
            if n > rite_z {
                row[off] = (n - rite_z) as u8;
                break;
            }
            rite_z -= n;
            end = off;
            if rite_z <= 0 {
                break;
            }
        }
    }
    row.truncate(end);
    row.drain(..start);
}

// --- SkAAClip::Builder ---------------------------------------------------------

struct BRow {
    y: i32,
    width: i32,
    data: Vec<u8>,
}

struct Builder {
    bounds: IRect,
    rows: Vec<BRow>,
    curr_row: Option<usize>,
    prev_y: i32,
    width: i32,
    min_y: i32,
}

impl Builder {
    fn new(bounds: IRect) -> Self {
        Self {
            bounds,
            rows: Vec::new(),
            curr_row: None,
            prev_y: -1,
            width: bounds.width(),
            min_y: bounds.top,
        }
    }

    fn append_run(data: &mut Vec<u8>, alpha: u8, mut count: i32) {
        loop {
            let n = count.min(255);
            data.push(n as u8);
            data.push(alpha);
            count -= n;
            if count <= 0 {
                break;
            }
        }
    }

    fn add_run(&mut self, x: i32, y: i32, alpha: u8, count: i32) {
        let x = x - self.bounds.left;
        let y = y - self.bounds.top;
        if y != self.prev_y {
            self.prev_y = y;
            let row = self.flush_row(true).unwrap();
            self.rows[row].y = y;
            self.rows[row].width = 0;
            self.curr_row = Some(row);
        }
        let row = &mut self.rows[self.curr_row.unwrap()];
        let gap = x - row.width;
        if gap != 0 {
            Self::append_run(&mut row.data, 0, gap);
            row.width += gap;
        }
        Self::append_run(&mut row.data, alpha, count);
        row.width += count;
    }

    fn add_column(&mut self, x: i32, y: i32, alpha: u8, height: i32) {
        self.add_run(x, y, alpha, 1);
        let cur = self.curr_row.unwrap();
        self.flush_row_h(cur);
        self.rows[cur].y = y - self.bounds.top + height - 1;
    }

    fn add_rect_run(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.add_run(x, y, 0xFF, width);
        let cur = self.curr_row.unwrap();
        self.flush_row_h(cur);
        self.rows[cur].y = y - self.bounds.top + height - 1;
    }

    fn add_anti_rect_run(
        &mut self,
        mut x: i32,
        y: i32,
        mut width: i32,
        height: i32,
        left_alpha: u8,
        right_alpha: u8,
    ) {
        if left_alpha == 0xFF {
            width += 1;
        } else if left_alpha > 0 {
            self.add_run(x, y, left_alpha, 1);
            x += 1;
        } else {
            x += 1;
        }
        if right_alpha == 0xFF {
            width += 1;
        }
        if width > 0 {
            self.add_run(x, y, 0xFF, width);
        }
        if right_alpha > 0 && right_alpha < 255 {
            self.add_run(x + width, y, right_alpha, 1);
        }
        if let Some(cur) = self.curr_row {
            self.flush_row_h(cur);
            self.rows[cur].y = y - self.bounds.top + height - 1;
        }
    }

    fn flush_row_h(&mut self, row: usize) {
        let w = self.width;
        let r = &mut self.rows[row];
        if r.width < w {
            Self::append_run(&mut r.data, 0, w - r.width);
            r.width = w;
        }
    }

    fn flush_row(&mut self, ready_for_another: bool) -> Option<usize> {
        let count = self.rows.len();
        if count > 0 {
            self.flush_row_h(count - 1);
        }
        let new_row = |rows: &mut Vec<BRow>| {
            rows.push(BRow {
                y: 0,
                width: 0,
                data: Vec::new(),
            });
            rows.len() - 1
        };
        if count > 1 {
            if self.rows[count - 2].data == self.rows[count - 1].data {
                self.rows[count - 2].y = self.rows[count - 1].y;
                if ready_for_another {
                    self.rows[count - 1].data.clear();
                    Some(count - 1)
                } else {
                    self.rows.pop();
                    None
                }
            } else if ready_for_another {
                Some(new_row(&mut self.rows))
            } else {
                None
            }
        } else if ready_for_another {
            Some(new_row(&mut self.rows))
        } else {
            None
        }
    }

    fn finish(mut self) -> AAClip {
        self.flush_row(false);
        let data_size: usize = self.rows.iter().map(|r| r.data.len()).sum();
        if data_size == 0 {
            return AAClip::default();
        }
        let adjust_y = self.min_y - self.bounds.top;
        let mut bounds = self.bounds;
        bounds.top = self.min_y;
        let rows = self
            .rows
            .into_iter()
            .map(|r| Row {
                y: r.y - adjust_y,
                data: r.data,
            })
            .collect();
        let mut clip = AAClip { bounds, rows };
        clip.trim_bounds();
        clip
    }
}

/// `SkAAClip::Builder::Blitter`.
struct BuilderBlitter<'a> {
    builder: &'a mut Builder,
    left: i32,
    right: i32,
    min_y: i32,
    last_y: i32,
}

const MAX_S32: i32 = i32::MAX;

impl<'a> BuilderBlitter<'a> {
    fn new(builder: &'a mut Builder) -> Self {
        let (left, right) = (builder.bounds.left, builder.bounds.right);
        Self {
            builder,
            left,
            right,
            min_y: MAX_S32,
            last_y: -MAX_S32,
        }
    }

    fn finish(&mut self) {
        if self.min_y < MAX_S32 {
            self.builder.min_y = self.min_y;
        }
    }

    fn record_min_y(&mut self, y: i32) {
        if y < self.min_y {
            self.min_y = y;
        }
    }

    fn check_for_y_gap(&mut self, y: i32) {
        if self.last_y > -MAX_S32 {
            let gap = y - self.last_y;
            if gap > 1 {
                self.builder
                    .add_run(self.left, y - 1, 0, self.right - self.left);
            }
        }
        self.last_y = y;
    }
}

impl Blitter for BuilderBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder.add_run(x, y, 0xFF, width);
    }

    fn blit_anti_h(&mut self, mut x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        let mut i = 0usize;
        loop {
            let count = runs[i] as i32;
            if count <= 0 {
                return;
            }
            let mut local_x = x;
            let mut local_count = count;
            if x < self.left {
                let gap = self.left - x;
                local_x += gap;
                local_count -= gap;
            }
            let right = x + count;
            if right > self.right {
                local_count -= right - self.right;
            }
            if local_count > 0 {
                self.builder.add_run(local_x, y, aa[i], local_count);
            }
            i += count as usize;
            x += count;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if height == 1 {
            self.blit_anti_h(x, y, &[alpha, 0], &[1, 0]);
        } else {
            self.record_min_y(y);
            self.builder.add_column(x, y, alpha, height);
            self.last_y = y + height - 1;
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder.add_rect_run(x, y, width, height);
        self.last_y = y + height - 1;
    }

    fn blit_anti_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        left_alpha: u8,
        right_alpha: u8,
    ) {
        self.record_min_y(y);
        self.check_for_y_gap(y);
        self.builder
            .add_anti_rect_run(x, y, width, height, left_alpha, right_alpha);
        self.last_y = y + height - 1;
    }

    fn blit_mask(&mut self, _mask: &[u8], _bounds: IRect, _row_bytes: i32, _clip: IRect) {
        // The scan converters don't use their mask blitters here (forceRLE).
        unreachable!("SkAAClip::Builder::Blitter::blitMask");
    }
}

// --- SkAAClipBlitter -----------------------------------------------------------

/// `SkAAClipBlitter` over `inner`. Blits are first clipped to the clip's
/// bounds (the `SkRectClipBlitter` `SkScanClipper` puts in front of it).
pub struct AAClipBlitter<'a> {
    pub inner: &'a mut dyn Blitter,
    pub clip: &'a AAClip,
}

/// `expandToRuns`.
fn expand_to_runs(data: &[u8], mut off: usize, initial_count: i32, mut width: i32) -> (Vec<u8>, Vec<i16>) {
    let mut runs = vec![0i16; width as usize + 1];
    let mut aa = vec![0u8; width as usize + 1];
    let mut k = 0usize;
    let mut n = initial_count;
    loop {
        if n > width {
            n = width;
        }
        runs[k] = n as i16;
        aa[k] = data[off + 1];
        k += n as usize;
        off += 2;
        width -= n;
        if width == 0 {
            break;
        }
        n = data[off] as i32;
    }
    runs[k] = 0;
    (aa, runs)
}

impl AAClipBlitter<'_> {
    fn bounds(&self) -> IRect {
        self.clip.bounds
    }

    /// `SkAAClipBlitter::blitH` for a span inside the bounds.
    fn blit_h_inside(&mut self, x: i32, y: i32, width: i32) {
        let (row, _) = self.clip.find_row(y);
        let (off, initial) = self.clip.find_x(row, x);
        let data = &self.clip.rows[row].data;
        if initial >= width {
            let alpha = data[off + 1];
            if alpha == 0 {
                return;
            }
            if alpha == 0xFF {
                self.inner.blit_h(x, y, width);
                return;
            }
        }
        let (aa, runs) = expand_to_runs(data, off, initial, width);
        self.inner.blit_anti_h(x, y, &aa, &runs);
    }

    /// `SkAAClipBlitter::blitAntiH` (`merge`) for runs inside the bounds.
    fn blit_anti_h_inside(&mut self, x: i32, y: i32, src_aa: &[u8], src_runs: &[i16]) {
        let (row, _) = self.clip.find_row(y);
        let (mut off, mut row_n) = self.clip.find_x(row, x);
        let data = &self.clip.rows[row].data;
        let mut src_n = src_runs[0] as i32;
        if src_n == 0 {
            return;
        }
        let total: usize = {
            let mut t = 0usize;
            let mut i = 0usize;
            while src_runs[i] > 0 {
                t += src_runs[i] as usize;
                i += src_runs[i] as usize;
            }
            t
        };
        let mut dst_runs = vec![0i16; total + 1];
        let mut dst_aa = vec![0u8; total + 1];
        let (mut s, mut d) = (0usize, 0usize);
        loop {
            let new_alpha = mul_div_255_round(src_aa[s] as u32, data[off + 1] as u32);
            let min_n = src_n.min(row_n);
            dst_runs[d] = min_n as i16;
            dst_aa[d] = new_alpha as u8;
            d += min_n as usize;
            src_n -= min_n;
            if src_n == 0 {
                let run = src_runs[s] as usize;
                s += run;
                src_n = src_runs[s] as i32;
                if src_n == 0 {
                    break;
                }
            }
            row_n -= min_n;
            if row_n == 0 {
                off += 2;
                row_n = data[off] as i32;
            }
        }
        dst_runs[d] = 0;
        self.inner.blit_anti_h(x, y, &dst_aa, &dst_runs);
    }
}

impl Blitter for AAClipBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        let b = self.bounds();
        if y < b.top || y >= b.bottom {
            return;
        }
        let l = x.max(b.left);
        let r = (x + width).min(b.right);
        if r > l {
            self.blit_h_inside(l, y, r - l);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        let b = self.bounds();
        if y < b.top || y >= b.bottom || x >= b.right {
            return;
        }
        // SkRectClipBlitter: break the runs at the bounds.
        let mut width = 0i32;
        let mut i = 0usize;
        while runs[i] > 0 {
            width += runs[i] as i32;
            i += runs[i] as usize;
        }
        let (x0, x1) = (x, x + width);
        if x1 <= b.left {
            return;
        }
        if x0 >= b.left && x1 <= b.right {
            self.blit_anti_h_inside(x, y, aa, runs);
            return;
        }
        let l = x0.max(b.left);
        let r = x1.min(b.right);
        let n = (r - l) as usize;
        let mut cruns = vec![0i16; n + 1];
        let mut caa = vec![0u8; n + 1];
        let mut px = x;
        let mut i = 0usize;
        while runs[i] > 0 {
            let count = runs[i] as i32;
            let a = px.max(l);
            let e = (px + count).min(r);
            if e > a {
                let k = (a - l) as usize;
                cruns[k] = (e - a) as i16;
                caa[k] = aa[i];
            }
            px += count;
            i += count as usize;
        }
        self.blit_anti_h_inside(l, y, &caa, &cruns);
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        let b = self.bounds();
        if x < b.left || x >= b.right {
            return;
        }
        let y1 = (y + height).min(b.bottom);
        let mut y = y.max(b.top);
        let mut height = y1 - y;
        if height <= 0 {
            return;
        }
        if self.clip.quick_contains(x, y, x + 1, y + height) {
            self.inner.blit_v(x, y, height, alpha);
            return;
        }
        loop {
            let (row, last_y) = self.clip.find_row(y);
            let dy = (last_y - y + 1).min(height);
            height -= dy;
            let (off, _) = self.clip.find_x(row, x);
            let new_alpha = mul_div_255_round(alpha as u32, self.clip.rows[row].data[off + 1] as u32);
            if new_alpha != 0 {
                self.inner.blit_v(x, y, dy, new_alpha as u8);
            }
            if height <= 0 {
                break;
            }
            y = last_y + 1;
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let mut r = IRect::from_ltrb(x, y, x + width, y + height);
        if !r.intersect(&self.bounds()) {
            return;
        }
        if self.clip.quick_contains(r.left, r.top, r.right, r.bottom) {
            self.inner.blit_rect(r.left, r.top, r.width(), r.height());
            return;
        }
        for yy in r.top..r.bottom {
            self.blit_h_inside(r.left, yy, r.width());
        }
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        let mut c = clip;
        if !c.intersect(&self.bounds()) {
            return;
        }
        if self.clip.quick_contains(c.left, c.top, c.right, c.bottom) {
            self.inner.blit_mask(mask, bounds, row_bytes, c);
            return;
        }
        // mergeT<uint8_t>, one row at a time.
        let width = c.width();
        let mut row_mask = vec![0u8; width as usize];
        for y in c.top..c.bottom {
            let (row, _) = self.clip.find_row(y);
            let (mut off, mut row_n) = self.clip.find_x(row, c.left);
            let data = &self.clip.rows[row].data;
            let src = ((y - bounds.top) * row_bytes + (c.left - bounds.left)) as usize;
            let mut src_n = width;
            let mut k = 0usize;
            loop {
                let n = row_n.min(src_n) as usize;
                let row_a = data[off + 1];
                for i in 0..n {
                    row_mask[k + i] = match row_a {
                        0xFF => mask[src + k + i],
                        0 => 0,
                        _ => mul_div_255_round(mask[src + k + i] as u32, row_a as u32) as u8,
                    };
                }
                src_n -= n as i32;
                if src_n == 0 {
                    break;
                }
                k += n;
                off += 2;
                row_n = data[off] as i32;
            }
            let rb = IRect::from_ltrb(c.left, y, c.right, y + 1);
            self.inner.blit_mask(&row_mask, rb, width, rb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;

    #[test]
    fn ellipse_clip_coverage() {
        let mut p = Path::new();
        p.add_oval(Rect::from_ltrb(10.0, 10.0, 50.0, 30.0), crate::path::Direction::Cw);
        let clip = AAClip::from_path(&p, IRect::from_ltrb(0, 0, 64, 64));
        assert_eq!(clip.bounds(), IRect::from_ltrb(10, 10, 50, 30));
        assert_eq!(clip.alpha_at(30, 20), 255);
        assert_eq!(clip.alpha_at(10, 10), 0);
        let partial = (10..50)
            .flat_map(|x| (10..30).map(move |y| (x, y)))
            .filter(|&(x, y)| (1..255).contains(&clip.alpha_at(x, y)))
            .count();
        assert!(partial > 20);
        // Skia's test is conservative: the first row must reach past `bottom`.
        assert!(!clip.quick_contains(25, 15, 35, 16));
        assert!(!clip.quick_contains(10, 10, 20, 12));
    }

    #[test]
    fn trims_rows() {
        let mut row = vec![3, 0, 2, 9, 4, 0];
        trim_row_left_right(&mut row, 9, 2, 3);
        assert_eq!(row, vec![1, 0, 2, 9, 1, 0]);
        let mut row = vec![3, 0, 2, 9, 4, 0];
        trim_row_left_right(&mut row, 9, 3, 4);
        assert_eq!(row, vec![2, 9]);
    }
}
