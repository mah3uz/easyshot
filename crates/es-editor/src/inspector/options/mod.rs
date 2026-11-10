pub mod crop;
mod font_picker;
mod reset;
mod sections;
pub(crate) mod swatches;
mod text_box;
mod text_style;

use es_anno::{GestureId, StylePatch};
use es_model::Tool;
use es_ui::{
    a11y::{Accessible, Role},
    components::{EditorButton, InspectorSlider, SliderEvent, ValueFormat},
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Div, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Subscription,
    Window, div, prelude::FluentBuilder, px,
};

use crate::session::{
    EditorSession,
    crop::{CropEvent, CropModel},
    doc::{DocEvent, DocModel},
    style::StyleStore,
};
use crate::strings::fl;
use crate::toolbar::{Drawn, tools::title, track};
use swatches::Facets;

const SECTION_GAP: Pixels = px(16.);
const LABEL_GAP: Pixels = px(8.);
const HEADER_PADDING_Y: Pixels = px(7.);

/// Asks the window to show the sidebar, which may be hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolOptionsEvent {
    Opened,
}

/// A control whose last drawn bounds the page remembers, for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Page,
    Back,
    Swatches,
    Stroke,
    FontFamily,
    FontSize,
    TextFill,
    TextFillNone,
    TextBorder,
    TextBorderNone,
    TextBorderWidth,
    ArrowStart,
    ArrowEnd,
    Dash,
    HighlighterMode,
    Strength,
    Delete,
    ResetStyle,
    CropAspect,
    CropReset,
    CropCancel,
    CropApply,
}

/// The sidebar page of the armed tool's options, of the selection's, or of the crop in progress.
pub struct ToolOptions {
    doc: Entity<DocModel>,
    crop: Entity<CropModel>,
    style: Entity<StyleStore>,
    open: bool,
    strength: Option<Entity<InspectorSlider>>,
    font_size: Option<Entity<gpui_component::input::InputState>>,
    fonts: Option<font_picker::FontPicker>,
    gestures: u64,
    dragging: Option<GestureId>,
    drawn: Drawn<Control>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ToolOptionsEvent> for ToolOptions {}

impl ToolOptions {
    pub fn new(session: &EditorSession, cx: &mut Context<Self>) -> Self {
        let doc = session.doc.clone();
        let on_doc = cx.subscribe(&doc, |this, _, event: &DocEvent, cx| this.on_doc(event, cx));
        let on_crop = cx.subscribe(&session.crop, |this, _, event: &CropEvent, cx| {
            if *event == CropEvent::Started {
                this.request_sidebar(cx);
            }
            this.redraw(cx);
        });
        let crop_changed = cx.observe(&session.crop, |_, _, cx| cx.notify());
        Self {
            doc,
            crop: session.crop.clone(),
            style: session.style.clone(),
            open: false,
            strength: None,
            font_size: None,
            fonts: None,
            gestures: 0,
            dragging: None,
            drawn: Drawn::default(),
            _subscriptions: vec![on_doc, on_crop, crop_changed],
        }
    }

    /// A newly armed tool brings its page forward; a new selection shows its page only if the sidebar is open.
    fn on_doc(&mut self, event: &DocEvent, cx: &mut Context<Self>) {
        let editor = self.doc.read(cx).editor();
        match event {
            DocEvent::ToolChanged => {
                self.open = true;
                if editor.tool() != Tool::Select {
                    self.request_sidebar(cx);
                }
            }
            DocEvent::SelectionChanged if !editor.selected_ids().is_empty() => self.open = true,
            _ => {}
        }
        self.redraw(cx);
    }

    fn request_sidebar(&self, cx: &mut Context<Self>) {
        if self.shows_page(cx) {
            cx.emit(ToolOptionsEvent::Opened);
        }
    }

    /// A page that is no longer drawn never repaints, so its old bounds go now.
    fn redraw(&self, cx: &mut Context<Self>) {
        self.drawn.borrow_mut().clear();
        cx.notify();
    }

    fn cropping(&self, cx: &App) -> bool {
        self.crop.read(cx).is_cropping()
    }

    /// Whether the sidebar shows this page instead of the usual inspector.
    pub fn shows_page(&self, cx: &App) -> bool {
        self.cropping(cx) || (self.open && Facets::of(self.doc.read(cx).editor()).is_some())
    }

    pub fn show(&mut self, cx: &mut Context<Self>) {
        self.open = true;
        self.request_sidebar(cx);
        self.redraw(cx);
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.redraw(cx);
    }

    /// Where a control was last drawn, in window coordinates; `None` when it is not shown.
    pub fn control_bounds(&self, control: Control) -> Option<Bounds<Pixels>> {
        self.drawn.borrow().get(&control).copied()
    }

    /// The text in the font size field; `None` before the text page first draws.
    pub fn font_size_text(&self, cx: &App) -> Option<gpui::SharedString> {
        self.font_size.as_ref().map(|f| f.read(cx).value())
    }

