//! Hairlines: strokes no wider than a device pixel, which Skia draws as
//! one-pixel lines instead of filling a stroke outline
//! (`SkDrawTreatAsHairline` in `SkDrawBase::drawPath`). `hair_path` and the
//! curve subdivision are `SkScan_Hairline.cpp`; the anti-aliased line
//! scanner is `SkScan_Antihair.cpp` (`SkScan::AntiHairLineRgn`,
//! `do_anti_hairline` and its four hair blitters). Clips are rectangles
//! (a BW `SkRasterClip` that is a rect).

use crate::blitter::Blitter;
use crate::clip::intersect_line;
use crate::draw::Cap;
use crate::fixed::{
    FDot6, FIXED_HALF, FIXED1, Fixed, fdot6_to_fixed, fixed_ceil_to_int, fixed_floor_to_int,
    left_shift, scalar_to_fdot6,
};
use crate::geom::{Conic, IRect, Point, Rect, chop_cubic_at_ts, find_unit_quad_roots};
use crate::path::{Matrix, Path, Verb};

/// `SkDrawTreatAsHairline` for an anti-aliased stroke of `width` under
/// `ctm` (`SkDrawTreatAAStrokeAsHairline`): the coverage the hairline
/// stands for, or `None` when the stroke is filled as an outline.
pub fn hairline_coverage(width: f32, ctm: &Matrix) -> Option<f32> {
    if width == 0.0 {
        return Some(1.0);
    }
    // fast_len of the mapped (width, 0) and (0, width) vectors.
    let fast_len = |x: f32, y: f32| {
        let (mut x, mut y) = (x.abs(), y.abs());
        if x < y {
            std::mem::swap(&mut x, &mut y);
        }
        x + y * 0.5
    };
    let len0 = fast_len(width * ctm.sx, width * ctm.ky);
    let len1 = fast_len(width * ctm.kx, width * ctm.sy);
    if len0 <= 1.0 && len1 <= 1.0 {
        Some((len0 + len1) * 0.5)
    } else {
        None
    }
}

/// The paint alpha `SkDrawBase::drawPath` gives a hairline standing for a
/// thinner stroke (`coverage` < 1, a blend mode that takes coverage as
/// alpha): "the old technique".
pub fn hairline_alpha(alpha: u8, coverage: f32) -> u8 {
    if coverage == 1.0 {
        return alpha;
    }
    let scale = (coverage * 256.0) as i32;
    ((alpha as i32 * scale) >> 8) as u8
}

// --- SkScan_Hairline.cpp: paths ----------------------------------------------

const MAX_CUBIC_SUBDIVIDE_LEVEL: i32 = 9;
const MAX_QUAD_SUBDIVIDE_LEVEL: i32 = 5;

/// `SkScan::AntiHairPath` / `AntiHairSquarePath` / `AntiHairRoundPath`
/// (by `cap`): `path` is in device space, `clip` the device clip.
pub fn anti_hair_path(path: &Path, cap: Cap, clip: IRect, blitter: &mut dyn Blitter) {
    let clip = crate::blitter::device_clip(clip);
    if path.is_empty() {
        return;
    }
    let cap_out = if cap == Cap::Butt { 1 } else { 2 };
    let b = path.bounds().round_out();
    let ibounds = IRect::from_ltrb(
        b.left - cap_out,
        b.top - cap_out,
        b.right + cap_out,
        b.bottom + cap_out,
    );
    if !intersects(&clip, &ibounds) {
        return;
    }
    let mut rgn = None;
    let mut inset_clip = None;
    let mut outset_clip = None;
    if !region_quick_contains(&clip, &ibounds) {
        rgn = Some(clip);
        let c = clip.to_rect();
        let outset = Rect::from_ltrb(c.left - 1.0, c.top - 1.0, c.right + 1.0, c.bottom + 1.0);
        let mut inset = Rect::from_ltrb(c.left + 1.0, c.top + 1.0, c.right - 1.0, c.bottom - 1.0);
        if is_inverted(&inset) {
            inset = Rect::default();
        }
        // The clip is a rect, so the inset clip is used for quick accepts.
        inset_clip = Some(inset);
        outset_clip = Some(outset);
    }
    let clips = Clips {
        rgn,
        inset: inset_clip,
        outset: outset_clip,
    };

    let verbs = path.iter_verbs();
    let mut first_pt = Point::default();
    let mut last_pt = Point::default();
    // kDone_Verb: None.
    let mut prev_verb: Option<Verb> = None;
    for (i, (verb, vpts, w)) in verbs.iter().enumerate() {
        let verb = *verb;
        let next_verb = verbs.get(i + 1).map(|v| v.0);
        let mut pts: Vec<Point> = vpts.clone();
        match verb {
            Verb::Move => {
                first_pt = pts[0];
                last_pt = pts[0];
            }
            Verb::Line => {
                if cap != Cap::Butt {
                    extend_pts(cap, prev_verb, next_verb, &mut pts);
                }
                anti_hair_line_rgn(&pts, clips.rgn, blitter);
                last_pt = pts[1];
            }
            Verb::Quad => {
                if cap != Cap::Butt {
                    extend_pts(cap, prev_verb, next_verb, &mut pts);
                }
                let q = [pts[0], pts[1], pts[2]];
                hairquad(&q, &clips, blitter, compute_quad_level(&q));
                last_pt = pts[2];
            }
            Verb::Conic => {
                if cap != Cap::Butt {
                    extend_pts(cap, prev_verb, next_verb, &mut pts);
                }
                let quads = Conic::new([pts[0], pts[1], pts[2]], *w).to_quads(0.25);
                let mut k = 0;
                while k + 2 < quads.len() {
                    let q = [quads[k], quads[k + 1], quads[k + 2]];
                    hairquad(&q, &clips, blitter, compute_quad_level(&q));
                    k += 2;
                }
                last_pt = pts[2];
            }
            Verb::Cubic => {
                if cap != Cap::Butt {
                    extend_pts(cap, prev_verb, next_verb, &mut pts);
                }
                haircubic(&[pts[0], pts[1], pts[2], pts[3]], &clips, blitter);
                last_pt = pts[3];
            }
            Verb::Close => {
                pts = vec![last_pt, first_pt];
                if cap != Cap::Butt && prev_verb == Some(Verb::Move) {
                    // cap moveTo/close to match svg expectations for degenerate segments
                    extend_pts(cap, prev_verb, next_verb, &mut pts);
                }
                anti_hair_line_rgn(&pts, clips.rgn, blitter);
            }
        }
        if cap != Cap::Butt {
            if prev_verb == Some(Verb::Move)
                && matches!(verb, Verb::Line | Verb::Quad | Verb::Conic | Verb::Cubic)
            {
                // the curve moved the initial point, so close to it instead
                first_pt = pts[0];
            }
            prev_verb = Some(verb);
        }
    }
}

