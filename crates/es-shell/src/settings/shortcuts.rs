use super::binds::{BindStatus, compositor_trigger, status, trigger_string};
use super::widgets::{group, row, status_mark};
use super::{ClipboardSink, SettingsWindow};
use crate::strings::{fl, loader};
use es_backend::{BindHelper, BindSyntax, Trigger as CompositorTrigger};
use es_model::Config;
use es_protocol::{CATALOGUE, Route, Scope, ShortcutEntry, Target, Trigger};
use es_ui::{
    components::{ButtonKind, EditorButton, ShortcutField, ShortcutFieldEvent},
    theme::tokens,
};
use gpui::{
    AnyElement, AppContext, Context, Entity, IntoElement, ParentElement, Styled, Subscription,
    Window, div, px,
};
use gpui_component::h_flex;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct ShortcutRow {
    pub entry: &'static ShortcutEntry,
    pub field: Entity<ShortcutField>,
}

pub(super) struct Shortcuts {
    binds: Option<Arc<dyn BindHelper>>,
    clipboard: Rc<dyn ClipboardSink>,
    rows: Vec<ShortcutRow>,
    global: BTreeMap<&'static str, Trigger>,
    status: BTreeMap<&'static str, BindStatus>,
    check: Option<Result<(usize, usize), String>>,
    copied: bool,
    _subscriptions: Vec<Subscription>,
}

fn group_of(id: &str) -> usize {
    const TOOLS: [&str; 14] = [
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

fn route_of(entry: &ShortcutEntry) -> Option<&Route> {
    match &entry.target {
        Target::Route(route) if route.args().0.is_empty() => Some(route),
        _ => None,
    }
}

impl Shortcuts {
    pub(super) fn new(
        config: &Config,
        binds: Option<Arc<dyn BindHelper>>,
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
        Self {
            binds,
            clipboard,
            rows,
            global: BTreeMap::new(),
            status: BTreeMap::new(),
            check: None,
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

    pub fn copy_snippet(&mut self, cx: &mut Context<Self>) {
        if let Some(snippet) = self.snippet().filter(|s| !s.is_empty()) {
            self.shortcuts.clipboard.copy(snippet, cx);
            self.shortcuts.copied = true;
            cx.notify();
        }
    }

    pub fn check_binds(&mut self, cx: &mut Context<Self>) {
        let Some(binds) = self.shortcuts.binds.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { binds.existing_binds() })
                .await;
            this.update(cx, |this, cx| this.apply_check(found, cx)).ok();
        })
        .detach();
    }

    fn apply_check(
        &mut self,
        found: es_backend::Result<Vec<(CompositorTrigger, String)>>,
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
                    if let Some(found) = status(&existing, route, ours.as_ref()) {
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
                match trigger {
                    Some(t) => self.shortcuts.global.insert(entry.id, t),
                    None => self.shortcuts.global.remove(entry.id),
                };
                self.shortcuts.status.remove(entry.id);
                self.shortcuts.copied = false;
            }
        }
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
        let snippet = self.snippet().unwrap_or_default();
        let empty = snippet.is_empty();
        let buttons = h_flex()
            .gap(px(8.))
            .child(
                EditorButton::new("check-binds")
                    .label(fl!("shortcuts-check"))
                    .tooltip(fl!("shortcuts-check-help"))
                    .kind(ButtonKind::Bordered)
                    .on_click(cx.listener(|this, _, _, cx| this.check_binds(cx))),
            )
            .child(
                EditorButton::new("copy-snippet")
                    .label(if s.copied {
                        fl!("shortcuts-copied")
                    } else {
                        fl!("shortcuts-copy-snippet")
                    })
                    .kind(ButtonKind::Prominent)
                    .disabled(empty)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_snippet(cx))),
            );
        let code = div()
            .mx(px(16.))
            .mb(px(12.))
            .p(px(10.))
            .rounded(px(8.))
            .bg(t.track)
            .text_size(px(11.))
            .text_color(if empty { t.text_tertiary } else { t.text })
            .child(if empty {
                fl!("shortcuts-snippet-empty")
            } else {
                snippet.trim_end().to_owned()
            });
        let mut rows = vec![row(
            syntax_name(binds.syntax()),
            Some(fl!("shortcuts-compositor-help").into()),
            buttons,
            cx,
        )];
        rows.push(code.into_any_element());
        let footer = match &s.check {
            Some(Ok((bound, conflicts))) => Some(fl!(
                "shortcuts-check-summary",
                bound = bound,
                conflicts = conflicts
            )),
            Some(Err(error)) => Some(fl!("shortcuts-check-failed", error = error.clone())),
            None => None,
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
