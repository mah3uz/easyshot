#![allow(clippy::unwrap_used)]
use super::share::{Fake, finished, made, rig, wait_until};
use es_app::share::{FileOrigin, ShareFlow, ShareHooks, WindowPopover};
use es_app::toast::{MemorySink, install_toasts};
use es_desktop::RecordingNotifier;
use es_model::{ShareKind, ShareSource};
use es_record::{CommandRunner, SystemRunner};
use es_share::ShareRequest;
use es_ui::components::{ShareEvent, ShareView};
use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _,
    TestAppContext, Window, div, px,
};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

/// A window with only a Share button, as the trim viewer's title bar has.
struct Viewer(WindowPopover);

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let button = div().w(px(80.)).h(px(24.));
        div()
            .size_full()
            .child(self.0.around(button, gpui::Anchor::TopRight, window, cx))
    }
}

fn recording(dir: &Path) -> PathBuf {
    let out = dir.join("recording.mp4");
    let args: Vec<String> = [
        "-y",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=64x36:rate=10:duration=1",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([out.to_string_lossy().into_owned()])
    .collect();
    let made = SystemRunner.run("ffmpeg", &args).unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    out
}

struct Shown {
    flow: Entity<ShareFlow>,
    host: WindowPopover,
    video: PathBuf,
    sent: Arc<Mutex<Vec<ShareRequest>>>,
    rig: super::share::Rig,
}

fn viewer(cx: &mut TestAppContext, video_max_bytes: u64) -> Shown {
    let sent: Arc<Mutex<Vec<ShareRequest>>> = Arc::default();
    let into = sent.clone();
    let fake = Fake::new(Box::new(move |r| {
        into.lock().unwrap().push(r.clone());
        Ok(made(r))
    }));
    fake.video_max_bytes
        .store(video_max_bytes, Ordering::SeqCst);
    let rig = rig(cx, &fake, |_| {});
    let video = recording(rig._dir.path());
    let host = WindowPopover::default();
    let shares = rig.shares.clone();
    let flow = cx.update(|cx| {
        es_ui::init(cx);
        install_toasts(
            MemorySink::new(),
            Arc::new(RecordingNotifier::default()),
            cx,
        );
        let hooks = ShareHooks {
            open_settings: Rc::new(|_| {}),
            open_url: Rc::new(|_, _| {}),
        };
        let clipboard = Rc::new(super::share_deck::Clip::default());
        cx.new(|cx| ShareFlow::new(shares, hooks, clipboard, cx))
    });
    let shown = host.clone();
    cx.add_window(move |_, _| Viewer(shown));
    let origin = Rc::new(FileOrigin {
        file: video.clone(),
        host: host.clone(),
    });
    flow.update(cx, |f, cx| f.open(origin, cx));
    cx.run_until_parked();
    Shown {
        flow,
        host,
        video,
        sent,
        rig,
    }
}

impl Shown {
    fn view(&self, cx: &mut TestAppContext) -> Option<ShareView> {
        let popover = self.flow.read_with(cx, |f, _| f.popover())?;
        Some(popover.read_with(cx, |p, _| p.view().clone()))
    }

    fn share(&self, cx: &mut TestAppContext) {
        let popover = self.flow.read_with(cx, |f, _| f.popover()).unwrap();
        popover.update(cx, |_, cx| cx.emit(ShareEvent::Share { title: None }));
        cx.run_until_parked();
    }
}

#[gpui::test]
fn share_in_a_window_opens_the_popover_there_and_sends_the_recording_as_its_own(
    cx: &mut TestAppContext,
) {
    let s = viewer(cx, 1 << 26);
    assert!(s.host.is_open(), "the popover opens in the viewer's window");
    assert_eq!(s.view(cx), Some(ShareView::Ready));

    s.share(cx);
    assert!(!s.host.is_open(), "the popover closes as it shares");
    wait_until(cx, "the upload", || finished(&s.rig));

    let sent = s.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].source,
        ShareSource::Recording(s.video.clone()),
        "the link belongs to the recording, so its card shows the cloud mark"
    );
    assert_eq!(sent[0].media.kind, ShareKind::Video);
}

#[gpui::test]
fn a_recording_too_long_to_share_reopens_the_popover_in_its_window_saying_so(
    cx: &mut TestAppContext,
) {
    let s = viewer(cx, 1024);
    s.share(cx);
    wait_until(cx, "the too-long answer", || s.host.is_open());
    cx.run_until_parked();

    assert!(
        matches!(s.view(cx), Some(ShareView::TooLong { .. })),
        "{:?}",
        s.view(cx)
    );
    assert!(s.sent.lock().unwrap().is_empty(), "nothing was uploaded");
}
