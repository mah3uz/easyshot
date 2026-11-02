use anyhow::{Context as _, Result, anyhow};
use gpui::{
    AnyWindowHandle, App, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, OperatingSystem, PlatformInput, point, px,
};
use serde::Deserialize;

/// One event of the `Test.Inject` payload, a JSON array of these.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Key { keystroke: String },
    MouseDown { x: f32, y: f32 },
    MouseUp { x: f32, y: f32 },
    MouseMove { x: f32, y: f32 },
}

fn input(event: Event) -> Result<PlatformInput> {
    let at = |x: f32, y: f32| point(px(x), px(y));
    Ok(match event {
        Event::Key { keystroke } => PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse_with_os(&keystroke, OperatingSystem::Linux)
                .map_err(|e| anyhow!("bad keystroke {keystroke:?}: {e}"))?,
            is_held: false,
            prefer_character_input: false,
        }),
        Event::MouseDown { x, y } => PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at(x, y),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }),
        Event::MouseUp { x, y } => PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at(x, y),
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
        Event::MouseMove { x, y } => PlatformInput::MouseMove(MouseMoveEvent {
            position: at(x, y),
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
    })
}

pub fn parse(text: &str) -> Result<Vec<PlatformInput>> {
    serde_json::from_str::<Vec<Event>>(text)
        .context("Test.Inject takes a JSON array of events")?
        .into_iter()
        .map(input)
        .collect()
}

/// The focused window gets the events, else the newest one.
fn target(cx: &App) -> Option<AnyWindowHandle> {
    cx.active_window().or_else(|| cx.windows().last().copied())
}

pub fn dispatch(text: &str, cx: &mut App) {
    let events = match parse(text) {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "injected events");
            return;
        }
    };
    let Some(window) = target(cx) else {
        tracing::warn!("injected events with no window open");
        return;
    };
    for event in events {
        let sent = window.update(cx, |_, window, cx| {
            window.dispatch_event(event, cx);
        });
        if sent.is_err() {
            tracing::warn!("injected events: the window closed");
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        AppContext as _, Context, FocusHandle, InteractiveElement as _, IntoElement,
        ParentElement as _, Render, Styled as _, TestAppContext, Window, WindowOptions, div,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Probe {
        focus: FocusHandle,
        keys: Rc<RefCell<Vec<String>>>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let keys = self.keys.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(move |_, event: &KeyDownEvent, _, _| {
                    keys.borrow_mut().push(event.keystroke.key.clone());
                }))
                .child("probe")
        }
    }

    #[test]
    fn events_parse_and_unknown_ones_are_refused() {
        let ok =
            parse(r#"[{"type":"key","keystroke":"ctrl-a"},{"type":"mouse_move","x":1,"y":2}]"#)
                .unwrap();
        assert!(matches!(ok[0], PlatformInput::KeyDown(_)));
        assert!(matches!(ok[1], PlatformInput::MouseMove(_)));
        assert!(parse(r#"[{"type":"format_disk"}]"#).is_err());
        assert!(parse("key:Escape").is_err());
    }

    #[gpui::test]
    fn injected_keys_reach_the_focused_window(cx: &mut TestAppContext) {
        let keys = Rc::new(RefCell::new(Vec::new()));
        let seen = keys.clone();
        cx.update(|cx| {
            cx.open_window(WindowOptions::default(), move |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Probe { focus, keys: seen }
                })
            })
            .unwrap();
        });
        cx.run_until_parked();
        cx.update(|cx| dispatch(r#"[{"type":"key","keystroke":"escape"}]"#, cx));
        cx.run_until_parked();
        assert_eq!(*keys.borrow(), ["escape"]);
    }
}