/// The region clip of `hair_path` (a rect, or none when the path is inside
/// the clip) and its culling rects.
struct Clips {
    rgn: Option<IRect>,
    inset: Option<Rect>,
    outset: Option<Rect>,
}

/// `SkIRect::Intersects`.
fn intersects(a: &IRect, b: &IRect) -> bool {
    a.left.max(b.left) < a.right.min(b.right) && a.top.max(b.top) < a.bottom.min(b.bottom)
}

/// `SkRegion::quickContains` of a rect region.
fn region_quick_contains(rgn: &IRect, r: &IRect) -> bool {
    r.left < r.right
        && r.top < r.bottom
        && !rgn.is_empty()
        && rgn.left <= r.left
        && rgn.top <= r.top
        && rgn.right >= r.right
        && rgn.bottom >= r.bottom
}

/// `SkRegion::quickReject` of a rect region.
fn region_quick_reject(rgn: &IRect, r: &IRect) -> bool {
    rgn.is_empty() || r.is_empty() || !intersects(rgn, r)
}

fn is_inverted(r: &Rect) -> bool {
    r.left > r.right || r.top > r.bottom
}

fn geometric_overlap(a: &Rect, b: &Rect) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

fn geometric_contains(outer: &Rect, inner: &Rect) -> bool {
    inner.right <= outer.right
        && inner.left >= outer.left
        && inner.bottom <= outer.bottom
        && inner.top >= outer.top
}

fn bounds_nocheck(pts: &[Point]) -> Rect {
    let mut min = pts[0];
    let mut max = pts[0];
    for p in &pts[1..] {
        min = Point::new(min.x.min(p.x), min.y.min(p.y));
        max = Point::new(max.x.max(p.x), max.y.max(p.y));
    }
    Rect::from_ltrb(min.x, min.y, max.x, max.y)
}

/// `SkScalarCeilToInt`.
fn ceil_to_int(x: f32) -> i32 {
    x.ceil() as i32
}

fn compute_int_quad_dist(pts: &[Point; 3]) -> u32 {
    let dx = ((pts[0].x + pts[2].x) * 0.5 - pts[1].x).abs();
    let dy = ((pts[0].y + pts[2].y) * 0.5 - pts[1].y).abs();
    let idx = ceil_to_int(dx) as u32;
    let idy = ceil_to_int(dy) as u32;
    if idx > idy {
        idx.wrapping_add(idy >> 1)
    } else {
        idy.wrapping_add(idx >> 1)
    }
}

fn compute_quad_level(pts: &[Point; 3]) -> i32 {
    let d = compute_int_quad_dist(pts);
    let level = (33 - d.leading_zeros() as i32) >> 1;
    level.min(MAX_QUAD_SUBDIVIDE_LEVEL)
}

