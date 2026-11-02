#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::{
    CaptureCoordinator, CaptureKind, CaptureRequest, Direct, DirectCapture,
};
use es_app::deck::{Deck, DeckPolicy};
use es_app::errors::{Error, Result};
use es_app::history::History;
use es_app::jobs::Jobs;
use es_app::pipeline::{self, PipelineDeps, Reporting};
use es_app::platform_ui::{ClipboardSink, Selection};
use es_app::settings_model::SettingsModel;
use es_app::toast::{MemorySink, install_toasts};
use es_desktop::RecordingNotifier;
use es_geom::{Rect, Rgba};
use es_model::Config;
use es_model::capture::CaptureVariant;
use es_model::platform::{ClipItem, Frame, Transform};
use es_protocol::JobStatus;
use es_store::{ConfigFile, Paths};
use gpui::{App, AppContext as _, Entity, Task, TestAppContext};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
struct FakeClipboard {
    copied: RefCell<Vec<Vec<ClipItem>>>,
    fail: bool,
}

impl ClipboardSink for FakeClipboard {
    fn copy(&self, items: Vec<ClipItem>, _cx: &mut App) -> Task<Result<()>> {
        if self.fail {
            return Task::ready(Err(Error::Protocol("no clipboard".into())));
        }
        self.copied.borrow_mut().push(items);
        Task::ready(Ok(()))
    }
}

type Calls = Rc<RefCell<Vec<PathBuf>>>;

struct Rig {
    _dir: tempfile::TempDir,
    paths: Paths,
    save_dir: PathBuf,
    settings: Entity<SettingsModel>,
    history: Entity<History>,
    deck: Entity<Deck>,
    clipboard: Rc<FakeClipboard>,
    toasts: Arc<MemorySink>,
    pins: Calls,
    edits: Calls,
    deps: Rc<PipelineDeps>,
}

fn rig_with(cx: &mut TestAppContext, clipboard: FakeClipboard, text: &'static str) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let save_dir = dir.path().join("saved");
    let file = ConfigFile::load(&paths.config_file()).unwrap();
    let settings = cx.new(|_| SettingsModel::new(file));
    let history = cx.new(|_| History::open(&paths, 50).unwrap());
    let deck = cx.new(|_| Deck::new(DeckPolicy::default()));
    let clipboard = Rc::new(clipboard);
    let toasts = MemorySink::new();
    let (pins, edits) = (Calls::default(), Calls::default());
    let (pin_log, edit_log) = (pins.clone(), edits.clone());
    let deps = Rc::new(PipelineDeps {
        paths: paths.clone(),
        settings: settings.clone(),
        history: history.clone(),
        deck: deck.clone(),
        clipboard: clipboard.clone(),
        recognize: Arc::new(move |_, _| Ok(text.to_owned())),
        pin: Rc::new(move |file, _, _| {
            pin_log.borrow_mut().push(file);
            Ok(())
        }),
        open_editor: Rc::new(move |file, _| {
            edit_log.borrow_mut().push(file);
            Ok(())
        }),
    });
    let sink = toasts.clone();
    cx.update(|cx| install_toasts(sink, Arc::new(RecordingNotifier::new(false)), cx));
    let rig = Rig {
        _dir: dir,
        paths,
        save_dir,
        settings,
        history,
        deck,
        clipboard,
        toasts,
        pins,
        edits,
        deps,
    };
    rig.configure(cx, |c| {
        let rules = &mut c.after_capture.screenshot;
        rules.show_preview = true;
        rules.copy = false;
        rules.save = false;
        rules.pin = false;
        rules.open_editor = false;
        c.general.copy_after_save = false;
        c.capture.open_editor_after_capture = false;
        c.capture.keep_in_deck_until_saved = true;
    });
    rig
}

fn rig(cx: &mut TestAppContext) -> Rig {
    rig_with(cx, FakeClipboard::default(), "")
}

fn frame() -> Arc<Frame> {
    let (w, h) = (4u32, 2u32);
    let bgra: Vec<u8> = (0..w * h).flat_map(|_| [0x00, 0x80, 0xFF, 0x00]).collect();
    Arc::new(Frame {
        output: None,
        size_px: (w, h),
        logical: Rect::new(0., 0., f64::from(w), f64::from(h)),
        scale: 1.,
        bgra: bgra.into(),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    })
}

fn region() -> Selection {
    Selection::Window {
        id: es_model::platform::WindowId("w".into()),
        frame: frame(),
    }
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    found.sort();
    found
}

