use es_shell::preview::{self, Preview};
use es_testkit::Headless;
use es_ui::components::Toast;
use es_ui::icons::IconName;
use es_ui::{assets::Assets, theme, tokens::ThemeMode};
use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::Root;

fn gradient(width: u32, height: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x * 255 / width) as u8, (y * 255 / height) as u8, 180, 255])
    })
}

fn open_preview(mode: ThemeMode) -> (Headless<Root>, Entity<Preview>) {
    let image = gradient(1600, 900);
    let pixels = image.dimensions();
    let mut bgra = image;
    for px in bgra.pixels_mut() {
        px.0.swap(0, 2);
    }
    let render = std::sync::Arc::new(gpui::RenderImage::new([image::Frame::new(bgra)]));
    let mut host = None;
    let app = Headless::open(
        Assets,
        move |cx| {
            es_ui::init(cx);
            theme::set_mode(mode, cx);
        },
        900.,
        560.,
        |window, cx| {
            let view = cx.new(|cx| Preview::new(render, pixels, window, cx));
            host = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    (app, host.expect("the window built its view"))
}

#[test]
fn the_window_fits_the_image_on_the_screen_and_never_enlarges_it() {
    assert_eq!(
        preview::initial_size((400, 300), (1920., 1080.)),
        (400., 300.)
    );
    let (w, h) = preview::initial_size((3840, 2160), (1920., 1080.));
    assert!(
        (w - 1536.).abs() < 0.5 && (h - 864.).abs() < 0.5,
        "{w} x {h}"
    );
    assert_eq!(
        preview::initial_size((20, 10), (1920., 1080.)),
        (360., 240.)
    );
}

#[test]
fn the_preview_draws_fitted_and_at_actual_size_in_light_and_dark() {
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let (mut app, view) = open_preview(mode);
        let fitted = app.screenshot(&format!("preview-fit-{name}"));
        app.cx
            .update(|cx| view.update(cx, |v, cx| v.toggle_actual_size(cx)));
        let actual = app.screenshot(&format!("preview-actual-{name}"));
        assert_ne!(
            fitted.as_raw(),
            actual.as_raw(),
            "100% looks different from fit"
        );
    }
}

struct ToastShot;

impl Render for ToastShot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = es_ui::theme::tokens(cx).clone();
        div().size_full().p(px(24.)).bg(t.window).child(
            Toast::new("undo-toast", "Moved to the Trash")
                .icon(IconName::Trash)
                .action("Undo", |_, _| {})
                .settled(),
        )
    }
}

#[test]
fn the_undo_toast_draws_in_light_and_dark() {
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut app = Headless::open(
            Assets,
            move |cx| {
                es_ui::init(cx);
                theme::set_mode(mode, cx);
            },
            420.,
            110.,
            |window, cx| {
                let view = cx.new(|_| ToastShot);
                cx.new(|cx| Root::new(view, window, cx))
            },
        );
        app.screenshot(&format!("undo-toast-{name}"));
    }
}
