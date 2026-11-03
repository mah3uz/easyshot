use super::{open_editor_with, rig};
use es_editor::session::style::DELAY;
use es_model::{StylePreset, Tool};
use es_store::presets;
use gpui::TestAppContext;

#[gpui::test]
fn the_editor_the_daemon_opens_starts_from_the_saved_style_and_saves_changes(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let mut file = presets::open_style(&rig.paths).unwrap();
    file.value = StylePreset {
        tool: Tool::Arrow,
        stroke_width: 11.0,
        ..StylePreset::default()
    };
    file.save().unwrap();
    let editor = open_editor_with(&rig, |_| {}, cx);
    let doc = editor.read_with(cx, |w, _| w.session().unwrap().doc.clone());
    let (tool, width) = doc.read_with(cx, |d, _| {
        let e = d.editor();
        (e.tool(), e.current_style().stroke_width)
    });
    assert_eq!((tool, width), (Tool::Arrow, 11.0));

    let row = editor.read_with(cx, |w, _| w.toolbar().unwrap());
    cx.update(|cx| row.update(cx, |r, cx| r.pick_tool(Tool::Ellipse, cx)));
    cx.executor().advance_clock(DELAY * 2);
    cx.run_until_parked();
    let saved = presets::open_style(&rig.paths).unwrap().value;
    assert_eq!((saved.tool, saved.stroke_width), (Tool::Ellipse, 11.0));
}
