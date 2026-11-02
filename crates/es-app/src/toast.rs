use crate::strings::loader;
use es_desktop::{Notification, Notifier, Urgency};
use es_model::platform::OutputId;
use gpui::{App, BorrowAppContext, Global};
use i18n_embed::fluent::FluentLanguageLoader;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub use gpui_component::IconName;

/// `i18n_embed::fluent` does not re-export `FluentArgs`; this is what `FluentLanguageLoader::get_args` accepts.
pub type FluentArgs = HashMap<&'static str, String>;

pub const TOAST_DURATION: Duration = Duration::from_millis(2500);

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
            l.get(toast.title_id)
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

struct ToastRouter {
    sink: Box<dyn ToastSink>,
    notifier: Arc<dyn Notifier>,
}

impl Global for ToastRouter {}

pub fn install_toasts(sink: impl ToastSink + 'static, notifier: Arc<dyn Notifier>, cx: &mut App) {
    cx.set_global(ToastRouter {
        sink: Box::new(sink),
        notifier,
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
        actions: Vec::new(),
        urgency,
        timeout: Some(request.duration),
    };
    if let Err(error) = notifier.notify(note) {
        tracing::warn!(%error, "toast notification failed");
    }
}
