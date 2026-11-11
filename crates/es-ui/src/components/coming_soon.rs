use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, RenderOnce, StatefulInteractiveElement, Styled, Window, div, hsla,
    prelude::FluentBuilder, px, relative,
};
use gpui_component::Icon;

use crate::{
    a11y::{Accessible, Role},
    fl,
    icons::IconName,
    motion::{
        Clocked, PANEL_IN, PANEL_IN_SCALE, PANEL_OUT, PANEL_OUT_SCALE, PANEL_REDUCED, PANEL_RISE,
        Transition,
    },
    theme::{UI_FONT, tokens},
    tokens::Typeset,
};

use super::{IconButton, popover_shadow};

const WIDTH: Pixels = px(340.);
const RADIUS: Pixels = px(16.);
const PADDING_X: Pixels = px(28.);
const PADDING_TOP: Pixels = px(28.);
const PADDING_BOTTOM: Pixels = px(26.);
const CLOSE_INSET: Pixels = px(10.);
const MARK: Pixels = px(56.);
const MARK_RADIUS: Pixels = px(14.);
const MARK_GLYPH: Pixels = px(28.);
const BADGE_HEIGHT: Pixels = px(20.);
const NAME_SIZE: Pixels = px(16.);
const SCRIM_INK: f32 = 0.22;
const SHADOW: f32 = 2.;

/// A feature that is planned but not built yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    VideoStudio,
}

impl Feature {
    pub fn icon(self) -> IconName {
        match self {
            Self::VideoStudio => IconName::Film,
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::VideoStudio => fl!("coming-soon-video-studio"),
        }
    }

    pub fn line(self) -> String {
        match self {
            Self::VideoStudio => fl!("coming-soon-video-studio-line"),
        }
    }

    pub fn badge() -> String {
        fl!("coming-soon-badge")
    }

    pub fn label(self) -> String {
        fl!("coming-soon-a11y", feature = self.name())
    }
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The only "coming soon" surface: a panel centred over a scrim that fills its host; the close
/// button and a press on the scrim call [`ComingSoon::on_close`], and the host plays the exit with
/// [`ComingSoon::leaving`] for [`PANEL_OUT`].
#[derive(IntoElement)]
pub struct ComingSoon {
    id: ElementId,
    feature: Feature,
    leaving: bool,
    settled: bool,
    on_close: Option<CloseHandler>,
}

impl ComingSoon {
    pub fn new(id: impl Into<ElementId>, feature: Feature) -> Self {
        Self {
            id: id.into(),
            feature,
            leaving: false,
            settled: false,
            on_close: None,
        }
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn leaving(mut self, leaving: bool) -> Self {
        self.leaving = leaving;
        self
    }

    /// Draws the panel at rest with no entrance, for pictures.
    pub fn settled(mut self) -> Self {
        self.settled = true;
        self
    }
}

struct Motion {
    entry: Option<(Clocked, bool)>,
    exit: Option<(Clocked, bool)>,
}

fn clock(transition: Transition, cx: &App) -> Option<(Clocked, bool)> {
    Clocked::start(transition, cx)
        .map(|c| (c, true))
        .or_else(|| Clocked::start(PANEL_REDUCED, cx).map(|c| (c, false)))
}

/// The panel's scale, how far below its place it is drawn, and the opacity of panel and scrim.
fn pose(window: &mut Window, cx: &mut App, id: ElementId, leaving: bool) -> (f32, Pixels, f32) {
    let motion = window.use_keyed_state(id, cx, |_, cx| Motion {
        entry: clock(PANEL_IN, cx),
        exit: None,
    });
    let (entry, exit) = motion.update(cx, |m, cx| {
        if leaving && m.exit.is_none() {
            m.exit = clock(PANEL_OUT, cx);
        }
        (m.entry, m.exit)
    });
    if leaving {
        let Some((c, moves)) = exit else {
            return (1., px(0.), 0.);
        };
        let o = c.progress(window, cx);
        let scale = if moves {
            1. - (1. - PANEL_OUT_SCALE) * o
        } else {
            1.
        };
        return (scale, px(0.), 1. - o);
    }
    let Some((c, moves)) = entry else {
        return (1., px(0.), 1.);
    };
    let e = c.progress(window, cx);
    if !moves {
        return (1., px(0.), e);
    }
    (
        PANEL_IN_SCALE + (1. - PANEL_IN_SCALE) * e,
        PANEL_RISE * (1. - e),
        (e * 1.5).clamp(0., 1.),
    )
}

impl RenderOnce for ComingSoon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let (scale, rise, alpha) = if self.settled && !self.leaving {
            (1., px(0.), 1.)
        } else {
            pose(window, cx, self.id.clone(), self.leaving)
        };
        let feature = self.feature;
        let inset = relative((1. - scale) / 2.);
        let shell = div()
            .absolute()
            .top(inset)
            .bottom(inset)
            .left(inset)
            .right(inset)
            .rounded(RADIUS)
            .bg(t.dropdown)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .shadow(popover_shadow(SHADOW));
        let close = self.on_close.clone();
        let scrim_close = self.on_close;
        let mark = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(MARK)
            .rounded(MARK_RADIUS)
            .bg(t.accent_tint)
            .text_color(t.accent)
            .child(Icon::new(feature.icon()).size(MARK_GLYPH));
        let badge = div()
            .flex()
            .flex_none()
            .items_center()
            .h(BADGE_HEIGHT)
            .px(px(9.))
            .rounded_full()
            .bg(t.accent)
            .text_color(t.on_accent)
            .typeset(t.type_scale.section_header)
            .child(Feature::badge());
        let panel = div()
            .id("coming-soon-panel")
            .accessible(Role::Dialog, feature.label())
            .aria_description(feature.line())
            .relative()
            .top(rise)
            .w(WIDTH)
            .flex()
            .flex_col()
            .items_center()
            .pt(PADDING_TOP)
            .pb(PADDING_BOTTOM)
            .px(PADDING_X)
            .font_family(UI_FONT)
            .text_color(t.text)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(shell)
            .child(mark)
            .child(div().mt(px(16.)).child(badge))
            .child(
                div()
                    .mt(px(10.))
                    .text_size(NAME_SIZE)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(feature.name()),
            )
            .child(
                div()
                    .mt(px(6.))
                    .w_full()
                    .text_center()
                    .typeset(t.type_scale.footer)
                    .line_height(px(18.))
                    .text_color(t.text_secondary)
                    .child(feature.line()),
            )
            .child(
                div().absolute().top(CLOSE_INSET).right(CLOSE_INSET).child(
                    IconButton::new("coming-soon-close", IconName::X, fl!("coming-soon-close"))
                        .when_some(close, |b, close| {
                            b.on_click(move |_, window, cx| close(window, cx))
                        }),
                ),
            );
        div()
            .id(self.id)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .when(alpha < 1., |d| d.opacity(alpha))
            .bg(hsla(0., 0., 0., SCRIM_INK))
            .when_some(scrim_close, |d, close| {
                d.on_mouse_down(MouseButton::Left, move |_, window, cx| close(window, cx))
            })
            .child(panel)
    }
}
