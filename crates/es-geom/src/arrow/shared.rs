use super::types::{ArcInfo, ArrowInput, Bound, NORMALIZED_ANCHOR_EPSILON};
use crate::math::{PI2, center_of_circle_from_three_points};
use crate::vec::PVec;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Relationship {
    Safe,
    DoubleBound,
    StartContainsEnd,
    EndContainsStart,
}

pub(super) fn relationship(input: &ArrowInput) -> Relationship {
    let (Some(start), Some(end)) = (&input.start_bound, &input.end_bound) else {
        return Relationship::Safe;
    };
    if start.id == end.id {
        return Relationship::DoubleBound;
    }
    if start.page_bounds.contains(end.page_bounds) {
        return Relationship::StartContainsEnd;
    }
    if end.page_bounds.contains(start.page_bounds) {
        return Relationship::EndContainsStart;
    }
    Relationship::Safe
}

fn terminal(input: &ArrowInput, bound: &Bound, force_precise: bool) -> PVec {
    let bounds = bound.geometry.bounds();
    let anchor = if bound.precise || force_precise {
        let e = NORMALIZED_ANCHOR_EPSILON;
        PVec::new(
            bound.normalized_anchor.x.clamp(e, 1.0 - e),
            bound.normalized_anchor.y.clamp(e, 1.0 - e),
        )
    } else {
        PVec::new(0.5, 0.5)
    };
    let local = PVec::add(
        bounds.point(),
        PVec::mul_v(anchor, PVec::new(bounds.w, bounds.h)),
    );
    let page = bound.transform.apply_to_point(local);
    input.transform.inverse().apply_to_point(page)
}

/// Both terminals in arrow space; an unbound end uses the arrow's own point.
pub(super) fn terminals(input: &ArrowInput) -> (PVec, PVec) {
    let rel = relationship(input);
    let start = match &input.start_bound {
        Some(b) => terminal(
            input,
            b,
            matches!(
                rel,
                Relationship::DoubleBound | Relationship::StartContainsEnd
            ),
        ),
        None => input.start,
    };
    let end = match &input.end_bound {
        Some(b) => terminal(
            input,
            b,
            matches!(
                rel,
                Relationship::DoubleBound | Relationship::EndContainsStart
            ),
        ),
        None => input.end,
    };
    (start, end)
}

pub(super) fn arc_info(a: PVec, b: PVec, c: PVec) -> ArcInfo {
    let center = center_of_circle_from_three_points(a, b, c).unwrap_or_else(|| PVec::med(a, b));
    let radius = PVec::dist(center, a);
    let sweep = PVec::clockwise(a, c, b);

    let ab = PVec::dist(a, b);
    let bc = PVec::dist(b, c);
    let ca = PVec::dist(c, a);
    let theta = ((bc * bc + ca * ca - ab * ab) / (2.0 * bc * ca)).acos() * 2.0;

    let large_arc = PI > theta;
    let size = (PI2 - theta) * if sweep { 1.0 } else { -1.0 };
    ArcInfo {
        center,
        radius,
        size,
        length: size * radius,
        large_arc,
        sweep,
    }
}
