use crate::support;

use std::{cell::RefCell, rc::Rc};

use es_protocol::{Modifiers, Trigger};
use es_ui::components::{ShortcutField, ShortcutFieldEvent};
use gpui::{
    AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Pixels, Render, Styled,
    TextRun, Window, div, px,
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
    drawn: Rc<RefCell<Option<Bounds<Pixels>>>>,
}

impl Render for ShortcutHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let drawn = self.drawn.clone();
        div()
            .flex()
            .on_children_prepainted(move |bounds, _, _| {
                *drawn.borrow_mut() = bounds.first().copied();
            })
            .child(self.field.clone())
    }
}

struct Recorder {
    app: App,
    host: Entity<ShortcutHost>,
    field: Entity<ShortcutField>,
}

impl Recorder {
    fn new() -> Self {
        let (mut app, host) = open(120., |window, cx| {
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
                drawn: Rc::default(),
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

    fn bounds(&mut self) -> Bounds<Pixels> {
        self.app.settle();
        let host = self.host.clone();
        self.app
            .cx
            .update(|cx| *host.read(cx).drawn.borrow())
            .expect("the field is drawn")
    }

    fn set(&mut self, trigger: &str) {
        let field = self.field.clone();
        let trigger = Trigger::parse(trigger).ok();
        self.app
            .cx
            .update(|cx| field.update(cx, |f, cx| f.set_trigger(trigger, cx)));
        self.app.settle();
    }

    fn clear_button(&mut self) -> (f32, f32) {
        let b = self.bounds();
        (f32::from(b.right()) - 3. - 8., f32::from(b.center().y))
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
    let (x, y) = r.clear_button();
    click(&mut r.app, x, y);
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

fn label_width(app: &mut App, text: String) -> Pixels {
    let style = es_ui::tokens::TYPE.value;
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight: style.weight,
            features: style.features(),
            ..gpui::font(es_ui::theme::UI_FONT)
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| {
            window
                .text_system()
                .shape_line(text.into(), style.size, &[run], None)
                .width
        })
        .expect("window is open")
}

#[test]
fn every_field_is_one_width_and_fits_the_longest_chord_beside_its_clear_button() {
    let mut r = Recorder::new();
    r.set("Super+Shift+4");
    let short = r.bounds().size.width;
    let longest = "Super+Ctrl+Alt+Shift+Backspace";
    r.set(longest);
    assert_eq!(r.shown(), longest);
    let long = r.bounds().size.width;
    assert_eq!(
        short, long,
        "a column of fields lines up whatever they hold"
    );
    let text = label_width(&mut r.app, longest.to_owned());
    let clear = px(16.);
    let margins = px(2. * 4.);
    assert!(
        long >= text + clear + margins,
        "the chord ({text:?}) and the clear button fit inside {long:?} with no ellipsis"
    );
}

#[test]
fn the_clear_button_names_itself_in_a_tooltip() {
    let mut r = Recorder::new();
    let (x, y) = r.clear_button();
    assert!(
        tooltip_shows_at(&mut r.app, x, y),
        "an icon-only button says what it does on hover"
    );
}
