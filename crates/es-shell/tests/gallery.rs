#![allow(clippy::unwrap_used, clippy::expect_used)]

mod gallery_support;

use es_shell::gallery::{Category, GalleryEvent, MediaKind, SortOrder, ViewMode};
use gallery_support::*;
use gpui::{EntityInputHandler, ScrollDelta, ScrollEvent, point, px};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

#[test]
fn sections_show_only_their_kind_and_count_it() {
    let mut rig = rig();
    assert_eq!(rig.read(|g| g.shown().len()), SCREENSHOTS + VIDEOS);
    assert_eq!(rig.read(|g| g.count(Category::Screenshots)), SCREENSHOTS);
    assert_eq!(rig.read(|g| g.count(Category::Videos)), VIDEOS);

    rig.update(|g, cx| g.set_category(Category::Videos, cx));
    let kinds: Vec<MediaKind> = rig.read(|g| g.shown().iter().map(|i| i.kind).collect());
    assert_eq!(kinds.len(), VIDEOS);
    assert!(kinds.iter().all(|k| *k == MediaKind::Video));

    rig.update(|g, cx| g.set_category(Category::Screenshots, cx));
    let shown = rig.read(|g| g.shown().to_vec());
    assert_eq!(shown.len(), SCREENSHOTS);
    assert!(shown.iter().all(|i| i.kind == MediaKind::Screenshot));
}

#[test]
fn typing_in_search_keeps_names_that_contain_it_in_any_case() {
    let mut rig = rig();
    let (handle, search) = (rig.app.window.into(), rig.read(|g| g.search_field()));
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            search.update(cx, |input, cx| {
                input.replace_text_in_range(None, "iNvOiCe", window, cx)
            })
        })
        .unwrap();
    rig.app.settle();
    let names: Vec<String> = rig.read(|g| g.shown().iter().map(|i| i.name.clone()).collect());
    assert_eq!(names.len(), INVOICES, "{names:?}");
    assert!(names.iter().all(|n| n.starts_with("Invoice")));

    rig.update(|g, cx| g.set_category(Category::Videos, cx));
    assert!(
        rig.read(|g| g.shown().is_empty()),
        "search and section combine"
    );
}

#[test]
fn each_sort_order_orders_the_items_it_names() {
    let mut rig = rig();
    let shown = |rig: &mut Rig| rig.read(|g| g.shown().to_vec());

    let newest = shown(&mut rig);
    assert!(
        newest
            .windows(2)
            .all(|w| w[0].created_at >= w[1].created_at)
    );
    assert_eq!(
        newest[0].name,
        name_of(0),
        "the newest capture leads by default"
    );

    rig.update(|g, cx| g.set_sort(SortOrder::OldestFirst, cx));
    let oldest = shown(&mut rig);
    assert!(
        oldest
            .windows(2)
            .all(|w| w[0].created_at <= w[1].created_at)
    );
    assert_eq!(oldest.last().unwrap().name, name_of(0));

    rig.update(|g, cx| g.set_sort(SortOrder::Name, cx));
    let by_name: Vec<String> = shown(&mut rig)
        .into_iter()
        .map(|i| i.name.to_lowercase())
        .collect();
    assert!(by_name.windows(2).all(|w| w[0] <= w[1]), "{by_name:?}");

    rig.update(|g, cx| g.set_sort(SortOrder::Kind, cx));
    let by_kind = shown(&mut rig);
    let first_video = by_kind
        .iter()
        .position(|i| i.kind == MediaKind::Video)
        .unwrap();
    assert_eq!(first_video, SCREENSHOTS, "screenshots, then videos");
    assert!(
        by_kind[..SCREENSHOTS]
            .windows(2)
            .all(|w| w[0].created_at >= w[1].created_at)
    );
}

#[test]
fn arrow_keys_move_by_one_cell_and_by_one_grid_row() {
    let mut rig = rig();
    let columns = rig.read(|g| g.columns());
    assert_eq!(columns, 6, "1080 px window, 210 px sidebar, 112 px cells");
    let at = |rig: &mut Rig| rig.read(|g| g.selected_index());

    rig.press("right");
    assert_eq!(at(&mut rig), Some(0), "the first key picks the first item");
    rig.press("right");
    rig.press("down");
    assert_eq!(at(&mut rig), Some(1 + columns));
    rig.press("left");
    rig.press("up");
    assert_eq!(at(&mut rig), Some(0));
    rig.press("up");
    assert_eq!(at(&mut rig), Some(0), "the top row stays put");
    rig.press("end");
    assert_eq!(at(&mut rig), Some(SCREENSHOTS + VIDEOS - 1));
    rig.press("home");
    assert_eq!(at(&mut rig), Some(0));

    rig.update(|g, cx| g.set_view(ViewMode::List, cx));
    rig.press("down");
    assert_eq!(at(&mut rig), Some(1), "a list row is one item");
}

