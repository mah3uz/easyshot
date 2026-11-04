use super::binds::{BindStatus, compositor_trigger, status, trigger_string};
use super::setup::{self, route_of};
use super::widgets::{group, row, status_mark};
use super::{ClipboardSink, SettingsWindow};
use crate::strings::{fl, loader};
use es_backend::{BindHelper, BindSyntax, Trigger as CompositorTrigger};
use es_model::Config;
use es_protocol::{CATALOGUE, Route, Scope, ShortcutEntry, Trigger};
use es_store::paths::Paths;
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton, ShortcutField, ShortcutFieldEvent},
    theme::tokens,
};
use gpui::{
    AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Styled, Subscription, Window, div, px,
};
use gpui_component::{ActiveTheme, h_flex};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct ShortcutRow {
    pub entry: &'static ShortcutEntry,
    pub field: Entity<ShortcutField>,
}

pub(super) struct Shortcuts {
    binds: Option<Arc<dyn BindHelper>>,
    paths: Paths,
    clipboard: Rc<dyn ClipboardSink>,
    rows: Vec<ShortcutRow>,
    global: BTreeMap<&'static str, Trigger>,
    status: BTreeMap<&'static str, BindStatus>,
    check: Option<Result<(usize, usize), String>>,
    included: Option<Result<bool, String>>,
    reloaded: Option<Result<(), String>>,
    file_error: Option<String>,
    copied: bool,
    _subscriptions: Vec<Subscription>,
}

fn group_of(id: &str) -> usize {
    const TOOLS: [&str; 15] = [
        "image-background",
        "image-select",
        "image-crop",
        "image-rectangle",
        "image-filled-rectangle",
        "image-ellipse",
        "image-line",
        "image-arrow",
        "image-freehand",
        "image-counter",
        "image-text",
        "image-highlight",
        "image-highlighter",
        "image-pixelate",
        "image-blur",
    ];
    match id {
        "ocr" | "ocr-single-line" | "color-picker" => 2,
        _ if id.starts_with("capture-") && id != "capture-bar" => 1,
        _ if id.starts_with("record") => 3,
        _ if id.starts_with("deck-") => 4,
        _ if TOOLS.contains(&id) => 5,
        _ if id.starts_with("image-") => 6,
        _ if id.starts_with("video-") => 7,
        _ => 0,
    }
}

fn group_title(index: usize) -> String {
    match index {
        0 => fl!("shortcuts-group-general"),
        1 => fl!("shortcuts-group-screenshots"),
        2 => fl!("shortcuts-group-ocr-color"),
        3 => fl!("shortcuts-group-recording"),
        4 => fl!("shortcuts-group-deck"),
        5 => fl!("shortcuts-group-image-tools"),
        6 => fl!("shortcuts-group-image-editing"),
        _ => fl!("shortcuts-group-video-editing"),
    }
}

fn syntax_name(syntax: BindSyntax) -> String {
    match syntax {
        BindSyntax::HyprLua => fl!("shortcuts-syntax-hypr-lua"),
        BindSyntax::Hyprlang => fl!("shortcuts-syntax-hyprlang"),
        BindSyntax::Sway => fl!("shortcuts-syntax-sway"),
        BindSyntax::NiriKdl => fl!("shortcuts-syntax-niri"),
    }
}

/// An editor override of "" means the user cleared the default.
fn editor_trigger(entry: &ShortcutEntry, config: &Config) -> Option<Trigger> {
    match config.shortcuts.get(entry.id) {
        Some(value) => Trigger::parse(value).ok(),
        None => entry.default_trigger.and_then(|t| Trigger::parse(t).ok()),
    }
}

