use super::*;
use crate::daemon::routes::deck_events;
use es_app::deck::{DeckAction, DeckItem, ItemStatus};
use es_app::toast::{CloseReason, MemorySink, UNDO_DURATION, install_toasts};
use es_model::CaptureKind;
use es_overlays::pin::Pins;
use es_shell::preview::Preview;
use std::sync::Mutex;
use std::time::Duration;

pub(super) struct NoFiles;

impl GalleryFiles for NoFiles {
    fn show_in_folder(&self, _: PathBuf) -> BoxFuture<'static, FileResult> {
        Box::pin(async { Ok(()) })
    }
    fn trash(&self, _: &Path) -> FileResult {
        Ok(())
    }
}

/// A Trash kept in a directory the test owns.
struct DirTrash {
    dir: PathBuf,
    moved: Mutex<Vec<(PathBuf, PathBuf)>>,
}

impl DirTrash {
    fn new(dir: &Path) -> Arc<Self> {
        std::fs::create_dir_all(dir).unwrap();
        Arc::new(Self {
            dir: dir.to_owned(),
            moved: Mutex::default(),
        })
    }

    fn trashed(&self) -> Vec<PathBuf> {
        self.moved
            .lock()
            .unwrap()
            .iter()
            .map(|(from, _)| from.clone())
            .collect()
    }
}

impl GalleryFiles for DirTrash {
    fn show_in_folder(&self, _: PathBuf) -> BoxFuture<'static, FileResult> {
        Box::pin(async { Ok(()) })
    }
    fn trash(&self, file: &Path) -> FileResult {
        let mut moved = self.moved.lock().unwrap();
        let to = self.dir.join(format!(
            "{}-{}",
            moved.len(),
            file.file_name().unwrap().to_string_lossy()
        ));
        std::fs::rename(file, &to).map_err(|e| e.to_string())?;
        moved.push((file.to_owned(), to));
        Ok(())
    }
    fn restore(&self, files: &[PathBuf]) -> FileResult {
        let mut moved = self.moved.lock().unwrap();
        for file in files {
            let at = moved
                .iter()
                .position(|(from, _)| from == file)
                .ok_or("not in the trash")?;
            let (from, to) = moved.remove(at);
            std::fs::rename(to, from).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

struct Fixture {
    rig: Rig,
    trash: Arc<DirTrash>,
    sink: Arc<MemorySink>,
    _pins: Rc<Pins>,
    _events: gpui::Subscription,
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    let probe = tempfile::tempdir().unwrap();
    let trash = DirTrash::new(&probe.path().join("trash"));
    std::mem::forget(probe);
    let rig = rig_files(cx, None, trash.clone());
    let sink = MemorySink::new();
    let (pins, events) = cx.update(|cx| {
        es_ui::init(cx);
        install_toasts(sink.clone(), Arc::new(Silent), cx);
        let surfaces = Rc::new(LayerShellSurfaces::new(Arc::new(NoOutputs), None));
        let pins = Rc::new(Pins::new(surfaces, Rc::new(|_, _| {})));
        let events = deck_events(
            &rig.deck,
            &rig.history,
            &rig.wiring,
            &pins,
            &rig.paths,
            Rc::new(NoClipboard),
            cx,
        );
        (pins, events)
    });
    Fixture {
        rig,
        trash,
        sink,
        _pins: pins,
        _events: events,
    }
}

fn library_card(f: &Fixture, cx: &mut TestAppContext) -> (DeckItem, PathBuf, PathBuf) {
    let image = f.rig.paths.library_dir().join("shot.png");
    png(&image);
    let sidecar = es_store::document::sidecar_path(&image);
    std::fs::write(&sidecar, b"{}").unwrap();
    let entry = es_store::library::entry_for_file(&image).unwrap();
    let item = DeckItem {
        id: entry.id,
        name: entry.name.clone(),
        file: image.clone(),
        thumb: image.clone(),
        kind: CaptureKind::Region,
        video: false,
        staged: false,
        status: ItemStatus::Idle,
        auto_dismiss: false,
        saved: None,
    };
    cx.update(|cx| {
        f.rig.history.update(cx, |h, cx| h.add(entry, cx)).unwrap();
        f.rig.deck.update(cx, |d, cx| d.show(item.clone(), cx));
    });
    (item, image, sidecar)
}

fn staged_card(f: &Fixture, cx: &mut TestAppContext) -> DeckItem {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        8,
        6,
        image::Rgba([1, 2, 3, 255]),
    ))
    .write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    let manifest = es_store::Deck::new(&f.rig.paths)
        .stage(
            &es_store::StageRequest {
                file_name: "Staged.png",
                kind: CaptureKind::Region,
                history_id: None,
                image: &bytes,
                raw: &bytes,
                thumb_size: 64,
            },
            time::OffsetDateTime::now_utc(),
        )
        .unwrap();
    let item = DeckItem::staged(&manifest);
    cx.update(|cx| f.rig.deck.update(cx, |d, cx| d.show(item.clone(), cx)));
    item
}

