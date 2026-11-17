//! Skia's supersampling scan converter (`SkScan_SAAPath.cpp`), used for
//! paths with many points for their size: the non-AA edge walker of
//! `SkScan_Path.cpp` over edges scaled up 4x (`SkEdge`), feeding a
//! `SuperBlitter` (a row of runs) or, for small paths, a
//! `MaskSuperBlitter` (one A8 mask).

use crate::aaa::{AEdge, EdgeType, sk_tq_sort};
use crate::blitter::Blitter;
use crate::fixed::*;
use crate::geom::{self, IRect, Point};
use crate::path::{Edge as PathEdge, FillType, LINE_SEGMENT_MASK, Path};

const SHIFT: i32 = 2;
const SCALE: i32 = 1 << SHIFT;
const MASK: i32 = SCALE - 1;
const NONE: usize = usize::MAX;

// --- SkEdge -----------------------------------------------------------------

/// `SkEdge_Compute_DY`.
fn compute_dy(top: i32, y0: FDot6) -> FDot6 {
    left_shift(top, 6) + 32 - y0
}

fn set_line(e: &mut AEdge, p0: Point, p1: Point, shift: i32) -> bool {
    let scale = (1 << (shift + 6)) as f32;
    let mut x0 = (p0.x * scale) as i32;
    let mut y0 = (p0.y * scale) as i32;
    let mut x1 = (p1.x * scale) as i32;
    let mut y1 = (p1.y * scale) as i32;
    let mut winding = 1;
    if y0 > y1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
        winding = -1;
    }
    let top = fdot6_round(y0);
    let bot = fdot6_round(y1);
    if top == bot {
        return false;
    }
    let slope = fdot6_div(x1 - x0, y1 - y0);
    let dy = compute_dy(top, y0);
    e.x = fdot6_to_fixed(x0.wrapping_add(fixed_mul(slope, dy)));
    e.dx = slope;
    e.first_y = top;
    e.last_y = bot - 1;
    e.edge_type = EdgeType::Line;
    e.curve_count = 0;
    e.winding = winding;
    e.curve_shift = 0;
    true
}

fn update_line(e: &mut AEdge, x0: Fixed, y0: Fixed, x1: Fixed, y1: Fixed) -> bool {
    let y0 = y0 >> 10;
    let y1 = y1 >> 10;
    let top = fdot6_round(y0);
    let bot = fdot6_round(y1);
    if top == bot {
        return false;
    }
    let x0 = x0 >> 10;
    let x1 = x1 >> 10;
    let slope = fdot6_div(x1 - x0, y1 - y0);
    let dy = compute_dy(top, y0);
    e.x = fdot6_to_fixed(x0.wrapping_add(fixed_mul(slope, dy)));
    e.dx = slope;
    e.first_y = top;
    e.last_y = bot - 1;
    true
}

fn update_quadratic(e: &mut AEdge) -> bool {
    let mut success;
    let mut count = e.curve_count as i32;
    let mut oldx = e.qx;
    let mut oldy = e.qy;
    let mut dx = e.qdx;
    let mut dy = e.qdy;
    let (mut newx, mut newy);
    let shift = e.curve_shift as i32;
    loop {
        count -= 1;
        if count > 0 {
            newx = oldx.wrapping_add(dx >> shift);
            dx = dx.wrapping_add(e.qddx);
            newy = oldy.wrapping_add(dy >> shift);
            dy = dy.wrapping_add(e.qddy);
        } else {
            newx = e.qlast_x;
            newy = e.qlast_y;
        }
        success = update_line(e, oldx, oldy, newx, newy);
        oldx = newx;
        oldy = newy;
        if !(count > 0 && !success) {
            break;
        }
    }
    e.qx = newx;
    e.qy = newy;
    e.qdx = dx;
    e.qdy = dy;
    e.curve_count = count as i8;
    success
}

