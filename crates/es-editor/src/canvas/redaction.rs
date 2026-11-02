use super::{EditorCanvas, RedactionPainter, underlay};
use crate::budget::Lease;
use crate::session::doc::DocModel;
use es_anno::draw::RedactionKind;
use es_anno::{DrawItem, Viewport};
use es_geom::Rect;
use gpui::{
    App, Bounds, Corners, Entity, Pixels, RenderImage, WeakEntity, Window, point, px, size,
};
use image::RgbaImage;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::{IntSize, Pixmap};

/// Patches sample the full-resolution base, so the preview scale is always 1.
const PREVIEW_SCALE: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    base: u64,
    rect: Rect,
    rotation: f64,
    mirrored: bool,
    kind: RedactionKind,
    density: f64,
}

impl Key {
    fn of(base: u64, item: &DrawItem) -> Option<Self> {
        match *item {
            DrawItem::Redaction {
                rect,
                rotation,
                mirrored,
                kind,
                density,
            } => Some(Self {
                base,
                rect,
                rotation,
                mirrored,
                kind,
                density,
            }),
            _ => None,
        }
    }
}

struct Patch {
    id: u64,
    key: Key,
    page: Rect,
    image: Arc<RenderImage>,
}

#[derive(Default)]
struct Slot {
    current: Option<Patch>,
    pending: Option<Key>,
}

/// One slot per redaction in z-order: at most the patch on screen plus the one being built.
#[derive(Default)]
pub struct Redactions {
    slots: Vec<Slot>,
    base: Option<(u64, Arc<Pixmap>)>,
    retired: Vec<Arc<RenderImage>>,
    next_id: u64,
    painted: Vec<u64>,
}

impl Redactions {
    /// Patch ids painted by the last frame, in z-order.
    pub fn painted(&self) -> &[u64] {
        &self.painted
    }

    /// Patches built and in memory, on screen or waiting to be.
    pub fn held(&self) -> usize {
        self.slots.iter().filter(|s| s.current.is_some()).count()
    }

    pub fn building(&self) -> usize {
        self.slots.iter().filter(|s| s.pending.is_some()).count()
    }

    /// Lines the slots up with this frame's redactions, keeping a slot whose patch still matches.
    fn align(&mut self, keys: &[Key]) -> bool {
        if self.slots.len() == keys.len() {
            return false;
        }
        let mut old = std::mem::take(&mut self.slots);
        for key in keys {
            let found = old
                .iter()
                .position(|s| s.current.as_ref().is_some_and(|p| p.key == *key));
            self.slots
                .push(found.map(|i| old.remove(i)).unwrap_or_default());
        }
        self.retired
            .extend(old.into_iter().filter_map(|s| s.current.map(|p| p.image)));
        true
    }

    /// Hands back the patch in a slot the shared budget gave up.
    fn release(&mut self, slot: usize) {
        if let Some(patch) = self.slots.get_mut(slot).and_then(|s| s.current.take()) {
            self.retired.push(patch.image);
        }
    }
}

pub(super) fn premultiplied(base: &RgbaImage) -> Option<Pixmap> {
    let mut data = base.as_raw().clone();
    for px in data.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        if a < 255 {
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
    }
    Pixmap::from_vec(data, IntSize::from_wh(base.width(), base.height())?)
}

fn screen_bounds(bounds: Bounds<Pixels>, vp: &Viewport, page: Rect) -> Bounds<Pixels> {
    let s = vp.scale;
    Bounds::new(
        bounds.origin
            + point(
                px((vp.origin.x + page.x0 * s) as f32),
                px((vp.origin.y + page.y0 * s) as f32),
            ),
        size(
            px((page.width() * s) as f32),
            px((page.height() * s) as f32),
        ),
    )
}

type Built = Option<(Arc<Pixmap>, Option<(Rect, Arc<RenderImage>)>)>;

fn build(base: &RgbaImage, pixmap: Option<Arc<Pixmap>>, key: Key) -> Built {
    let pixmap = match pixmap {
        Some(p) => p,
        None => Arc::new(premultiplied(base)?),
    };
    let patch = es_render::effects::redaction_patch(
        &pixmap,
        key.rect,
        key.rotation,
        key.mirrored,
        key.kind,
        key.density,
        PREVIEW_SCALE,
    );
    let image = patch.and_then(|p| {
        let page = Rect::new(
            f64::from(p.x),
            f64::from(p.y),
            f64::from(p.x) + f64::from(p.pixmap.width()),
            f64::from(p.y) + f64::from(p.pixmap.height()),
        );
        Some((page, underlay::render_image(&p.pixmap)?))
    });
    Some((pixmap, image))
}

