use es_model::platform::OutputId;
use gpui::App;
use std::collections::HashMap;
use std::time::Duration;

pub use gpui_component::IconName;

/// `i18n_embed::fluent` does not re-export `FluentArgs`; this is what `FluentLanguageLoader::get_args` accepts.
pub type FluentArgs = HashMap<&'static str, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStyle {
    Normal,
    Error,
}

pub struct Toast {
    pub title_id: &'static str,
    pub message: FluentArgs,
    pub icon: IconName,
    pub style: ToastStyle,
    /// 2.5 s by default.
    pub duration: Duration,
    pub output: Option<OutputId>,
}

pub fn show_toast(_t: Toast, _cx: &mut App) {
    todo!()
}
