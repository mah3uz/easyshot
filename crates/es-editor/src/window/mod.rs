mod crop_bar;
pub mod redact;
mod states;
mod title_bar;

use std::time::Duration;

use es_ui::{
    a11y::{Accessible, Role},
    components::ZoomCommand,
    icons::IconName,
    theme::{UI_FONT, tokens},
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, AnyView, AppContext, Context, Div, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Stateful, Styled, Subscription, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, Sizable, spinner::Spinner};

use crate::inspector::Inspector;
use crate::session::crop::CropEvent;
use crate::toolbar::{ToolRow, ToolRowEvent};
use crate::{EditorCanvas, EditorSession, EditorSource, Result, strings::fl};

pub const TITLE_BAR_HEIGHT: Pixels = px(38.);
pub const TOOL_ROW_HEIGHT: Pixels = px(48.);
pub const FOOTER_HEIGHT: Pixels = px(52.);
pub const INSPECTOR_WIDTH: Pixels = px(300.);
pub const CHROME_PADDING_X: Pixels = px(16.);
pub const FLASH: Duration = Duration::from_millis(1400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Save,
    Export,
    Copy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Progress {
    #[default]
    Idle,
    Busy,
    Done,
}

/// The window performs no IO: whoever owns it does the work and reports back with `finish`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorWindowEvent {
    Save,
    Export,
    Copy,
}

impl EditorWindowEvent {
    pub(crate) fn job(self) -> Job {
        match self {
            Self::Save => Job::Save,
            Self::Export => Job::Export,
            Self::Copy => Job::Copy,
        }
    }
}

impl From<Job> for EditorWindowEvent {
    fn from(job: Job) -> Self {
        match job {
            Job::Save => Self::Save,
            Job::Export => Self::Export,
            Job::Copy => Self::Copy,
        }
    }
}

enum Content {
    Loading,
    Ready {
        session: EditorSession,
        canvas: Entity<EditorCanvas>,
    },
    Failed,
}

#[derive(Default)]
struct Action {
    progress: Progress,
    _reset: Option<Task<()>>,
}

pub struct EditorWindow {
    content: Content,
    tool_row: Option<AnyView>,
    inspector: Option<AnyView>,
    inspector_open: bool,
    pub(crate) low_res: bool,
    pub(crate) low_res_expanded: bool,
    save: Action,
    export: Action,
    copy: Action,
    error: Option<SharedString>,
    pub(crate) redact: redact::State,
    redact_toast: Option<redact::Toast>,
    ocr: es_ocr::OcrOptions,
    redact_tasks: redact::Tasks,
    focus_canvas: bool,
    pub(crate) held: Option<crate::keyboard::Command>,
    guard_hooked: bool,
    shown_title: SharedString,
    _loading: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<EditorWindowEvent> for EditorWindow {}

impl EditorWindow {
    pub fn loading() -> Self {
        Self {
            content: Content::Loading,
            tool_row: None,
            inspector: None,
            inspector_open: true,
            low_res: false,
            low_res_expanded: false,
            save: Action::default(),
            export: Action::default(),
            copy: Action::default(),
            error: None,
            redact: redact::State::default(),
            redact_toast: None,
            ocr: es_ocr::OcrOptions::default(),
            redact_tasks: redact::Tasks::default(),
            focus_canvas: false,
            held: None,
            guard_hooked: false,
            shown_title: SharedString::default(),
            _loading: None,
            _subscriptions: Vec::new(),
        }
    }

    pub fn new(session: EditorSession, cx: &mut Context<Self>) -> Self {
        let mut this = Self::loading();
        this.loaded(Ok(session), cx);
        this
    }

    pub fn open(source: EditorSource, cx: &mut Context<Self>) -> Self {
        let opening = EditorSession::open(source, cx);
        let mut this = Self::loading();
        this._loading = Some(cx.spawn(async move |this, cx| {
            let result = opening.await;
            let _ = this.update(cx, |w, cx| w.loaded(result, cx));
        }));
        this
    }

