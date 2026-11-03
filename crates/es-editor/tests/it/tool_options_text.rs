#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{Editor, launch};
use es_editor::inspector::options::Control;
use es_geom::Point;
use es_model::{ShapeKind, Tool};
use es_ui::tokens::ThemeMode;
use gpui::point;

impl Editor {
    fn new_text(&mut self) {
        self.arm(Tool::Text);
        self.edit(|e| e.create_text(Point::new(200., 200.)));
        self.edit(|e| e.update_text("Note"));
        let id = self.texts().last().map(|(id, _)| *id).unwrap();
        self.edit(|e| e.stop_text());
        self.edit(|e| e.select([id]));
    }

    fn texts(&self) -> Vec<(es_model::AnnoShapeId, es_model::TextProps)> {
        self.window.read_with(&self.app.cx, |w, cx| {
            let doc = w.session().unwrap().doc.read(cx);
            doc.editor()
                .doc()
                .shapes()
                .iter()
                .filter_map(|s| match &s.kind {
                    ShapeKind::Text(p) => Some((s.id, p.clone())),
                    _ => None,
                })
                .collect()
        })
    }

    fn text(&self) -> es_model::TextProps {
        self.texts().last().unwrap().1.clone()
    }

    fn pick_width(&mut self, index: usize) {
        let b = self.bounds(Control::TextBorderWidth).unwrap();
        let cell = b.size.width / 3.;
        self.click_at(point(
            b.origin.x + cell * (index as f32 + 0.5),
            b.center().y,
        ));
    }
}

#[test]
fn a_background_picked_for_the_selected_text_fills_it_and_none_clears_it() {
    let mut e = launch(ThemeMode::Light);
    e.new_text();
    assert_eq!(e.text().fill, None);
    e.pick_swatch(Control::TextFill, 6);
    assert_eq!(e.text().fill.map(|s| s.id), Some("blue".into()));
    e.click(Control::TextFillNone);
    assert_eq!(e.text().fill, None);
}

#[test]
fn the_border_width_pills_show_only_while_a_border_is_set() {
    let mut e = launch(ThemeMode::Light);
    e.new_text();
    assert!(e.bounds(Control::TextBorder).is_some());
    assert!(e.bounds(Control::TextBorderWidth).is_none());
    e.pick_swatch(Control::TextBorder, 1);
    let border = e.text().border.unwrap();
    assert_eq!((border.swatch.id.as_str(), border.width), ("red", 3.0));
    assert!(e.bounds(Control::TextBorderWidth).is_some());
    e.pick_width(2);
    let border = e.text().border.unwrap();
    assert_eq!((border.swatch.id.as_str(), border.width), ("red", 5.0));
    e.click(Control::TextBorderNone);
    assert_eq!(e.text().border, None);
    assert!(e.bounds(Control::TextBorderWidth).is_none());
}

#[test]
fn new_text_starts_with_the_chosen_background_and_border() {
    let mut e = launch(ThemeMode::Light);
    e.arm(Tool::Text);
    e.pick_swatch(Control::TextFill, 9);
    e.pick_swatch(Control::TextBorder, 4);
    e.pick_width(0);
    e.edit(|e| e.create_text(Point::new(300., 300.)));
    e.edit(|e| e.update_text("Note"));
    let props = e.text();
    assert_eq!(props.fill.map(|s| s.id), Some("white".into()));
    let border = props.border.unwrap();
    assert_eq!((border.swatch.id.as_str(), border.width), ("green", 2.0));
}

#[test]
fn undo_reverts_a_background_and_a_border_one_step_each() {
    let mut e = launch(ThemeMode::Light);
    e.new_text();
    e.pick_swatch(Control::TextFill, 6);
    e.pick_swatch(Control::TextBorder, 1);
    e.edit(|e| e.undo());
    assert_eq!(e.text().border, None);
    assert!(e.text().fill.is_some());
    e.edit(|e| e.undo());
    assert_eq!(e.text().fill, None);
}