fn start(
    state: &Rc<RefCell<Redactions>>,
    slot: usize,
    key: Key,
    base: Arc<RgbaImage>,
    canvas: WeakEntity<EditorCanvas>,
    cx: &mut App,
) {
    let pixmap = {
        let mut s = state.borrow_mut();
        if let Some(sl) = s.slots.get_mut(slot) {
            sl.pending = Some(key);
        }
        s.base
            .as_ref()
            .filter(|(revision, _)| *revision == key.base)
            .map(|(_, p)| p.clone())
    };
    let job = cx
        .background_executor()
        .spawn(async move { build(&base, pixmap, key) });
    let state = state.clone();
    cx.spawn(async move |cx| {
        let built = job.await;
        {
            let mut s = state.borrow_mut();
            let s = &mut *s;
            let Some(sl) = s.slots.get_mut(slot).filter(|sl| sl.pending == Some(key)) else {
                return;
            };
            sl.pending = None;
            if let Some((pixmap, image)) = built {
                if let Some((page, image)) = image {
                    s.next_id += 1;
                    let fresh = Patch {
                        id: s.next_id,
                        key,
                        page,
                        image,
                    };
                    if let Some(old) = sl.current.replace(fresh) {
                        s.retired.push(old.image);
                    }
                } else if let Some(old) = sl.current.take() {
                    s.retired.push(old.image);
                }
                s.base = Some((key.base, pixmap));
            }
        }
        let _ = canvas.update(cx, |_, cx| cx.notify());
    })
    .detach();
}

/// Builds blur and pixelate patches off the main thread and paints the latest one each redaction has.
pub(super) fn painter(
    state: Rc<RefCell<Redactions>>,
    doc: Entity<DocModel>,
    canvas: WeakEntity<EditorCanvas>,
) -> RedactionPainter {
    let lease = Lease::new(canvas.entity_id().as_u64());
    Box::new(move |items, vp, bounds, window, cx| {
        let (base, revision) = {
            let d = doc.read(cx);
            (d.base().clone(), d.base_revision())
        };
        let keys: Vec<Key> = items.iter().filter_map(|i| Key::of(revision, i)).collect();
        let mut wanted = Vec::new();
        let mut paint = Vec::new();
        {
            let mut s = state.borrow_mut();
            for image in s.retired.drain(..) {
                drop_image(window, image);
            }
            if s.base.as_ref().is_some_and(|(r, _)| *r != revision) {
                s.base = None;
            }
            if s.align(&keys) {
                lease.release_group(cx, "patch/");
            }
            for name in lease.evicted(cx, "patch/") {
                if let Some(slot) = name.strip_prefix("patch/").and_then(|i| i.parse().ok()) {
                    s.release(slot);
                }
            }
            for image in s.retired.drain(..) {
                drop_image(window, image);
            }
            let s = &mut *s;
            s.painted.clear();
            for (i, (slot, key)) in s.slots.iter().zip(&keys).enumerate() {
                let fresh = slot.current.as_ref().is_some_and(|p| p.key == *key);
                if !fresh && slot.pending.is_none() {
                    wanted.push((i, *key));
                }
                if let Some(p) = &slot.current {
                    s.painted.push(p.id);
                    lease.hold(cx, &format!("patch/{i}"), &p.image);
                    paint.push((screen_bounds(bounds, vp, p.page), p.image.clone()));
                }
            }
        }
        for (at, image) in paint {
            if let Err(error) = window.paint_image(bounds, at, Corners::default(), image, 0, false)
            {
                tracing::warn!(%error, "a redaction patch could not be painted");
            }
        }
        for (slot, key) in wanted {
            start(&state, slot, key, base.clone(), canvas.clone(), cx);
        }
    })
}

fn drop_image(window: &mut Window, image: Arc<RenderImage>) {
    if let Err(error) = window.drop_image(image) {
        tracing::debug!(%error, "dropping a replaced redaction patch");
    }
}

impl EditorCanvas {
    pub fn redactions(&self) -> std::cell::Ref<'_, Redactions> {
        self.redactions.borrow()
    }
}
