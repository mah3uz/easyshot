use crate::support::{self, WIDTH};
use es_ui::components::Toast;
use es_ui::icons::IconName;
use es_ui::tokens::{ThemeMode, Typeset as _};
use gpui::{AppContext as _, Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(8.)).child(
            Toast::new("toast", "Moved to the Trash")
                .icon(IconName::Trash)
                .action("Undo", |_, _| {})
                .settled(),
        )
    }
}

/// Window pixels of card background between the title's last letter and the button's left border,
/// read from a band through the card's middle where the border is the first full-height line.
fn space_before_button(name: &str, mode: ThemeMode) -> f32 {
    let (mut app, _) = support::open(70., |_, cx| {
        es_ui::theme::set_mode(mode, cx);
        Host
    });
    let image = app.screenshot(name);
    let scale = image.width() as f32 / WIDTH;
    let band = (30. * scale) as u32..(40. * scale) as u32;
    let card = *image.get_pixel((20. * scale) as u32, band.start);
    let inked = |x: u32| {
        band.clone()
            .filter(|&y| *image.get_pixel(x, y) != card)
            .count()
    };
    let inside = (20. * scale) as u32;
    let border = (inside..image.width())
        .find(|&x| inked(x) == band.len())
        .expect("the button's border crosses the band");
    let letter = (inside..border)
        .rev()
        .find(|&x| inked(x) > 0)
        .expect("the title sits before the button");
    (border - letter - 1) as f32 / scale
}

#[test]
fn the_toast_button_keeps_a_row_gap_from_the_title() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let space = space_before_button(&format!("toast-action-{tag}"), mode);
        assert!(
            (16.0..22.0).contains(&space),
            "Undo sits a settings row's 16 px from the title, not tight against it: {space} px ({tag})"
        );
    }
}

struct ShareStates(&'static str);

impl Render for ShareStates {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let card = match self.0 {
            "preparing" => Toast::new("toast", "Preparing…")
                .icon(IconName::Info)
                .progress(0.3)
                .action("Cancel", |_, _| {}),
            "uploading" => Toast::new("toast", "Link copied")
                .message("Uploading 42 %")
                .icon(IconName::Info)
                .progress(0.42)
                .action("Cancel", |_, _| {}),
            "shared" => Toast::new("toast", "Shared")
                .message("Expires in 7 days")
                .icon(IconName::Check)
                .action("Open", |_, _| {})
                .action("Copy Link", |_, _| {}),
            _ => Toast::new("toast", "Couldn’t share")
                .message("Can’t reach the share service. Check your connection and try again.")
                .icon(IconName::TriangleAlert)
                .style(es_ui::components::ToastStyle::Error)
                .action("Retry", |_, _| {}),
        };
        div().size_full().p(px(8.)).child(card.settled())
    }
}

#[test]
fn a_share_toast_draws_its_progress_bar_and_every_button() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut bars = Vec::new();
        for state in ["preparing", "uploading", "shared", "failed"] {
            let (mut app, _) = support::open(120., move |_, cx| {
                es_ui::theme::set_mode(mode, cx);
                ShareStates(state)
            });
            let image = app.screenshot(&format!("toast-share-{state}-{tag}"));
            let accent = app
                .cx
                .update(|cx| gpui::Rgba::from(es_ui::theme::tokens(cx).accent));
            let want = [accent.r, accent.g, accent.b].map(|c| (c * 255.).round() as i16);
            let inked = image
                .pixels()
                .filter(|p| {
                    p.0[..3]
                        .iter()
                        .zip(want)
                        .all(|(&g, w)| (i16::from(g) - w).abs() <= 6)
                })
                .count();
            bars.push(inked);
        }
        assert!(
            bars[0] > 0 && bars[1] > bars[0],
            "the bar fills as the upload goes: {bars:?} ({tag})"
        );
        assert_eq!(bars[2], 0, "a finished share has no bar ({tag})");
    }
}

