use super::windows::FolderPicker;
use es_backend::Backend;
use es_desktop::SaveRequest;
use es_editor::session::io::{ChooseRequest, ExportChooser};
use gpui::{App, PathPromptOptions, Task};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Asks the desktop's file chooser portal where to export.
pub struct PortalChooser(pub Arc<Backend>);

impl ExportChooser for PortalChooser {
    fn choose(
        &self,
        request: ChooseRequest,
        cx: &mut App,
    ) -> Task<Result<Option<PathBuf>, String>> {
        let Some(files) = &self.0.files else {
            return Task::ready(Err("no file chooser on this desktop".into()));
        };
        let asked = files.save(SaveRequest {
            title: request.title,
            suggested_name: request.suggested_name,
            dir: Some(request.folder),
        });
        cx.background_executor()
            .spawn(async move { asked.await.map_err(|e| e.to_string()) })
    }
}

/// Test mode never opens a real dialog.
pub struct Declining;

impl ExportChooser for Declining {
    fn choose(&self, _: ChooseRequest, _: &mut App) -> Task<Result<Option<PathBuf>, String>> {
        Task::ready(Ok(None))
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
