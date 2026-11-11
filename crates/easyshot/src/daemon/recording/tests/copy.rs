use super::*;
use es_app::platform_ui::ClipboardSink;
use es_model::CaptureKind;
use es_model::platform::ClipItem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct FakeFfmpeg {
    fails: AtomicBool,
    calls: Mutex<Vec<Vec<String>>>,
}

impl CommandRunner for FakeFfmpeg {
    fn run(&self, _: &str, args: &[String]) -> std::io::Result<Output> {
        self.calls.lock().unwrap().push(args.to_vec());
        let fails = self.fails.load(Ordering::SeqCst);
        if !fails {
            std::fs::write(args.last().unwrap(), b"mp4").unwrap();
        }
        Ok(Output {
            status: ExitStatus::from_raw(if fails { 256 } else { 0 }),
            stdout: Vec::new(),
            stderr: b"remux failed".to_vec(),
        })
    }
}

#[derive(Default)]
struct FakeClipboard(RefCell<Vec<Vec<ClipItem>>>);

impl ClipboardSink for FakeClipboard {
    fn copy(&self, items: Vec<ClipItem>, _: &mut App) -> Task<es_app::errors::Result<()>> {
        self.0.borrow_mut().push(items);
        Task::ready(Ok(()))
    }
}

struct Copying {
    rig: Rig,
    ffmpeg: Arc<FakeFfmpeg>,
    clipboard: Rc<FakeClipboard>,
    sink: Arc<es_app::toast::MemorySink>,
}

fn copying(cx: &mut TestAppContext) -> Copying {
    let ffmpeg = Arc::new(FakeFfmpeg::default());
    let rig = rig_runner(
        cx,
        FakeRecorder::default(),
        None,
        false,
        ffmpeg.clone() as Arc<dyn CommandRunner>,
    );
    let sink = es_app::toast::MemorySink::new();
    cx.update(|cx| {
        es_app::toast::install_toasts(
            sink.clone(),
            Arc::new(es_desktop::RecordingNotifier::new(false)),
            cx,
        )
    });
    Copying {
        rig,
        ffmpeg,
        clipboard: Rc::default(),
        sink,
    }
}

impl Copying {
    fn card(&self, saved: Option<PathBuf>, cx: &mut TestAppContext) -> DeckItem {
        let dir = self.rig.paths.recordings_dir().join("Recording A");
        std::fs::create_dir_all(&dir).unwrap();
        let video = dir.join("recording.mkv");
        std::fs::write(&video, b"mkv").unwrap();
        let mut item = finish::card(&dir, video, CaptureKind::Fullscreen);
        item.saved = saved;
        let shown = item.clone();
        cx.update(|cx| self.rig.deck.update(cx, |d, cx| d.show(shown, cx)));
        item
    }

    fn copy(&self, item: &DeckItem, cx: &mut TestAppContext) {
        let clipboard = self.clipboard.clone();
        cx.update(|cx| self.rig.recordings.copy_card(item.id, clipboard, cx));
        cx.run_until_parked();
    }

    fn copied(&self) -> Vec<Vec<ClipItem>> {
        self.clipboard.0.borrow().clone()
    }
}

fn files(items: &[ClipItem]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut uris = Vec::new();
    let mut gnome = Vec::new();
    for item in items {
        match item {
            ClipItem::UriList(f) => uris.extend(f.clone()),
            ClipItem::GnomeCopiedFiles(f) => gnome.extend(f.clone()),
            _ => {}
        }
    }
    (uris, gnome)
}

#[gpui::test]
async fn copy_of_a_saved_recording_puts_the_saved_mp4_on_the_clipboard_and_closes_the_card(
    cx: &mut TestAppContext,
) {
    let t = copying(cx);
    let saved = t.rig.paths.videos_dir().join("kept.mp4");
    std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
    std::fs::write(&saved, b"mp4").unwrap();
    let item = t.card(Some(saved.clone()), cx);

    t.copy(&item, cx);

    let copied = t.copied();
    assert_eq!(copied.len(), 1);
    assert_eq!(files(&copied[0]), (vec![saved.clone()], vec![saved]));
    assert!(t.ffmpeg.calls.lock().unwrap().is_empty(), "no remux");
    assert!(cards(&t.rig, cx).is_empty(), "the card closed");
    assert!(item.file.is_file(), "the recording itself stays");
}

#[gpui::test]
async fn copy_of_an_unsaved_recording_remuxes_into_the_temp_folder_not_the_save_folder(
    cx: &mut TestAppContext,
) {
    let t = copying(cx);
    let item = t.card(None, cx);

    t.copy(&item, cx);

    let copied = t.copied();
    assert_eq!(copied.len(), 1);
    let (uris, gnome) = files(&copied[0]);
    assert_eq!(uris, gnome);
    let file = &uris[0];
    assert!(file.starts_with(t.rig.paths.video_copies_dir()), "{file:?}");
    assert_eq!(file.extension().unwrap(), "mp4");
    assert!(file.is_file());
    let calls = t.ffmpeg.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].contains(&item.file.to_string_lossy().into_owned()));
    let saved_here = std::fs::read_dir(t.rig.paths.videos_dir())
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(saved_here, 0, "nothing went to the save folder");
    assert!(cards(&t.rig, cx).is_empty(), "the card closed");
    assert!(item.file.is_file(), "the recording itself stays");
}

#[gpui::test]
async fn a_failed_remux_shows_the_copy_failed_toast_and_keeps_the_card(cx: &mut TestAppContext) {
    let t = copying(cx);
    t.ffmpeg.fails.store(true, Ordering::SeqCst);
    let item = t.card(None, cx);

    t.copy(&item, cx);

    assert!(t.copied().is_empty(), "nothing reached the clipboard");
    assert_eq!(cards(&t.rig, cx).len(), 1, "the card stays");
    let shown = t.sink.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].title, "Copy Failed");
}
