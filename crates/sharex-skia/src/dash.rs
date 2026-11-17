//! Dashing: `SkDashPathEffect` (`SkDashImpl`, `src/utils/SkDashPath.cpp`)
//! over `SkContourMeasure` (`src/core/SkContourMeasure.cpp`), which
//! measures and cuts lines, quads, conics and cubics as they are (a conic
//! is dashed into conic pieces, not into quads).

use crate::draw::{Cap, Join};
use crate::geom::{Conic, Point, Rect, chop_cubic_at, chop_quad_at};
use crate::path::{Convexity, Matrix, Path, Verb};

const MAX_T_VALUE: i32 = 0x3FFF_FFFF;

/// `SkDashPath::kMaxDashCount`.
const MAX_DASH_COUNT: f32 = 1_000_000.0;

fn t_value_to_scalar(t: i32) -> f32 {
    let recip = 1.0f32 / MAX_T_VALUE as f32;
    t as f32 * recip
}

/// `SkPoint::Length`.
fn length(dx: f32, dy: f32) -> f32 {
    let mag2 = dx * dx + dy * dy;
    if mag2.is_finite() {
        mag2.sqrt()
    } else {
        ((dx as f64) * (dx as f64) + (dy as f64) * (dy as f64)).sqrt() as f32
    }
}

/// `SkPoint::Distance`.
fn distance(a: Point, b: Point) -> f32 {
    length(a.x - b.x, a.y - b.y)
}

/// `SkScalarInterp`.
fn interp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// `SkMatrixPriv::ComputeResScaleForStroking`.
pub fn res_scale_for_stroking(m: &Matrix) -> f32 {
    let sx = length(m.sx, m.ky);
    let sy = length(m.kx, m.sy);
    if sx.is_finite() && sy.is_finite() {
        let scale = sx.max(sy);
        if scale > 0.0 {
            return scale;
        }
    }
    1.0
}

// --- SkConic pieces (SkGeometry.cpp) -----------------------------------------

/// `SkConicCoeff`: numerator and denominator quads.
struct ConicCoeff {
    na: Point,
    nb: Point,
    nc: Point,
    da: f32,
    db: f32,
    dc: f32,
}

impl ConicCoeff {
    fn new(c: &Conic) -> Self {
        let (p0, p1, p2) = (c.pts[0], c.pts[1], c.pts[2]);
        let p1w = p1 * c.w;
        let nc = p0;
        let na = p2 - (p1w + p1w) + p0;
        let d = p1w - p0;
        let nb = d + d;
        let dc = 1.0f32;
        let db = (c.w - dc) + (c.w - dc);
        let da = 0.0 - db;
        Self {
            na,
            nb,
            nc,
            da,
            db,
            dc,
        }
    }

    fn numer(&self, t: f32) -> Point {
        Point::new(
            (self.na.x * t + self.nb.x) * t + self.nc.x,
            (self.na.y * t + self.nb.y) * t + self.nc.y,
        )
    }

    fn denom(&self, t: f32) -> f32 {
        (self.da * t + self.db) * t + self.dc
    }

    fn eval(&self, t: f32) -> Point {
        let n = self.numer(t);
        let d = self.denom(t);
        Point::new(n.x / d, n.y / d)
    }
}

/// `SkConic::evalAt(t)`.
pub(crate) fn conic_eval_at(c: &Conic, t: f32) -> Point {
    ConicCoeff::new(c).eval(t)
}

/// `SkConic::chopAt(t, dst)`: false if it made a non-finite value.
fn conic_chop_at(c: &Conic, t: f32) -> Option<[Conic; 2]> {
    // ratquad_mapTo3D, then p3d_interp per coordinate.
    let w = c.w;
    let src = [
        [c.pts[0].x * 1.0, c.pts[0].y * 1.0, 1.0],
        [c.pts[1].x * w, c.pts[1].y * w, w],
        [c.pts[2].x * 1.0, c.pts[2].y * 1.0, 1.0],
    ];
    let mut tmp2 = [[0.0f32; 3]; 3];
    for k in 0..3 {
        let ab = interp(src[0][k], src[1][k], t);
        let bc = interp(src[1][k], src[2][k], t);
        tmp2[0][k] = ab;
        tmp2[1][k] = interp(ab, bc, t);
        tmp2[2][k] = bc;
    }
    let down = |p: [f32; 3]| Point::new(p[0] / p[2], p[1] / p[2]);
    let mid = down(tmp2[1]);
    let root = tmp2[1][2].sqrt();
    let d0 = Conic {
        pts: [c.pts[0], down(tmp2[0]), mid],
        w: tmp2[0][2] / root,
    };
    let d1 = Conic {
        pts: [mid, down(tmp2[2]), c.pts[2]],
        w: tmp2[2][2] / root,
    };
    let finite = d0.pts.iter().chain(d1.pts.iter()).all(|p| p.is_finite())
        && d0.w.is_finite()
        && d1.w.is_finite();
    if finite { Some([d0, d1]) } else { None }
}

