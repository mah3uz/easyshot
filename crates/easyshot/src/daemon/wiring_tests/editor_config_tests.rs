use super::*;

fn bound_command(
    key: &'static str,
    cx: &mut TestAppContext,
) -> Option<es_editor::keyboard::Command> {
    use es_editor::keyboard::{CONTEXT, Run};
    let keystroke = gpui::Keystroke::parse(key).ok()?;
    cx.update(|cx| {
        let context = [gpui::KeyContext::parse(CONTEXT).unwrap()];
        let (found, _) = cx
            .key_bindings()
            .borrow()
            .bindings_for_input(&[keystroke], &context);
        found
            .first()
            .and_then(|b| b.action().as_any().downcast_ref::<Run>())
            .map(|run| run.0)
    })
}

#[gpui::test]
fn the_editor_nudges_by_the_configured_steps(cx: &mut TestAppContext) {
    use es_editor::keyboard::Command;
    let rig = rig(cx);
    let _editor = open_editor_with(
        &rig,
        |c| {
            c.editor.nudge_step = 3;
            c.editor.nudge_step_shift = 25;
        },
        cx,
    );
    assert_eq!(bound_command("right", cx), Some(Command::Nudge(3.0, 0.0)));
    assert_eq!(
        bound_command("shift-up", cx),
        Some(Command::Nudge(0.0, -25.0))
    );
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.editor.nudge_step = 5, cx))
    });
    cx.run_until_parked();
    assert_eq!(bound_command("left", cx), Some(Command::Nudge(-5.0, 0.0)));
}

#[gpui::test]
fn the_editor_zooms_about_the_configured_anchor(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(&rig, |c| c.editor.zoom_anchor = "centre".into(), cx);
    let at_pointer = |cx: &mut TestAppContext| {
        editor.read_with(cx, |w, cx| {
            w.session().unwrap().viewport.read(cx).zoom_anchor_pointer()
        })
    };
    assert!(!at_pointer(cx), "a centre anchor was not applied on open");
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.editor.zoom_anchor = "pointer".into(), cx)
        })
    });
    cx.run_until_parked();
    assert!(
        at_pointer(cx),
        "a changed anchor did not reach the open editor"
    );
}

#[gpui::test]
fn smart_redaction_uses_the_configured_ocr_languages(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(
        &rig,
        |c| c.ocr.languages = vec!["deu".into(), "fra".into()],
        cx,
    );
    let languages =
        |cx: &mut TestAppContext| editor.read_with(cx, |w, _| w.ocr_options().languages.clone());
    assert_eq!(languages(cx), ["deu", "fra"]);
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.ocr.languages = vec!["spa".into()], cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(languages(cx), ["spa"]);
}
