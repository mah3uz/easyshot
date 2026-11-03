use super::windows::FolderPicker;
use es_backend::Backend;
use es_desktop::{FilePicker, SaveRequest};
use es_editor::session::io::{ChooseRequest, ExportChooser};
use es_store::naming::{exists, unique_path};
use gpui::{App, PathPromptOptions, Task};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Asks the desktop's file chooser portal where to export; without one, the save folder.
pub struct PortalChooser(pub Arc<Backend>);

impl ExportChooser for PortalChooser {
    fn choose(
        &self,
        request: ChooseRequest,
        cx: &mut App,
    ) -> Task<Result<Option<PathBuf>, String>> {
        choose_with(self.0.files.as_deref(), request, cx)
    }
}

fn choose_with(
    files: Option<&dyn FilePicker>,
    request: ChooseRequest,
    cx: &mut App,
) -> Task<Result<Option<PathBuf>, String>> {
    let Some(files) = files else {
        let path = unique_path(&request.suggested_name, &request.folder, "-", exists);
        return Task::ready(Ok(Some(path)));
    };
    let asked = files.save(SaveRequest {
        title: request.title,
        suggested_name: request.suggested_name,
        dir: Some(request.folder),
    });
    cx.background_executor()
        .spawn(async move { asked.await.map_err(|e| e.to_string()) })
}

/// Test mode never opens a real dialog.
pub struct Declining;

impl ExportChooser for Declining {
    fn choose(&self, _: ChooseRequest, _: &mut App) -> Task<Result<Option<PathBuf>, String>> {
        Task::ready(Ok(None))
    }
}

impl es_editor::inspector::background::WallpaperChooser for Declining {
    fn choose(&self, _: &mut App) -> Task<Option<PathBuf>> {
        Task::ready(None)
    }
}

pub fn folder_picker() -> FolderPicker {
    Rc::new(|cx| {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(es_app::strings::loader().get("picker-folder-title").into()),
        });
        cx.spawn(async move |_| match picked.await {
            Ok(Ok(Some(mut dirs))) => dirs.pop(),
            _ => None,
        })
    })
}

pub fn declining_folder_picker() -> FolderPicker {
    Rc::new(|_| Task::ready(None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_store::ExportFormat;
    use futures::future::BoxFuture;
    use gpui::TestAppContext;
    use std::path::Path;

    struct Chosen(PathBuf);

    impl FilePicker for Chosen {
        fn save(&self, _: SaveRequest) -> BoxFuture<'static, es_desktop::Result<Option<PathBuf>>> {
            let path = self.0.clone();
            Box::pin(async move { Ok(Some(path)) })
        }
        fn open(
            &self,
            _: es_desktop::OpenRequest,
        ) -> BoxFuture<'static, es_desktop::Result<Vec<PathBuf>>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn request(folder: &Path) -> ChooseRequest {
        ChooseRequest {
            title: "Export".into(),
            folder: folder.to_path_buf(),
            suggested_name: "shot.png".into(),
            format: ExportFormat::Png,
        }
    }

    fn choose(
        files: Option<&dyn FilePicker>,
        folder: &Path,
        cx: &mut TestAppContext,
    ) -> Result<Option<PathBuf>, String> {
        let task = cx.update(|cx| choose_with(files, request(folder), cx));
        cx.foreground_executor().block_test(task)
    }

    #[gpui::test]
    fn without_a_file_chooser_export_goes_to_the_save_folder(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            choose(None, dir.path(), cx),
            Ok(Some(dir.path().join("shot.png")))
        );
        std::fs::write(dir.path().join("shot.png"), b"x").unwrap();
        assert_eq!(
            choose(None, dir.path(), cx),
            Ok(Some(dir.path().join("shot-1.png")))
        );
    }

    #[gpui::test]
    fn with_a_file_chooser_its_answer_is_used(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = PathBuf::from("/elsewhere/out.png");
        let picker = Chosen(elsewhere.clone());
        assert_eq!(choose(Some(&picker), dir.path(), cx), Ok(Some(elsewhere)));
    }
}
