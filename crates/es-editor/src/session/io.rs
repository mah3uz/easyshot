use crate::inspector::background::wallpaper_cache::wallpaper_bytes;
use crate::session::doc::{DocEvent, DocModel};
use crate::session::journal::{self, Entry, Journal};
use crate::strings::fl;
use crate::window::{EditorWindow, Job};
use es_anno::{BuildContext, DrawList, build_draw_list};
use es_app::platform_ui::ClipboardSink;
use es_app::services::Services;
use es_model::platform::ClipItem;
use es_model::{AnnotationDocument, BackgroundSettings, BackgroundStyle, Config};
use es_render::encode::OutputFormat;
use es_render::export::{Purpose, export};
use es_store::atomic::write_atomic;
use es_store::export::jpeg_quality_percent;
use es_store::naming::{self, exists, unique_path};
use es_store::{Commit, ExportFormat, Paths};
use futures::channel::oneshot;
use gpui::{App, Context, Entity, Subscription, Task};
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Weak};
use time::OffsetDateTime;
use tiny_skia::{IntSize, Pixmap};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct IoSettings {
    pub save_dir: PathBuf,
    pub template: String,
    pub format: ExportFormat,
    pub quality: f64,
}

impl IoSettings {
    pub fn from_config(config: &Config) -> Option<Self> {
        Some(Self {
            save_dir: es_store::paths::save_dir(config)?,
            template: config.general.file_name_template.clone(),
            format: ExportFormat::from_setting(&config.general.export_format),
            quality: config.general.export_quality,
        })
    }
}

pub struct ChooseRequest {
    pub title: String,
    pub folder: PathBuf,
    pub suggested_name: String,
    pub format: ExportFormat,
}

/// `None` when the user dismisses the chooser.
pub trait ExportChooser {
    fn choose(&self, request: ChooseRequest, cx: &mut App)
    -> Task<Result<Option<PathBuf>, String>>;
}

/// What the app hands the editor; anything left out falls back to the app's services.
#[derive(Default)]
pub struct IoConfig {
    pub paths: Option<Paths>,
    pub settings: Option<IoSettings>,
    pub chooser: Option<Rc<dyn ExportChooser>>,
    pub clipboard: Option<Rc<dyn ClipboardSink>>,
}

struct Frame {
    base: Arc<RgbaImage>,
    draw: DrawList,
    look: BackgroundSettings,
}

impl Frame {
    fn of(doc: &DocModel) -> Self {
        Self {
            base: doc.base().clone(),
            draw: build_draw_list(doc.editor().doc(), &BuildContext::new(&**doc.text())),
            look: doc.background().clone(),
        }
    }