/// The toast surface is 420 wide; the card reaches its own maximum width there.
#[test]
fn the_share_toasts_at_the_surface_width() {
    for (mode, tag) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")]
        .into_iter()
        .flat_map(|m| ["shared", "failed"].map(|state| (m, state)))
        .map(|((mode, tag), state)| (mode, (tag, state)))
    {
        let (tag, state) = tag;
        let mut app = es_testkit::Headless::open(
            es_ui::assets::Assets,
            es_ui::init,
            420.,
            120.,
            |window, cx| {
                es_ui::theme::set_mode(mode, cx);
                let view = cx.new(|_| ShareStates(state));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        );
        app.screenshot(&format!("toast-share-{state}-wide-{tag}"));
    }
}

const QUOTA: &str = "You have shared as much as the service allows today. Try again in 12 min.";

struct Failed;

impl Render for Failed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(8.)).child(
            Toast::new("toast", "Couldn’t share")
                .message(QUOTA)
                .icon(IconName::TriangleAlert)
                .style(es_ui::components::ToastStyle::Error)
                .action("Retry", |_, _| {})
                .settled(),
        )
    }
}

/// The message on one unclipped line, in the toast's own type and colours.
struct Reference;

impl Render for Reference {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = es_ui::theme::tokens(cx).clone();
        div().size_full().bg(t.dropdown).p(px(8.)).child(
            div()
                .typeset(t.type_scale.label)
                .text_color(t.text_secondary)
                .child(QUOTA),
        )
    }
}

const K: f32 = 2.;

fn shot<V: Render>(
    width: f32,
    name: &str,
    mode: ThemeMode,
    view: impl FnOnce() -> V + 'static,
) -> image::RgbaImage {
    let mut app = es_testkit::Headless::open(
        es_ui::assets::Assets,
        es_ui::init,
        width,
        150.,
        move |window, cx| {
            es_ui::theme::set_mode(mode, cx);
            let view = cx.new(|_| view());
            cx.new(|cx| gpui_component::Root::new(view, window, cx))
        },
    );
    let image = app.screenshot(name);
    assert_eq!(
        image.width() as f32,
        width * K,
        "pictures are drawn at twice the size"
    );
    image
}

fn differs(a: &image::Rgba<u8>, b: &image::Rgba<u8>) -> bool {
    let d: i16 = a.0[..3]
        .iter()
        .zip(&b.0[..3])
        .map(|(&x, &y)| (i16::from(x) - i16::from(y)).abs())
        .sum();
    d > 60
}

/// Window points of every pixel unlike `bg` inside the box.
fn ink(
    image: &image::RgbaImage,
    bg: image::Rgba<u8>,
    x: (f32, f32),
    y: (f32, f32),
) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    for py in (y.0 * K) as u32..((y.1 * K) as u32).min(image.height()) {
        for px in (x.0 * K) as u32..((x.1 * K) as u32).min(image.width()) {
            if differs(image.get_pixel(px, py), &bg) {
                out.push((px as f32 / K, py as f32 / K));
            }
        }
    }
    out
}

#[test]
fn a_long_failure_wraps_in_full_inside_the_toast_and_left_of_its_button() {
    for (mode, tag) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let reference = shot(
            900.,
            &format!("toast-share-failed-reference-{tag}"),
            mode,
            || Reference,
        );
        let wanted = ink(
            &reference,
            *reference.get_pixel(4, 4),
            (0., 900.),
            (0., 150.),
        )
        .len();
        for width in [300., 420.] {
            let name = format!("toast-share-failed-long-{}-{tag}", width as u32);
            let toast = shot(width, &name, mode, || Failed);
            let card = *toast.get_pixel((14. * K) as u32, (30. * K) as u32);
            let bottom = ((30. * K) as u32..toast.height())
                .find(|&y| differs(toast.get_pixel((14. * K) as u32, y), &card))
                .map(|y| y as f32 / K)
                .expect("the card's bottom edge");
            // A run of 20 pt down one column inside the card is the button's border; no glyph is that tall.
            let tall = |x: u32| {
                let mut run = 0;
                (0..(bottom * K) as u32).any(|y| {
                    run = if differs(toast.get_pixel(x, y), &card) {
                        run + 1
                    } else {
                        0
                    };
                    run as f32 / K >= 20.
                })
            };
            let button = ((150. * K) as u32..toast.width())
                .find(|&x| tall(x))
                .map(|x| x as f32 / K)
                .expect("the Retry button's border");
            // The text column starts after the 16 pt padding, the 32 pt icon box and the 10 pt gap,
            // and the message after the title's line.
            let message = ink(&toast, card, (8. + 58., button - 1.), (38., bottom));
            assert!(
                message.len() * 10 > wanted * 9,
                "{tag} at {width}: every glyph of the message is drawn ({} of {wanted} inked pixels)",
                message.len()
            );
            assert!(
                message.iter().all(|&(x, _)| x < button - 8.),
                "{tag} at {width}: the message stays clear of Retry"
            );
        }
    }
}
