use es_protocol::{ShortcutEntry, Trigger};
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton},
    tokens::Tokens,
};
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::ActiveTheme;

use super::ShortcutSetup;
use super::rows::{caption, form_row, group, row_title};
use super::view::{Onboarding, OnboardingEvent};
use crate::settings::setup;
use crate::settings::status_mark;
use crate::strings::{fl, loader};

#[derive(Default)]
pub(super) struct State {
    suggestions: Vec<(&'static ShortcutEntry, Option<Trigger>)>,
    saved: bool,
    included: Option<Result<bool, String>>,
    reloaded: Option<Result<(), String>>,
    file_error: Option<String>,
    copied: bool,
}

impl Onboarding {
    pub(super) fn enter_shortcuts(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.input.binds.clone() else {
            return;
        };
        let existing = binds.existing_binds().unwrap_or_default();
        let s = &mut self.shortcuts;
        s.suggestions = setup::suggest(binds.as_ref(), &existing, &self.input.global_shortcuts);
        s.file_error =
            setup::ensure_binds_file(binds.as_ref(), &self.paths, &self.input.global_shortcuts)
                .err()
                .map(|e| e.to_string());
        self.check_include(cx);
    }

    /// Each suggested action with the key it would get; `None` when every candidate is taken.
    pub fn suggested(&self) -> Vec<(&'static str, Option<Trigger>)> {
        let list = &self.shortcuts.suggestions;
        list.iter().map(|(e, t)| (e.id, t.clone())).collect()
    }

    pub fn include_line(&self) -> Option<String> {
        let binds = self.input.binds.as_ref()?;
        Some(setup::include_line(binds.as_ref(), &self.paths))
    }

    pub fn included(&self) -> Option<&Result<bool, String>> {
        self.shortcuts.included.as_ref()
    }

    pub fn shortcuts_saved(&self) -> bool {
        self.shortcuts.saved
    }

