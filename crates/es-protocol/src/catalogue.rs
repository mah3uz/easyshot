use std::path::PathBuf;

use crate::{Route, Scope, ShortcutEntry, Target};

macro_rules! global {
    ($id:literal, $route:expr, $trigger:expr) => {
        ShortcutEntry {
            id: $id,
            scope: Scope::Global,
            title_id: concat!("shortcut-", $id),
            default_trigger: $trigger,
            target: Target::Route($route),
        }
    };
}

macro_rules! editor {
    ($id:literal, $trigger:expr) => {
        ShortcutEntry {
            id: $id,
            scope: Scope::Editor,
            title_id: concat!("shortcut-", $id),
            default_trigger: $trigger,
            target: Target::Action($id),
        }
    };
}

pub static CATALOGUE: &[ShortcutEntry] = &[
    global!("capture-bar", Route::CaptureBar, None),
    global!("gallery", Route::Gallery, None),
    global!("restore-last", Route::RestoreLast, None),
    global!("pin-last", Route::PinLast, None),
    global!("open-image", Route::Open(PathBuf::new()), None),
    global!("settings", Route::Settings(None), None),
    global!("unpin-all", Route::UnpinAll, None),
    global!("edit-clipboard", Route::EditClipboard, None),
    global!("capture-region", Route::CaptureRegion, None),
    global!("capture-fullscreen", Route::CaptureFullscreen, None),
    global!("capture-window", Route::CaptureWindow, None),
    global!("capture-previous", Route::CapturePrevious, None),
    global!("capture-timed", Route::CaptureTimed, None),
    global!("capture-region-copy", Route::CaptureRegionCopy, None),
    global!("capture-region-save", Route::CaptureRegionSave, None),
    global!(
        "capture-region-annotate",
        Route::CaptureRegionAnnotate,
        None
    ),
    global!("capture-region-pin", Route::CaptureRegionPin, None),
    global!("capture-scroll", Route::CaptureScroll, None),
    global!("ocr", Route::Ocr, None),
    global!("ocr-single-line", Route::OcrSingleLine, None),
    global!("color-picker", Route::ColorPicker, None),
    global!("record", Route::Record, None),
    global!("record-area", Route::RecordArea, None),
    global!("record-stop", Route::RecordStop, None),
    global!("record-pause", Route::RecordPause, None),
    global!("record-restart", Route::RecordRestart, None),
    global!("record-discard", Route::RecordDiscard, None),
    global!("deck-toggle", Route::DeckToggle, None),
    global!("deck-save-all", Route::DeckSaveAll, None),
    global!("deck-close-all", Route::DeckCloseAll, None),
    editor!("image-background", None),
    editor!("image-select", Some("H")),
    editor!("image-crop", Some("C")),
    editor!("image-rectangle", Some("R")),
    editor!("image-filled-rectangle", None),
    editor!("image-ellipse", Some("O")),
    editor!("image-line", Some("L")),
    editor!("image-arrow", Some("A")),
    editor!("image-freehand", None),
    editor!("image-counter", Some("1")),
    editor!("image-text", Some("T")),
    editor!("image-highlight", None),
    editor!("image-highlighter", Some("M")),
    editor!("image-pixelate", Some("P")),
    editor!("image-blur", Some("B")),
    editor!("image-save", Some("Ctrl+S")),
    editor!("image-export", None),
    editor!("image-copy", Some("Ctrl+Shift+C")),
    editor!("image-share", None),
    editor!("image-undo", Some("Ctrl+Z")),
    editor!("image-redo", Some("Ctrl+Shift+Z")),
    editor!("image-select-all", Some("Ctrl+A")),
    editor!("image-delete", Some("Delete")),
    editor!("image-zoom-in", Some("Ctrl+Equal")),
    editor!("image-zoom-out", Some("Ctrl+Minus")),
    editor!("image-fit", Some("Ctrl+1")),
    editor!("image-actual-size", Some("Ctrl+0")),
    editor!("image-increase-size", None),
    editor!("image-decrease-size", None),
    editor!("image-rotate-left", None),
    editor!("image-rotate-right", None),
    editor!("image-flip-horizontal", None),
    editor!("image-flip-vertical", None),
    editor!("video-save", Some("Ctrl+S")),
    editor!("video-export", None),
    editor!("video-share", None),
    editor!("video-play", Some("Space")),
    editor!("video-start", None),
    editor!("video-end", None),
    editor!("video-split-tool", Some("S")),
    editor!("video-cut", Some("C")),
    editor!("video-delete", Some("Delete")),
    editor!("video-undo", Some("Ctrl+Z")),
    editor!("video-redo", Some("Ctrl+Shift+Z")),
    editor!("video-zoom-in", Some("Ctrl+Equal")),
    editor!("video-zoom-out", Some("Ctrl+Minus")),
    editor!("video-fit", Some("Ctrl+0")),
    editor!("video-crop", None),
    editor!("video-blur", None),
    editor!("video-pixelate", None),
    editor!("video-inspector", None),
    editor!("video-add-zoom", None),
];

