#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{TEXT, opened, rect};

use std::{cell::RefCell, rc::Rc};

use es_anno::crop::{AspectPreset, CropHandle};
use es_editor::session::crop::{CropModel, CropTarget};
use es_editor::toolbar::{Control, ToolRow};
use es_editor::window::{TITLE_BAR_HEIGHT, TOOL_ROW_HEIGHT};
use es_editor::{EditorSession, EditorWindow};
use es_geom::Point;
use es_model::{BackgroundSettings, Tool};
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Bounds, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, point, px,
};
use gpui_component::Root;

const WIDTH: f32 = 1080.;
const HEIGHT: f32 = 720.;

type App = Headless<Root>;

struct Editor {
    app: App,
    window: Entity<EditorWindow>,
    row: Entity<ToolRow>,
}

fn launch(mode: ThemeMode) -> Editor {
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
            es_ui::motion::set_reduce_motion(true, cx);
        },
        WIDTH,
        HEIGHT,
        move |window, cx| {
            let doc = opened(
                1200,
                640,
                vec![rect(80., 80., 300., 200.)],
                BackgroundSettings::default(),
            );
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    app.settle();
    let window = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    let row = window
        .read_with(&app.cx, |w, _| w.toolbar())
        .expect("a loaded session has a tool row");
    Editor { app, window, row }
}

impl Editor {
    fn send(&mut self, event: PlatformInput) {
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .expect("window is open");
        self.app.settle();
    }

    fn click_at(&mut self, at: gpui::Point<Pixels>) {
        for down in [true, false] {
            self.send(if down {
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                })
            } else {
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                })
            });
        }
    }

    fn bounds(&mut self, control: Control) -> Option<Bounds<Pixels>> {
        self.app.settle();
        self.row
            .read_with(&self.app.cx, |r, _| r.control_bounds(control))
    }

    fn click(&mut self, control: Control) {
        let b = self
            .bounds(control)
            .unwrap_or_else(|| panic!("{control:?} is not drawn"));
        self.click_at(b.center());
    }

    fn tool(&self) -> Tool {
        self.window.read_with(&self.app.cx, |w, cx| {
            w.session().unwrap().doc.read(cx).editor().tool()
        })
    }

    fn arm(&mut self, tool: Tool) {
        if self.tool() != tool {
            self.click(Control::Tool(tool));
        }
        assert_eq!(self.tool(), tool);
    }

    fn cropping(&self) -> bool {
        self.window
            .read_with(&self.app.cx, |w, cx| w.is_cropping(cx))
    }

    fn image_size(&self) -> (f64, f64) {
        self.window.read_with(&self.app.cx, |w, cx| {
            let size = w.session().unwrap().doc.read(cx).image_size();
            (size.width, size.height)
        })
    }

    fn crop_rect(&self) -> es_geom::Rect {
        self.window.read_with(&self.app.cx, |w, cx| {
            w.session().unwrap().crop.read(cx).state().unwrap().rect()
        })
    }

    fn with_crop(&mut self, f: impl FnOnce(&mut CropModel, &mut gpui::Context<CropModel>)) {
        let crop = self
            .window
            .read_with(&self.app.cx, |w, _| w.session().unwrap().crop.clone());
        self.app.cx.update(|cx| crop.update(cx, f));
        self.app.settle();
    }

    fn set_aspect(&mut self, aspect: AspectPreset) {
        self.with_crop(|c, cx| c.set_aspect(aspect, cx));
    }

    fn shot(&mut self, name: &str) {
        self.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(4.), px(HEIGHT - 4.)),
            pressed_button: None,
            modifiers: Default::default(),
        }));
        self.app.screenshot(name);
    }

    /// Checks the panel hangs below the row, inside the window, across the anchor's centre.
    fn assert_under(&mut self, anchor: Control) -> Bounds<Pixels> {
        let panel = self.bounds(Control::Options).expect("the options are open");
        let button = self.bounds(anchor).expect("the anchor is drawn");
        let row_bottom = TITLE_BAR_HEIGHT + TOOL_ROW_HEIGHT;
        assert!(
            panel.origin.y >= row_bottom,
            "{anchor:?}: the panel at {:?} covers the tool row",
            panel.origin.y
        );
        assert!(
            panel.origin.y - button.bottom() < px(24.),
            "{anchor:?}: the panel floats away from its button"
        );
        assert!(
            panel.origin.x >= px(0.) && panel.right() <= px(WIDTH),
            "{anchor:?}: the panel leaves the window"
        );
        let centre = button.center().x;
        assert!(
            panel.origin.x < centre && centre < panel.right(),
            "{anchor:?}: the panel is not under its button"
        );
        panel
    }
}

#[test]
fn the_options_open_under_the_armed_tool_and_follow_it_to_the_next() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Rectangle);
    let under_rectangle = e.assert_under(Control::Tool(Tool::Rectangle));
    e.arm(Tool::Blur);
    let under_blur = e.assert_under(Control::Tool(Tool::Blur));
    assert!(
        under_blur.center().x > under_rectangle.center().x + px(100.),
        "the panel moved with the tool"
    );
    e.arm(Tool::Select);
    assert!(
        e.bounds(Control::Options).is_none(),
        "Select with nothing selected has no options"
    );
}

