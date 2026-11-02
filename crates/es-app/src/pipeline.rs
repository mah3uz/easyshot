use crate::capture_coordinator::{CaptureKind, CaptureRequest, Direct, DirectCapture, Pipeline};
use crate::deck::{Deck, DeckItem};
use crate::errors::{self, Error, Op, Presentation, Result};
use crate::history::History;
use crate::jobs::run_cpu;
use crate::platform_ui::{ClipboardSink, SelectMode, Selection, SelectionFlow};
use crate::settings_model::SettingsModel;
use crate::toast::{IconName, Toast, show_toast};
use es_anno::draw::DrawList;
use es_backend::wayland::normalise_frame;
use es_geom::Rgba;
use es_model::capture::{Step, after_capture_plan};
use es_model::platform::{ClipItem, Frame, Transform};
use es_model::{
    AnnotationDocument, BackgroundSettings, BackgroundStyle, CaptureKind as StoredKind, Config,
    HistoryEntry, resolve_new_capture_look,
};
use es_render::beautify::compose_with_wallpaper;
use es_render::beautify::wallpaper::decode_wallpaper;
use es_render::encode::{PngProfile, encode_jpeg, encode_png};
use es_store::atomic::write_atomic;
use es_store::naming::{self, Kind, exists, unique_path};
use es_store::temp::{self, TempKind};
use es_store::{ExportFormat, Paths, StageRequest};
use gpui::{App, Entity, Task};
use image::RgbaImage;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use time::OffsetDateTime;
use tiny_skia::{IntSize, Pixmap};

/// Twice the Large card width, so the card stays sharp at 2x.
pub const THUMB_SIZE: u32 = 600;

pub type Recognize = Arc<dyn Fn(&RgbaImage, &Config) -> Result<String> + Send + Sync>;
/// Takes the kept file and its pixels.
pub type Pinner = Rc<dyn Fn(PathBuf, Arc<RgbaImage>, &mut App) -> Result<()>>;
pub type EditorOpener = Rc<dyn Fn(PathBuf, &mut App) -> Result<()>>;

pub struct PipelineDeps {
    pub paths: Paths,
    pub settings: Entity<SettingsModel>,
    pub history: Entity<History>,
    pub deck: Entity<Deck>,
    pub clipboard: Rc<dyn ClipboardSink>,
    pub recognize: Recognize,
    pub pin: Pinner,
    pub open_editor: EditorOpener,
}

pub fn tesseract() -> Recognize {
    Arc::new(|image, cfg| {
        let options = es_ocr::OcrOptions {
            languages: cfg.ocr.languages.clone(),
            ..es_ocr::OcrOptions::default()
        };
        let found = es_ocr::recognize(image, &options).map_err(|e| match e {
            es_ocr::Error::Cancelled => Error::Cancelled,
            other => Error::Protocol(other.to_string()),
        })?;
        Ok(es_ocr::join_text(&found.codes, &found.lines))
    })
}

pub fn pipeline(deps: Rc<PipelineDeps>) -> Pipeline {
    Rc::new(move |request, selection, cx| run(&deps, request, selection, cx))
}

/// Shows its own failures, then returns the first so the waiting caller sees it too.
pub fn run(
    deps: &Rc<PipelineDeps>,
    request: CaptureRequest,
    selection: Selection,
    cx: &mut App,
) -> Task<Result<()>> {
    let frame = match selection {
        Selection::Cancelled => return Task::ready(Err(Error::Cancelled)),
        Selection::Color(color) => return copy_color(deps, color, cx),
        Selection::Region { frame, .. } | Selection::Window { frame, .. } => frame,
    };
    match request.kind {
        CaptureKind::Text | CaptureKind::TextLine => read_text(deps, request.kind, frame, cx),
        CaptureKind::Recording | CaptureKind::RecordingArea => {
            tracing::error!(kind = ?request.kind, "recording is not wired into the daemon");
            Task::ready(Err(fail(Error::Unsupported, Op::Recording, cx)))
        }
        kind => screenshot(deps, request, stored_kind(kind), frame, cx),
    }
}

