use super::Handle;
use crate::CursorKind;
use crate::document::page_transform;
use crate::editor::Editor;
use es_geom::{PVec, Point};

/// Clockwise from the screen's +x axis, since screen y points down; in [0, 360).
fn degrees(v: PVec) -> f64 {
    v.y.atan2(v.x).to_degrees().rem_euclid(360.0)
}

fn sign(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v.signum() }
}

fn resize(v: PVec) -> CursorKind {
    CursorKind::Resize {
        screen_angle_deg: degrees(v) as f32,
    }
}

fn rotate(v: PVec) -> CursorKind {
    let octant = (degrees(v) / 45.0).round().rem_euclid(8.0);
    CursorKind::Rotate {
        octant: octant as u8,
    }
}

impl Editor {
    /// Resize points the way the handle pulls, so a corner reads diagonal on any frame.
    pub(super) fn handle_cursor(&self, handle: Handle) -> CursorKind {
        match handle {
            Handle::ArrowStart | Handle::ArrowEnd => CursorKind::Crosshair,
            Handle::ArrowMiddle => self.bend_cursor(),
            Handle::Rotate { .. } => {
                let bounds = self.selection_bounds();
                match (bounds, handle.position()) {
                    (Some(b), Some(corner)) => {
                        let (at, center) = self.rotate_point(&b, corner);
                        rotate(PVec::sub(at, center))
                    }
                    _ => CursorKind::Arrow,
                }
            }
            _ => {
                let (Some(b), Some(at)) = (self.selection_bounds(), handle.position()) else {
                    return CursorKind::Arrow;
                };
                let c = b.bbox.center();
                let pull = PVec::add(c, PVec::new(sign(at.x - 0.5), sign(at.y - 0.5)));
                let from = self.screen_of(b.transform.apply_to_point(c));
                let to = self.screen_of(b.transform.apply_to_point(pull));
                resize(PVec::sub(to, from))
            }
        }
    }

    /// The bend handle moves across the chord.
    fn bend_cursor(&self) -> CursorKind {
        let Some(shape) = self.lone_arrow() else {
            return CursorKind::Arrow;
        };
        let Some(info) = self.doc.arrow_info(shape.id) else {
            return CursorKind::Arrow;
        };
        let t = page_transform(shape);
        let start = self.screen_of(t.apply_to_point(info.start.handle));
        let end = self.screen_of(t.apply_to_point(info.end.handle));
        resize(PVec::sub(end, start).per())
    }

    pub(super) fn rotating_cursor(&self, center: PVec, screen: Point) -> CursorKind {
        rotate(PVec::sub(
            PVec::new(screen.x, screen.y),
            self.screen_of(center),
        ))
    }
}