    pub fn loaded(&mut self, result: Result<EditorSession>, cx: &mut Context<Self>) {
        match result {
            Ok(session) => {
                let canvas = cx.new(|cx| EditorCanvas::new(&session, cx));
                let tool_row = cx.new(|cx| ToolRow::new(&session, cx));
                let look = session.look.clone();
                let inspector = cx.new(|cx| Inspector::new(look, &canvas, cx));
                self.inspector = Some(inspector.into());
                self._subscriptions = vec![
                    cx.observe(&session.doc, |_, _, cx| cx.notify()),
                    cx.observe(&session.viewport, |_, _, cx| cx.notify()),
                    cx.observe(&session.io, |_, _, cx| cx.notify()),
                    cx.subscribe_self(|this, event: &EditorWindowEvent, cx| {
                        this.run_job(event.job(), cx)
                    }),
                    cx.observe(&session.crop, |_, _, cx| cx.notify()),
                    cx.subscribe(&session.crop, {
                        let row = tool_row.clone();
                        move |_, _, event: &CropEvent, cx| {
                            let on = *event == CropEvent::Started;
                            row.update(cx, |r, cx| r.set_cropping(on, cx));
                        }
                    }),
                    cx.subscribe(&tool_row, |this, row, event: &ToolRowEvent, cx| {
                        if *event == ToolRowEvent::ToggleInspector {
                            this.toggle_inspector(cx);
                            let open = this.inspector_open;
                            row.update(cx, |r, cx| r.set_inspector_open(open, cx));
                        } else if *event == ToolRowEvent::Crop {
                            this.with_crop(cx, |c, cx| c.toggle(cx));
                        } else if let ToolRowEvent::SmartRedact(kind) = *event {
                            this.smart_redact(kind, cx);
                        } else if let Some(op) = crop_bar::image_op(*event) {
                            this.with_crop(cx, |c, cx| {
                                c.transform(op, cx);
                            });
                        }
                    }),
                ];
                self.tool_row = Some(tool_row.clone().into());
                self.probe_ocr(cx);
                self.content = Content::Ready { session, canvas };
                self.focus_canvas = true;
            }
            Err(error) => {
                tracing::warn!(%error, "cannot open the screenshot");
                self.content = Content::Failed;
            }
        }
        cx.notify();
    }

    pub fn session(&self) -> Option<&EditorSession> {
        match &self.content {
            Content::Ready { session, .. } => Some(session),
            _ => None,
        }
    }

    pub fn canvas(&self) -> Option<&Entity<EditorCanvas>> {
        match &self.content {
            Content::Ready { canvas, .. } => Some(canvas),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.content, Content::Loading)
    }

    pub fn failed(&self) -> bool {
        matches!(self.content, Content::Failed)
    }

