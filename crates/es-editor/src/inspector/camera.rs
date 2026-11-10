use std::{cell::RefCell, collections::HashMap, hash::Hash, ops::RangeInclusive, rc::Rc};

use es_model::BackgroundSettings;
use es_ui::components::Tip;
use es_ui::{
    components::{InspectorSlider, ValueFormat},
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, SharedString, Styled, Subscription, Window, canvas, div,
};

use crate::session::look::LookModel;
use crate::strings::fl;

/// One beautify slider: what it shows and which setting it drives.
pub(super) struct Field {
    pub label: String,
    pub range: RangeInclusive<f64>,
    pub format: ValueFormat,
    pub set: fn(&mut BackgroundSettings, f64),
}

pub(super) type Drawn<K> = Rc<RefCell<HashMap<K, Bounds<Pixels>>>>;

/// The sliders of one section, built on first draw because a slider needs a window, and kept in step
/// with the look on every draw after.
pub(super) struct Sliders<K> {
    look: Entity<LookModel>,
    made: HashMap<K, (Entity<InspectorSlider>, bool)>,
    drawn: Drawn<K>,
    _bindings: Vec<Subscription>,
}

impl<K: Copy + Eq + Hash + 'static> Sliders<K> {
    pub fn new(look: Entity<LookModel>) -> Self {
        Self {
            look,
            made: HashMap::new(),
            drawn: Rc::default(),
            _bindings: Vec::new(),
        }
    }

    pub fn get(&self, key: K) -> Option<Entity<InspectorSlider>> {
        self.made.get(&key).map(|(slider, _)| slider.clone())
    }

    /// Where a control was last drawn, in window coordinates; `None` when it is hidden.
    pub fn bounds(&self, key: K) -> Option<Bounds<Pixels>> {
        self.drawn.borrow().get(&key).copied()
    }

    /// Forgets the last frame, so a control that is no longer drawn has no bounds.
    pub fn begin_frame(&self) {
        self.drawn.borrow_mut().clear();
    }

    pub fn recorded(&self, key: K, element: impl IntoElement) -> AnyElement {
        let drawn = self.drawn.clone();
        div()
            .relative()
            .w_full()
            .child(element)
            .child(
                canvas(
                    move |bounds, _, _| {
                        drawn.borrow_mut().insert(key, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .into_any_element()
    }

    pub fn row(
        &mut self,
        key: K,
        field: impl FnOnce() -> Field,
        value: f64,
        disabled: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let (slider, was_disabled) = match self.made.get(&key) {
            Some(made) => made.clone(),
            None => {
                let field = field();
                let slider = cx.new(|cx| {
                    InspectorSlider::new(field.label, value, field.range, field.format, window, cx)
                });
                let set = field.set;
                self._bindings
                    .push(LookModel::bind_slider(&self.look, &slider, set, cx));
                (slider, false)
            }
        };
        self.made.insert(key, (slider.clone(), disabled));
        slider.update(cx, |s, cx| {
            if s.value() != value {
                s.set_value(value, window, cx);
            }
            if was_disabled != disabled {
                s.set_disabled(disabled, window, cx);
            }
        });
        self.recorded(key, slider)
    }
}

/// A caption over a run of controls, with the caption's help as its tooltip.
pub(super) fn group(
    t: &Tokens,
    id: &'static str,
    title: String,
    help: Option<SharedString>,
    rows: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    let m = t.metrics;
    let mut caption = div()
        .id(id)
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(title);
    if let Some(help) = help {
        caption = caption.tip(help);
    }
    div()
        .flex()
        .flex_col()
        .child(div().flex().pb(m.group_label_spacing).child(caption))
        .child(div().flex().flex_col().gap(m.row_spacing).children(rows))
        .into_any_element()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CameraControl {
    TiltX,
    TiltY,
    Roll,
    Fov,
    Zoom,
    PanX,
    PanY,
    RotateX,
    RotateY,
}

const ANGLE: RangeInclusive<f64> = -45.0..=45.0;
const ROTATION: RangeInclusive<f64> = -60.0..=60.0;
const PAN: RangeInclusive<f64> = -0.5..=0.5;

impl CameraControl {
    pub const ALL: [Self; 9] = [
        Self::TiltX,
        Self::TiltY,
        Self::Roll,
        Self::Fov,
        Self::Zoom,
        Self::PanX,
        Self::PanY,
        Self::RotateX,
        Self::RotateY,
    ];

    pub fn range(self) -> RangeInclusive<f64> {
        match self {
            Self::TiltX | Self::TiltY | Self::Roll => ANGLE,
            Self::Fov => 18.0..=80.0,
            Self::Zoom => 0.4..=2.5,
            Self::PanX | Self::PanY => PAN,
            Self::RotateX | Self::RotateY => ROTATION,
        }
    }

    pub fn get(self, s: &BackgroundSettings) -> f64 {
        let c = &s.camera;
        match self {
            Self::TiltX => c.tilt_x_degrees,
            Self::TiltY => c.tilt_y_degrees,
            Self::Roll => c.roll_degrees,
            Self::Fov => c.field_of_view_degrees,
            Self::Zoom => c.zoom,
            Self::PanX => c.pan_x,
            Self::PanY => c.pan_y,
            Self::RotateX => c.rotation_x_degrees,
            Self::RotateY => c.rotation_y_degrees,
        }
    }

    fn field(self) -> Field {
        let (label, format, set): (String, ValueFormat, fn(&mut BackgroundSettings, f64)) =
            match self {
                Self::TiltX => (
                    fl!("editor-camera-tilt-x"),
                    ValueFormat::degrees().signed(),
                    |s, v| s.camera.tilt_x_degrees = v,
                ),
                Self::TiltY => (
                    fl!("editor-camera-tilt-y"),
                    ValueFormat::degrees().signed(),
                    |s, v| s.camera.tilt_y_degrees = v,
                ),
                Self::Roll => (
                    fl!("editor-camera-roll"),
                    ValueFormat::degrees().signed(),
                    |s, v| s.camera.roll_degrees = v,
                ),
                Self::Fov => (fl!("editor-camera-fov"), ValueFormat::degrees(), |s, v| {
                    s.camera.field_of_view_degrees = v
                }),
                Self::Zoom => (
                    fl!("editor-camera-zoom"),
                    ValueFormat::magnification(2),
                    |s, v| s.camera.zoom = v,
                ),
                Self::PanX => (
                    fl!("editor-camera-pan-x"),
                    ValueFormat::percent().signed(),
                    |s, v| s.camera.pan_x = v,
                ),
                Self::PanY => (
                    fl!("editor-camera-pan-y"),
                    ValueFormat::percent().signed(),
                    |s, v| s.camera.pan_y = v,
                ),
                Self::RotateX => (
                    fl!("editor-camera-rotate-x"),
                    ValueFormat::degrees().signed(),
                    |s, v| s.camera.rotation_x_degrees = v,
                ),
                Self::RotateY => (
                    fl!("editor-camera-rotate-y"),
                    ValueFormat::degrees().signed(),
                    |s, v| s.camera.rotation_y_degrees = v,
                ),
            };
        Field {
            label,
            range: self.range(),
            format,
            set,
        }
    }
}

/// The Camera section: orbit and roll, lens and framing, then the card's own rotation.
pub struct CameraBody {
    look: Entity<LookModel>,
    sliders: Sliders<CameraControl>,
    _look_changes: Subscription,
}

impl CameraBody {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        Self {
            _look_changes: cx.observe(&look, |_, _, cx| cx.notify()),
            sliders: Sliders::new(look.clone()),
            look,
        }
    }

    pub fn slider(&self, control: CameraControl) -> Option<Entity<InspectorSlider>> {
        self.sliders.get(control)
    }

    pub fn control_bounds(&self, control: CameraControl) -> Option<Bounds<Pixels>> {
        self.sliders.bounds(control)
    }
}

impl Render for CameraBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use CameraControl::*;
        let t = tokens(cx).clone();
        let settings = self.look.read(cx).settings().clone();
        self.sliders.begin_frame();
        let groups = [
            (
                "camera-angle",
                fl!("editor-camera-angle"),
                fl!("editor-camera-angle-help"),
                &[TiltX, TiltY, Roll][..],
            ),
            (
                "camera-framing",
                fl!("editor-camera-framing"),
                fl!("editor-camera-framing-help"),
                &[Fov, Zoom, PanX, PanY][..],
            ),
            (
                "camera-card-rotation",
                fl!("editor-camera-card-rotation"),
                fl!("editor-camera-card-rotation-help"),
                &[RotateX, RotateY][..],
            ),
        ]
        .map(|(id, title, help, controls)| {
            let rows: Vec<AnyElement> = controls
                .iter()
                .map(|&c| {
                    self.sliders
                        .row(c, || c.field(), c.get(&settings), false, window, cx)
                })
                .collect();
            group(&t, id, title, Some(help.into()), rows)
        });
        div()
            .flex()
            .flex_col()
            .gap(t.metrics.section_padding)
            .children(groups)
    }
}
