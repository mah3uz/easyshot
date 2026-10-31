use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use gpui::{App, AssetSource, Entity, HeadlessAppContext, Render, Window, WindowHandle, px, size};
use image::RgbaImage;

const PATIENCE: Duration = Duration::from_secs(10);

/// Encoding a screenshot takes as long as drawing it, so it runs beside the test and is awaited as the window closes.
static WRITING: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

pub struct Headless<V: Render + 'static> {
    pub cx: HeadlessAppContext,
    pub window: WindowHandle<V>,
}

impl<V: Render + 'static> Headless<V> {
    pub fn open(
        assets: impl AssetSource,
        init: impl FnOnce(&mut App),
        width: f32,
        height: f32,
        build_root: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
    ) -> Self {
        let text_system = gpui_platform::current_platform(true).text_system();
        let mut cx = HeadlessAppContext::with_platform(
            text_system,
            Arc::new(assets),
            gpui_platform::current_headless_renderer,
        );
        cx.update(init);
        let window = cx
            .open_window(size(px(width), px(height)), build_root)
            .expect("open headless window");
        let mut this = Self { cx, window };
        this.settle();
        this
    }

    pub fn settle(&mut self) {
        self.cx.run_until_parked();
    }

    pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut App) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        loop {
            self.settle();
            if self.cx.update(&mut done) {
                self.settle();
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// The exact frame, also written to `target/screenshots/<name>.png` on a side thread.
    pub fn screenshot(&mut self, name: &str) -> RgbaImage {
        self.settle();
        let image = self.cx.capture_screenshot(self.window.into()).expect("capture screenshot");
        let dir = screenshots_dir();
        std::fs::create_dir_all(&dir).expect("create screenshots dir");
        let (written, path) = (image.clone(), dir.join(format!("{name}.png")));
        let handle = thread::spawn(move || written.save(path).expect("write screenshot"));
        WRITING.lock().unwrap_or_else(|e| e.into_inner()).push(handle);
        image
    }
}

impl<V: Render + 'static> Drop for Headless<V> {
    /// Ends the window before the app: the off-screen scheduler can lock up when a thread wakes the UI during shutdown.
    fn drop(&mut self) {
        let window = self.window;
        let _ = self.cx.update(|cx| window.update(cx, |_, window, _| window.remove_window()));
        self.cx.run_until_parked();
        thread::sleep(Duration::from_millis(40));
        let writing = std::mem::take(&mut *WRITING.lock().unwrap_or_else(|e| e.into_inner()));
        for handle in writing {
            let ok = handle.join().is_ok();
            assert!(ok || thread::panicking(), "a screenshot could not be written");
        }
    }
}

fn screenshots_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots")
}
