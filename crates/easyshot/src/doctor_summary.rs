use crate::doctor::{render_config, render_input};
use crate::strings::fl;
use serde_json::Value;
use std::fmt::Write as _;

pub(crate) const CAPS: &[&str] = &[
    "LAYER_SHELL",
    "CAPTURE_OUTPUT",
    "CAPTURE_WINDOW",
    "WINDOW_LIST",
    "WINDOW_GEOMETRY",
    "CLIPBOARD_NO_FOCUS",
    "TRAY",
    "NOTIFY_ACTIONS",
    "PORTAL_SHORTCUTS",
    "SHORTCUT_TRIGGERS",
    "POINTER_GLOBAL",
    "VIRTUAL_INPUT",
    "RECORDER",
    "OCR",
    "BIND_CHECK",
    "FILE_PICKER",
    "APPEARANCE",
    "CURSOR_STREAM",
    "FOCUS_WINDOW",
    "RECORD_WINDOW",
];

fn cap_text(flag: &str, missing: bool) -> String {
    let (label, note) = match flag {
        "LAYER_SHELL" => (
            fl!("doctor-cap-layer-shell"),
            fl!("doctor-cap-layer-shell-missing"),
        ),
        "CAPTURE_OUTPUT" => (
            fl!("doctor-cap-capture-output"),
            fl!("doctor-cap-capture-output-missing"),
        ),
        "CAPTURE_WINDOW" => (
            fl!("doctor-cap-capture-window"),
            fl!("doctor-cap-capture-window-missing"),
        ),
        "WINDOW_LIST" => (
            fl!("doctor-cap-window-list"),
            fl!("doctor-cap-window-list-missing"),
        ),
        "WINDOW_GEOMETRY" => (
            fl!("doctor-cap-window-geometry"),
            fl!("doctor-cap-window-geometry-missing"),
        ),
        "CLIPBOARD_NO_FOCUS" => (
            fl!("doctor-cap-clipboard-no-focus"),
            fl!("doctor-cap-clipboard-no-focus-missing"),
        ),
        "TRAY" => (fl!("doctor-cap-tray"), fl!("doctor-cap-tray-missing")),
        "NOTIFY_ACTIONS" => (
            fl!("doctor-cap-notify-actions"),
            fl!("doctor-cap-notify-actions-missing"),
        ),
        "PORTAL_SHORTCUTS" => (
            fl!("doctor-cap-portal-shortcuts"),
            fl!("doctor-cap-portal-shortcuts-missing"),
        ),
        "SHORTCUT_TRIGGERS" => (
            fl!("doctor-cap-shortcut-triggers"),
            fl!("doctor-cap-shortcut-triggers-missing"),
        ),
        "POINTER_GLOBAL" => (
            fl!("doctor-cap-pointer-global"),
            fl!("doctor-cap-pointer-global-missing"),
        ),
        "VIRTUAL_INPUT" => (
            fl!("doctor-cap-virtual-input"),
            fl!("doctor-cap-virtual-input-missing"),
        ),
        "RECORDER" => (
            fl!("doctor-cap-recorder"),
            fl!("doctor-cap-recorder-missing"),
        ),
        "OCR" => (fl!("doctor-cap-ocr"), fl!("doctor-cap-ocr-missing")),
        "BIND_CHECK" => (
            fl!("doctor-cap-bind-check"),
            fl!("doctor-cap-bind-check-missing"),
        ),
        "FILE_PICKER" => (
            fl!("doctor-cap-file-picker"),
            fl!("doctor-cap-file-picker-missing"),
        ),
        "APPEARANCE" => (
            fl!("doctor-cap-appearance"),
            fl!("doctor-cap-appearance-missing"),
        ),
        "CURSOR_STREAM" => (
            fl!("doctor-cap-cursor-stream"),
            fl!("doctor-cap-cursor-stream-missing"),
        ),
        "FOCUS_WINDOW" => (
            fl!("doctor-cap-focus-window"),
            fl!("doctor-cap-focus-window-missing"),
        ),
        "RECORD_WINDOW" => (
            fl!("doctor-cap-record-window"),
            fl!("doctor-cap-record-window-missing"),
        ),
        _ => (flag.to_owned(), String::new()),
    };
    if missing { note } else { label }
}

/// A tool with an alternative is not missing while its concern has a chosen provider.
const ALTERNATIVES: &[(&str, &str)] = &[
    ("gpu-screen-recorder", "recorders"),
    ("wf-recorder", "recorders"),
];

struct Style(bool);

