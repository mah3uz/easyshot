#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::shell::{Fixture, launch_seeded};

use es_anno::StylePatch;
use es_editor::session::style::DELAY;
use es_editor::toolbar::Control;
use es_model::{Arrowhead, StylePreset, Swatch, Tool};
use es_store::{Paths, presets};
use std::collections::BTreeMap;

fn seed(preset: StylePreset) -> impl FnOnce(&Paths) {
    move |paths| {
        let mut file = presets::open_style(paths).unwrap();
        file.value = preset;
        file.save().unwrap();
    }
}

fn open(seed: impl FnOnce(&Paths)) -> Fixture {
    launch_seeded(BTreeMap::new(), seed)
}

fn current(f: &Fixture) -> StylePreset {
    f.session_doc()
        .read_with(&f.app.cx, |d, _| d.editor().preset())
}

fn on_disk(f: &Fixture) -> StylePreset {
    presets::open_style(&f.paths).unwrap().value
}

fn apply(f: &mut Fixture, patch: StylePatch) {
    let row = f.row();
    f.app
        .cx
        .update(|cx| row.update(cx, |r, cx| r.apply_style(patch, None, cx)));
    f.app.settle();
}

#[test]
fn a_saved_style_is_what_a_new_editor_starts_with() {
    let saved = StylePreset {
        tool: Tool::Arrow,
        swatch: Swatch::custom(10, 200, 30, 255),
        stroke_width: 9.0,
        arrowhead_start: Arrowhead::Dot,
        highlighter_straight: true,
        ..StylePreset::default()
    };
    let f = open(seed(saved.clone()));
    assert_eq!(current(&f), saved);
}

#[test]
fn without_a_file_the_editor_starts_from_the_defaults() {
    let f = open(|_| {});
    assert_eq!(current(&f), StylePreset::default());
}

#[test]
fn a_changed_style_is_saved_after_the_delay_and_not_before() {
    let mut f = open(|_| {});
    apply(&mut f, StylePatch::StrokeWidth(12.0));
    f.wait(DELAY / 2);
    assert_eq!(
        on_disk(&f).stroke_width,
        StylePreset::default().stroke_width
    );
    f.wait(DELAY);
    assert_eq!(on_disk(&f).stroke_width, 12.0);
}

#[test]
fn a_burst_of_changes_saves_the_last_one() {
    let mut f = open(|_| {});
    for width in [5.0, 6.0, 7.0] {
        apply(&mut f, StylePatch::StrokeWidth(width));
    }
    f.wait(DELAY * 2);
    assert_eq!(on_disk(&f).stroke_width, 7.0);
}

#[test]
fn an_open_editor_keeps_its_style_when_another_saves() {
    let mut f = open(|_| {});
    apply(&mut f, StylePatch::StrokeWidth(3.0));
    let mut other = presets::open_style(&f.paths).unwrap();
    other.value.stroke_width = 20.0;
    other.save().unwrap();
    f.wait(DELAY / 2);
    assert_eq!(current(&f).stroke_width, 3.0);
    f.wait(DELAY);
    assert_eq!(on_disk(&f).stroke_width, 3.0, "the last change wins");
}

#[test]
fn reset_returns_the_armed_tool_family_to_the_defaults_and_saves() {
    let saved = StylePreset {
        tool: Tool::Rectangle,
        swatch: Swatch::custom(10, 200, 30, 255),
        stroke_width: 15.0,
        ..StylePreset::default()
    };
    let mut f = open(seed(saved));
    f.click(Control::ResetStyle);
    let defaults = StylePreset::default();
    let now = current(&f);
    assert_eq!(
        (now.swatch.id.clone(), now.stroke_width),
        (defaults.swatch.id.clone(), defaults.stroke_width)
    );
    f.wait(DELAY * 2);
    assert_eq!(on_disk(&f).stroke_width, defaults.stroke_width);
    assert_eq!(on_disk(&f).swatch.id, defaults.swatch.id);
}

#[test]
fn a_newer_file_is_not_used_and_never_overwritten() {
    let body = r#"{"format":"easyshot.style","version":9,"stroke_width":17.0}"#;
    let mut f = open(|paths| std::fs::write(paths.style_file(), body).unwrap());
    assert_eq!(current(&f), StylePreset::default());
    apply(&mut f, StylePatch::StrokeWidth(12.0));
    f.wait(DELAY * 2);
    assert_eq!(std::fs::read_to_string(f.paths.style_file()).unwrap(), body);
}

#[test]
fn a_damaged_file_is_not_used_and_never_overwritten() {
    let body = "{ not json";
    let mut f = open(|paths| std::fs::write(paths.style_file(), body).unwrap());
    assert_eq!(current(&f), StylePreset::default());
    apply(&mut f, StylePatch::StrokeWidth(12.0));
    f.wait(DELAY * 2);
    assert_eq!(std::fs::read_to_string(f.paths.style_file()).unwrap(), body);
}
