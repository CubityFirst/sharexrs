//! `SkIntersections.h`, `SkIntersections.cpp` and `SkDLineIntersection.cpp`.
//! The curve/line solvers are in `line_intersections.rs`, the curve/curve
//! ones (`SkTSect`) in `tsect.rs`.

use super::line::DLine;
use super::point::DPoint;
use super::types::*;

#[derive(Clone, Debug)]
pub struct Intersections {
    pub pt: [DPoint; 13],
    pub pt2: [DPoint; 2],
    pub ft: [[f64; 13]; 2],
    pub is_coincident: [u16; 2],
    pub nearly_same: [bool; 2],
    pub used: u8,
    pub max: u8,
    pub allow_near: bool,
    pub swap: bool,
}

impl Default for Intersections {
    fn default() -> Self {
        Intersections {
            pt: [DPoint::default(); 13],
            pt2: [DPoint::default(); 2],
            ft: [[0.0; 13]; 2],
            is_coincident: [0; 2],
            nearly_same: [false; 2],
            used: 0,
            max: 0,
            allow_near: true,
            swap: false,
        }
    }
}

impl Intersections {
    /// `operator[](n)[i]`.
    #[inline]
    pub fn t(&self, n: usize, i: usize) -> f64 {
        self.ft[n][i]
    }

    pub fn allow_near(&mut self, near_allowed: bool) {
        self.allow_near = near_allowed;
    }

    pub fn clear_coincidence(&mut self, index: usize) {
        let bit = 1u16 << index;
        self.is_coincident[0] &= !bit;
        self.is_coincident[1] &= !bit;
    }

    pub fn insert_swap(&mut self, one: f64, two: f64, pt: &DPoint) -> i32 {
        if self.swap { self.insert(two, one, pt) } else { self.insert(one, two, pt) }
    }

    pub fn is_coincident(&self, index: usize) -> bool {
        (self.is_coincident[0] & (1 << index)) != 0
    }

    pub fn nearly_same(&self, index: usize) -> bool {
        self.nearly_same[index]
    }

    #[inline]
    pub fn pt(&self, index: usize) -> DPoint {
        self.pt[index]
    }

    #[inline]
    pub fn pt2(&self, index: usize) -> DPoint {
        self.pt2[index]
    }

    /// Leaves swap, max alone.
    pub fn reset(&mut self) {
        self.allow_near = true;
        self.used = 0;
        self.is_coincident = [0; 2];
    }

    pub fn set(&mut self, swap: bool, t_index: usize, t: f64) {
        self.ft[swap as usize][t_index] = t;
    }

    pub fn set_max(&mut self, max: i32) {
        self.max = max as u8;
    }

    pub fn swap(&mut self) {
        self.swap ^= true;
    }

    pub fn swapped(&self) -> bool {
        self.swap
    }

    #[inline]
    pub fn used(&self) -> i32 {
        self.used as i32
    }

    pub fn down_depth(&mut self) {}

    pub fn up_depth(&mut self) {}

    pub fn depth(&self) -> i32 {
        0
    }

    pub fn un_bump_t(&mut self, index: usize) -> bool {
        self.ft[0][index] = self.ft[0][index] * (1.0 + BUMP_EPSILON * 2.0) - BUMP_EPSILON;
        if !between(0.0, self.ft[0][index], 1.0) {
            self.used = 0;
            return false;
        }
        true
    }

    pub fn closest_to(&self, range_start: f64, range_end: f64, test_pt: &DPoint, closest_dist: &mut f64) -> i32 {
        let mut closest = -1;
        *closest_dist = f32::MAX as f64;
        for index in 0..self.used as usize {
            if !between(range_start, self.ft[0][index], range_end) {
                continue;
            }
            let i_pt = self.pt[index];
            let dist = test_pt.distance_squared(&i_pt);
            if *closest_dist > dist {
                *closest_dist = dist;
                closest = index as i32;
            }
        }
        closest
    }

    pub fn flip(&mut self) {
        for index in 0..self.used as usize {
            self.ft[1][index] = 1.0 - self.ft[1][index];
        }
    }