fn update_cubic(e: &mut AEdge) -> bool {
    let mut success;
    let mut count = e.curve_count as i32;
    let mut oldx = e.cx;
    let mut oldy = e.cy;
    let (mut newx, mut newy);
    let ddshift = e.curve_shift as i32;
    let dshift = e.cubic_dshift as i32;
    loop {
        count += 1;
        if count < 0 {
            newx = oldx.wrapping_add(e.cdx >> dshift);
            e.cdx = e.cdx.wrapping_add(e.cddx >> ddshift);
            e.cddx = e.cddx.wrapping_add(e.cdddx);
            newy = oldy.wrapping_add(e.cdy >> dshift);
            e.cdy = e.cdy.wrapping_add(e.cddy >> ddshift);
            e.cddy = e.cddy.wrapping_add(e.cdddy);
        } else {
            newx = e.clast_x;
            newy = e.clast_y;
        }
        if newy < oldy {
            newy = oldy;
        }
        success = update_line(e, oldx, oldy, newx, newy);
        oldx = newx;
        oldy = newy;
        if !(count < 0 && !success) {
            break;
        }
    }
    e.cx = newx;
    e.cy = newy;
    e.curve_count = count as i8;
    success
}

fn set_quadratic(e: &mut AEdge, pts: &[Point; 3], shift: i32) -> bool {
    if !e.set_quadratic_without_update(pts, shift) {
        return false;
    }
    e.curve_shift = e.q_curve_shift;
    update_quadratic(e)
}

fn set_cubic(e: &mut AEdge, pts: &[Point; 4], shift: i32) -> bool {
    if !e.set_cubic_without_update(pts, shift, true) {
        return false;
    }
    e.curve_shift = e.c_curve_shift;
    e.cubic_dshift = e.c_cubic_dshift;
    update_cubic(e)
}

// --- SkBasicEdgeBuilder -----------------------------------------------------

enum Combine {
    No,
    Partial,
    Total,
}

