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
            es_ui::components::gallery::init(cx);
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
