//! `SkPathWriter.h`, `SkPathWriter.cpp`: construct the path one contour at a
//! time. If the contour is closed, copy it to the final output; otherwise
//! keep the partial contour for later assembly.

use super::graph::*;
use super::types::SK_MAX_S32;
use super::types::zero_or_one;
use crate::geom::Point;
use crate::path::Path;

pub struct PathWriter<'a> {
    current: Path,           // contour under construction
    partials: Vec<Path>,     // contours with mismatched starts and ends
    end_pt_ts: Vec<PtT>,     // possible pt values for partial starts and ends
    path: &'a mut Path,      // closed contours are written here
    defer: [PtT; 2],         // [0] deferred move, [1] deferred line
    first_pt_t: PtT,         // first in current contour
}

impl<'a> PathWriter<'a> {
    pub fn new(path: &'a mut Path) -> Self {
        let mut w = PathWriter {
            current: Path::new(),
            partials: Vec::new(),
            end_pt_ts: Vec::new(),
            path,
            defer: [PtT::NULL; 2],
            first_pt_t: PtT::NULL,
        };
        w.init();
        w
    }

    pub fn native_path(&self) -> &Path {
        self.path
    }

    fn close(&mut self) {
        if self.current.is_empty() {
            return;
        }
        self.current.close();
        self.path.add_path(&self.current);
        self.current = Path::new();
        self.init();
    }

    pub fn conic_to(&mut self, pt1: Point, pt2: PtT, weight: f32) {
        let pt2pt = self.update(pt2);
        self.current.conic_to(pt1.x, pt1.y, pt2pt.x, pt2pt.y, weight);
    }

    pub fn cubic_to(&mut self, pt1: Point, pt2: Point, pt3: PtT) {
        let pt3pt = self.update(pt3);
        self.current.cubic_to(pt1.x, pt1.y, pt2.x, pt2.y, pt3pt.x, pt3pt.y);
    }

    pub fn deferred_line(&mut self, pt: PtT) -> bool {
        if self.defer[0] == pt {
            // FIXME: why we're adding a degenerate line? Caller should have preflighted this.
            return true;
        }
        if pt.contains_pt_t(self.defer[0]) {
            // FIXME: why we're adding a degenerate line?
            return true;
        }
        if self.matched_last(pt) {
            return false;
        }
        if self.defer[1].is_some() && self.changed_slopes(pt) {
            self.line_to();
            self.defer[0] = self.defer[1];
        }
        self.defer[1] = pt;
        true
    }

    pub fn deferred_move(&mut self, pt: PtT) {
        if self.defer[1].is_null() {
            self.first_pt_t = pt;
            self.defer[0] = pt;
            return;
        }
        if !self.matched_last(pt) {
            self.finish_contour();
            self.first_pt_t = pt;
            self.defer[0] = pt;
        }
    }

    pub fn finish_contour(&mut self) {
        if !self.matched_last(self.defer[0]) {
            if self.defer[1].is_null() {
                return;
            }
            self.line_to();
        }
        if self.current.is_empty() {
            return;
        }
        if self.is_closed() {
            self.close();
        } else {
            self.end_pt_ts.push(self.first_pt_t);
            self.end_pt_ts.push(self.defer[1]);
            self.partials.push(self.current.clone());
            self.init();
        }
    }

    pub fn has_move(&self) -> bool {
        self.first_pt_t.is_null()
    }

    pub fn init(&mut self) {
        self.current = Path::new();
        self.first_pt_t = PtT::NULL;
        self.defer = [PtT::NULL; 2];
    }

    pub fn is_closed(&self) -> bool {
        self.matched_last(self.first_pt_t)
    }

    fn line_to(&mut self) {
        if self.current.is_empty() {
            self.move_to();
        }
        let p = self.defer[1].pt();
        self.current.line_to(p.x, p.y);
    }

    fn matched_last(&self, test: PtT) -> bool {
        if test == self.defer[1] {
            return true;
        }
        if test.is_null() {
            return false;
        }
        if self.defer[1].is_null() {
            return false;
        }
        test.contains_pt_t(self.defer[1])
    }