fn combine_vertical(edge: &AEdge, last: &mut AEdge) -> Combine {
    if last.edge_type != EdgeType::Line || last.dx != 0 || edge.x != last.x {
        return Combine::No;
    }
    if edge.winding == last.winding {
        if edge.last_y + 1 == last.first_y {
            last.first_y = edge.first_y;
            return Combine::Partial;
        }
        if edge.first_y == last.last_y + 1 {
            last.last_y = edge.last_y;
            return Combine::Partial;
        }
        return Combine::No;
    }
    if edge.first_y == last.first_y {
        if edge.last_y == last.last_y {
            return Combine::Total;
        }
        if edge.last_y < last.last_y {
            last.first_y = edge.last_y + 1;
            return Combine::Partial;
        }
        last.first_y = last.last_y + 1;
        last.last_y = edge.last_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    if edge.last_y == last.last_y {
        if edge.first_y > last.first_y {
            last.last_y = edge.first_y - 1;
            return Combine::Partial;
        }
        last.last_y = last.first_y - 1;
        last.first_y = edge.first_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    Combine::No
}

fn add_line(edges: &mut Vec<AEdge>, pts: &[Point; 2]) {
    let mut e = AEdge::default();
    if set_line(&mut e, pts[0], pts[1], SHIFT) {
        let is_vertical = e.dx == 0 && e.edge_type == EdgeType::Line;
        let combine = if is_vertical && let Some(last) = edges.last_mut() {
            combine_vertical(&e, last)
        } else {
            Combine::No
        };
        match combine {
            Combine::Total => {
                edges.pop();
            }
            Combine::Partial => {}
            Combine::No => edges.push(e),
        }
    }
}

fn add_quad(edges: &mut Vec<AEdge>, pts: &[Point; 3]) {
    let mut e = AEdge::default();
    if set_quadratic(&mut e, pts, SHIFT) {
        edges.push(e);
    }
}

fn add_cubic(edges: &mut Vec<AEdge>, pts: &[Point; 4]) {
    let mut e = AEdge::default();
    if set_cubic(&mut e, pts, SHIFT) {
        edges.push(e);
    }
}

fn build_edges(path: &Path, clip: Option<crate::geom::Rect>) -> Vec<AEdge> {
    let mut edges = Vec::new();
    let can_cull_to_the_right = !path.is_convex();
    let line_only = path.segment_mask() == LINE_SEGMENT_MASK;
    if let Some(clip) = clip {
        if line_only {
            for e in path.edges() {
                if let PathEdge::Line(p) = e {
                    let lines = crate::clip::clip_line(&p, &clip, can_cull_to_the_right);
                    for w in lines.windows(2) {
                        add_line(&mut edges, &[w[0], w[1]]);
                    }
                }
            }
        } else {
            let mut clipper = crate::clip::EdgeClipper {
                can_cull_to_the_right,
                out: Vec::new(),
            };
            for e in path.edges() {
                match e {
                    PathEdge::Line(p) => clipper.clip_line(p[0], p[1], &clip),
                    PathEdge::Quad(p) => clipper.clip_quad(&p, &clip),
                    PathEdge::Conic(p, w) => {
                        let q = geom::Conic::new(p, w).to_quads(0.25);
                        for i in 0..(q.len() - 1) / 2 {
                            clipper.clip_quad(&[q[2 * i], q[2 * i + 1], q[2 * i + 2]], &clip);
                        }
                    }
                    PathEdge::Cubic(p) => clipper.clip_cubic(&p, &clip),
                }
            }
            for seg in clipper.out {
                match seg {
                    crate::clip::Seg::Line(p) => add_line(&mut edges, &p),
                    crate::clip::Seg::Quad(p) => add_quad(&mut edges, &p),
                    crate::clip::Seg::Cubic(p) => add_cubic(&mut edges, &p),
                }
            }
        }
        return edges;
    }
    let handle_quad = |edges: &mut Vec<AEdge>, pts: &[Point; 3]| {
        let mut mono = [Point::default(); 5];
        let n = geom::chop_quad_at_y_extrema(pts, &mut mono);
        for i in 0..=n {
            add_quad(edges, &[mono[i * 2], mono[i * 2 + 1], mono[i * 2 + 2]]);
        }
    };
    for e in path.edges() {
        match e {
            PathEdge::Line(p) => add_line(&mut edges, &p),
            PathEdge::Quad(p) if !line_only => handle_quad(&mut edges, &p),
            PathEdge::Conic(p, w) if !line_only => {
                let q = geom::Conic::new(p, w).to_quads(0.25);
                for i in 0..(q.len() - 1) / 2 {
                    handle_quad(&mut edges, &[q[2 * i], q[2 * i + 1], q[2 * i + 2]]);
                }
            }
            PathEdge::Cubic(p) if !line_only => {
                let mut mono = [Point::default(); 10];
                let n = geom::chop_cubic_at_y_extrema(&p, &mut mono);
                for i in 0..=n {
                    add_cubic(
                        &mut edges,
                        &[
                            mono[i * 3],
                            mono[i * 3 + 1],
                            mono[i * 3 + 2],
                            mono[i * 3 + 3],
                        ],
                    );
                }
            }
            _ => {}
        }
    }
    edges
}

// --- the super blitters -----------------------------------------------------

fn coverage_to_partial_alpha(aa: i32) -> i32 {
    aa << (8 - 2 * SHIFT)
}

fn coverage_to_exact_alpha(aa: i32) -> i32 {
    let alpha = (256 >> SHIFT) * aa;
    alpha - (alpha >> 8)
}

enum Super<'a> {
    /// `SuperBlitter`: a row of alphas (standing in for its SkAlphaRuns).
    Rle {
        real: &'a mut dyn Blitter,
        curr_iy: i32,
        curr_y: i32,
        width: i32,
        left: i32,
        super_left: i32,
        top: i32,
        row: Vec<u8>,
        dirty: bool,
    },
    /// `MaskSuperBlitter`.
    Mask {
        real: &'a mut dyn Blitter,
        mask: Vec<u8>,
        bounds: IRect,
        row_bytes: i32,
        clip_rect: IRect,
    },
}

impl Super<'_> {
    fn flush(&mut self) {
        if let Super::Rle {
            real,
            curr_iy,
            top,
            row,
            dirty,
            left,
            ..
        } = self
            && *curr_iy >= *top
        {
            if *dirty {
                let mut runs = vec![1i16; row.len() + 1];
                runs[row.len()] = 0;
                real.blit_anti_h(*left, *curr_iy, row, &runs);
                row.fill(0);
                *dirty = false;
            }
            *curr_iy = *top - 1;
        }
    }

    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        match self {
            Super::Rle { .. } => self.rle_blit_h(x, y, width),
            Super::Mask {
                mask,
                bounds,
                row_bytes,
                ..
            } => {
                let iy = (y >> SHIFT) - bounds.top;
                if iy < 0 {
                    return;
                }
                let mut x = x - left_shift(bounds.left, SHIFT);
                let mut width = width;
                if x < 0 {
                    width += x;
                    x = 0;
                }
                let base = (iy * *row_bytes + (x >> SHIFT)) as usize;
                let start = x;
                let stop = x + width;
                let fb = start & MASK;
                let fe = stop & MASK;
                let n = (stop >> SHIFT) - (start >> SHIFT) - 1;
                let saturated_add = |p: &mut u8, add: i32| {
                    let tmp = *p as i32 + add;
                    *p = (tmp - (tmp >> 8)) as u8;
                };
                if n < 0 {
                    saturated_add(&mut mask[base], coverage_to_partial_alpha(fe - fb));
                } else {
                    let fb = SCALE - fb;
                    let max_value = (1 << (8 - SHIFT)) - (((y & MASK) + 1) >> SHIFT);
                    saturated_add(&mut mask[base], coverage_to_partial_alpha(fb));
                    for i in 0..n as usize {
                        let p = &mut mask[base + 1 + i];
                        *p = p.wrapping_add(max_value as u8);
                    }
                    saturated_add(
                        &mut mask[base + 1 + n as usize],
                        coverage_to_partial_alpha(fe),
                    );
                }
            }
        }
    }

    fn rle_blit_h(&mut self, x: i32, y: i32, width: i32) {
        let iy = y >> SHIFT;
        let (mut x, mut width) = (x, width);
        if let Super::Rle {
            super_left, curr_y, ..
        } = self
        {
            x -= *super_left;
            if x < 0 {
                width += x;
                x = 0;
            }
            if *curr_y != y {
                *curr_y = y;
            }
        }
        let new_line = matches!(self, Super::Rle { curr_iy, .. } if *curr_iy != iy);
        if new_line {
            self.flush();
            if let Super::Rle { curr_iy, .. } = self {
                *curr_iy = iy;
            }
        }
        let Super::Rle {
            row,
            dirty,
            width: w,
            ..
        } = self
        else {
            return;
        };
        let start = x;
        let stop = x + width;
        let mut fb = start & MASK;
        let mut fe = stop & MASK;
        let mut n = (stop >> SHIFT) - (start >> SHIFT) - 1;
        if n < 0 {
            fb = fe - fb;
            n = 0;
            fe = 0;
        } else if fb == 0 {
            n += 1;
        } else {
            fb = SCALE - fb;
        }
        let max_value = (1 << (8 - SHIFT)) - (((y & MASK) + 1) >> SHIFT);
        // SkAlphaRuns::add, pixel by pixel
        let mut ix = (x >> SHIFT) as usize;
        let (start_alpha, stop_alpha) =
            (coverage_to_partial_alpha(fb), coverage_to_partial_alpha(fe));
        let cap = *w as usize;
        if start_alpha != 0 && ix < cap {
            let tmp = row[ix] as i32 + start_alpha;
            row[ix] = (tmp - (tmp >> 8)) as u8;
            ix += 1;
        } else if start_alpha != 0 {
            ix += 1;
        }
        for _ in 0..n {
            if ix < cap {
                let v = row[ix] as i32 + max_value;
                row[ix] = (v - (v >> 8)) as u8;
            }
            ix += 1;
        }
        if stop_alpha != 0 && ix < cap {
            row[ix] = row[ix].wrapping_add(stop_alpha as u8);
        }
        *dirty = true;
    }

    fn blit_rect(&mut self, x: i32, mut y: i32, mut width: i32, mut height: i32) {
        if matches!(self, Super::Mask { .. }) {
            // MaskSuperBlitter has no blitRect: SkBlitter's, row by row.
            for i in 0..height {
                self.blit_h(x, y + i, width);
            }
            return;
        }
        while (y & MASK) != 0 {
            self.blit_h(x, y, width);
            y += 1;
            height -= 1;
            if height <= 0 {
                return;
            }
        }
        let start_y = y >> SHIFT;
        let stop_y = (y + height) >> SHIFT;
        let count = stop_y - start_y;
        let mut x = x;
        if count > 0 {
            y += count << SHIFT;
            height -= count << SHIFT;
            let orig_x = x;
            let (super_left, left) = match self {
                Super::Rle {
                    super_left, left, ..
                } => (*super_left, *left),
                _ => unreachable!(),
            };
            x -= super_left;
            if x < 0 {
                width += x;
                x = 0;
            }
            let ileft = x >> SHIFT;
            let mut xleft = x & MASK;
            let mut irite = (x + width) >> SHIFT;
            let mut xrite = (x + width) & MASK;
            if xrite == 0 {
                xrite = SCALE;
                irite -= 1;
            }
            self.flush();
            let n = irite - ileft - 1;
            if let Super::Rle {
                real,
                curr_iy,
                curr_y,
                row,
                dirty,
                ..
            } = self
            {
                if n < 0 {
                    xleft = xrite - xleft;
                    real.blit_v(
                        ileft + left,
                        start_y,
                        count,
                        coverage_to_exact_alpha(xleft) as u8,
                    );
                } else {
                    xleft = SCALE - xleft;
                    let l = coverage_to_exact_alpha(xleft) as u8;
                    let r = coverage_to_exact_alpha(xrite) as u8;
                    real.blit_anti_rect(ileft + left, start_y, n, count, l, r);
                }
                *curr_iy = stop_y - 1;
                *curr_y = y - 1;
                row.fill(0);
                *dirty = false;
            }
            x = orig_x;
        }
        while height > 0 {
            self.blit_h(x, y, width);
            y += 1;
            height -= 1;
        }
    }

    fn finish(mut self) {
        match &mut self {
            Super::Rle { .. } => self.flush(),
            Super::Mask {
                real,
                mask,
                bounds,
                row_bytes,
                clip_rect,
            } => {
                if !clip_rect.is_empty() {
                    real.blit_mask(mask, *bounds, *row_bytes, *clip_rect);
                }
            }
        }
    }
}

