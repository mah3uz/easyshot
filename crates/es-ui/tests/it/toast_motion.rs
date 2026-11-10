use crate::support::{self, App};
use es_ui::components::Toast;
use es_ui::icons::IconName;
use es_ui::motion::{TOAST_IN, TOAST_OUT, TOAST_REDUCED, set_reduce_motion};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};

struct Host {
    leaving: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().justify_center().pt(px(12.)).child(
            Toast::new("toast", "Copied to the clipboard")
                .icon(IconName::Copy)
                .message("Screenshot 1280 x 720")
                .leaving(self.leaving),
        )
    }
}

/// The card's drawn width and vertical centre in window pixels, and its strongest contrast
/// with the window behind it.
#[derive(Debug, Clone, Copy)]
struct Seen {
    width: u32,
    centre: f32,
    contrast: u32,
}

fn seen(app: &mut App, name: &str) -> Option<Seen> {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .unwrap_or_default();
    let image = app.screenshot(name);
    let back = image.get_pixel(0, 0).0;
    let diff = |x: u32, y: u32| {
        let p = image.get_pixel(x, y).0;
        (0..3)
            .map(|c| u32::from(p[c].abs_diff(back[c])))
            .max()
            .unwrap_or(0)
    };
    let inked = |x: u32, y: u32| diff(x, y) > 6;
    let rows: Vec<u32> = (0..image.height())
        .filter(|&y| inked(image.width() / 2, y))
        .collect();
    let middle = *rows.get(rows.len() / 2)?;
    let xs: Vec<u32> = (0..image.width()).filter(|&x| inked(x, middle)).collect();
    let contrast = (0..image.height())
        .flat_map(|y| (0..image.width()).map(move |x| (x, y)))
        .map(|(x, y)| diff(x, y))
        .max()
        .unwrap_or(0);
    Some(Seen {
        width: xs.last()? - xs.first()?,
        centre: rows.iter().sum::<u32>() as f32 / rows.len() as f32,
        contrast,
    })
}

fn open(reduce: bool) -> (App, Entity<Host>) {
    let (mut app, host) = support::open(90., |_, cx| {
        set_reduce_motion(reduce, cx);
        Host { leaving: false }
    });
    app.settle();
    (app, host)
}

fn leave(app: &mut App, host: &Entity<Host>) {
    app.cx.update(|cx| {
        host.update(cx, |h, cx| {
            h.leaving = true;
            cx.notify();
        })
    });
    app.settle();
}

#[test]
fn a_toast_grows_and_rises_in_on_a_spring_and_leaves_by_shrinking_as_it_fades() {
    let (mut app, host) = open(false);
    let step = TOAST_IN.duration / 8;
    let mut frames = Vec::new();
    for i in 0..10 {
        frames.push(seen(&mut app, &format!("toast-in-{i}")));
        app.cx.advance_clock(step);
    }
    let rest = seen(&mut app, "toast-rest").expect("the toast is drawn");
    let early = frames[1].expect("the toast is drawn as it enters");
    assert!(
        early.width + 12 < rest.width,
        "it starts smaller than it rests: {early:?} then {rest:?}"
    );
    assert!(
        early.centre > rest.centre + 2.,
        "and lower, rising into place: {early:?} then {rest:?}"
    );
    assert!(early.contrast < rest.contrast, "and faint, fading in");

    leave(&mut app, &host);
    let mut out = Vec::new();
    for i in 0..5 {
        app.cx.advance_clock(TOAST_OUT.duration / 4);
        out.push(seen(&mut app, &format!("toast-out-{i}")));
    }
    let half = seen_at(&out, 1);
    assert!(
        half.width < rest.width && half.contrast < rest.contrast,
        "leaving, it shrinks as it fades: {half:?} from {rest:?}"
    );
    assert!(
        out.iter().flatten().all(|f| f.width <= rest.width),
        "and never swings past its size on the way out"
    );
    assert!(out[4].is_none(), "then it is gone");
}

fn seen_at(frames: &[Option<Seen>], i: usize) -> Seen {
    frames[i].unwrap_or_else(|| panic!("frame {i} shows the toast"))
}

#[test]
fn under_reduce_motion_a_toast_only_fades_in_and_out_in_place() {
    let (mut moving, _) = open(false);
    moving.cx.advance_clock(TOAST_IN.duration * 2);
    let rest = seen(&mut moving, "toast-reduced-rest").expect("the toast is drawn");

    let (mut app, host) = open(true);
    app.cx.advance_clock(TOAST_REDUCED.duration / 4);
    let fading = seen(&mut app, "toast-reduced-in").expect("the toast is drawn");
    assert_eq!(
        (fading.width, fading.centre),
        (rest.width, rest.centre),
        "it is drawn at its size and place at once"
    );
    assert!(fading.contrast < rest.contrast, "and only fades in");

    app.cx.advance_clock(TOAST_REDUCED.duration);
    leave(&mut app, &host);
    app.cx.advance_clock(TOAST_REDUCED.duration / 2);
    let going = seen(&mut app, "toast-reduced-out").expect("the toast is still fading");
    assert_eq!((going.width, going.centre), (rest.width, rest.centre));
    assert!(going.contrast < rest.contrast, "and it only fades out");
}
