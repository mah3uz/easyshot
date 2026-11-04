#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::gallery_support::*;

use es_shell::gallery::{Category, ViewMode};
use es_ui::tokens::ThemeMode;
use gpui::{Modifiers, ScrollDelta, ScrollEvent, point, px};

const DEFAULT: u32 = 96;

fn wheel(rig: &mut Rig, lines: f32, control: bool) {
    let handle = rig.app.window.into();
    let event = gpui::PlatformInput::Scroll(ScrollEvent {
        position: point(px(600.), px(400.)),
        delta: ScrollDelta::Lines(point(0., lines)),
        modifiers: Modifiers {
            control,
            ..Modifiers::default()
        },
        ..ScrollEvent::default()
    });
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
        .unwrap();
    rig.app.settle();
    rig.draw();
}

fn columns_at(rig: &mut Rig, size: u32) -> usize {
    rig.set_item_size(size);
    rig.draw();
    rig.read(|g| g.columns())
}

#[test]
fn bigger_previews_fit_fewer_columns_and_the_largest_leaves_two_at_the_default_width() {
    let mut rig = rig();
    assert_eq!(rig.item_size(), DEFAULT);
    assert_eq!(rig.read(|g| g.columns()), 5);
    assert_eq!(columns_at(&mut rig, 64), 6, "the smallest is the old grid");
    assert_eq!(columns_at(&mut rig, 256), 2);
    assert_eq!(columns_at(&mut rig, 9999), 2, "the size is clamped");
    assert_eq!(rig.item_size(), 256);

    let mut wide = rig_with(SCREENSHOTS, VIDEOS, ThemeMode::Dark, 1600., 900.);
    assert_eq!(columns_at(&mut wide, 256), 4);
    let mut narrow = rig_with(SCREENSHOTS, VIDEOS, ThemeMode::Dark, 780., 560.);
    assert_eq!(columns_at(&mut narrow, 256), 1);
    assert_eq!(columns_at(&mut narrow, 64), 4);
}

#[test]
fn the_size_is_saved_in_the_config_and_the_next_gallery_opens_at_it() {
    let mut rig = rig();
    rig.set_item_size(176);
    let settings = rig.settings.clone();
    rig.app
        .cx
        .update(|cx| settings.update(cx, |s, cx| s.flush(cx)));
    let text = std::fs::read_to_string(rig.paths.config_file()).unwrap();
    assert!(text.contains("[gallery]\nitem_size = 176"), "{text}");

    let Rig { _dir, paths, .. } = rig;
    let mut again = rig_in(_dir, paths, ThemeMode::Dark, 1080., 740.);
    assert_eq!(again.item_size(), 176);
    assert_eq!(again.read(|g| g.columns()), 3);
}

#[test]
fn ctrl_wheel_over_the_grid_resizes_without_scrolling_and_a_plain_wheel_scrolls() {
    let mut rig = rig();
    wheel(&mut rig, 1., true);
    assert_eq!(
        rig.item_size(),
        DEFAULT + 16,
        "wheel up makes previews bigger"
    );
    wheel(&mut rig, -2., true);
    assert_eq!(rig.item_size(), DEFAULT - 16);
    assert_eq!(
        rig.read(|g| g.grid_scrolled()),
        0.,
        "Ctrl+wheel never scrolls"
    );

    wheel(&mut rig, -3., false);
    assert_eq!(rig.item_size(), DEFAULT - 16);
    assert!(rig.read(|g| g.grid_scrolled()) > 0.);
}

#[test]
fn ctrl_plus_and_minus_step_the_size_and_ctrl_0_restores_the_default() {
    let mut rig = rig();
    rig.press("ctrl-=");
    assert_eq!(rig.item_size(), 112);
    rig.press("ctrl-+");
    assert_eq!(rig.item_size(), 128);
    rig.press("ctrl--");
    rig.press("ctrl--");
    rig.press("ctrl--");
    assert_eq!(rig.item_size(), 80);
    rig.press("ctrl-0");
    assert_eq!(rig.item_size(), DEFAULT);
}

#[test]
fn list_view_ignores_the_size_and_never_decodes_large_previews() {
    let mut rig = rig();
    rig.update(|g, cx| g.set_view(ViewMode::List, cx));
    rig.set_item_size(256);
    rig.press("ctrl-0");
    wheel(&mut rig, 2., true);
    assert_eq!(
        rig.item_size(),
        256,
        "keys and Ctrl+wheel act on the grid only"
    );
    rig.wait_for_thumbnails(20);
    assert_eq!(rig.read_sharp(), 0, "24 px list icons load the small cache");
    rig.press("down");
    rig.press("down");
    assert_eq!(rig.read(|g| g.selected_index()), Some(1));
}

#[test]
fn large_previews_load_a_larger_cached_thumbnail() {
    let mut rig = rig();
    rig.wait_for_thumbnails(10);
    rig.set_item_size(256);
    rig.draw();
    rig.wait_for_sharp_thumbnails(4);
    let big = std::fs::read_dir(rig.paths.thumbs_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with("-512.png"))
        .count();
    assert!(big >= 4, "{big} thumbnails cached at 512 px");
}

fn pictures(mode: ThemeMode, tag: &str) {
    let mut rig = rig_with(SCREENSHOTS, VIDEOS, mode, 1080., 740.);
    rig.press("right");
    rig.press("right");
    for (name, size) in [("small", 64), ("default", DEFAULT), ("large", 256)] {
        rig.set_item_size(size);
        rig.draw();
        rig.wait_for_sharp_thumbnails(4);
        rig.picture(&format!("gallery-size-{name}-{tag}"));
    }
    rig.update(|g, cx| g.set_category(Category::Videos, cx));
    rig.update(|g, cx| g.set_sort(es_shell::gallery::SortOrder::OldestFirst, cx));
    rig.picture(&format!("gallery-size-videos-large-{tag}"));
    rig.set_item_size(64);
    rig.picture(&format!("gallery-size-videos-small-{tag}"));
}

#[test]
fn size_pictures_dark() {
    pictures(ThemeMode::Dark, "dark");
}

#[test]
fn size_pictures_light() {
    pictures(ThemeMode::Light, "light");
}