impl Style {
    fn ok(&self) -> &'static str {
        if self.0 { "\x1b[32m✓\x1b[0m" } else { "ok" }
    }
    fn no(&self) -> &'static str {
        if self.0 { "\x1b[31m✗\x1b[0m" } else { "no" }
    }
    fn heading(&self, text: &str) -> String {
        if self.0 {
            format!("\x1b[1m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }
}

pub fn render(raw: &str, styled: bool) -> String {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return format!("{raw}\n");
    };
    let style = Style(styled);
    let mut out = String::new();
    let text = |v: &Value| v.as_str().unwrap_or("-").to_owned();
    if let Some(source) = v.get("source").and_then(Value::as_str) {
        let _ = writeln!(out, "{}", fl!("doctor-daemon-down", source = source));
    }
    let compositor = &v["compositor"];
    let _ = writeln!(
        out,
        "{} {} {}",
        style.heading(&fl!("doctor-compositor")),
        text(&compositor["name"]),
        compositor["version"].as_str().unwrap_or("")
    );
    let has: Vec<&str> = v["caps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let _ = writeln!(out, "\n{}", style.heading(&fl!("doctor-heading-works")));
    for flag in CAPS {
        let label = cap_text(flag, false);
        if has.contains(flag) {
            let _ = writeln!(out, "  {} {label}", style.ok());
        } else {
            let note = cap_text(flag, true);
            let _ = writeln!(out, "  {} {label}: {note}", style.no());
        }
    }
    let providers = v["providers"].as_array().map_or(&[][..], Vec::as_slice);
    if !providers.is_empty() {
        let _ = writeln!(out, "\n{}", style.heading(&fl!("doctor-heading-providers")));
        for p in providers {
            let forced = p["forced"].as_str().map_or(String::new(), |f| {
                format!(" {}", fl!("doctor-forced", forced = f))
            });
            let _ = writeln!(
                out,
                "  {}: {}{forced}",
                text(&p["capability"]),
                p["chosen"]
                    .as_str()
                    .map_or_else(|| fl!("doctor-provider-none"), str::to_owned)
            );
        }
    }
    let chosen = |concern: &str| {
        providers
            .iter()
            .any(|p| p["capability"] == concern && p["chosen"].is_string())
    };
    let missing: Vec<&Value> = v["missing_tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| {
            !ALTERNATIVES
                .iter()
                .any(|(tool, concern)| m["tool"] == *tool && chosen(concern))
        })
        .collect();
    if !missing.is_empty() {
        let _ = writeln!(out, "\n{}", style.heading(&fl!("doctor-heading-missing")));
        for m in missing {
            let _ = writeln!(
                out,
                "  {} {}: {}",
                style.no(),
                text(&m["tool"]),
                fl!("doctor-install-package", package = text(&m["package"]))
            );
        }
    }
    render_input(&v, &mut out);
    render_config(&v, &mut out);
    let errors = v["errors"].as_array().map_or(&[][..], Vec::as_slice);
    if !errors.is_empty() {
        let _ = writeln!(out, "\n{}", style.heading(&fl!("doctor-heading-errors")));
        for e in errors {
            let _ = writeln!(out, "  {}", text(e));
        }
    }
    let _ = writeln!(out, "\n{}", fl!("doctor-raw-hint"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report() -> String {
        json!({
            "compositor": {"name": "Hyprland", "version": "0.50"},
            "caps": ["LAYER_SHELL", "CAPTURE_OUTPUT", "RECORDER"],
            "providers": [
                {"capability": "capture", "chosen": "ext-image-copy", "forced": null,
                 "rejected": [{"provider": "wlr-screencopy", "reason": "outranked by ext-image-copy"}]},
                {"capability": "tray", "chosen": null, "forced": "none", "rejected": []},
                {"capability": "recorders", "chosen": "gsr-native", "forced": null, "rejected": []}
            ],
            "missing_tools": [
                {"tool": "wf-recorder", "package": "wf-recorder"},
                {"tool": "tesseract", "package": "tesseract"}
            ],
            "wayland_globals": {"wl_shm": 1},
            "portals": {"FileChooser": true},
            "errors": []
        })
        .to_string()
    }

    #[test]
    fn every_capability_has_a_line() {
        let names: Vec<_> = es_backend::Caps::all()
            .iter_names()
            .map(|(n, _)| n)
            .collect();
        let listed: Vec<_> = CAPS.to_vec();
        assert_eq!(names, listed);
        for flag in CAPS {
            for missing in [false, true] {
                let text = cap_text(flag, missing);
                assert!(
                    !text.is_empty() && !text.contains("No localization"),
                    "{flag}"
                );
            }
        }
    }

    #[test]
    fn plain_summary_is_ascii_and_says_what_a_gap_means() {
        let out = render(&report(), false);
        assert!(out.is_ascii(), "{out}");
        assert!(!out.contains('\x1b'));
        for want in [
            "Compositor: Hyprland 0.50",
            "  ok Capture a screen\n",
            "  no Capture a window: you cannot capture a single window\n",
            "  capture: ext-image-copy\n",
            "  tray: none (forced: none)\n",
            "  no tesseract: install the package tesseract\n",
            "doctor --raw",
        ] {
            assert!(out.contains(want), "{want} missing from\n{out}");
        }
        for gone in ["rejected", "outranked", "wl_shm", "FileChooser"] {
            assert!(!out.contains(gone), "{gone} in\n{out}");
        }
    }

    #[test]
    fn styled_summary_uses_colour_and_glyphs() {
        let out = render(&report(), true);
        assert!(out.contains("\x1b[32m✓\x1b[0m Capture a screen\n"), "{out}");
        assert!(
            out.contains("\x1b[31m✗\x1b[0m Capture a window: you cannot capture a single window\n"),
            "{out}"
        );
        assert!(out.contains("\x1b[1mCompositor:\x1b[0m Hyprland 0.50"));
    }

    #[test]
    fn a_recorder_tool_is_missing_only_when_no_recorder_is_chosen() {
        let with = render(&report(), false);
        assert!(!with.contains("wf-recorder"), "{with}");
        let mut v: Value = serde_json::from_str(&report()).unwrap();
        v["providers"][2]["chosen"] = Value::Null;
        let without = render(&v.to_string(), false);
        assert!(
            without.contains("no wf-recorder: install the package wf-recorder"),
            "{without}"
        );
    }
}
