use super::FrameStats;
use super::paths::{self, Bucket, Painted, Placement};
use es_anno::document::page_transform;
use es_anno::{DrawItem, Editor};
use es_geom::freehand::ChunkKey;
use es_geom::{Affine, FillRule};
use es_model::{AnnoShapeId, ShapeKind};
use std::collections::HashMap;

struct LiveChunk {
    bucket: Option<Bucket>,
    built: Vec<Painted>,
    placement: Option<Placement>,
    placed: Vec<Painted>,
}

/// GPU paths of the stroke being drawn, per chunk, so a move re-tessellates only the chunks it changed.
#[derive(Default)]
pub(super) struct LiveCache {
    chunks: HashMap<(AnnoShapeId, ChunkKey), LiveChunk>,
}

impl LiveCache {
    pub(super) fn paths(
        &mut self,
        editor: &Editor,
        bucket: Bucket,
        at: Placement,
        stats: &mut FrameStats,
    ) -> Vec<Painted> {
        let live = editor.live_stroke().and_then(|(id, stroke)| {
            let shape = editor.doc().shape(id)?;
            match &shape.kind {
                ShapeKind::Draw(props) => Some((id, stroke, shape, props)),
                _ => None,
            }
        });
        let Some((id, stroke, shape, props)) = live else {
            self.chunks.clear();
            return Vec::new();
        };
        let chunks = stroke.chunks();
        self.chunks
            .retain(|(owner, key), _| *owner == id && chunks.iter().any(|c| c.key == *key));
        let m = page_transform(shape);
        let transform = Affine::new([m.a, m.b, m.c, m.d, m.e, m.f]);
        let color = props.swatch.to_rgba();
        let mut out = Vec::new();
        for chunk in chunks {
            let entry = self
                .chunks
                .entry((id, chunk.key))
                .or_insert_with(|| LiveChunk {
                    bucket: None,
                    built: Vec::new(),
                    placement: None,
                    placed: Vec::new(),
                });
            if entry.bucket != Some(bucket) {
                let item = DrawItem::Group {
                    transform,
                    opacity: shape.opacity,
                    items: vec![DrawItem::Fill {
                        path: chunk.path.as_ref().clone(),
                        color,
                        rule: FillRule::NonZero,
                    }],
                };
                entry.built = paths::tessellate(&[item], bucket);
                entry.bucket = Some(bucket);
                entry.placement = None;
                stats.live_chunks_tessellated += 1;
            }
            if entry.placement != Some(at) {
                entry.placed = entry
                    .built
                    .iter()
                    .map(|p| Painted {
                        path: at.place(&p.path),
                        color: p.color,
                    })
                    .collect();
                entry.placement = Some(at);
            }
            out.extend(entry.placed.iter().cloned());
        }
        out
    }
}
