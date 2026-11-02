use super::widgets::{caption, card_fill, group, row, status_mark};
use super::{Page, SettingsWindow, SettingsWindowEvent};
use crate::strings::fl;
use es_backend::Caps;
use es_protocol::Route;
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton},
    icons::IconName,
    theme::tokens,
};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::{Icon, h_flex, v_flex};

pub const SOURCE_URL: &str = "https://github.com/mah3uz/easyshot";
pub const ISSUES_URL: &str = "https://github.com/mah3uz/easyshot/issues";

const TILES: [(&str, IconName, Route); 6] = [
    (
        "capture-region",
        IconName::SquareDashed,
        Route::CaptureRegion,
    ),
    (
        "capture-fullscreen",
        IconName::Monitor,
        Route::CaptureFullscreen,
    ),
    ("capture-window", IconName::AppWindow, Route::CaptureWindow),
    (
        "capture-scroll",
        IconName::UnfoldVertical,
        Route::CaptureScroll,
    ),
    ("record", IconName::CircleDot, Route::Record),
    ("ocr", IconName::ScanText, Route::Ocr),
];

/// The Setup checklist: what each capability gives the user.
pub fn capability_rows() -> Vec<(Caps, String, String)> {
    vec![
        (
            Caps::LAYER_SHELL,
            fl!("home-cap-overlay"),
            fl!("home-cap-overlay-help"),
        ),
        (
            Caps::CAPTURE_OUTPUT,
            fl!("home-cap-screen"),
            fl!("home-cap-screen-help"),
        ),
        (
            Caps::CAPTURE_WINDOW,
            fl!("home-cap-window"),
            fl!("home-cap-window-help"),
        ),
        (
            Caps::PORTAL_SHORTCUTS | Caps::SHORTCUT_TRIGGERS,
            fl!("home-cap-shortcuts"),
            fl!("home-cap-shortcuts-help"),
        ),
        (
            Caps::RECORDER,
            fl!("home-cap-recorder"),
            fl!("home-cap-recorder-help"),
        ),
        (Caps::OCR, fl!("home-cap-ocr"), fl!("home-cap-ocr-help")),
        (
            Caps::CLIPBOARD_NO_FOCUS,
            fl!("home-cap-clipboard"),
            fl!("home-cap-clipboard-help"),
        ),
        (Caps::TRAY, fl!("home-cap-tray"), fl!("home-cap-tray-help")),
        (
            Caps::POINTER_GLOBAL,
            fl!("home-cap-pointer"),
            fl!("home-cap-pointer-help"),
        ),
        (
            Caps::FILE_PICKER,
            fl!("home-cap-file-picker"),
            fl!("home-cap-file-picker-help"),
        ),
    ]
}

fn tile_title(id: &str) -> String {
    match id {
        "capture-region" => fl!("home-tile-region"),
        "capture-fullscreen" => fl!("home-tile-screen"),
        "capture-window" => fl!("home-tile-window"),
        "capture-scroll" => fl!("home-tile-scrolling"),
        "record" => fl!("home-tile-record"),
        _ => fl!("home-tile-text"),
    }
}

impl SettingsWindow {
    /// Each Setup row's title and whether `Caps` offers it, in display order.
    pub fn setup_checklist(&self) -> Vec<(String, bool)> {
        capability_rows()
            .into_iter()
            .map(|(caps, title, _)| (title, self.caps.intersects(caps)))
            .collect()
    }

