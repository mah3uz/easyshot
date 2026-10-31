use gpui::{App, AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, WindowOptions, div, px};
use gpui_component::{ActiveTheme, Icon, Root, button::{Button, ButtonVariants}};

use crate::{fl, fonts, theme};

pub fn init(cx: &mut App) {
    fonts::register(cx);
    theme::install(cx);
}

pub fn root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    let view = cx.new(|_| HelloView);
    cx.new(|cx| Root::new(view, window, cx))
}

pub fn open(cx: &mut App) {
    init(cx);
    if let Err(error) = cx.open_window(WindowOptions::default(), root) {
        tracing::error!(%error, "cannot open the hello window");
    }
    cx.activate(true);
}

pub struct HelloView;

impl Render for HelloView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .justify_center()
            .gap_4()
            .p_6()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(div().text_xl().child(fl!("ui-shortcut-help", title = "Hello easyshot", shortcut = "Ctrl+Shift+4")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(Icon::empty().path("icons/lucide/camera.svg").size(px(28.)))
                    .child(Button::new("hello").primary().label(fl!("ui-copy"))),
            )
            .child(div().font_family("Inter").text_lg().child("Inter: Sphinx of black quartz"))
            .child(div().font_family("Nunito").text_lg().child("Nunito: Sphinx of black quartz"))
    }
}