/// `SkConic::chopAt(t1, t2, dst)`.
fn conic_chop_at2(c: &Conic, t1: f32, t2: f32) -> Conic {
    if t1 == 0.0 || t2 == 1.0 {
        if t1 == 0.0 && t2 == 1.0 {
            return *c;
        }
        if let Some(pair) = conic_chop_at(c, if t1 != 0.0 { t1 } else { t2 }) {
            return pair[(t1 != 0.0) as usize];
        }
    }
    let coeff = ConicCoeff::new(c);
    let a_xy = coeff.numer(t1);
    let a_zz = coeff.denom(t1);
    let mid = (t1 + t2) / 2.0;
    let d_xy = coeff.numer(mid);
    let d_zz = coeff.denom(mid);
    let c_xy = coeff.numer(t2);
    let c_zz = coeff.denom(t2);
    let b_xy = Point::new(
        (d_xy.x + d_xy.x) - (a_xy.x + c_xy.x) * 0.5,
        (d_xy.y + d_xy.y) - (a_xy.y + c_xy.y) * 0.5,
    );
    let b_zz = (d_zz + d_zz) - (a_zz + c_zz) * 0.5;
    Conic {
        pts: [
            Point::new(a_xy.x / a_zz, a_xy.y / a_zz),
            Point::new(b_xy.x / b_zz, b_xy.y / b_zz),
            Point::new(c_xy.x / c_zz, c_xy.y / c_zz),
        ],
        w: b_zz / (a_zz * c_zz).sqrt(),
    }
}

// --- SkContourMeasure ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SegType {
    Line,
    Quad,
    Cubic,
    Conic,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    distance: f32,
    pt_index: usize,
    t_value: i32,
    kind: SegType,
}

impl Segment {
    fn scalar_t(&self) -> f32 {
        t_value_to_scalar(self.t_value)
    }
}

/// One contour, measured (`SkContourMeasure`).
pub struct ContourMeasure {
    segments: Vec<Segment>,
    pts: Vec<Point>,
    length: f32,
    is_closed: bool,
}

/// `SkContourMeasure_segTo`: appends the piece of a segment between
/// `start_t` and `stop_t` to `dst`.
fn seg_to(pts: &[Point], kind: SegType, start_t: f32, stop_t: f32, dst: &mut Path) {
    if start_t == stop_t {
        if let Some(last) = dst.last_pt()
            && !dst.is_empty()
        {
            // a zero-length on segment: add a zero-length line, which the
            // stroker caps as appropriate
            dst.line_to(last.x, last.y);
        }
        return;
    }
    match kind {
        SegType::Line => {
            if stop_t == 1.0 {
                dst.line_to(pts[1].x, pts[1].y);
            } else {
                dst.line_to(
                    interp(pts[0].x, pts[1].x, stop_t),
                    interp(pts[0].y, pts[1].y, stop_t),
                );
            }
        }
        SegType::Quad => {
            let q = [pts[0], pts[1], pts[2]];
            if start_t == 0.0 {
                if stop_t == 1.0 {
                    dst.quad_to(pts[1].x, pts[1].y, pts[2].x, pts[2].y);
                } else {
                    let t0 = chop_quad_at(&q, stop_t);
                    dst.quad_to(t0[1].x, t0[1].y, t0[2].x, t0[2].y);
                }
            } else {
                let t0 = chop_quad_at(&q, start_t);
                if stop_t == 1.0 {
                    dst.quad_to(t0[3].x, t0[3].y, t0[4].x, t0[4].y);
                } else {
                    let t1 = chop_quad_at(
                        &[t0[2], t0[3], t0[4]],
                        (stop_t - start_t) / (1.0 - start_t),
                    );
                    dst.quad_to(t1[1].x, t1[1].y, t1[2].x, t1[2].y);
                }
            }
        }
        SegType::Conic => {
            let conic = Conic::new([pts[0], pts[2], pts[3]], pts[1].x);
            if start_t == 0.0 {
                if stop_t == 1.0 {
                    let p = conic.pts;
                    dst.conic_to(p[1].x, p[1].y, p[2].x, p[2].y, conic.w);
                } else if let Some(tmp) = conic_chop_at(&conic, stop_t) {
                    let p = tmp[0].pts;
                    dst.conic_to(p[1].x, p[1].y, p[2].x, p[2].y, tmp[0].w);
                }
            } else if stop_t == 1.0 {
                if let Some(tmp) = conic_chop_at(&conic, start_t) {
                    let p = tmp[1].pts;
                    dst.conic_to(p[1].x, p[1].y, p[2].x, p[2].y, tmp[1].w);
                }
            } else {
                let tmp = conic_chop_at2(&conic, start_t, stop_t);
                let p = tmp.pts;
                dst.conic_to(p[1].x, p[1].y, p[2].x, p[2].y, tmp.w);
            }
        }
        SegType::Cubic => {
            let mut t0 = [Point::default(); 7];
            if start_t == 0.0 {
                if stop_t == 1.0 {
                    dst.cubic_to(pts[1].x, pts[1].y, pts[2].x, pts[2].y, pts[3].x, pts[3].y);
                } else {
                    chop_cubic_at(pts, &mut t0, stop_t);
                    dst.cubic_to(t0[1].x, t0[1].y, t0[2].x, t0[2].y, t0[3].x, t0[3].y);
                }
            } else {
                chop_cubic_at(pts, &mut t0, start_t);
                if stop_t == 1.0 {
                    dst.cubic_to(t0[4].x, t0[4].y, t0[5].x, t0[5].y, t0[6].x, t0[6].y);
                } else {
                    let mut t1 = [Point::default(); 7];
                    chop_cubic_at(&t0[3..7], &mut t1, (stop_t - start_t) / (1.0 - start_t));
                    dst.cubic_to(t1[1].x, t1[1].y, t1[2].x, t1[2].y, t1[3].x, t1[3].y);
                }
            }
        }
    }
}

