use super::export::ExportMode;
use super::trim::clock;
use super::view::{ExportState, PlayerWindow, TITLE_BAR_HEIGHT};
use crate::capture::strings::fl;
use es_ui::a11y::{Accessible, Role};
use es_ui::components::{ButtonKind, EditorButton, IconButton, Segment, Segmented};
use es_ui::icons::IconName;
use es_ui::theme::tokens;
use es_ui::tokens::Typeset;
use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::TitleBar;

impl PlayerWindow {
    pub(super) fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let name = self
            .config
            .video
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let running = matches!(self.export, ExportState::Running(_));
        TitleBar::new()
            .h(TITLE_BAR_HEIGHT)
            .child(side())
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .max_w(px(520.))
                    .px(px(12.))
                    .truncate()
                    .typeset(t.type_scale.title)
                    .text_color(t.text)
                    .child(name),
            )
            .child(
                side().justify_end().pr(px(12.)).child(
                    hold("export-hold").child(
                        EditorButton::new("export")
                            .kind(ButtonKind::Prominent)
                            .icon(IconName::CircleArrowDown)
                            .label(fl!("player-export"))
                            .accessible_label(fl!("player-export-a11y"))
                            .disabled(running || self.engine.is_none())
                            .on_click(cx.listener(|p, _, _, cx| p.export(cx))),
                    ),
                ),
            )
    }

    pub(super) fn transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let (icon, label) = if self.playing {
            (IconName::CirclePause, fl!("player-pause"))
        } else {
            (IconName::CirclePlay, fl!("player-play"))
        };
        let time = fl!(
            "player-time",
            position = clock(self.position),
            duration = clock(self.trim.duration())
        );
        let mode = Segmented::new(
            "mode",
            fl!("player-mode-a11y"),
            [
                Segment::text(fl!("player-mode-precise")),
                Segment::text(fl!("player-mode-fast")),
            ],
        )
        .selected(usize::from(self.mode == ExportMode::Fast))
        .on_select({
            let view = cx.entity().downgrade();
            move |i, _, cx| {
                let mode = if i == 1 {
                    ExportMode::Fast
                } else {
                    ExportMode::Precise
                };
                let _ = view.update(cx, |p, cx| p.set_mode(mode, cx));
            }
        });
        div()
            .flex()
            .items_center()
            .h(px(32.))
            .child(
                div()
                    .flex_1()
                    .flex_basis(px(0.))
                    .typeset(t.type_scale.value)
                    .text_color(t.text_secondary)
                    .child(time),
            )
            .child(
                IconButton::new("play", icon, label)
                    .size(px(32.), px(22.))
                    .disabled(self.engine.is_none())
                    .on_click(cx.listener(|p, _, _, cx| p.toggle_play(cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_basis(px(0.))
                    .justify_end()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(176.)).child(mode))
                    .child(
                        EditorButton::new("remove-audio")
                            .icon(IconName::VolumeOff)
                            .label(fl!("player-remove-audio"))
                            .toggle(true)
                            .selected(self.remove_audio)
                            .on_click(
                                cx.listener(|p, _, _, cx| p.set_remove_audio(!p.remove_audio, cx)),
                            ),
                    ),
            )
    }

    pub(super) fn status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let kept = fl!(
            "player-kept",
            start = clock(self.trim.start()),
            end = clock(self.trim.end()),
            kept = format!("{:.2}", self.trim.kept())
        );
        let (note, colour, progress): (Option<String>, _, Option<f64>) =
            match (&self.failure, &self.export) {
                (Some(reason), _) => (
                    Some(fl!("player-open-failed", reason = reason.clone())),
                    t.destructive,
                    None,
                ),
                (None, ExportState::Running(p)) => (
                    Some(fl!(
                        "player-exporting",
                        percent = ((p * 100.).round() as u32)
                    )),
                    t.text_secondary,
                    Some(*p),
                ),
                (None, ExportState::Done(path)) => (
                    Some(fl!(
                        "player-exported",
                        name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    )),
                    t.success,
                    None,
                ),
                (None, ExportState::Failed(reason)) => (
                    Some(fl!("player-export-failed", reason = reason.clone())),
                    t.destructive,
                    None,
                ),
                (None, ExportState::Idle) => (None, t.text_secondary, None),
            };
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .h(px(18.))
            .typeset(t.type_scale.footer)
            .text_color(t.text_secondary)
            .child(div().flex_1().child(kept))
            .when_some(progress, |row, p| {
                row.child(
                    div()
                        .w(px(120.))
                        .h(px(4.))
                        .rounded_full()
                        .bg(t.track)
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(t.accent)
                                .w(gpui::relative(p as f32)),
                        ),
                )
            })
            .when_some(note, |row, note| {
                row.child(
                    div()
                        .id("player-status")
                        .accessible(Role::Status, note.clone())
                        .text_color(colour)
                        .truncate()
                        .child(note),
                )
            })
    }
}

fn side() -> gpui::Div {
    div()
        .flex()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .items_center()
}

/// A pressed button must not start a window move, which the title bar begins on any press.
fn hold(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}