impl Shortcuts {
    pub(super) fn new(
        config: &Config,
        binds: Option<Arc<dyn BindHelper>>,
        paths: Paths,
        clipboard: Rc<dyn ClipboardSink>,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> Self {
        let mut rows = Vec::new();
        let mut subs = Vec::new();
        for entry in CATALOGUE
            .iter()
            .filter(|e| e.scope == Scope::Editor || route_of(e).is_some())
        {
            let trigger = match entry.scope {
                Scope::Global => None,
                Scope::Editor => editor_trigger(entry, config),
            };
            let title = loader().get(entry.title_id);
            let field = cx.new(|cx| {
                let mut field =
                    ShortcutField::new(fl!("shortcuts-recorder-a11y", title = title), window, cx);
                field.set_trigger(trigger, cx);
                field.set_clears_on_delete(entry.scope == Scope::Global, cx);
                field
            });
            subs.push(
                cx.subscribe(&field, move |this: &mut SettingsWindow, _, event, cx| {
                    this.on_shortcut(entry, event, cx)
                }),
            );
            rows.push(ShortcutRow { entry, field });
        }
        let global: BTreeMap<&'static str, Trigger> = CATALOGUE
            .iter()
            .filter(|e| e.scope == Scope::Global)
            .filter_map(|e| {
                Some((
                    e.id,
                    Trigger::parse(config.global_shortcuts.get(e.id)?).ok()?,
                ))
            })
            .collect();
        for row in &rows {
            if let Some(t) = global.get(row.entry.id) {
                let t = t.clone();
                row.field.update(cx, |f, cx| f.set_trigger(Some(t), cx));
            }
        }
        let file_error = binds.as_ref().and_then(|b| {
            setup::ensure_binds_file(b.as_ref(), &paths, &config.global_shortcuts)
                .err()
                .map(|e| e.to_string())
        });
        Self {
            binds,
            paths,
            clipboard,
            rows,
            global,
            status: BTreeMap::new(),
            check: None,
            included: None,
            reloaded: None,
            file_error,
            copied: false,
            _subscriptions: subs,
        }
    }

    pub(super) fn global_trigger(&self, id: &str) -> Option<&Trigger> {
        self.global.get(id)
    }
}

impl SettingsWindow {
    pub fn shortcut_rows(&self) -> &[ShortcutRow] {
        &self.shortcuts.rows
    }

    pub fn bind_status(&self, id: &str) -> Option<&BindStatus> {
        self.shortcuts.status.get(id)
    }

    /// The compositor snippet for every global action with a trigger, or `None` without a bind helper.
    pub fn snippet(&self) -> Option<String> {
        let binds = self.shortcuts.binds.as_ref()?;
        let syntax = binds.syntax();
        let list: Vec<(CompositorTrigger, Route)> = CATALOGUE
            .iter()
            .filter_map(|e| {
                let trigger = self.shortcuts.global.get(e.id)?;
                Some((compositor_trigger(trigger, syntax), route_of(e)?.clone()))
            })
            .collect();
        Some(binds.snippet(&list))
    }

    /// The one line to add to the compositor config, or `None` without a bind helper.
    pub fn include_line(&self) -> Option<String> {
        let binds = self.shortcuts.binds.as_ref()?;
        Some(setup::include_line(binds.as_ref(), &self.shortcuts.paths))
    }

    pub fn included(&self) -> Option<&Result<bool, String>> {
        self.shortcuts.included.as_ref()
    }

    pub fn file_error(&self) -> Option<&str> {
        self.shortcuts.file_error.as_deref()
    }

    pub fn copy_include_line(&mut self, cx: &mut Context<Self>) {
        if let Some(line) = self.include_line() {
            self.shortcuts.clipboard.copy(line, cx);
            self.shortcuts.copied = true;
            cx.notify();
        }
    }

