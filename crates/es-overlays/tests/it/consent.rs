use crate::window_mode_support::Surface;

use es_app::agent_access::{Answer, ChangeRequest, ConsentRequest, Loosening};
use es_overlays::consent::{Ask, ConsentDialog, WIDTH};
use es_protocol::agent::GrantSpan;
use es_ui::theme;
use es_ui::tokens::ThemeMode;
use gpui::{KeyUpEvent, Keystroke, PlatformInput};

fn access(remember: bool) -> Ask {
    Ask::Access(ConsentRequest {
        route: "agent/capture",
        target: Some("output DP-1".into()),
        pid: Some(48213),
        comm: Some("claude".into()),
        remember,
    })
}

fn change(change: Loosening) -> Ask {
    Ask::Change(ChangeRequest {
        change,
        pid: Some(48213),
        comm: Some("claude".into()),
    })
}

fn dialog(ask: Ask) -> Surface<ConsentDialog> {
    let size = ConsentDialog::size();
    Surface::open(WIDTH as f32, size.height as f32, move |cx| {
        ConsentDialog::new(ask, cx)
    })
}

/// The answers given after pressing these keys.
fn answer_after(ask: Ask, keys: &[&'static str]) -> Vec<Answer> {
    let mut s = dialog(ask);
    let (seen, _sub) = s.record::<Answer>();
    for key in keys {
        press(&mut s, key);
    }
    seen.borrow().clone()
}

/// Down and up: a focused button clicks on the release.
fn press(s: &mut Surface<ConsentDialog>, key: &'static str) {
    s.key(key);
    s.send(PlatformInput::KeyUp(KeyUpEvent {
        keystroke: Keystroke::parse(key).expect("keystroke"),
    }));
}

#[test]
fn each_button_gives_its_answer_in_tab_order_with_deny_first() {
    let hour = Answer::Allow(GrantSpan::Hour);
    let session = Answer::Allow(GrantSpan::Session);
    let always = Answer::Allow(GrantSpan::Always);
    for (tabs, expected) in [(1, Answer::Deny), (2, hour), (3, session), (4, always)] {
        let mut keys = vec!["tab"; tabs];
        keys.push("enter");
        assert_eq!(answer_after(access(true), &keys), [expected], "{tabs} tabs");
    }
    assert_eq!(
        answer_after(access(false), &["tab", "tab", "enter"]),
        [Answer::Once],
        "a request asked about every time is allowed only once"
    );
    assert_eq!(
        answer_after(change(Loosening::TurnOn), &["tab", "tab", "enter"]),
        [Answer::Once]
    );
    assert_eq!(
        answer_after(change(Loosening::TurnOn), &["tab", "enter"]),
        [Answer::Deny],
        "cancel"
    );
}

#[test]
fn enter_as_the_dialog_opens_allows_nothing_and_esc_denies() {
    assert!(answer_after(access(true), &["enter", "space", "a"]).is_empty());
    assert!(answer_after(change(Loosening::TurnOn), &["enter"]).is_empty());
    assert_eq!(answer_after(access(true), &["escape"]), [Answer::Deny]);
    assert_eq!(
        answer_after(
            change(Loosening::WiderScope),
            &["escape", "tab", "tab", "enter"]
        ),
        [Answer::Deny],
        "the first answer is the only one"
    );
}

#[test]
fn pictures_of_each_dialog_in_both_themes() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        for (ask, what) in [
            (access(true), "access"),
            (access(false), "once"),
            (change(Loosening::TurnOn), "enable"),
            (change(Loosening::AlwaysAllow), "always"),
        ] {
            let mut s = dialog(ask);
            let view = s.view.clone();
            s.app.cx.update(|cx| {
                theme::set_mode(mode, cx);
                view.update(cx, |_, cx| cx.notify());
            });
            s.app.screenshot(&format!("consent-{what}-{name}"));
        }
    }
    let mut s = dialog(access(true));
    press(&mut s, "tab");
    press(&mut s, "tab");
    s.app.screenshot("consent-access-focused");
}

#[test]
fn an_unanswered_dialog_denies_after_its_wait_and_not_before() {
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let (tx, rx) = futures::channel::oneshot::channel::<()>();
    let held = std::rc::Rc::new(std::cell::RefCell::new(Some(rx)));
    let seen = asked.clone();
    let mut s = Surface::open(WIDTH as f32, 196., move |cx| {
        let sleep: es_overlays::countdown::Sleep = std::rc::Rc::new(move |d, cx| {
            seen.borrow_mut().push(d);
            let rx = held.borrow_mut().take();
            cx.spawn(async move |_| {
                if let Some(rx) = rx {
                    let _ = rx.await;
                }
            })
        });
        ConsentDialog::with_sleep(access(true), &sleep, cx)
    });
    let (answers, _sub) = s.record::<Answer>();
    s.app.settle();
    assert_eq!(*asked.borrow(), [es_overlays::consent::ANSWER_WAIT]);
    assert_eq!(es_overlays::consent::ANSWER_WAIT.as_secs(), 60);
    assert!(
        answers.borrow().is_empty(),
        "nothing is answered while it waits"
    );
    tx.send(()).unwrap_or_default();
    s.app.settle();
    assert_eq!(*answers.borrow(), [Answer::Deny]);
}
