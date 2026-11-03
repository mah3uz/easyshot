#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use es_app::history::History;
use es_model::{CaptureKind, HistoryEntry};
use es_shell::gallery::{self, FileResult, Gallery, GalleryFiles};
use es_store::Paths;
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use futures::future::BoxFuture;
use gpui::{AppContext, Entity, KeyDownEvent, Keystroke, PlatformInput};
use gpui_component::Root;
use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use uuid::Uuid;

pub const SCREENSHOTS: usize = 180;
pub const VIDEOS: usize = 20;
pub const INVOICES: usize = 18;

/// Records what the gallery asked for; trashing moves the file out of the library into the test's own folder.
#[derive(Default)]
pub struct FakeFiles {
    pub shown: Mutex<Vec<PathBuf>>,
    pub trashed: Mutex<Vec<PathBuf>>,
    pub trash_dir: PathBuf,
}

impl GalleryFiles for FakeFiles {
    fn show_in_folder(&self, file: PathBuf) -> BoxFuture<'static, FileResult> {
        self.shown.lock().unwrap().push(file);
        Box::pin(async { Ok(()) })
    }

    fn trash(&self, file: &Path) -> FileResult {
        let name = file.file_name().ok_or("no file name")?;
        std::fs::rename(file, self.trash_dir.join(name)).map_err(|e| e.to_string())?;
        self.trashed.lock().unwrap().push(file.to_path_buf());
        Ok(())
    }
}

type Made = Rc<RefCell<Option<(Entity<Gallery>, Entity<History>)>>>;

pub struct Rig {
    pub app: Headless<Root>,
    pub gallery: Entity<Gallery>,
    pub history: Entity<History>,
    pub files: Arc<FakeFiles>,
    pub paths: Paths,
    pub _dir: TempDir,
}

pub fn name_of(i: usize) -> String {
    match i {
        _ if i % 10 == 3 => format!("Invoice {i:03}"),
        7 => "Quarterly planning board with every sticky note from the offsite".into(),
        _ => format!(
            "easyshot_2026-10-{:02}-{:02}.{:02}",
            1 + i % 28,
            i % 24,
            i % 60
        ),
    }
}

/// Item `i` is `i` minutes old, so newest first is index order.
fn age(i: usize) -> SystemTime {
    SystemTime::now() - Duration::from_secs(60 * (i as u64 + 1))
}

fn picture(path: &Path, i: usize) {
    let hue = (i * 37 % 360) as f32 / 360.;
    let (w, h) = if i.is_multiple_of(4) {
        (180, 240)
    } else {
        (320, 200)
    };
    let image = image::RgbaImage::from_fn(w, h, |x, y| {
        let band = if y < h / 6 { 0.25 } else { 0.0 };
        let t = x as f32 / w as f32;
        let r = (0.5 + 0.5 * (std::f32::consts::TAU * (hue + 0.0)).cos())
            * (1. - band)
            * (0.6 + 0.4 * t);
        let g = (0.5 + 0.5 * (std::f32::consts::TAU * (hue + 0.33)).cos()) * (1. - band);
        let b = (0.5 + 0.5 * (std::f32::consts::TAU * (hue + 0.66)).cos())
            * (1. - band)
            * (1. - 0.3 * t);
        image::Rgba([(r * 255.) as u8, (g * 255.) as u8, (b * 255.) as u8, 255])
    });
    image.save(path).unwrap();
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(age(i))
        .unwrap();
}

/// 180 screenshots in the library, every fifth with a sidecar and base, plus 20 videos referenced elsewhere.
pub fn library(dir: &Path, count: usize) -> Paths {
    let paths = Paths::rooted(dir);
    paths.ensure_all().unwrap();
    let library = paths.library_dir();
    for i in 0..count.min(SCREENSHOTS) {
        let image = library.join(format!("{}.png", name_of(i)));
        picture(&image, i);
        if i % 5 == 0 {
            std::fs::write(es_store::document::sidecar_path(&image), "{}").unwrap();
            picture(&es_store::library::base_path(&image), i);
        }
    }
    paths
}

pub fn video_entries(dir: &Path, count: usize) -> Vec<HistoryEntry> {
    let folder = dir.join("Videos");
    std::fs::create_dir_all(&folder).unwrap();
    (0..count)
        .map(|k| {
            let i = SCREENSHOTS + k;
            let file = folder.join(format!("Recording {k:02}.mp4"));
            std::fs::write(&file, b"not decoded").unwrap();
            HistoryEntry {
                id: Uuid::new_v4(),
                created_at: age(i).into(),
                kind: CaptureKind::Region,
                name: format!("Recording {k:02}"),
                file,
                export_path: None,
                pixel_size: (1920, 1080),
                has_annotations: false,
                share_url: None,
            }
        })
        .collect()
}

pub fn rig_with(screenshots: usize, videos: usize, mode: ThemeMode, w: f32, h: f32) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = library(dir.path(), screenshots);
    let videos = video_entries(dir.path(), videos);
    let trash_dir = dir.path().join("Trash");
    std::fs::create_dir_all(&trash_dir).unwrap();
    let files = Arc::new(FakeFiles {
        trash_dir,
        ..FakeFiles::default()
    });
    let made: Made = Rc::default();
    let (sink, root_paths, root_files) = (made.clone(), paths.clone(), files.clone());
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        w,
        h,
        move |window, cx| {
            let history = cx.new(|_| History::open(&root_paths, 0).unwrap());
            history.update(cx, |h, cx| {
                for entry in videos {
                    h.add(entry, cx).unwrap();
                }
            });
            let (root, gallery) =
                gallery::root(history.clone(), root_paths, root_files, window, cx);
            *sink.borrow_mut() = Some((gallery, history));
            root
        },
    );
    let (gallery, history) = made.borrow_mut().take().unwrap();
    Rig {
        app,
        gallery,
        history,
        files,
        paths,
        _dir: dir,
    }
}

pub fn rig() -> Rig {
    rig_with(SCREENSHOTS, VIDEOS, ThemeMode::Dark, 1080., 740.)
}

impl Rig {
    pub fn update<R>(
        &mut self,
        f: impl FnOnce(&mut Gallery, &mut gpui::Context<Gallery>) -> R,
    ) -> R {
        let gallery = self.gallery.clone();
        let r = self.app.cx.update(|cx| gallery.update(cx, f));
        self.app.settle();
        r
    }

    pub fn read<R>(&mut self, f: impl FnOnce(&Gallery) -> R) -> R {
        let gallery = self.gallery.clone();
        self.app.cx.update(|cx| f(gallery.read(cx)))
    }

    pub fn press(&mut self, key: &'static str) {
        let handle = self.app.window.into();
        let event = PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        });
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .unwrap();
        self.app.settle();
    }

    pub fn wait_for_thumbnails(&mut self, at_least: usize) {
        let gallery = self.gallery.clone();
        self.app.until("thumbnails", |cx| {
            gallery.read(cx).thumbnails_ready() >= at_least
        });
    }

    /// Writes the frame where a reviewer can find it as well as under target/screenshots.
    pub fn picture(&mut self, name: &str) {
        let image = self.app.screenshot(name);
        if let Some(dir) = std::env::var_os("EASYSHOT_PICTURES") {
            std::fs::create_dir_all(&dir).unwrap();
            image
                .save(Path::new(&dir).join(format!("{name}.png")))
                .unwrap();
        }
    }
}
