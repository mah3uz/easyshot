#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod shell;

use es_editor::EditorWindowEvent;
use es_editor::keyboard::{Command, bindings, command_for, keystroke};
use es_editor::toolbar::{FlipAxis, Rotation, ToolRowEvent};
use es_model::Tool;
use es_protocol::{CATALOGUE, Scope, Trigger};
use shell::{Fixture, launch, launch_with};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

fn tool(f: &Fixture) -> Tool {
    f.session_doc()
        .read_with(&f.app.cx, |d, _| d.editor().tool())
}

fn shape_count(f: &Fixture) -> usize {
    f.session_doc()
        .read_with(&f.app.cx, |d, _| d.editor().doc().shapes().len())
}

fn selected(f: &Fixture) -> usize {
    f.session_doc()
        .read_with(&f.app.cx, |d, _| d.editor().selected_ids().len())
}

fn toolbar_events(f: &mut Fixture) -> Rc<RefCell<Vec<ToolRowEvent>>> {
    let seen: Rc<RefCell<Vec<ToolRowEvent>>> = Rc::default();
    let sink = seen.clone();
    let row = f.editor.read_with(&f.app.cx, |w, _| w.toolbar().unwrap());
    f.app.cx.update(|cx| {
        cx.subscribe(&row, move |_, e: &ToolRowEvent, _| {
            sink.borrow_mut().push(*e)
        })
        .detach()
    });
    seen
}

#[test]
fn every_catalogue_default_for_the_editor_has_a_binding() {
    let bound = bindings(&BTreeMap::new());
    for entry in CATALOGUE
        .iter()
        .filter(|e| e.scope == Scope::Editor && e.id.starts_with("image-"))
    {
        let Some(default) = entry.default_trigger else {
            continue;
        };
        let key = keystroke(&Trigger::parse(default).unwrap()).unwrap();
        let command =
            command_for(entry.id).unwrap_or_else(|| panic!("{} has no command", entry.id));
        assert!(
            bound.contains(&(key.clone(), command)),
            "{} ({key}) is not bound",
            entry.id
        );
    }
}

#[test]
fn tool_keys_pick_their_tool() {
    let mut f = launch();
    for (key, expected) in [
        ("o", Tool::Ellipse),
        ("r", Tool::Rectangle),
        ("l", Tool::Line),
        ("a", Tool::Arrow),
        ("1", Tool::NumberedCircle),
        ("t", Tool::Text),
        ("p", Tool::Pixelate),
        ("b", Tool::Blur),
        ("h", Tool::Select),
    ] {
        f.key(key);
        assert_eq!(tool(&f), expected, "key {key}");
    }
}

#[test]
fn save_and_copy_keys_ask_for_their_job() {
    let mut f = launch();
    f.key("ctrl-s");
    f.wait(std::time::Duration::from_secs(2));
    f.key("ctrl-shift-c");
    assert_eq!(
        *f.events.borrow(),
        vec![EditorWindowEvent::Save, EditorWindowEvent::Copy]
    );
}

#[test]
fn undo_redo_select_all_and_delete_keys() {
    let mut f = launch();
    f.edit();
    let can_undo = |f: &Fixture| {
        f.session_doc()
            .read_with(&f.app.cx, |d, _| d.editor().can_undo())
    };
    f.key("ctrl-z");
    assert!(!can_undo(&f));
    f.key("ctrl-shift-z");
    assert!(can_undo(&f));
    f.key("ctrl-z");
    f.key("ctrl-y");
    assert!(can_undo(&f), "Ctrl+Y redoes too");

    f.key("o");
    f.key("ctrl-a");
    assert_eq!(selected(&f), 2);
    assert_eq!(tool(&f), Tool::Select, "select all also picks Select");
    f.key("delete");
    assert_eq!(shape_count(&f), 0);
    f.key("ctrl-z");
    f.key("ctrl-a");
    f.key("backspace");
    assert_eq!(shape_count(&f), 0, "Backspace deletes too");
}