    fn encode(
        &self,
        format: ExportFormat,
        purpose: Purpose,
        quality: f64,
    ) -> Result<Vec<u8>, String> {
        let (w, h) = self.base.dimensions();
        let mut data = self.base.as_raw().clone();
        for px in data.as_chunks_mut::<4>().0 {
            let a = u16::from(px[3]);
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
        let size = IntSize::from_wh(w, h).ok_or_else(|| "the image has no pixels".to_owned())?;
        let base =
            Pixmap::from_vec(data, size).ok_or_else(|| "the image has no pixels".to_owned())?;
        let output = match format {
            ExportFormat::Png => OutputFormat::Png(purpose.png_profile()),
            ExportFormat::Jpeg => OutputFormat::Jpeg {
                quality_percent: jpeg_quality_percent(quality),
            },
        };
        let wallpaper = match &self.look.style {
            BackgroundStyle::Wallpaper { path } => wallpaper_bytes(path),
            _ => None,
        };
        export(&base, &self.draw, &self.look, wallpaper.as_deref(), output)
            .map_err(|e| e.to_string())
    }
}

pub struct EditorIo {
    doc: Entity<DocModel>,
    session: String,
    config: IoConfig,
    baseline: AnnotationDocument,
    baseline_look: BackgroundSettings,
    /// Weak keeps the allocation alive, so no later image can reuse its address.
    disk_base: Weak<RgbaImage>,
    committed: Option<PathBuf>,
    exported: Option<PathBuf>,
    saving: bool,
    save_waiters: Vec<oneshot::Sender<bool>>,
    prompt: Option<oneshot::Sender<Choice>>,
    journal_timer: Option<Task<()>>,
    journal: Journal,
    gestures: u64,
    _doc_events: Subscription,
}

impl EditorIo {
    pub fn new(doc: Entity<DocModel>, cx: &mut Context<Self>) -> Self {
        let (baseline, baseline_look, disk_base) = {
            let d = doc.read(cx);
            (
                d.editor().doc().model().clone(),
                d.background().clone(),
                Arc::downgrade(d.base()),
            )
        };
        let events = cx.subscribe(&doc, |this, _, event: &DocEvent, cx| {
            if *event == DocEvent::UndoChanged {
                this.schedule_journal(cx);
            }
        });
        Self {
            doc,
            session: Uuid::new_v4().to_string(),
            config: IoConfig::default(),
            baseline,
            baseline_look,
            disk_base,
            committed: None,
            exported: None,
            saving: false,
            save_waiters: Vec::new(),
            prompt: None,
            journal_timer: None,
            journal: Journal::default(),
            gestures: 0,
            _doc_events: events,
        }
    }

    pub fn configure(&mut self, config: IoConfig) {
        self.config = config;
    }