    fn move_down(&mut self, index: usize, remaining: usize) {
        // memmove(&fX[index], &fX[index + 1], remaining)
        self.pt.copy_within(index + 1..index + 1 + remaining, index);
        self.ft[0].copy_within(index + 1..index + 1 + remaining, index);
        self.ft[1].copy_within(index + 1..index + 1 + remaining, index);
    }

    /// FIXME: does not respect swap.
    pub fn insert(&mut self, one: f64, two: f64, pt: &DPoint) -> i32 {
        if self.is_coincident[0] == 3 && between(self.ft[0][0], one, self.ft[0][1]) {
            // For now, don't allow a mix of coincident and non-coincident intersections
            return -1;
        }
        let mut index = 0usize;
        while index < self.used as usize {
            let old_one = self.ft[0][index];
            let old_two = self.ft[1][index];
            if one == old_one && two == old_two {
                return -1;
            }
            if more_roughly_equal(old_one, one) && more_roughly_equal(old_two, two) {
                if (!precisely_zero(one) || precisely_zero(old_one))
                    && (!precisely_equal(one, 1.0) || precisely_equal(old_one, 1.0))
                    && (!precisely_zero(two) || precisely_zero(old_two))
                    && (!precisely_equal(two, 1.0) || precisely_equal(old_two, 1.0))
                {
                    return -1;
                }
                // remove this and reinsert below in case replacing would make list unsorted
                let remaining = self.used as usize - index - 1;
                self.move_down(index, remaining);
                let clear_mask: i32 = !((1i32 << index) - 1);
                for k in 0..2 {
                    let c = self.is_coincident[k] as i32;
                    self.is_coincident[k] = (c - ((c >> 1) & clear_mask)) as u16;
                }
                self.used -= 1;
                break;
            }
            index += 1;
        }
        index = 0;
        while index < self.used as usize {
            if self.ft[0][index] > one {
                break;
            }
            index += 1;
        }
        if self.used >= self.max {
            self.used = 0;
            return 0;
        }
        let remaining = self.used as isize - index as isize;
        if remaining > 0 {
            let remaining = remaining as usize;
            self.pt.copy_within(index..index + remaining, index + 1);
            self.ft[0].copy_within(index..index + remaining, index + 1);
            self.ft[1].copy_within(index..index + remaining, index + 1);
            let clear_mask: i32 = !((1i32 << index) - 1);
            for k in 0..2 {
                let c = self.is_coincident[k] as i32;
                self.is_coincident[k] = (c + (c & clear_mask)) as u16;
            }
        }
        self.pt[index] = *pt;
        if one < 0.0 || one > 1.0 {
            return -1;
        }
        if two < 0.0 || two > 1.0 {
            return -1;
        }
        self.ft[0][index] = one;
        self.ft[1][index] = two;
        self.used += 1;
        index as i32
    }

    pub fn insert_near(&mut self, one: f64, two: f64, pt1: &DPoint, pt2: &DPoint) {
        let k = if one != 0.0 { 1 } else { 0 };
        self.nearly_same[k] = true;
        let _ = self.insert(one, two, pt1);
        self.pt2[k] = *pt2;
    }

    /// Start if index == 0 : end if index == 1.
    pub fn insert_coincident(&mut self, one: f64, two: f64, pt: &DPoint) -> i32 {
        let index = self.insert_swap(one, two, pt);
        if index >= 0 {
            self.set_coincident(index as usize);
        }
        index
    }

    pub fn set_coincident(&mut self, index: usize) {
        let bit = 1u16 << index;
        self.is_coincident[0] |= bit;
        self.is_coincident[1] |= bit;
    }

    pub fn merge(&mut self, a: &Intersections, a_index: usize, b: &Intersections, b_index: usize) {
        self.reset();
        self.ft[0][0] = a.ft[0][a_index];
        self.ft[1][0] = b.ft[0][b_index];
        self.pt[0] = a.pt[a_index];
        self.pt2[0] = b.pt[b_index];
        self.used = 1;
    }

    pub fn most_outside(&self, range_start: f64, range_end: f64, origin: &DPoint) -> i32 {
        let mut result: i32 = -1;
        for index in 0..self.used as usize {
            if !between(range_start, self.ft[0][index], range_end) {
                continue;
            }
            if result < 0 {
                result = index as i32;
                continue;
            }
            let best = self.pt[result as usize] - *origin;
            let test = self.pt[index] - *origin;
            if test.cross_check(best) < 0.0 {
                result = index as i32;
            }
        }
        result
    }