fn hair_quad(pts: &[Point; 3], clip: Option<IRect>, blitter: &mut dyn Blitter, level: i32) {
    // SkQuadCoeff
    let c = pts[0];
    let p1 = pts[1];
    let p2 = pts[2];
    let b = (p1 - c) * 2.0;
    let a = p2 - p1 * 2.0 + c;

    let lines = 1 << level;
    let mut t = 0.0f32;
    let dt = 1.0f32 / lines as f32;
    let mut tmp = Vec::with_capacity(lines + 1);
    tmp.push(pts[0]);
    for _ in 1..lines {
        t += dt;
        tmp.push(Point::new(
            (a.x * t + b.x) * t + c.x,
            (a.y * t + b.y) * t + c.y,
        ));
    }
    tmp.push(pts[2]);
    anti_hair_line_rgn(&tmp, clip, blitter);
}

fn hairquad(pts: &[Point; 3], clips: &Clips, blitter: &mut dyn Blitter, level: i32) {
    let mut clip = clips.rgn;
    if let (Some(inset), Some(outset)) = (&clips.inset, &clips.outset) {
        let bounds = bounds_nocheck(pts);
        if !geometric_overlap(outset, &bounds) {
            return;
        } else if geometric_contains(inset, &bounds) {
            clip = None;
        }
    }
    hair_quad(pts, clip, blitter, level);
}

fn compute_cubic_segs(pts: &[Point; 4]) -> usize {
    let one_third = 1.0f32 / 3.0;
    let two_third = 2.0f32 / 3.0;
    let p13 = pts[3] * one_third + pts[0] * two_third;
    let p23 = pts[0] * one_third + pts[3] * two_third;
    let d1 = pts[1] - p13;
    let d2 = pts[2] - p23;
    let mx = d1.x.abs().max(d2.x.abs());
    let my = d1.y.abs().max(d2.y.abs());
    let diff = mx.max(my);
    let mut tol = 1.0f32 / 8.0;
    for i in 0..MAX_CUBIC_SUBDIVIDE_LEVEL {
        if diff < tol {
            return 1 << i;
        }
        tol *= 4.0;
    }
    1 << MAX_CUBIC_SUBDIVIDE_LEVEL
}

fn lt_90(p0: Point, pivot: Point, p2: Point) -> bool {
    (p0 - pivot).dot(p2 - pivot) >= 0.0
}

fn quick_cubic_niceness_check(pts: &[Point; 4]) -> bool {
    lt_90(pts[1], pts[0], pts[3])
        && lt_90(pts[2], pts[0], pts[3])
        && lt_90(pts[1], pts[3], pts[0])
        && lt_90(pts[2], pts[3], pts[0])
}

fn hair_cubic(pts: &[Point; 4], clip: Option<IRect>, blitter: &mut dyn Blitter) {
    let lines = compute_cubic_segs(pts);
    if lines == 1 {
        anti_hair_line_rgn(&[pts[0], pts[3]], clip, blitter);
        return;
    }
    // SkCubicCoeff
    let (p0, p1, p2, p3) = (pts[0], pts[1], pts[2], pts[3]);
    let a = p3 + (p1 - p2) * 3.0 - p0;
    let b = (p2 - p1 * 2.0 + p0) * 3.0;
    let c = (p1 - p0) * 3.0;
    let d = p0;

    let dt = 1.0f32 / lines as f32;
    let mut t = 0.0f32;
    let mut tmp = Vec::with_capacity(lines + 1);
    tmp.push(pts[0]);
    let mut finite = true;
    for _ in 1..lines {
        t += dt;
        let p = Point::new(
            ((a.x * t + b.x) * t + c.x) * t + d.x,
            ((a.y * t + b.y) * t + c.y) * t + d.y,
        );
        finite &= p.is_finite();
        tmp.push(p);
    }
    if finite {
        tmp.push(pts[3]);
        anti_hair_line_rgn(&tmp, clip, blitter);
    }
}

fn haircubic(pts: &[Point; 4], clips: &Clips, blitter: &mut dyn Blitter) {
    let mut clip = clips.rgn;
    if let (Some(inset), Some(outset)) = (&clips.inset, &clips.outset) {
        let bounds = bounds_nocheck(pts);
        if !geometric_overlap(outset, &bounds) {
            return;
        } else if geometric_contains(inset, &bounds) {
            clip = None;
        }
    }
    if quick_cubic_niceness_check(pts) {
        hair_cubic(pts, clip, blitter);
    } else {
        let mut tmp = [Point::default(); 13];
        let count = chop_cubic_at_max_curvature(pts, &mut tmp);
        for i in 0..count {
            let c = [tmp[i * 3], tmp[i * 3 + 1], tmp[i * 3 + 2], tmp[i * 3 + 3]];
            hair_cubic(&c, clip, blitter);
        }
    }
}

