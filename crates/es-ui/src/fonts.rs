use gpui::App;
use std::borrow::Cow;

pub fn register(cx: &mut App) {
    let data = es_text::BUNDLED.iter().map(|b| Cow::Borrowed(*b)).collect();
    if let Err(error) = cx.text_system().add_fonts(data) {
        tracing::error!(%error, "cannot register bundled fonts");
    }
}