    fn move_to(&mut self) {
        let p = self.first_pt_t.pt();
        self.current.move_to(p.x, p.y);
    }

    pub fn quad_to(&mut self, pt1: Point, pt2: PtT) {
        let pt2pt = self.update(pt2);
        self.current.quad_to(pt1.x, pt1.y, pt2pt.x, pt2pt.y);
    }

    /// If the last point to be written matches the current path's first
    /// point, alter the last to avoid writing a degenerate lineTo when the
    /// path is closed.
    fn update(&mut self, pt: PtT) -> Point {
        if self.defer[1].is_null() {
            self.move_to();
        } else if !self.matched_last(self.defer[0]) {
            self.line_to();
        }
        let mut result = pt.pt();
        if self.first_pt_t.is_some() && result != self.first_pt_t.pt() && self.first_pt_t.contains_pt_t(pt) {
            result = self.first_pt_t.pt();
        }
        self.defer = [pt, pt]; // set both to know that there is not a pending deferred line
        result
    }

    fn some_assembly_required(&mut self) -> bool {
        self.finish_contour();
        !self.end_pt_ts.is_empty()
    }

    fn changed_slopes(&self, pt_t: PtT) -> bool {
        if self.matched_last(self.defer[0]) {
            return false;
        }
        let d0 = self.defer[0].pt();
        let d1 = self.defer[1].pt();
        let p = pt_t.pt();
        let defer_dxdy = Point::new(d1.x - d0.x, d1.y - d0.y);
        let line_dxdy = Point::new(p.x - d1.x, p.y - d1.y);
        defer_dxdy.x * line_dxdy.y != defer_dxdy.y * line_dxdy.x
    }

