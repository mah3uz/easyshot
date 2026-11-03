#![allow(clippy::unwrap_used)]
use crate::window_mode_support;

use es_app::deck::{DeckItem, ItemStatus};
use es_model::CaptureKind;
use es_overlays::deck::card::Card;
use es_overlays::deck::layout::{CardSize, ToolLayout};
use es_overlays::deck::view::load_thumbnail;
use es_ui::motion::{CROSSFADE, set_reduce_motion};
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement as _, Render, RenderImage, Styled as _,
    Window, div, px, rgb,
};
use image::RgbaImage;
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;
use window_mode_support::Surface;

const PAD: f32 = 16.;

struct OneCard {
    item: DeckItem,
    image: Option<Arc<RenderImage>>,
    focus: FocusHandle,
}

impl Render for OneCard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let card = Card::new(
            self.item.clone(),
            CardSize::Small.size(),
            self.image.clone(),
            ToolLayout::default(),
            CardSize::Small.control_scale(),
            self.focus.clone(),
            Rc::new(|_, _, _| {}),
        );
        div().size_full().p(px(PAD)).bg(rgb(0x2a2d33)).child(card)
    }
}

struct Rig {
    s: Surface<OneCard>,
    image: Arc<RenderImage>,
    _dir: tempfile::TempDir,
}

fn rig(image: bool, status: ItemStatus, reduce: bool) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let thumb = dir.path().join("thumb.png");
    RgbaImage::from_fn(360, 220, |x, y| match (x < 140, (y / 22) % 2 == 0) {
        (true, _) => image::Rgba([40, 110, 230, 255]),
        (false, true) => image::Rgba([250, 250, 252, 255]),
        (false, false) => image::Rgba([225, 229, 236, 255]),
    })
    .save(&thumb)
    .unwrap();
    let picture = load_thumbnail(&thumb).unwrap();
    let item = DeckItem {
        id: Uuid::from_u128(7),
        name: "shot".into(),
        file: dir.path().join("shot.png"),
        thumb,
        kind: CaptureKind::Region,
        video: false,
        staged: true,
        status,
        auto_dismiss: true,
        saved: None,
    };
    let card = CardSize::Small.size();
    let shown = image.then(|| picture.clone());
    let mut s = Surface::open(
        card.width as f32 + PAD * 2.,
        card.height as f32 + PAD * 2.,
        move |cx| OneCard {
            item,
            image: shown,
            focus: cx.focus_handle(),
        },
    );
    s.app.cx.update(|cx| set_reduce_motion(reduce, cx));
    s.app.settle();
    Rig {
        s,
        image: picture,
        _dir: dir,
    }
}

impl Rig {
    fn change(&mut self, f: impl FnOnce(&mut OneCard)) {
        let view = self.s.view.clone();
        self.s.app.cx.update(|cx| {
            view.update(cx, |v, cx| {
                f(v);
                cx.notify();
            })
        });
        self.s.app.settle();
    }

    fn frame(&mut self, name: &str) -> RgbaImage {
        self.change(|_| {});
        self.s.app.screenshot(name)
    }

    /// Frames at the start, halfway, three quarters of the way and at the end of the cross-fade.
    fn frames(&mut self, name: &str) -> [RgbaImage; 4] {
        let quarter = CROSSFADE.duration / 4;
        let start = self.frame(&format!("{name}-0-start"));
        self.s.app.cx.advance_clock(quarter * 2);
        let mid = self.frame(&format!("{name}-1-mid"));
        self.s.app.cx.advance_clock(quarter);
        let late = self.frame(&format!("{name}-2-late"));
        self.s.app.cx.advance_clock(quarter);
        let end = self.frame(&format!("{name}-3-end"));
        [start, mid, late, end]
    }
}

fn brightness(image: &RgbaImage, x: u32, y: u32) -> u32 {
    image.get_pixel(x, y).0[..3]
        .iter()
        .map(|&c| u32::from(c))
        .sum()
}

/// On the picture's top light band, which the card centres, and clear of the centred caption.
fn probe(image: &RgbaImage) -> u32 {
    let (w, h) = image.dimensions();
    brightness(image, w * 3 / 4, (h - 220) / 2 + 11)
}

fn assert_fades(frames: &[RgbaImage; 4], settled: &RgbaImage, what: &str) {
    let [start, mid, late, end] = frames;
    let (a, m, z) = (probe(start), probe(mid), probe(end));
    let left = (m as f32 - z as f32) / (a as f32 - z as f32);
    let expected = 1. - CROSSFADE.easing.at(0.5);
    assert!(
        (left - expected).abs() < 0.03,
        "{what} has {left} of its fade left halfway, as its token says ({expected}): {a} -> {m} -> {z}"
    );
    assert!(
        late != end,
        "{what} is still fading three quarters of the way"
    );
    assert!(end == settled, "{what} arrives when its duration is up");
}

#[test]
fn a_loaded_thumbnail_cross_fades_in_over_the_loading_caption() {
    let mut r = rig(false, ItemStatus::Idle, false);
    let loading = r.s.app.screenshot("deck-thumb-loading");
    let image = r.image.clone();
    r.change(move |v| v.image = Some(image));
    let frames = r.frames("deck-thumb-fade");
    assert!(
        frames[0] == loading,
        "the picture starts where the caption was"
    );
    let settled = rig(true, ItemStatus::Idle, false).frame("deck-thumb-settled");
    assert_fades(&frames, &settled, "the thumbnail");

    let mut still = rig(false, ItemStatus::Idle, true);
    let image = still.image.clone();
    still.change(move |v| v.image = Some(image));
    assert!(
        still.frame("deck-thumb-reduced") == settled,
        "under Reduce Motion the picture shows at once"
    );
}

#[test]
fn the_saving_chip_cross_fades_in_and_out() {
    let mut r = rig(true, ItemStatus::Idle, false);
    let idle = r.frame("deck-saving-idle");
    r.change(|v| v.item.status = ItemStatus::Saving);
    let saving = rig(true, ItemStatus::Saving, false).frame("deck-saving-settled");
    let fading_in = r.frames("deck-saving-in");
    assert!(fading_in[0] == idle, "the chip starts unseen");
    assert_fades(&fading_in, &saving, "the chip fading in");

    r.change(|v| v.item.status = ItemStatus::Idle);
    let fading_out = r.frames("deck-saving-out");
    assert!(fading_out[0] == saving, "the chip starts in full");
    assert_fades(&fading_out, &idle, "the chip fading out");

    let mut still = rig(true, ItemStatus::Idle, true);
    still.change(|v| v.item.status = ItemStatus::Saving);
    assert!(
        still.frame("deck-saving-reduced") == saving,
        "under Reduce Motion the chip shows at once"
    );
}
