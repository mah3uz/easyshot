#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::shell::{Fixture, launch_image};
use es_editor::Job;
use es_editor::session::preview::{PREVIEW_EDGE, PREVIEW_PIXELS};
use image::{Rgba, RgbaImage};
use std::time::Duration;

fn open(width: u32, height: u32, forced: bool) -> Fixture {
    let image = RgbaImage::from_pixel(width, height, Rgba([200, 120, 40, 255]));
    let mut f = launch_image(Default::default(), |_| {}, image);
    if forced {
        let mut config = es_model::Config::default();
        config.general.low_res_preview = true;
        f.editor
            .update(&mut f.app.cx, |w, cx| w.apply_config(&config, cx));
    }
    f.wait(Duration::from_millis(50));
    f.app.cx.run_until_parked();
    f.app.settle();
    f
}

fn sizes(f: &Fixture) -> ((u32, u32), (u32, u32)) {
    f.session_doc().read_with(&f.app.cx, |d, _| {
        (d.base().dimensions(), d.shown().dimensions())
    })
}

fn low_res(f: &Fixture) -> bool {
    f.editor.read_with(&f.app.cx, |w, _| w.is_low_res())
}

#[test]
fn a_very_large_capture_is_shown_downscaled_and_exported_at_full_size() {
    let mut f = open(9000, 6000, false);
    let (base, shown) = sizes(&f);
    assert_eq!(base, (9000, 6000));
    assert!(shown.0.max(shown.1) <= PREVIEW_EDGE, "{shown:?}");
    assert!(
        u64::from(shown.0) * u64::from(shown.1) <= PREVIEW_PIXELS,
        "{shown:?}"
    );
    assert!(shown.0 < base.0);
    assert!(low_res(&f), "the footer banner follows");

    let target = f.dir.path().join("big.jpg");
    *f.chooser.answer.borrow_mut() = Some(target.clone());
    f.request(Job::Export);
    let exported = image::ImageReader::open(&target)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap();
    assert_eq!(exported, (9000, 6000));
}

#[test]
fn a_small_capture_is_shown_at_full_size_without_the_banner() {
    let f = open(1200, 640, false);
    let (base, shown) = sizes(&f);
    assert_eq!(base, shown);
    assert!(!low_res(&f));
}

#[test]
fn the_setting_forces_the_preview_on_a_small_capture_and_turning_it_off_restores_it() {
    let mut f = open(1200, 640, true);
    assert_eq!(sizes(&f), ((1200, 640), (600, 320)));
    assert!(low_res(&f));

    let off = es_model::Config::default();
    f.editor
        .update(&mut f.app.cx, |w, cx| w.apply_config(&off, cx));
    f.wait(Duration::from_millis(50));
    let (base, shown) = sizes(&f);
    assert_eq!(base, shown);
    assert!(!low_res(&f));
}

#[test]
fn a_crop_of_a_previewed_capture_is_previewed_again_from_the_new_base() {
    let mut f = open(1200, 640, true);
    let doc = f.session_doc();
    f.app.cx.update(|cx| {
        doc.update(cx, |d, cx| {
            d.replace_base(RgbaImage::from_pixel(400, 200, Rgba([1, 2, 3, 255])), cx)
        })
    });
    f.app.cx.run_until_parked();
    f.wait(Duration::from_millis(50));
    assert_eq!(sizes(&f), ((400, 200), (200, 100)));
}
