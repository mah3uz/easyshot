use es_model::{BlurEdgeMode, BlurMode, ProgressiveBlurSettings};
use es_ui::{
    components::{FocusPad, InspectorSlider, Segment, Segmented, ValueFormat},
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, App, Bounds, ContentMask, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, PathBuilder, Pixels, Point, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, canvas, div, fill, point,
    prelude::FluentBuilder, px, size,
};
use gpui_component::tooltip::Tooltip;

use super::camera::{Field, Sliders};
use crate::session::look::LookModel;
use crate::strings::fl;

const DIMMED: f32 = 0.48;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlurControl {
    ApplyTo,
    Mode,
    Strength,
    Falloff,
    FocusSize,
    Direction,
    FocusPad,
}

impl BlurControl {
    pub const SLIDERS: [Self; 4] = [
        Self::Strength,
        Self::Falloff,
        Self::FocusSize,
        Self::Direction,
    ];

    pub fn range(self) -> std::ops::RangeInclusive<f64> {
        match self {
            Self::Strength => 0.0..=40.0,
            Self::Direction => 0.0..=180.0,
            _ => 0.0..=1.0,
        }
    }

    pub fn get(self, b: &ProgressiveBlurSettings) -> f64 {
        match self {
            Self::Strength => b.strength,
            Self::Falloff => b.falloff,
            Self::FocusSize => b.focus_size,
            Self::Direction => b.direction_degrees,
            _ => 0.0,
        }
    }

