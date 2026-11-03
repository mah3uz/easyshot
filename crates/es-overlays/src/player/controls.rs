use super::export::ExportMode;
use super::options::{ExportOptions, Format, Look, Quality, Size};
use super::trim::clock;
use super::view::{ExportState, PlayerWindow, TITLE_BAR_HEIGHT};
use crate::capture::strings::fl;
use es_ui::a11y::{Accessible, Role};
use es_ui::components::{
    ButtonKind, EditorButton, IconButton, InspectorSlider, Segment, Segmented,
};
use es_ui::icons::IconName;
use es_ui::theme::tokens;
use es_ui::tokens::Typeset;
use gpui::{
    Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, div,
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
        .disabled(!self.can_copy())
        .selected(usize::from(
            self.mode == ExportMode::Fast && self.can_copy(),
        ))
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
                            .disabled(self.options.format == Format::Gif)
                            .selected(self.remove_audio || self.options.format == Format::Gif)
                            .on_click(
                                cx.listener(|p, _, _, cx| p.set_remove_audio(!p.remove_audio, cx)),
                            ),
                    ),
            )
    }

    fn can_copy(&self) -> bool {
        self.options.format == Format::Mp4 && !self.options.alters_picture()
    }

    pub(super) fn options_panel(
        &self,
        brightness: &Entity<InspectorSlider>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let pick = move |change: fn(ExportOptions, usize) -> ExportOptions| {
            let view = view.clone();
            move |i: usize, _: &mut gpui::Window, cx: &mut gpui::App| {
                let _ = view.update(cx, |p, cx| p.set_options(change(p.options, i), cx));
            }
        };
        let o = self.options;
        let format = Segmented::new(
            "export-format",
            fl!("player-format-a11y"),
            [
                Segment::text(fl!("player-format-mp4")),
                Segment::text(fl!("player-format-webm")),
                Segment::text(fl!("player-format-gif")),
            ],
        )
        .selected(Format::ALL.iter().position(|f| *f == o.format).unwrap_or(0))
        .on_select(pick(|o, i| ExportOptions {
            format: Format::ALL[i],
            ..o
        }));
        let size = Segmented::new(
            "export-size",
            fl!("player-size-a11y"),
            [
                Segment::text(fl!("player-size-original")),
                Segment::text(fl!("player-size-1080")),
                Segment::text(fl!("player-size-720")),
                Segment::text(fl!("player-size-480")),
            ],
        )
        .selected(Size::ALL.iter().position(|s| *s == o.size).unwrap_or(0))
        .on_select(pick(|o, i| ExportOptions {
            size: Size::ALL[i],
            ..o
        }));
        let quality = Segmented::new(
            "export-quality",
            fl!("player-quality-a11y"),
            [
                Segment::text(fl!("player-quality-high")),
                Segment::text(fl!("player-quality-balanced")),
                Segment::text(fl!("player-quality-small")),
            ],
        )
        .selected(
            Quality::ALL
                .iter()
                .position(|q| *q == o.quality)
                .unwrap_or(0),
        )
        .on_select(pick(|o, i| ExportOptions {
            quality: Quality::ALL[i],
            ..o
        }));
        let look = Segmented::new(
            "export-look",
            fl!("player-look-a11y"),
            [
                Segment::text(fl!("player-look-natural")),
                Segment::text(fl!("player-look-vivid")),
                Segment::text(fl!("player-look-muted")),
                Segment::text(fl!("player-look-mono")),
            ],
        )
        .selected(Look::ALL.iter().position(|l| *l == o.look).unwrap_or(0))
        .on_select(pick(|o, i| ExportOptions {
            look: Look::ALL[i],
            ..o
        }));
        div()
            .id("export-options")
            .accessible(Role::Group, fl!("player-options-a11y"))
            .flex()
            .flex_wrap()
            .items_end()
            .gap_x(px(16.))
            .gap_y(px(8.))
            .child(option(fl!("player-format"), 150., format, cx))
            .child(option(fl!("player-size"), 250., size, cx))
            .child(option(fl!("player-quality"), 200., quality, cx))
            .child(option(fl!("player-look"), 260., look, cx))
            .child(div().w(px(300.)).child(brightness.clone()))
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

fn option(
    caption: String,
    width: f32,
    control: Segmented,
    cx: &Context<PlayerWindow>,
) -> impl IntoElement {
    let t = tokens(cx).clone();
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .w(px(width))
        .child(
            div()
                .typeset(t.type_scale.footer)
                .text_color(t.text_secondary)
                .child(caption),
        )
        .child(control)
}
