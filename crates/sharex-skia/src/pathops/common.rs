//! `SkPathOpsCommon.h`, `SkPathOpsCommon.cpp`: winding lookup, chasing,
//! contour sorting and `HandleCoincidence`.

use super::coincidence::Coincidence;
use super::graph::*;
use super::types::*;

/// `AngleWinding`.
pub fn angle_winding(start: Span, end: Span, winding_ptr: &mut i32, sortable_ptr: &mut bool) -> Angle {
    // find first angle, initialize winding to computed fWindSum
    let mut segment = start.segment();
    let mut angle = segment.span_to_angle(start, end);
    if angle.is_null() {
        *winding_ptr = SK_MIN_S32;
        return Angle::NULL;
    }
    let mut compute_winding;
    let mut first_angle = angle;
    let mut loop_ = false;
    let mut unorderable = false;
    let mut winding = SK_MIN_S32;
    loop {
        angle = angle.next();
        if angle.is_null() {
            return Angle::NULL;
        }
        unorderable |= angle.unorderable();
        compute_winding = unorderable || (angle == first_angle && loop_);
        if compute_winding {
            break; // if we get here, there's no winding, loop is unorderable
        }
        loop_ |= angle == first_angle;
        segment = angle.segment();
        winding = segment.wind_sum_angle(angle);
        if winding != SK_MIN_S32 {
            break;
        }
    }
    // if the angle loop contains an unorderable span, the angle order may be useless
    // directly compute the winding in this case for each span
    if compute_winding {
        first_angle = angle;
        winding = SK_MIN_S32;
        loop {
            let start_span = angle.start();
            let end_span = angle.end();
            let lesser = start_span.starter(end_span);
            let mut test_winding = lesser.wind_sum();
            if test_winding == SK_MIN_S32 {
                test_winding = lesser.compute_wind_sum();
            }
            if test_winding != SK_MIN_S32 {
                winding = test_winding;
            }
            angle = angle.next();
            if angle == first_angle {
                break;
            }
        }
    }
    let _ = segment;
    *sortable_ptr = !unorderable;
    *winding_ptr = winding;
    angle
}

/// `FindUndone`.
pub fn find_undone(contour_head: Contour) -> Span {
    let mut contour = contour_head;
    loop {
        if !contour.done() {
            let result = contour.undone_span();
            if result.is_some() {
                return result;
            }
        }
        contour = contour.next();
        if contour.is_null() {
            return Span::NULL;
        }
    }
}

/// `FindChase`.
pub fn find_chase(chase: &mut Vec<Span>, start_ptr: &mut Span, end_ptr: &mut Span) -> Segment {
    while let Some(span) = chase.pop() {
        let mut segment = span.segment();
        *start_ptr = span.pt_t().next().span();
        let mut done = true;
        *end_ptr = Span::NULL;
        let start = *start_ptr;
        let last = segment.active_angle(start, start_ptr, end_ptr, &mut done);
        if last.is_some() {
            *start_ptr = last.start();
            *end_ptr = last.end();
            chase.push(span);
            return last.segment();
        }
        if done {
            continue;
        }
        // find first angle, initialize winding to computed wind sum
        let mut winding = 0;
        let mut sortable = false;
        let mut angle = angle_winding(*start_ptr, *end_ptr, &mut winding, &mut sortable);
        if angle.is_null() {
            return Segment::NULL;
        }
        if winding == SK_MIN_S32 {
            continue;
        }
        let mut sum_winding = 0;
        if sortable {
            segment = angle.segment();
            sum_winding = segment.update_winding_reverse(angle);
        }
        let mut first = Segment::NULL;
        let first_angle = angle;
        loop {
            angle = angle.next();
            if angle == first_angle {
                break;
            }
            segment = angle.segment();
            let start = angle.start();
            let end = angle.end();
            let mut max_winding = 0;
            if sortable {
                segment.set_up_winding(start, end, &mut max_winding, &mut sum_winding);
            }
            if !segment.done_angle(angle) {
                if first.is_null() && (sortable || start.starter(end).wind_sum() != SK_MIN_S32) {
                    first = segment;
                    *start_ptr = start;
                    *end_ptr = end;
                }
                // OPTIMIZATION: should this also add to the chase?
                if sortable {
                    // TODO: add error handling
                    let _ = segment.mark_angle(max_winding, sum_winding, angle, None);
                }
            }
        }
        if first.is_some() {
            chase.push(span);
            return first;
        }
    }
    Segment::NULL
}

/// `SortContourList`.
pub fn sort_contour_list(contour_list: &mut Contour, even_odd: bool, opp_even_odd: bool) -> bool {
    let mut list: Vec<Contour> = Vec::new();
    let mut contour = *contour_list;
    loop {
        if contour.count() != 0 {
            contour.set_opp_xor(if contour.operand() { even_odd } else { opp_even_odd });
            list.push(contour);
        }
        contour = contour.next();
        if contour.is_null() {
            break;
        }
    }
    let count = list.len();
    if count == 0 {
        return false;
    }
    if count > 1 {
        super::sort::tq_sort(&mut list, |a, b| a.less_than(*b));
    }
    contour = list[0];
    contour.global_state().contour_head = contour;
    *contour_list = contour;
    for &next in &list[1..] {
        contour.set_next(next);
        contour = next;
    }
    contour.set_next(Contour::NULL);
    true
}

