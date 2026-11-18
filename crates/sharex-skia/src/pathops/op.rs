//! `SkPathOpsOp.cpp` (`Op`, `bridgeOp`, `findChaseOp`) and
//! `SkPathOpsSimplify.cpp` (`Simplify`, `bridgeWinding`, `bridgeXor`).

use super::add_intersections::add_intersect_ts;
use super::coincidence::Coincidence;
use super::common::*;
use super::edge_builder::EdgeBuilder;
use super::graph::*;
use super::types::*;
use super::winding::find_sortable_top;
use super::writer::PathWriter;
use super::PathOp;
use crate::path::{FillType, Path, Verb};

fn find_chase_op(chase: &mut Vec<Span>, start_ptr: &mut Span, end_ptr: &mut Span, result: &mut Segment) -> bool {
    while let Some(span) = chase.pop() {
        // OPTIMIZE: prev makes this compatible with old code -- but is it necessary?
        *start_ptr = span.pt_t().prev().span();
        let mut segment = start_ptr.segment();
        let mut done = true;
        *end_ptr = Span::NULL;
        let start = *start_ptr;
        let last = segment.active_angle(start, start_ptr, end_ptr, &mut done);
        if last.is_some() {
            *start_ptr = last.start();
            *end_ptr = last.end();
            chase.push(span);
            *result = last.segment();
            return true;
        }
        if done {
            continue;
        }
        let mut winding = 0;
        let mut sortable = false;
        let mut angle = angle_winding(*start_ptr, *end_ptr, &mut winding, &mut sortable);
        if angle.is_null() {
            *result = Segment::NULL;
            return true;
        }
        if winding == SK_MIN_S32 {
            continue;
        }
        let (mut sum_mi_winding, mut sum_su_winding) = (0, 0);
        if sortable {
            segment = angle.segment();
            sum_mi_winding = segment.update_winding_reverse(angle);
            if sum_mi_winding == SK_MIN_S32 {
                *result = Segment::NULL;
                return true;
            }
            sum_su_winding = segment.update_opp_winding_reverse(angle);
            if sum_su_winding == SK_MIN_S32 {
                *result = Segment::NULL;
                return true;
            }
            if segment.operand() {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
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
            let (mut max_winding, mut sum_winding, mut opp_max_winding, mut opp_sum_winding) = (0, 0, 0, 0);
            if sortable {
                segment.set_up_windings_op(
                    start,
                    end,
                    &mut sum_mi_winding,
                    &mut sum_su_winding,
                    &mut max_winding,
                    &mut sum_winding,
                    &mut opp_max_winding,
                    &mut opp_sum_winding,
                );
            }
            if !segment.done_angle(angle) {
                if first.is_null() && (sortable || start.starter(end).wind_sum() != SK_MIN_S32) {
                    first = segment;
                    *start_ptr = start;
                    *end_ptr = end;
                }
                // OPTIMIZATION: should this also add to the chase?
                if sortable
                    && !segment.mark_angle_op(max_winding, sum_winding, opp_max_winding, opp_sum_winding, angle, None)
                {
                    return false;
                }
            }
        }
        if first.is_some() {
            chase.push(span);
            *result = first;
            return true;
        }
    }
    *result = Segment::NULL;
    true
}

fn bridge_op(contour_list: Contour, op: PathOp, xor_mask: i32, xor_op_mask: i32, writer: &mut PathWriter) -> bool {
    let mut unsortable = false;
    let mut last_simple;
    let mut simple = false;
    loop {
        let span = find_sortable_top(contour_list);
        if span.is_null() {
            break;
        }
        let mut current = span.segment();
        let mut start = span.next();
        let mut end = span;
        let mut chase: Vec<Span> = Vec::new();
        loop {
            if current.active_op(start, end, xor_mask, xor_op_mask, op) {
                loop {
                    if !unsortable && current.done() {
                        break;
                    }
                    let mut next_start = start;
                    let mut next_end = end;
                    last_simple = simple;
                    let next = current.find_next_op(
                        &mut chase,
                        &mut next_start,
                        &mut next_end,
                        &mut unsortable,
                        &mut simple,
                        op,
                        xor_mask,
                        xor_op_mask,
                    );
                    if next.is_null() {
                        if !unsortable && writer.has_move() && current.verb() != Verb::Line && !writer.is_closed() {
                            if !current.add_curve_to(start, end, writer) {
                                return false;
                            }
                        } else if last_simple && !current.add_curve_to(start, end, writer) {
                            return false;
                        }
                        break;
                    }
                    if !current.add_curve_to(start, end, writer) {
                        return false;
                    }
                    current = next;
                    start = next_start;
                    end = next_end;
                    if !(!writer.is_closed() && (!unsortable || !start.starter(end).done())) {
                        break;
                    }
                }
                if current.active_winding(start, end) && !writer.is_closed() {
                    let span_start = start.starter(end);
                    if !span_start.done() {
                        if !current.add_curve_to(start, end, writer) {
                            return false;
                        }
                        current.mark_done(span_start);
                    }
                }
                writer.finish_contour();
            } else {
                let mut last = Span::NULL;
                if !current.mark_and_chase_done(start, end, Some(&mut last)) {
                    return false;
                }
                if last.is_some() && !last.chased() {
                    last.set_chased(true);
                    chase.push(last);
                }
            }
            if !find_chase_op(&mut chase, &mut start, &mut end, &mut current) {
                return false;
            }
            if current.is_null() {
                break;
            }
        }
    }
    true
}

// diagram of why this simplifcation is possible is here:
// https://skia.org/dev/present/pathops link at bottom of the page
static OP_INVERSE: [[[PathOp; 2]; 2]; 5] = {
    use PathOp::*;
    [
        //                  inside minuend                               outside minuend
        //     inside subtrahend     outside subtrahend      inside subtrahend     outside subtrahend
        [[Difference, Intersect], [Union, ReverseDifference]],
        [[Intersect, Difference], [ReverseDifference, Union]],
        [[Union, ReverseDifference], [Difference, Intersect]],
        [[Xor, Xor], [Xor, Xor]],
        [[ReverseDifference, Union], [Intersect, Difference]],
    ]
};

static OUT_INVERSE: [[[bool; 2]; 2]; 5] = [
    [[false, false], [true, false]], // diff
    [[false, false], [false, true]], // sect
    [[false, true], [true, true]],   // union
    [[false, true], [true, false]],  // xor
    [[false, true], [false, false]], // rev diff
];

/// The per-operation state: the arena, the global state and the head
/// contour (`SkOpContour contour` on Skia's stack).
struct OpState {
    global_state: Box<GlobalState>,
    head: Box<ContourData>,
}

impl OpState {
    fn new() -> OpState {
        let mut head = Box::new(ContourData::default());
        let head_handle = Contour(&mut *head);
        let mut global_state = Box::new(GlobalState::new(head_handle));
        // SkOpContour's head is reached from its global state once init'd
        head.state = &mut *global_state;
        OpState { global_state, head }
    }

    fn head(&mut self) -> Contour {
        Contour(&mut *self.head)
    }

    fn gs(&mut self) -> *mut GlobalState {
        &mut *self.global_state
    }
}

/// `Op` (`OpDebug`): `one op two`, as `SkPathOps`'s `Op` computes it. The
/// result's fill type is even-odd. Returns `None` where Skia returns false.
pub fn op(one: &Path, two: &Path, op_in: PathOp) -> Option<Path> {
    // sharex-skia's paths have no inverse fill types
    let op = OP_INVERSE[op_in as usize][0][0];
    let inverse_fill = OUT_INVERSE[op as usize][0][0];
    let _ = inverse_fill;
    let fill_type = FillType::EvenOdd;
    if PathOp::Intersect == op
        && let (Some(mut rect1), Some(rect2)) = (one.is_rect(), two.is_rect())
    {
        let mut result = Path::new();
        result.fill_type = fill_type;
        if rect1.intersect(&rect2) {
            result.add_rect(rect1, crate::path::Direction::Cw, 0);
        }
        return Some(result);
    }
    if one.is_empty() || two.is_empty() {
        let work = match op {
            PathOp::Intersect => Path::new(),
            PathOp::Union | PathOp::Xor => {
                if one.is_empty() {
                    two.clone()
                } else {
                    one.clone()
                }
            }
            PathOp::Difference => {
                if !one.is_empty() {
                    one.clone()
                } else {
                    Path::new()
                }
            }
            PathOp::ReverseDifference => {
                if !two.is_empty() {
                    two.clone()
                } else {
                    Path::new()
                }
            }
        };
        return simplify(&work);
    }
    let mut state = OpState::new();
    let gs = state.gs();
    let mut contour_list = state.head();
    let mut coincidence = Box::new(Coincidence::new(gs));
    unsafe { (*gs).coincidence = &mut *coincidence };
    let (mut minuend, mut subtrahend) = (one, two);
    let mut op = op;
    if op == PathOp::ReverseDifference {
        std::mem::swap(&mut minuend, &mut subtrahend);
        op = PathOp::Difference;
    }
    // turn path into list of segments
    let mut builder = EdgeBuilder::new(minuend, contour_list, gs);
    if builder.unparseable() {
        return None;
    }
    let xor_mask = builder.xor_mask();
    builder.add_operand(subtrahend);
    if !builder.finish() {
        return None;
    }
    let xor_op_mask = builder.xor_mask();
    if !sort_contour_list(&mut contour_list, xor_mask == EVEN_ODD_PATH_OPS_MASK, xor_op_mask == EVEN_ODD_PATH_OPS_MASK) {
        let mut result = Path::new();
        result.fill_type = fill_type;
        return Some(result);
    }
    // find all intersections between segments
    let mut current = contour_list;
    loop {
        let mut next = current;
        while add_intersect_ts(current, next, &mut coincidence) {
            next = next.next();
            if next.is_null() {
                break;
            }
        }
        current = current.next();
        if current.is_null() {
            break;
        }
    }
    let success = handle_coincidence(contour_list, &mut coincidence);
    if !success {
        return None;
    }
    // construct closed contours
    let mut result = Path::new();
    result.fill_type = fill_type;
    {
        let mut wrapper = PathWriter::new(&mut result);
        if !bridge_op(contour_list, op, xor_mask, xor_op_mask, &mut wrapper) {
            return None;
        }
        wrapper.assemble(); // if some edges could not be resolved, assemble remaining
    }
    drop(builder);
    drop(coincidence);
    drop(state);
    Some(result)
}

fn bridge_winding(contour_list: Contour, writer: &mut PathWriter) -> bool {
    let mut unsortable = false;
    loop {
        let span = find_sortable_top(contour_list);
        if span.is_null() {
            break;
        }
        let mut current = span.segment();
        let mut start = span.next();
        let mut end = span;
        let mut chase: Vec<Span> = Vec::new();
        loop {
            if current.active_winding(start, end) {
                loop {
                    if !unsortable && current.done() {
                        break;
                    }
                    let mut next_start = start;
                    let mut next_end = end;
                    let next = current.find_next_winding(&mut chase, &mut next_start, &mut next_end, &mut unsortable);
                    if next.is_null() {
                        break;
                    }
                    if !current.add_curve_to(start, end, writer) {
                        return false;
                    }
                    current = next;
                    start = next_start;
                    end = next_end;
                    if !(!writer.is_closed() && (!unsortable || !start.starter(end).done())) {
                        break;
                    }
                }
                if current.active_winding(start, end) && !writer.is_closed() {
                    let span_start = start.starter(end);
                    if !span_start.done() {
                        if !current.add_curve_to(start, end, writer) {
                            return false;
                        }
                        current.mark_done(span_start);
                    }
                }
                writer.finish_contour();
            } else {
                let mut last = Span::NULL;
                if !current.mark_and_chase_done(start, end, Some(&mut last)) {
                    return false;
                }
                if last.is_some() && !last.chased() {
                    last.set_chased(true);
                    chase.push(last);
                }
            }
            current = find_chase(&mut chase, &mut start, &mut end);
            if current.is_null() {
                break;
            }
        }
    }
    true
}

/// Returns true if all edges were processed.
fn bridge_xor(contour_list: Contour, writer: &mut PathWriter) -> bool {
    let mut unsortable = false;
    let mut safety_net = 1000000;
    loop {
        let span = find_undone(contour_list);
        if span.is_null() {
            break;
        }
        let mut current = span.segment();
        let mut start = span.next();
        let mut end = span;
        loop {
            safety_net -= 1;
            if safety_net < 0 {
                return false;
            }
            if !unsortable && current.done() {
                break;
            }
            let mut next_start = start;
            let mut next_end = end;
            let next = current.find_next_xor(&mut next_start, &mut next_end, &mut unsortable);
            if next.is_null() {
                break;
            }
            if !current.add_curve_to(start, end, writer) {
                return false;
            }
            current = next;
            start = next_start;
            end = next_end;
            if !(!writer.is_closed() && (!unsortable || !start.starter(end).done())) {
                break;
            }
        }
        if !writer.is_closed() {
            let span_start = start.starter(end);
            if !span_start.done() {
                return false;
            }
        }
        writer.finish_contour();
    }
    true
}

/// `Simplify`: the path with overlaps removed, even-odd.
pub fn simplify(path: &Path) -> Option<Path> {
    let fill_type = FillType::EvenOdd;
    if path.is_convex() {
        let mut result = path.clone();
        result.fill_type = fill_type;
        return Some(result);
    }
    // turn path into list of segments
    let mut state = OpState::new();
    let gs = state.gs();
    let mut contour_list = state.head();
    let mut coincidence = Box::new(Coincidence::new(gs));
    unsafe { (*gs).coincidence = &mut *coincidence };
    let mut builder = EdgeBuilder::new(path, contour_list, gs);
    if !builder.finish() {
        return None;
    }
    if !sort_contour_list(&mut contour_list, false, false) {
        let mut result = Path::new();
        result.fill_type = fill_type;
        return Some(result);
    }
    // find all intersections between segments
    let mut current = contour_list;
    loop {
        let mut next = current;
        while add_intersect_ts(current, next, &mut coincidence) {
            next = next.next();
            if next.is_null() {
                break;
            }
        }
        current = current.next();
        if current.is_null() {
            break;
        }
    }
    if !handle_coincidence(contour_list, &mut coincidence) {
        return None;
    }
    // construct closed contours
    let mut result = Path::new();
    result.fill_type = fill_type;
    {
        let mut wrapper = PathWriter::new(&mut result);
        let ok = if builder.xor_mask() == WINDING_PATH_OPS_MASK {
            bridge_winding(contour_list, &mut wrapper)
        } else {
            bridge_xor(contour_list, &mut wrapper)
        };
        if !ok {
            return None;
        }
        wrapper.assemble(); // if some edges could not be resolved, assemble remaining
    }
    drop(builder);
    drop(coincidence);
    drop(state);
    Some(result)
}
