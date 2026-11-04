use super::*;

#[test]
fn a_long_window_menu_stays_within_the_output_and_scrolls_to_its_last_row() {
    let mut s = Surface::open(WIDTH as f32, 480., |_| {
        RecordingOptions::new(vec![output("A", 0., 1280., 720.)], devices())
    });
    let (events, _sub) = s.record::<OptionsEvent>();
    let view = s.view.clone();
    let many: Vec<WindowInfo> = (0..60)
        .map(|i| placed(&format!("w{i}"), Some("1"), true))
        .collect();
    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.set_windows(many, cx)));
    s.click(WINDOW_X, ROW_Y);
    let height = s.app.cx.update(|cx| view.read(cx).size().height);
    assert!(height <= 720. * 0.6, "the panel grew to {height}");

    let scrolled = s
        .app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.scroll_menu("end", cx)));
    assert!(scrolled);
    let offset = s.app.cx.update(|cx| view.read(cx).menu_offset());
    assert!(offset > 0., "End did not scroll the menu");
    s.click(LESS_X, 350.);
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::RefreshWindows)
    ));

    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.scroll_menu("home", cx)));
    let top = s.app.cx.update(|cx| view.read(cx).menu_offset());
    assert_eq!(top, 0.);
}