/// `compute_pos_tan`, position only.
fn compute_pos(pts: &[Point], kind: SegType, t: f32) -> Point {
    match kind {
        SegType::Line => Point::new(interp(pts[0].x, pts[1].x, t), interp(pts[0].y, pts[1].y, t)),
        SegType::Quad => {
            // SkQuadCoeff
            let c = pts[0];
            let b = (pts[1] - c) + (pts[1] - c);
            let a = pts[2] - (pts[1] + pts[1]) + c;
            Point::new((a.x * t + b.x) * t + c.x, (a.y * t + b.y) * t + c.y)
        }
        SegType::Conic => conic_eval_at(&Conic::new([pts[0], pts[2], pts[3]], pts[1].x), t),
        SegType::Cubic => {
            let (p0, p1, p2, p3) = (pts[0], pts[1], pts[2], pts[3]);
            let a = p3 + (p1 - p2) * 3.0 - p0;
            let b = (p2 - (p1 + p1) + p0) * 3.0;
            let c = (p1 - p0) * 3.0;
            Point::new(
                ((a.x * t + b.x) * t + c.x) * t + p0.x,
                ((a.y * t + b.y) * t + c.y) * t + p0.y,
            )
        }
    }
}

fn tspan_big_enough(tspan: i32) -> bool {
    (tspan >> 10) != 0
}

fn quad_too_curvy(pts: &[Point], tolerance: f32) -> bool {
    let dx = pts[1].x * 0.5 - ((pts[0].x + pts[2].x) * 0.5) * 0.5;
    let dy = pts[1].y * 0.5 - ((pts[0].y + pts[2].y) * 0.5) * 0.5;
    let dist = dx.abs().max(dy.abs());
    dist > tolerance
}

fn conic_too_curvy(first: Point, mid_t: Point, last: Point, tolerance: f32) -> bool {
    let mid_ends = (first + last) * 0.5;
    let dxy = mid_t - mid_ends;
    let dist = dxy.x.abs().max(dxy.y.abs());
    dist > tolerance
}

fn cheap_dist_exceeds_limit(pt: Point, x: f32, y: f32, tolerance: f32) -> bool {
    let dist = (x - pt.x).abs().max((y - pt.y).abs());
    dist > tolerance
}

fn cubic_too_curvy(pts: &[Point], tolerance: f32) -> bool {
    let third = 1.0f32 / 3.0;
    let two_thirds = 1.0f32 * 2.0 / 3.0;
    cheap_dist_exceeds_limit(
        pts[1],
        interp(pts[0].x, pts[3].x, third),
        interp(pts[0].y, pts[3].y, third),
        tolerance,
    ) || cheap_dist_exceeds_limit(
        pts[2],
        interp(pts[0].x, pts[3].x, two_thirds),
        interp(pts[0].y, pts[3].y, two_thirds),
        tolerance,
    )
}

/// `SkContourMeasureIter`: each contour of a path, measured.
pub struct ContourMeasureIter {
    verbs: Vec<(Verb, Vec<Point>, f32)>,
    index: usize,
    tolerance: f32,
    force_closed: bool,
}

impl ContourMeasureIter {
    pub fn new(path: &Path, force_closed: bool, res_scale: f32) -> Self {
        let verbs = if path.is_finite() {
            path.iter_verbs()
        } else {
            Vec::new()
        };
        Self {
            verbs,
            index: 0,
            // CHEAP_DIST_LIMIT * (1 / resScale)
            tolerance: 0.5 * (1.0f32 / res_scale),
            force_closed,
        }
    }

