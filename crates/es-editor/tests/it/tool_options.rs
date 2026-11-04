#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{TEXT, opened, rect};

use std::{cell::RefCell, rc::Rc};

use es_anno::crop::{AspectPreset, CropHandle};
use es_editor::inspector::options::{Control, ToolOptions};
use es_editor::session::crop::{CropModel, CropTarget};
use es_editor::toolbar::{Control as Row, ToolRow};
use es_editor::window::{INSPECTOR_WIDTH, TITLE_BAR_HEIGHT};
use es_editor::{EditorSession, EditorWindow};
use es_geom::Point;
use es_model::{BackgroundSettings, ShapeKind, Swatch, Tool};
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Bounds, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, point, px,
};
use gpui_component::Root;

#[path = "tool_options_text.rs"]
mod text;

const WIDTH: f32 = 1080.;
const HEIGHT: f32 = 860.;

type App = Headless<Root>;

struct Editor {
    app: App,
    window: Entity<EditorWindow>,
    row: Entity<ToolRow>,
    options: Entity<ToolOptions>,
}

fn launch(mode: ThemeMode) -> Editor {
    launch_with_text(mode, TEXT.clone())
}

fn launch_with_text(mode: ThemeMode, text: std::sync::Arc<es_anno::BundledText>) -> Editor {
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
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, text.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    app.settle();
    let window = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    let (row, options) = window.read_with(&app.cx, |w, _| {
        (
            w.toolbar().expect("a loaded session has a tool row"),
            w.tool_options().expect("and a tool options page"),
        )
    });
    Editor {
        app,
        window,
        row,
        options,
    }
}

fn stroke_of(shape: &es_model::AnnoShape) -> f64 {
    match &shape.kind {
        ShapeKind::Geo(g) => g.stroke_width,
        _ => panic!("not a rectangle"),
    }
}

fn swatch_of(shape: &es_model::AnnoShape) -> Swatch {
    match &shape.kind {
        ShapeKind::Geo(g) => g.swatch.clone(),
        _ => panic!("not a rectangle"),
    }
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
        self.options
            .read_with(&self.app.cx, |o, _| o.control_bounds(control))
    }

    fn shows(&mut self, control: Control) -> bool {
        self.bounds(control).is_some()
    }

    fn click(&mut self, control: Control) {
        let b = self
            .bounds(control)
            .unwrap_or_else(|| panic!("{control:?} is not drawn"));
        self.click_at(b.center());
    }

    fn click_row(&mut self, control: Row) {
        self.app.settle();
        let b = self
            .row
            .read_with(&self.app.cx, |r, _| r.control_bounds(control))
            .unwrap_or_else(|| panic!("{control:?} is not drawn"));
        self.click_at(b.center());
    }

    fn inspector_open(&self) -> bool {
        self.window
            .read_with(&self.app.cx, |w, _| w.inspector_open())
    }

    /// Whether the page is up; it is drawn exactly when it is up and the sidebar is open.
    fn page_shown(&mut self) -> bool {
        self.app.settle();
        let up = self
            .options
            .read_with(&self.app.cx, |o, cx| o.shows_page(cx));
        let drawn = self.shows(Control::Page);
        if self.inspector_open() {
            assert_eq!(drawn, up, "the page is drawn exactly when it is up");
        }
        up
    }

    fn assert_in_sidebar(&mut self) -> Bounds<Pixels> {
        let page = self.bounds(Control::Page).expect("the options page is up");
        assert!(
            page.origin.x >= px(WIDTH) - INSPECTOR_WIDTH,
            "the page at {:?} is not in the sidebar",
            page.origin.x
        );
        assert!(page.origin.y > TITLE_BAR_HEIGHT);
        page
    }

    fn tool(&self) -> Tool {
        self.window.read_with(&self.app.cx, |w, cx| {
            w.session().unwrap().doc.read(cx).editor().tool()
        })
    }

    fn arm(&mut self, tool: Tool) {
        if self.tool() != tool {
            self.click_row(Row::Tool(tool));
        }
        assert_eq!(self.tool(), tool);
    }

    fn first_shape(&self) -> es_model::AnnoShape {
        self.window.read_with(&self.app.cx, |w, cx| {
            w.session().unwrap().doc.read(cx).editor().doc().shapes()[0].clone()
        })
    }

    fn can_undo(&self) -> bool {
        self.window.read_with(&self.app.cx, |w, cx| {
            w.session().unwrap().doc.read(cx).editor().can_undo()
        })
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

    fn edit(&mut self, f: impl FnOnce(&mut es_anno::Editor) -> es_anno::Changes) {
        let doc = self
            .window
            .read_with(&self.app.cx, |w, _| w.session().unwrap().doc.clone());
        self.app
            .cx
            .update(|cx| doc.update(cx, |d, cx| d.apply(cx, f)));
        self.app.settle();
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
}

#[test]
fn the_editor_opens_on_the_usual_sidebar_with_a_tool_armed() {
    let mut e = launch(ThemeMode::Dark);
    assert_eq!(e.tool(), Tool::Rectangle);
    assert!(
        !e.page_shown(),
        "opening the editor is for the image settings"
    );
}

#[test]
fn clicking_the_active_tool_keeps_it_and_brings_its_page_back() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Arrow);
    e.click(Control::Back);
    assert!(!e.page_shown());
    e.click_row(Row::Tool(Tool::Arrow));
    assert_eq!(e.tool(), Tool::Arrow, "a second click keeps the tool");
    assert!(e.page_shown(), "and shows its options again");
    e.assert_in_sidebar();
}

