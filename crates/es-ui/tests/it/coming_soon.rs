use es_testkit::Headless;
use es_ui::assets::Assets;
use es_ui::components::{ComingSoon, Feature};
use es_ui::icons::IconName;
use es_ui::theme::tokens;
use es_ui::tokens::ThemeMode;
use gpui::{AppContext, Context, IntoElement, Render, Rgba, Window};
use gpui_component::Root;
use image::RgbaImage;

const W: f32 = 460.;
const H: f32 = 300.;

struct Host(Feature);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        ComingSoon::new("coming-soon", self.0).settled()
    }
}

fn picture(feature: Feature, mode: ThemeMode, name: &str) -> (RgbaImage, Rgba, Rgba) {
    let mut colours = None;
    let mut app = Headless::open(Assets, es_ui::init, W, H, |window, cx| {
        es_ui::theme::set_mode(mode, cx);
        let t = tokens(cx);
        colours = Some((Rgba::from(t.accent), Rgba::from(t.dropdown)));
        let view = cx.new(|_| Host(feature));
        cx.new(|cx| Root::new(view, window, cx))
    });
    let (accent, panel) = colours.expect("the window read the theme");
    (app.screenshot(name), accent, panel)
}

fn near(pixel: &image::Rgba<u8>, colour: Rgba) -> bool {
    let want = [colour.r, colour.g, colour.b].map(|c| (c * 255.).round() as i16);
    pixel.0[..3]
        .iter()
        .zip(want)
        .all(|(&got, want)| (i16::from(got) - want).abs() <= 6)
}

fn count(image: &RgbaImage, colour: Rgba) -> usize {
    image.pixels().filter(|p| near(p, colour)).count()
}

#[test]
fn each_feature_names_itself_and_says_what_it_will_do() {
    assert_eq!(Feature::VideoStudio.name(), "Video Studio");
    assert_eq!(
        Feature::VideoStudio.line(),
        "Edit recordings: cut, zoom, captions and backgrounds, then export."
    );
    assert_eq!(Feature::VideoStudio.icon(), IconName::Film);
    assert_eq!(Feature::badge(), "Coming soon");
    assert_eq!(
        Feature::VideoStudio.label(),
        "Video Studio, coming soon",
        "a screen reader hears the feature and that it is not here yet"
    );
}

#[test]
fn the_panel_draws_its_badge_on_its_card_in_light_and_dark() {
    for (mode, tag) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        {
            let (feature, name) = (Feature::VideoStudio, "video-studio");
            let (image, accent, panel) =
                picture(feature, mode, &format!("coming-soon-{name}-{tag}"));
            let scale = image.width() as f32 / W;
            assert!(
                count(&image, panel) as f32 > 300. * 150. * scale * scale,
                "{name} {tag}: the card fills the middle of the scrim"
            );
            assert!(
                count(&image, accent) as f32 > 60. * 14. * scale * scale,
                "{name} {tag}: the Coming soon badge is a solid accent pill"
            );
        }
    }
}
