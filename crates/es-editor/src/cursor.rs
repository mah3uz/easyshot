use es_anno::CursorKind;
use es_ui::cursors::Cursors;
use gpui::{App, CursorStyle, Global};

struct Registered(Cursors);

impl Global for Registered {}

/// The custom cursor images, registered with the app on first use.
pub fn cursors(cx: &mut App) -> Cursors {
    if let Some(r) = cx.try_global::<Registered>() {
        return r.0;
    }
    let cursors = es_ui::cursors::register(cx);
    cx.set_global(Registered(cursors));
    cursors
}

pub fn style(kind: CursorKind, cursors: &Cursors) -> CursorStyle {
    match kind {
        CursorKind::Arrow => CursorStyle::Arrow,
        CursorKind::AnnotationPlus => CursorStyle::Custom(cursors.annotation_plus),
        CursorKind::OpenHand => CursorStyle::OpenHand,
        CursorKind::ClosedHand => CursorStyle::ClosedHand,
        CursorKind::IBeam => CursorStyle::IBeam,
        CursorKind::Crosshair => CursorStyle::Crosshair,
        CursorKind::Resize { screen_angle_deg } => resize_for(screen_angle_deg),
        CursorKind::Rotate { octant } => {
            CursorStyle::Custom(cursors.rotate[usize::from(octant % 8)])
        }
    }
}

/// `degrees` is clockwise from screen +x with y down, so 45° points down and to the right.
pub fn resize_for(degrees: f32) -> CursorStyle {
    let octant = if degrees.is_finite() {
        ((degrees / 45.0).round() as i32).rem_euclid(4)
    } else {
        0
    };
    match octant {
        0 => CursorStyle::ResizeLeftRight,
        1 => CursorStyle::ResizeUpLeftDownRight,
        2 => CursorStyle::ResizeUpDown,
        _ => CursorStyle::ResizeUpRightDownLeft,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resize_direction_snaps_to_the_nearest_standard_cursor_whichever_way_it_points() {
        let cases = [
            (0.0, CursorStyle::ResizeLeftRight),
            (180.0, CursorStyle::ResizeLeftRight),
            (21.0, CursorStyle::ResizeLeftRight),
            (45.0, CursorStyle::ResizeUpLeftDownRight),
            (225.0, CursorStyle::ResizeUpLeftDownRight),
            (90.0, CursorStyle::ResizeUpDown),
            (269.0, CursorStyle::ResizeUpDown),
            (135.0, CursorStyle::ResizeUpRightDownLeft),
            (315.0, CursorStyle::ResizeUpRightDownLeft),
            (359.0, CursorStyle::ResizeLeftRight),
            (-45.0, CursorStyle::ResizeUpRightDownLeft),
            (f32::NAN, CursorStyle::ResizeLeftRight),
        ];
        for (degrees, want) in cases {
            assert_eq!(resize_for(degrees), want, "{degrees}°");
        }
    }
}
