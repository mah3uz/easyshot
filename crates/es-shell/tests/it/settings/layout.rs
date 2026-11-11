use super::*;
use es_shell::settings::{
    INLINE_CONTROL_MAX, MIN_SIZE, ROW_TEXT, WINDOW_SIZE, beside, row_content_width,
    segmented_width, title_column_width,
};
use es_ui::tokens::TYPE;
use gpui::{FontWeight, TextRun, black, font};

const PAGE_HEIGHT: f32 = 2000.;
const SHORTCUTS_HEIGHT: f32 = 6000.;
const TRACK_INSET: f32 = 2. * 2.;
const SEGMENT_CHROME: f32 = 2. * 6. + 2. * 1.;

fn text(id: &str) -> String {
    es_shell::strings::loader().get(id)
}

/// Every choice row in Settings: its title and its options, as the pages build them.
fn choice_rows() -> Vec<(String, Vec<String>)> {
    let ids =
        |title: &str, options: &[&str]| (text(title), options.iter().map(|o| text(o)).collect());
    let mixed = |title: &str, options: &[&str]| {
        (
            text(title),
            options.iter().map(|o| (*o).to_owned()).collect(),
        )
    };
    let mut unlimited: (String, Vec<String>) =
        mixed("general-recent-keep", &["50", "100", "250", "500"]);
    unlimited.1.push(text("general-recent-unlimited"));
    vec![
        ids(
            "general-theme",
            &[
                "general-theme-system",
                "general-theme-light",
                "general-theme-dark",
            ],
        ),
        ids(
            "general-format-save-as",
            &["general-format-png", "general-format-jpeg"],
        ),
        unlimited,
        mixed("capture-countdown", &["Off", "3s", "5s", "10s"]),
        ids(
            "capture-selector-on",
            &[
                "capture-selector-on-pointer",
                "capture-selector-on-specific",
            ],
        ),
        ids(
            "overlay-layout",
            &[
                "overlay-layout-standard",
                "overlay-layout-sharing",
                "overlay-layout-minimal",
            ],
        ),
        ids(
            "overlay-position",
            &[
                "overlay-position-bottom-right",
                "overlay-position-bottom-left",
            ],
        ),
        ids(
            "overlay-card-size",
            &[
                "overlay-card-size-small",
                "overlay-card-size-medium",
                "overlay-card-size-large",
            ],
        ),
        ids(
            "recording-microphone",
            &["recording-microphone-off", "recording-microphone-default"],
        ),
        mixed(
            "recording-countdown",
            &["None", "1 second", "3 seconds", "5 seconds"],
        ),
        mixed("recording-export-fps", &["30 fps", "60 fps"]),
        ids(
            "recording-export-codec",
            &[
                "recording-export-codec-h264",
                "recording-export-codec-hevc",
                "recording-export-codec-av1",
            ],
        ),
        ids(
            "recording-export-container",
            &[
                "recording-export-container-mkv",
                "recording-export-container-mp4",
            ],
        ),
        ids(
            "recording-export-quality",
            &[
                "recording-export-quality-medium",
                "recording-export-quality-high",
                "recording-export-quality-very-high",
                "recording-export-quality-ultra",
            ],
        ),
        ids(
            "recording-recorder",
            &[
                "recording-recorder-auto",
                "recording-recorder-gsr",
                "recording-recorder-wf",
                "recording-recorder-portal",
            ],
        ),
        ids(
            "agents-consent",
            &[
                "agents-consent-once",
                "agents-consent-always-ask",
                "agents-consent-never-ask",
            ],
        ),
        ids(
            "sharing-days",
            &["sharing-days-1", "sharing-days-3", "sharing-days-7"],
        ),
    ]
}

/// Titles that sit beside a text field or a slider, which are as wide as an inline control may be.
fn field_titles() -> Vec<String> {
    [
        "general-file-name",
        "general-format-quality",
        "general-look-padding",
        "general-look-corner-radius",
        "general-look-shadow",
        "overlay-edge-margin",
        "overlay-hide-after",
        "agents-scope-outputs",
        "agents-scope-windows",
        "sharing-endpoint",
        "sharing-bucket",
        "sharing-region",
        "sharing-public-url",
        "sharing-prefix",
        "sharing-access-key-id",
        "sharing-secret-key",
    ]
    .into_iter()
    .map(text)
    .collect()
}