#[test]
fn activating_a_tool_shows_its_page_and_opens_a_hidden_sidebar() {
    let mut e = launch(ThemeMode::Dark);
    e.click_row(Row::Inspector);
    assert!(!e.inspector_open());
    e.arm(Tool::Arrow);
    assert!(e.inspector_open(), "a click on a tool opens the sidebar");
    e.assert_in_sidebar();
    assert!(e.shows(Control::ArrowStart));

    e.click_row(Row::Inspector);
    e.edit(|e| e.select_tool(Tool::Text));
    assert!(e.inspector_open(), "so does a tool's key");
    assert!(e.shows(Control::FontFamily));
}

#[test]
fn a_new_selection_waits_in_a_hidden_sidebar() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Select);
    e.click_row(Row::Inspector);
    e.edit(|e| e.select_all());
    assert!(!e.inspector_open(), "selecting does not open the sidebar");
    e.click_row(Row::Inspector);
    assert!(e.page_shown());
    assert!(e.shows(Control::Delete));
}

#[test]
fn back_returns_to_the_usual_sidebar_until_the_next_tool() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Blur);
    e.assert_in_sidebar();
    e.click(Control::Back);
    assert!(!e.page_shown(), "Back shows the usual sidebar");
    assert!(e.inspector_open(), "and keeps the sidebar open");
    assert_eq!(e.tool(), Tool::Blur, "the tool stays armed");
    e.arm(Tool::Line);
    assert!(e.page_shown(), "the next tool brings its page");
}

#[test]
fn hand_and_select_show_the_usual_sidebar() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Arrow);
    assert!(e.page_shown());
    e.arm(Tool::Select);
    assert!(
        !e.page_shown(),
        "Select with nothing selected has no options"
    );
    e.arm(Tool::Highlight);
    assert!(!e.page_shown(), "the spotlight has no options");
}

#[test]
fn the_page_stays_across_clicks_on_the_canvas() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Arrow);
    let before = e.assert_in_sidebar();
    let canvas = e
        .window
        .read_with(&e.app.cx, |w, cx| w.canvas().unwrap().read(cx).bounds());
    for at in [
        canvas.center(),
        point(canvas.right() - px(40.), canvas.bottom() - px(40.)),
    ] {
        e.click_at(at);
        assert_eq!(e.bounds(Control::Page), Some(before));
    }
    assert_eq!(e.tool(), Tool::Arrow);
}

#[test]
fn the_options_offer_only_what_the_inspected_tool_takes() {
    let mut e = launch(ThemeMode::Dark);
    let shown = |e: &mut Editor| {
        [
            Control::Swatches,
            Control::Stroke,
            Control::FontFamily,
            Control::ArrowStart,
            Control::Dash,
            Control::HighlighterMode,
            Control::Strength,
            Control::Delete,
        ]
        .map(|c| e.shows(c))
    };
    let (y, n) = (true, false);
    e.arm(Tool::Ellipse);
    assert_eq!(shown(&mut e), [y, y, n, n, n, n, n, n], "shape");
    e.arm(Tool::Arrow);
    assert_eq!(shown(&mut e), [y, y, n, y, y, n, n, n], "arrow");
    e.arm(Tool::Line);
    assert_eq!(shown(&mut e), [y, y, n, n, y, n, n, n], "line");
    e.arm(Tool::Text);
    assert_eq!(shown(&mut e), [y, n, y, n, n, n, n, n], "text");
    e.arm(Tool::Highlighter);
    assert_eq!(shown(&mut e), [y, y, n, n, n, y, n, n], "highlighter");
    e.arm(Tool::Blur);
    assert_eq!(shown(&mut e), [n, n, n, n, n, n, y, n], "blur");
    e.arm(Tool::Select);
    e.edit(|e| e.select_all());
    assert_eq!(
        shown(&mut e),
        [y, y, n, n, n, n, n, y],
        "select with a rectangle selected inspects the rectangle"
    );
}

#[test]
fn the_highlighter_mode_switches_from_its_options() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Highlighter);
    let straight = |e: &Editor| {
        e.window.read_with(&e.app.cx, |w, cx| {
            w.session()
                .unwrap()
                .doc
                .read(cx)
                .editor()
                .highlighter_straight()
        })
    };
    assert!(!straight(&e));
    let modes = e.bounds(Control::HighlighterMode).unwrap();
    e.click_at(point(
        modes.origin.x + modes.size.width * 0.75,
        modes.center().y,
    ));
    assert!(straight(&e), "the second segment draws straight lines");
    assert_eq!(e.tool(), Tool::Highlighter, "and keeps the tool armed");
    e.click_at(point(
        modes.origin.x + modes.size.width * 0.25,
        modes.center().y,
    ));
    assert!(!straight(&e));
}

