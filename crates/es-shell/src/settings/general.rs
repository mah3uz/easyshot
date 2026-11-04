use super::widgets::{FIELD_WIDTH, group, row};
use super::{Page, SettingsWindow, SettingsWindowEvent, Toggle};
use crate::strings::fl;
use es_protocol::Route;
use es_ui::components::{ButtonKind, EditorButton};
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, px};
use gpui_component::input::Input;

fn text(s: String) -> Option<SharedString> {
    Some(s.into())
}

const HISTORY_LIMITS: [u32; 5] = [50, 100, 250, 500, 0];

impl SettingsWindow {
    pub(super) fn render_general(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let g = &c.general;
        let s = |v: &str| v.to_owned();
        let folder = g
            .save_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| fl!("general-save-folder-default"));
        let autosave_help = if g.auto_save {
            fl!("general-autosave-on-help")
        } else {
            fl!("general-autosave-off-help")
        };
        let jpeg = g.export_format == "jpeg";
        let mut format_rows = vec![self.choice_row(
            "format",
            fl!("general-format-save-as"),
            Some(if jpeg {
                fl!("general-format-jpeg-help")
            } else {
                fl!("general-format-png-help")
            }),
            vec![
                (s("png"), fl!("general-format-png")),
                (s("jpeg"), fl!("general-format-jpeg")),
            ],
            &g.export_format.clone(),
            |c, v| c.general.export_format = v,
            cx,
        )];
        if jpeg {
            format_rows.push(self.slider_row(&self.sliders.quality, cx));
        }
        let look = if c.default_look.style.is_empty() {
            fl!("general-look-no-background")
        } else {
            c.default_look.style.clone()
        };
        let limits = HISTORY_LIMITS
            .iter()
            .map(|&n| {
                let label = if n == 0 {
                    fl!("general-recent-unlimited")
                } else {
                    n.to_string()
                };
                (n, label)
            })
            .collect();
        vec![
            group(
                text(fl!("general-startup")),
                vec![self.toggle_row(
                    Toggle::ShowBarAtLaunch,
                    fl!("general-show-bar-at-launch"),
                    None,
                    cx,
                )],
                None,
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("general-appearance")),
                vec![
                    self.choice_row(
                        "theme",
                        fl!("general-theme"),
                        None,
                        vec![
                            (s("system"), fl!("general-theme-system")),
                            (s("light"), fl!("general-theme-light")),
                            (s("dark"), fl!("general-theme-dark")),
                        ],
                        &g.theme.clone(),
                        |c, v| c.general.theme = v,
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::EditorsFullscreen,
                        fl!("general-editors-fullscreen"),
                        None,
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::LowResPreview,
                        fl!("general-low-res-preview"),
                        Some(fl!("general-low-res-preview-help")),
                        cx,
                    ),
                ],
                None,
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("general-saving")),
                vec![
                    row(
                        fl!("general-save-folder"),
                        text(folder),
                        EditorButton::new("choose-folder")
                            .label(fl!("general-save-folder-choose"))
                            .kind(ButtonKind::Bordered)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SettingsWindowEvent::ChooseSaveFolder)
                            })),
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::AutoSave,
                        fl!("general-autosave"),
                        Some(autosave_help),
                        cx,
                    ),
                    row(
                        fl!("general-file-name"),
                        None,
                        Input::new(&self.file_name).w(px(FIELD_WIDTH)),
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::CopyAfterSave,
                        fl!("general-copy-to-clipboard"),
                        Some(fl!("general-copy-to-clipboard-help")),
                        cx,
                    ),
                    self.toggle_row(Toggle::PlaySound, fl!("general-shutter-sound"), None, cx),
                ],
                text(fl!("general-saving-footer")),
                cx,
            )
            .into_any_element(),
            group(text(fl!("general-format")), format_rows, None, cx).into_any_element(),
            group(
                text(fl!("general-look")),
                vec![
                    row(
                        look,
                        None,
                        EditorButton::new("reset-look")
                            .label(fl!("general-look-reset-a11y"))
                            .kind(ButtonKind::Bordered)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.edit(|c| c.default_look = Default::default(), cx)
                            })),
                        cx,
                    ),
                    self.slider_row(&self.sliders.padding, cx),
                    self.slider_row(&self.sliders.radius, cx),
                    self.slider_row(&self.sliders.shadow, cx),
                ],
                text(fl!("general-look-footer")),
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("general-recent")),
                vec![
                    self.choice_row(
                        "history-limit",
                        fl!("general-recent-keep"),
                        Some(fl!("general-recent-keep-help")),
                        limits,
                        &g.history_limit,
                        |c, v| c.general.history_limit = v,
                        cx,
                    ),
                    row(
                        fl!("general-recent-gallery"),
                        text(fl!("general-recent-gallery-help")),
                        EditorButton::new("open-gallery")
                            .label(fl!("general-recent-gallery-open"))
                            .accessible_label(fl!("general-recent-gallery-open-a11y"))
                            .kind(ButtonKind::Bordered)
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(SettingsWindowEvent::Run(Route::Gallery))
                            })),
                        cx,
                    ),
                ],
                None,
                cx,
            )
            .into_any_element(),
            self.restore_row(
                Page::General,
                fl!("general-restore-help"),
                |c| {
                    let keep = c.general.history_limit;
                    c.general = Default::default();
                    c.general.history_limit = keep;
                    c.default_look = Default::default();
                },
                cx,
            ),
        ]
    }
}
