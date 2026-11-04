use super::*;

// The start delay is part of the start: a screenshot frozen then would end up in the video.
#[gpui::test]
fn a_screenshot_waits_for_the_recording_to_start_then_runs_alongside_it(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.slow.borrow_mut().push(CaptureKind::Recording);
    let recording = rig.submit(cx, CaptureKind::Recording);
    let refused = rig.submit(cx, CaptureKind::Fullscreen);
    assert_eq!(rig.finished(), [(refused, JobStatus::Error)]);

    rig.finish_pipeline(cx);
    assert_eq!(rig.finished(), [(recording, JobStatus::Ok)]);
    let shot = rig.submit(cx, CaptureKind::Fullscreen);
    assert_eq!(rig.finished(), [(shot, JobStatus::Ok)]);
}

#[gpui::test]
fn the_replaced_request_s_cancel_leaves_the_capture_that_replaced_it(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.submit(cx, CaptureKind::Region);
    let second = rig.submit(cx, CaptureKind::Window);
    assert_eq!(rig.finished(), [(first, JobStatus::Cancelled)]);

    assert!(
        rig.coordinator.cancel(first),
        "answered, so nothing is cancelled"
    );
    cx.run_until_parked();
    assert_eq!(rig.screen.open.get(), 1);
    rig.release(cx, 0);
    assert_eq!(rig.finished(), [(second, JobStatus::Ok)]);
}

#[gpui::test]
fn the_same_request_switches_away_from_a_mode_the_user_changed_to(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.submit(cx, CaptureKind::Region);
    rig.screen.showing.set(Some(SelectMode::Window));
    rig.submit(cx, CaptureKind::Region);
    assert_eq!(
        rig.screen.open.get(),
        1,
        "Region shows the region view again"
    );
    assert_eq!(rig.log().last().unwrap(), "To(Region) on frame 1");
    rig.screen.showing.set(Some(SelectMode::Window));
    rig.submit(cx, CaptureKind::Window);
    assert_eq!(rig.screen.open.get(), 0, "Window closes the window view");
    assert!(rig.finished().contains(&(first, JobStatus::Cancelled)));
}

#[gpui::test]
fn a_capture_that_closed_the_bar_waits_for_it_to_leave_the_screen(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.coordinator.set_before_capture(Rc::new(|_| true));
    rig.submit(cx, CaptureKind::Region);
    assert_eq!(rig.screen.freezes.get(), 0);
    cx.executor().advance_clock(UNMAP_SETTLE);
    cx.run_until_parked();
    assert_eq!(rig.screen.freezes.get(), 1);
}

#[gpui::test]
fn the_bar_s_area_pick_is_taken_over_like_any_pick(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let area = cx.update(|cx| rig.coordinator.select_area(cx));
    cx.run_until_parked();
    let window = rig.submit(cx, CaptureKind::Window);
    assert!(matches!(area.now_or_never(), Some(Err(Error::Cancelled))));
    assert_eq!(rig.screen.freezes.get(), 1, "the same frozen frame");
    rig.release(cx, 0);
    assert!(rig.finished().contains(&(window, JobStatus::Ok)));

    let area = cx.update(|cx| rig.coordinator.select_area(cx));
    cx.run_until_parked();
    let region = rig.submit(cx, CaptureKind::Region);
    assert_eq!(rig.screen.open.get(), 0, "the mode shown closes it");
    assert!(matches!(area.now_or_never(), Some(Err(Error::Cancelled))));
    let finished = rig.finished();
    assert!(
        finished.contains(&(region, JobStatus::Ok)),
        "{finished:?} {region:?}"
    );
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn a_refused_bar_area_pick_shows_one_toast_and_no_error(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let _claim = rig.coordinator.claim().unwrap();
    let area = cx.update(|cx| rig.coordinator.select_area(cx));
    assert!(matches!(area.now_or_never(), Some(Err(Error::Cancelled))));
    assert_eq!(rig.busy_toasts(), 1);
    assert_eq!(rig.toasts.shown().len(), 1);
}