    pub(super) fn render_home(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = tokens(cx);
        let mut grid = div().grid().grid_cols(3).gap(px(10.));
        for (id, icon, route) in TILES {
            let title = tile_title(id);
            let shortcut = self
                .shortcuts
                .global_trigger(id)
                .map(|t| t.label())
                .unwrap_or_else(|| fl!("home-tile-no-shortcut"));
            grid = grid.child(
                v_flex()
                    .id(SharedString::from(format!("tile-{id}")))
                    .accessible(
                        Role::Button,
                        fl!(
                            "home-tile-a11y",
                            title = title.clone(),
                            shortcut = shortcut.clone()
                        ),
                    )
                    .p(px(12.))
                    .gap(px(8.))
                    .rounded(px(12.))
                    .bg(card_fill(t))
                    .border_1()
                    .border_color(t.border)
                    .hover(|d| d.bg(t.hover_fill))
                    .child(Icon::new(icon).size(px(18.)).text_color(t.text_secondary))
                    .child(
                        v_flex()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(t.text_tertiary)
                                    .child(shortcut),
                            ),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(SettingsWindowEvent::Run(route.clone()))
                    })),
            );
        }

        let recent: Vec<_> = self
            .history
            .as_ref()
            .map(|h| h.read(cx).entries().iter().rev().take(5).cloned().collect())
            .unwrap_or_default();
        let recent_rows = if recent.is_empty() {
            vec![
                v_flex()
                    .items_center()
                    .py(px(20.))
                    .gap(px(4.))
                    .child(
                        Icon::new(IconName::Images)
                            .size(px(22.))
                            .text_color(t.text_tertiary),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.text)
                            .child(fl!("home-recent-empty-title")),
                    )
                    .child(caption(fl!("home-recent-empty-body"), cx))
                    .into_any_element(),
            ]
        } else {
            recent
                .into_iter()
                .map(|entry| {
                    let file = entry.file.clone();
                    let when = es_store::datefmt::format(entry.created_at, "yyyy-MM-dd HH:mm");
                    h_flex()
                        .id(SharedString::from(format!("recent-{}", entry.id)))
                        .accessible(
                            Role::Button,
                            fl!("home-recent-open-a11y", name = entry.name.clone()),
                        )
                        .px(px(16.))
                        .py(px(8.))
                        .gap(px(10.))
                        .hover(|d| d.bg(t.hover_fill))
                        .child(
                            Icon::new(IconName::Image)
                                .size(px(16.))
                                .text_color(t.text_tertiary),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .text_color(t.text)
                                .child(entry.name),
                        )
                        .child(caption(when, cx))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(SettingsWindowEvent::OpenRecent(file.clone()))
                        }))
                        .into_any_element()
                })
                .collect()
        };

        let setup = capability_rows()
            .into_iter()
            .map(|(caps, title, help)| {
                let ok = self.caps.intersects(caps);
                let label = if ok {
                    fl!("home-setup-available")
                } else {
                    fl!("home-setup-missing")
                };
                row(title, Some(help.into()), status_mark(ok, label, cx), cx)
            })
            .collect();

        vec![
            v_flex()
                .gap(px(8.))
                .child(self.home_header(
                    fl!("home-header-capture"),
                    fl!("home-link-shortcuts"),
                    Page::Shortcuts,
                    cx,
                ))
                .child(grid)
                .into_any_element(),
            v_flex()
                .gap(px(8.))
                .child(self.home_header(
                    fl!("home-header-recent"),
                    fl!("home-link-gallery"),
                    Page::Home,
                    cx,
                ))
                .child(group(None, recent_rows, None, cx))
                .into_any_element(),
            group(
                Some(fl!("home-group-setup").into()),
                setup,
                Some(fl!("home-setup-footer").into()),
                cx,
            )
            .into_any_element(),
        ]
    }

    /// `target` Home means the link opens the gallery instead of a page.
    fn home_header(
        &self,
        title: String,
        link: String,
        target: Page,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = tokens(cx);
        h_flex()
            .px(px(4.))
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_secondary)
                    .child(title),
            )
            .child(
                h_flex()
                    .id(SharedString::from(format!("link-{link}")))
                    .accessible(Role::Link, link.clone())
                    .gap(px(2.))
                    .text_size(px(12.))
                    .text_color(t.text_secondary)
                    .hover(|d| d.text_color(t.text))
                    .child(link)
                    .child(Icon::new(IconName::ChevronRight).size(px(12.)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if target == Page::Home {
                            cx.emit(SettingsWindowEvent::Run(Route::Gallery));
                        } else {
                            this.show(target, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_about(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let t = tokens(cx);
        let link = |id: &'static str, label: String, url: &'static str, cx: &mut Context<Self>| {
            EditorButton::new(id)
                .label(label)
                .kind(ButtonKind::Bordered)
                .on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(SettingsWindowEvent::OpenUrl(url))),
                )
        };
        vec![
            v_flex()
                .items_center()
                .gap(px(6.))
                .py(px(8.))
                .child(Icon::new(IconName::Scan).size(px(56.)).text_color(t.accent))
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(fl!("about-title")),
                )
                .child(caption(
                    fl!("about-version-short", version = self.version.clone()),
                    cx,
                ))
                .child(
                    caption(fl!("about-tagline"), cx)
                        .max_w(px(480.))
                        .text_center(),
                )
                .into_any_element(),
            group(
                Some(fl!("about-onboarding").into()),
                vec![row(
                    fl!("about-tour"),
                    Some(fl!("about-onboarding-help").into()),
                    EditorButton::new("show-onboarding")
                        .label(fl!("about-onboarding-button"))
                        .kind(ButtonKind::Bordered)
                        .on_click(
                            cx.listener(|_, _, _, cx| cx.emit(SettingsWindowEvent::ShowOnboarding)),
                        ),
                    cx,
                )],
                Some(fl!("about-packaging-footer").into()),
                cx,
            )
            .into_any_element(),
            group(
                Some(fl!("about-open-source").into()),
                vec![
                    row(
                        fl!("about-source"),
                        Some(fl!("about-source-help").into()),
                        link("source", fl!("about-source-link"), SOURCE_URL, cx),
                        cx,
                    ),
                    row(
                        fl!("about-feedback"),
                        Some(fl!("about-feedback-help").into()),
                        link("feedback", fl!("about-feedback-link"), ISSUES_URL, cx),
                        cx,
                    ),
                ],
                Some(fl!("about-license-footer").into()),
                cx,
            )
            .into_any_element(),
        ]
    }
}