    /// Check start and end of each contour; if not the same, record them,
    /// match them up, connect closest, reassemble contour pieces into a new path.
    pub fn assemble(&mut self) {
        if !self.some_assembly_required() {
            return;
        }
        let end_count = self.end_pt_ts.len(); // all starts and ends
        // lengthen any partial contour adjacent to a simple segment
        for p_index in 0..end_count {
            let mut op_pt_t = self.end_pt_ts[p_index];
            let mut p = Path::new();
            let part_partials;
            {
                let mut part_writer = PathWriter::new(&mut p);
                loop {
                    if !zero_or_one(op_pt_t.t()) {
                        break;
                    }
                    let op_span_base = op_pt_t.span();
                    let mut start = if op_pt_t.t() != 0.0 { op_span_base.prev() } else { op_span_base.up_cast().next() };
                    let mut step = if op_pt_t.t() != 0.0 { 1 } else { -1 };
                    let op_segment = op_span_base.segment();
                    let next_segment = op_segment.is_simple(&mut start, &mut step);
                    if next_segment.is_null() {
                        break;
                    }
                    let op_span_end = if start.t() != 0.0 { start.prev() } else { start.up_cast().next() };
                    if start.starter(op_span_end).already_added() {
                        break;
                    }
                    next_segment.add_curve_to(start, op_span_end, &mut part_writer);
                    op_pt_t = op_span_end.pt_t();
                    self.end_pt_ts[p_index] = op_pt_t;
                }
                part_writer.finish_contour();
                part_partials = std::mem::take(&mut part_writer.partials);
            }
            if part_partials.is_empty() {
                continue;
            }
            // if pIndex is even, reverse and prepend to fPartials; otherwise, append
            let part = &part_partials[0];
            if p_index & 1 != 0 {
                self.partials[p_index >> 1].add_path_extend(part);
            } else {
                let mut reverse = Path::new();
                reverse.reverse_add_path(part);
                reverse.add_path_extend(&self.partials[p_index >> 1]);
                self.partials[p_index >> 1] = reverse;
            }
        }
        let link_count = end_count / 2; // number of partial contours
        let mut s_link = vec![SK_MAX_S32; link_count];
        let mut e_link = vec![SK_MAX_S32; link_count];
        let entries = end_count * (end_count - 1) / 2; // folded triangle
        let mut distances: Vec<f64> = Vec::with_capacity(entries);
        let mut sorted_dist: Vec<i32> = Vec::with_capacity(entries);
        let mut dist_lookup: Vec<i32> = Vec::with_capacity(entries);
        let mut r_row = 0i32;
        let mut d_index = 0i32;
        for r_index in 0..end_count.saturating_sub(1) {
            let o_pt_t = self.end_pt_ts[r_index];
            for i_index in r_index + 1..end_count {
                let i_pt_t = self.end_pt_ts[i_index];
                let dx = (i_pt_t.pt().x - o_pt_t.pt().x) as f64;
                let dy = (i_pt_t.pt().y - o_pt_t.pt().y) as f64;
                let dist = dx * dx + dy * dy;
                dist_lookup.push(r_row + i_index as i32);
                distances.push(dist); // oStart distance from iStart
                sorted_dist.push(d_index);
                d_index += 1;
            }
            r_row += end_count as i32;
        }
        super::sort::tq_sort(&mut sorted_dist, |&a, &b| distances[a as usize] < distances[b as usize]);
        let mut remaining = link_count; // number of start/end pairs
        for &sd in sorted_dist.iter().take(entries) {
            let pair = dist_lookup[sd as usize];
            let row = pair / end_count as i32;
            let col = pair - row * end_count as i32;
            let ndx_one = (row >> 1) as usize;
            let end_one = row & 1 != 0;
            let link_one_val = if end_one { e_link[ndx_one] } else { s_link[ndx_one] };
            if link_one_val != SK_MAX_S32 {
                continue;
            }
            let ndx_two = (col >> 1) as usize;
            let end_two = col & 1 != 0;
            let link_two_val = if end_two { e_link[ndx_two] } else { s_link[ndx_two] };
            if link_two_val != SK_MAX_S32 {
                continue;
            }
            let flip = end_one == end_two;
            let v1 = if flip { !(ndx_two as i32) } else { ndx_two as i32 };
            let v2 = if flip { !(ndx_one as i32) } else { ndx_one as i32 };
            if end_one { e_link[ndx_one] = v1 } else { s_link[ndx_one] = v1 }
            if end_two { e_link[ndx_two] = v2 } else { s_link[ndx_two] = v2 }
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
        let mut r_index: i32 = 0;
        loop {
            let mut forward = true;
            let mut first = true;
            let s_index = s_link[r_index as usize];
            s_link[r_index as usize] = SK_MAX_S32;
            let mut e_index;
            if s_index < 0 {
                e_index = s_link[!s_index as usize];
                s_link[!s_index as usize] = SK_MAX_S32;
            } else {
                e_index = e_link[s_index as usize];
                e_link[s_index as usize] = SK_MAX_S32;
            }
            loop {
                let contour = &self.partials[r_index as usize];
                if !first && self.path.last_pt().is_none() {
                    return;
                }
                if forward {
                    if first {
                        self.path.add_path(contour);
                    } else {
                        self.path.add_path_extend(contour);
                    }
                } else {
                    self.path.reverse_path_to(contour);
                }
                if first {
                    first = false;
                }
                if s_index == (if (r_index != e_index) ^ forward { e_index } else { !e_index }) {
                    self.path.close();
                    break;
                }
                if forward {
                    e_index = e_link[r_index as usize];
                    e_link[r_index as usize] = SK_MAX_S32;
                    if e_index >= 0 {
                        s_link[e_index as usize] = SK_MAX_S32;
                    } else {
                        e_link[!e_index as usize] = SK_MAX_S32;
                    }
                } else {
                    e_index = s_link[r_index as usize];
                    s_link[r_index as usize] = SK_MAX_S32;
                    if e_index >= 0 {
                        e_link[e_index as usize] = SK_MAX_S32;
                    } else {
                        s_link[!e_index as usize] = SK_MAX_S32;
                    }
                }
                r_index = e_index;
                if r_index < 0 {
                    forward ^= true;
                    r_index = !r_index;
                }
            }
            r_index = 0;
            while (r_index as usize) < link_count {
                if s_link[r_index as usize] != SK_MAX_S32 {
                    break;
                }
                r_index += 1;
            }
            if (r_index as usize) >= link_count {
                break;
            }
        }
    }
}
