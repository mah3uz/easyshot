use std::sync::Arc;

use es_model::{
    BackgroundColor, BackgroundGradient, BackgroundStyle,
    background::{MAX_GRADIENT_STOPS, MIN_GRADIENT_STOPS, clean_gradient_name},
};
use es_render::beautify::gradients::{PRESETS, render_gradient};
use es_store::{Paths, presets::GradientFile};
use es_ui::components::{InspectorSlider, SliderEvent, ValueFormat};
use gpui::{App, AppContext, Context, Entity, Hsla, RenderImage, Rgba, Subscription, Window};
use gpui_component::{
    color_picker::{ColorPickerEvent, ColorPickerState},
    input::{InputEvent, InputState},
};

use super::view::{gradient_name, hsla};
use super::{BackgroundBody, GRADIENT_PIXELS};
use crate::canvas::underlay;
use crate::strings::fl;

pub const QUICK_ANGLES: [f64; 6] = [0., 45., 90., 135., 180., 270.];

/// A gradient being made or changed; the canvas shows it until it is saved or cancelled.
pub(super) struct Draft {
    pub(super) gradient: BackgroundGradient,
    pub(super) is_new: bool,
    before: BackgroundStyle,
    pub(super) name: Entity<InputState>,
    pub(super) stops: Vec<Entity<ColorPickerState>>,
    pub(super) angle: Entity<InspectorSlider>,
    _subscriptions: Vec<Subscription>,
    stop_subscriptions: Vec<Subscription>,
}

pub(super) fn thumb(g: &BackgroundGradient) -> Option<Arc<RenderImage>> {
    render_gradient(g, GRADIENT_PIXELS, GRADIENT_PIXELS)
        .and_then(|pixmap| underlay::render_image(&pixmap))
}

pub(super) fn open_mine(paths: Option<&Paths>) -> Option<GradientFile> {
    match es_store::presets::open_gradients(paths?) {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(%error, "cannot read the saved gradients");
            None
        }
    }
}

pub(super) fn display_name(g: &BackgroundGradient) -> String {
    if g.is_user() {
        g.title.clone()
    } else {
        gradient_name(&g.id)
    }
}

fn colour_of(colour: Hsla) -> BackgroundColor {
    let c = Rgba::from(colour);
    BackgroundColor::custom(f64::from(c.r), f64::from(c.g), f64::from(c.b))
}

fn mix(a: &BackgroundColor, b: &BackgroundColor) -> BackgroundColor {
    BackgroundColor::custom(
        (a.red + b.red) / 2.0,
        (a.green + b.green) / 2.0,
        (a.blue + b.blue) / 2.0,
    )
}

impl BackgroundBody {
    pub fn user_gradients(&self) -> &[BackgroundGradient] {
        self.mine
            .as_ref()
            .map_or(&[], |file| file.value.gradients.as_slice())
    }

    pub fn gradient_draft(&self) -> Option<&BackgroundGradient> {
        self.draft.as_ref().map(|d| &d.gradient)
    }

    /// The gradient on the canvas when it is a built-in or one of the user's.
    pub fn selected_gradient(&self, cx: &App) -> Option<BackgroundGradient> {
        let BackgroundStyle::Gradient(g) = &self.settings(cx).style else {
            return None;
        };
        let known = PRESETS.iter().any(|p| p.id == g.id)
            || self.user_gradients().iter().any(|u| u.id == g.id);
        known.then(|| g.clone())
    }

    /// A new gradient that starts from the selected one, or from the first built-in.
    pub fn new_gradient(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut start = self
            .selected_gradient(cx)
            .unwrap_or_else(|| PRESETS[0].style());
        start.id = String::new();
        start.title = fl!("editor-gradient-untitled");
        self.open_draft(start, window, cx);
    }

