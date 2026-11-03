use super::RedactionPainter;
use super::redaction::{premultiplied, screen_bounds};
use super::underlay;
use crate::session::doc::DocModel;
use es_anno::DrawItem;
use es_geom::{BezPath, Rect, Rgba};
use gpui::{Corners, Entity, RenderImage, Window};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia::Pixmap;

#[derive(Clone, PartialEq)]
struct Key {
    base: u64,
    path: BezPath,
    width: f64,
    color: Rgba,
}

struct Patch {
    key: Key,
    page: Rect,
    image: Arc<RenderImage>,
}

#[derive(Default)]
pub struct Highlighters {
    patches: Vec<Patch>,
    base: Option<(u64, Arc<Pixmap>)>,
}

impl Highlighters {
    pub fn held(&self) -> usize {
        self.patches.len()
    }

    fn patch(&mut self, base: &image::RgbaImage, key: Key) -> Option<Patch> {
        let pixmap = match &self.base {
            Some((revision, p)) if *revision == key.base => p.clone(),
            _ => {
                let p = Arc::new(premultiplied(base)?);
                self.base = Some((key.base, p.clone()));
                p
            }
        };
        let patch =
            es_render::effects::highlighter_patch(&pixmap, &key.path, key.width, key.color)?;
        let page = Rect::new(
            f64::from(patch.x),
            f64::from(patch.y),
            f64::from(patch.x) + f64::from(patch.pixmap.width()),
            f64::from(patch.y) + f64::from(patch.pixmap.height()),
        );
        Some(Patch {
            key,
            page,
            image: underlay::render_image(&patch.pixmap)?,
        })
    }
}

/// Paints each highlighter stroke as the base multiplied by the stroke, the pixels export produces.
pub(super) fn painter(state: Rc<RefCell<Highlighters>>, doc: Entity<DocModel>) -> RedactionPainter {
    Box::new(move |items, vp, bounds, window: &mut Window, cx| {
        let (base, revision) = {
            let d = doc.read(cx);
            (d.base().clone(), d.base_revision())
        };
        let mut s = state.borrow_mut();
        if s.base.as_ref().is_some_and(|(r, _)| *r != revision) {
            s.base = None;
        }
        let mut old = std::mem::take(&mut s.patches);
        for item in items {
            let DrawItem::Highlighter { path, width, color } = item else {
                continue;
            };
            let key = Key {
                base: revision,
                path: path.clone(),
                width: *width,
                color: *color,
            };
            let kept = old.iter().position(|p| p.key == key).map(|i| old.remove(i));
            if let Some(patch) = kept.or_else(|| s.patch(&base, key)) {
                s.patches.push(patch);
            }
        }
        for stale in old {
            if let Err(error) = window.drop_image(stale.image) {
                tracing::debug!(%error, "dropping a replaced highlighter patch");
            }
        }
        for patch in &s.patches {
            let at = screen_bounds(bounds, vp, patch.page);
            if let Err(error) = window.paint_image(
                bounds,
                at,
                Corners::default(),
                patch.image.clone(),
                0,
                false,
            ) {
                tracing::warn!(%error, "a highlighter patch could not be painted");
            }
        }
    })
}

impl super::EditorCanvas {
    pub fn highlighters(&self) -> std::cell::Ref<'_, Highlighters> {
        self.highlighters.borrow()
    }
}
