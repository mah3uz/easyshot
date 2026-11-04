use crate::support::{self, App};
use es_ui::{
    components::{FocusPad, PresetBar},
    motion::{BUTTON_POP, KNOB_GROW, SETTLE, Transition, set_reduce_motion},
    tokens::ThemeMode,
};
use gpui::{
    Context, Entity, IntoElement, MouseButton, MouseDownEvent, ParentElement, PlatformInput, Point,
    Render, Styled, Window, div, point, px,
};
use image::RgbaImage;

struct Host {
    focus: Point<f32>,
    applied: Option<usize>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                PresetBar::new("presets", ["Wide".into(), "Tight".into()])
                    .applied(self.applied)
                    .on_add(|_, _| {})
                    .on_delete(|_, _, _| {}),
            )
            .child(
                div().p(px(8.)).child(
                    FocusPad::new("pad", "Focus")
                        .position(self.focus)
                        .on_change(move |at, _, cx| {
                            let _ = this.update(cx, |h, cx| {
                                h.focus = at;
                                cx.notify();
                            });
                        }),
                ),
            )
    }
}

const PAD_TOP: f32 = 46. + 8.;

fn open(mode: ThemeMode, reduce: bool) -> (App, Entity<Host>) {
    let (mut app, host) = support::open(190., |_, _| Host {
        focus: point(0.2, 0.2),
        applied: None,
    });
    app.cx.update(|cx| {
        es_ui::theme::set_mode(mode, cx);
        set_reduce_motion(reduce, cx);
    });
    app.settle();
    (app, host)
}

fn shot(app: &mut App, name: &str) -> RgbaImage {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .unwrap_or_default();
    app.screenshot(name)
}

fn frames(app: &mut App, t: Transition, name: &str) -> [RgbaImage; 3] {
    let start = shot(app, &format!("{name}-0-start"));
    app.cx.advance_clock(t.duration / 3);
    let mid = shot(app, &format!("{name}-1-mid"));
    app.cx.advance_clock(t.duration);
    let end = shot(app, &format!("{name}-2-end"));
    [start, mid, end]
}

fn assert_moves(moving: &[RgbaImage; 3], settled: &RgbaImage, what: &str) {
    let [start, mid, end] = moving;
    assert!(start != mid && mid != end, "{what} draws in motion");
    assert!(
        end == settled,
        "{what} arrives where Reduce Motion puts it at once"
    );
    assert!(start != settled, "Reduce Motion skips {what}");
}

fn pad_press(app: &mut App, x: f32, y: f32, clicks: usize) {
    support::send(
        app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Default::default(),
            click_count: clicks,
            first_mouse: false,
        }),
    );
}

fn apply(app: &mut App, host: &Entity<Host>, applied: Option<usize>) {
    app.cx.update(|cx| {
        host.update(cx, |h, cx| {
            h.applied = applied;
            cx.notify();
        })
    });
    app.settle();
}

#[test]
fn the_focus_knob_grows_as_a_drag_starts_and_glides_home_on_a_double_click() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let tag = format!("{mode:?}").to_lowercase();
        let grab = |app: &mut App| pad_press(app, 8. + 284. * 0.2, PAD_TOP + 118. * 0.2, 1);
        let (mut app, _) = open(mode, false);
        grab(&mut app);
        let moving = frames(&mut app, KNOB_GROW, &format!("focus-knob-grow-{tag}"));
        let (mut still, _) = open(mode, true);
        grab(&mut still);
        let settled = shot(&mut still, &format!("focus-knob-grow-{tag}-reduced"));
        assert_moves(&moving, &settled, "the knob growing");

        let home = |app: &mut App| pad_press(app, 8. + 284. * 0.8, PAD_TOP + 118. * 0.8, 2);
        let (mut app, host) = open(mode, false);
        home(&mut app);
        assert_eq!(
            app.cx.update(|cx| host.read(cx).focus),
            point(0.5, 0.5),
            "the focus is centred at once"
        );
        let moving = frames(&mut app, SETTLE, &format!("focus-recentre-{tag}"));
        let (mut still, _) = open(mode, true);
        home(&mut still);
        let settled = shot(&mut still, &format!("focus-recentre-{tag}-reduced"));
        assert_moves(&moving, &settled, "the recentre");
    }
}

#[test]
fn the_preset_delete_button_pops_in_and_shrinks_away() {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        let tag = format!("{mode:?}").to_lowercase();
        let (mut app, host) = open(mode, false);
        let none = shot(&mut app, &format!("preset-delete-{tag}-before"));
        apply(&mut app, &host, Some(0));
        let moving = frames(&mut app, BUTTON_POP, &format!("preset-delete-in-{tag}"));
        let (mut still, still_host) = open(mode, true);
        apply(&mut still, &still_host, Some(0));
        let settled = shot(&mut still, &format!("preset-delete-in-{tag}-reduced"));
        assert_moves(&moving, &settled, "the delete button appearing");

        apply(&mut app, &host, None);
        let leaving = frames(&mut app, BUTTON_POP, &format!("preset-delete-out-{tag}"));
        assert!(
            leaving[0] != leaving[1] && leaving[1] != leaving[2],
            "the delete button shrinks away"
        );
        assert!(leaving[2] == none, "the bar ends as it was before");
    }
}