/// `extend_pts`: moves the ends of an open contour's first and last
/// segments out by a square or round cap's share of a pixel.
fn extend_pts(cap: Cap, prev_verb: Option<Verb>, next_verb: Option<Verb>, pts: &mut [Point]) {
    let pt_count = pts.len();
    // The area of a circle is PI*R*R. For a unit circle, R=1/2, and the cap covers half of that.
    let cap_outset = if cap == Cap::Square {
        0.5f32
    } else {
        std::f32::consts::PI / 8.0
    };
    if prev_verb == Some(Verb::Move) {
        let first = 0usize;
        let mut ctrl = first;
        let mut controls = pt_count - 1;
        let mut tangent;
        loop {
            ctrl += 1;
            tangent = pts[first] - pts[ctrl];
            if !(tangent.is_zero() && {
                controls -= 1;
                controls > 0
            }) {
                break;
            }
        }
        if tangent.is_zero() {
            tangent = Point::new(1.0, 0.0);
            controls = pt_count - 1; // If all points are equal, move all but one
        } else {
            tangent = normalize(tangent);
        }
        let mut f = first;
        loop {
            // If the end point and control points are equal, loop to move them in tandem.
            pts[f].x += tangent.x * cap_outset;
            pts[f].y += tangent.y * cap_outset;
            f += 1;
            controls += 1;
            if controls >= pt_count {
                break;
            }
        }
    }
    if matches!(next_verb, None | Some(Verb::Move) | Some(Verb::Close)) {
        let last = pt_count - 1;
        let mut ctrl = last;
        let mut controls = pt_count - 1;
        let mut tangent;
        loop {
            ctrl -= 1;
            tangent = pts[last] - pts[ctrl];
            if !(tangent.is_zero() && {
                controls -= 1;
                controls > 0
            }) {
                break;
            }
        }
        if tangent.is_zero() {
            tangent = Point::new(-1.0, 0.0);
            controls = pt_count - 1;
        } else {
            tangent = normalize(tangent);
        }
        let mut l = last as isize;
        loop {
            pts[l as usize].x += tangent.x * cap_outset;
            pts[l as usize].y += tangent.y * cap_outset;
            l -= 1;
            controls += 1;
            if controls >= pt_count {
                break;
            }
        }
    }
}

/// `SkPoint::normalize` (`setLength(x, y, 1)`): zero when it can't.
fn normalize(v: Point) -> Point {
    let dmag = ((v.x as f64) * (v.x as f64) + (v.y as f64) * (v.y as f64)).sqrt();
    let dscale = 1.0 / dmag;
    let x = (v.x as f64 * dscale) as f32;
    let y = (v.y as f64 * dscale) as f32;
    if !x.is_finite() || !y.is_finite() || (x == 0.0 && y == 0.0) {
        return Point::default();
    }
    Point::new(x, y)
}

// --- SkGeometry.cpp: SkChopCubicAtMaxCurvature -------------------------------

/// `collaps_duplicates`.
fn collaps_duplicates(array: &mut [f32], mut count: usize) -> usize {
    let mut base = 0usize;
    let mut n = count;
    while n > 1 {
        if array[base] == array[base + 1] {
            for i in 1..n {
                array[base + i - 1] = array[base + i];
            }
            count -= 1;
        } else {
            base += 1;
        }
        n -= 1;
    }
    count
}