fn stored_kind(kind: CaptureKind) -> StoredKind {
    match kind {
        CaptureKind::Fullscreen => StoredKind::Fullscreen,
        CaptureKind::Window => StoredKind::Window,
        CaptureKind::Scroll => StoredKind::Scroll,
        _ => StoredKind::Region,
    }
}

fn fail(error: Error, op: Op, cx: &mut App) -> Error {
    if let Presentation::Toast(toast) = errors::present(&error, errors::Context::toast(op)) {
        show_toast(toast, cx);
    }
    error
}

pub fn hex(color: Rgba) -> String {
    let byte = |c: f32| (c.clamp(0., 1.) * 255.).round() as u8;
    format!(
        "#{:02X}{:02X}{:02X}",
        byte(color.r),
        byte(color.g),
        byte(color.b)
    )
}

fn copy_color(deps: &Rc<PipelineDeps>, color: Rgba, cx: &mut App) -> Task<Result<()>> {
    let hex = hex(color);
    let copied = deps.clipboard.copy(vec![ClipItem::Text(hex.clone())], cx);
    cx.spawn(async move |cx| {
        let result = copied.await;
        cx.update(|cx| match result {
            Ok(()) => {
                show_toast(
                    Toast::new("toast-copied-color", IconName::Copy).arg("hex", hex),
                    cx,
                );
                Ok(())
            }
            Err(e) => Err(fail(e, Op::Copy, cx)),
        })
    })
}