// --- the walkers (SkScan_Path.cpp) -------------------------------------------

struct Edges {
    e: Vec<AEdge>,
}

impl Edges {
    fn remove(&mut self, edge: usize) {
        let (p, n) = (self.e[edge].prev, self.e[edge].next);
        self.e[p].next = n;
        self.e[n].prev = p;
    }

    fn insert_after(&mut self, edge: usize, after: usize) {
        let an = self.e[after].next;
        self.e[edge].prev = after;
        self.e[edge].next = an;
        self.e[an].prev = edge;
        self.e[after].next = edge;
    }

    fn backward_insert_edge_based_on_x(&mut self, edge: usize) {
        let x = self.e[edge].x;
        let mut prev = self.e[edge].prev;
        while self.e[prev].prev != NONE && self.e[prev].x > x {
            prev = self.e[prev].prev;
        }
        if self.e[prev].next != edge {
            self.remove(edge);
            self.insert_after(edge, prev);
        }
    }

    fn insert_new_edges(&mut self, mut new_edge: usize, curr_y: i32) {
        if self.e[new_edge].first_y != curr_y {
            return;
        }
        let prev = self.e[new_edge].prev;
        if self.e[prev].x <= self.e[new_edge].x {
            return;
        }
        let mut start = prev;
        while self.e[start].prev != NONE && self.e[start].x > self.e[new_edge].x {
            start = self.e[start].prev;
        }
        loop {
            let next = self.e[new_edge].next;
            let mut skip = false;
            loop {
                if self.e[start].next == new_edge {
                    skip = true;
                    break;
                }
                let after = self.e[start].next;
                if self.e[after].x >= self.e[new_edge].x {
                    break;
                }
                start = after;
            }
            if !skip {
                self.remove(new_edge);
                self.insert_after(new_edge, start);
            }
            start = new_edge;
            new_edge = next;
            if self.e[new_edge].first_y != curr_y {
                break;
            }
        }
    }