/// `solve_cubic_poly`.
fn solve_cubic_poly(coeff: &[f32; 4], t_values: &mut [f32; 3]) -> usize {
    if coeff[0].abs() <= 1.0 / 4096.0 {
        // we're just a quadratic
        let mut roots = [0.0f32; 2];
        let n = find_unit_quad_roots(coeff[1], coeff[2], coeff[3], &mut roots);
        t_values[..2].copy_from_slice(&roots);
        return n;
    }
    let inva = 1.0 / coeff[0];
    let a = coeff[1] * inva;
    let b = coeff[2] * inva;
    let c = coeff[3] * inva;
    let q = (a * a - b * 3.0) / 9.0;
    let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
    let q3 = q * q * q;
    let r2_minus_q3 = r * r - q3;
    let adiv3 = a / 3.0;
    if r2_minus_q3 < 0.0 {
        // we have 3 real roots
        let theta = (r / q3.sqrt()).clamp(-1.0, 1.0).acos();
        let neg2_root_q = -2.0 * q.sqrt();
        let pi = std::f32::consts::PI;
        t_values[0] = (neg2_root_q * (theta / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        t_values[1] = (neg2_root_q * ((theta + 2.0 * pi) / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        t_values[2] = (neg2_root_q * ((theta - 2.0 * pi) / 3.0).cos() - adiv3).clamp(0.0, 1.0);
        // bubble_sort
        for i in 1..3 {
            let mut j = i;
            while j > 0 && t_values[j - 1] > t_values[j] {
                t_values.swap(j - 1, j);
                j -= 1;
            }
        }
        collaps_duplicates(t_values, 3)
    } else {
        // we have 1 real root
        let mut a3 = r.abs() + r2_minus_q3.sqrt();
        a3 = a3.cbrt();
        if r > 0.0 {
            a3 = -a3;
        }
        if a3 != 0.0 {
            a3 += q / a3;
        }
        t_values[0] = (a3 - adiv3).clamp(0.0, 1.0);
        1
    }
}

/// `formulate_F1DotF2` over x (`axis` 0) or y.
fn formulate_f1_dot_f2(src: &[Point; 4], axis: usize) -> [f32; 4] {
    let v = |i: usize| if axis == 0 { src[i].x } else { src[i].y };
    let a = v(1) - v(0);
    let b = v(2) - 2.0 * v(1) + v(0);
    let c = v(3) + 3.0 * (v(1) - v(2)) - v(0);
    [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

/// `SkChopCubicAtMaxCurvature`: the number of cubics in `dst`.
fn chop_cubic_at_max_curvature(src: &[Point; 4], dst: &mut [Point; 13]) -> usize {
    let mut cx = formulate_f1_dot_f2(src, 0);
    let cy = formulate_f1_dot_f2(src, 1);
    for i in 0..4 {
        cx[i] += cy[i];
    }
    let mut roots = [0.0f32; 3];
    let root_count = solve_cubic_poly(&cx, &mut roots);
    let mut t_values = Vec::with_capacity(3);
    for &r in &roots[..root_count] {
        if 0.0 < r && r < 1.0 {
            t_values.push(r);
        }
    }
    if t_values.is_empty() {
        dst[..4].copy_from_slice(src);
    } else {
        chop_cubic_at_ts(src, &mut dst[..], &t_values);
    }
    t_values.len() + 1
}

// --- SkScan_Antihair.cpp: lines ----------------------------------------------

const HLINE_STACK_BUFFER: i32 = 100;

/// A run of `count` pixels of coverage `alpha` (Skia's `aa`/`runs` layout).
pub(crate) fn blit_run(blitter: &mut dyn Blitter, x: i32, y: i32, count: i32, alpha: u8) {
    let n = count as usize;
    let mut runs = vec![0i16; n + 1];
    let mut aa = vec![0u8; n];
    runs[0] = count as i16;
    aa[0] = alpha;
    blitter.blit_anti_h(x, y, &aa, &runs);
}

fn call_hline_blitter(blitter: &mut dyn Blitter, mut x: i32, y: i32, mut count: i32, alpha: u8) {
    debug_assert!(count > 0);
    loop {
        let n = count.min(HLINE_STACK_BUFFER);
        blit_run(blitter, x, y, n, alpha);
        x += n;
        count -= n;
        if count <= 0 {
            break;
        }
    }
}

fn small_dot6_scale(value: i32, dot6: i32) -> i32 {
    (value * dot6) >> 6
}

/// The four `SkAntiHairBlitter`s.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HairBlitter {
    HLine,
    Horish,
    VLine,
    Vertish,
}

impl HairBlitter {
    fn draw_cap(self, b: &mut dyn Blitter, x: i32, fy: Fixed, slope: Fixed, mod64: i32) -> Fixed {
        match self {
            HairBlitter::HLine => {
                let fy = fy.wrapping_add(FIXED1 / 2);
                let y = fy >> 16;
                let a = (fy >> 8) & 0xFF;
                // lower line
                let ma = small_dot6_scale(a, mod64);
                if ma != 0 {
                    call_hline_blitter(b, x, y, 1, ma as u8);
                }
                // upper line
                let ma = small_dot6_scale(255 - a, mod64);
                if ma != 0 {
                    call_hline_blitter(b, x, y - 1, 1, ma as u8);
                }
                fy.wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::Horish => {
                let dy = slope;
                let fy = fy.wrapping_add(FIXED1 / 2);
                let lower_y = fy >> 16;
                let a = (fy >> 8) & 0xFF;
                let a0 = small_dot6_scale(255 - a, mod64);
                let a1 = small_dot6_scale(a, mod64);
                b.blit_anti_v2(x, lower_y - 1, a0 as u8, a1 as u8);
                fy.wrapping_add(dy).wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::VLine => {
                let (y, fx) = (x, fy);
                let fx = fx.wrapping_add(FIXED1 / 2);
                let x = fx >> 16;
                let a = (fx >> 8) & 0xFF;
                let ma = small_dot6_scale(a, mod64);
                if ma != 0 {
                    b.blit_v(x, y, 1, ma as u8);
                }
                let ma = small_dot6_scale(255 - a, mod64);
                if ma != 0 {
                    b.blit_v(x - 1, y, 1, ma as u8);
                }
                fx.wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::Vertish => {
                let (y, fx, dx) = (x, fy, slope);
                let fx = fx.wrapping_add(FIXED1 / 2);
                let x = fx >> 16;
                let a = (fx >> 8) & 0xFF;
                b.blit_anti_h2(
                    x - 1,
                    y,
                    small_dot6_scale(255 - a, mod64) as u8,
                    small_dot6_scale(a, mod64) as u8,
                );
                fx.wrapping_add(dx).wrapping_sub(FIXED1 / 2)
            }
        }
    }

    fn draw_line(self, b: &mut dyn Blitter, x: i32, stopx: i32, fy: Fixed, slope: Fixed) -> Fixed {
        debug_assert!(x < stopx);
        match self {
            HairBlitter::HLine => {
                let count = stopx - x;
                let fy = fy.wrapping_add(FIXED1 / 2);
                let y = fy >> 16;
                let a = ((fy >> 8) & 0xFF) as u8;
                if a != 0 {
                    call_hline_blitter(b, x, y, count, a);
                }
                let a = 255 - a;
                if a != 0 {
                    call_hline_blitter(b, x, y - 1, count, a);
                }
                fy.wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::Horish => {
                let dy = slope;
                let mut fy = fy.wrapping_add(FIXED1 / 2);
                for x in x..stopx {
                    let lower_y = fy >> 16;
                    let a = ((fy >> 8) & 0xFF) as u8;
                    b.blit_anti_v2(x, lower_y - 1, 255 - a, a);
                    fy = fy.wrapping_add(dy);
                }
                fy.wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::VLine => {
                let (y, stopy, fx) = (x, stopx, fy);
                let fx = fx.wrapping_add(FIXED1 / 2);
                let x = fx >> 16;
                let a = ((fx >> 8) & 0xFF) as u8;
                if a != 0 {
                    b.blit_v(x, y, stopy - y, a);
                }
                let a = 255 - a;
                if a != 0 {
                    b.blit_v(x - 1, y, stopy - y, a);
                }
                fx.wrapping_sub(FIXED1 / 2)
            }
            HairBlitter::Vertish => {
                let (y, stopy, dx) = (x, stopx, slope);
                let mut fx = fy.wrapping_add(FIXED1 / 2);
                for y in y..stopy {
                    let x = fx >> 16;
                    let a = ((fx >> 8) & 0xFF) as u8;
                    b.blit_anti_h2(x - 1, y, 255 - a, a);
                    fx = fx.wrapping_add(dx);
                }
                fx.wrapping_sub(FIXED1 / 2)
            }
        }
    }
}

fn fastfixdiv(a: FDot6, b: FDot6) -> Fixed {
    left_shift(a, 16) / b
}

fn any_bad_ints(a: i32, b: i32, c: i32, d: i32) -> bool {
    let bad = |x: i32| x & x.wrapping_neg();
    ((bad(a) | bad(b) | bad(c) | bad(d)) >> 31) != 0
}

fn contribution_64(ordinate: FDot6) -> i32 {
    ((ordinate - 1) & 63) + 1
}

fn fdot6_floor(x: FDot6) -> i32 {
    x >> 6
}

fn fdot6_ceil(x: FDot6) -> i32 {
    x.wrapping_add(63) >> 6
}

fn do_anti_hairline(
    mut x0: FDot6,
    mut y0: FDot6,
    mut x1: FDot6,
    mut y1: FDot6,
    clip: Option<IRect>,
    blitter: &mut dyn Blitter,
) {
    // check for integer NaN (0x80000000) which we can't handle (can't negate it)
    if any_bad_ints(x0, y0, x1, y1) {
        return;
    }
    if (x1 - x0).abs() > (511 << 6) || (y1 - y0).abs() > (511 << 6) {
        let hx = (x0 >> 1) + (x1 >> 1);
        let hy = (y0 >> 1) + (y1 >> 1);
        do_anti_hairline(x0, y0, hx, hy, clip, blitter);
        do_anti_hairline(hx, hy, x1, y1, clip, blitter);
        return;
    }

    let mut clip = clip;
    let mut scale_stop;
    let mut istart;
    let mut istop;
    let mut fstart: Fixed;
    let slope: Fixed;
    let hair: HairBlitter;
    let mut scale_start;

    if (x1 - x0).abs() > (y1 - y0).abs() {
        // mostly horizontal
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        istart = fdot6_floor(x0);
        istop = fdot6_ceil(x1);
        fstart = fdot6_to_fixed(y0);
        if y0 == y1 {
            slope = 0;
            hair = HairBlitter::HLine;
        } else {
            slope = fastfixdiv(y1 - y0, x1 - x0);
            fstart = fstart
                .wrapping_add(slope.wrapping_mul(32 - (x0 & 63)).wrapping_add(32) >> 6);
            hair = HairBlitter::Horish;
        }
        if istop - istart == 1 {
            scale_start = x1 - x0;
            scale_stop = 0;
        } else {
            scale_start = 64 - (x0 & 63);
            scale_stop = x1 & 63;
        }
        if let Some(c) = clip {
            if istart >= c.right || istop <= c.left {
                return;
            }
            if istart < c.left {
                fstart = fstart.wrapping_add(slope.wrapping_mul(c.left - istart));
                istart = c.left;
                scale_start = 64;
                if istop - istart == 1 {
                    scale_start = contribution_64(x1);
                    scale_stop = 0;
                }
            }
            if istop > c.right {
                istop = c.right;
                scale_stop = 0;
            }
            if istart == istop {
                return;
            }
            let (mut top, mut bottom);
            let span = (istop - istart - 1).wrapping_mul(slope);
            if slope >= 0 {
                top = fixed_floor_to_int(fstart.wrapping_sub(FIXED_HALF));
                bottom = fixed_ceil_to_int(fstart.wrapping_add(span).wrapping_add(FIXED_HALF));
            } else {
                bottom = fixed_ceil_to_int(fstart.wrapping_add(FIXED_HALF));
                top = fixed_floor_to_int(fstart.wrapping_add(span).wrapping_sub(FIXED_HALF));
            }
            top -= 1;
            bottom += 1;
            if top >= c.bottom || bottom <= c.top {
                return;
            }
            if c.top <= top && c.bottom >= bottom {
                clip = None;
            }
        }
    } else {
        // mostly vertical
        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        istart = fdot6_floor(y0);
        istop = fdot6_ceil(y1);
        fstart = fdot6_to_fixed(x0);
        if x0 == x1 {
            if y0 == y1 {
                return;
            }
            slope = 0;
            hair = HairBlitter::VLine;
        } else {
            slope = fastfixdiv(x1 - x0, y1 - y0);
            fstart = fstart
                .wrapping_add(slope.wrapping_mul(32 - (y0 & 63)).wrapping_add(32) >> 6);
            hair = HairBlitter::Vertish;
        }
        if istop - istart == 1 {
            scale_start = y1 - y0;
            scale_stop = 0;
        } else {
            scale_start = 64 - (y0 & 63);
            scale_stop = y1 & 63;
        }
        if let Some(c) = clip {
            if istart >= c.bottom || istop <= c.top {
                return;
            }
            if istart < c.top {
                fstart = fstart.wrapping_add(slope.wrapping_mul(c.top - istart));
                istart = c.top;
                scale_start = 64;
                if istop - istart == 1 {
                    scale_start = contribution_64(y1);
                    scale_stop = 0;
                }
            }
            if istop > c.bottom {
                istop = c.bottom;
                scale_stop = 0;
            }
            if istart == istop {
                return;
            }
            let (mut left, mut right);
            let span = (istop - istart - 1).wrapping_mul(slope);
            if slope >= 0 {
                left = fixed_floor_to_int(fstart.wrapping_sub(FIXED_HALF));
                right = fixed_ceil_to_int(fstart.wrapping_add(span).wrapping_add(FIXED_HALF));
            } else {
                right = fixed_ceil_to_int(fstart.wrapping_add(FIXED_HALF));
                left = fixed_floor_to_int(fstart.wrapping_add(span).wrapping_sub(FIXED_HALF));
            }
            left -= 1;
            right += 1;
            if left >= c.right || right <= c.left {
                return;
            }
            if c.left <= left && c.right >= right {
                clip = None;
            }
        }
    }

    let mut rect_clipper;
    let blitter: &mut dyn Blitter = if let Some(c) = clip {
        rect_clipper = RectClipBlitter { inner: blitter, clip: c };
        &mut rect_clipper
    } else {
        blitter
    };

    fstart = hair.draw_cap(blitter, istart, fstart, slope, scale_start);
    istart += 1;
    let full_spans = istop - istart - (scale_stop > 0) as i32;
    if full_spans > 0 {
        fstart = hair.draw_line(blitter, istart, istart + full_spans, fstart, slope);
    }
    if scale_stop > 0 {
        hair.draw_cap(blitter, istop - 1, fstart, slope, scale_stop);
    }
}

/// `SkScan::AntiHairLineRgn`: the polyline `array`, clipped to `clip` (a
/// rect region) when there is one.
pub fn anti_hair_line_rgn(array: &[Point], clip: Option<IRect>, blitter: &mut dyn Blitter) {
    let clip = match clip {
        Some(c) => Some(crate::blitter::device_clip(c)),
        None => crate::blitter::device_clip_rect(),
    };
    if let Some(c) = &clip
        && c.is_empty()
    {
        return;
    }
    let max = 32767.0f32;
    let fixed_bounds = Rect::from_ltrb(-max, -max, max, max);
    let clip_bounds = clip.map(|c| {
        let r = c.to_rect();
        Rect::from_ltrb(r.left - 1.0, r.top - 1.0, r.right + 1.0, r.bottom + 1.0)
    });
    for i in 0..array.len().saturating_sub(1) {
        let Some(mut pts) = intersect_line(&[array[i], array[i + 1]], &fixed_bounds) else {
            continue;
        };
        if let Some(cb) = &clip_bounds {
            match intersect_line(&pts, cb) {
                Some(p) => pts = p,
                None => continue,
            }
        }
        let x0 = scalar_to_fdot6(pts[0].x);
        let y0 = scalar_to_fdot6(pts[0].y);
        let x1 = scalar_to_fdot6(pts[1].x);
        let y1 = scalar_to_fdot6(pts[1].y);
        if let Some(c) = &clip {
            let ir = IRect::from_ltrb(
                fdot6_floor(x0.min(x1)) - 1,
                fdot6_floor(y0.min(y1)) - 1,
                fdot6_ceil(x0.max(x1)) + 1,
                fdot6_ceil(y0.max(y1)) + 1,
            );
            if region_quick_reject(c, &ir) {
                continue;
            }
            if !region_quick_contains(c, &ir) {
                // SkRegion::Cliperator over a rect region: one rect.
                let mut r = *c;
                if r.intersect(&ir) {
                    do_anti_hairline(x0, y0, x1, y1, Some(r), blitter);
                }
                continue;
            }
            // fall through to no-clip case
        }
        do_anti_hairline(x0, y0, x1, y1, None, blitter);
    }
}

/// `SkScan::AntiHairRect`.
pub fn anti_hair_rect(rect: Rect, clip: IRect, blitter: &mut dyn Blitter) {
    let clip = crate::blitter::device_clip(clip);
    let pts = [
        Point::new(rect.left, rect.top),
        Point::new(rect.right, rect.top),
        Point::new(rect.right, rect.bottom),
        Point::new(rect.left, rect.bottom),
        Point::new(rect.left, rect.top),
    ];
    // SkScan::AntiHairLine with a BW clip.
    anti_hair_line_rgn(&pts, Some(clip), blitter);
}

/// `SkRectClipBlitter`. It doesn't override `blitAntiH2`/`blitAntiV2`, so
/// those reach the blitter as `blitAntiH` calls.
pub(crate) struct RectClipBlitter<'a> {
    pub inner: &'a mut dyn Blitter,
    pub clip: IRect,
}

impl Blitter for RectClipBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        if y < self.clip.top || y >= self.clip.bottom {
            return;
        }
        let left = x.max(self.clip.left);
        let right = (x + width).min(self.clip.right);
        if right > left {
            self.inner.blit_h(left, y, right - left);
        }
    }

    fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        if y < self.clip.top || y >= self.clip.bottom || x >= self.clip.right {
            return;
        }
        let mut px = x;
        let mut i = 0usize;
        loop {
            let count = runs[i] as i32;
            if count <= 0 {
                return;
            }
            let l = px.max(self.clip.left);
            let r = (px + count).min(self.clip.right);
            if r > l {
                blit_run(self.inner, l, y, r - l, aa[i]);
            }
            px += count;
            i += count as usize;
        }
    }

    fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        if x < self.clip.left || x >= self.clip.right {
            return;
        }
        let y0 = y.max(self.clip.top);
        let y1 = (y + height).min(self.clip.bottom);
        if y0 < y1 {
            self.inner.blit_v(x, y0, y1 - y0, alpha);
        }
    }

    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        let mut r = IRect::from_ltrb(x, y, x + width, y + height);
        if r.intersect(&self.clip) {
            self.inner.blit_rect(r.left, r.top, r.width(), r.height());
        }
    }

    fn blit_mask(&mut self, mask: &[u8], bounds: IRect, row_bytes: i32, clip: IRect) {
        let mut r = clip;
        if r.intersect(&self.clip) {
            self.inner.blit_mask(mask, bounds, row_bytes, r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blitter::Call;

    /// Records every blit in order.
    #[derive(Default)]
    struct Log(Vec<(i32, i32, Call, u8)>);

    impl Blitter for Log {
        fn blit_h(&mut self, x: i32, y: i32, width: i32) {
            for i in 0..width {
                self.0.push((x + i, y, Call::H, 255));
            }
        }
        fn blit_anti_h(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
            let mut px = x;
            let mut i = 0usize;
            while runs[i] > 0 {
                for k in 0..runs[i] as i32 {
                    self.0.push((px + k, y, Call::AntiH, aa[i]));
                }
                px += runs[i] as i32;
                i += runs[i] as usize;
            }
        }
        fn blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
            for i in 0..height {
                self.0.push((x, y + i, Call::V, alpha));
            }
        }
        fn blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
            self.0.push((x, y, Call::AntiH2, a0));
            self.0.push((x + 1, y, Call::AntiH2, a1));
        }
        fn blit_anti_v2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
            self.0.push((x, y, Call::AntiH2, a0));
            self.0.push((x, y + 1, Call::AntiH2, a1));
        }
    }

    #[test]
    fn horizontal_hairline_on_pixel_centres() {
        // y = 10.5: fully on row 10, nothing above.
        let mut log = Log::default();
        anti_hair_line_rgn(
            &[Point::new(2.0, 10.5), Point::new(6.0, 10.5)],
            None,
            &mut log,
        );
        assert!(log.0.iter().all(|&(_, y, _, a)| y == 10 && a == 255));
        let xs: Vec<i32> = log.0.iter().map(|e| e.0).collect();
        assert_eq!(xs, vec![2, 3, 4, 5]);
    }

    #[test]
    fn closed_rect_corners_are_hit_twice() {
        let mut path = Path::new();
        path.add_rect(
            Rect::from_ltrb(0.5, 0.5, 4.5, 4.5),
            crate::path::Direction::Cw,
            0,
        );
        let mut log = Log::default();
        anti_hair_path(&path, Cap::Butt, IRect::from_ltrb(0, 0, 10, 10), &mut log);
        let hits = log.0.iter().filter(|e| e.0 == 0 && e.1 == 0).count();
        assert_eq!(hits, 2);
    }
}
