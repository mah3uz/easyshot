use es_testkit::Headless;
use es_ui::{
    assets::Assets,
    components::gallery::{self, WIDTH},
    theme,
    tokens::{Appearance, ThemeMode},
};
use gpui_component::ActiveTheme;

fn near(pixel: [u8; 4], rgb: u32) -> bool {
    let want = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    pixel[..3]
        .iter()
        .zip(want)
        .all(|(&got, want)| got.abs_diff(want) <= 2)
}

#[test]
fn every_section_draws_in_dark_and_light_side_by_side() {
    for section in gallery::sections() {
        let mut app = Headless::open(
            Assets,
            gallery::init,
            WIDTH,
            section.window_height(),
            |window, cx| gallery::root(vec![section], window, cx),
        );
        let image = app.screenshot(&format!("gallery-{}", section.name));
        let (w, h) = (image.width(), image.height());
        assert_eq!(w, (WIDTH * 2.) as u32, "headless windows draw at scale 2");

        let dark = image.get_pixel(4, 4).0;
        let light = image.get_pixel(w - 4, 4).0;
        assert!(
            near(dark, 0x1B1B1B),
            "{}: dark column is {dark:?}",
            section.name
        );
        assert!(
            near(light, 0xFFFFFF),
            "{}: light column is {light:?}",
            section.name
        );

        for (half, background) in [(0..w / 2, dark), (w / 2..w, light)] {
            let inked = half
                .clone()
                .flat_map(|x| (0..h).map(move |y| (x, y)))
                .filter(|&(x, y)| image.get_pixel(x, y).0 != background)
                .count();
            assert!(
                inked > 2000,
                "{}: a column is nearly empty ({inked} pixels drawn)",
                section.name
            );
        }

        let appearance = app.cx.update(|cx| theme::tokens(cx).appearance);
        assert_eq!(
            appearance,
            Appearance::Dark,
            "a scoped column must hand the window's own theme back"
        );
    }
}

#[test]
fn switching_mode_repaints_components_with_the_new_palette() {
    let mut app = Headless::open(Assets, gallery::init, 320., 200., |window, cx| {
        gallery::root(Vec::new(), window, cx)
    });
    let background = |app: &mut Headless<gpui_component::Root>| {
        app.cx.update(|cx| cx.theme().background.to_rgb())
    };
    let dark = background(&mut app);
    assert!(dark.r < 0.15, "starts dark");

    app.cx
        .update(|cx| theme::set_mode(ThemeMode::System(Some(Appearance::Light)), cx));
    app.settle();
    let light = background(&mut app);
    assert!(
        light.r > 0.99,
        "the component theme took the light window colour"
    );
    let tokens = app.cx.update(|cx| theme::tokens(cx).appearance);
    assert_eq!(tokens, Appearance::Light);
}
