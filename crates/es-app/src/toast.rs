use crate::strings::loader;
use es_desktop::{DEFAULT_ACTION, Notification, NotificationId, Notifier, Urgency};
use es_model::platform::OutputId;
use gpui::{App, BorrowAppContext, Global};
use i18n_embed::fluent::FluentLanguageLoader;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub use gpui_component::IconName;

/// `i18n_embed::fluent` does not re-export `FluentArgs`; this is what `FluentLanguageLoader::get_args` accepts.
pub type FluentArgs = HashMap<&'static str, String>;

pub const TOAST_DURATION: Duration = Duration::from_millis(2500);

pub const EDIT_ACTION: &str = "edit";
pub const SHOW_IN_FOLDER_ACTION: &str = "show-in-folder";

const REMEMBERED_NOTICES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStyle {
    Normal,
    Error,
}

#[derive(Clone)]
pub struct Toast {
    pub title_id: &'static str,
    pub message: FluentArgs,
    pub icon: IconName,
    pub style: ToastStyle,
    /// 2.5 s by default.
    pub duration: Duration,
    pub output: Option<OutputId>,
    /// The saved file; a notification shown instead of the toast opens it.
    pub file: Option<PathBuf>,
    /// The notification also offers to open `file` in the editor.
    pub edit: bool,
}

impl Toast {
    pub fn new(title_id: &'static str, icon: IconName) -> Self {
        Self {
            title_id,
            message: FluentArgs::new(),
            icon,
            style: ToastStyle::Normal,
            duration: TOAST_DURATION,
            output: None,
            file: None,
            edit: false,
        }
    }

    pub fn error(title_id: &'static str) -> Self {
        Self {
            style: ToastStyle::Error,
            ..Self::new(title_id, IconName::TriangleAlert)
        }
    }

    pub fn arg(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.message.insert(name, value.into());
        self
    }

    pub fn on(mut self, output: OutputId) -> Self {
        self.output = Some(output);
        self
    }

    pub fn file(mut self, file: PathBuf) -> Self {
        self.file = Some(file);
        self
    }

    pub fn editable(mut self) -> Self {
        self.edit = true;
        self
    }
}

/// A toast with its strings looked up, ready for a surface to draw.
#[derive(Clone)]
pub struct ToastRequest {
    pub title: String,
    pub message: Option<String>,
    pub icon: IconName,
    pub style: ToastStyle,
    pub duration: Duration,
    pub output: Option<OutputId>,
}

impl ToastRequest {
    pub fn resolve(toast: &Toast) -> Self {
        Self::resolve_in(loader(), toast)
    }

    fn resolve_in(l: &FluentLanguageLoader, toast: &Toast) -> Self {
        let title = if l.has(toast.title_id) {
            l.get_args(toast.title_id, toast.message.clone())
        } else {
            toast.title_id.to_owned()
        };
        let message_id = format!("{}-message", toast.title_id);
        let message = if l.has(&message_id) {
            Some(l.get_args(&message_id, toast.message.clone()))
        } else {
            toast.message.get("error").cloned()
        };
        Self {
            title,
            message,
            icon: toast.icon.clone(),
            style: toast.style,
            duration: toast.duration,
            output: toast.output.clone(),
        }
    }
}

/// Returns false when no surface is visible to draw on.
pub trait ToastSink {
    fn show(&self, request: &ToastRequest, cx: &mut App) -> bool;
}

#[derive(Default)]
pub struct MemorySink {
    shown: Mutex<Vec<ToastRequest>>,
    hidden: bool,
}

impl MemorySink {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// A sink that has no surface, so every toast falls back to a notification.
    pub fn without_surface() -> Arc<Self> {
        Arc::new(Self {
            hidden: true,
            ..Self::default()
        })
    }

    pub fn shown(&self) -> Vec<ToastRequest> {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ToastSink for Arc<MemorySink> {
    fn show(&self, request: &ToastRequest, _cx: &mut App) -> bool {
        if self.hidden {
            return false;
        }
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        true
    }
}

struct Sent {
    id: NotificationId,
    file: PathBuf,
    edit: bool,
}

struct ToastRouter {
    sink: Box<dyn ToastSink>,
    notifier: Arc<dyn Notifier>,
    sent: VecDeque<Sent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAction {
    Open(PathBuf),
    ShowInFolder(PathBuf),
    Edit(PathBuf),
}

fn notice_actions(t: &Toast) -> Vec<(String, String)> {
    if t.file.is_none() {
        return Vec::new();
    }
    let l = loader();
    let mut actions = vec![
        (DEFAULT_ACTION.to_owned(), l.get("notice-open")),
        (
            SHOW_IN_FOLDER_ACTION.to_owned(),
            l.get("notice-show-in-folder"),
        ),
    ];
    if t.edit {
        actions.push((EDIT_ACTION.to_owned(), l.get("notice-edit")));
    }
    actions
}

/// What a click on a notification that stood in for a toast asks for; `None` when it is not one.
pub fn notice_action(id: NotificationId, action: &str, cx: &App) -> Option<NoticeAction> {
    let router = cx.try_global::<ToastRouter>()?;
    let sent = router.sent.iter().find(|s| s.id == id)?;
    let file = sent.file.clone();
    match action {
        DEFAULT_ACTION => Some(NoticeAction::Open(file)),
        SHOW_IN_FOLDER_ACTION => Some(NoticeAction::ShowInFolder(file)),
        EDIT_ACTION if sent.edit => Some(NoticeAction::Edit(file)),
        _ => None,
    }
}

impl Global for ToastRouter {}

pub fn install_toasts(sink: impl ToastSink + 'static, notifier: Arc<dyn Notifier>, cx: &mut App) {
    cx.set_global(ToastRouter {
        sink: Box::new(sink),
        notifier,
        sent: VecDeque::new(),
    });
}

#[allow(clippy::needless_pass_by_value)]
pub fn show_toast(t: Toast, cx: &mut App) {
    let request = ToastRequest::resolve(&t);
    let Some(router) = cx.try_global::<ToastRouter>() else {
        tracing::warn!(title = %request.title, "toast dropped: no toast router installed");
        return;
    };
    let notifier = router.notifier.clone();
    let shown = cx.update_global(|router: &mut ToastRouter, cx| router.sink.show(&request, cx));
    if shown {
        return;
    }
    let urgency = match request.style {
        ToastStyle::Error => Urgency::Normal,
        ToastStyle::Normal => Urgency::Low,
    };
    let note = Notification {
        title: request.title,
        body: request.message.unwrap_or_default(),
        icon: None,
        image: None,
        actions: notice_actions(&t),
        urgency,
        timeout: Some(request.duration),
    };
    match notifier.notify(note) {
        Ok(id) => {
            if let Some(file) = t.file {
                cx.update_global(|router: &mut ToastRouter, _| {
                    if router.sent.len() == REMEMBERED_NOTICES {
                        router.sent.pop_front();
                    }
                    router.sent.push_back(Sent {
                        id,
                        file,
                        edit: t.edit,
                    });
                });
            }
        }
        Err(error) => tracing::warn!(%error, "toast notification failed"),
    }
}
