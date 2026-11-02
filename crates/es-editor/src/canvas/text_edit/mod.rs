mod geometry;
mod input;
mod keys;

use super::EditorCanvas;
use super::paths::{self, Bucket, Painted};
use es_anno::{BundledText, Viewport};
use es_geom::FillRule;
use es_model::{AnnoShape, AnnoShapeId, ShapeKind};
use es_text::TextLayout;
use gpui::{App, Context, ElementInputHandler, Entity, Hsla, Pixels, Task, Window};
use kurbo::Shape;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

pub const CARET_BLINK: Duration = Duration::from_millis(530);
const CARET_WIDTH: f64 = 2.0;
const SELECTION_ALPHA: f32 = 0.3;
const MARK_THICKNESS: f64 = 1.5;

/// What the text being edited shows, in byte offsets into the shape's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEditState {
    pub id: AnnoShapeId,
    pub selection: Range<usize>,
    pub marked: Option<Range<usize>>,
    pub caret_visible: bool,
}

pub(super) struct TextEdit {
    id: AnnoShapeId,
    selection: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    caret_on: bool,
    epoch: u64,
    _blink: Task<()>,
}

impl TextEdit {
    fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }
}

fn clamp(text: &str, mut i: usize) -> usize {
    i = i.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn clamp_range(text: &str, r: &Range<usize>) -> Range<usize> {
    let (a, b) = (clamp(text, r.start), clamp(text, r.end));
    a.min(b)..a.max(b)
}

fn props_text(shape: &AnnoShape) -> Option<&str> {
    match &shape.kind {
        ShapeKind::Text(p) => Some(&p.text),
        _ => None,
    }
}

impl EditorCanvas {
    /// While true, every key but Ctrl+S belongs to the text being edited.
    pub fn is_editing_text(&self, cx: &App) -> bool {
        self.doc.read(cx).editor().editing_text_id().is_some()
    }

    pub fn text_edit_state(&self) -> Option<TextEditState> {
        self.text_edit.as_ref().map(|e| TextEditState {
            id: e.id,
            selection: e.selection.clone(),
            marked: e.marked.clone(),
            caret_visible: e.caret_on,
        })
    }

    /// Follows the engine: editing starts with the caret at the end of the text.
    pub(super) fn sync_text_edit(&mut self, cx: &mut Context<Self>) {
        let editing = self.doc.read(cx).editor().editing_text_id();
        if editing == self.text_edit.as_ref().map(|e| e.id) {
            return;
        }
        self.text_edit = editing.map(|id| {
            let end = self
                .editing_shape(cx)
                .and_then(props_text)
                .map_or(0, str::len);
            TextEdit {
                id,
                selection: end..end,
                reversed: false,
                marked: None,
                caret_on: true,
                epoch: 0,
                _blink: Task::ready(()),
            }
        });
        self.restart_blink(cx);
    }

    fn editing_shape<'a>(&self, cx: &'a App) -> Option<&'a AnnoShape> {
        let editor = self.doc.read(cx).editor();
        editor.doc().shape(editor.editing_text_id()?)
    }

    fn editing_text(&self, cx: &App) -> Option<String> {
        self.editing_shape(cx)
            .and_then(props_text)
            .map(str::to_owned)
    }

    fn editing_layout(&self, cx: &App) -> Option<(AnnoShape, Arc<TextLayout>)> {
        let shape = self.editing_shape(cx)?.clone();
        let ShapeKind::Text(props) = &shape.kind else {
            return None;
        };
        let layout = self.doc.read(cx).text().layout(props);
        Some((shape, layout))
    }

    /// The caret stays solid while it moves and starts blinking again from there.
    fn restart_blink(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        edit.caret_on = true;
        edit.epoch += 1;
        let epoch = edit.epoch;
        edit._blink = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CARET_BLINK).await;
                let blinking = this.update(cx, |canvas, cx| {
                    let Some(e) = canvas.text_edit.as_mut().filter(|e| e.epoch == epoch) else {
                        return false;
                    };
                    e.caret_on = !e.caret_on;
                    cx.notify();
                    true
                });
                if !matches!(blinking, Ok(true)) {
                    break;
                }
            }
        });
    }

    /// Puts `new` over `range`; a marked replacement stays composing until it is committed.
    fn replace_text(
        &mut self,
        range: Range<usize>,
        new: &str,
        mark: bool,
        selected: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = self.editing_text(cx) else {
            return;
        };
        let range = clamp_range(&text, &range);
        let next = format!("{}{new}{}", &text[..range.start], &text[range.end..]);
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.update_text(&next)));
        if let Some(e) = self.text_edit.as_mut() {
            let at = range.start + new.len();
            e.marked = (mark && !new.is_empty()).then_some(range.start..at);
            e.selection = match selected {
                Some(s) => clamp_range(&next, &((range.start + s.start)..(range.start + s.end))),
                None => at..at,
            };
            e.reversed = false;
        }
        self.restart_blink(cx);
        cx.notify();
    }

    fn move_caret(&mut self, to: usize, extend: bool, cx: &mut Context<Self>) {
        let Some(text) = self.editing_text(cx) else {
            return;
        };
        let to = clamp(&text, to);
        let Some(e) = self.text_edit.as_mut() else {
            return;
        };
        if extend {
            if e.reversed {
                e.selection.start = to;
            } else {
                e.selection.end = to;
            }
            if e.selection.end < e.selection.start {
                e.reversed = !e.reversed;
                e.selection = e.selection.end..e.selection.start;
            }
        } else {
            e.selection = to..to;
            e.reversed = false;
        }
        self.restart_blink(cx);
        cx.notify();
    }

    fn select_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        if let Some(e) = self.text_edit.as_mut() {
            e.selection = range;
            e.reversed = false;
        }
        self.restart_blink(cx);
        cx.notify();
    }

    pub(super) fn commit_text(&mut self, cx: &mut Context<Self>) {
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.stop_text()));
        self.text_edit = None;
        cx.notify();
    }

    /// A press on the text being edited moves its caret there; Shift extends the selection.
    pub(super) fn place_caret(
        &mut self,
        at: gpui::Point<Pixels>,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        self.sync_text_edit(cx);
        let Some((shape, layout)) = self.editing_layout(cx) else {
            return;
        };
        let vp = self.viewport.read(cx).viewport();
        let ts = geometry::to_window(&shape, &vp, gpui::Point::default());
        let local = ts.inverse() * self.local(at);
        if layout.lines.is_empty() {
            return;
        }
        let index = layout.hit_test(local.x, local.y);
        self.move_caret(index, extend, cx);
    }

    /// The selection, the composing underline and the caret, drawn above every shape.
    pub(super) fn text_edit_overlay(
        &self,
        vp: &Viewport,
        origin: gpui::Point<Pixels>,
        bucket: Bucket,
        accent: Hsla,
        text: &BundledText,
        cx: &App,
    ) -> Vec<Painted> {
        let mut out = Vec::new();
        let (Some(e), Some(shape)) = (self.text_edit.as_ref(), self.editing_shape(cx)) else {
            return out;
        };
        let ShapeKind::Text(props) = &shape.kind else {
            return out;
        };
        let layout = text.layout(props);
        let ts = geometry::to_window(shape, vp, origin);
        let px = 1.0 / vp.scale.max(1e-9);
        let mut fill = |r: es_geom::Rect, color: Hsla| {
            paths::fill(
                &r.to_path(0.1),
                ts,
                FillRule::NonZero,
                color,
                bucket,
                &mut out,
            );
        };
        for r in geometry::spans(&layout, &e.selection) {
            fill(r, accent.opacity(SELECTION_ALPHA));
        }
        for r in e.marked.iter().flat_map(|m| geometry::spans(&layout, m)) {
            let y = r.y1 - MARK_THICKNESS * px;
            fill(es_geom::Rect::new(r.x0, y, r.x1, r.y1), accent);
        }
        if e.caret_on && e.selection.is_empty() {
            fill(
                geometry::caret(&layout, e.cursor(), CARET_WIDTH * px),
                accent,
            );
        }
        out
    }
}

/// Text input reaches the canvas only while a text shape is being edited.
pub(super) fn register_input(
    view: &Entity<EditorCanvas>,
    bounds: gpui::Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let focus = {
        let c = view.read(cx);
        if c.text_edit.is_none() {
            return;
        }
        c.focus.clone()
    };
    window.handle_input(&focus, ElementInputHandler::new(bounds, view.clone()), cx);
}
