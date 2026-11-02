use super::EditorCanvas;
use super::paths::Bucket;
use super::underlay;
use es_anno::draw::draw_items;
use es_anno::{BuildContext, Document, DrawItem, Viewport};
use es_geom::{Affine, Rect};
use es_model::{AnnoShape, AnnoShapeId};
use es_render::effects::Patch;
use gpui::{Context, RenderImage};
use std::collections::HashMap;
use std::sync::Arc;

/// The blur is soft at any zoom, so device pixels past this only cost memory.
const MAX_DEVICE_SCALE: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    revision: u64,
    bucket: Bucket,
}

impl Key {
    fn device_scale(&self) -> f64 {
        (self.bucket.scale() * f64::from(self.bucket.scale_factor.max(1.0))).min(MAX_DEVICE_SCALE)
    }
}

type Built = Option<(Rect, Arc<RenderImage>)>;

#[derive(Default)]
struct Slot {
    built: Option<Key>,
    image: Built,
    pending: Option<Key>,
}

/// One shadow patch per text shape: the one on screen, kept until its replacement is built.
#[derive(Default)]
pub struct TextShadows {
    slots: HashMap<AnnoShapeId, Slot>,
    queued: Vec<(AnnoShapeId, Key, Vec<DrawItem>)>,
}

impl TextShadows {
    /// Shadow patches built and in memory.
    pub fn held(&self) -> usize {
        self.slots.values().filter(|s| s.image.is_some()).count()
    }

    /// Where a text shape's shadow patch sits, in page space.
    pub fn page_rect(&self, id: AnnoShapeId) -> Option<Rect> {
        self.slots.get(&id)?.image.as_ref().map(|(r, _)| *r)
    }

    pub fn building(&self) -> usize {
        self.slots.values().filter(|s| s.pending.is_some()).count()
    }

    /// Queues a rebuild when the shape or zoom bucket moved on, and returns the patch to paint now.
    pub(super) fn track(
        &mut self,
        document: &Document,
        shape: &AnnoShape,
        revision: u64,
        bucket: Bucket,
        build: &BuildContext,
    ) -> Built {
        let key = Key { revision, bucket };
        let slot = self.slots.entry(shape.id).or_default();
        if slot.built != Some(key) && slot.pending != Some(key) {
            slot.pending = Some(key);
            self.queued
                .push((shape.id, key, draw_items(document, shape, build)));
        }
        slot.image.clone()
    }

    pub(super) fn retain(
        &mut self,
        keep: impl Fn(AnnoShapeId) -> bool,
        retired: &mut Vec<Arc<RenderImage>>,
    ) {
        self.slots.retain(|id, slot| {
            let kept = keep(*id);
            if !kept && let Some((_, image)) = slot.image.take() {
                retired.push(image);
            }
            kept
        });
    }
}

/// The shadow under a text shape's glyphs, built by the same function export paints with, at
/// `scale` device pixels per page pixel.
pub fn build_patch(items: &[DrawItem], scale: f64) -> Option<Patch> {
    fn find(
        items: &[DrawItem],
        ts: Affine,
        opacity: f64,
    ) -> Option<(&es_geom::BezPath, Affine, f64)> {
        items.iter().find_map(|item| match item {
            DrawItem::Text { glyphs, .. } => Some((glyphs, ts, opacity)),
            DrawItem::Group {
                transform,
                opacity: o,
                items,
            } => find(items, ts * *transform, opacity * o),
            _ => None,
        })
    }
    let (glyphs, ts, opacity) = find(items, Affine::scale(scale), 1.0)?;
    es_render::paint::text_shadow_patch(glyphs, ts, opacity)
}

fn render(items: &[DrawItem], scale: f64) -> Built {
    let patch = build_patch(items, scale)?;
    let (x, y) = (f64::from(patch.x), f64::from(patch.y));
    let (w, h) = (
        f64::from(patch.pixmap.width()),
        f64::from(patch.pixmap.height()),
    );
    let page = Rect::new(x / scale, y / scale, (x + w) / scale, (y + h) / scale);
    Some((page, underlay::render_image(&patch.pixmap)?))
}

/// A page rect in element coordinates.
pub(super) fn on_screen(vp: &Viewport, page: Rect) -> Rect {
    let s = vp.scale;
    Rect::new(
        vp.origin.x + page.x0 * s,
        vp.origin.y + page.y0 * s,
        vp.origin.x + page.x1 * s,
        vp.origin.y + page.y1 * s,
    )
}

impl EditorCanvas {
    pub fn text_shadows(&self) -> &TextShadows {
        &self.text_shadows
    }

    pub(super) fn start_text_shadows(&mut self, cx: &mut Context<Self>) {
        for (id, key, items) in std::mem::take(&mut self.text_shadows.queued) {
            let job = cx
                .background_executor()
                .spawn(async move { render(&items, key.device_scale()) });
            cx.spawn(async move |this, cx| {
                let built = job.await;
                let _ = this.update(cx, |canvas, cx| {
                    let Some(slot) = canvas
                        .text_shadows
                        .slots
                        .get_mut(&id)
                        .filter(|s| s.pending == Some(key))
                    else {
                        return;
                    };
                    slot.pending = None;
                    slot.built = Some(key);
                    if let Some((_, old)) = std::mem::replace(&mut slot.image, built) {
                        canvas.retired.push(old);
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}
