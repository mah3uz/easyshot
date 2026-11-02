use es_ui::{
    components::{ButtonKind, EditorButton, Popover, PresetBar},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    Anchor, AnyElement, App, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement,
    Render, SharedString, Styled, Subscription, WeakEntity, Window, div, px,
};
use gpui_component::{
    Icon, Sizable, WindowExt,
    button::ButtonVariant,
    input::{Input, InputEvent, InputState},
    menu::PopupMenuItem,
};
use uuid::Uuid;

use crate::session::look::{LookModel, PresetError, wallpaper_missing};
use crate::strings::fl;

const NEW_PRESET_WIDTH: gpui::Pixels = px(292.);

/// The pinned row of saved background presets, wired to the preset library.
pub struct PresetsBar {
    look: Entity<LookModel>,
    adding: bool,
    name: Option<Entity<InputState>>,
    error: Option<PresetError>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct Entry {
    id: Uuid,
    name: SharedString,
    missing: bool,
}

impl PresetsBar {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&look, |_, _, cx| cx.notify());
        Self {
            look,
            adding: false,
            name: None,
            error: None,
            _subscriptions: vec![observe],
        }
    }

    pub fn is_adding(&self) -> bool {
        self.adding
    }

    pub fn error(&self) -> Option<PresetError> {
        self.error
    }

    pub fn open_new_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = fl!("editor-preset-new-placeholder");
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let sub =
            cx.subscribe_in(
                &name,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => this.save(window, cx),
                    InputEvent::Change => {
                        this.error = None;
                        cx.notify();
                    }
                    _ => {}
                },
            );
        name.update(cx, |state, cx| state.focus(window, cx));
        self._subscriptions.push(sub);
        self.name = Some(name);
        self.error = None;
        self.adding = true;
        cx.notify();
    }

    pub fn set_name(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = &self.name {
            let text = text.to_owned();
            name.update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        self.adding = false;
        self.error = None;
        cx.notify();
    }

    pub fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .name
            .as_ref()
            .map(|n| n.read(cx).value().to_string())
            .unwrap_or_default();
        match self.look.update(cx, |look, cx| look.add_preset(&text, cx)) {
            Ok(_) => self.cancel(cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    fn entries(&self, cx: &App) -> Vec<Entry> {
        self.look
            .read(cx)
            .presets()
            .iter()
            .map(|p| Entry {
                id: p.id,
                name: p.name.clone().into(),
                missing: wallpaper_missing(&p.background),
            })
            .collect()
    }

    fn new_preset_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let this = cx.entity().downgrade();
        let error = self.error.map(|e| {
            let (icon, text) = match e {
                PresetError::Duplicate => {
                    (IconName::CircleAlert, fl!("editor-preset-error-duplicate"))
                }
                PresetError::WallpaperMissing => (
                    IconName::TriangleAlert,
                    fl!("editor-preset-error-wallpaper"),
                ),
            };
            div()
                .flex()
                .items_start()
                .gap(px(4.))
                .text_size(px(10.))
                .text_color(t.destructive)
                .child(Icon::new(icon).size(px(11.)).flex_none())
                .child(text)
        });
        let cancel = this.clone();
        let save = this;
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(4.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(fl!("editor-preset-new-title")),
                    )
                    .child(
                        div()
                            .typeset(t.type_scale.label)
                            .text_color(t.text_secondary)
                            .child(fl!("editor-preset-new-body")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(self.name.as_ref().map(|n| Input::new(n).small()))
                    .children(error),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        EditorButton::new("preset-cancel")
                            .label(fl!("editor-preset-cancel"))
                            .kind(ButtonKind::Bordered)
                            .on_click(move |_, _, cx| {
                                let _ = cancel.update(cx, |this, cx| this.cancel(cx));
                            }),
                    )
                    .child(
                        EditorButton::new("preset-save")
                            .label(fl!("editor-preset-save"))
                            .kind(ButtonKind::Prominent)
                            .on_click(move |_, window, cx| {
                                let _ = save.update(cx, |this, cx| this.save(window, cx));
                            }),
                    ),
            )
            .into_any_element()
    }
}

fn confirm_delete(
    look: WeakEntity<LookModel>,
    id: Uuid,
    name: SharedString,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let look = look.clone();
        dialog
            .title(fl!("editor-preset-delete-title"))
            .description(fl!("editor-preset-delete-message", name = name.to_string()))
            .ok_text(fl!("editor-preset-delete-confirm"))
            .ok_variant(ButtonVariant::Danger)
            .cancel_text(fl!("editor-preset-cancel"))
            .show_cancel(true)
            .on_ok(move |_, _, cx| {
                let _ = look.update(cx, |look, cx| look.delete_preset(id, cx));
                true
            })
    });
}