fn images(dir: &Path) -> Vec<PathBuf> {
    files(dir)
        .into_iter()
        .filter(|p| es_store::library::is_image(p))
        .collect()
}

impl Rig {
    fn configure(&self, cx: &mut TestAppContext, f: impl FnOnce(&mut Config)) {
        let save_dir = self.save_dir.clone();
        self.settings.update(cx, |s, cx| {
            s.update(
                |c| {
                    c.general.save_dir = Some(save_dir);
                    f(c);
                },
                cx,
            )
        });
    }

    fn run(
        &self,
        cx: &mut TestAppContext,
        kind: CaptureKind,
        variant: CaptureVariant,
    ) -> Result<()> {
        self.run_with(cx, kind, variant, region())
    }

    fn run_with(
        &self,
        cx: &mut TestAppContext,
        kind: CaptureKind,
        variant: CaptureVariant,
        selection: Selection,
    ) -> Result<()> {
        let request = CaptureRequest::new(kind, variant);
        let task = cx.update(|cx| pipeline::run(&self.deps, request, selection, cx));
        cx.run_until_parked();
        cx.foreground_executor().block_test(task)
    }

    fn toast_titles(&self) -> Vec<String> {
        self.toasts.shown().into_iter().map(|t| t.title).collect()
    }

    fn deck_len(&self, cx: &mut TestAppContext) -> usize {
        self.deck.read_with(cx, |d, _| d.len())
    }

    fn staged_dirs(&self) -> usize {
        files(&self.paths.deck_dir()).len()
    }
}

#[gpui::test]
fn a_saved_capture_lands_in_the_save_folder_and_on_the_deck(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.after_capture.screenshot.save = true);
    rig.run(cx, CaptureKind::Window, CaptureVariant::Normal)
        .unwrap();
    let saved = files(&rig.save_dir);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].extension().unwrap(), "png");
    let pixels = image::open(&saved[0]).unwrap().to_rgba8();
    assert_eq!(
        pixels.get_pixel(0, 0).0,
        [0xFF, 0x80, 0x00, 0xFF],
        "BGRA to opaque RGBA"
    );
    assert_eq!(rig.deck_len(cx), 1);
    let item = rig
        .deck
        .read_with(cx, |d, _| d.items().next().cloned().unwrap());
    assert!(
        item.staged,
        "kept in the deck until saved: not in the library"
    );
    assert!(item.file.exists() && item.thumb.exists());
    assert!(images(&rig.paths.library_dir()).is_empty());
    assert!(rig.clipboard.copied.borrow().is_empty());
}

#[gpui::test]
fn jpeg_export_format_saves_a_jpeg(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.general.export_format = "jpeg".into());
    rig.run(cx, CaptureKind::Region, CaptureVariant::RegionSave)
        .unwrap();
    let saved = files(&rig.save_dir);
    assert_eq!(saved.len(), 1);
    assert_eq!(
        image::ImageFormat::from_path(&saved[0]).unwrap(),
        image::ImageFormat::Jpeg
    );
}

#[gpui::test]
fn region_copy_puts_a_png_on_the_clipboard_and_saves_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.after_capture.screenshot.save = true);
    rig.run(cx, CaptureKind::Region, CaptureVariant::RegionCopy)
        .unwrap();
    let copied = rig.clipboard.copied.borrow();
    assert_eq!(copied.len(), 1);
    assert!(matches!(&copied[0][..], [ClipItem::Png(png)] if png.starts_with(b"\x89PNG")));
    assert!(files(&rig.save_dir).is_empty());
}

#[gpui::test]
fn without_keep_in_deck_the_capture_goes_into_the_library_and_history(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.capture.keep_in_deck_until_saved = false);
    rig.run(cx, CaptureKind::Window, CaptureVariant::Normal)
        .unwrap();
    let library = images(&rig.paths.library_dir());
    assert_eq!(library.len(), 1);
    let entries = rig.history.read_with(cx, |h, _| h.entries().to_vec());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file, library[0]);
    assert_eq!(entries[0].kind, es_model::CaptureKind::Window);
    let item = rig
        .deck
        .read_with(cx, |d, _| d.items().next().cloned().unwrap());
    assert!(!item.staged);
}