#[test]
fn a_swatch_recolours_new_shapes_and_the_selection_in_one_undo_step() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Select);
    e.edit(|e| e.select_all());
    let before = swatch_of(&e.first_shape());
    e.pick_swatch(Control::Swatches, 2);
    let style = e.window.read_with(&e.app.cx, |w, cx| {
        let doc = w.session().unwrap().doc.read(cx);
        doc.editor().current_style().swatch.id.clone()
    });
    assert_eq!(style, "orange", "new shapes take the pick");
    assert_eq!(swatch_of(&e.first_shape()).id, "orange");
    e.edit(|e| e.undo());
    assert_eq!(swatch_of(&e.first_shape()), before);
    assert!(!e.can_undo(), "the pick was a single undo step");
}

#[test]
fn a_stroke_dot_restyles_the_selection_in_one_undo_step() {
    let mut e = launch(ThemeMode::Dark);
    e.arm(Tool::Select);
    e.edit(|e| e.select_all());
    let before = stroke_of(&e.first_shape());
    let dots = e.bounds(Control::Stroke).unwrap();
    e.click_at(point(
        dots.origin.x + dots.size.width * 0.9,
        dots.center().y,
    ));
    let (slider, page) = e.window.read_with(&e.app.cx, |w, cx| {
        let doc = w.session().unwrap().doc.read(cx);
        let editor = doc.editor();
        (
            editor.current_style().stroke_width,
            editor.viewport().page_stroke_width(12.),
        )
    });
    assert_eq!(slider, 12.);
    assert_eq!(stroke_of(&e.first_shape()), page);
    assert_ne!(page, before);
    e.edit(|e| e.undo());
    assert_eq!(stroke_of(&e.first_shape()), before);
    assert!(!e.can_undo());
}

#[test]
fn crop_options_fill_the_sidebar_and_apply_cancel_and_reset_work() {
    let mut e = launch(ThemeMode::Dark);
    let whole = e.image_size();
    e.click_row(Row::Inspector);
    e.click_row(Row::Crop);
    assert!(e.cropping());
    assert!(e.inspector_open(), "crop opens the sidebar for its buttons");
    e.assert_in_sidebar();
    assert!(!e.shows(Control::Back), "crop is left with Cancel or Apply");

    e.with_crop(|c, cx| {
        c.press(CropTarget::Handle(CropHandle::TopLeft), Point::ZERO);
        c.drag_to(Point::new(240.0, 120.0), false, cx);
        c.release(cx);
    });
    assert!(e.crop_rect().x0 > 0.);
    e.click(Control::CropReset);
    assert_eq!(e.crop_rect().x0, 0., "Reset returns to the whole image");

    e.click(Control::CropCancel);
    assert!(!e.cropping(), "Cancel leaves crop mode");
    assert_eq!(e.image_size(), whole, "Cancel changes nothing");
    assert!(e.bounds(Control::CropApply).is_none());

    e.click_row(Row::Crop);
    e.set_aspect(AspectPreset::Square);
    e.click(Control::CropApply);
    assert!(!e.cropping(), "Apply leaves crop mode");
    let (w, h) = e.image_size();
    assert_eq!(w, h, "Apply cut the image to the square");
}

#[test]
fn the_title_bar_holds_no_crop_buttons() {
    let mut e = launch(ThemeMode::Dark);
    e.click_row(Row::Crop);
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
fn screenshots_of_each_page_in_both_themes() {
    let families = [
        (Tool::Rectangle, "shape"),
        (Tool::Arrow, "arrow"),
        (Tool::Line, "line"),
        (Tool::Freehand, "pen"),
        (Tool::Highlighter, "highlighter"),
        (Tool::Text, "text"),
        (Tool::NumberedCircle, "counter"),
        (Tool::Blur, "blur"),
        (Tool::Pixelate, "pixelate"),
    ];
    for (mode, theme) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut e = launch(mode);
        e.shot(&format!("sidebar-{theme}-usual"));
        for (tool, name) in families {
            e.arm(tool);
            e.click_row(Row::Tool(tool));
            e.assert_in_sidebar();
            e.shot(&format!("sidebar-{theme}-{name}"));
        }
        e.arm(Tool::Select);
        e.edit(|e| e.select_all());
        e.assert_in_sidebar();
        e.shot(&format!("sidebar-{theme}-selection"));
        e.edit(|e| e.select([]));
        e.click_row(Row::Crop);
        e.assert_in_sidebar();
        e.shot(&format!("sidebar-{theme}-crop"));
        e.click(Control::CropCancel);
        e.click_row(Row::Inspector);
        e.shot(&format!("sidebar-{theme}-hidden"));
        e.arm(Tool::Arrow);
        e.assert_in_sidebar();
        e.shot(&format!("sidebar-{theme}-hidden-then-arrow"));
    }
}
