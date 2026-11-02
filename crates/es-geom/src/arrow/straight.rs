use super::shared::{Relationship, relationship, terminals};
use super::types::{
    ArrowBody, ArrowInfo, ArrowInput, ArrowPoint, Arrowhead, BOUND_ARROW_OFFSET, Bound,
    MIN_ARROW_LENGTH,
};
use crate::mat::Mat;
use crate::vec::PVec;

pub(super) fn resolve(input: &ArrowInput) -> ArrowInfo {
    let (term_start, term_end) = terminals(input);
    let mut a = term_start;
    let mut b = term_end;

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
            middle: PVec::med(a, b),
            body: ArrowBody::Straight { start: a, end: b },
            handle_arc: None,
            stroke_width: input.stroke_width,
            is_valid: false,
        };
    }

    let u_ab = PVec::sub(b, a).uni();
    let start_bound = input.start_bound.as_ref();
    let end_bound = input.end_bound.as_ref();

    let end_hit = clip(&mut b, term_start, input.transform, end_bound);
    let start_hit = clip(&mut a, term_end, input.transform, start_bound);

    let mut offset_a = 0.0;
    let mut offset_b = 0.0;
    let mut min_length = MIN_ARROW_LENGTH;

    let sw = input.stroke_width;
    let self_intersection = matches!((start_bound, end_bound), (Some(s), Some(e)) if s.id == e.id);
    let rel = relationship(input);

    if rel == Relationship::Safe
        && let (Some(s), Some(e)) = (start_bound, end_bound)
        && !self_intersection
        && !s.exact
        && !e.exact
    {
        if end_hit && !start_hit {
            if s.geometry.is_closed() {
                a = PVec::add(b, PVec::mul(u_ab, MIN_ARROW_LENGTH));
            }
        } else if !end_hit && e.geometry.is_closed() {
            b = PVec::sub(a, PVec::mul(u_ab, MIN_ARROW_LENGTH));
        }
    }

    let distance = PVec::sub(b, a);
    let u = if distance.len() != 0.0 {
        distance.uni()
    } else {
        distance
    };
    let flipped = !PVec::equals(u, u_ab);
    let dir = if flipped { -1.0 } else { 1.0 };

    if !self_intersection {
        if rel != Relationship::StartContainsEnd
            && let Some(s) = start_bound
            && input.head_start != Arrowhead::None
            && !s.exact
        {
            let stroke_offset = sw / 2.0 + s.stroke_width / 2.0;
            offset_a = BOUND_ARROW_OFFSET + stroke_offset;
            min_length += stroke_offset;
        }
        if rel != Relationship::EndContainsStart
            && let Some(e) = end_bound
            && input.head_end != Arrowhead::None
            && !e.exact
        {
            let stroke_offset = sw / 2.0 + e.stroke_width / 2.0;
            offset_b = BOUND_ARROW_OFFSET + stroke_offset;
            min_length += stroke_offset;
        }
    }

    let t_a = PVec::add(a, PVec::mul(u, offset_a * dir));
    let t_b = PVec::sub(b, PVec::mul(u, offset_b * dir));

    if PVec::dist_min(t_a, t_b, min_length) {
        if offset_a != 0.0 && offset_b != 0.0 {
            offset_a *= -1.5;
            offset_b *= -1.5;
        } else if offset_a != 0.0 {
            offset_a *= -1.0;
        } else if offset_b != 0.0 {
            offset_b *= -1.0;
        }
    }

    a = PVec::add(a, PVec::mul(u, offset_a * dir));
    b = PVec::sub(b, PVec::mul(u, offset_b * dir));

    let middle = if flipped {
        if start_bound.is_some() && end_bound.is_some() {
            b = PVec::add(a, PVec::mul(u, -MIN_ARROW_LENGTH));
        }
        PVec::med(term_start, term_end)
    } else {
        PVec::med(a, b)
    };

    ArrowInfo {
        start: ArrowPoint {
            handle: term_start,
            point: a,
            head: input.head_start,
        },
        end: ArrowPoint {
            handle: term_end,
            point: b,
            head: input.head_end,
        },
        middle,
        body: ArrowBody::Straight { start: a, end: b },
        handle_arc: None,
        stroke_width: input.stroke_width,
        is_valid: PVec::dist(a, b) > 0.0,
    }
}

/// Pull `point` back to where the segment from `opposite` crosses the bound shape, or onto its
/// edge when it comes within one unit. Returns whether the shape was hit.
fn clip(point: &mut PVec, opposite: PVec, arrow: Mat, bound: Option<&Bound>) -> bool {
    let Some(bound) = bound else { return false };
    if bound.exact {
        return false;
    }
    let inverse = bound.transform.inverse();
    let to_local = |p: PVec| inverse.apply_to_point(arrow.apply_to_point(p));
    let from = to_local(opposite);
    let to = to_local(*point);

    let hit = bound
        .geometry
        .intersect_line_segment(from, to)
        .into_iter()
        .min_by(|p, q| PVec::dist2(*p, from).total_cmp(&PVec::dist2(*q, from)));
    let hit = match hit {
        Some(h) => h,
        None => {
            let nearest = bound.geometry.nearest_point(to);
            if !PVec::dist_min(nearest, to, 1.0) {
                return false;
            }
            nearest
        }
    };
    *point = arrow
        .inverse()
        .apply_to_point(bound.transform.apply_to_point(hit));
    true
}
