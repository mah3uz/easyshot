use es_testkit::Headless;
use es_ui::{assets::Assets, hello};
use gpui::CursorHideMode;

#[test]
fn hello_window_renders_its_stack() {
    let mut app = Headless::open(Assets, hello::init, 480.0, 320.0, hello::root);
    let image = app.screenshot("hello");
    assert_eq!((image.width(), image.height()), (960, 640));

    let background = image.get_pixel(2, 2).0;
    let differing = image.pixels().filter(|p| p.0 != background).count();
    assert!(
        differing > 4000,
        "frame is nearly uniform: {differing} pixels differ from the background"
    );

    let names = app.cx.update(|cx| cx.text_system().all_font_names());
    for family in ["Inter", "Inter Tight", "Nunito"] {
        assert!(
            names.iter().any(|n| n == family),
            "{family} is not registered"
        );
    }
}

#[test]
fn key_presses_never_hide_the_pointer() {
    let mut app = Headless::open(Assets, hello::init, 480.0, 320.0, hello::root);
    assert_eq!(
        app.cx.update(|cx| cx.cursor_hide_mode()),
        CursorHideMode::Never
    );
}
