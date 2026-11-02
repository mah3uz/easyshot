mod support;

use es_protocol::{Modifiers, Trigger};
use es_ui::components::{ShortcutField, ShortcutFieldEvent};
use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};
use support::*;

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct ShortcutHost {
    field: Entity<ShortcutField>,
    events: Vec<ShortcutFieldEvent>,
}

impl Render for ShortcutHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(150.)).child(self.field.clone())
    }
}

struct Recorder {
    app: App,
    host: Entity<ShortcutHost>,
    field: Entity<ShortcutField>,
}

impl Recorder {
    fn new() -> Self {
        let (mut app, host) = open(60., |window, cx| {
            let field = cx.new(|cx| {
                let mut f = ShortcutField::new("Capture area", window, cx);
                f.set_trigger(Trigger::parse("Super+Shift+4").ok(), cx);
                f
            });
            cx.subscribe(&field, |host: &mut ShortcutHost, _, event, _| {
                host.events.push(event.clone())
            })
            .detach();
            ShortcutHost {
                field,
                events: Vec::new(),
            }
        });
        let field = app.cx.update(|cx| host.read(cx).field.clone());
        Self { app, host, field }
    }

    fn events(&mut self) -> Vec<ShortcutFieldEvent> {
        let host = self.host.clone();
        self.app
            .cx
            .update(|cx| host.update(cx, |host, _| std::mem::take(&mut host.events)))
    }

    fn recording(&mut self) -> bool {
        let field = self.field.clone();
        self.app.cx.update(|cx| field.read(cx).is_recording())
    }

    fn shown(&mut self) -> String {
        let field = self.field.clone();
        self.app.cx.update(|cx| field.read(cx).text().0.to_string())
    }

    fn start(&mut self) {
        let field = self.field.clone();
        focus(&mut self.app, &field);
        if !self.recording() {
            key(&mut self.app, "space");
        }
        assert!(self.recording(), "the field is recording");
        self.events();
    }

    fn clears_on_delete(&mut self, clears: bool) {
        let field = self.field.clone();
        self.app
            .cx
            .update(|cx| field.update(cx, |f, cx| f.set_clears_on_delete(clears, cx)));
    }
}

fn recorded(modifiers: Modifiers, key: &str) -> ShortcutFieldEvent {
    ShortcutFieldEvent::Recorded(Trigger {
        modifiers,
        key: key.to_owned(),
    })
}

#[test]
fn recorder_echoes_held_modifiers_then_reports_the_chord_without_adopting_it() {
    let mut r = Recorder::new();
    assert_eq!(r.shown(), "Super+Shift+4");

    let field = r.field.clone();
    focus(&mut r.app, &field);
    assert!(r.recording(), "keyboard focus starts recording");
    assert_eq!(r.events(), vec![ShortcutFieldEvent::RecordingChanged(true)]);

    modifiers(&mut r.app, "shift-super-a");
    assert_eq!(
        r.shown(),
        "Super+Shift",
        "held modifiers show in canonical order"
    );
    assert!(r.events().is_empty(), "modifiers alone record nothing");
    key(&mut r.app, "shift");
    assert!(r.recording(), "a bare modifier key does not end recording");

    key(&mut r.app, "super-shift-a");
    assert_eq!(
        r.events(),
        vec![
            recorded(Modifiers::SUPER | Modifiers::SHIFT, "A"),
            ShortcutFieldEvent::RecordingChanged(false)
        ]
    );
    assert!(!r.recording());
    assert_eq!(
        r.shown(),
        "Super+Shift+4",
        "the caller decides whether to adopt it"
    );
}

#[test]
fn recorder_escape_cancels_and_keeps_the_old_binding() {
    let mut r = Recorder::new();
    r.start();
    key(&mut r.app, "escape");
    assert_eq!(
        r.events(),
        vec![ShortcutFieldEvent::RecordingChanged(false)]
    );
    assert!(!r.recording());
    assert_eq!(r.shown(), "Super+Shift+4");
}

#[test]
fn recorder_clears_on_bare_delete_only_for_global_shortcuts() {
    let mut r = Recorder::new();
    r.clears_on_delete(true);
    for bare in ["backspace", "delete"] {
        r.start();
        key(&mut r.app, bare);
        assert_eq!(
            r.events(),
            vec![
                ShortcutFieldEvent::Cleared,
                ShortcutFieldEvent::RecordingChanged(false)
            ],
            "bare {bare} clears a global shortcut"
        );
    }
    r.start();
    key(&mut r.app, "super-backspace");
    assert_eq!(
        r.events().first(),
        Some(&recorded(Modifiers::SUPER, "Backspace")),
        "a modified Delete stays bindable"
    );

    r.clears_on_delete(false);
    for (bare, name) in [("backspace", "Backspace"), ("delete", "Delete")] {
        r.start();
        key(&mut r.app, bare);
        assert_eq!(
            r.events().first(),
            Some(&recorded(Modifiers::empty(), name)),
            "an editor shortcut can bind bare {bare}"
        );
    }
}

#[test]
fn recorder_clear_button_clears_without_recording() {
    let mut r = Recorder::new();
    click(&mut r.app, 150. - 3. - 8., 12.);
    assert_eq!(r.events(), vec![ShortcutFieldEvent::Cleared]);
    assert!(!r.recording(), "the clear button does not start recording");
}

#[test]
fn recorder_ignores_key_repeat() {
    let mut r = Recorder::new();
    r.start();
    key(&mut r.app, "escape");
    r.events();
    key(&mut r.app, "space");
    assert!(r.recording(), "Space on the focused field starts recording");
    held(&mut r.app, "space");
    assert!(
        r.recording(),
        "the repeat of the key that started recording does not commit"
    );
    assert_eq!(r.events(), vec![ShortcutFieldEvent::RecordingChanged(true)]);
}

#[test]
fn recorder_lifecycle_is_balanced_on_every_exit() {
    let mut r = Recorder::new();
    r.start();
    blur(&mut r.app);
    assert!(!r.recording(), "losing focus cancels recording");

    let field = r.field.clone();
    focus(&mut r.app, &field);
    key(&mut r.app, "tab");
    assert!(!r.recording(), "Tab cancels and moves on");

    key(&mut r.app, "space");
    let handle = r.app.window.into();
    r.app
        .cx
        .update_window(handle, |_, _, cx| {
            field.update(cx, |f, cx| {
                f.cancel_recording(cx);
                f.cancel_recording(cx);
            })
        })
        .expect("window is open");
    r.app.settle();

    key(&mut r.app, "space");
    assert!(r.recording());
    let other = r
        .app
        .cx
        .update(|cx| cx.open_window(Default::default(), |_, cx| cx.new(|_| Blank)))
        .expect("second window opens");
    r.app
        .cx
        .update(|cx| other.update(cx, |_, window, _| window.activate_window()))
        .expect("second window is open");
    r.app.settle();
    assert!(
        !r.recording(),
        "leaving the window gives the app its shortcuts back"
    );
    r.app
        .cx
        .update(|cx| other.update(cx, |_, window, _| window.remove_window()))
        .expect("second window is open");

    let transitions: Vec<bool> = r
        .events()
        .into_iter()
        .map(|e| match e {
            ShortcutFieldEvent::RecordingChanged(on) => on,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        transitions,
        [false, true, false, true, false, true, false],
        "every start is matched by exactly one stop"
    );
}