    /// `next()`: the next contour with a length.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<ContourMeasure> {
        while self.index < self.verbs.len() {
            if let Some(cm) = self.build_segments() {
                return Some(cm);
            }
        }
        None
    }

    fn compute_line_seg(
        segs: &mut Vec<Segment>,
        p0: Point,
        p1: Point,
        mut dist: f32,
        pt_index: usize,
    ) -> f32 {
        let d = distance(p0, p1);
        let prev = dist;
        dist += d;
        if dist > prev {
            segs.push(Segment {
                distance: dist,
                pt_index,
                t_value: MAX_T_VALUE,
                kind: SegType::Line,
            });
        }
        dist
    }

    fn compute_quad_segs(
        &self,
        segs: &mut Vec<Segment>,
        pts: &[Point],
        mut dist: f32,
        mint: i32,
        maxt: i32,
        pt_index: usize,
    ) -> f32 {
        if tspan_big_enough(maxt - mint) && quad_too_curvy(pts, self.tolerance) {
            let tmp = chop_quad_at(&[pts[0], pts[1], pts[2]], 0.5);
            let halft = (mint + maxt) >> 1;
            dist = self.compute_quad_segs(segs, &tmp[0..3], dist, mint, halft, pt_index);
            dist = self.compute_quad_segs(segs, &tmp[2..5], dist, halft, maxt, pt_index);
        } else {
            let d = distance(pts[0], pts[2]);
            let prev = dist;
            dist += d;
            if dist > prev {
                segs.push(Segment {
                    distance: dist,
                    pt_index,
                    t_value: maxt,
                    kind: SegType::Quad,
                });
            }
        }
        dist
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_conic_segs(
        &self,
        segs: &mut Vec<Segment>,
        conic: &Conic,
        mut dist: f32,
        mint: i32,
        min_pt: Point,
        maxt: i32,
        max_pt: Point,
        pt_index: usize,
    ) -> f32 {
        let halft = (mint + maxt) >> 1;
        let half_pt = conic_eval_at(conic, t_value_to_scalar(halft));
        if !half_pt.is_finite() {
            return dist;
        }
        if tspan_big_enough(maxt - mint) && conic_too_curvy(min_pt, half_pt, max_pt, self.tolerance)
        {
            dist = self.compute_conic_segs(segs, conic, dist, mint, min_pt, halft, half_pt, pt_index);
            dist = self.compute_conic_segs(segs, conic, dist, halft, half_pt, maxt, max_pt, pt_index);
        } else {
            let d = distance(min_pt, max_pt);
            let prev = dist;
            dist += d;
            if dist > prev {
                segs.push(Segment {
                    distance: dist,
                    pt_index,
                    t_value: maxt,
                    kind: SegType::Conic,
                });
            }
        }
        dist
    }

    fn compute_cubic_segs(
        &self,
        segs: &mut Vec<Segment>,
        pts: &[Point],
        mut dist: f32,
        mint: i32,
        maxt: i32,
        pt_index: usize,
    ) -> f32 {
        if tspan_big_enough(maxt - mint) && cubic_too_curvy(pts, self.tolerance) {
            let mut tmp = [Point::default(); 7];
            chop_cubic_at(pts, &mut tmp, 0.5);
            let halft = (mint + maxt) >> 1;
            dist = self.compute_cubic_segs(segs, &tmp[0..4], dist, mint, halft, pt_index);
            dist = self.compute_cubic_segs(segs, &tmp[3..7], dist, halft, maxt, pt_index);
        } else {
            let d = distance(pts[0], pts[3]);
            let prev = dist;
            dist += d;
            if dist > prev {
                segs.push(Segment {
                    distance: dist,
                    pt_index,
                    t_value: maxt,
                    kind: SegType::Cubic,
                });
            }
        }
        dist
    }

    fn build_segments(&mut self) -> Option<ContourMeasure> {
        let mut pt_index: isize = -1;
        let mut dist = 0.0f32;
        let mut have_seen_close = self.force_closed;
        let mut have_seen_move_to = false;
        let mut segs = Vec::new();
        let mut fpts: Vec<Point> = Vec::new();

        while self.index < self.verbs.len() {
            let (verb, pts, w) = self.verbs[self.index].clone();
            if have_seen_move_to && verb == Verb::Move {
                break;
            }
            match verb {
                Verb::Move => {
                    pt_index += 1;
                    fpts.push(pts[0]);
                    have_seen_move_to = true;
                }
                Verb::Line => {
                    let prev = dist;
                    dist = Self::compute_line_seg(&mut segs, pts[0], pts[1], dist, pt_index as usize);
                    if dist > prev {
                        fpts.push(pts[1]);
                        pt_index += 1;
                    }
                }
                Verb::Quad => {
                    let prev = dist;
                    dist = self.compute_quad_segs(&mut segs, &pts, dist, 0, MAX_T_VALUE, pt_index as usize);
                    if dist > prev {
                        fpts.extend_from_slice(&pts[1..3]);
                        pt_index += 2;
                    }
                }
                Verb::Conic => {
                    let conic = Conic::new([pts[0], pts[1], pts[2]], w);
                    let prev = dist;
                    dist = self.compute_conic_segs(
                        &mut segs,
                        &conic,
                        dist,
                        0,
                        conic.pts[0],
                        MAX_T_VALUE,
                        conic.pts[2],
                        pt_index as usize,
                    );
                    if dist > prev {
                        // the weight is stored in the next point, followed by the last 2 pts
                        fpts.push(Point::new(conic.w, 0.0));
                        fpts.extend_from_slice(&pts[1..3]);
                        pt_index += 3;
                    }
                }
                Verb::Cubic => {
                    let prev = dist;
                    dist = self.compute_cubic_segs(&mut segs, &pts, dist, 0, MAX_T_VALUE, pt_index as usize);
                    if dist > prev {
                        fpts.extend_from_slice(&pts[1..4]);
                        pt_index += 3;
                    }
                }
                Verb::Close => {
                    have_seen_close = true;
                }
            }
            self.index += 1;
        }

        if !dist.is_finite() || segs.is_empty() {
            return None;
        }
        if have_seen_close {
            let prev = dist;
            let first = fpts[0];
            let pi = pt_index as usize;
            dist = Self::compute_line_seg(&mut segs, fpts[pi], first, dist, pi);
            if dist > prev {
                fpts.push(first);
            }
        }
        Some(ContourMeasure {
            segments: segs,
            pts: fpts,
            length: dist,
            is_closed: have_seen_close,
        })
    }
}