fn measure(fx: &mut Fixture, s: &str, size: f32, weight: FontWeight) -> f32 {
    let handle = fx.app.window.into();
    let s = s.to_owned();
    fx.app
        .cx
        .update_window(handle, |_, window, _| {
            let run = TextRun {
                len: s.len(),
                font: gpui::Font {
                    weight,
                    ..font(es_ui::theme::UI_FONT)
                },
                color: black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(s.into(), px(size), &[run], None)
                    .width,
            )
        })
        .unwrap()
}

#[test]
fn no_title_wraps_and_no_option_is_cut_at_the_default_or_minimum_width() {
    let mut fx = fixture(Caps::empty(), None, |_| {});
    let mut problems = Vec::new();
    for (w, _) in [WINDOW_SIZE, MIN_SIZE] {
        for (title, options) in choice_rows() {
            let width = segmented_width(options.iter().map(String::as_str));
            let inline = beside(width);
            let column = if inline {
                title_column_width(w, width)
            } else {
                row_content_width(w)
            };
            let title_w = measure(&mut fx, &title, ROW_TEXT, FontWeight::NORMAL);
            if title_w > column {
                problems.push(format!("{w}: \"{title}\" is {title_w} wide in {column}"));
            }
            let size = f32::from(TYPE.segmented.size);
            let labels: Vec<f32> = options
                .iter()
                .map(|o| measure(&mut fx, o, size, TYPE.segmented.weight))
                .collect();
            let cut = if inline {
                let room = (width - TRACK_INSET) / options.len() as f32 - SEGMENT_CHROME;
                labels.iter().any(|l| *l > room)
            } else {
                labels.iter().map(|l| l + SEGMENT_CHROME).sum::<f32>() > column - TRACK_INSET
            };
            if cut {
                problems.push(format!("{w}: \"{title}\" options {labels:?} are cut"));
            }
        }
        for title in field_titles() {
            let column = title_column_width(w, INLINE_CONTROL_MAX);
            let title_w = measure(&mut fx, &title, ROW_TEXT, FontWeight::NORMAL);
            if title_w > column {
                problems.push(format!("{w}: \"{title}\" is {title_w} wide in {column}"));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn the_empty_agent_scope_fields_say_that_empty_allows_every_one() {
    let mut fx = fixture(Caps::empty(), None, |_| {});
    let (outputs, windows) = fx.read(|v, _| v.agent_scope_fields());
    let shown = fx.read(|_, cx| {
        [&outputs, &windows].map(|f| {
            let f = f.read(cx);
            (
                f.value().to_string(),
                f.presentation().placeholder().to_string(),
            )
        })
    });
    assert_eq!(
        shown,
        [
            (String::new(), text("agents-scope-outputs-placeholder")),
            (String::new(), text("agents-scope-windows-placeholder")),
        ],
        "an empty field allows everything, so it must not look unset"
    );
    assert!(shown.iter().all(|(_, p)| !p.is_empty()));
}

#[test]
fn every_page_full_length_at_the_default_and_minimum_width() {
    for ((w, _), size_name) in [(WINDOW_SIZE, "default"), (MIN_SIZE, "minimum")] {
        for (pages, h) in [
            (
                &Page::ALL
                    .into_iter()
                    .filter(|p| *p != Page::Shortcuts)
                    .collect::<Vec<_>>()[..],
                PAGE_HEIGHT,
            ),
            (&[Page::Shortcuts][..], SHORTCUTS_HEIGHT),
        ] {
            let mut fx = fixture_sized((w, h), Caps::empty(), None, |_| {});
            for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
                fx.app.cx.update(|cx| theme::set_mode(mode, cx));
                for &page in pages {
                    fx.update(|v, cx| v.show(page, cx));
                    fx.shot(&format!("settings-full-{page:?}-{size_name}-{name}").to_lowercase());
                }
            }
        }
    }
}
