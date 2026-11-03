#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::{TEXT, opened, rect};
use es_editor::toolbar::{Control, Rotation, ToolRow, ToolRowEvent, tools};
use es_editor::window::{CHROME_PADDING_X, TOOL_ROW_HEIGHT};
use es_editor::{EditorSession, EditorWindow};
use es_model::{RedactionKind, ShapeKind, Swatch, Tool};
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Bounds, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, point,
};
use gpui::{IntoElement, ParentElement, Styled, div};
use gpui_component::Root;
use std::cell::RefCell;
use std::rc::Rc;

const WIDTH: f32 = 1080.;
const HEIGHT: f32 = 1000.;

type App = Headless<Root>;
type Built = (Entity<ToolRow>, EditorSession);

struct Row {
    app: App,
    row: Entity<ToolRow>,
    session: EditorSession,
}

/// The window's 48 px tool-row slot, so the row is drawn as the editor draws it.
struct Slot(Entity<ToolRow>);

impl gpui::Render for Slot {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let t = es_ui::theme::tokens(cx);
        div().size_full().bg(t.window).child(
            div()
                .flex()
                .items_center()
                .h(TOOL_ROW_HEIGHT)
                .px(CHROME_PADDING_X)
                .border_b(t.metrics.hairline)
                .border_color(t.border)
                .child(self.0.clone()),
        )
    }
}

fn open(shapes: Vec<es_model::AnnoShape>, mode: ThemeMode) -> Row {
    let doc = opened(400, 300, shapes, Default::default());
    let slot: Rc<RefCell<Option<Built>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        es_ui::components::gallery::init,
        WIDTH,
        HEIGHT,
        move |window, cx| {
            es_ui::theme::set_mode(mode, cx);
            es_ui::motion::set_reduce_motion(true, cx);
            let session = EditorSession::new(doc, TEXT.clone(), cx);
            let row = cx.new(|cx| ToolRow::new(&session, cx));
            *out.borrow_mut() = Some((row.clone(), session));
            let slot = cx.new(|_| Slot(row));
            cx.new(|cx| Root::new(slot, window, cx))
        },
    );
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.activate_window())
        .expect("window is open");
    app.settle();
    let (row, session) = slot.borrow_mut().take().expect("the window built the row");
    Row { app, row, session }
}

fn send(app: &mut App, event: PlatformInput) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
        .expect("window is open");
    app.settle();
}

fn click(app: &mut App, at: gpui::Point<Pixels>) {
    send(
        app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
    );
    send(
        app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Default::default(),
            click_count: 1,
        }),
    );
}

impl Row {
    fn send(&mut self, event: PlatformInput) {
        send(&mut self.app, event);
    }

    fn click_at(&mut self, at: gpui::Point<Pixels>) {
        click(&mut self.app, at);
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

    /// With the pointer parked away from the row, so nothing shows its hover look.
    fn shot(&mut self, name: &str) {
        self.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(gpui::px(4.), gpui::px(HEIGHT - 4.)),
            pressed_button: None,
            modifiers: Default::default(),
        }));
        let row = self.row.clone();
        self.app.cx.update(|cx| row.update(cx, |_, cx| cx.notify()));
        self.app.screenshot(name);
    }

    fn shows(&mut self, control: Control) -> bool {
        self.bounds(control).is_some()
    }

    fn tool(&self) -> Tool {
        self.session
            .doc
            .read_with(&self.app.cx, |d, _| d.editor().tool())
    }

    fn arm(&mut self, tool: Tool) {
        if self.tool() != tool {
            self.click(Control::Tool(tool));
        }
    }

    fn select_all(&mut self) {
        self.app.cx.update(|cx| {
            self.session
                .doc
                .update(cx, |d, cx| d.apply(cx, |e| e.select_all()))
        });
        self.app.settle();
    }

    fn first_shape(&self) -> es_model::AnnoShape {
        self.session
            .doc
            .read_with(&self.app.cx, |d, _| d.editor().doc().shapes()[0].clone())
    }

    fn undo(&mut self) {
        self.app.cx.update(|cx| self.session.undo(cx));
        self.app.settle();
    }

    fn can_undo(&self) -> bool {
        self.session
            .doc
            .read_with(&self.app.cx, |d, _| d.editor().can_undo())
    }

    fn events(&mut self) -> Rc<RefCell<Vec<ToolRowEvent>>> {
        let log: Rc<RefCell<Vec<ToolRowEvent>>> = Rc::default();
        let sink = log.clone();
        let row = self.row.clone();
        self.app.cx.update(|cx| {
            cx.subscribe(&row, move |_, e: &ToolRowEvent, _| {
                sink.borrow_mut().push(*e)
            })
            .detach()
        });
        log
    }
}