impl Render for PresetsBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = self.look.read(cx);
        let entries = self.entries(cx);
        let applied = look.applied_preset().map(|p| p.id);
        let modified = look.modified();
        let clean = applied.filter(|_| !modified);
        let default = look.default_preset();
        let current_missing = wallpaper_missing(look.settings());
        let usable: Vec<Entry> = entries.iter().filter(|e| !e.missing).cloned().collect();
        let missing: Vec<Entry> = entries.iter().filter(|e| e.missing).cloned().collect();
        let names = usable.iter().map(|e| e.name.clone());
        let applied_index = applied.and_then(|id| usable.iter().position(|e| e.id == id));

        let help: SharedString = match default.and_then(|id| entries.iter().find(|e| e.id == id)) {
            Some(e) => fl!("editor-preset-help-applied", name = e.name.to_string()).into(),
            None => fl!("editor-preset-help").into(),
        };
        let title = applied_index
            .map(|i| usable[i].name.to_string())
            .unwrap_or_else(|| fl!("editor-preset-menu-title"));
        let mut value = fl!("editor-preset-a11y-value", title = title);
        if modified {
            value.push_str(&fl!("editor-preset-a11y-modified"));
        }
        if current_missing {
            value.push_str(&fl!("editor-preset-a11y-wallpaper-missing"));
        }

        let weak_look = self.look.downgrade();
        let menu_look = weak_look.clone();
        let delete_look = weak_look.clone();
        let delete_names: Vec<(Uuid, SharedString)> =
            usable.iter().map(|e| (e.id, e.name.clone())).collect();
        let this = cx.entity().downgrade();
        let adding = self.adding;

        PresetBar::new("preset-bar", names)
            .applied(applied_index)
            .help(help)
            .value(value)
            .on_delete(move |index, window, cx| {
                if let Some((id, name)) = delete_names.get(index).cloned() {
                    confirm_delete(delete_look.clone(), id, name, window, cx);
                }
            })
            .menu(move |menu, window, cx| {
                let pick = |label: SharedString, id: Option<Uuid>, checked: bool| {
                    let look = menu_look.clone();
                    PopupMenuItem::new(label)
                        .checked(checked)
                        .on_click(move |_, _, cx| {
                            let _ = look.update(cx, |look, cx| look.apply_preset(id, cx));
                        })
                };
                let mut menu = menu.label(fl!("editor-preset-menu-saved")).item(pick(
                    fl!("editor-preset-menu-current").into(),
                    None,
                    clean.is_none(),
                ));
                if entries.is_empty() {
                    menu = menu.label(fl!("editor-preset-menu-empty"));
                }
                for e in &usable {
                    menu = menu.item(pick(e.name.clone(), Some(e.id), clean == Some(e.id)));
                }
                if !missing.is_empty() {
                    menu = menu
                        .separator()
                        .label(fl!("editor-preset-menu-wallpaper-missing"));
                    for e in &missing {
                        let look = menu_look.clone();
                        let (id, name) = (e.id, e.name.clone());
                        menu = menu.item(
                            PopupMenuItem::new(fl!(
                                "editor-preset-menu-delete-item",
                                name = e.name.to_string()
                            ))
                            .icon(Icon::new(IconName::Trash))
                            .on_click(move |_, window, cx| {
                                confirm_delete(look.clone(), id, name.clone(), window, cx);
                            }),
                        );
                    }
                }
                if !entries.is_empty() {
                    let look = menu_look.clone();
                    let choices: Vec<(Option<Uuid>, SharedString)> =
                        std::iter::once((None, fl!("editor-preset-menu-none").into()))
                            .chain(entries.iter().map(|e| (Some(e.id), e.name.clone())))
                            .collect();
                    menu = menu.separator().submenu(
                        fl!("editor-preset-menu-default"),
                        window,
                        cx,
                        move |mut sub, _, _| {
                            for (id, label) in &choices {
                                let (look, id) = (look.clone(), *id);
                                sub = sub.item(
                                    PopupMenuItem::new(label.clone())
                                        .checked(id == default)
                                        .on_click(move |_, _, cx| {
                                            let _ = look.update(cx, |look, cx| {
                                                look.set_default_preset(id, cx)
                                            });
                                        }),
                                );
                            }
                            sub
                        },
                    );
                }
                menu
            })
            .wrap_add(move |add| {
                let on_open = this.clone();
                let form = this.clone();
                Popover::new("new-preset", fl!("editor-preset-new-title"))
                    .anchor(Anchor::TopRight)
                    .width(NEW_PRESET_WIDTH)
                    .open(adding)
                    .on_open_change(move |open, window, cx| {
                        let _ = on_open.update(cx, |this, cx| {
                            if open {
                                this.open_new_preset(window, cx);
                            } else {
                                this.cancel(cx);
                            }
                        });
                    })
                    .trigger(add)
                    .content(move |_, cx| {
                        form.update(cx, |this, cx| this.new_preset_form(cx))
                            .unwrap_or_else(|_| div().into_any_element())
                    })
                    .into_any_element()
            })
    }
}