/// Opaque RGBA, turned upright by the output's transform; compositors hand out XRGB whose alpha
/// byte is undefined.
pub fn frame_rgba(frame: &Frame) -> Result<RgbaImage> {
    let upright;
    let frame = if frame.transform == Transform::Normal {
        frame
    } else {
        upright = normalise_frame(frame.clone());
        &upright
    };
    let (w, h) = frame.size_px;
    let len = w as usize * h as usize * 4;
    let bgra = frame
        .bgra
        .get(..len)
        .ok_or_else(|| Error::Internal("frame is shorter than its size".into()))?;
    let mut rgba = Vec::with_capacity(len);
    for px in bgra.as_chunks::<4>().0 {
        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    RgbaImage::from_raw(w, h, rgba).ok_or_else(|| Error::Internal("empty frame".into()))
}

pub(crate) fn pixmap(image: &RgbaImage) -> Result<Pixmap> {
    let size = IntSize::from_wh(image.width(), image.height())
        .ok_or_else(|| Error::Internal("empty frame".into()))?;
    Pixmap::from_vec(image.as_raw().clone(), size)
        .ok_or_else(|| Error::Internal("frame does not fit a pixmap".into()))
}

fn encoding(e: &es_render::encode::EncodeError) -> Error {
    Error::Internal(e.to_string())
}

fn read_text(
    deps: &Rc<PipelineDeps>,
    kind: CaptureKind,
    frame: Arc<Frame>,
    cx: &mut App,
) -> Task<Result<()>> {
    let config = deps.settings.read(cx).config().clone();
    let recognize = deps.recognize.clone();
    let work = run_cpu(cx, move || recognize(&frame_rgba(&frame)?, &config));
    let deps = deps.clone();
    cx.spawn(async move |cx| {
        let text = match work.await.and_then(|r| r) {
            Ok(text) => text,
            Err(e) => return Err(cx.update(|cx| fail(e, Op::Ocr, cx))),
        };
        let text = match kind {
            CaptureKind::TextLine => text.split_whitespace().collect::<Vec<_>>().join(" "),
            _ => text,
        };
        if text.trim().is_empty() {
            cx.update(|cx| show_toast(Toast::new("toast-ocr-empty", IconName::Info), cx));
            return Ok(());
        }
        let copied = cx.update(|cx| deps.clipboard.copy(vec![ClipItem::Text(text)], cx));
        let result = copied.await;
        cx.update(|cx| match result {
            Ok(()) => {
                show_toast(Toast::new("toast-copied-text", IconName::Copy), cx);
                Ok(())
            }
            Err(e) => Err(fail(e, Op::Copy, cx)),
        })
    })
}

struct Prepared {
    /// Present when the plan shows a card.
    card: Option<DeckItem>,
    /// Where a capture that is shown, saved and kept nowhere lives until its pin or editor is done.
    transient: Option<PathBuf>,
    png: Arc<[u8]>,
    image: Arc<RgbaImage>,
    saved: Option<Result<PathBuf>>,
    library: Option<Result<HistoryEntry>>,
}

pub(crate) struct Work {
    pub(crate) paths: Paths,
    pub(crate) config: Arc<Config>,
    pub(crate) file_stem: String,
    pub(crate) kind: StoredKind,
    pub(crate) save: bool,
    pub(crate) show: bool,
}

pub(crate) fn new_capture_look(paths: &Paths, config: &Config) -> BackgroundSettings {
    let presets = es_store::presets::open_backgrounds(paths)
        .map(|f| f.value)
        .unwrap_or_default();
    resolve_new_capture_look(config, &presets)
}

/// The capture inside its look, or `None` when the look draws nothing.
pub(crate) fn compose(raw: &Pixmap, look: &BackgroundSettings) -> Result<Option<Arc<Pixmap>>> {
    if !look.has_renderable_content() {
        return Ok(None);
    }
    let wallpaper = match &look.style {
        BackgroundStyle::Wallpaper { path } => {
            es_store::wallpapers::read(path).and_then(|bytes| decode_wallpaper(&bytes))
        }
        _ => None,
    };
    compose_with_wallpaper(raw, &DrawList::default(), look, wallpaper.as_ref())
        .map(|pixmap| Some(Arc::new(pixmap)))
        .ok_or_else(|| Error::Internal("the composed image could not be allocated".into()))
}

pub(crate) fn save_to_folder(work: &Work, pixmap: &Pixmap) -> Result<PathBuf> {
    let cfg = &work.config.general;
    let dir = es_store::paths::save_dir(&work.config)
        .ok_or_else(|| Error::NotFound("a save folder".into()))?;
    let format = ExportFormat::from_setting(&cfg.export_format);
    let bytes = match format {
        ExportFormat::Png => encode_png(pixmap, PngProfile::Small),
        ExportFormat::Jpeg => encode_jpeg(
            pixmap,
            es_store::export::jpeg_quality_percent(cfg.export_quality),
        ),
    }
    .map_err(|e| encoding(&e))?;
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    let name = format!("{}.{}", work.file_stem, format.extension());
    let target = unique_path(&name, &dir, "-", exists);
    write_atomic(&target, &bytes)?;
    Ok(target)
}

/// Adds the capture to the library; `look` carries the raw pixels and the look applied to them, so
/// the editor reopens it with the look still editable.
pub(crate) fn retain(
    work: &Work,
    png: &[u8],
    look: Option<(&[u8], BackgroundSettings)>,
    saved: Option<&PathBuf>,
) -> Result<HistoryEntry> {
    let edit = look.map(|(raw, background)| {
        (
            raw,
            AnnotationDocument {
                background,
                ..AnnotationDocument::default()
            },
        )
    });
    let target = es_store::commit_new(&work.paths, &format!("{}.png", work.file_stem), png, edit)?;
    let mut entry = es_store::library::entry_for_file(&target)
        .ok_or_else(|| Error::Store(format!("unreadable capture {}", target.display())))?;
    entry.kind = work.kind;
    entry.export_path = saved.cloned();
    Ok(entry)
}

fn prepare(work: &Work, frame: &Frame) -> Result<Prepared> {
    let image = frame_rgba(frame)?;
    let raw = pixmap(&image)?;
    let raw_png = encode_png(&raw, PngProfile::Fast).map_err(|e| encoding(&e))?;
    let look = new_capture_look(&work.paths, &work.config);
    let composed = compose(&raw, &look)?;
    let shown = composed.as_deref().unwrap_or(&raw);
    let png: Arc<[u8]> = match &composed {
        Some(p) => encode_png(p, PngProfile::Fast)
            .map_err(|e| encoding(&e))?
            .into(),
        None => raw_png.clone().into(),
    };
    let edit = composed.is_some().then_some((&raw_png[..], look));
    let saved = work.save.then(|| save_to_folder(work, shown));
    let library = (!work.config.capture.keep_in_deck_until_saved).then(|| {
        retain(
            work,
            &png,
            edit,
            saved.as_ref().and_then(|s| s.as_ref().ok()),
        )
    });
    let kept = matches!(library, Some(Ok(_))) || matches!(saved, Some(Ok(_)));
    let card = if work.show {
        let kept_card = library
            .as_ref()
            .and_then(|l| l.as_ref().ok())
            .and_then(|entry| library_card(work, entry));
        match kept_card {
            Some(card) => Some(card),
            None => Some(staged_card(work, &png, &raw_png)?),
        }
    } else {
        None
    };
    let transient = (card.is_none() && !kept)
        .then(|| {
            temp::create(
                &work.paths,
                TempKind::Unstaged,
                &format!("{}.png", work.file_stem),
                &png,
            )
        })
        .transpose()?;
    Ok(Prepared {
        card,
        transient,
        png,
        image: Arc::new(image),
        saved,
        library,
    })
}

fn staged_card(work: &Work, png: &[u8], raw_png: &[u8]) -> Result<DeckItem> {
    let manifest = es_store::Deck::new(&work.paths).stage(
        &StageRequest {
            file_name: &format!("{}.png", work.file_stem),
            kind: work.kind,
            history_id: None,
            image: png,
            raw: raw_png,
            thumb_size: THUMB_SIZE,
        },
        OffsetDateTime::now_utc(),
    )?;
    if let Err(e) = es_store::thumbs::ensure(&work.paths, &manifest.staged, THUMB_SIZE) {
        tracing::warn!(error = %e, "deck thumbnail");
    }
    Ok(DeckItem::staged(&manifest))
}

fn library_card(work: &Work, entry: &HistoryEntry) -> Option<DeckItem> {
    let thumb = es_store::thumbs::ensure(&work.paths, &entry.file, THUMB_SIZE)
        .map_err(|e| tracing::warn!(error = %e, "deck thumbnail"))
        .ok()?;
    Some(DeckItem {
        id: entry.id,
        name: entry.name.clone(),
        file: entry.file.clone(),
        thumb,
        kind: entry.kind,
        video: false,
        staged: false,
        status: crate::deck::ItemStatus::Idle,
        auto_dismiss: true,
    })
}

/// Spends a `{counter}` number only when the template uses one; state is written on the UI thread only.
fn next_stem(paths: &Paths, config: &Config) -> String {
    let template = &config.general.file_name_template;
    let now = OffsetDateTime::now_utc();
    let mut state = match es_store::state::open(paths) {
        Ok(state) => Some(state),
        Err(e) => {
            tracing::warn!(error = %e, "state file");
            None
        }
    };
    let mut scratch = es_model::State::default();
    let target = state.as_mut().map_or(&mut scratch, |s| &mut s.value);
    let name = naming::next_file_name(template, "", Kind::Screenshot, now, target);
    if naming::uses_counter(template)
        && let Some(state) = &state
        && let Err(e) = state.save()
    {
        tracing::warn!(error = %e, "state file");
    }
    name
}

fn screenshot(
    deps: &Rc<PipelineDeps>,
    request: CaptureRequest,
    kind: StoredKind,
    frame: Arc<Frame>,
    cx: &mut App,
) -> Task<Result<()>> {
    let config = deps.settings.read(cx).config().clone();
    let plan = after_capture_plan(request.variant, &config);
    let work = Work {
        paths: deps.paths.clone(),
        file_stem: next_stem(&deps.paths, &config),
        kind,
        save: plan.contains(&Step::Save),
        show: plan.contains(&Step::ShowPreview),
        config,
    };
    let prepared = run_cpu(cx, move || prepare(&work, &frame));
    let deps = deps.clone();
    cx.spawn(async move |cx| {
        let prepared = match prepared.await.and_then(|r| r) {
            Ok(p) => p,
            Err(e) => return Err(cx.update(|cx| fail(e, Op::Capture, cx))),
        };
        finish(&deps, &plan, prepared, cx).await
    })
}

async fn finish(
    deps: &Rc<PipelineDeps>,
    plan: &[Step],
    p: Prepared,
    cx: &mut gpui::AsyncApp,
) -> Result<()> {
    let mut first: Option<Error> = None;
    let mut keep = |e: Error| {
        if first.is_none() {
            first = Some(e);
        }
    };
    let saved = match p.saved {
        Some(Ok(path)) => Some(path),
        Some(Err(e)) => {
            keep(cx.update(|cx| fail(e, Op::Save, cx)));
            None
        }
        None => None,
    };
    let library = match p.library {
        Some(Ok(entry)) => {
            let file = entry.file.clone();
            let added = cx.update(|cx| deps.history.update(cx, |h, cx| h.add(entry, cx)));
            if let Err(e) = added {
                tracing::warn!(error = %e, "history");
            }
            Some(file)
        }
        Some(Err(e)) => {
            keep(cx.update(|cx| fail(e, Op::Save, cx)));
            None
        }
        None => None,
    };
    let kept: PathBuf = library
        .clone()
        .or_else(|| saved.clone())
        .or_else(|| p.card.as_ref().map(|c| c.file.clone()))
        .or_else(|| p.transient.clone())
        .unwrap_or_default();
    for step in plan {
        match step {
            Step::Save => {}
            Step::Copy => {
                let copied =
                    cx.update(|cx| deps.clipboard.copy(vec![ClipItem::Png(p.png.clone())], cx));
                match copied.await {
                    Ok(()) => cx.update(|cx| {
                        if !plan.contains(&Step::ShowPreview) {
                            show_toast(Toast::new("toast-copied", IconName::Copy), cx);
                        }
                    }),
                    Err(e) => keep(cx.update(|cx| fail(e, Op::Copy, cx))),
                }
            }
            Step::ShowPreview => {
                if let Some(item) = p.card.clone() {
                    cx.update(|cx| deps.deck.update(cx, |deck, cx| deck.show(item, cx)));
                }
            }
            Step::Pin => {
                let (file, image) = (kept.clone(), p.image.clone());
                if let Err(e) = cx.update(|cx| (deps.pin)(file, image, cx)) {
                    keep(cx.update(|cx| fail(e, Op::Other, cx)));
                }
            }
            Step::OpenEditor => {
                let file = kept.clone();
                if let Err(e) = cx.update(|cx| (deps.open_editor)(file, cx)) {
                    keep(cx.update(|cx| fail(e, Op::Open, cx)));
                }
            }
        }
    }
    if saved.is_some() && !plan.contains(&Step::ShowPreview) && !plan.contains(&Step::Copy) {
        cx.update(|cx| show_toast(Toast::new("toast-saved", IconName::Check), cx));
    }
    first.map_or(Ok(()), Err)
}

fn reported(task: Task<Result<Selection>>, op: Op, cx: &mut App) -> Task<Result<Selection>> {
    cx.spawn(async move |cx| match task.await {
        Ok(selection) => Ok(selection),
        Err(e) => Err(cx.update(|cx| fail(e, op, cx))),
    })
}

/// Shows why a capture could not start, which the coordinator only reports to the caller.
pub struct Reporting<T: ?Sized>(pub Rc<T>);

impl<T: SelectionFlow + ?Sized> SelectionFlow for Reporting<T> {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        let op = match mode {
            SelectMode::Scroll => Op::Scroll,
            SelectMode::Text => Op::Ocr,
            _ => Op::Capture,
        };
        let task = self.0.run(mode, cx);
        reported(task, op, cx)
    }
}

impl<T: DirectCapture + ?Sized> DirectCapture for Reporting<T> {
    fn run(&self, source: Direct, cx: &mut App) -> Task<Result<Selection>> {
        let task = self.0.run(source, cx);
        reported(task, Op::Capture, cx)
    }
}