    pub fn remove_one(&mut self, index: usize) {
        self.used -= 1;
        let remaining = self.used as isize - index as isize;
        if remaining <= 0 {
            return;
        }
        self.move_down(index, remaining as usize);
        let co_bit = self.is_coincident[0] as i32 & (1 << index);
        let mask = !((1i32 << index) - 1);
        let c0 = self.is_coincident[0] as i32;
        self.is_coincident[0] = (c0 - (((c0 >> 1) & mask) + co_bit)) as u16;
        let c1 = self.is_coincident[1] as i32;
        self.is_coincident[1] = (c1 - (((c1 >> 1) & mask) + co_bit)) as u16;
    }

    // SkDLineIntersection.cpp

    fn clean_up_parallel_lines(&mut self, parallel: bool) {
        while self.used > 2 {
            self.remove_one(1);
        }
        if self.used == 2 && !parallel {
            let start_match = self.ft[0][0] == 0.0 || zero_or_one(self.ft[1][0]);
            let end_match = self.ft[0][1] == 1.0 || zero_or_one(self.ft[1][1]);
            if (!start_match && !end_match) || approximately_equal(self.ft[0][0], self.ft[0][1]) {
                if start_match
                    && end_match
                    && (self.ft[0][0] != 0.0 || !zero_or_one(self.ft[1][0]))
                    && self.ft[0][1] == 1.0
                    && zero_or_one(self.ft[1][1])
                {
                    self.remove_one(0);
                } else {
                    self.remove_one(end_match as usize);
                }
            }
        }
        if self.used == 2 {
            self.is_coincident = [0x03, 0x03];
        }
    }

    pub(crate) fn compute_points(&mut self, line: &DLine, used: i32) {
        self.pt[0] = line.pt_at_t(self.ft[0][0]);
        self.used = used as u8;
        if used == 2 {
            self.pt[1] = line.pt_at_t(self.ft[0][1]);
        }
    }

    /// `intersectRay(const SkDLine&, const SkDLine&)`.
    pub fn intersect_ray_line(&mut self, a: &DLine, b: &DLine) -> i32 {
        self.max = 2;
        let a_len = a[1] - a[0];
        let b_len = b[1] - b[0];
        let denom = b_len.y * a_len.x - a_len.y * b_len.x;
        let used;
        if !approximately_zero(denom) {
            let ab0 = a[0] - b[0];
            let mut numer_a = ab0.y * b_len.x - b_len.y * ab0.x;
            let mut numer_b = ab0.y * a_len.x - a_len.y * ab0.x;
            numer_a /= denom;
            numer_b /= denom;
            self.ft[0][0] = numer_a;
            self.ft[1][0] = numer_b;
            used = 1;
        } else {
            if !almost_equal_ulps_d(a_len.x * a[0].y - a_len.y * a[0].x, a_len.x * b[0].y - a_len.y * b[0].x) {
                self.used = 0;
                return 0;
            }
            // there's no great answer for intersection points for coincident rays, but return something
            self.ft[0][0] = 0.0;
            self.ft[1][0] = 0.0;
            self.ft[1][0] = 1.0;
            self.ft[1][1] = 1.0;
            used = 2;
        }
        self.compute_points(a, used);
        self.used as i32
    }