/// The pixel two points inside a control's leading edge, where a selected chip shows its fill.
fn chip(shot: &image::RgbaImage, b: Bounds<Pixels>) -> [u8; 3] {
    let x = (f32::from(b.origin.x) + 2.) * 2.;
    let y = f32::from(b.center().y) * 2.;
    let p = shot.get_pixel(x as u32, y as u32).0;
    [p[0], p[1], p[2]]
}

fn is_accent(p: [u8; 3]) -> bool {
    p[2] > 200 && p[0] < 60 && (100..180).contains(&p[1])
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

#[test]
fn a_click_arms_the_tool_draws_it_selected_and_a_second_click_returns_to_select() {
    let mut r = open(vec![], ThemeMode::Dark);
    assert_eq!(r.tool(), Tool::Rectangle);
    r.click(Control::Tool(Tool::Arrow));
    assert_eq!(r.tool(), Tool::Arrow, "the engine took the tool");

    let shot = r.app.screenshot("toolbar-test-arrow-armed");
    let arrow = r.bounds(Control::Tool(Tool::Arrow)).unwrap();
    let rect = r.bounds(Control::Tool(Tool::Rectangle)).unwrap();
    assert!(
        is_accent(chip(&shot, arrow)),
        "arrow chip is {:?}",
        chip(&shot, arrow)
    );
    assert!(
        !is_accent(chip(&shot, rect)),
        "only the armed tool is filled"
    );

    r.click(Control::Tool(Tool::Arrow));
    assert_eq!(
        r.tool(),
        Tool::Select,
        "the armed tool again returns to Select"
    );
}

#[test]
fn the_options_offer_only_what_the_inspected_tool_takes() {
    let mut r = open(vec![rect(40., 40., 120., 80.)], ThemeMode::Dark);
    let shown = |r: &mut Row| {
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
        .map(|c| r.shows(c))
    };
    let (y, n) = (true, false);
    r.arm(Tool::Rectangle);
    assert_eq!(shown(&mut r), [y, y, n, n, n, n, n, n], "rectangle");
    r.arm(Tool::Arrow);
    assert_eq!(shown(&mut r), [y, y, n, y, y, n, n, n], "arrow");
    r.arm(Tool::Line);
    assert_eq!(
        shown(&mut r),
        [y, y, n, n, y, n, n, n],
        "a line has no heads"
    );
    r.arm(Tool::Text);
    assert_eq!(shown(&mut r), [y, n, y, n, n, n, n, n], "text");
    r.arm(Tool::Highlighter);
    assert_eq!(shown(&mut r), [y, y, n, n, n, y, n, n], "highlighter");
    r.arm(Tool::Blur);
    assert_eq!(shown(&mut r), [n, n, n, n, n, n, y, n], "blur");
    r.arm(Tool::Highlight);
    assert!(!r.shows(Control::Options), "the spotlight has no options");
    r.arm(Tool::Select);
    assert!(!r.shows(Control::Options), "select with nothing selected");
    r.select_all();
    assert_eq!(
        shown(&mut r),
        [y, y, n, n, n, n, n, y],
        "select with a rectangle selected inspects the rectangle"
    );
    assert!(r.shows(Control::Tool(Tool::Blur)) && r.shows(Control::Inspector));
}

#[test]
fn the_highlighter_mode_switches_from_its_options() {
    let mut r = open(vec![], ThemeMode::Dark);
    r.arm(Tool::Highlighter);
    let straight = |r: &Row| {
        r.session
            .doc
            .read_with(&r.app.cx, |d, _| d.editor().highlighter_straight())
    };
    assert!(!straight(&r));
    let modes = r.bounds(Control::HighlighterMode).unwrap();
    r.click_at(point(
        modes.origin.x + modes.size.width * 0.75,
        modes.center().y,
    ));
    assert!(straight(&r), "the second segment draws straight lines");
    assert_eq!(r.tool(), Tool::Highlighter, "and keeps the tool armed");
    r.click_at(point(
        modes.origin.x + modes.size.width * 0.25,
        modes.center().y,
    ));
    assert!(!straight(&r));
}

#[test]
fn a_swatch_recolours_new_shapes_and_the_selection_in_one_undo_step() {
    let mut r = open(vec![rect(40., 40., 120., 80.)], ThemeMode::Dark);
    r.arm(Tool::Select);
    r.select_all();
    let before = swatch_of(&r.first_shape());
    let strip = r.bounds(Control::Swatches).expect("the swatches show");
    let column = (f32::from(strip.size.width) - 5. * 8.) / 6.;
    let orange = point(
        strip.origin.x + gpui::px(2. * (column + 8.) + column / 2.),
        strip.origin.y + gpui::px(14.5),
    );
    r.click_at(orange);

    let style = r.session.doc.read_with(&r.app.cx, |d, _| {
        d.editor().current_style().swatch.id.clone()
    });
    assert_eq!(style, "orange", "new shapes take the pick");
    assert_eq!(
        swatch_of(&r.first_shape()).id,
        "orange",
        "the selection took it"
    );
    r.undo();
    assert_eq!(swatch_of(&r.first_shape()), before);
    assert!(!r.can_undo(), "the pick was a single undo step");
}

#[test]
fn a_stroke_dot_restyles_the_selection_in_one_undo_step() {
    let mut r = open(vec![rect(40., 40., 120., 80.)], ThemeMode::Dark);
    r.arm(Tool::Select);
    r.select_all();
    let before = stroke_of(&r.first_shape());
    let dots = r.bounds(Control::Stroke).unwrap();
    let widest = point(dots.origin.x + dots.size.width * 0.9, dots.center().y);
    r.click_at(widest);
    let (slider, page) = r.session.doc.read_with(&r.app.cx, |d, _| {
        let e = d.editor();
        (
            e.current_style().stroke_width,
            e.viewport().page_stroke_width(12.),
        )
    });
    assert_eq!(slider, 12.);
    assert_eq!(stroke_of(&r.first_shape()), page);
    assert_ne!(page, before);
    r.undo();
    assert_eq!(stroke_of(&r.first_shape()), before);
    assert!(!r.can_undo());
}

#[test]
fn crop_rotate_redaction_and_inspector_only_ask_the_window() {
    let mut r = open(vec![rect(40., 40., 120., 80.)], ThemeMode::Dark);
    let log = r.events();
    let shapes = r.first_shape();
    r.click(Control::Crop);
    r.click(Control::Rotate);
    r.click(Control::SmartRedact);
    r.click(Control::RedactBlur);
    assert!(!r.shows(Control::RedactBlur), "a choice closes the popover");
    r.click(Control::Inspector);
    assert_eq!(
        *log.borrow(),
        vec![
            ToolRowEvent::Crop,
            ToolRowEvent::Rotate(Rotation::Left),
            ToolRowEvent::SmartRedact(RedactionKind::Blur),
            ToolRowEvent::ToggleInspector,
        ]
    );
    assert_eq!(r.first_shape(), shapes);
    assert!(!r.can_undo(), "none of them touched the document");
}

#[test]
fn tool_tooltips_name_the_tool_and_its_key() {
    assert_eq!(tools::tooltip(Tool::Arrow), "Arrow (A)");
    assert_eq!(tools::tooltip(Tool::Select), "Select (H)");
    assert_eq!(tools::tooltip(Tool::NumberedCircle), "Numbered circle (1)");
    assert_eq!(
        tools::tooltip(Tool::Freehand),
        "Freehand",
        "no key, no suffix"
    );
    assert!(tools::tooltip(Tool::Highlight).starts_with("Draw an area"));
}

#[test]
fn screenshots_of_each_state_in_both_themes() {
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut r = open(vec![], mode);
        r.arm(Tool::Select);
        r.shot(&format!("toolbar-{name}-select"));
        drop(r);

        let mut r = open(vec![], mode);
        r.arm(Tool::Arrow);
        r.shot(&format!("toolbar-{name}-arrow"));
        drop(r);

        let mut r = open(vec![], mode);
        r.arm(Tool::Text);
        assert!(r.shows(Control::FontFamily));
        r.shot(&format!("toolbar-{name}-text-style"));
        drop(r);

        let mut r = open(vec![], mode);
        r.arm(Tool::Blur);
        assert!(r.shows(Control::Strength));
        r.shot(&format!("toolbar-{name}-blur"));
    }
}

#[test]
fn the_toolbar_inspector_toggle_closes_and_opens_the_window_column() {
    let doc = opened(400, 300, vec![], Default::default());
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        es_ui::components::gallery::init,
        WIDTH,
        720.,
        move |window, cx| {
            let session = EditorSession::new(doc, TEXT.clone(), cx);
            let editor = cx.new(|cx| EditorWindow::new(session, cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    let row = editor
        .read_with(&app.cx, |w, _| w.toolbar())
        .expect("a loaded session gets a tool row");
    let open = |app: &App| editor.read_with(&app.cx, |w, _| w.inspector_open());
    let toggle = |app: &mut App| {
        app.settle();
        let at = row
            .read_with(&app.cx, |r, _| r.control_bounds(Control::Inspector))
            .expect("the toggle is drawn")
            .center();
        click(app, at);
    };
    assert!(open(&app));
    toggle(&mut app);
    assert!(!open(&app), "the toggle closes the column");
    toggle(&mut app);
    assert!(open(&app), "and opens it again");
}
