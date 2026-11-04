use crate::support::{self, WIDTH};
use es_ui::components::Toast;
use es_ui::icons::IconName;
use es_ui::tokens::ThemeMode;
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px};

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
