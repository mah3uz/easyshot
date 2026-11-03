use super::{open_editor_with, rig};
use gpui::TestAppContext;

#[gpui::test]
fn the_editor_the_daemon_opens_follows_the_low_resolution_setting(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(&rig, |_| {}, cx);
    let doc = editor.read_with(cx, |w, _| w.session().unwrap().doc.clone());
    assert!(!editor.read_with(cx, |w, _| w.is_low_res()));
    assert_eq!(
        doc.read_with(cx, |d, _| d.shown().dimensions()),
        (16, 12),
        "a small capture is shown as it is"
    );

    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.general.low_res_preview = true, cx)
        })
    });
    cx.run_until_parked();
    assert!(editor.read_with(cx, |w, _| w.is_low_res()));
    let (base, shown) = doc.read_with(cx, |d, _| (d.base().dimensions(), d.shown().dimensions()));
    assert_eq!((base, shown), ((16, 12), (8, 6)));

    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.general.low_res_preview = false, cx)
        })
    });
    cx.run_until_parked();
    assert!(!editor.read_with(cx, |w, _| w.is_low_res()));
}

#[gpui::test]
fn an_editor_opened_with_the_setting_on_starts_low_resolution(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(&rig, |c| c.general.low_res_preview = true, cx);
    cx.run_until_parked();
    assert!(editor.read_with(cx, |w, _| w.is_low_res()));
    let doc = editor.read_with(cx, |w, _| w.session().unwrap().doc.clone());
    assert_eq!(doc.read_with(cx, |d, _| d.shown().dimensions()), (8, 6));
}
