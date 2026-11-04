use es_app::toast::{NoticeAction, notice_action};
use es_desktop::NotificationId;
use es_shell::gallery::GalleryFiles;
use gpui::App;
use std::path::Path;

pub struct Answers<'a> {
    pub open: &'a dyn Fn(&Path),
    pub files: &'a dyn GalleryFiles,
    pub edit: &'a dyn Fn(&Path, &mut App),
}

/// Answers a click on a capture, recording or export notification; any other is not ours.
pub fn on_action(id: NotificationId, action: &str, answers: &Answers, cx: &mut App) {
    match notice_action(id, action, cx) {
        Some(NoticeAction::Open(file)) => (answers.open)(&file),
        Some(NoticeAction::Edit(file)) => (answers.edit)(&file, cx),
        Some(NoticeAction::ShowInFolder(file)) => {
            let shown = answers.files.show_in_folder(file);
            cx.background_executor()
                .spawn(async move {
                    if let Err(e) = shown.await {
                        tracing::warn!(error = %e, "show in folder");
                    }
                })
                .detach();
        }
        Some(NoticeAction::Undo(undo)) => undo.run(cx),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use es_app::toast::{
        EDIT_ACTION, IconName, MemorySink, SHOW_IN_FOLDER_ACTION, Toast, install_toasts, show_toast,
    };
    use es_desktop::{DEFAULT_ACTION, RecordingNotifier};
    use futures::future::BoxFuture;
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Files(Mutex<Vec<PathBuf>>);

    impl GalleryFiles for Files {
        fn show_in_folder(&self, file: PathBuf) -> BoxFuture<'static, Result<(), String>> {
            self.0.lock().unwrap().push(file);
            Box::pin(async { Ok(()) })
        }
        fn trash(&self, _: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    #[gpui::test]
    fn each_button_on_a_capture_notice_reaches_its_file(cx: &mut gpui::TestAppContext) {
        let notifier = Arc::new(RecordingNotifier::default());
        let file = PathBuf::from("/shots/one.png");
        let (opened, edited, files) = (
            RefCell::new(Vec::new()),
            RefCell::new(Vec::new()),
            Files::default(),
        );
        cx.update(|cx| {
            install_toasts(MemorySink::without_surface(), notifier.clone(), cx);
            let toast = Toast::new("toast-saved", IconName::Check).arg("path", "/shots/one.png");
            show_toast(es_app::pipeline::capture_notice(toast, &file), cx);
            let sent = notifier.sent();
            let keys: Vec<&str> = sent[0].actions.iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, [DEFAULT_ACTION, SHOW_IN_FOLDER_ACTION, EDIT_ACTION]);
            assert!(sent[0].body.contains("/shots/one.png"), "a save says where");
            let answers = Answers {
                open: &|p| opened.borrow_mut().push(p.to_path_buf()),
                files: &files,
                edit: &|p, _| edited.borrow_mut().push(p.to_path_buf()),
            };
            let id = NotificationId(1);
            on_action(id, DEFAULT_ACTION, &answers, cx);
            on_action(id, EDIT_ACTION, &answers, cx);
            on_action(id, SHOW_IN_FOLDER_ACTION, &answers, cx);
            on_action(NotificationId(2), DEFAULT_ACTION, &answers, cx);
        });
        assert_eq!(*opened.borrow(), std::slice::from_ref(&file));
        assert_eq!(*edited.borrow(), std::slice::from_ref(&file));
        assert_eq!(*files.0.lock().unwrap(), [file]);
    }

    #[gpui::test]
    fn a_video_notice_opens_but_does_not_offer_the_image_editor(cx: &mut gpui::TestAppContext) {
        let notifier = Arc::new(RecordingNotifier::default());
        let edited = RefCell::new(0);
        cx.update(|cx| {
            install_toasts(MemorySink::without_surface(), notifier.clone(), cx);
            let toast = Toast::new("toast-recording-saved", IconName::Check)
                .arg("path", "/videos/a.mp4")
                .file("/videos/a.mp4".into());
            show_toast(toast, cx);
            let keys: Vec<String> = notifier.sent()[0]
                .actions
                .iter()
                .map(|(k, _)| k.clone())
                .collect();
            assert_eq!(keys, [DEFAULT_ACTION, SHOW_IN_FOLDER_ACTION]);
            let answers = Answers {
                open: &|_| {},
                files: &Files::default(),
                edit: &|_, _| *edited.borrow_mut() += 1,
            };
            on_action(NotificationId(1), EDIT_ACTION, &answers, cx);
        });
        assert_eq!(*edited.borrow(), 0);
    }
}
