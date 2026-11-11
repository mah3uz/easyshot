use super::{Page, SettingsWindow, widgets};
use es_ui::motion::{PAGE_RISE, progress};
use es_ui::theme::tokens;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::h_flex;

impl SettingsWindow {
    fn render_page(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut children = self.banners(cx);
        children.extend(match page {
            Page::Home => self.render_home(window, cx),
            Page::General => self.render_general(cx),
            Page::Capture => self.render_capture(cx),
            Page::Overlay => self.render_overlay(cx),
            Page::Recording => self.render_recording(cx),
            Page::Shortcuts => self.render_shortcuts(window, cx),
            Page::Agents => self.render_agents(cx),
            Page::Sharing => self.render_sharing(cx),
            Page::About => self.render_about(cx),
        });
        widgets::page(page.title().into(), children, cx).into_any_element()
    }
}

fn scroller(page: Page) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("page-{page:?}")))
        .absolute()
        .inset_0()
        .overflow_y_scroll()
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (arrived, from) = match self.page_motion {
            Some((clock, from)) => (progress(Some(clock), window, cx), from),
            None => (1.0, self.page),
        };
        let leaving = (arrived < 1.0 && from != self.page).then(|| {
            scroller(from)
                .opacity(1.0 - arrived)
                .child(self.render_page(from, window, cx))
        });
        let page = self.page;
        let arriving = self.render_page(page, window, cx);
        let t = tokens(cx);
        h_flex()
            .size_full()
            .font_family(es_ui::theme::UI_FONT)
            .bg(widgets::page_fill(t))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .children(leaving)
                    .child(
                        scroller(page).child(
                            div()
                                .relative()
                                .top(PAGE_RISE * (1.0 - arrived))
                                .opacity(arrived)
                                .child(arriving),
                        ),
                    ),
            )
    }
}
