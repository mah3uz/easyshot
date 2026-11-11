pub mod a11y;
pub mod assets;
pub mod components;
pub mod convert;
pub mod cursors;
pub mod fonts;
pub mod hello;
pub mod icons;
pub mod motion;
pub mod strings;
pub mod theme;
pub mod tokens;

pub fn init(cx: &mut gpui::App) {
    fonts::register(cx);
    theme::install(cx);
    a11y::bind_focus_keys(cx);
    components::share_popover::bind_keys(cx);
    // Letters pick tools and Space is held while dragging, so a key press must not hide the pointer.
    cx.set_cursor_hide_mode(gpui::CursorHideMode::Never);
}
