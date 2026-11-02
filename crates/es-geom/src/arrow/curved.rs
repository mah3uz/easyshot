use super::shared::{Relationship, arc_info, relationship, terminals};
use super::straight;
use super::types::{
    ArrowBody, ArrowInfo, ArrowInput, ArrowPoint, Arrowhead, BOUND_ARROW_OFFSET, Bound,
    MIN_ARROW_LENGTH, WAY_TOO_BIG_ARROW_BEND_FACTOR,
};
use crate::mat::Mat;
use crate::math::{PI2, clockwise_angle_dist, counter_clockwise_angle_dist, is_safe_float};
use crate::vec::PVec;
use std::f64::consts::PI;

fn angle_dist(clockwise: bool, a0: f64, a1: f64) -> f64 {
    if clockwise {
        clockwise_angle_dist(a0, a1)
    } else {
        counter_clockwise_angle_dist(a0, a1)
    }
}

fn on_circle(center: PVec, radius: f64, angle: f64) -> PVec {
    PVec::add(center, PVec::from_angle(angle, radius))
}

pub(super) fn resolve(input: &ArrowInput) -> ArrowInfo {
    let bend = input.bend;
    if bend.abs() > (bend * WAY_TOO_BIG_ARROW_BEND_FACTOR).abs() {
        return straight::resolve(input);
    }

    let (term_start, term_end) = terminals(input);
    let med = PVec::med(term_start, term_end);
    let distance = PVec::sub(term_end, term_start);
    let u = if distance.len() != 0.0 {
        distance.uni()
    } else {
        distance
    };
    let middle = PVec::add(med, PVec::mul(u.per(), -bend));

    let sw = input.stroke_width;
    let start_bound = input.start_bound.as_ref();
    let end_bound = input.end_bound.as_ref();

    let a = term_start;
    let b = term_end;
    let c = middle;

    if PVec::equals(a, b) {
        return ArrowInfo {
            start: ArrowPoint {
                handle: a,
                point: a,
                head: input.head_start,
            },
            end: ArrowPoint {
                handle: b,
                point: b,
                head: input.head_end,
            },
            middle: c,
            body: ArrowBody::Straight { start: a, end: b },
            handle_arc: None,
            stroke_width: sw,
            is_valid: false,
        };
    }

    let clockwise = bend < 0.0;
    let dist = |a0: f64, a1: f64| angle_dist(clockwise, a0, a1);

    let handle_arc = arc_info(a, b, c);
    let handle_aca = handle_arc.center.angle(a);
    let handle_acb = handle_arc.center.angle(b);
    let handle_dab = dist(handle_aca, handle_acb);

    if handle_arc.length == 0.0
        || handle_arc.size == 0.0
        || !is_safe_float(handle_arc.length)
        || !is_safe_float(handle_arc.size)
    {
        return straight::resolve(input);
    }

    let center = handle_arc.center;
    let radius = handle_arc.radius;
    let mut temp_a = a;
    let mut temp_b = b;
    let mut temp_c = c;

    let mut offset_a = 0.0;
    let mut offset_b = 0.0;
    let mut min_length = MIN_ARROW_LENGTH;
    let mut start_hit = false;
    let mut end_hit = false;

    if let Some(s) = start_bound
        && !s.exact
        && let Some(point) = clip_terminal(&Clip {
            bound: s,
            arrow: input.transform,
            this_end: temp_a,
            other_end: temp_b,
            center,
            radius,
            clockwise,
            target_fraction: 0.25,
            measure_from: None,
        })
    {
        temp_a = input
            .transform
            .inverse()
            .apply_to_point(s.transform.apply_to_point(point));
        start_hit = true;
        if input.head_start != Arrowhead::None {
            let stroke_offset = sw / 2.0 + s.stroke_width / 2.0;
            offset_a = BOUND_ARROW_OFFSET + stroke_offset;
            min_length += stroke_offset;
        }
    }

    if let Some(e) = end_bound
        && !e.exact
        && let Some(point) = clip_terminal(&Clip {
            bound: e,
            arrow: input.transform,
            this_end: temp_b,
            other_end: temp_a,
            center,
            radius,
            clockwise,
            target_fraction: 0.75,
            measure_from: Some(temp_a),
        })
    {
        temp_b = input
            .transform
            .inverse()
            .apply_to_point(e.transform.apply_to_point(point));
        end_hit = true;
        if input.head_end != Arrowhead::None {
            let stroke_offset = sw / 2.0 + e.stroke_width / 2.0;
            offset_b = BOUND_ARROW_OFFSET + stroke_offset;
            min_length += stroke_offset;
        }
    }

    let mut aca = center.angle(temp_a);
    let mut acb = center.angle(temp_b);
    let mut dab = dist(aca, acb);
    let mut lab = dab * radius;
    let side = if clockwise { 1.0 } else { -1.0 };

    let shifted_a = |offset: f64, dab: f64, lab: f64, aca: f64| {
        on_circle(center, radius, aca + dab * ((offset / lab) * side))
    };
    let shifted_b = |offset: f64, dab: f64, lab: f64, acb: f64| {
        on_circle(center, radius, acb + dab * ((offset / lab) * -side))
    };

    let mut t_a = temp_a;
    let mut t_b = temp_b;
    if offset_a != 0.0 {
        t_a = shifted_a(offset_a, dab, lab, aca);
    }
    if offset_b != 0.0 {
        t_b = shifted_b(offset_b, dab, lab, acb);
    }

    if PVec::dist_min(t_a, t_b, min_length) {
        if offset_a != 0.0 && offset_b != 0.0 {
            offset_a *= -1.5;
            offset_b *= -1.5;
        } else if offset_a != 0.0 {
            offset_a *= -2.0;
        } else if offset_b != 0.0 {
            offset_b *= -2.0;
        }

        let min_offset_a = 0.1 - dist(handle_aca, aca) * radius;
        let min_offset_b = 0.1 - dist(acb, handle_acb) * radius;
        offset_a = offset_a.max(min_offset_a);
        offset_b = offset_b.max(min_offset_b);
    }

    if offset_a != 0.0 {
        temp_a = shifted_a(offset_a, dab, lab, aca);
    }
    if offset_b != 0.0 {
        temp_b = shifted_b(offset_b, dab, lab, acb);
    }

    if let (Some(s), Some(e)) = (start_bound, end_bound)
        && !s.exact
        && !e.exact
    {
        aca = center.angle(temp_a);
        acb = center.angle(temp_b);
        dab = dist(aca, acb);
        lab = dab * radius;
        let rel = relationship(input);

        if rel == Relationship::DoubleBound && lab < 30.0 {
            temp_a = a;
            temp_b = b;
            temp_c = c;
        } else if rel == Relationship::Safe {
            if !start_hit {
                temp_a = a;
            }
            if !end_hit || dist(handle_aca, aca) > dist(handle_aca, acb) {
                let t = (MIN_ARROW_LENGTH / lab).min(0.9) * side;
                temp_b = on_circle(center, radius, aca + dab * t);
            }
        }
    }

    place_center_handle(
        center,
        radius,
        &mut temp_a,
        &mut temp_b,
        &mut temp_c,
        handle_dab,
        clockwise,
    );

    if PVec::equals(temp_a, temp_b) {
        temp_a = PVec::add_xy(temp_c, 1.0, 1.0);
        temp_b = PVec::sub_xy(temp_c, 1.0, 1.0);
    }

    let body_arc = arc_info(temp_a, temp_b, temp_c);

    ArrowInfo {
        start: ArrowPoint {
            handle: term_start,
            point: temp_a,
            head: input.head_start,
        },
        end: ArrowPoint {
            handle: term_end,
            point: temp_b,
            head: input.head_end,
        },
        middle: temp_c,
        body: ArrowBody::Arc(body_arc),
        handle_arc: Some(handle_arc),
        stroke_width: sw,
        is_valid: body_arc.length != 0.0 && body_arc.center.is_finite(),
    }
}