#[test]
fn zoom_keys_move_the_viewport() {
    let mut f = launch();
    let zoom = |f: &Fixture| f.editor.read_with(&f.app.cx, |w, cx| w.zoom_percent(cx));
    f.key("ctrl-0");
    assert_eq!(zoom(&f), 100);
    f.key("ctrl-=");
    let bigger = zoom(&f);
    assert!(bigger > 100);
    f.key("ctrl--");
    f.key("ctrl--");
    assert!(zoom(&f) < bigger);
    f.key("ctrl-1");
    assert_ne!(zoom(&f), 100, "Ctrl+1 fits the 1200 px image to the window");
}

#[test]
fn crop_key_emits_the_toolbars_crop_event() {
    let mut f = launch();
    let seen = toolbar_events(&mut f);
    f.key("c");
    assert_eq!(*seen.borrow(), vec![ToolRowEvent::Crop]);
}

#[test]
fn unbound_catalogue_actions_can_be_bound_by_the_user() {
    let mut f = launch_with(BTreeMap::from([
        ("image-rotate-left".into(), "Ctrl+Shift+L".into()),
        ("image-flip-vertical".into(), "Ctrl+Shift+V".into()),
        ("image-export".into(), "Ctrl+E".into()),
        ("image-highlight".into(), "G".into()),
    ]));
    let seen = toolbar_events(&mut f);
    f.key("ctrl-shift-l");
    f.key("ctrl-shift-v");
    assert_eq!(
        *seen.borrow(),
        vec![
            ToolRowEvent::Rotate(Rotation::Left),
            ToolRowEvent::Flip(FlipAxis::Vertical)
        ]
    );
    f.key("g");
    assert_eq!(tool(&f), Tool::Highlight);
    f.key("ctrl-e");
    assert_eq!(*f.events.borrow(), vec![EditorWindowEvent::Export]);
}

#[test]
fn held_keys_do_not_repeat_but_nudges_do() {
    let mut f = launch();
    f.key_down("o");
    assert_eq!(tool(&f), Tool::Ellipse);
    f.key_down("o");
    assert_eq!(
        tool(&f),
        Tool::Ellipse,
        "a repeat would have toggled back to Select"
    );
    f.key("o");
    f.key("ctrl-a");
    let x = |f: &Fixture| {
        f.session_doc()
            .read_with(&f.app.cx, |d, _| d.editor().doc().shapes()[0].x)
    };
    let start = x(&f);
    f.key_down("right");
    f.key_down("right");
    assert_eq!(x(&f) - start, 2.0);
}

#[test]
fn while_text_is_edited_only_save_passes_through() {
    let mut f = launch();
    let doc = f.session_doc();
    f.app.cx.update(|cx| {
        doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_tool(Tool::Text));
            d.apply(cx, |e| e.create_text(es_geom::Point::new(300.0, 400.0)));
        })
    });
    f.app.settle();
    let editing = f.editor.read_with(&f.app.cx, |w, cx| {
        w.canvas().unwrap().read(cx).is_editing_text(cx)
    });
    assert!(editing, "the fixture must be editing text");

    for key in ["r", "ctrl-a", "ctrl-shift-c", "ctrl-=", "ctrl-0"] {
        f.key(key);
    }
    assert_eq!(tool(&f), Tool::Text, "tool keys are typing, not shortcuts");
    assert!(f.events.borrow().is_empty());

    f.key("ctrl-s");
    assert_eq!(*f.events.borrow(), vec![EditorWindowEvent::Save]);

    let handled = |f: &mut Fixture, command| {
        let handle = f.app.window.into();
        let editor = f.editor.clone();
        f.app
            .cx
            .update_window(handle, |_, window, cx| {
                editor.update(cx, |w, cx| w.run_command(command, window, cx))
            })
            .unwrap()
    };
    assert!(
        !handled(&mut f, Command::Tool(Tool::Rectangle)),
        "the key must reach the text"
    );
    assert!(handled(&mut f, Command::Save));
}
