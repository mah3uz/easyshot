use super::{CARET_WIDTH, geometry, props_text};
use crate::canvas::EditorCanvas;
use gpui::{Bounds, Context, EntityInputHandler, Pixels, UTF16Selection, Window};
use std::ops::Range;

fn from_utf16(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (i, ch) in text.char_indices() {
        if units >= offset {
            return i;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn to_utf16(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

fn range_from_utf16(text: &str, r: &Range<usize>) -> Range<usize> {
    from_utf16(text, r.start)..from_utf16(text, r.end)
}

fn range_to_utf16(text: &str, r: &Range<usize>) -> Range<usize> {
    to_utf16(text, r.start)..to_utf16(text, r.end)
}

impl EditorCanvas {
    /// The range a replacement without one lands on: the composing text, else the selection.
    fn target(&self, text: &str, range_utf16: Option<Range<usize>>) -> Option<Range<usize>> {
        let e = self.text_edit.as_ref()?;
        Some(match range_utf16 {
            Some(r) => range_from_utf16(text, &r),
            None => e.marked.clone().unwrap_or_else(|| e.selection.clone()),
        })
    }
}

impl EntityInputHandler for EditorCanvas {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.editing_text(cx)?;
        let range = range_from_utf16(&text, &range_utf16);
        adjusted.replace(range_to_utf16(&text, &range));
        Some(text[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.editing_text(cx)?;
        let e = self.text_edit.as_ref()?;
        Some(UTF16Selection {
            range: range_to_utf16(&text, &e.selection),
            reversed: e.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let text = self.editing_text(cx)?;
        let marked = self.text_edit.as_ref()?.marked.as_ref()?;
        Some(range_to_utf16(&text, marked))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(e) = self.text_edit.as_mut() {
            e.marked = None;
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_text_edit(cx);
        let Some(text) = self.editing_text(cx) else {
            return;
        };
        if let Some(range) = self.target(&text, range_utf16) {
            self.replace_text(range, new, false, None, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new: &str,
        selected_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_text_edit(cx);
        let Some(text) = self.editing_text(cx) else {
            return;
        };
        let selected = selected_utf16.map(|r| range_from_utf16(new, &r));
        if let Some(range) = self.target(&text, range_utf16) {
            self.replace_text(range, new, true, selected, cx);
        }
    }

    /// The candidate window goes beside the range's first line, boxed after the shape's rotation.
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (shape, layout) = self.editing_layout(cx)?;
        let text = props_text(&shape)?;
        let range = range_from_utf16(text, &range_utf16);
        let vp = self.viewport.read(cx).viewport();
        let ts = geometry::to_window(&shape, &vp, element_bounds.origin);
        let width = CARET_WIDTH / vp.scale.max(1e-9);
        let local = geometry::spans(&layout, &range)
            .into_iter()
            .next()
            .unwrap_or_else(|| geometry::caret(&layout, range.start, width));
        Some(geometry::window_bounds(local, ts))
    }

    fn character_index_for_point(
        &mut self,
        at: gpui::Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let (shape, layout) = self.editing_layout(cx)?;
        if layout.lines.is_empty() {
            return Some(0);
        }
        let vp = self.viewport.read(cx).viewport();
        let ts = geometry::to_window(&shape, &vp, self.bounds.origin);
        let p = ts.inverse() * es_geom::Point::new(f64::from(at.x), f64::from(at.y));
        let index = layout.hit_test(p.x, p.y);
        Some(to_utf16(props_text(&shape)?, index))
    }
}
