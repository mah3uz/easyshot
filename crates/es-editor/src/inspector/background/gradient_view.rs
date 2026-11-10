use es_model::{
    BackgroundStyle,
    background::{MAX_GRADIENT_STOPS, MIN_GRADIENT_STOPS},
};
use es_render::beautify::gradients::PRESETS;
use es_ui::components::Tip;
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton, IconButton, Segment, Segmented, Tile, TileGrid},
    icons::IconName,
    motion::{FILL_SWITCH, since_change},
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div, img,
    px,
};
use gpui_component::{
    Icon, Sizable, WindowExt, button::ButtonVariant, color_picker::ColorPicker, input::Input,
};

use super::BackgroundBody;
use super::gradient_editor::{QUICK_ANGLES, display_name};
use crate::strings::fl;

const STOP_REMOVE: gpui::Pixels = px(18.);

fn label(t: &Tokens, text: String) -> gpui::Div {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(text)
}

fn confirm_delete(
    body: WeakEntity<BackgroundBody>,
    id: String,
    name: String,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let (body, id) = (body.clone(), id.clone());
        dialog
            .title(fl!("editor-gradient-delete-title"))
            .description(fl!("editor-gradient-delete-message", name = name.clone()))
            .ok_text(fl!("editor-gradient-delete-confirm"))
            .ok_variant(ButtonVariant::Danger)
            .cancel_text(fl!("editor-gradient-cancel"))
            .show_cancel(true)
            .on_ok(move |_, _, cx| {
                let _ = body.update(cx, |b, cx| b.delete_gradient(&id, cx));
                true
            })
    });
}