pub fn entry_by_id(id: &str) -> Option<&'static ShortcutEntry> {
    CATALOGUE.iter().find(|e| e.id == id)
}

/// `Open` matches whatever its path, so the file-picker entry answers for any file.
pub fn entry_for_route(route: &Route) -> Option<&'static ShortcutEntry> {
    CATALOGUE.iter().find(|e| match (&e.target, route) {
        (Target::Route(Route::Open(_)), Route::Open(_)) => true,
        (Target::Route(r), route) => r == route,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Trigger;
    use std::collections::HashSet;

    #[test]
    fn ids_and_title_ids_are_unique_and_derived_from_the_id() {
        let mut ids = HashSet::new();
        for e in CATALOGUE {
            assert!(ids.insert(e.id), "duplicate id {}", e.id);
            assert_eq!(e.title_id, format!("shortcut-{}", e.id));
        }
    }

    #[test]
    fn targets_match_scopes() {
        for e in CATALOGUE {
            match (&e.scope, &e.target) {
                (Scope::Global, Target::Route(_)) => {}
                (Scope::Editor, Target::Action(a)) => assert!(!a.is_empty()),
                _ => panic!("{} has a target that does not fit its scope", e.id),
            }
        }
    }

    #[test]
    fn counts_per_scope() {
        let global = CATALOGUE
            .iter()
            .filter(|e| e.scope == Scope::Global)
            .count();
        assert_eq!(global, 30);
        assert_eq!(CATALOGUE.len() - global, 52);
    }

    #[test]
    fn no_route_has_two_global_entries() {
        let mut seen = Vec::new();
        for e in CATALOGUE {
            if let Target::Route(r) = &e.target {
                assert!(!seen.contains(&r), "{} repeats a route", e.id);
                seen.push(r);
            }
        }
    }

    #[test]
    fn no_global_action_has_a_default_trigger() {
        for e in CATALOGUE.iter().filter(|e| e.scope == Scope::Global) {
            assert_eq!(e.default_trigger, None, "{} has a default key", e.id);
        }
    }

    #[test]
    fn default_triggers_parse_and_do_not_collide_within_a_context() {
        let mut seen = HashSet::new();
        for e in CATALOGUE {
            let Some(t) = e.default_trigger else { continue };
            assert!(Trigger::parse(t).is_ok(), "{} trigger {t}", e.id);
            let context = e.id.split('-').next().unwrap_or_default();
            let context = if e.scope == Scope::Global {
                "global"
            } else {
                context
            };
            assert!(seen.insert((context, t)), "{} reuses {t}", e.id);
        }
    }

    #[test]
    fn lookups_find_entries() {
        assert_eq!(
            entry_by_id("capture-region").unwrap().target,
            Target::Route(Route::CaptureRegion)
        );
        assert!(entry_by_id("nope").is_none());
        assert_eq!(entry_for_route(&Route::Ocr).unwrap().id, "ocr");
        assert_eq!(
            entry_for_route(&Route::Open("/x.png".into())).unwrap().id,
            "open-image"
        );
        assert!(entry_for_route(&Route::Quit).is_none());
    }
}