#[test]
fn the_options_stay_open_across_clicks_on_the_canvas() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Arrow);
    let before = e.assert_under(Control::Tool(Tool::Arrow));
    let canvas = e
        .window
        .read_with(&e.app.cx, |w, cx| w.canvas().unwrap().read(cx).bounds());
    for at in [
        canvas.center(),
        point(canvas.right() - px(40.), canvas.bottom() - px(40.)),
    ] {
        e.click_at(at);
        assert_eq!(
            e.bounds(Control::Options),
            Some(before),
            "a canvas click closed or moved the options"
        );
    }
    assert_eq!(e.tool(), Tool::Arrow);
}

#[test]
fn crop_options_hang_under_crop_and_apply_cancel_and_reset_work() {
    let mut e = launch(ThemeMode::Dark);
    let whole = e.image_size();
    e.click(Control::Crop);
    assert!(e.cropping());
    e.assert_under(Control::Crop);

    e.with_crop(|c, cx| {
        c.press(CropTarget::Handle(CropHandle::TopLeft), Point::ZERO);
        c.drag_to(Point::new(240.0, 120.0), false, cx);
        c.release(cx);
    });
    assert!(e.crop_rect().x0 > 0.);
    e.click(Control::CropReset);
    assert_eq!(
        e.crop_rect().x0,
        0.,
        "Reset returns the rect to the whole image"
    );

    e.click(Control::CropCancel);
    assert!(!e.cropping(), "Cancel leaves crop mode");
    assert_eq!(e.image_size(), whole, "Cancel changes nothing");
    assert!(e.bounds(Control::CropApply).is_none());

    e.click(Control::Crop);
    e.set_aspect(AspectPreset::Square);
    e.click(Control::CropApply);
    assert!(!e.cropping(), "Apply leaves crop mode");
    let (w, h) = e.image_size();
    assert_eq!(w, h, "Apply cut the image to the square");
}

#[test]
fn the_title_bar_holds_no_crop_buttons() {
    let mut e = launch(ThemeMode::Dark);
    e.click(Control::Crop);
    e.set_aspect(AspectPreset::Square);
    let whole = e.image_size();
    let y = TITLE_BAR_HEIGHT / 2.;
    for x in [WIDTH - 40., WIDTH - 120., WIDTH - 200., WIDTH - 280.] {
        e.click_at(point(px(x), y));
        assert!(e.cropping(), "a title bar click at {x} ended the crop");
    }
    assert_eq!(e.image_size(), whole);
}

#[test]
fn screenshots_of_each_tool_family_in_both_themes() {
    let families = [
        (Tool::Rectangle, "shape"),
        (Tool::Arrow, "arrow"),
        (Tool::Text, "text"),
        (Tool::Blur, "blur"),
        (Tool::Highlighter, "highlighter"),
    ];
    for (mode, theme) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut e = launch(mode);
        for (tool, name) in families {
            e.arm(tool);
            e.assert_under(Control::Tool(tool));
            e.shot(&format!("tool-options-{theme}-{name}"));
        }
        e.arm(Tool::Select);
        e.click(Control::Crop);
        e.assert_under(Control::Crop);
        e.shot(&format!("tool-options-{theme}-crop"));
    }
}

impl Editor {
    fn edit(&mut self, f: impl FnOnce(&mut es_anno::Editor) -> es_anno::Changes) {
        let doc = self
            .window
            .read_with(&self.app.cx, |w, _| w.session().unwrap().doc.clone());
        self.app
            .cx
            .update(|cx| doc.update(cx, |d, cx| d.apply(cx, f)));
        self.app.settle();
    }

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
                    es_model::ShapeKind::Text(p) => Some((s.id, p.clone())),
                    _ => None,
                })
                .collect()
        })
    }

    fn text(&self) -> es_model::TextProps {
        self.texts().last().unwrap().1.clone()
    }

    fn pick_swatch(&mut self, strip: Control, index: usize) {
        let b = self
            .bounds(strip)
            .unwrap_or_else(|| panic!("{strip:?} is not drawn"));
        let column = b.size.width / 6.;
        let (row, col) = (index / 6, index % 6);
        self.click_at(point(
            b.origin.x + column * (col as f32 + 0.5),
            b.origin.y + b.size.height / 2. * (row as f32 + 0.5),
        ));
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

#[test]
fn screenshots_of_the_text_panel_in_both_themes() {
    for (mode, theme) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut e = launch(mode);
        e.arm(Tool::Text);
        e.pick_swatch(Control::TextFill, 3);
        e.pick_swatch(Control::TextBorder, 6);
        e.pick_width(2);
        e.shot(&format!("text-box-panel-{theme}"));
    }
}