impl BackgroundBody {
    pub(super) fn gradient_picker(
        &self,
        t: &Tokens,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mine = self.user_gradients();
        let selected = match &self.settings(cx).style {
            BackgroundStyle::Gradient(g) => {
                PRESETS.iter().position(|p| p.id == g.id).or_else(|| {
                    mine.iter()
                        .position(|u| u.id == g.id)
                        .map(|i| i + PRESETS.len())
                })
            }
            _ => None,
        };
        let built_in = PRESETS.iter().map(|p| p.style()).zip(self.gradients.iter());
        let tiles = built_in
            .chain(mine.iter().cloned().zip(self.mine_thumbs.iter()))
            .map(|(g, image)| {
                let tile = Tile::new(SharedString::from(g.id.clone()), display_name(&g));
                match image {
                    Some(image) => tile.child(img(image.clone()).size_full()),
                    None => tile,
                }
            });
        let this = cx.entity().downgrade();
        let add_help = fl!("editor-gradient-add-help");
        let add = div()
            .id("bg-gradient-add")
            .p(px(2.5))
            .accessible(Role::Button, add_help.clone())
            .tip(add_help)
            .child(
                div()
                    .w_full()
                    .aspect_ratio(1.)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(t.metrics.tile_radius)
                    .border(px(1.2))
                    .border_dashed()
                    .border_color(t.text_tertiary.opacity(0.6))
                    .text_color(t.text_tertiary)
                    .hover(|s| s.bg(t.hover_fill.opacity(0.4)))
                    .child(Icon::new(IconName::Plus).size(px(12.))),
            )
            .on_click({
                let this = this.clone();
                move |_, window, cx| {
                    let _ = this.update(cx, |b, cx| b.new_gradient(window, cx));
                }
            });
        let grid = TileGrid::new("bg-gradients", fl!("editor-bg-fill-gradient"), tiles)
            .selected(selected)
            .on_select(move |i, _, cx| {
                let _ = this.update(cx, |this, cx| this.pick_gradient(i, cx));
            })
            .trailing(add);
        let below = match &self.draft {
            Some(_) => self.gradient_panel(t, cx),
            None => self.gradient_actions(t, cx),
        };
        let shown = since_change(
            "bg-gradient-editor",
            self.draft.is_some(),
            FILL_SWITCH,
            window,
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .child(grid)
            .children(below.map(|b| div().opacity(shown).child(b)))
            .into_any_element()
    }

    fn gradient_actions(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let g = self.selected_gradient(cx)?;
        let this = cx.entity().downgrade();
        let (edit_label, edit_help) = if g.is_user() {
            (
                fl!("editor-gradient-edit"),
                fl!("editor-gradient-edit-help"),
            )
        } else {
            (
                fl!("editor-gradient-edit-copy"),
                fl!("editor-gradient-edit-copy-help"),
            )
        };
        let name = display_name(&g);
        let delete = g.is_user().then(|| {
            let (this, id, name) = (this.clone(), g.id.clone(), name.clone());
            IconButton::new(
                "bg-gradient-delete",
                IconName::Trash,
                fl!("editor-gradient-delete-help"),
            )
            .size(px(26.), px(13.))
            .on_click(move |_, window, cx| {
                confirm_delete(this.clone(), id.clone(), name.clone(), window, cx);
            })
        });
        Some(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(label(t, name).flex_1().min_w_0().truncate())
                .children(delete)
                .child(
                    EditorButton::new("bg-gradient-edit")
                        .kind(ButtonKind::Bordered)
                        .icon(IconName::Pencil)
                        .label(edit_label)
                        .tip(edit_help)
                        .on_click(move |_, window, cx| {
                            let _ = this.update(cx, |b, cx| b.edit_gradient(window, cx));
                        }),
                )
                .into_any_element(),
        )
    }

    fn gradient_panel(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let draft = self.draft.as_ref()?;
        let this = cx.entity().downgrade();
        let count = draft.stops.len();
        let stops = draft.stops.iter().enumerate().map(|(i, stop)| {
            let n = i + 1;
            let remove = (count > MIN_GRADIENT_STOPS).then(|| {
                let this = this.clone();
                IconButton::new(
                    SharedString::from(format!("bg-gradient-stop-remove-{i}")),
                    IconName::X,
                    fl!("editor-gradient-stop-remove", n = n),
                )
                .size(STOP_REMOVE, px(10.))
                .on_click(move |_, window, cx| {
                    let _ = this.update(cx, |b, cx| b.remove_stop(i, window, cx));
                })
            });
            div()
                .flex()
                .items_center()
                .gap(px(1.))
                .child(
                    ColorPicker::new(stop)
                        .small()
                        .accessibility_label(fl!("editor-gradient-stop", n = n)),
                )
                .children(remove)
        });
        let add = (count < MAX_GRADIENT_STOPS).then(|| {
            let this = this.clone();
            IconButton::new(
                "bg-gradient-stop-add",
                IconName::Plus,
                fl!("editor-gradient-stop-add"),
            )
            .size(px(24.), px(12.))
            .on_click(move |_, window, cx| {
                let _ = this.update(cx, |b, cx| b.add_stop(window, cx));
            })
        });
        let angle = draft.gradient.angle_degrees();
        let quick = Segmented::new(
            "bg-gradient-quick-angles",
            fl!("editor-gradient-quick-angles"),
            QUICK_ANGLES.map(|d| {
                let degrees = d as i64;
                Segment::text(fl!("editor-gradient-angle-value", degrees = degrees))
                    .help(fl!("editor-gradient-angle-help", degrees = degrees))
            }),
        )
        .selected(
            QUICK_ANGLES
                .iter()
                .position(|d| (d - angle).abs() < 0.5)
                .unwrap_or(usize::MAX),
        )
        .on_select({
            let this = this.clone();
            move |i, window, cx| {
                let degrees = QUICK_ANGLES[i.min(QUICK_ANGLES.len() - 1)];
                let _ = this.update(cx, |b, cx| b.set_gradient_angle(degrees, window, cx));
            }
        });
        let title = if draft.is_new {
            fl!("editor-gradient-new-title")
        } else {
            fl!("editor-gradient-edit-title")
        };
        let delete = (!draft.is_new).then(|| {
            let (this, id, name) = (
                this.clone(),
                draft.gradient.id.clone(),
                draft.gradient.title.clone(),
            );
            IconButton::new(
                "bg-gradient-panel-delete",
                IconName::Trash,
                fl!("editor-gradient-delete-help"),
            )
            .size(px(26.), px(13.))
            .on_click(move |_, window, cx| {
                confirm_delete(this.clone(), id.clone(), name.clone(), window, cx);
            })
        });
        let (cancel, save, escape) = (this.clone(), this.clone(), this.clone());
        Some(
            div()
                .id("bg-gradient-panel")
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(10.))
                .rounded(t.metrics.card_radius)
                .border(t.metrics.hairline)
                .border_color(t.border)
                .bg(t.bordered_fill)
                .accessible(Role::Group, title.clone())
                .on_key_down(move |event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        let _ = escape.update(cx, |b, cx| b.cancel_gradient(cx));
                    }
                })
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.metrics.group_label_spacing)
                        .child(label(t, fl!("editor-gradient-name")))
                        .child(Input::new(&draft.name).small()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.metrics.group_label_spacing)
                        .child(label(t, fl!("editor-gradient-colors")))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .children(stops)
                                .children(add),
                        ),
                )
                .child(draft.angle.clone())
                .child(quick)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .children(delete)
                        .child(div().flex_1())
                        .child(
                            EditorButton::new("bg-gradient-cancel")
                                .kind(ButtonKind::Bordered)
                                .label(fl!("editor-gradient-cancel"))
                                .tip(fl!("editor-gradient-cancel-help"))
                                .on_click(move |_, _, cx| {
                                    let _ = cancel.update(cx, |b, cx| b.cancel_gradient(cx));
                                }),
                        )
                        .child(
                            EditorButton::new("bg-gradient-save")
                                .kind(ButtonKind::Prominent)
                                .label(fl!("editor-gradient-save"))
                                .tip(fl!("editor-gradient-save-help"))
                                .on_click(move |_, _, cx| {
                                    let _ = save.update(cx, |b, cx| {
                                        b.save_gradient(cx);
                                    });
                                }),
                        ),
                )
                .into_any_element(),
        )
    }
}
