#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::{Editor, launch, launch_with_text};
use crate::canvas_support::renamed_nunito_text;
use es_editor::inspector::options::Control;
use es_geom::Point;
use es_model::{ShapeKind, Tool};
use es_ui::tokens::ThemeMode;
use gpui::{KeyDownEvent, Keystroke, PlatformInput, point};

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

const FACES: [&str; 10] = [
    "Face00", "Face01", "Face02", "Face03", "Face04", "Face05", "Face06", "Face07", "Face08",
    "Face09",
];

impl Editor {
    fn type_key(&mut self, key: &'static str) {
        self.send(PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }));
    }

    fn open_font_menu(&mut self) {
        let b = self.bounds(Control::FontFamily).unwrap();
        self.click_at(b.center());
    }
}

#[test]
fn the_font_menu_lists_the_bundled_families_then_the_installed_ones_in_order() {
    let text = renamed_nunito_text("font-order", &FACES);
    let mut e = launch_with_text(ThemeMode::Light, text);
    e.new_text();
    assert_eq!(e.text().font_family, es_model::FontFamily::Pro);
    e.open_font_menu();
    for _ in 0..3 {
        e.type_key("down");
    }
    e.type_key("enter");
    assert_eq!(
        e.text().font_family,
        es_model::FontFamily::System("Face00".into()),
        "Inter, Inter Tight, Nunito, then the first installed family"
    );
}

impl Editor {
    fn keys(&mut self, keys: &[&'static str]) {
        let handle = self.app.window.into();
        for key in keys {
            self.app
                .cx
                .update_window(handle, |_, window, cx| {
                    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                })
                .unwrap();
            self.app.settle();
        }
    }

    fn type_size(&mut self, text: &str) {
        self.click(Control::FontSize);
        let mut keys: Vec<&'static str> = text
            .chars()
            .map(|c| &*Box::leak(c.to_string().into_boxed_str()))
            .collect();
        keys.push("enter");
        self.keys(&keys);
    }

    fn size_text(&self) -> String {
        self.options
            .read_with(&self.app.cx, |o, cx| o.font_size_text(cx))
            .unwrap()
            .to_string()
    }
}

#[test]
fn a_typed_font_size_applies_on_enter_and_is_held_to_the_size_range() {
    let mut e = launch(ThemeMode::Light);
    e.new_text();
    let tool = e.tool();
    e.type_size("48");
    assert_eq!(e.text().font_size, 48.0);
    assert_eq!(e.size_text(), "48");
    assert_eq!(
        e.tool(),
        tool,
        "digits typed in the field are not tool keys"
    );
    e.type_size("1000");
    assert_eq!(e.text().font_size, 300.0, "clamped to the largest size");
    assert_eq!(e.size_text(), "300");
    e.type_size("1");
    assert_eq!(e.text().font_size, 4.0, "clamped to the smallest size");
    e.edit(|e| e.undo());
    assert_eq!(
        e.text().font_size,
        300.0,
        "each typed size is one undo step"
    );
}

#[test]
fn text_that_is_not_a_size_puts_the_old_size_back() {
    let mut e = launch(ThemeMode::Light);
    e.new_text();
    let size = e.text().font_size;
    let shown = e.size_text();
    e.type_size("big");
    assert_eq!(e.text().font_size, size);
    assert_eq!(e.size_text(), shown);
    e.click(Control::FontSize);
    e.keys(&["backspace", "enter"]);
    assert_eq!(e.text().font_size, size);
    assert_eq!(e.size_text(), shown, "an emptied field reverts");
    e.click(Control::FontSize);
    e.keys(&["6", "0", "escape"]);
    assert_eq!(e.text().font_size, size, "escape drops what was typed");
    assert_eq!(e.size_text(), shown);
}
