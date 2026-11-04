use es_desktop::dbus::Endpoint;
use es_record::project::{CommandRunner, SystemRunner};
use futures::future::BoxFuture;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type FileResult = Result<(), String>;

/// What the gallery asks of the desktop, behind a trait so tests never reach a real file manager or Trash.
pub trait GalleryFiles: Send + Sync {
    fn show_in_folder(&self, file: PathBuf) -> BoxFuture<'static, FileResult>;
    fn trash(&self, file: &Path) -> FileResult;

    /// Puts files moved to the Trash back where they were.
    fn restore(&self, _files: &[PathBuf]) -> FileResult {
        Err("restoring from the Trash is not supported".into())
    }

    /// A PNG of the video's first frame, at most `size` pixels each way; `None` when it cannot be made.
    fn video_frame(&self, _file: &Path, _size: u32) -> Option<Vec<u8>> {
        None
    }
}

/// Runs ffmpeg through `runner` for one frame; `None` when ffmpeg is missing or fails.
pub fn first_frame(runner: &dyn CommandRunner, file: &Path, size: u32) -> Option<Vec<u8>> {
    let args: Vec<String> = [
        "-v",
        "error",
        "-i",
        &file.to_string_lossy(),
        "-frames:v",
        "1",
        "-vf",
        &format!("scale={size}:{size}:force_original_aspect_ratio=decrease"),
        "-f",
        "image2pipe",
        "-c:v",
        "png",
        "-",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let out = runner.run("ffmpeg", &args).ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

pub struct DesktopFiles {
    endpoint: Endpoint,
    runner: Arc<dyn CommandRunner>,
}

impl DesktopFiles {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            runner: Arc::new(SystemRunner),
        }
    }
}

impl GalleryFiles for DesktopFiles {
    fn show_in_folder(&self, file: PathBuf) -> BoxFuture<'static, FileResult> {
        let endpoint = self.endpoint.clone();
        Box::pin(async move {
            es_desktop::show_in_file_manager(&endpoint, &file)
                .await
                .map_err(|e| e.to_string())
        })
    }

    fn trash(&self, file: &Path) -> FileResult {
        es_desktop::move_to_trash(file).map_err(|e| e.to_string())
    }

    fn restore(&self, files: &[PathBuf]) -> FileResult {
        es_desktop::restore_from_trash(files).map_err(|e| e.to_string())
    }

    fn video_frame(&self, file: &Path, size: u32) -> Option<Vec<u8>> {
        first_frame(self.runner.as_ref(), file, size)
    }
}

/// The image, its annotation sidecar and its unannotated base, those that exist.
pub fn capture_files(image: &Path) -> Vec<PathBuf> {
    [
        image.to_path_buf(),
        es_store::document::sidecar_path(image),
        es_store::library::base_path(image),
    ]
    .into_iter()
    .filter(|p| p.symlink_metadata().is_ok())
    .collect()
}