    /// Sets the style for new shapes and restyles the selection as one undo step; calls sharing a gesture make one.
    pub fn apply_style(
        &mut self,
        patch: StylePatch,
        gesture: Option<GestureId>,
        cx: &mut Context<Self>,
    ) {
        let gesture = gesture.unwrap_or_else(|| self.fresh_gesture());
        self.doc
            .update(cx, |d, cx| d.apply(cx, |e| e.apply_style(patch, gesture)));
        self.style.update(cx, |s, cx| s.changed(cx));
        cx.notify();
    }

    fn fresh_gesture(&mut self) -> GestureId {
        self.gestures += 1;
        GestureId(self.gestures)
    }

    fn switch_highlighter_mode(&mut self, cx: &mut Context<Self>) {
        self.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_tool(Tool::Highlighter))
        });
    }

    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        self.doc
            .update(cx, |d, cx| d.apply(cx, |e| e.delete_selected()));
    }

    /// Built on first draw, since the slider needs a window; kept in step with the engine's density.
    fn strength(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InspectorSlider> {
        let density = self.doc.read(cx).editor().current_style().redaction_density;
        if let Some(slider) = &self.strength {
            if slider.read(cx).value() != density.clamp(0.15, 1.0) {
                slider.update(cx, |s, cx| s.set_value(density, window, cx));
            }
            return slider.clone();
        }
        let slider = cx.new(|cx| {
            InspectorSlider::new(
                fl!("editor-redact-strength"),
                density,
                0.15..=1.0,
                ValueFormat::percent(),
                window,
                cx,
            )
        });
        let on_strength = cx.subscribe(&slider, |this, _, event: &SliderEvent, cx| match event {
            SliderEvent::DragStarted => this.dragging = Some(this.fresh_gesture()),
            SliderEvent::Changed(value) => {
                let gesture = this.dragging;
                this.apply_style(StylePatch::RedactionDensity(*value), gesture, cx);
            }
            SliderEvent::DragEnded => this.dragging = None,
        });
        self._subscriptions.push(on_strength);
        self.strength = Some(slider.clone());
        slider
    }

    fn track(&self, control: Control, child: impl IntoElement) -> Div {
        track(&self.drawn, control, child)
    }

    fn back_button(&self, cx: &mut Context<Self>) -> Div {
        let weak = cx.entity().downgrade();
        self.track(
            Control::Back,
            EditorButton::new("options-back")
                .icon(IconName::ChevronLeft)
                .accessible_label(fl!("editor-options-back"))
                .tip(fl!("editor-options-back-help"))
                .padding_x(px(6.))
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |o, cx| o.back(cx));
                }),
        )
    }

    fn delete_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.entity().downgrade();
        let shortcut = crate::toolbar::tools::shortcut_label("image-delete").unwrap_or_default();
        let button = EditorButton::new("delete")
            .icon(IconName::Trash)
            .accessible_label(fl!("editor-delete-a11y"))
            .tip(fl!("editor-delete-help", shortcut = shortcut))
            .padding_x(tokens(cx).metrics.tool_button_padding_x)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |o, cx| o.delete_selection(cx));
            });
        self.track(Control::Delete, button).into_any_element()
    }
}

/// A labelled row of the page, in the inspector's label style.
pub(super) fn section(label: String, control: impl IntoElement, t: &Tokens) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(LABEL_GAP)
        .child(
            div()
                .text_color(t.text_secondary)
                .typeset(t.type_scale.label)
                .child(label),
        )
        .child(control)
}

impl Render for ToolOptions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn.borrow_mut().clear();
        let t = tokens(cx).clone();
        let m = t.metrics;
        let (heading, back, action, body) = if self.cropping(cx) {
            (fl!("editor-crop"), None, None, crop::content(self, cx))
        } else {
            let (facets, style) = {
                let editor = self.doc.read(cx).editor();
                (Facets::of(editor), editor.current_style().clone())
            };
            let Some(facets) = facets.filter(|_| self.open) else {
                return div().id("tool-options");
            };
            let body = sections::content(self, facets, &style, window, cx)
                .child(reset::button(self, facets, cx));
            let action = facets.selection.then(|| self.delete_button(cx));
            (
                title(facets.inspected),
                Some(self.back_button(cx)),
                action,
                body,
            )
        };
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(m.button_height + HEADER_PADDING_Y * 2.)
            .px(m.inspector_padding)
            .when_some(back, |d, back| d.ml(px(-6.)).child(back))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .typeset(t.type_scale.section_header)
                    .text_color(t.text)
                    .child(heading.clone()),
            )
            .children(action);
        let page = div()
            .id("tool-options")
            .flex()
            .flex_col()
            .size_full()
            .text_color(t.text)
            .accessible(Role::Group, fl!("editor-tool-options-a11y", tool = heading))
            .child(header)
            .child(
                div()
                    .mx(m.inspector_padding)
                    .h(m.hairline)
                    .bg(t.separator.opacity(0.45)),
            )
            .child(
                div()
                    .id("tool-options-sections")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        body.gap(SECTION_GAP)
                            .px(m.inspector_padding)
                            .pt(m.section_padding)
                            .pb(m.section_padding * 2.),
                    ),
            );
        self.track(Control::Page, page)
            .size_full()
            .id("tool-options-page")
    }
}
