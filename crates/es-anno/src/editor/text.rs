use super::history::GestureKey;
use super::{Editor, GestureId};
use crate::Changes;
use es_geom::{PVec, Point};
use es_model::{AnnoShape, AnnoShapeId, ShapeKind, Swatch, TextAlign, TextBorder, TextProps};

impl Editor {
    /// The click lands on the first line's middle at the alignment's anchor, which typing keeps.
    pub fn create_text(&mut self, page: Point) -> Changes {
        let mut ch = Changes::default();
        if !page.x.is_finite() || !page.y.is_finite() {
            return ch;
        }
        self.stop_text_into(&mut ch);
        let c = &self.current;
        let props = TextProps {
            swatch: c.swatch.clone(),
            font_size: c.font_size,
            font_family: c.font_family,
            is_bold: c.bold,
            is_italic: c.italic,
            is_underline: c.underline,
            align: c.align,
            ..TextProps::default()
        };
        let size = self.doc.measure_text(&props);
        let x = match props.align {
            TextAlign::Start => page.x,
            TextAlign::Middle => page.x - size.width / 2.0,
            TextAlign::End => page.x - size.width,
        };
        let shape = AnnoShape::new(x, page.y - size.height / 2.0, ShapeKind::Text(props));
        let id = shape.id;
        self.begin_gesture(GestureKey::Text(id), &mut ch);
        self.edit(&mut ch, |doc| doc.add(shape));
        self.set_selection([id], &mut ch);
        self.editing_text_id = Some(id);
        ch
    }

    /// Everything until `stop_text` is one undo entry.
    pub fn begin_text(&mut self, id: AnnoShapeId) -> Changes {
        let mut ch = Changes::default();
        if self.editing_text_id == Some(id) || !self.doc.shape(id).is_some_and(|s| s.is_text()) {
            return ch;
        }
        self.stop_text_into(&mut ch);
        self.begin_gesture(GestureKey::Text(id), &mut ch);
        self.editing_text_id = Some(id);
        self.set_selection([id], &mut ch);
        ch.needs_redraw = true;
        ch
    }

    /// Centred text grows evenly both ways and end-aligned text grows leftwards.
    pub fn update_text(&mut self, text: &str) -> Changes {
        let mut ch = Changes::default();
        let Some(id) = self.editing_text_id else {
            return ch;
        };
        let Some(ShapeKind::Text(props)) = self.doc.shape(id).map(|s| &s.kind) else {
            return ch;
        };
        if props.text == text {
            return ch;
        }
        let mut next = props.clone();
        next.text = text.to_owned();
        let grown = self.doc.measure_text(&next).width - self.doc.measure_text(props).width;
        let shift = match props.align {
            TextAlign::Start => 0.0,
            TextAlign::Middle => grown / 2.0,
            TextAlign::End => grown,
        };
        self.begin_gesture(GestureKey::Text(id), &mut ch);
        self.edit(&mut ch, |doc| {
            doc.update(id, |s| {
                let dx = if s.mirrored { -shift } else { shift };
                let d = PVec::rot(PVec::new(dx, 0.0), s.rotation);
                s.x -= d.x;
                s.y -= d.y;
                s.kind = ShapeKind::Text(next);
            })
        });
        ch
    }

    /// Applies to the one selected text shape; calls sharing `gesture` make one undo entry, or join the text session being edited.
    pub fn set_text_fill(&mut self, fill: Option<Swatch>, gesture: GestureId) -> Changes {
        self.update_selected_text(gesture, move |p| p.fill = fill.clone())
    }

    /// A non-finite or negative width leaves the shape as it was.
    pub fn set_text_border(&mut self, border: Option<TextBorder>, gesture: GestureId) -> Changes {
        if border
            .as_ref()
            .is_some_and(|b| !b.width.is_finite() || b.width < 0.0)
        {
            return Changes::default();
        }
        self.update_selected_text(gesture, move |p| p.border = border.clone())
    }

    fn update_selected_text(
        &mut self,
        gesture: GestureId,
        change: impl Fn(&mut TextProps),
    ) -> Changes {
        let mut ch = Changes::default();
        let Some(id) = self.single_selected_text() else {
            return ch;
        };
        let key = match self.editing_text_id {
            Some(id) => GestureKey::Text(id),
            None => GestureKey::Style(gesture),
        };
        self.begin_gesture(key, &mut ch);
        self.edit(&mut ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Text(p) = &mut s.kind {
                    change(p);
                }
            })
        });
        ch
    }

    /// Text left empty is removed.
    pub fn stop_text(&mut self) -> Changes {
        let mut ch = Changes::default();
        self.stop_text_into(&mut ch);
        ch
    }

    pub(crate) fn stop_text_into(&mut self, ch: &mut Changes) {
        let Some(id) = self.editing_text_id.take() else {
            return;
        };
        ch.needs_redraw = true;
        if self.is_blank_text(id) {
            self.edit(ch, |doc| doc.delete(&[id]));
            let rest: Vec<_> = self.selected_ids.iter().copied().collect();
            self.set_selection(rest, ch);
        }
        if self.history.is_open(GestureKey::Text(id)) {
            self.end_gesture(ch);
        }
    }
}