impl ContourMeasure {
    pub fn length(&self) -> f32 {
        self.length
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// `distanceToSegment`: the segment's index and t.
    fn distance_to_segment(&self, distance: f32) -> (usize, f32) {
        // SkTKSearch
        let segs = &self.segments;
        let count = segs.len();
        let mut lo = 0usize;
        let mut hi = count - 1;
        while lo < hi {
            let mid = (hi + lo) >> 1;
            if segs[mid].distance < distance {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let mut index = hi as isize;
        if segs[hi].distance < distance {
            index = !(hi as isize + 1);
        } else if distance < segs[hi].distance {
            index = !(hi as isize);
        }
        // don't care if we hit an exact match or not
        let index = (index ^ (index >> 63)) as usize;
        let seg = &segs[index];
        let mut start_t = 0.0f32;
        let mut start_d = 0.0f32;
        if index > 0 {
            start_d = segs[index - 1].distance;
            if segs[index - 1].pt_index == seg.pt_index {
                start_t = segs[index - 1].scalar_t();
            }
        }
        let t = start_t
            + (seg.scalar_t() - start_t) * (distance - start_d) / (seg.distance - start_d);
        (index, t)
    }

    /// `getSegment`: appends the part between two distances to `dst`.
    pub fn get_segment(
        &self,
        mut start_d: f32,
        mut stop_d: f32,
        dst: &mut Path,
        start_with_move_to: bool,
    ) -> bool {
        let length = self.length;
        if start_d < 0.0 {
            start_d = 0.0;
        }
        if stop_d > length {
            stop_d = length;
        }
        if !(start_d <= stop_d) || self.segments.is_empty() {
            return false;
        }
        let (mut seg, mut start_t) = self.distance_to_segment(start_d);
        if !start_t.is_finite() {
            return false;
        }
        let (stop_seg, stop_t) = self.distance_to_segment(stop_d);
        if !stop_t.is_finite() {
            return false;
        }
        let s = self.segments[seg];
        if start_with_move_to {
            let p = compute_pos(&self.pts[s.pt_index..], s.kind, start_t);
            dst.move_to(p.x, p.y);
        }
        let stop = self.segments[stop_seg];
        if s.pt_index == stop.pt_index {
            seg_to(&self.pts[s.pt_index..], s.kind, start_t, stop_t, dst);
        } else {
            loop {
                let cur = self.segments[seg];
                seg_to(&self.pts[cur.pt_index..], cur.kind, start_t, 1.0, dst);
                // Segment::Next: the first segment with a different ptIndex
                let pi = cur.pt_index;
                while self.segments[seg].pt_index == pi {
                    seg += 1;
                }
                start_t = 0.0;
                if self.segments[seg].pt_index >= stop.pt_index {
                    break;
                }
            }
            let cur = self.segments[seg];
            seg_to(&self.pts[cur.pt_index..], cur.kind, 0.0, stop_t, dst);
        }
        true
    }
}

/// `SkPathMeasure`: a path's contours, one at a time.
struct PathMeasure {
    iter: ContourMeasureIter,
    contour: Option<ContourMeasure>,
}

impl PathMeasure {
    fn new(path: &Path, force_closed: bool, res_scale: f32) -> Self {
        let mut iter = ContourMeasureIter::new(path, force_closed, res_scale);
        let contour = iter.next();
        Self { iter, contour }
    }

    fn length(&self) -> f32 {
        self.contour.as_ref().map_or(0.0, |c| c.length())
    }

    fn is_closed(&self) -> bool {
        self.contour.as_ref().is_some_and(|c| c.is_closed())
    }

    fn get_segment(&self, start: f32, stop: f32, dst: &mut Path, move_to: bool) -> bool {
        self.contour
            .as_ref()
            .is_some_and(|c| c.get_segment(start, stop, dst, move_to))
    }

    fn next_contour(&mut self) -> bool {
        self.contour = self.iter.next();
        self.contour.is_some()
    }
}

// --- SkDashPath ---------------------------------------------------------------

/// What `SkStrokeRec` holds that dashing looks at.
#[derive(Clone, Copy, Debug)]
pub struct StrokeRec {
    /// 0 for a hairline.
    pub width: f32,
    pub cap: Cap,
    pub join: Join,
    pub miter: f32,
    pub res_scale: f32,
}

/// `SkDashImpl`: intervals with their dash parameters.
#[derive(Clone, Debug)]
pub struct DashEffect {
    intervals: Vec<f32>,
    phase: f32,
    initial_dash_length: f32,
    initial_dash_index: usize,
    interval_length: f32,
}

fn find_first_interval(intervals: &[f32], mut phase: f32) -> (f32, usize) {
    for (i, &gap) in intervals.iter().enumerate() {
        if phase > gap || (phase == gap && gap != 0.0) {
            phase -= gap;
        } else {
            return (gap - phase, i);
        }
    }
    (intervals[0], 0)
}

/// `sk_float_mod` (`fmodf`).
fn fmod(x: f32, y: f32) -> f32 {
    x % y
}

impl DashEffect {
    /// `SkDashPathEffect::Make`: `None` for intervals Skia rejects
    /// (`ValidDashPath`).
    pub fn new(intervals: &[f32], phase: f32) -> Option<Self> {
        let count = intervals.len();
        if count < 2 || !count.is_multiple_of(2) {
            return None;
        }
        let mut length = 0.0f32;
        for &v in intervals {
            if v < 0.0 {
                return None;
            }
            length += v;
        }
        if !(length > 0.0 && phase.is_finite() && length.is_finite()) {
            return None;
        }
        // CalcDashParameters
        let mut len = 0.0f32;
        for &v in intervals {
            len += v;
        }
        let mut phase = phase;
        if phase < 0.0 {
            phase = -phase;
            if phase > len {
                phase = fmod(phase, len);
            }
            phase = len - phase;
            if phase == len {
                phase = 0.0;
            }
        } else if phase >= len {
            phase = fmod(phase, len);
        }
        let (initial_dash_length, initial_dash_index) = find_first_interval(intervals, phase);
        Some(Self {
            intervals: intervals.to_vec(),
            phase,
            initial_dash_length,
            initial_dash_index,
            interval_length: len,
        })
    }

    /// `SkDashImpl::onFilterPath` (`SkDashPath::InternalFilter`): the dashed
    /// path, and whether it is now to be filled (a butt-capped line's dashes
    /// come out as quads, `SpecialLineRec`) rather than stroked. `None` when
    /// Skia gives up (too many dashes).
    pub fn filter_path(
        &self,
        src: &Path,
        rec: &StrokeRec,
        cull_rect: Option<Rect>,
    ) -> Option<(Path, bool)> {
        let intervals = &self.intervals;
        let count = intervals.len();
        let mut dash_count = 0.0f32;
        let mut seg_count = 0;
        let mut dst = Path::new();

        let mut cull_storage = Path::new();
        let mut src_ref = src;
        if cull_path(src, rec, cull_rect, self.interval_length, &mut cull_storage) {
            if src.is_rect().is_some()
                && src.verbs.last() == Some(&Verb::Close)
                && self.initial_dash_index.is_multiple_of(2)
            {
                let path_length = PathMeasure::new(src, false, rec.res_scale).length();
                let mut end_phase = fmod(path_length + self.phase, self.interval_length);
                let mut index = 0;
                while end_phase > intervals[index] {
                    end_phase -= intervals[index];
                    index += 1;
                    if index == count {
                        end_phase = 0.0;
                        break;
                    }
                }
                if index.is_multiple_of(2) == (end_phase > 0.0) {
                    let pts = &src.points;
                    let mid = pts[0];
                    let mut last = pts.len() - 1;
                    while mid == pts[last] {
                        last -= 1;
                    }
                    let mut next = 1;
                    while mid == pts[next] {
                        next += 1;
                    }
                    let tiny = 1.0f32 / 4096.0;
                    let v = (mid - pts[last]) * tiny;
                    let a = mid - v;
                    cull_storage.move_to(a.x, a.y);
                    cull_storage.line_to(mid.x, mid.y);
                    let v = (mid - pts[next]) * tiny;
                    let b = mid - v;
                    cull_storage.line_to(b.x, b.y);
                }
            }
            src_ref = &cull_storage;
        }

        let line_rec = SpecialLineRec::new(src_ref, rec);
        let mut meas = PathMeasure::new(src_ref, false, rec.res_scale);
        loop {
            let mut skip_first_segment = meas.is_closed();
            let mut added_segment = false;
            let length = meas.length();
            let mut index = self.initial_dash_index;

            dash_count += length * (count >> 1) as f32 / self.interval_length;
            if dash_count > MAX_DASH_COUNT {
                return None;
            }

            let mut distance = 0.0f64;
            let mut dlen = self.initial_dash_length as f64;
            while distance < length as f64 {
                added_segment = false;
                if index.is_multiple_of(2) && !skip_first_segment {
                    added_segment = true;
                    seg_count += 1;
                    if let Some(lr) = &line_rec {
                        lr.add_segment(distance as f32, (distance + dlen) as f32, &mut dst);
                    } else {
                        meas.get_segment(distance as f32, (distance + dlen) as f32, &mut dst, true);
                    }
                }
                distance += dlen;
                skip_first_segment = false;
                index += 1;
                if index == count {
                    index = 0;
                }
                dlen = intervals[index] as f64;
            }

            if meas.is_closed()
                && self.initial_dash_index.is_multiple_of(2)
                && self.initial_dash_length >= 0.0
            {
                meas.get_segment(0.0, self.initial_dash_length, &mut dst, !added_segment);
                seg_count += 1;
            }
            if !meas.next_contour() {
                break;
            }
        }
        if seg_count > 1 {
            dst.set_convexity(Some(Convexity::Concave));
        }
        Some((dst, line_rec.is_some()))
    }
}

fn outset_for_stroke(rect: &mut Rect, rec: &StrokeRec) {
    let mut radius = rec.width * 0.5;
    if radius == 0.0 {
        radius = 1.0; // hairlines
    }
    if rec.join == Join::Miter {
        radius *= rec.miter;
    }
    *rect = Rect::from_ltrb(
        rect.left - radius,
        rect.top - radius,
        rect.right + radius,
        rect.bottom + radius,
    );
}

fn adjust_zero_length_line(pts: &mut [Point; 2]) {
    pts[1].x += pts[1].x.max(1.001) * (1.0 / 4096.0);
}

fn clip_line(
    pts: &mut [Point; 2],
    bounds: &Rect,
    interval_length: f32,
    prior_phase: f32,
) -> bool {
    let dxy = pts[1] - pts[0];
    if dxy.x != 0.0 && dxy.y != 0.0 {
        return false;
    }
    let vertical = dxy.y != 0.0;
    let get = |p: &Point| if vertical { p.y } else { p.x };
    let mut min_xy = get(&pts[0]);
    let mut max_xy = get(&pts[1]);
    let swapped = max_xy < min_xy;
    if swapped {
        std::mem::swap(&mut min_xy, &mut max_xy);
    }
    let (left_top, right_bottom) = if vertical {
        (bounds.top, bounds.bottom)
    } else {
        (bounds.left, bounds.right)
    };
    if max_xy < left_top || min_xy > right_bottom {
        return false;
    }
    if min_xy < left_top {
        min_xy = left_top - fmod(left_top - min_xy, interval_length);
        if !swapped {
            min_xy -= prior_phase; // for rectangles, adjust by prior phase
        }
    }
    if max_xy > right_bottom {
        max_xy = right_bottom + fmod(max_xy - right_bottom, interval_length);
        if swapped {
            max_xy += prior_phase; // for rectangles, adjust by prior phase
        }
    }
    if swapped {
        std::mem::swap(&mut min_xy, &mut max_xy);
    }
    if vertical {
        pts[0].y = min_xy;
        pts[1].y = max_xy;
    } else {
        pts[0].x = min_xy;
        pts[1].x = max_xy;
    }
    if min_xy == max_xy {
        adjust_zero_length_line(pts);
    }
    true
}

/// `SkPath::isLine`.
fn is_line(path: &Path) -> Option<[Point; 2]> {
    if path.verbs.len() == 2 && path.verbs[1] == Verb::Line {
        Some([path.points[0], path.points[1]])
    } else {
        None
    }
}

fn cull_path(
    src: &Path,
    rec: &StrokeRec,
    cull_rect: Option<Rect>,
    interval_length: f32,
    dst: &mut Path,
) -> bool {
    let Some(cull) = cull_rect else {
        if let Some(mut pts) = is_line(src)
            && pts[0] == pts[1]
        {
            adjust_zero_length_line(&mut pts);
            dst.move_to(pts[0].x, pts[0].y);
            dst.line_to(pts[1].x, pts[1].y);
            return true;
        }
        return false;
    };
    let mut bounds = cull;
    outset_for_stroke(&mut bounds, rec);

    if let Some(mut pts) = is_line(src) {
        if clip_line(&mut pts, &bounds, interval_length, 0.0) {
            dst.move_to(pts[0].x, pts[0].y);
            dst.line_to(pts[1].x, pts[1].y);
            return true;
        }
        return false;
    }

    if src.is_rect().is_some() {
        // SkPath::Iter (no forced close): the lines after the first moveTo,
        // with the closing line a close adds when it isn't degenerate.
        let mut lines: Vec<[Point; 2]> = Vec::new();
        let verbs = src.iter_verbs();
        let mut first = Point::default();
        let mut last = Point::default();
        for (i, (verb, pts, _)) in verbs.iter().enumerate() {
            match verb {
                Verb::Move if i == 0 => {
                    first = pts[0];
                    last = pts[0];
                }
                Verb::Line => {
                    lines.push([pts[0], pts[1]]);
                    last = pts[1];
                }
                Verb::Close => {
                    if last != first {
                        lines.push([last, first]);
                    }
                    break;
                }
                _ => break,
            }
        }
        let mut accum = 0.0f64;
        for mut pts in lines {
            let v = pts[1] - pts[0];
            let prior = (accum % interval_length as f64) as f32;
            if clip_line(&mut pts, &bounds, interval_length, prior) {
                if dst.last_pt() != Some(pts[0]) || dst.is_empty() {
                    dst.move_to(pts[0].x, pts[0].y);
                }
                dst.line_to(pts[1].x, pts[1].y);
            }
            accum += (v.x + v.y).abs() as f64;
        }
        return !dst.is_empty();
    }
    false
}

/// `SpecialLineRec`: a butt-capped stroked line's dashes as quads.
struct SpecialLineRec {
    pts: [Point; 2],
    tangent: Point,
    normal: Point,
    path_length: f32,
}

impl SpecialLineRec {
    fn new(src: &Path, rec: &StrokeRec) -> Option<Self> {
        if rec.width == 0.0 {
            return None;
        }
        let pts = is_line(src)?;
        if rec.cap != Cap::Butt {
            return None;
        }
        let path_length = distance(pts[0], pts[1]);
        let tangent = pts[1] - pts[0];
        if tangent.is_zero() {
            return None;
        }
        let tangent = tangent * (1.0f32 / path_length);
        if !tangent.is_finite() {
            return None;
        }
        // SkPointPriv::RotateCCW
        let normal = Point::new(tangent.y, -tangent.x) * (rec.width * 0.5);
        Some(Self {
            pts,
            tangent,
            normal,
            path_length,
        })
    }

    fn add_segment(&self, d0: f32, mut d1: f32, path: &mut Path) {
        if d1 > self.path_length {
            d1 = self.path_length;
        }
        let x0 = self.pts[0].x + self.tangent.x * d0;
        let x1 = self.pts[0].x + self.tangent.x * d1;
        let y0 = self.pts[0].y + self.tangent.y * d0;
        let y1 = self.pts[0].y + self.tangent.y * d1;
        let n = self.normal;
        path.add_poly(
            &[
                Point::new(x0 + n.x, y0 + n.y),
                Point::new(x1 + n.x, y1 + n.y),
                Point::new(x1 - n.x, y1 - n.y),
                Point::new(x0 - n.x, y0 - n.y),
            ],
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_length_and_dashes() {
        let mut p = Path::new();
        p.move_to(0.0, 0.0).line_to(10.0, 0.0);
        let dash = DashEffect::new(&[2.0, 2.0], 0.0).unwrap();
        let rec = StrokeRec {
            width: 0.0,
            cap: Cap::Butt,
            join: Join::Miter,
            miter: 4.0,
            res_scale: 1.0,
        };
        let (d, fill) = dash.filter_path(&p, &rec, None).unwrap();
        assert!(!fill);
        // 0-2, 4-6, 8-10
        assert_eq!(d.verbs.iter().filter(|v| **v == Verb::Move).count(), 3);
        assert_eq!(d.last_pt(), Some(Point::new(10.0, 0.0)));
    }

    #[test]
    fn conic_dashes_stay_conics() {
        let mut p = Path::new();
        p.add_oval(Rect::from_ltrb(0.0, 0.0, 100.0, 60.0), crate::path::Direction::Cw);
        let dash = DashEffect::new(&[4.0, 4.0], 0.0).unwrap();
        let rec = StrokeRec {
            width: 4.0,
            cap: Cap::Butt,
            join: Join::Round,
            miter: 10.0,
            res_scale: 1.0,
        };
        let (d, fill) = dash.filter_path(&p, &rec, None).unwrap();
        assert!(!fill);
        assert!(d.verbs.contains(&Verb::Conic));
        assert!(!d.verbs.contains(&Verb::Quad));
    }
}
