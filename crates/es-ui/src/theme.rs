use std::mem;

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use gpui_component::{Theme, ThemeMode as KitMode};

use crate::tokens::{Appearance, METRICS, ThemeMode, Tokens};

pub const UI_FONT: &str = "Inter";

impl gpui::Global for Tokens {}

#[derive(Clone)]
struct Palette {
    tokens: Tokens,
    kit: Theme,
}

struct Palettes {
    light: Palette,
    dark: Palette,
}

impl gpui::Global for Palettes {}

impl Palettes {
    fn get(&self, appearance: Appearance) -> &Palette {
        match appearance {
            Appearance::Light => &self.light,
            Appearance::Dark => &self.dark,
        }
    }
}

impl From<Appearance> for KitMode {
    fn from(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => KitMode::Light,
            Appearance::Dark => KitMode::Dark,
        }
    }
}

pub fn install(cx: &mut App) {
    gpui_component::init(cx);
    crate::components::tooltip::install(cx);
    let palettes = Palettes {
        light: palette(Appearance::Light, cx),
        dark: palette(Appearance::Dark, cx),
    };
    cx.set_global(palettes);
    set_mode(ThemeMode::Dark, cx);
}

pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    let palette = cx.global::<Palettes>().get(mode.appearance()).clone();
    *Theme::global_mut(cx) = palette.kit;
    Theme::sync_base(cx);
    cx.set_global(palette.tokens);
    cx.refresh_windows();
}

pub fn tokens(cx: &App) -> &Tokens {
    cx.global::<Tokens>()
}

fn palette(appearance: Appearance, cx: &mut App) -> Palette {
    let tokens = Tokens::new(appearance);
    Theme::change(KitMode::from(appearance), None, cx);
    Theme::update(cx, |theme| apply(&tokens, theme));
    Palette {
        tokens,
        kit: Theme::global(cx).clone(),
    }
}

fn apply(t: &Tokens, theme: &mut Theme) {
    theme.font_family = UI_FONT.into();
    theme.radius = METRICS.button_radius;
    theme.radius_lg = METRICS.popover_radius;
    let c = &mut theme.colors;
    c.background = t.window;
    c.foreground = t.text;
    c.border = t.border;
    c.window_border = t.border;
    c.input = t.bordered_border;
    c.ring = t.accent;
    c.caret = t.accent;
    c.selection = t.accent_tint;
    c.muted = t.track;
    c.muted_foreground = t.text_secondary;
    c.accent = t.hover_fill;
    c.accent_foreground = t.text;
    c.primary = t.accent;
    c.primary_hover = t.accent.opacity(0.9);
    c.primary_active = t.accent.opacity(0.8);
    c.primary_foreground = t.on_accent;
    c.secondary = t.bordered_fill;
    c.secondary_hover = t.hover_fill;
    c.secondary_active = t.press_fill;
    c.secondary_foreground = t.text;
    c.popover = t.dropdown;
    c.popover_foreground = t.text;
    c.danger = t.destructive;
    c.success = t.success;
    c.link = t.accent;
    c.title_bar = t.window;
    c.title_bar_border = t.border;
    c.sidebar = t.inspector;
    c.sidebar_border = t.border;
    c.sidebar_foreground = t.text;
    c.tab_bar_segmented = t.segment_track;
    c.slider_bar = t.accent;
    c.list_active = t.accent_tint;
    c.list_hover = t.hover_fill;
}

/// Draws its child with another appearance's tokens and component theme, so both can share one window.
pub fn scoped(appearance: Appearance, child: impl IntoElement) -> Scoped {
    Scoped {
        appearance,
        child: child.into_any_element(),
    }
}

pub struct Scoped {
    appearance: Appearance,
    child: AnyElement,
}

impl Scoped {
    // Components read the theme while they lay out and paint, so the swap spans each phase.
    fn within<R>(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        phase: impl FnOnce(&mut AnyElement, &mut Window, &mut App) -> R,
    ) -> R {
        let mut palette = cx.global::<Palettes>().get(self.appearance).clone();
        swap(&mut palette, cx);
        let result = phase(&mut self.child, window, cx);
        swap(&mut palette, cx);
        result
    }
}

fn swap(palette: &mut Palette, cx: &mut App) {
    mem::swap(Theme::global_mut(cx), &mut palette.kit);
    mem::swap(cx.global_mut::<Tokens>(), &mut palette.tokens);
}

impl IntoElement for Scoped {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Scoped {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let layout = self.within(window, cx, |child, window, cx| {
            child.request_layout(window, cx)
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(window, cx, |child, window, cx| {
            child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.within(window, cx, |child, window, cx| child.paint(window, cx));
    }
}
