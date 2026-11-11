#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{TEXT, opened, rect};
use es_app::share::{PopoverHost, WindowPopover};
use es_editor::{EditorSession, EditorWindow};
use es_testkit::Headless;
use es_ui::components::{SharePopover, ShareView};
use es_ui::tokens::ThemeMode;
use gpui::{AppContext, Entity};
use gpui_component::Root;
use std::cell::RefCell;
use std::rc::Rc;

type App = Headless<Root>;

fn launch(mode: ThemeMode) -> (App, Entity<EditorWindow>) {
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let doc = opened(
        1200,
        640,
        vec![rect(100.0, 100.0, 300.0, 200.0)],
        Default::default(),
    );
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        1080.0,
        720.0,
        move |window, cx| {
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    (app, editor)
}

/// Lets the popover's opening play out, redrawing the headless window after each step.
fn draw(app: &mut App) {
    let window = app.window.into();
    for _ in 0..3 {
        app.cx.advance_clock(es_ui::motion::POPOVER_OPEN.duration);
        std::thread::sleep(es_ui::motion::POPOVER_OPEN.duration);
        app.cx
            .update_window(window, |_, window, _| window.refresh())
            .unwrap();
        app.settle();
    }
}

#[test]
fn pictures_of_share_in_the_footer_and_its_popover() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let (mut app, editor) = launch(mode);
        let host = WindowPopover::default();
        let given = host.clone();
        app.cx
            .update(|cx| editor.update(cx, |w, cx| w.set_share(given, cx)));
        draw(&mut app);
        let closed = app.screenshot(&format!("editor-share-{name}"));

        let shown = host.clone();
        app.cx.update(|cx| {
            let build = Box::new(|window: &mut gpui::Window, cx: &mut gpui::App| {
                cx.new(|cx| {
                    let mut p = SharePopover::new(window, cx);
                    p.set_view(ShareView::Ready, cx);
                    p.set_days(7, cx);
                    p
                })
            });
            shown.open(None, build, Rc::new(|_| {}), cx).unwrap()
        });
        draw(&mut app);
        assert!(host.is_open());
        let open = app.screenshot(&format!("editor-share-popover-{name}"));
        assert!(
            open != closed,
            "{name}: the popover is drawn over the editor"
        );
    }
}