    fn update_curve(&mut self, i: usize) -> bool {
        let e = &mut self.e[i];
        if e.curve_count > 0 {
            update_quadratic(e)
        } else if e.curve_count < 0 {
            update_cubic(e)
        } else {
            false
        }
    }
}

fn walk_edges(
    ed: &mut Edges,
    head: usize,
    fill: FillType,
    blitter: &mut Super,
    start_y: i32,
    stop_y: i32,
    right_clip: i32,
) {
    let mut curr_y = start_y;
    let winding_mask = if fill == FillType::EvenOdd { 1 } else { -1 };
    loop {
        let mut w: i32 = 0;
        let mut left = 0;
        let mut curr_e = ed.e[head].next;
        let mut prev_x = ed.e[head].x;
        while ed.e[curr_e].first_y <= curr_y {
            let x = fixed_round_to_int(ed.e[curr_e].x);
            if (w & winding_mask) == 0 {
                left = x;
            }
            w += ed.e[curr_e].winding as i32;
            if (w & winding_mask) == 0 {
                let width = x - left;
                if width > 0 {
                    blitter.blit_h(left, curr_y, width);
                }
            }
            let next = ed.e[curr_e].next;
            let new_x;
            let mut keep = true;
            if ed.e[curr_e].last_y == curr_y {
                if ed.e[curr_e].curve_count != 0 && ed.update_curve(curr_e) {
                    new_x = ed.e[curr_e].x;
                } else {
                    ed.remove(curr_e);
                    keep = false;
                    new_x = 0;
                }
            } else {
                new_x = ed.e[curr_e].x.wrapping_add(ed.e[curr_e].dx);
                ed.e[curr_e].x = new_x;
            }
            if keep {
                if new_x < prev_x {
                    ed.backward_insert_edge_based_on_x(curr_e);
                } else {
                    prev_x = new_x;
                }
            }
            curr_e = next;
        }
        if (w & winding_mask) != 0 {
            let width = right_clip - left;
            if width > 0 {
                blitter.blit_h(left, curr_y, width);
            }
        }
        curr_y += 1;
        if curr_y >= stop_y {
            break;
        }
        ed.insert_new_edges(curr_e, curr_y);
    }
}

