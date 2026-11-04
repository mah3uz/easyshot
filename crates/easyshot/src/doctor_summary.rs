use crate::doctor::{render_config, render_input};
use serde_json::Value;
use std::fmt::Write as _;

/// `(flag, what it is, what a missing one means for you)`.
pub(crate) const CAPS: &[(&str, &str, &str)] = &[
    (
        "LAYER_SHELL",
        "Overlays on screen",
        "the selection and the deck cannot draw over your screen",
    ),
    (
        "CAPTURE_OUTPUT",
        "Capture a screen",
        "you cannot capture a whole screen or a region",
    ),
    (
        "CAPTURE_WINDOW",
        "Capture a window",
        "you cannot capture a single window",
    ),
    (
        "WINDOW_LIST",
        "List windows",
        "you cannot pick a window by pointing at it",
    ),
    (
        "WINDOW_GEOMETRY",
        "Window positions",
        "windows cannot be outlined or snapped to",
    ),
    (
        "CLIPBOARD_NO_FOCUS",
        "Copy without focus",
        "copying may fail unless an easyshot window has focus",
    ),
    ("TRAY", "Tray icon", "there is no tray icon"),
    (
        "NOTIFY_ACTIONS",
        "Notification buttons",
        "notifications have no buttons",
    ),
    (
        "PORTAL_SHORTCUTS",
        "Shortcuts through the portal",
        "easyshot cannot register keys through the desktop portal",
    ),
    (
        "SHORTCUT_TRIGGERS",
        "Shortcut keys",
        "easyshot cannot read which keys you have set",
    ),
    (
        "POINTER_GLOBAL",
        "Pointer position",
        "easyshot cannot tell where the pointer is on screen",
    ),
    (
        "VIRTUAL_INPUT",
        "Scrolling for you",
        "scroll capture cannot scroll the page itself",
    ),
    (
        "RECORDER",
        "Screen recording",
        "you cannot record the screen",
    ),
    ("OCR", "Read text", "you cannot read text from a capture"),
    (
        "BIND_CHECK",
        "Shortcut check",
        "easyshot cannot tell whether your keys are bound",
    ),
    (
        "FILE_PICKER",
        "File dialogs",
        "open and save use easyshot's own dialog",
    ),
    (
        "APPEARANCE",
        "Light and dark mode",
        "easyshot does not follow your light or dark setting",
    ),
    (
        "CURSOR_STREAM",
        "Cursor in recordings",
        "recordings cannot show the pointer",
    ),
];

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
        let _ = writeln!(out, "The daemon is not running; this is a {source} probe.");
    }
    let compositor = &v["compositor"];
    let _ = writeln!(
        out,
        "{} {} {}",
        style.heading("Compositor:"),
        text(&compositor["name"]),
        compositor["version"].as_str().unwrap_or("")
    );
    let has: Vec<&str> = v["caps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let _ = writeln!(out, "\n{}", style.heading("What works"));
    for (flag, label, note) in CAPS {
        if has.contains(flag) {
            let _ = writeln!(out, "  {} {label}", style.ok());
        } else {
            let _ = writeln!(out, "  {} {label}: {note}", style.no());
        }
    }
    let providers = v["providers"].as_array().map_or(&[][..], Vec::as_slice);
    if !providers.is_empty() {
        let _ = writeln!(out, "\n{}", style.heading("Chosen providers"));
        for p in providers {
            let forced = p["forced"]
                .as_str()
                .map_or(String::new(), |f| format!(" (forced: {f})"));
            let _ = writeln!(
                out,
                "  {}: {}{forced}",
                text(&p["capability"]),
                p["chosen"].as_str().unwrap_or("none")
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
        let _ = writeln!(out, "\n{}", style.heading("Missing tools"));
        for m in missing {
            let _ = writeln!(
                out,
                "  {} {}: install the package {}",
                style.no(),
                text(&m["tool"]),
                text(&m["package"])
            );
        }
    }
    render_input(&v, &mut out);
    render_config(&v, &mut out);
    let errors = v["errors"].as_array().map_or(&[][..], Vec::as_slice);
    if !errors.is_empty() {
        let _ = writeln!(out, "\n{}", style.heading("Errors"));
        for e in errors {
            let _ = writeln!(out, "  {}", text(e));
        }
    }
    let _ = writeln!(out, "\nRun `easyshot doctor --raw` for the full report.");
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
        let listed: Vec<_> = CAPS.iter().map(|(n, ..)| *n).collect();
        assert_eq!(names, listed);
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