struct Clip<'a, 'b> {
    bound: &'b Bound<'a>,
    arrow: Mat,
    this_end: PVec,
    other_end: PVec,
    center: PVec,
    radius: f64,
    clockwise: bool,
    target_fraction: f64,
    /// Where angles are measured from when this is the end terminal.
    measure_from: Option<PVec>,
}

/// Intersect the arrow's circle with the bound shape and pick the crossing that best fits where
/// this end belongs. The result is in the bound shape's space.
fn clip_terminal(clip: &Clip) -> Option<PVec> {
    let bound = clip.bound;
    let inverse = bound.transform.inverse();
    let to_local = |p: PVec| inverse.apply_to_point(clip.arrow.apply_to_point(p));
    let this_local = to_local(clip.this_end);
    let other_local = to_local(clip.other_end);
    let center = to_local(clip.center);

    let (local_start, local_end) = match clip.measure_from {
        Some(start) => (to_local(start), this_local),
        None => (this_local, other_local),
    };
    let dist = |a0: f64, a1: f64| angle_dist(clip.clockwise, a0, a1);

    let mut crossings = bound.geometry.intersect_circle(center, clip.radius);
    let mut point = None;

    if !crossings.is_empty() {
        let angle_start = center.angle(local_start);
        let angle_end = center.angle(local_end);
        let dab = dist(angle_start, angle_end);
        crossings.retain(|p| dist(angle_start, center.angle(*p)) <= dab);

        let along = |p: &PVec| dist(angle_start, center.angle(*p));
        if bound.geometry.is_closed() {
            let target = dab * clip.target_fraction;
            crossings.sort_by(|p, q| {
                (along(p) - target)
                    .abs()
                    .total_cmp(&(along(q) - target).abs())
            });
        } else {
            crossings.sort_by(|p, q| along(p).total_cmp(&along(q)));
        }
        point = crossings.first().copied();
    }

    if point.is_none() {
        if bound.geometry.is_closed() {
            let nearest = bound.geometry.nearest_point(this_local);
            if PVec::dist_min(nearest, this_local, 1.0) {
                point = Some(nearest);
            }
        } else {
            point = Some(this_local);
        }
    }
    point
}

/// Put the middle handle halfway along the body arc. If the body arc has outgrown the handle arc
/// the ends have swapped, so swap them back and put the handle on the other side.
fn place_center_handle(
    center: PVec,
    radius: f64,
    temp_a: &mut PVec,
    temp_b: &mut PVec,
    temp_c: &mut PVec,
    original_arc: f64,
    clockwise: bool,
) {
    let aca = center.angle(*temp_a);
    let acb = center.angle(*temp_b);
    let mut dab = clockwise_angle_dist(aca, acb);
    if !clockwise {
        dab = PI2 - dab;
    }

    let half = if clockwise { 0.5 } else { -0.5 };
    *temp_c = on_circle(center, radius, aca + dab * half);

    if dab > original_arc {
        *temp_c = PVec::rot_with(*temp_c, center, PI);
        std::mem::swap(temp_a, temp_b);
    }
}