    /// Writes the binds file with the suggested keys and asks for them to be saved in the config.
    pub fn use_suggestions(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.input.binds.clone() else {
            return;
        };
        let globals =
            setup::globals_with(&self.input.global_shortcuts, &self.shortcuts.suggestions);
        match setup::write_binds_file(binds.as_ref(), &self.paths, &globals) {
            Ok(_) => {
                self.shortcuts.file_error = None;
                self.shortcuts.saved = true;
                self.input.global_shortcuts = globals.clone();
                cx.emit(OnboardingEvent::SetShortcuts(globals));
            }
            Err(error) => self.shortcuts.file_error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub fn copy_include_line(&mut self, cx: &mut Context<Self>) {
        if let (Some(line), Some(clipboard)) = (self.include_line(), self.input.clipboard.clone()) {
            clipboard.copy(line, cx);
            self.shortcuts.copied = true;
            cx.notify();
        }
    }

    pub fn check_include(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.input.binds.clone() else {
            return;
        };
        self.shortcuts.included = None;
        let dir = self.paths.config_dir().to_path_buf();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { binds.is_included(&dir) })
                .await;
            this.update(cx, |this, cx| {
                this.shortcuts.included = Some(found.map_err(|e| e.to_string()));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn reload_compositor(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.input.binds.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move { binds.reload() })
                .await;
            this.update(cx, |this, cx| {
                this.shortcuts.reloaded = Some(done.map_err(|e| e.to_string()));
                this.check_include(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn shortcuts_step(&self, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let page = div().flex().flex_col().gap(px(16.)).w_full();
        let body = match ShortcutSetup::for_caps(self.input.caps, self.input.binds.is_some()) {
            Some(ShortcutSetup::Portal) => {
                return page.child(caption(t, fl!("onboarding-shortcuts-portal")));
            }
            None => return page.child(caption(t, fl!("onboarding-shortcuts-none"))),
            Some(ShortcutSetup::Binds) => self.binds_body(t, cx),
        };
        page.children(body)
    }

    fn binds_body(&self, t: &Tokens, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let s = &self.shortcuts;
        let Some(binds) = self.input.binds.clone() else {
            return Vec::new();
        };
        let mut list = group(t);
        for (i, (entry, trigger)) in s.suggestions.iter().enumerate() {
            let key = match trigger {
                Some(trigger) => div().child(trigger.label()),
                None => div()
                    .text_color(t.text_tertiary)
                    .child(fl!("onboarding-shortcuts-none-free")),
            };
            list = list.child(
                form_row(t, i > 0)
                    .child(row_title(loader().get(entry.title_id)))
                    .child(div().flex_1())
                    .child(key.text_size(px(13.))),
            );
        }
        let any = s.suggestions.iter().any(|(_, t)| t.is_some());
        let entity = cx.entity().downgrade();
        let use_button = EditorButton::new("onboarding-use-shortcuts")
            .label(if s.saved {
                fl!("onboarding-shortcuts-saved")
            } else {
                fl!("onboarding-shortcuts-use")
            })
            .kind(ButtonKind::Prominent)
            .disabled(!any || s.saved)
            .on_click(move |_, _, cx| {
                let _ = entity.update(cx, |o, cx| o.use_suggestions(cx));
            });

        let file = format!("{}/{}", setup::display_dir(&self.paths), binds.file_name());
        let line = self.include_line().unwrap_or_default();
        let code = div()
            .id("onboarding-include-line")
            .accessible(Role::Code, fl!("shortcuts-include-a11y"))
            .p(px(10.))
            .rounded(px(8.))
            .bg(t.window)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .whitespace_nowrap()
            .overflow_x_scroll()
            .child(line);

        let (copy, check, reload) = (
            cx.entity().downgrade(),
            cx.entity().downgrade(),
            cx.entity().downgrade(),
        );
        let mut buttons = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                EditorButton::new("onboarding-copy-include")
                    .label(if s.copied {
                        fl!("shortcuts-copied")
                    } else {
                        fl!("shortcuts-copy-line")
                    })
                    .kind(ButtonKind::Bordered)
                    .on_click(move |_, _, cx| {
                        let _ = copy.update(cx, |o, cx| o.copy_include_line(cx));
                    }),
            )
            .child(
                EditorButton::new("onboarding-check-include")
                    .label(fl!("shortcuts-check"))
                    .kind(ButtonKind::Bordered)
                    .on_click(move |_, _, cx| {
                        let _ = check.update(cx, |o, cx| o.check_include(cx));
                    }),
            );
        if binds.needs_reload() {
            buttons = buttons.child(
                EditorButton::new("onboarding-reload")
                    .label(fl!("shortcuts-reload"))
                    .kind(ButtonKind::Bordered)
                    .on_click(move |_, _, cx| {
                        let _ = reload.update(cx, |o, cx| o.reload_compositor(cx));
                    }),
            );
        }
        let status = match &s.included {
            Some(Ok(true)) => Some(status_mark(true, fl!("shortcuts-included"), cx)),
            Some(Ok(false)) => Some(status_mark(false, fl!("shortcuts-not-found"), cx)),
            _ => None,
        };
        let problem = match (&s.file_error, &s.included, &s.reloaded) {
            (Some(error), ..) => Some(fl!("shortcuts-file-failed", error = error.clone())),
            (_, Some(Err(error)), _) => {
                Some(fl!("shortcuts-include-failed", error = error.clone()))
            }
            (.., Some(Err(error))) => Some(fl!("shortcuts-reload-failed", error = error.clone())),
            _ => None,
        };
        let include = group(t).child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .px(px(14.))
                .py(px(12.))
                .child(row_title(fl!("onboarding-shortcuts-include")))
                .child(caption(
                    t,
                    fl!("onboarding-shortcuts-include-help", file = file),
                ))
                .child(code)
                .child(buttons)
                .children(status)
                .children(problem.map(|p| caption(t, p))),
        );
        vec![
            list.into_any_element(),
            div()
                .flex()
                .justify_end()
                .child(use_button)
                .into_any_element(),
            include.into_any_element(),
        ]
    }
}