    fn field(self) -> Field {
        let (label, format, set): (String, ValueFormat, fn(&mut _, f64)) = match self {
            Self::Strength => (
                fl!("editor-blur-strength"),
                ValueFormat::integer(),
                |s: &mut es_model::BackgroundSettings, v| s.progressive_blur.strength = v,
            ),
            Self::Falloff => (
                fl!("editor-blur-falloff"),
                ValueFormat::decimal(2),
                |s, v| s.progressive_blur.falloff = v,
            ),
            Self::FocusSize => (
                fl!("editor-blur-focus-size"),
                ValueFormat::percent(),
                |s, v| s.progressive_blur.focus_size = v,
            ),
            _ => (
                fl!("editor-blur-direction"),
                ValueFormat::degrees(),
                |s, v| s.progressive_blur.direction_degrees = v,
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

/// The help under Focus Size names what the sharp area is measured against.
fn focus_size_help(edge_mode: BlurEdgeMode) -> SharedString {
    match edge_mode {
        BlurEdgeMode::Clipped => fl!("editor-blur-focus-size-clipped-help"),
        BlurEdgeMode::Bleed => fl!("editor-blur-focus-size-bleed-help"),
    }
    .into()
}

/// The Progressive Blur section: what is blurred, the shape of the sharp area and where it sits.
/// Everything stays visible but inert while the section's switch is off.
pub struct BlurBody {
    look: Entity<LookModel>,
    sliders: Sliders<BlurControl>,
    pad_dragging: bool,
    _look_changes: Subscription,
}

impl BlurBody {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        Self {
            _look_changes: cx.observe(&look, |_, _, cx| cx.notify()),
            sliders: Sliders::new(look.clone()),
            pad_dragging: false,
            look,
        }
    }

    pub fn slider(&self, control: BlurControl) -> Option<Entity<InspectorSlider>> {
        self.sliders.get(control)
    }

    pub fn control_bounds(&self, control: BlurControl) -> Option<Bounds<Pixels>> {
        self.sliders.bounds(control)
    }

    pub fn focus_size_help(&self, cx: &App) -> SharedString {
        focus_size_help(self.look.read(cx).settings().progressive_blur.edge_mode)
    }

    fn edit(&self, cx: &mut App, f: impl FnOnce(&mut ProgressiveBlurSettings)) {
        self.look.update(cx, |look, cx| {
            look.update(|s| f(&mut s.progressive_blur), cx)
        });
    }

    fn live_adjust(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.pad_dragging == on {
            return;
        }
        self.pad_dragging = on;
        self.look.update(cx, |look, cx| {
            if on {
                look.begin_live_adjust(cx)
            } else {
                look.end_live_adjust(cx)
            }
        });
    }

    fn segmented(
        &self,
        control: BlurControl,
        segments: [(String, Option<SharedString>); 2],
        selected: usize,
        cx: &mut Context<Self>,
        pick: fn(&mut ProgressiveBlurSettings, usize),
    ) -> AnyElement {
        let t = tokens(cx).clone();
        let on = self.look.read(cx).settings().progressive_blur.is_enabled;
        let (id, title) = match control {
            BlurControl::ApplyTo => ("blur-apply-to", fl!("editor-blur-apply-to")),
            _ => ("blur-mode", fl!("editor-blur-mode")),
        };
        let help = segments[selected].1.clone();
        let this = cx.entity().downgrade();
        let control_el = Segmented::new(
            id,
            title.clone(),
            segments.map(|(label, _)| Segment::text(label)),
        )
        .selected(selected)
        .disabled(!on)
        .on_select(move |index, _, cx| {
            let _ = this.update(cx, |body, cx| body.edit(cx, |b| pick(b, index)));
        });
        let control_el = div()
            .id(SharedString::from(format!("{id}-help")))
            .w_full()
            .child(control_el)
            .when_some(help, |d, help| {
                d.tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
            });
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(t.metrics.group_label_spacing)
                    .typeset(t.type_scale.label)
                    .text_color(t.text_secondary)
                    .when(!on, |d| d.opacity(DIMMED))
                    .child(title),
            )
            .child(self.sliders.recorded(control, control_el))
            .into_any_element()
    }
}

/// The sharp area as the canvas draws it, scaled onto the pad as if the pad were the image.
fn paint_region(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    b: &ProgressiveBlurSettings,
    t: &Tokens,
) {
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let shortest = w.min(h);
    let focus = point(b.focus_x as f32 * w, b.focus_y as f32 * h);
    let corners = [point(0., 0.), point(w, 0.), point(0., h), point(w, h)];
    let size = b.focus_size.clamp(0.0, 1.0) as f32;
    let at = |p: Point<f32>| bounds.origin + point(px(p.x), px(p.y));
    let fill_colour = t.accent.opacity(0.055);
    window.with_content_mask(Some(ContentMask { bounds }), |window| match b.mode {
        BlurMode::Radial => {
            let reach = corners
                .iter()
                .map(|c| ((c.x - focus.x).powi(2) + (c.y - focus.y).powi(2)).sqrt())
                .fold(0.0, f32::max);
            let least = 0.04 * shortest;
            let r = least + size * (reach - least).max(0.);
            let square = Bounds::new(at(point(focus.x - r, focus.y - r)), size_of(2. * r, 2. * r));
            window.paint_quad(
                fill(square, fill_colour)
                    .corner_radii(px(r))
                    .border_widths(px(1.))
                    .border_color(t.accent.opacity(0.34)),
            );
        }
        BlurMode::Directional => {
            let angle = (b.direction_degrees as f32).to_radians();
            let along = point(angle.cos(), angle.sin());
            let normal = point(-angle.sin(), angle.cos());
            let reach = corners
                .iter()
                .map(|c| ((c.x - focus.x) * normal.x + (c.y - focus.y) * normal.y).abs())
                .fold(0.0, f32::max);
            let least = 0.025 * shortest;
            let half = least + size * (reach - least).max(0.);
            let long = w + h;
            let corner = |s: f32, n: f32| {
                at(point(
                    focus.x + along.x * s + normal.x * n,
                    focus.y + along.y * s + normal.y * n,
                ))
            };
            let outline = [
                corner(-long, -half),
                corner(long, -half),
                corner(long, half),
                corner(-long, half),
            ];
            let mut band = PathBuilder::fill();
            band.add_polygon(&outline, true);
            if let Ok(path) = band.build() {
                window.paint_path(path, fill_colour);
            }
            for edge in [-half, half] {
                let mut line = PathBuilder::stroke(px(1.));
                line.move_to(corner(-long, edge));
                line.line_to(corner(long, edge));
                if let Ok(path) = line.build() {
                    window.paint_path(path, t.accent.opacity(0.28));
                }
            }
        }
    });
}

fn size_of(w: f32, h: f32) -> gpui::Size<Pixels> {
    size(px(w), px(h))
}

impl Render for BlurBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let blur = self.look.read(cx).settings().progressive_blur.clone();
        let on = blur.is_enabled;
        self.sliders.begin_frame();

        let apply_to = self.segmented(
            BlurControl::ApplyTo,
            [
                (
                    fl!("editor-blur-target-screenshot"),
                    Some(fl!("editor-blur-target-screenshot-help").into()),
                ),
                (
                    fl!("editor-blur-target-scene"),
                    Some(fl!("editor-blur-target-scene-help").into()),
                ),
            ],
            usize::from(blur.edge_mode == BlurEdgeMode::Bleed),
            cx,
            |b, i| {
                b.edge_mode = if i == 0 {
                    BlurEdgeMode::Clipped
                } else {
                    BlurEdgeMode::Bleed
                }
            },
        );
        let mode = self.segmented(
            BlurControl::Mode,
            [
                (fl!("editor-blur-mode-radial"), None),
                (fl!("editor-blur-mode-directional"), None),
            ],
            usize::from(blur.mode == BlurMode::Directional),
            cx,
            |b, i| {
                b.mode = if i == 0 {
                    BlurMode::Radial
                } else {
                    BlurMode::Directional
                }
            },
        );

        let shown = BlurControl::SLIDERS
            .into_iter()
            .filter(|&c| c != BlurControl::Direction || blur.mode == BlurMode::Directional);
        let mut sliders = Vec::new();
        for c in shown {
            let row = self
                .sliders
                .row(c, || c.field(), c.get(&blur), !on, window, cx);
            sliders.push(if c == BlurControl::FocusSize {
                let help = self.focus_size_help(cx);
                div()
                    .id("blur-focus-size-help")
                    .w_full()
                    .child(row)
                    .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
                    .into_any_element()
            } else {
                row
            });
        }

        let percent = |v: f64| (v * 100.0).round() as i64;
        let (x, y, focus_size) = (
            percent(blur.focus_x),
            percent(blur.focus_y),
            percent(blur.focus_size),
        );
        let this = cx.entity().downgrade();
        let pad = FocusPad::new("blur-focus-pad", fl!("editor-blur-focus-pad-a11y"))
            .position(point(blur.focus_x as f32, blur.focus_y as f32))
            .value(fl!(
                "editor-blur-focus-pad-value",
                x = x,
                y = y,
                size = focus_size
            ))
            .when(on, |pad| {
                pad.on_change(move |at, _, cx| {
                    let _ = this.update(cx, |body, cx| {
                        body.edit(cx, |b| {
                            b.focus_x = f64::from(at.x);
                            b.focus_y = f64::from(at.y);
                        })
                    });
                })
            });
        let region = {
            let blur = blur.clone();
            let t = t.clone();
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| paint_region(window, bounds, &blur, &t),
            )
            .absolute()
            .inset_0()
            .size_full()
        };
        let starting = cx.entity().downgrade();
        let ending = starting.clone();
        let ending_out = starting.clone();
        let pad = div()
            .relative()
            .w_full()
            .child(region)
            .child(pad)
            .when(on, |d| {
                // Capture, so the adjustment is open before the pad's own press moves the focus.
                d.capture_any_mouse_down(move |event, _, cx| {
                    if event.button == MouseButton::Left {
                        let _ = starting.update(cx, |body, cx| body.live_adjust(true, cx));
                    }
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    let _ = ending.update(cx, |body, cx| body.live_adjust(false, cx));
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    let _ = ending_out.update(cx, |body, cx| body.live_adjust(false, cx));
                })
            });
        let position = div()
            .flex()
            .items_center()
            .justify_between()
            .pb(m.group_label_spacing)
            .typeset(t.type_scale.label)
            .text_color(t.text_secondary)
            .child(fl!("editor-blur-focus-position"))
            .child(div().typeset(t.type_scale.value).child(fl!(
                "editor-blur-focus-readout",
                x = x,
                y = y
            )));
        let focus = div()
            .flex()
            .flex_col()
            .when(!on, |d| d.opacity(DIMMED))
            .child(position)
            .child(self.sliders.recorded(BlurControl::FocusPad, pad));

        div()
            .flex()
            .flex_col()
            .gap(m.section_padding)
            .child(apply_to)
            .child(mode)
            .child(div().flex().flex_col().gap(m.row_spacing).children(sliders))
            .child(focus)
    }
}
