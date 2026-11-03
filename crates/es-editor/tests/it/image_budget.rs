#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::sync::Arc;

use canvas_support::{App, canvas, open, opened, redraw};
use es_app::image_budget::{ImageBudget, ImageStores};
use es_model::BackgroundSettings;

fn released(app: &App) -> usize {
    canvas(app).read_with(&app.cx, |c, _| c.last_frame().images_released)
}

fn budget_of(app: &mut App) -> Arc<ImageBudget> {
    app.cx
        .update(|cx| cx.global::<ImageStores>().budget.clone())
}

/// A 5000 px wide image is two tiles, 4.9 MB and 1.1 MB.
fn app_with_cap(cap: usize) -> App {
    let (mut app, _) = open(
        1400.0,
        700.0,
        opened(5000, 300, Vec::new(), BackgroundSettings::default()),
    );
    app.cx.update(|cx| {
        cx.set_global(ImageStores {
            budget: Arc::new(ImageBudget::new(cap)),
            ..ImageStores::default()
        });
    });
    app
}

#[test]
fn painted_tiles_are_counted_against_the_budget() {
    let mut app = app_with_cap(256 * 1024 * 1024);
    redraw(&mut app);
    assert_eq!(budget_of(&mut app).used(), 5000 * 300 * 4);
}

#[test]
fn an_image_the_budget_gave_up_is_released_and_built_again() {
    let mut app = app_with_cap(5_500_000);
    redraw(&mut app);
    assert_eq!(
        released(&app),
        0,
        "nothing is dropped until the owner hears of it"
    );
    redraw(&mut app);
    assert!(
        released(&app) >= 1,
        "the evicted tile went back to the window"
    );
    let used = budget_of(&mut app).used();
    assert!(
        used <= 5_500_000 + 5000 * 300 * 4 / 2,
        "only what is held is counted: {used}"
    );
}