#[test]
fn enter_opens_and_edit_hands_the_file_to_the_app() {
    let mut rig = rig();
    let seen: Rc<RefCell<Vec<GalleryEvent>>> = Rc::default();
    let (sink, gallery) = (seen.clone(), rig.gallery.clone());
    rig.app.cx.update(|cx| {
        cx.subscribe(&gallery, move |_, e: &GalleryEvent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
    });
    rig.press("down");
    rig.press("enter");
    let first = rig.read(|g| g.shown()[0].clone());
    rig.update(|g, cx| g.edit(first.id, cx));
    let video = rig.read(|g| {
        g.shown()
            .iter()
            .find(|i| i.kind == MediaKind::Video)
            .cloned()
            .unwrap()
    });
    rig.update(|g, cx| g.edit(video.id, cx));
    assert_eq!(
        *seen.borrow(),
        vec![
            GalleryEvent::Open(first.file.clone()),
            GalleryEvent::Edit(first.file)
        ],
        "a video is not opened in the image editor"
    );
}

#[test]
fn trash_moves_the_capture_its_sidecar_and_base_and_drops_the_entry() {
    let mut rig = rig();
    let item = rig.read(|g| g.shown()[0].clone());
    let (sidecar, base) = (
        es_store::document::sidecar_path(&item.file),
        es_store::library::base_path(&item.file),
    );
    assert!(item.file.exists() && sidecar.exists() && base.exists());

    rig.update(|g, cx| g.trash(item.id, cx)).unwrap();

    assert_eq!(
        *rig.files.trashed.lock().unwrap(),
        vec![item.file.clone(), sidecar.clone(), base.clone()]
    );
    assert!(!item.file.exists() && !sidecar.exists() && !base.exists());
    assert_eq!(rig.read(|g| g.shown().len()), SCREENSHOTS + VIDEOS - 1);
    assert!(rig.read(|g| g.shown().iter().all(|i| i.id != item.id)));
    let history = rig.history.clone();
    assert!(
        rig.app
            .cx
            .update(|cx| history.read(cx).get(item.id).is_none())
    );
    let index = std::fs::read_to_string(rig.paths.history_index()).unwrap();
    assert!(
        !index.contains(&item.id.to_string()),
        "the index on disk forgets it too"
    );
}

#[test]
fn show_in_folder_asks_the_file_manager_for_the_capture_itself() {
    let mut rig = rig();
    let item = rig.read(|g| g.shown()[3].clone());
    rig.update(|g, cx| g.show_in_folder(item.id, cx));
    assert_eq!(*rig.files.shown.lock().unwrap(), vec![item.file]);
    assert_eq!(rig.read(|g| g.error().map(str::to_owned)), None);
}

#[test]
fn a_file_deleted_behind_the_gallery_reports_it_instead_of_opening() {
    let mut rig = rig();
    let item = rig.read(|g| g.shown()[2].clone());
    std::fs::remove_file(&item.file).unwrap();
    rig.update(|g, cx| g.show_in_folder(item.id, cx));
    assert!(rig.files.shown.lock().unwrap().is_empty());
    assert!(rig.read(|g| g.error().is_some()));
    rig.update(|g, cx| g.refresh(cx));
    assert!(rig.read(|g| g.shown().iter().all(|i| i.id != item.id)));
}

fn pictures(mode: es_ui::tokens::ThemeMode, tag: &str) {
    let mut rig = rig_with(SCREENSHOTS, VIDEOS, mode, 1080., 740.);
    rig.wait_for_thumbnails(30);
    rig.press("right");
    rig.press("right");
    rig.picture(&format!("gallery-grid-{tag}"));
    rig.update(|g, cx| g.set_view(ViewMode::List, cx));
    rig.wait_for_thumbnails(30);
    rig.picture(&format!("gallery-list-{tag}"));

    let mut empty = rig_with(0, 0, mode, 1080., 740.);
    empty.picture(&format!("gallery-empty-{tag}"));
}

#[test]
fn pictures_dark() {
    pictures(es_ui::tokens::ThemeMode::Dark, "dark");
}

#[test]
fn pictures_light() {
    pictures(es_ui::tokens::ThemeMode::Light, "light");
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// Scrolls past the end of the 34-row grid and back, timing the event and the frame it draws.
fn scroll_through(rig: &mut Rig) -> Vec<f64> {
    let handle = rig.app.window.into();
    let over = point(px(600.), px(400.));
    let mut times = Vec::new();
    for step in 0..260 {
        let down = if step < 130 { -40. } else { 40. };
        let event = gpui::PlatformInput::Scroll(ScrollEvent {
            position: over,
            delta: ScrollDelta::Pixels(point(px(0.), px(down))),
            ..ScrollEvent::default()
        });
        let took = rig
            .app
            .cx
            .update_window(handle, |_, window, cx| {
                let t = Instant::now();
                window.dispatch_event(event, cx);
                window.draw(cx).clear(cx);
                t.elapsed()
            })
            .unwrap();
        rig.app.settle();
        times.push(took.as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    times
}

#[test]
#[ignore = "a timing bench; run alone with --run-ignored only"]
fn scrolling_the_grid_of_200_items_keeps_frame_cpu_low() {
    let mut rig = rig();
    rig.wait_for_thumbnails(30);
    let cold = scroll_through(&mut rig);
    // Every row drew at least once, so every screenshot asked for its thumbnail.
    rig.wait_for_thumbnails(SCREENSHOTS);
    let warm = scroll_through(&mut rig);
    for (pass, times) in [("thumbnails arriving", &cold), ("thumbnails loaded", &warm)] {
        let (p50, p95) = (percentile(times, 0.5), percentile(times, 0.95));
        eprintln!("gallery grid scroll, 200 items, {pass}: p50 {p50:.2} ms, p95 {p95:.2} ms");
    }
    let p95 = percentile(&warm, 0.95);
    assert!(p95 <= 8.0, "p95 {p95:.2} ms");
}