fn walk_simple_edges(ed: &mut Edges, head: usize, blitter: &mut Super, start_y: i32, stop_y: i32) {
    let mut left_e = ed.e[head].next;
    let mut rite_e = ed.e[left_e].next;
    let mut curr_e = ed.e[rite_e].next;
    let mut local_top = ed.e[left_e].first_y.max(ed.e[rite_e].first_y);
    if local_top < start_y {
        return;
    }
    while local_top < stop_y {
        let mut local_bot = ed.e[left_e].last_y.min(ed.e[rite_e].last_y);
        local_bot = local_bot.min(stop_y - 1);
        if local_top > local_bot {
            return;
        }
        let mut left = ed.e[left_e].x;
        let d_left = ed.e[left_e].dx;
        let mut rite = ed.e[rite_e].x;
        let d_rite = ed.e[rite_e].dx;
        let mut count = local_bot - local_top;
        if 0 == (d_left | d_rite) {
            let mut l = fixed_round_to_int(left);
            let mut r = fixed_round_to_int(rite);
            if l > r {
                std::mem::swap(&mut l, &mut r);
            }
            if l < r {
                count += 1;
                blitter.blit_rect(l, local_top, r - l, count);
            }
            local_top = local_bot + 1;
        } else {
            loop {
                let mut l = fixed_round_to_int(left);
                let mut r = fixed_round_to_int(rite);
                if l > r {
                    std::mem::swap(&mut l, &mut r);
                }
                if l < r {
                    blitter.blit_h(l, local_top, r - l);
                }
                left = left.wrapping_add(d_left);
                rite = rite.wrapping_add(d_rite);
                local_top += 1;
                count -= 1;
                if count < 0 {
                    break;
                }
            }
        }
        ed.e[left_e].x = left;
        ed.e[rite_e].x = rite;
        let update_edge = |ed: &mut Edges, i: usize, last_y: i32| -> bool {
            if last_y == ed.e[i].last_y {
                return ed.e[i].curve_count != 0 && ed.update_curve(i);
            }
            true
        };
        if !update_edge(ed, left_e, local_bot) {
            if ed.e[curr_e].first_y >= stop_y {
                return;
            }
            left_e = curr_e;
            curr_e = ed.e[curr_e].next;
        }
        if !update_edge(ed, rite_e, local_bot) {
            if ed.e[curr_e].first_y >= stop_y {
                return;
            }
            rite_e = curr_e;
            curr_e = ed.e[curr_e].next;
        }
    }
}

