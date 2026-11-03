mod crop_bar;
mod preview;
pub mod redact;
mod states;
mod title_bar;

pub(crate) use states::{busy, done};

use std::time::Duration;

use es_ui::{
    a11y::{Accessible, Role},
    components::ZoomCommand,
    motion::{PAGE_CHANGE, PAGE_RISE, since_change},
    theme::{UI_FONT, tokens},
    tokens::Tokens,
};
use gpui::{
    AnyElement, AnyView, AppContext, Context, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Styled, Subscription, Task, Window,
    div, prelude::FluentBuilder, px,
};

use crate::inspector::{
    Inspector,
    options::{ToolOptions, ToolOptionsEvent},
};
use crate::session::crop::CropEvent;
use crate::toolbar::{ToolRow, ToolRowEvent};
use crate::{EditorCanvas, EditorSession, EditorSource, Result, strings::fl};

pub const TITLE_BAR_HEIGHT: Pixels = px(38.);
pub const TOOL_ROW_HEIGHT: Pixels = px(48.);
pub const FOOTER_HEIGHT: Pixels = px(52.);
pub const INSPECTOR_WIDTH: Pixels = px(300.);
pub const CHROME_PADDING_X: Pixels = px(16.);
pub const FLASH: Duration = es_ui::motion::FLASH_HOLD;

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
        session: Box<EditorSession>,
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
    tool_options: Option<Entity<ToolOptions>>,
    inspector_open: bool,
    pub(crate) low_res: bool,
    pub(crate) low_res_expanded: bool,
    force_low_res: bool,
    preview_request: Option<(u64, bool)>,
    _preview: Option<Task<()>>,
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
            tool_options: None,
            inspector_open: true,
            low_res: false,
            low_res_expanded: false,
            force_low_res: false,
            preview_request: None,
            _preview: None,
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
                let options = cx.new(|cx| ToolOptions::new(&session, cx));
                self._subscriptions = vec![
                    cx.observe(&session.doc, |this, _, cx| {
                        cx.notify();
                        this.refresh_preview(cx);
                    }),
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
                    cx.observe(&options, |_, _, cx| cx.notify()),
                    cx.subscribe(&options, {
                        let row = tool_row.clone();
                        move |this, _, _: &ToolOptionsEvent, cx| {
                            if !this.inspector_open {
                                this.toggle_inspector(cx);
                                row.update(cx, |r, cx| r.set_inspector_open(true, cx));
                            }
                        }
                    }),
                    cx.subscribe(&tool_row, |this, row, event: &ToolRowEvent, cx| {
                        if *event == ToolRowEvent::ShowOptions {
                            if let Some(options) = this.tool_options.clone() {
                                options.update(cx, |o, cx| o.show(cx));
                            }
                        } else if *event == ToolRowEvent::ToggleInspector {
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
                self.tool_options = Some(options);
                self.probe_ocr(cx);
                self.content = Content::Ready {
                    session: Box::new(session),
                    canvas,
                };
                self.focus_canvas = true;
                self.preview_request = None;
                self.refresh_preview(cx);
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

    pub fn tool_options(&self) -> Option<Entity<ToolOptions>> {
        self.tool_options.clone()
    }

    /// Applies the `[editor]` zoom anchor, the `[ocr]` languages and the low-resolution preview setting to this window.
    pub fn apply_config(&mut self, config: &es_model::Config, cx: &mut Context<Self>) {
        if self.force_low_res != config.general.low_res_preview {
            self.force_low_res = config.general.low_res_preview;
            self.refresh_preview(cx);
        }
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

    pub fn is_low_res(&self) -> bool {
        self.low_res
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

    /// The tool's options replace the usual inspector while they are up; the page rises in as Settings pages do.
    fn inspector_column(
        &self,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let page: Option<AnyView> = self
            .tool_options
            .clone()
            .filter(|o| o.read(cx).shows_page(cx))
            .map(Into::into);
        let tool_page = page.is_some();
        let arrived = since_change("inspector-page", tool_page, PAGE_CHANGE, window, cx);
        let shown = page.or_else(|| self.inspector.clone());
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
            .child(
                div()
                    .relative()
                    .size_full()
                    .top(PAGE_RISE * (1.0 - arrived))
                    .opacity(arrived)
                    .children(shown),
            )
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
                        d.child(self.inspector_column(&t, window, cx))
                    }),
            )
            .child(self.footer(cx))
            .children(self.redact_toast_view(&t, cx))
            .children(dialog)
    }
}