#[gpui::test]
fn region_pin_pins_the_kept_file_and_shows_no_card(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.capture.keep_in_deck_until_saved = false);
    rig.run(cx, CaptureKind::Region, CaptureVariant::RegionPin)
        .unwrap();
    assert_eq!(rig.deck_len(cx), 0);
    let library = images(&rig.paths.library_dir());
    assert_eq!(*rig.pins.borrow(), library);
    cx.run_until_parked();
    assert_eq!(
        rig.staged_dirs(),
        0,
        "a capture kept elsewhere and never shown must not come back as a card"
    );
}

#[gpui::test]
fn region_annotate_opens_the_editor(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.run(cx, CaptureKind::Region, CaptureVariant::RegionAnnotate)
        .unwrap();
    assert_eq!(rig.edits.borrow().len(), 1);
    assert!(rig.edits.borrow()[0].exists());
}

#[gpui::test]
fn a_failed_save_is_shown_and_reported_but_the_card_stays(cx: &mut TestAppContext) {
    let rig = rig(cx);
    std::fs::write(&rig.save_dir, b"not a folder").unwrap();
    rig.configure(cx, |c| c.after_capture.screenshot.save = true);
    let result = rig.run(cx, CaptureKind::Region, CaptureVariant::Normal);
    assert!(result.is_err());
    assert_eq!(rig.toast_titles(), ["Couldn’t save capture"]);
    assert_eq!(rig.deck_len(cx), 1);
}

#[gpui::test]
fn a_failed_copy_is_shown_and_reported(cx: &mut TestAppContext) {
    let rig = rig_with(
        cx,
        FakeClipboard {
            fail: true,
            ..Default::default()
        },
        "",
    );
    let result = rig.run(cx, CaptureKind::Region, CaptureVariant::RegionCopy);
    assert!(result.is_err());
    assert_eq!(rig.toast_titles(), ["Copy Failed"]);
}

#[gpui::test]
fn a_picked_colour_is_copied_as_hex(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let color = Selection::Color(Rgba::new(1.0, 0.5, 0.0, 1.0));
    rig.run_with(cx, CaptureKind::Color, CaptureVariant::Normal, color)
        .unwrap();
    assert!(matches!(&rig.clipboard.copied.borrow()[0][..], [ClipItem::Text(t)] if t == "#FF8000"));
    assert_eq!(rig.toast_titles(), ["#FF8000 copied to clipboard"]);
    assert_eq!(rig.deck_len(cx), 0);
}

#[gpui::test]
fn recognised_text_is_copied_and_single_line_joins_lines(cx: &mut TestAppContext) {
    let rig = rig_with(cx, FakeClipboard::default(), "first line\nsecond");
    rig.run(cx, CaptureKind::Text, CaptureVariant::Normal)
        .unwrap();
    rig.run(cx, CaptureKind::TextLine, CaptureVariant::Normal)
        .unwrap();
    let texts: Vec<String> = rig
        .clipboard
        .copied
        .borrow()
        .iter()
        .flat_map(|items| items.iter())
        .filter_map(|i| match i {
            ClipItem::Text(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["first line\nsecond", "first line second"]);
    assert_eq!(rig.deck_len(cx), 0);
}

#[gpui::test]
fn no_text_found_copies_nothing(cx: &mut TestAppContext) {
    let rig = rig_with(cx, FakeClipboard::default(), "  \n");
    rig.run(cx, CaptureKind::Text, CaptureVariant::Normal)
        .unwrap();
    assert!(rig.clipboard.copied.borrow().is_empty());
    assert_eq!(rig.toast_titles(), ["No text found"]);
}

struct NoCapture;

impl DirectCapture for NoCapture {
    fn run(&self, _source: Direct, _cx: &mut App) -> Task<Result<Selection>> {
        Task::ready(Err(Error::Unsupported))
    }
}

#[gpui::test]
fn a_missing_capture_capability_is_shown_and_the_queue_keeps_serving(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let (jobs, done) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    coordinator.set_direct_capture(Rc::new(Reporting(Rc::new(NoCapture))));
    coordinator.set_pipeline(pipeline::pipeline(rig.deps.clone()));
    for _ in 0..2 {
        let job = jobs.allocate();
        cx.update(|cx| {
            coordinator.submit(
                job,
                CaptureRequest::new(CaptureKind::Fullscreen, CaptureVariant::Normal),
                cx,
            )
        });
        cx.run_until_parked();
        let finished = done.try_recv().unwrap();
        assert_eq!((finished.id, finished.status), (job, JobStatus::Error));
    }
    assert_eq!(
        rig.toast_titles(),
        ["Not available here", "Not available here"]
    );
}
