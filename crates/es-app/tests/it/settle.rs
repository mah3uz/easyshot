use es_app::capture_coordinator::{REPAINT_MARGIN, UNMAP_SETTLE, unmap_settle};
use std::time::Duration;

// On Hyprland the closed capture bar was still in the frozen frame 235 ms after it closed.
#[test]
fn a_grab_waits_out_the_close_animation_the_compositor_reports() {
    let fade = Duration::from_millis(270);
    assert_eq!(unmap_settle(Some(fade)), fade + REPAINT_MARGIN);
    assert!(unmap_settle(Some(fade)) > fade);
}

#[test]
fn a_compositor_that_does_not_animate_needs_only_its_next_repaint() {
    assert_eq!(unmap_settle(Some(Duration::ZERO)), REPAINT_MARGIN);
    assert!(REPAINT_MARGIN < UNMAP_SETTLE);
}

#[test]
fn a_compositor_that_cannot_say_gets_the_plain_wait() {
    assert_eq!(unmap_settle(None), UNMAP_SETTLE);
}