/// `sk_fill_path` with edges shifted up by `SHIFT`.
fn fill_path(
    path: &Path,
    clip_rect: IRect,
    blitter: &mut Super,
    start_y: i32,
    stop_y: i32,
    contained: bool,
) {
    let shifted = IRect::from_ltrb(
        left_shift(clip_rect.left, SHIFT),
        left_shift(clip_rect.top, SHIFT),
        left_shift(clip_rect.right, SHIFT),
        left_shift(clip_rect.bottom, SHIFT),
    );
    let mut edges = build_edges(
        path,
        if contained {
            None
        } else {
            Some(clip_rect.to_rect())
        },
    );
    let count = edges.len();
    if count == 0 {
        return;
    }
    let mut list: Vec<usize> = (0..count).collect();
    {
        let e = &edges;
        sk_tq_sort(&mut list, &|a, b| {
            let (ea, eb) = (&e[a], &e[b]);
            if ea.first_y != eb.first_y {
                ea.first_y < eb.first_y
            } else {
                ea.x < eb.x
            }
        });
    }
    let head = edges.len();
    let tail = head + 1;
    edges.push(AEdge::default());
    edges.push(AEdge::default());
    for i in 1..count {
        edges[list[i - 1]].next = list[i];
        edges[list[i]].prev = list[i - 1];
    }
    let (first, last) = (list[0], list[count - 1]);
    edges[head].prev = NONE;
    edges[head].next = first;
    edges[head].first_y = MIN_S32;
    edges[head].x = MIN_S32;
    edges[first].prev = head;
    edges[tail].prev = last;
    edges[tail].next = NONE;
    edges[tail].first_y = MAX_S32;
    edges[last].next = tail;

    let mut start_y = left_shift(start_y, SHIFT);
    let mut stop_y = left_shift(stop_y, SHIFT);
    if !contained && start_y < shifted.top {
        start_y = shifted.top;
    }
    if !contained && stop_y > shifted.bottom {
        stop_y = shifted.bottom;
    }
    let mut ed = Edges { e: edges };
    if path.is_convex() && count >= 2 {
        walk_simple_edges(&mut ed, head, blitter, start_y, stop_y);
    } else {
        walk_edges(
            &mut ed,
            head,
            path.fill_type,
            blitter,
            start_y,
            stop_y,
            shifted.right,
        );
    }
}

/// `SkScan::SAAFillPath`.
pub fn saa_fill_path(path: &Path, blitter: &mut dyn Blitter, ir: IRect, clip_bounds: IRect) {
    let contained = clip_bounds.contains(&ir);
    let width = ir.width();
    let rb = ((width + 3) & !3) as i64;
    let can_mask = width <= 32 && rb * ir.height() as i64 <= 1024;
    let mut sect = ir;
    if !sect.intersect(&clip_bounds) {
        sect = IRect::default();
    }
    let mut sup = if can_mask {
        let row_bytes = ir.width();
        Super::Mask {
            real: blitter,
            mask: vec![0u8; (ir.height() * row_bytes + 1) as usize],
            bounds: ir,
            row_bytes,
            clip_rect: sect,
        }
    } else {
        Super::Rle {
            real: blitter,
            curr_iy: sect.top - 1,
            curr_y: left_shift(sect.top, SHIFT) - 1,
            width: sect.width(),
            left: sect.left,
            super_left: left_shift(sect.left, SHIFT),
            top: sect.top,
            row: vec![0u8; sect.width().max(0) as usize],
            dirty: false,
        }
    };
    fill_path(path, clip_bounds, &mut sup, ir.top, ir.bottom, contained);
    sup.finish();
}