    /// A user gradient is changed in place; a built-in one stays fixed, so this edits a copy.
    pub fn edit_gradient(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut start) = self.selected_gradient(cx) else {
            return self.new_gradient(window, cx);
        };
        if !start.is_user() {
            start.title = fl!("editor-gradient-copy-name", name = gradient_name(&start.id));
            start.id = String::new();
        }
        self.open_draft(start, window, cx);
    }

    fn open_draft(
        &mut self,
        mut gradient: BackgroundGradient,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        gradient.clamp_stops();
        let before = self
            .draft
            .take()
            .map_or_else(|| self.settings(cx).style.clone(), |d| d.before);
        let title = gradient.title.clone();
        let placeholder = fl!("editor-gradient-name-placeholder");
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(title)
        });
        let angle_value = gradient.angle_degrees().round().rem_euclid(360.0);
        let angle = cx.new(|cx| {
            InspectorSlider::new(
                fl!("editor-gradient-angle"),
                angle_value,
                0.0..=359.0,
                ValueFormat::degrees(),
                window,
                cx,
            )
        });
        let subscriptions = vec![
            cx.subscribe_in(&name, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.save_gradient(cx);
                }
            }),
            cx.subscribe_in(
                &angle,
                window,
                |this, _, event: &SliderEvent, _, cx| match *event {
                    SliderEvent::DragStarted => {
                        this.look.update(cx, |look, cx| look.begin_live_adjust(cx))
                    }
                    SliderEvent::Changed(degrees) => this.apply_angle(degrees, cx),
                    SliderEvent::DragEnded => {
                        this.look.update(cx, |look, cx| look.end_live_adjust(cx))
                    }
                },
            ),
        ];
        self.draft = Some(Draft {
            is_new: !gradient.is_user(),
            gradient,
            before,
            name,
            stops: Vec::new(),
            angle,
            _subscriptions: subscriptions,
            stop_subscriptions: Vec::new(),
        });
        self.rebuild_stops(window, cx);
        self.preview_draft(cx);
    }

    fn rebuild_stops(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.draft else {
            return;
        };
        let colours = draft.gradient.colors.clone();
        let mut stops = Vec::new();
        let mut subscriptions = Vec::new();
        for (i, colour) in colours.iter().enumerate() {
            let value = hsla(colour);
            let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(value));
            subscriptions.push(cx.subscribe_in(
                &picker,
                window,
                move |this, _, event: &ColorPickerEvent, _, cx| {
                    if let ColorPickerEvent::Change(Some(colour)) = event {
                        this.set_stop_color(i, *colour, cx);
                    }
                },
            ));
            stops.push(picker);
        }
        if let Some(draft) = &mut self.draft {
            draft.stops = stops;
            draft.stop_subscriptions = subscriptions;
        }
        cx.notify();
    }

    pub fn set_stop_color(&mut self, index: usize, colour: Hsla, cx: &mut Context<Self>) {
        if let Some(slot) = self
            .draft
            .as_mut()
            .and_then(|d| d.gradient.colors.get_mut(index))
        {
            *slot = colour_of(colour);
            self.preview_draft(cx);
        }
    }

    /// The new colour sits between the last two, so adding it leaves the picture as it was.
    pub fn add_stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        let colors = &mut draft.gradient.colors;
        if colors.len() >= MAX_GRADIENT_STOPS {
            return;
        }
        let last = colors.len() - 1;
        let between = mix(&colors[last - 1], &colors[last]);
        colors.insert(last, between);
        self.rebuild_stops(window, cx);
        self.preview_draft(cx);
    }

    pub fn remove_stop(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        if draft.gradient.colors.len() <= MIN_GRADIENT_STOPS || index >= draft.gradient.colors.len()
        {
            return;
        }
        draft.gradient.colors.remove(index);
        self.rebuild_stops(window, cx);
        self.preview_draft(cx);
    }

    pub fn set_gradient_angle(
        &mut self,
        degrees: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(draft) = &self.draft {
            draft
                .angle
                .update(cx, |slider, cx| slider.set_value(degrees, window, cx));
        }
        self.apply_angle(degrees, cx);
    }

    fn apply_angle(&mut self, degrees: f64, cx: &mut Context<Self>) {
        if let Some(draft) = &mut self.draft {
            draft.gradient.set_angle_degrees(degrees);
            self.preview_draft(cx);
        }
    }

    pub fn set_gradient_name(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = &self.draft {
            let text = text.to_owned();
            draft
                .name
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    fn preview_draft(&mut self, cx: &mut Context<Self>) {
        if let Some(draft) = &self.draft {
            self.set_style(BackgroundStyle::Gradient(draft.gradient.clone()), cx);
        }
        cx.notify();
    }

    /// Stored through es-store and put on the canvas under its saved id; `None` without a draft.
    pub fn save_gradient(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let mut draft = self.draft.take()?;
        let typed = draft.name.read(cx).value().to_string();
        if let Some(name) = clean_gradient_name(&typed) {
            draft.gradient.title = name;
        } else if draft.gradient.title.trim().is_empty() {
            draft.gradient.title = fl!("editor-gradient-untitled");
        }
        let id = self.edit_mine(cx, |lib| lib.save(draft.gradient.clone()));
        let saved = self.user_gradients().iter().find(|g| g.id == id).cloned();
        if let Some(saved) = saved {
            self.set_style(BackgroundStyle::Gradient(saved), cx);
        }
        cx.notify();
        Some(id)
    }

    pub fn cancel_gradient(&mut self, cx: &mut Context<Self>) {
        if let Some(draft) = self.draft.take() {
            self.set_style(draft.before, cx);
        }
        cx.notify();
    }

    /// The canvas keeps its colours, as a deleted preset leaves the look it set.
    pub fn delete_gradient(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.draft.as_ref().is_some_and(|d| d.gradient.id == id) {
            self.draft = None;
        }
        self.edit_mine(cx, |lib| lib.remove(id));
        cx.notify();
    }

    fn edit_mine<T>(&mut self, cx: &App, f: impl FnOnce(&mut es_model::GradientLibrary) -> T) -> T {
        let paths = self.look.read(cx).paths().cloned();
        let file = self.mine.get_or_insert_with(|| {
            let path = paths
                .as_ref()
                .map(|p| p.gradients_file())
                .unwrap_or_default();
            GradientFile::fresh(&path, es_model::FormatKind::Gradients)
        });
        let out = f(&mut file.value);
        if paths.is_some()
            && let Err(error) = file.save()
        {
            tracing::warn!(%error, "cannot save the gradients");
        }
        self.mine_thumbs = self.user_gradients().iter().map(thumb).collect();
        out
    }
}