    pub fn check_include(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.shortcuts.binds.clone() else {
            return;
        };
        self.shortcuts.included = None;
        let dir = self.shortcuts.paths.config_dir().to_path_buf();
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
        let Some(binds) = self.shortcuts.binds.clone() else {
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

    pub fn reloaded(&self) -> Option<&Result<(), String>> {
        self.shortcuts.reloaded.as_ref()
    }

    pub fn check_binds(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.shortcuts.binds.clone() else {
            return;
        };
        self.check_include(cx);
        let dir = self.shortcuts.paths.config_dir().to_path_buf();
        cx.spawn(async move |this, cx| {
            let (found, included) = cx
                .background_executor()
                .spawn(async move {
                    let included = binds.is_included(&dir).unwrap_or(false);
                    (binds.existing_binds(), included)
                })
                .await;
            this.update(cx, |this, cx| this.apply_check(found, included, cx))
                .ok();
        })
        .detach();
    }

    fn apply_check(
        &mut self,
        found: es_backend::Result<Vec<(CompositorTrigger, String)>>,
        included: bool,
        cx: &mut Context<Self>,
    ) {
        let s = &mut self.shortcuts;
        s.status.clear();
        match found {
            Ok(existing) => {
                let syntax = s.binds.as_ref().map_or(BindSyntax::Sway, |b| b.syntax());
                for e in CATALOGUE {
                    let Some(route) = route_of(e) else { continue };
                    let ours = s.global.get(e.id).map(|t| compositor_trigger(t, syntax));
                    if let Some(found) = status(&existing, route, ours.as_ref(), included) {
                        s.status.insert(e.id, found);
                    }
                }
                let bound = s
                    .status
                    .values()
                    .filter(|v| **v == BindStatus::Bound)
                    .count();
                s.check = Some(Ok((bound, s.status.len() - bound)));
            }
            Err(error) => s.check = Some(Err(error.to_string())),
        }
        cx.notify();
    }

    fn on_shortcut(
        &mut self,
        entry: &'static ShortcutEntry,
        event: &ShortcutFieldEvent,
        cx: &mut Context<Self>,
    ) {
        let trigger = match event {
            ShortcutFieldEvent::Recorded(t) => Some(t.clone()),
            ShortcutFieldEvent::Cleared => None,
            ShortcutFieldEvent::RecordingChanged(_) => return,
        };
        if let Some(row) = self.shortcuts.rows.iter().find(|r| r.entry.id == entry.id) {
            let t = trigger.clone();
            row.field.update(cx, |f, cx| f.set_trigger(t, cx));
        }
        match entry.scope {
            Scope::Editor => {
                let value = trigger.as_ref().map(trigger_string).unwrap_or_default();
                self.edit(
                    move |c| {
                        c.shortcuts.insert(entry.id.to_owned(), value);
                    },
                    cx,
                );
            }
            Scope::Global => {
                let value = trigger.as_ref().map(trigger_string);
                match trigger {
                    Some(t) => self.shortcuts.global.insert(entry.id, t),
                    None => self.shortcuts.global.remove(entry.id),
                };
                self.shortcuts.status.remove(entry.id);
                self.shortcuts.copied = false;
                self.edit(
                    move |c| match value {
                        Some(v) => {
                            c.global_shortcuts.insert(entry.id.to_owned(), v);
                        }
                        None => {
                            c.global_shortcuts.remove(entry.id);
                        }
                    },
                    cx,
                );
                self.write_binds_file(cx);
            }
        }
        cx.notify();
    }

    fn write_binds_file(&mut self, cx: &mut Context<Self>) {
        let s = &mut self.shortcuts;
        let Some(binds) = &s.binds else { return };
        let globals = s
            .global
            .iter()
            .map(|(id, t)| ((*id).to_owned(), trigger_string(t)))
            .collect();
        s.file_error = setup::write_binds_file(binds.as_ref(), &s.paths, &globals)
            .err()
            .map(|e| e.to_string());
        cx.notify();
    }

    fn render_compositor(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx);
        let s = &self.shortcuts;
        let Some(binds) = &s.binds else {
            return group(
                Some(fl!("shortcuts-compositor").into()),
                vec![row(
                    fl!("shortcuts-compositor-none"),
                    Some(fl!("shortcuts-compositor-none-help").into()),
                    div(),
                    cx,
                )],
                None,
                cx,
            )
            .into_any_element();
        };
        let line = self.include_line().unwrap_or_default();
        let file = format!("{}/{}", setup::display_dir(&s.paths), binds.file_name());
        let mut buttons = h_flex().gap(px(8.)).child(
            EditorButton::new("check-binds")
                .label(fl!("shortcuts-check"))
                .tooltip(fl!("shortcuts-check-help"))
                .kind(ButtonKind::Bordered)
                .on_click(cx.listener(|this, _, _, cx| this.check_binds(cx))),
        );
        if binds.needs_reload() {
            buttons = buttons.child(
                EditorButton::new("reload-compositor")
                    .label(fl!("shortcuts-reload"))
                    .tooltip(fl!("shortcuts-reload-help"))
                    .kind(ButtonKind::Bordered)
                    .on_click(cx.listener(|this, _, _, cx| this.reload_compositor(cx))),
            );
        }
        buttons = buttons.child(
            EditorButton::new("copy-include")
                .label(if s.copied {
                    fl!("shortcuts-copied")
                } else {
                    fl!("shortcuts-copy-line")
                })
                .kind(ButtonKind::Prominent)
                .on_click(cx.listener(|this, _, _, cx| this.copy_include_line(cx))),
        );
        let code = div()
            .id("include-line")
            .accessible(Role::Code, fl!("shortcuts-include-a11y"))
            .mx(px(16.))
            .mb(px(12.))
            .p(px(10.))
            .rounded(px(8.))
            .bg(t.track)
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .child(line);
        let mut rows = vec![row(
            syntax_name(binds.syntax()),
            Some(fl!("shortcuts-compositor-help", file = file).into()),
            buttons,
            cx,
        )];
        rows.push(code.into_any_element());
        let included = match &s.included {
            Some(Ok(true)) => Some(status_mark(true, fl!("shortcuts-included"), cx)),
            Some(Ok(false)) => Some(status_mark(false, fl!("shortcuts-not-found"), cx)),
            _ => None,
        };
        if let Some(mark) = included {
            rows.push(div().mx(px(16.)).mb(px(12.)).child(mark).into_any_element());
        }
        let footer = match (&s.file_error, &s.included, &s.reloaded, &s.check) {
            (Some(error), ..) => Some(fl!("shortcuts-file-failed", error = error.clone())),
            (_, Some(Err(error)), ..) => {
                Some(fl!("shortcuts-include-failed", error = error.clone()))
            }
            (_, _, Some(Err(error)), _) => {
                Some(fl!("shortcuts-reload-failed", error = error.clone()))
            }
            (.., Some(Ok((bound, conflicts)))) => Some(fl!(
                "shortcuts-check-summary",
                bound = bound,
                conflicts = conflicts
            )),
            (.., Some(Err(error))) => Some(fl!("shortcuts-check-failed", error = error.clone())),
            _ => None,
        };
        group(
            Some(fl!("shortcuts-compositor").into()),
            rows,
            footer.map(Into::into),
            cx,
        )
        .into_any_element()
    }

    pub(super) fn render_shortcuts(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut out = vec![self.render_compositor(cx)];
        let mut groups: Vec<Vec<AnyElement>> = (0..8).map(|_| Vec::new()).collect();
        for r in &self.shortcuts.rows {
            let t = tokens(cx);
            let title = loader().get(r.entry.title_id);
            let route = route_of(r.entry)
                .map(|route| fl!("shortcuts-route", route = route.command_args().join(" ")));
            let mark = match self.shortcuts.status.get(r.entry.id) {
                Some(BindStatus::Bound) => {
                    Some(status_mark(true, fl!("shortcuts-status-bound"), cx).into_any_element())
                }
                Some(BindStatus::Conflict(command)) => Some(
                    div()
                        .max_w(px(160.))
                        .truncate()
                        .text_size(px(11.))
                        .text_color(t.destructive)
                        .child(fl!("shortcuts-status-conflict", command = command.clone()))
                        .into_any_element(),
                ),
                None => None,
            };
            let control = h_flex()
                .gap(px(10.))
                .children(mark)
                .child(div().w(px(150.)).child(r.field.clone()));
            let row_el = row(title, route.map(Into::into), control, cx);
            groups[group_of(r.entry.id)].push(row_el);
        }
        for (i, rows) in groups.into_iter().enumerate() {
            let footer = if i < 5 {
                fl!("shortcuts-global-footer-compositor")
            } else {
                fl!("shortcuts-editor-footer")
            };
            out.push(
                group(Some(group_title(i).into()), rows, Some(footer.into()), cx)
                    .into_any_element(),
            );
        }
        out
    }
}
