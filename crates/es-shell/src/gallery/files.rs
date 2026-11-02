use es_desktop::dbus::Endpoint;
use futures::future::BoxFuture;
use std::path::{Path, PathBuf};

pub type FileResult = Result<(), String>;

/// What the gallery asks of the desktop, behind a trait so tests never reach a real file manager or Trash.
pub trait GalleryFiles: Send + Sync {
    fn show_in_folder(&self, file: PathBuf) -> BoxFuture<'static, FileResult>;
    fn trash(&self, file: &Path) -> FileResult;
}

pub struct DesktopFiles {
    endpoint: Endpoint,
}

impl DesktopFiles {
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint }
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