    pub fn title(&self, cx: &gpui::App) -> SharedString {
        self.session()
            .and_then(|s| {
                s.doc
                    .read(cx)
                    .display()
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .filter(|stem| !stem.is_empty())
            .unwrap_or_else(|| fl!("editor-window-title-default"))
            .into()
    }

    pub fn can_undo(&self, cx: &gpui::App) -> bool {
        self.session().is_some_and(|s| s.can_undo(cx))
    }

    pub fn can_redo(&self, cx: &gpui::App) -> bool {
        self.session().is_some_and(|s| s.can_redo(cx))
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.session() {
            session.undo(cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.session() {
            session.redo(cx);
        }
    }

    pub fn zoom_percent(&self, cx: &gpui::App) -> u32 {
        self.session()
            .map_or(100, |s| s.viewport.read(cx).zoom_percent().round() as u32)
    }

    pub fn zoom(&mut self, command: ZoomCommand, cx: &mut Context<Self>) {
        let Some(session) = self.session() else {
            return;
        };
        session.viewport.update(cx, |v, cx| match command {
            ZoomCommand::In => v.zoom_in(cx),
            ZoomCommand::Out => v.zoom_out(cx),
            ZoomCommand::Fit => v.fit(cx),
            ZoomCommand::Percent(p) => v.set_percent(f64::from(p), cx),
        });
    }

    pub fn inspector_open(&self) -> bool {
        self.inspector_open
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = !self.inspector_open;
        cx.notify();
    }

    pub fn toolbar(&self) -> Option<Entity<ToolRow>> {
        self.tool_row.clone()?.downcast::<ToolRow>().ok()
    }

    pub fn inspector(&self) -> Option<Entity<Inspector>> {
        self.inspector.clone()?.downcast::<Inspector>().ok()
    }

    pub fn set_tool_row(&mut self, view: AnyView, cx: &mut Context<Self>) {
        self.tool_row = Some(view);
        cx.notify();
    }

    pub fn set_inspector(&mut self, view: AnyView, cx: &mut Context<Self>) {
        self.inspector = Some(view);
        cx.notify();
    }

    /// Applies the `[editor]` zoom anchor and the `[ocr]` languages to this window.
    pub fn apply_config(&mut self, config: &es_model::Config, cx: &mut Context<Self>) {
        if let Some(session) = self.session() {
            let at_pointer = config.editor.zoom_anchor != "centre";
            session
                .viewport
                .update(cx, |v, _| v.set_zoom_anchor_pointer(at_pointer));
        }
        if self.ocr.languages != config.ocr.languages {
            let options = es_ocr::OcrOptions {
                languages: config.ocr.languages.clone(),
                ..Default::default()
            };
            self.set_ocr_options(options, cx);
        }
    }

    pub fn set_low_res(&mut self, low_res: bool, cx: &mut Context<Self>) {
        self.low_res = low_res;
        self.low_res_expanded &= low_res;
        cx.notify();
    }

    pub fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    pub fn progress(&self, job: Job) -> Progress {
        self.action(job).progress
    }

    pub fn is_busy(&self) -> bool {
        [Job::Save, Job::Export, Job::Copy]
            .into_iter()
            .any(|job| self.progress(job) == Progress::Busy)
    }

    pub fn can_run(&self, job: Job, cx: &gpui::App) -> bool {
        let Some(session) = self.session() else {
            return false;
        };
        let empty = job == Job::Export && session.doc.read(cx).image_size().is_zero_area();
        !self.is_busy() && !empty
    }

    /// Starts `job` and emits its event; returns `false` when the job cannot start now.
    pub fn request(&mut self, job: Job, cx: &mut Context<Self>) -> bool {
        if !self.can_run(job, cx) {
            return false;
        }
        *self.action_mut(job) = Action {
            progress: Progress::Busy,
            _reset: None,
        };
        self.error = None;
        cx.emit(EditorWindowEvent::from(job));
        cx.notify();
        true
    }

    pub fn finish_save(&mut self, result: std::result::Result<(), String>, cx: &mut Context<Self>) {
        self.finish(Job::Save, result, cx);
    }

    pub fn finish_export(
        &mut self,
        result: std::result::Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.finish(Job::Export, result, cx);
    }

    pub fn finish_copy(&mut self, result: std::result::Result<(), String>, cx: &mut Context<Self>) {
        self.finish(Job::Copy, result, cx);
    }

    pub fn finish(
        &mut self,
        job: Job,
        result: std::result::Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if self.progress(job) != Progress::Busy {
            return;
        }
        let action = match result {
            Ok(()) if job == Job::Export => Action::default(),
            Ok(()) => Action {
                progress: Progress::Done,
                _reset: Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(FLASH).await;
                    let _ = this.update(cx, |w, cx| {
                        w.action_mut(job).progress = Progress::Idle;
                        cx.notify();
                    });
                })),
            },
            Err(error) => {
                self.error = Some(failure(job, error).into());
                Action::default()
            }
        };
        *self.action_mut(job) = action;
        cx.notify();
    }

    fn action(&self, job: Job) -> &Action {
        match job {
            Job::Save => &self.save,
            Job::Export => &self.export,
            Job::Copy => &self.copy,
        }
    }

    fn action_mut(&mut self, job: Job) -> &mut Action {
        match job {
            Job::Save => &mut self.save,
            Job::Export => &mut self.export,
            Job::Copy => &mut self.copy,
        }
    }

    fn tool_row(&self, t: &Tokens) -> impl IntoElement {
        div()
            .id("tool-row")
            .flex()
            .flex_none()
            .items_center()
            .h(TOOL_ROW_HEIGHT)
            .px(CHROME_PADDING_X)
            .gap(px(4.))
            .bg(t.window)
            .border_b(t.metrics.hairline)
            .border_color(t.border)
            .accessible(Role::Toolbar, fl!("editor-tool-row-a11y"))
            .children(self.tool_row.clone())
    }

    fn inspector_column(&self, t: &Tokens) -> impl IntoElement {
        div()
            .id("inspector")
            .flex_none()
            .w(INSPECTOR_WIDTH)
            .h_full()
            .overflow_hidden()
            .bg(t.inspector)
            .border_l(t.metrics.hairline)
            .border_color(t.border)
            .accessible(Role::Pane, fl!("editor-inspector-a11y"))
            .children(self.inspector.clone())
    }

    fn workspace(&self, t: &Tokens) -> AnyElement {
        let area = div()
            .id("workspace")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .bg(t.workspace);
        match &self.content {
            Content::Loading => area.child(states::loading(t)).into_any_element(),
            Content::Failed => area.child(states::failed(t)).into_any_element(),
            Content::Ready { canvas, .. } => area
                .child(canvas.clone())
                .when_some(self.error.clone(), |d, error| {
                    d.child(states::error_strip(error, t))
                })
                .into_any_element(),
        }
    }
}

fn failure(job: Job, error: String) -> String {
    match job {
        Job::Save => fl!("editor-error-save", error = error),
        Job::Export => fl!("editor-error-export", error = error),
        Job::Copy => fl!("editor-error-copy", error = error),
    }
}

/// The same footprint as an editor button, for the busy and done looks that replace one.
fn button_shell(id: &'static str, t: &Tokens) -> Stateful<Div> {
    let m = t.metrics;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .h(m.button_height)
        .min_w(m.button_height)
        .px(m.button_padding_x)
        .rounded(m.button_radius)
        .typeset(t.type_scale.tool_button)
}

pub(crate) fn busy(id: &'static str, label: String, prominent: bool, t: &Tokens) -> AnyElement {
    let colour = if prominent { t.on_accent } else { t.text };
    button_shell(id, t)
        .when(prominent, |d| d.bg(t.accent.opacity(0.8)))
        .accessible(Role::ProgressIndicator, label)
        .child(Spinner::new().small().color(colour))
        .into_any_element()
}

pub(crate) fn done(id: &'static str, label: String, t: &Tokens) -> AnyElement {
    button_shell(id, t)
        .text_color(t.success)
        .accessible(Role::Status, label.clone())
        .child(Icon::new(IconName::CircleCheck).size(px(16.)).flex_none())
        .child(label)
        .into_any_element()
}

impl Render for EditorWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let title = self.title(cx);
        if title != self.shown_title {
            window.set_window_title(&title);
            self.shown_title = title;
        }
        if self.focus_canvas
            && let Some(canvas) = self.canvas()
        {
            let focus = canvas.read(cx).focus_handle().clone();
            window.focus(&focus, cx);
            self.focus_canvas = false;
        }
        if !self.guard_hooked {
            self.guard_hooked = true;
            let this = cx.entity();
            window.on_window_should_close(cx, move |window, cx| {
                this.update(cx, |w, cx| w.should_close(window, cx))
            });
        }
        let dialog = self.close_dialog(&t, cx);
        div()
            .id("editor-window")
            .key_context(crate::keyboard::CONTEXT)
            .on_action(cx.listener(|w, a: &crate::keyboard::Run, window, cx| {
                if !w.run_command(a.0, window, cx) {
                    cx.propagate();
                }
            }))
            .on_key_down(cx.listener(|w, e, _, cx| w.on_editor_key(e, cx)))
            .on_key_up(cx.listener(|w, _, _, _| w.held = None))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .text_color(t.text)
            .font_family(UI_FONT)
            .child(self.title_bar(cx))
            .child(self.tool_row(&t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.workspace(&t))
                    .when(self.inspector_open && self.session().is_some(), |d| {
                        d.child(self.inspector_column(&t))
                    }),
            )
            .child(self.footer(cx))
            .children(self.redact_toast_view(&t, cx))
            .children(dialog)
    }
}