    /// `intersect(const SkDLine&, const SkDLine&)`: only works if both lines
    /// are neither horizontal nor vertical.
    pub fn intersect_line_line(&mut self, a: &DLine, b: &DLine) -> i32 {
        self.max = 3; // note that we clean up so that there is no more than two in the end
        // see if end points intersect the opposite line
        let mut t;
        for i_a in 0..2 {
            t = b.exact_point(&a[i_a]);
            if t >= 0.0 {
                self.insert(i_a as f64, t, &a[i_a]);
            }
        }
        for i_b in 0..2 {
            t = a.exact_point(&b[i_b]);
            if t >= 0.0 {
                self.insert(t, i_b as f64, &b[i_b]);
            }
        }
        let ax_len = a[1].x - a[0].x;
        let ay_len = a[1].y - a[0].y;
        let bx_len = b[1].x - b[0].x;
        let by_len = b[1].y - b[0].y;
        let ax_by_len = ax_len * by_len;
        let ay_bx_len = ay_len * bx_len;
        // detect parallel lines the same way here and in SkOpAngle operator <
        // so that non-parallel means they are also sortable
        let unparallel = if self.allow_near {
            not_almost_equal_ulps_pin_d(ax_by_len, ay_bx_len)
        } else {
            not_almost_dequal_ulps_d(ax_by_len, ay_bx_len)
        };
        if unparallel && self.used == 0 {
            let ab0y = a[0].y - b[0].y;
            let ab0x = a[0].x - b[0].x;
            let numer_a = ab0y * bx_len - by_len * ab0x;
            let numer_b = ab0y * ax_len - ay_len * ab0x;
            let denom = ax_by_len - ay_bx_len;
            if between(0.0, numer_a, denom) && between(0.0, numer_b, denom) {
                self.ft[0][0] = numer_a / denom;
                self.ft[1][0] = numer_b / denom;
                self.compute_points(a, 1);
            }
        }
        // Allow tracking that both sets of end points are near each other -- the lines are
        // entirely coincident -- even when the end points are not exactly the same.
        if self.allow_near || !unparallel {
            let mut a_near_b = [0.0f64; 2];
            let mut b_near_a = [0.0f64; 2];
            let mut a_not_b = [false, false];
            let mut b_not_a = [false, false];
            let mut near_count = 0;
            for index in 0..2 {
                t = b.near_point(&a[index], Some(&mut a_not_b[index]));
                a_near_b[index] = t;
                near_count += (t >= 0.0) as i32;
                t = a.near_point(&b[index], Some(&mut b_not_a[index]));
                b_near_a[index] = t;
                near_count += (t >= 0.0) as i32;
            }
            if near_count > 0 {
                // Skip if each segment contributes to one end point.
                if near_count != 2 || a_not_b[0] == a_not_b[1] {
                    for i_a in 0..2 {
                        if !a_not_b[i_a] {
                            continue;
                        }
                        let nearer = (a_near_b[i_a] > 0.5) as usize;
                        if !b_not_a[nearer] {
                            continue;
                        }
                        self.insert_near(i_a as f64, nearer as f64, &a[i_a], &b[nearer]);
                        a_near_b[i_a] = -1.0;
                        b_near_a[nearer] = -1.0;
                        near_count -= 2;
                    }
                }
                if near_count > 0 {
                    for i_a in 0..2 {
                        if a_near_b[i_a] >= 0.0 {
                            self.insert(i_a as f64, a_near_b[i_a], &a[i_a]);
                        }
                    }
                    for i_b in 0..2 {
                        if b_near_a[i_b] >= 0.0 {
                            self.insert(b_near_a[i_b], i_b as f64, &b[i_b]);
                        }
                    }
                }
            }
        }
        self.clean_up_parallel_lines(!unparallel);
        self.used as i32
    }

    #[allow(non_snake_case)]
    pub fn HorizontalInterceptLine(line: &DLine, y: f64) -> f64 {
        sk_pin_t((y - line[0].y) / (line[1].y - line[0].y))
    }

    pub fn horizontal_line(&mut self, line: &DLine, left: f64, right: f64, y: f64, flipped: bool) -> i32 {
        self.max = 3; // clean up parallel at the end will limit the result to 2 at the most
        // see if end points intersect the opposite line
        let mut t;
        let left_pt = DPoint::new(left, y);
        t = line.exact_point(&left_pt);
        if t >= 0.0 {
            self.insert(t, flipped as i32 as f64, &left_pt);
        }
        if left != right {
            let right_pt = DPoint::new(right, y);
            t = line.exact_point(&right_pt);
            if t >= 0.0 {
                self.insert(t, (!flipped) as i32 as f64, &right_pt);
            }
            for index in 0..2 {
                t = DLine::ExactPointH(&line[index], left, right, y);
                if t >= 0.0 {
                    self.insert(index as f64, if flipped { 1.0 - t } else { t }, &line[index]);
                }
            }
        }
        let result = horizontal_coincident(line, y);
        if result == 1 && self.used == 0 {
            self.ft[0][0] = Self::HorizontalInterceptLine(line, y);
            let x_intercept = line[0].x + self.ft[0][0] * (line[1].x - line[0].x);
            if between(left, x_intercept, right) {
                self.ft[1][0] = (x_intercept - left) / (right - left);
                if flipped {
                    for index in 0..result as usize {
                        self.ft[1][index] = 1.0 - self.ft[1][index];
                    }
                }
                self.pt[0].x = x_intercept;
                self.pt[0].y = y;
                self.used = 1;
            }
        }
        if self.allow_near || result == 2 {
            t = line.near_point(&left_pt, None);
            if t >= 0.0 {
                self.insert(t, flipped as i32 as f64, &left_pt);
            }
            if left != right {
                let right_pt = DPoint::new(right, y);
                t = line.near_point(&right_pt, None);
                if t >= 0.0 {
                    self.insert(t, (!flipped) as i32 as f64, &right_pt);
                }
                for index in 0..2 {
                    t = DLine::NearPointH(&line[index], left, right, y);
                    if t >= 0.0 {
                        self.insert(index as f64, if flipped { 1.0 - t } else { t }, &line[index]);
                    }
                }
            }
        }
        self.clean_up_parallel_lines(result == 2);
        self.used as i32
    }

