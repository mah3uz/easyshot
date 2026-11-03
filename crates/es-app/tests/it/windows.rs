#![allow(clippy::unwrap_used)]
use es_app::errors::Error;
use es_app::windows::{WindowHandle, WindowKey, WindowKind, Windows};
use gpui::{App, TestAppContext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Fake {
    open: Cell<bool>,
    focused: Cell<u32>,
}

impl WindowHandle for Fake {
    fn is_open(&self, _: &App) -> bool {
        self.open.get()
    }

    fn focus(&self, _: &mut App) {
        self.focused.set(self.focused.get() + 1);
    }

    fn close(&self, _: &mut App) {
        self.open.set(false);
    }
}

type Opened = Rc<RefCell<Vec<Rc<Fake>>>>;

fn registry(kind: WindowKind) -> (Windows, Opened) {
    let windows = Windows::default();
    let opened = Opened::default();
    let sink = opened.clone();
    windows.register(
        kind,
        Rc::new(move |_, _| {
            let fake = Rc::new(Fake {
                open: Cell::new(true),
                focused: Cell::new(0),
            });
            sink.borrow_mut().push(fake.clone());
            Ok(fake)
        }),
    );
    (windows, opened)
}

#[gpui::test]
fn a_second_open_for_the_same_key_focuses_instead_of_opening(cx: &mut TestAppContext) {
    let (windows, opened) = registry(WindowKind::Home);
    cx.update(|cx| {
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
    });
    assert_eq!(opened.borrow().len(), 1);
    assert_eq!(opened.borrow()[0].focused.get(), 1);
}

#[gpui::test]
fn the_capture_bar_toggles_so_one_key_both_opens_and_dismisses_it(cx: &mut TestAppContext) {
    let (windows, opened) = registry(WindowKind::CaptureBar);
    let open = |cx: &mut App| {
        windows
            .open(WindowKind::CaptureBar, WindowKey::Single, cx)
            .unwrap();
        windows.is_open(WindowKind::CaptureBar, &WindowKey::Single, cx)
    };
    cx.update(|cx| {
        assert!(open(cx));
        assert!(!open(cx));
        assert!(open(cx));
    });
    let opened = opened.borrow();
    assert_eq!(opened.len(), 2);
    assert!(!opened[0].open.get());
    assert_eq!(opened[0].focused.get(), 0);
}

#[gpui::test]
fn editor_windows_are_keyed_by_canonical_path(cx: &mut TestAppContext) {
    let (windows, opened) = registry(WindowKind::Editor);
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.png");
    let b = dir.path().join("b.png");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();
    let roundabout = dir.path().join(".").join("a.png");
    cx.update(|cx| {
        for path in [a, roundabout, b] {
            windows
                .open(WindowKind::Editor, WindowKey::Path(path), cx)
                .unwrap();
        }
    });
    assert_eq!(opened.borrow().len(), 2);
    assert_eq!(opened.borrow()[0].focused.get(), 1);
}

#[gpui::test]
fn a_closed_window_is_forgotten_and_opens_afresh(cx: &mut TestAppContext) {
    let (windows, opened) = registry(WindowKind::Home);
    cx.update(|cx| {
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
        windows.forget(WindowKind::Home, &WindowKey::Single);
        assert!(!windows.is_open(WindowKind::Home, &WindowKey::Single, cx));
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
    });
    assert_eq!(opened.borrow().len(), 2);
}

#[gpui::test]
fn a_window_closed_without_notice_is_reopened_not_focused(cx: &mut TestAppContext) {
    let (windows, opened) = registry(WindowKind::Home);
    cx.update(|cx| {
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
        opened.borrow()[0].open.set(false);
        windows
            .open(WindowKind::Home, WindowKey::Single, cx)
            .unwrap();
    });
    assert_eq!(opened.borrow().len(), 2);
}

#[gpui::test]
fn a_kind_without_an_opener_is_unsupported(cx: &mut TestAppContext) {
    let windows = Windows::default();
    let result = cx.update(|cx| windows.open(WindowKind::Home, WindowKey::Single, cx));
    assert!(matches!(result, Err(Error::Unsupported)));
}
