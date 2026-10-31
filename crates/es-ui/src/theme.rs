use gpui::App;
use gpui_component::{Theme, ThemeMode};

pub const UI_FONT: &str = "Inter";

pub fn install(cx: &mut App) {
    gpui_component::init(cx);
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = UI_FONT.into();
    cx.refresh_windows();
}