    /// Continues a session found in the recovery directory: same journal, and the edits count as
    /// unsaved. `staged` is the journal's staged base the editor opened, if it opened one.
    pub fn adopt_recovery(
        &mut self,
        session: String,
        baseline: AnnotationDocument,
        staged: Option<PathBuf>,
        cx: &App,
    ) {
        self.session = session;
        self.baseline = baseline;
        if let Some(file) = staged {
            self.disk_base = Weak::new();
            self.journal.adopt(self.doc.read(cx).base_revision(), file);
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        let d = self.doc.read(cx);
        d.editor().doc().model() != &self.baseline
            || d.background() != &self.baseline_look
            || !self.base_on_disk(d)
    }

    /// Whether the base is the one in the file on disk, as after undoing every crop.
    fn base_on_disk(&self, d: &DocModel) -> bool {
        std::ptr::eq(self.disk_base.as_ptr(), Arc::as_ptr(d.base()))
    }

    /// False when nothing is configured for `job`, so an owner outside the editor may do it instead.
    pub fn serves(&self, job: Job, cx: &App) -> bool {
        match job {
            Job::Save => self.paths(cx).is_some() && self.settings(cx).is_some(),
            Job::Export => self.config.chooser.is_some() && self.settings(cx).is_some(),
            Job::Copy => self.config.clipboard.is_some(),
        }
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn prompt_open(&self) -> bool {
        self.prompt.is_some()
    }

    pub(crate) fn next_gesture(&mut self) -> es_anno::GestureId {
        self.gestures += 1;
        es_anno::GestureId(1_000_000 + self.gestures)
    }

    fn paths(&self, cx: &App) -> Option<Paths> {
        self.config
            .paths
            .clone()
            .or_else(|| cx.try_global::<Services>().map(|s| s.stores.paths.clone()))
    }

    fn settings(&self, cx: &App) -> Option<IoSettings> {
        self.config.settings.clone().or_else(|| {
            let services = cx.try_global::<Services>()?;
            IoSettings::from_config(services.settings.read(cx).config())
        })
    }

    fn schedule_journal(&mut self, cx: &mut Context<Self>) {
        self.journal_timer = Some(cx.spawn(async move |io, cx| {
            cx.background_executor().timer(journal::DELAY).await;
            let _ = io.update(cx, |io, cx| io.flush_journal(cx));
        }));
    }

    /// Writes the journal now when the editor holds unsaved edits, and removes it when it does not.
    pub fn flush_journal(&mut self, cx: &mut Context<Self>) {
        let Some(paths) = self.paths(cx) else {
            return;
        };
        if !self.is_dirty(cx) {
            self.journal.remove(&paths, &self.session);
            return;
        }
        let d = self.doc.read(cx);
        let current = self
            .committed
            .clone()
            .unwrap_or_else(|| d.display().clone());
        let entry = Entry {
            base_image: es_store::reopen(&current).source,
            display: current,
            base_revision: d.base_revision(),
            document: d.editor().doc().model().clone(),
            staged_base: None,
        };
        let edited = (!self.base_on_disk(d)).then(|| d.base().clone());
        if let Some(staging) = self.journal.write(&paths, &self.session, entry, edited) {
            cx.background_executor()
                .spawn(async move { staging.run() })
                .detach();
        }
    }

    pub fn drop_journal(&mut self, cx: &App) {
        self.journal_timer = None;
        if let Some(paths) = self.paths(cx) {
            self.journal.remove(&paths, &self.session);
        }
    }

    pub fn save(&mut self, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let (Some(paths), Some(settings)) = (self.paths(cx), self.settings(cx)) else {
            return Task::ready(Err(fl!("editor-error-unavailable")));
        };
        let d = self.doc.read(cx);
        let frame = Frame::of(d);
        let baseline = d.editor().doc().model().clone();
        let mut document = baseline.clone();
        document.background = d.background().clone();
        let look = d.background().clone();
        let saved_base = Arc::downgrade(d.base());
        let base_pixels = (!self.base_on_disk(d)).then(|| d.base().clone());
        let display = self
            .committed
            .clone()
            .unwrap_or_else(|| d.display().clone());
        let exported = self.exported.clone();
        let session = self.session.clone();
        self.saving = true;
        let work = cx.background_executor().spawn({
            async move {
                let png = frame.encode(ExportFormat::Png, Purpose::Keep, settings.quality)?;
                let base_file = match base_pixels {
                    Some(pixels) => {
                        let file = paths.recovery_dir().join(format!("{session}-base.png"));
                        let mut out = std::io::Cursor::new(Vec::new());
                        pixels
                            .write_to(&mut out, image::ImageFormat::Png)
                            .map_err(|e| e.to_string())?;
                        write_atomic(&file, out.get_ref()).map_err(|e| e.to_string())?;
                        Some(file)
                    }
                    None => None,
                };
                let source = es_store::reopen(&display).source;
                let committed = es_store::commit(
                    &paths,
                    Commit {
                        display: &display,
                        base: base_file.as_deref().unwrap_or(&source),
                        rendered_png: &png,
                        document,
                    },
                )
                .map_err(|e| e.to_string());
                if let Some(file) = &base_file {
                    let _ = std::fs::remove_file(file);
                }
                let committed = committed?;
                let flat = match settings.format {
                    ExportFormat::Png => png,
                    ExportFormat::Jpeg => {
                        frame.encode(ExportFormat::Jpeg, Purpose::Keep, settings.quality)?
                    }
                };
                let target = exported.unwrap_or_else(|| {
                    let name = naming::file_name(
                        &settings.template,
                        settings.format.extension(),
                        &naming::Context::new(OffsetDateTime::now_utc()),
                    );
                    unique_path(&name, &settings.save_dir, "-", exists)
                });
                write_atomic(&target, &flat).map_err(|e| e.to_string())?;
                Ok::<_, String>((committed, target))
            }
        });
        cx.spawn(async move |io, cx| {
            let outcome = work.await;
            io.update(cx, |io, cx| {
                io.saving = false;
                let ok = outcome.is_ok();
                if let Ok((committed, target)) = &outcome {
                    io.baseline = baseline;
                    io.baseline_look = look;
                    io.disk_base = saved_base;
                    io.committed = Some(committed.clone());
                    io.exported = Some(target.clone());
                    io.drop_journal(cx);
                }
                for waiter in io.save_waiters.drain(..) {
                    let _ = waiter.send(ok);
                }
                outcome.map(|_| ())
            })
            .unwrap_or_else(|e| Err(e.to_string()))
        })
    }

    /// Resolves with the outcome of the save in flight, or `None` when none is.
    pub fn wait_for_save(&mut self) -> Option<oneshot::Receiver<bool>> {
        self.saving.then(|| {
            let (tx, rx) = oneshot::channel();
            self.save_waiters.push(tx);
            rx
        })
    }

    pub fn export(&mut self, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let (Some(chooser), Some(settings)) = (self.config.chooser.clone(), self.settings(cx))
        else {
            return Task::ready(Err(fl!("editor-error-unavailable")));
        };
        let d = self.doc.read(cx);
        let frame = Frame::of(d);
        let stem = d.display().file_stem().map_or_else(
            || "screenshot".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        let request = ChooseRequest {
            title: fl!("editor-save-panel-title"),
            folder: settings.save_dir.clone(),
            suggested_name: format!("{stem}.{}", settings.format.extension()),
            format: settings.format,
        };
        let chosen = chooser.choose(request, cx);
        cx.spawn(async move |_, cx| {
            let Some(path) = chosen.await? else {
                return Ok(());
            };
            let format = path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(ExportFormat::from_extension)
                .unwrap_or(settings.format);
            cx.background_executor()
                .spawn(async move {
                    let bytes = frame.encode(format, Purpose::Keep, settings.quality)?;
                    write_file(&path, &bytes)
                })
                .await
        })
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(sink) = self.config.clipboard.clone() else {
            return Task::ready(Err(fl!("editor-error-unavailable")));
        };
        let frame = Frame::of(self.doc.read(cx));
        let encoding = cx
            .background_executor()
            .spawn(async move { frame.encode(ExportFormat::Png, Purpose::Transient, 1.0) });
        cx.spawn(async move |_, cx| {
            let png = encoding.await?;
            let copied = cx.update(|cx| sink.copy(vec![ClipItem::Png(png.into())], cx));
            copied.await.map_err(|e| e.to_string())
        })
    }

    /// The image as edited, through the export renderer, to a PNG in the share folder.
    pub fn render_for_share(&mut self, cx: &mut Context<Self>) -> Task<Result<PathBuf, String>> {
        let Some(paths) = self.paths(cx) else {
            return Task::ready(Err(fl!("editor-error-unavailable")));
        };
        let frame = Frame::of(self.doc.read(cx));
        let file = paths
            .share_dir()
            .join(format!("edited-{}.png", self.session));
        cx.background_executor().spawn(async move {
            let png = frame.encode(ExportFormat::Png, Purpose::Keep, 1.0)?;
            write_file(&file, &png)?;
            Ok(file)
        })
    }

    /// Opens the close prompt; the receiver resolves with the user's choice.
    pub fn open_prompt(&mut self, cx: &mut Context<Self>) -> oneshot::Receiver<Choice> {
        let (tx, rx) = oneshot::channel();
        self.prompt = Some(tx);
        cx.notify();
        rx
    }

    pub fn choose(&mut self, choice: Choice, cx: &mut Context<Self>) {
        if let Some(tx) = self.prompt.take() {
            let _ = tx.send(choice);
            cx.notify();
        }
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic(path, bytes).map_err(|e| e.to_string())
}

impl EditorWindow {
    /// Runs one of the window's own events: the work happens in the session's `EditorIo`.
    pub(crate) fn run_job(&mut self, job: Job, cx: &mut Context<Self>) {
        let Some(session) = self.session() else {
            return;
        };
        let io = session.io.clone();
        if !io.read(cx).serves(job, cx) {
            return;
        }
        let task = io.update(cx, |io, cx| match job {
            Job::Save => io.save(cx),
            Job::Export => io.export(cx),
            Job::Copy => io.copy(cx),
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |w, cx| w.finish(job, result, cx));
        })
        .detach();
    }
}