fn act(f: &Fixture, item: &DeckItem, action: DeckAction, cx: &mut TestAppContext) {
    cx.update(|cx| f.rig.deck.update(cx, |d, cx| d.act(item.id, action, cx)));
    cx.run_until_parked();
}

fn press_undo(f: &Fixture, cx: &mut TestAppContext) {
    let undo = f
        .sink
        .shown()
        .last()
        .and_then(|t| t.undo.clone())
        .expect("the toast offers Undo")
        .1;
    cx.update(|cx| undo.run(cx));
    cx.run_until_parked();
}

fn on_deck(f: &Fixture, cx: &mut TestAppContext) -> usize {
    cx.update(|cx| f.rig.deck.read(cx).len())
}

#[gpui::test]
fn delete_moves_a_kept_capture_to_the_trash_and_undo_brings_everything_back(
    cx: &mut TestAppContext,
) {
    let f = fixture(cx);
    let (item, image, sidecar) = library_card(&f, cx);

    act(&f, &item, DeckAction::Delete, cx);

    assert!(
        !image.exists() && !sidecar.exists(),
        "both files left the library"
    );
    assert_eq!(f.trash.trashed(), [image.clone(), sidecar.clone()]);
    assert_eq!(on_deck(&f, cx), 0);
    assert!(cx.update(|cx| f.rig.history.read(cx).get(item.id).is_none()));
    let toast = f.sink.shown().pop().unwrap();
    assert_eq!(toast.title, "Moved to the Trash");
    assert_eq!(toast.undo.as_ref().unwrap().0, "Undo");

    press_undo(&f, cx);

    assert!(image.exists() && sidecar.exists());
    assert_eq!(on_deck(&f, cx), 1);
    assert!(cx.update(|cx| f.rig.history.read(cx).get(item.id).is_some()));
}

#[gpui::test]
fn delete_of_an_unsaved_capture_discards_it_and_undo_returns_the_card(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let item = staged_card(&f, cx);

    act(&f, &item, DeckAction::Delete, cx);

    assert!(
        f.trash.trashed().is_empty(),
        "nothing was saved, so nothing is trashed"
    );
    assert!(
        item.file.exists(),
        "the staging stays while Undo can bring the card back"
    );
    assert_eq!(on_deck(&f, cx), 0);
    assert_eq!(f.sink.shown().pop().unwrap().title, "Capture deleted");

    press_undo(&f, cx);
    assert_eq!(on_deck(&f, cx), 1);
    assert!(item.file.exists());
}

#[gpui::test]
fn undo_outlives_the_undo_duration_while_the_toast_is_held_open(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let item = staged_card(&f, cx);
    act(&f, &item, DeckAction::Delete, cx);

    cx.executor().advance_clock(UNDO_DURATION * 2);
    cx.run_until_parked();

    assert!(
        item.file.exists(),
        "the staging stays while the toast is up"
    );
    press_undo(&f, cx);
    assert_eq!(on_deck(&f, cx), 1);
}

#[gpui::test]
fn an_unsaved_capture_is_gone_for_good_once_its_toast_closes(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let item = staged_card(&f, cx);
    act(&f, &item, DeckAction::Delete, cx);

    cx.update(|cx| f.sink.close(0, CloseReason::Expired, cx));
    cx.run_until_parked();

    assert!(!item.file.exists(), "the staging is removed");
    press_undo(&f, cx);
    assert_eq!(on_deck(&f, cx), 0, "a late Undo does nothing");
}

#[gpui::test]
fn preview_opens_a_window_that_keeps_the_card_and_closes_on_space_and_escape(
    cx: &mut TestAppContext,
) {
    let f = fixture(cx);
    let (item, _, _) = library_card(&f, cx);
    for key in ["space", "escape"] {
        act(&f, &item, DeckAction::Preview, cx);
        assert_eq!(showing::<Preview>(cx), 1, "the preview window opened");
        assert_eq!(on_deck(&f, cx), 1, "Preview keeps the card");
        let window = cx.windows().into_iter().next().unwrap();
        cx.simulate_keystrokes(window, key);
        assert_eq!(showing::<Preview>(cx), 0, "{key} closed it");
    }
}

#[gpui::test]
fn z_and_a_click_toggle_actual_size(cx: &mut TestAppContext) {
    let f = fixture(cx);
    let (item, _, _) = library_card(&f, cx);
    act(&f, &item, DeckAction::Preview, cx);
    let window = cx.windows().into_iter().next().unwrap();
    let view = cx.update(|cx| {
        window
            .downcast::<Root>()
            .unwrap()
            .read(cx)
            .unwrap()
            .view()
            .clone()
            .downcast::<Preview>()
            .unwrap()
    });
    assert!(!cx.update(|cx| view.read(cx).actual_size()));
    cx.simulate_keystrokes(window, "z");
    assert!(cx.update(|cx| view.read(cx).actual_size()));
    cx.simulate_keystrokes(window, "z");
    assert!(!cx.update(|cx| view.read(cx).actual_size()));
}