fn calc_angles(contour_list: Contour) {
    let mut contour = contour_list;
    while contour.is_some() {
        contour.calc_angles();
        contour = contour.next();
    }
}

fn missing_coincidence(contour_list: Contour) -> bool {
    let mut contour = contour_list;
    let mut result = false;
    while contour.is_some() {
        result |= contour.missing_coincidence();
        contour = contour.next();
    }
    result
}

fn move_multiples(contour_list: Contour) -> bool {
    let mut contour = contour_list;
    while contour.is_some() {
        if !contour.move_multiples() {
            return false;
        }
        contour = contour.next();
    }
    true
}

fn move_nearby(contour_list: Contour) -> bool {
    let mut contour = contour_list;
    while contour.is_some() {
        if !contour.move_nearby() {
            return false;
        }
        contour = contour.next();
    }
    true
}

fn sort_angles(contour_list: Contour) -> bool {
    let mut contour = contour_list;
    while contour.is_some() {
        if !contour.sort_angles() {
            return false;
        }
        contour = contour.next();
    }
    true
}

/// `HandleCoincidence`.
pub fn handle_coincidence(contour_list: Contour, coincidence: &mut Coincidence) -> bool {
    let global_state: *mut GlobalState = contour_list.global_state();
    // match up points within the coincident runs
    if !coincidence.add_expanded() {
        return false;
    }
    // combine t values when multiple intersections occur on some segments but not others
    if !move_multiples(contour_list) {
        return false;
    }
    // move t values and points together to eliminate small/tiny gaps
    if !move_nearby(contour_list) {
        return false;
    }
    // add coincidence formed by pairing on curve points and endpoints
    coincidence.correct_ends();
    if !coincidence.add_end_moved_spans() {
        return false;
    }
    const SAFETY_COUNT: i32 = 3;
    let mut safety_hatch = SAFETY_COUNT;
    // look for coincidence present in A-B and A-C but missing in B-C
    loop {
        let mut added = false;
        if !coincidence.add_missing(&mut added) {
            return false;
        }
        if !added {
            break;
        }
        safety_hatch -= 1;
        if safety_hatch == 0 {
            return false;
        }
        move_nearby(contour_list);
    }
    // check to see if, loosely, coincident ranges may be expanded
    if coincidence.expand() {
        let mut added = false;
        if !coincidence.add_missing(&mut added) {
            return false;
        }
        if !coincidence.add_expanded() {
            return false;
        }
        if !move_multiples(contour_list) {
            return false;
        }
        move_nearby(contour_list);
    }
    // the expanded ranges may not align -- add the missing spans
    if !coincidence.add_expanded() {
        return false;
    }
    // mark spans of coincident segments as coincident
    coincidence.mark();
    // look for coincidence lines and curves undetected by intersection
    if missing_coincidence(contour_list) {
        let _ = coincidence.expand();
        if !coincidence.add_expanded() {
            return false;
        }
        if !coincidence.mark() {
            return false;
        }
    } else {
        let _ = coincidence.expand();
    }
    let _ = coincidence.expand();

    // `SkOpCoincidence overlaps(globalState)`: its constructor makes it the
    // global state's coincidence, which it stays from here on (empty).
    let overlaps: *mut Coincidence = unsafe { (*global_state).allocator.make(Coincidence::new(global_state)) };
    unsafe { (*global_state).coincidence = overlaps };
    let coincidence_ptr: *mut Coincidence = coincidence;
    safety_hatch = SAFETY_COUNT;
    loop {
        let pairs: *mut Coincidence = if unsafe { (*overlaps).is_empty() } { coincidence_ptr } else { overlaps };
        // adjust the winding value to account for coincident edges
        if !unsafe { (*pairs).apply() } {
            return false;
        }
        // For each coincident pair that overlaps another, when the receivers (the 1st of the pair)
        // are different, construct a new pair to resolve their mutual span
        let ok = unsafe {
            if pairs == overlaps {
                // overlaps.findOverlaps(&overlaps): Skia clears the output
                // (`this`) before reading `this->fHead`, so nothing is found.
                (*overlaps).clear();
                true
            } else {
                (*pairs).find_overlaps(&mut *overlaps)
            }
        };
        if !ok {
            return false;
        }
        safety_hatch -= 1;
        if safety_hatch == 0 {
            return false;
        }
        if unsafe { (*overlaps).is_empty() } {
            break;
        }
    }
    calc_angles(contour_list);
    if !sort_angles(contour_list) {
        return false;
    }
    true
}
