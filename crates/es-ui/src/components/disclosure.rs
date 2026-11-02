use std::{cell::Cell, f32::consts::FRAC_PI_2, rc::Rc};

use gpui::{
    AnyElement, App, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div,
    prelude::FluentBuilder, px, radians,
};
use gpui_component::{Icon, Sizable, switch::Switch};

use crate::{
    a11y::{Accessible, Role},
    components::{
        IconButton,
        button::{Interaction, focus_ring, keyed_focus, shows_focus},
    },
    fl,
    icons::IconName,
    motion::{SECTION_EXPAND, animate, reduce_motion},
    theme::tokens,
    tokens::Typeset,
};

const CHEVRON_BOX: Pixels = px(18.);
const CHEVRON: Pixels = px(11.);
const ACCESSORY: Pixels = px(18.);
const ACCESSORY_GLYPH: Pixels = px(10.);
const CONTENT_TOP: Pixels = px(4.);

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type ResetHandler = Rc<dyn Fn(&mut Window, &mut App)>;

struct Reset {
    label: SharedString,
    handler: ResetHandler,
}

struct Toggle {
    on: bool,
    handler: ToggleHandler,
}

struct Motion {
    expanded: bool,
    generation: usize,
    closing: bool,
    height: Rc<Cell<Option<Pixels>>>,
}

impl Motion {
    fn follow(&mut self, expanded: bool, reduce: bool, cx: &mut Context<Self>) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        self.generation += 1;
        self.closing = !expanded && SECTION_EXPAND.plays(reduce);
        if self.closing {
            let generation = self.generation;
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(SECTION_EXPAND.duration)
                    .await;
                this.update(cx, |this, cx| {
                    if this.generation == generation {
                        this.closing = false;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
    }
}

/// An inspector section whose title and chevron fold its content; accessories keep their own hit targets.
#[derive(IntoElement)]
pub struct DisclosureSection {
    id: ElementId,
    title: SharedString,
    expanded: bool,
    preview: Interaction,
    on_toggle: Option<ToggleHandler>,
    reset: Option<Reset>,
    toggle: Option<Toggle>,
    children: Vec<AnyElement>,
}

impl DisclosureSection {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            expanded: false,
            preview: Interaction::Rest,
            on_toggle: None,
            reset: None,
            toggle: None,
            children: Vec::new(),
        }
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// `Hover` pins the header highlight and `Focused` the keyboard ring.
    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Shows the × accessory; the label names what it clears, such as "Remove background".
    pub fn reset(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.reset = Some(Reset {
            label: label.into(),
            handler: Rc::new(handler),
        });
        self
    }

    pub fn toggle(
        mut self,
        on: bool,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.toggle = Some(Toggle {
            on,
            handler: Rc::new(handler),
        });
        self
    }
}

impl ParentElement for DisclosureSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DisclosureSection {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let expanded = self.expanded;
        let reduce = reduce_motion(cx);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| Motion {
            expanded,
            generation: 0,
            closing: false,
            height: Rc::default(),
        });
        let (generation, closing, height) = state.update(cx, |motion, cx| {
            motion.follow(expanded, reduce, cx);
            (motion.generation, motion.closing, motion.height.clone())
        });

        let focus = keyed_focus(&self.id, window, cx);
        let ring = shows_focus(&focus, self.preview, window);
        let hover_fill = t.text.opacity(0.025);
        let flip: Option<ToggleHandler> = self.on_toggle.clone().map(|on_toggle| {
            Rc::new(move |_: bool, window: &mut Window, cx: &mut App| {
                on_toggle(!expanded, window, cx)
            }) as ToggleHandler
        });

        let title = div()
            .id("title")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .typeset(t.type_scale.section_header)
            .text_color(t.text)
            .accessible(Role::Button, self.title.clone())
            .aria_expanded(expanded)
            .aria_description(if expanded {
                fl!("panels-section-collapse-hint")
            } else {
                fl!("panels-section-expand-hint")
            })
            .track_focus(&focus)
            .tab_index(0)
            .child(div().truncate().child(self.title.clone()))
            .when(ring, |d| {
                d.child(
                    focus_ring(&t, m.button_radius)
                        .left(-px(6.))
                        .right(-px(6.))
                        .top(px(6.))
                        .bottom(px(6.)),
                )
            })
            .when_some(flip.clone(), |d, flip| {
                d.on_click(move |_, window, cx| flip(true, window, cx))
            });

        let reset = self.reset.map(|reset| {
            let handler = reset.handler;
            IconButton::new("reset", IconName::X, reset.label)
                .size(ACCESSORY, ACCESSORY_GLYPH)
                .on_click(move |_, window, cx| handler(window, cx))
        });

        let toggle = self.toggle.map(|toggle| {
            let handler = toggle.handler;
            Switch::new("toggle")
                .checked(toggle.on)
                .xsmall()
                .accessibility_label(fl!("panels-section-toggle", title = self.title.to_string()))
                .on_click(move |on, window, cx| handler(*on, window, cx))
        });

        let turn = |delta: f32| radians(-FRAC_PI_2 * (1. - delta));
        let chevron_icon = Icon::new(IconName::ChevronDown).size(CHEVRON);
        let chevron_icon = if generation == 0 {
            chevron_icon
                .rotate(turn(if expanded { 1. } else { 0. }))
                .into_any_element()
        } else {
            animate(
                chevron_icon,
                ("chevron", generation),
                SECTION_EXPAND,
                cx,
                move |icon, delta| {
                    let delta = if expanded { delta } else { 1. - delta };
                    icon.rotate(turn(delta))
                },
            )
        };
        let chevron = div()
            .id("chevron")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(CHEVRON_BOX)
            .text_color(t.text_secondary)
            .child(chevron_icon)
            .when_some(flip, |d, flip| {
                d.on_click(move |_, window, cx| flip(true, window, cx))
            });

        let header = div()
            .id("header")
            .flex()
            .items_center()
            .gap(px(6.))
            .h(m.header_height)
            .px(m.inspector_padding)
            .when(self.preview == Interaction::Hover, |d| d.bg(hover_fill))
            .hover(move |s| s.bg(hover_fill))
            .child(title)
            .children(toggle)
            .children(reset)
            .child(chevron);

        let shown = expanded || closing;
        let body = shown.then(|| {
            let measured = height.clone();
            let content = div()
                .relative()
                .flex_none()
                .w_full()
                .flex()
                .flex_col()
                .pt(CONTENT_TOP)
                .pb(m.section_padding)
                .px(m.inspector_padding)
                .children(self.children)
                .child(
                    canvas(
                        move |bounds, _, _| measured.set(Some(bounds.size.height)),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                );
            let clip = div()
                .w_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(content);
            if generation == 0 {
                return clip.into_any_element();
            }
            animate(
                clip,
                ("body", generation),
                SECTION_EXPAND,
                cx,
                move |clip, delta| {
                    let open = if expanded { delta } else { 1. - delta };
                    let clip = clip.opacity(open);
                    match height.get() {
                        Some(h) if delta < 1. || !expanded => clip.h(h * open),
                        _ => clip,
                    }
                },
            )
        });

        div()
            .id(self.id)
            .relative()
            .w_full()
            .flex()
            .flex_col()
            .child(header)
            .children(body)
            .child(
                div()
                    .mx(m.inspector_padding)
                    .h(m.hairline)
                    .bg(t.separator.opacity(0.45)),
            )
    }
}