    #[allow(non_snake_case)]
    pub fn VerticalInterceptLine(line: &DLine, x: f64) -> f64 {
        sk_pin_t((x - line[0].x) / (line[1].x - line[0].x))
    }

    pub fn vertical_line(&mut self, line: &DLine, top: f64, bottom: f64, x: f64, flipped: bool) -> i32 {
        self.max = 3; // cleanup parallel lines will bring this back line
        // see if end points intersect the opposite line
        let mut t;
        let top_pt = DPoint::new(x, top);
        t = line.exact_point(&top_pt);
        if t >= 0.0 {
            self.insert(t, flipped as i32 as f64, &top_pt);
        }
        if top != bottom {
            let bottom_pt = DPoint::new(x, bottom);
            t = line.exact_point(&bottom_pt);
            if t >= 0.0 {
                self.insert(t, (!flipped) as i32 as f64, &bottom_pt);
            }
            for index in 0..2 {
                t = DLine::ExactPointV(&line[index], top, bottom, x);
                if t >= 0.0 {
                    self.insert(index as f64, if flipped { 1.0 - t } else { t }, &line[index]);
                }
            }
        }
        let result = vertical_coincident(line, x);
        if result == 1 && self.used == 0 {
            self.ft[0][0] = Self::VerticalInterceptLine(line, x);
            let y_intercept = line[0].y + self.ft[0][0] * (line[1].y - line[0].y);
            if between(top, y_intercept, bottom) {
                self.ft[1][0] = (y_intercept - top) / (bottom - top);
                if flipped {
                    for index in 0..result as usize {
                        self.ft[1][index] = 1.0 - self.ft[1][index];
                    }
                }
                self.pt[0].x = x;
                self.pt[0].y = y_intercept;
                self.used = 1;
            }
        }
        if self.allow_near || result == 2 {
            t = line.near_point(&top_pt, None);
            if t >= 0.0 {
                self.insert(t, flipped as i32 as f64, &top_pt);
            }
            if top != bottom {
                let bottom_pt = DPoint::new(x, bottom);
                t = line.near_point(&bottom_pt, None);
                if t >= 0.0 {
                    self.insert(t, (!flipped) as i32 as f64, &bottom_pt);
                }
                for index in 0..2 {
                    t = DLine::NearPointV(&line[index], top, bottom, x);
                    if t >= 0.0 {
                        self.insert(index as f64, if flipped { 1.0 - t } else { t }, &line[index]);
                    }
                }
            }
        }
        self.clean_up_parallel_lines(result == 2);
        self.used as i32
    }
}

fn horizontal_coincident(line: &DLine, y: f64) -> i32 {
    let mut min = line[0].y;
    let mut max = line[1].y;
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if min > y || max < y {
        return 0;
    }
    if almost_equal_ulps_d(min, max) && max - min < (line[0].x - line[1].x).abs() {
        return 2;
    }
    1
}

fn vertical_coincident(line: &DLine, x: f64) -> i32 {
    let mut min = line[0].x;
    let mut max = line[1].x;
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if !precisely_between(min, x, max) {
        return 0;
    }
    if almost_equal_ulps_d(min, max) {
        return 2;
    }
    1
}
