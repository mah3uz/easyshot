use es_model::{
    AnnotationDocument, Envelope, FormatKind, Header, LastRegion, LastTarget, Migrated,
    PresetLibrary, RecordingCapture, RecordingInput, RecordingProject, Recovery, ShapeKind, State,
    StylePreset, Tool, migrate,
};
use serde::{Serialize, de::DeserializeOwned};
use std::path::PathBuf;

fn fixture(dir: &str, file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/formats")
        .join(dir)
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dir_of(kind: FormatKind) -> &'static str {
    kind.name().trim_start_matches("easyshot.")
}

/// Loads the v1 fixture, checks its envelope, and proves write-then-read is lossless.
fn load_v1<T>(kind: FormatKind) -> T
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let src = fixture(dir_of(kind), "v1.json");
    let env: Envelope<T> = serde_json::from_str(&src).expect("envelope parses");
    assert_eq!(env.format, kind.name());
    assert_eq!(env.version, 1);
    let value: serde_json::Value = serde_json::from_str(&src).expect("value parses");
    assert!(matches!(
        migrate(kind, env.version, value).expect("migrates"),
        Migrated::Current(_)
    ));
    let written = serde_json::to_string_pretty(&env).expect("serialises");
    let again: Envelope<T> = serde_json::from_str(&written).expect("re-reads");
    assert_eq!(again, env, "{} must survive write and read", kind.name());
    env.body
}

#[test]
fn style_v1() {
    let s: StylePreset = load_v1(FormatKind::Style);
    assert_eq!(s.tool, Tool::Arrow);
    assert_eq!(s.swatch.id, "custom-10-200-30-255");
    assert_eq!(s.stroke_width, 6.0);
    assert!(s.text_italic && s.text_underline && !s.text_bold);
}

#[test]
fn bg_presets_v1() {
    let lib: PresetLibrary = load_v1(FormatKind::BgPresets);
    assert_eq!(lib.presets.len(), 2);
    assert_eq!(lib.active().map(|p| p.name.as_str()), Some("Dusk"));
    let dusk = &lib.presets[1].background;
    assert_eq!(dusk.camera.zoom, 1.2);
    assert_eq!(dusk.camera.field_of_view_degrees, 24.0);
    assert!(dusk.progressive_blur.is_active() && dusk.border.is_visible());
    assert!(dusk.watermark.is_visible());
}

#[test]
fn document_v1_holds_every_shape_kind_and_its_binding() {
    let doc: AnnotationDocument = load_v1(FormatKind::Document);
    assert_eq!(
        doc.base_image_file_name,
        "easyshot_2026-01-01-093000.base.png"
    );
    assert_eq!(doc.shapes.len(), 7);
    let tools: Vec<_> = doc.shapes.iter().map(|s| s.tool()).collect();
    assert_eq!(
        tools,
        [
            Tool::Ellipse,
            Tool::Freehand,
            Tool::Arrow,
            Tool::Text,
            Tool::Pixelate,
            Tool::Highlight,
            Tool::NumberedCircle
        ]
    );
    assert!(doc.shapes[0].mirrored && doc.shapes[0].opacity == 0.9);
    assert!(matches!(&doc.shapes[3].kind, ShapeKind::Text(t) if t.text == "Look here"));
    assert_eq!(doc.bindings.len(), 1);
    assert_eq!(doc.bindings[0].normalized_anchor.y, 0.75);
    assert_eq!(doc.next_numbered_value(), 4);
    assert!(doc.has_content());
}

#[test]
fn a_newer_document_is_read_only_and_never_parsed_as_current() {
    let src = fixture("document", "newer.json");
    let header: Header = serde_json::from_str(&src).unwrap();
    assert!(header.version > FormatKind::Document.version());
    assert!(serde_json::from_str::<Envelope<AnnotationDocument>>(&src).is_err());
    let value = serde_json::from_str(&src).unwrap();
    assert_eq!(
        migrate(FormatKind::Document, header.version, value).unwrap(),
        Migrated::NewerReadOnly { version: 99 }
    );
}

#[test]
fn recording_capture_v1() {
    let c: RecordingCapture = load_v1(FormatKind::Capture);
    assert_eq!(
        (c.pixel_width, c.pixel_height, c.pixel_scale),
        (2560, 1440, 1.5)
    );
    assert!(c.includes_system_audio && !c.includes_microphone);
    assert_eq!(c.created_at.unix_timestamp(), 1_770_091_506);
}

#[test]
fn recording_input_v1() {
    let i: RecordingInput = load_v1(FormatKind::Input);
    assert_eq!(
        (i.travel.len(), i.presses.len(), i.keystrokes.len()),
        (2, 2, 1)
    );
    assert_eq!(i.keystrokes[0].modifiers, ["ctrl", "shift"]);
    assert!(i.is_sanitized);
}

#[test]
fn recording_project_v1() {
    let p: RecordingProject = load_v1(FormatKind::Project);
    assert_eq!(p.display_name, "Demo walkthrough");
    assert_eq!(p.last_opened_at.map(|t| t.offset().whole_hours()), Some(10));
    assert_eq!(
        p.export_path,
        Some(PathBuf::from("/home/user/Videos/demo.mp4"))
    );
}

#[test]
fn recovery_v1_reads_a_document_written_with_most_fields_absent() {
    let r: Recovery = load_v1(FormatKind::Recovery);
    assert_eq!(r.document.shapes.len(), 1);
    assert_eq!(r.document.background.padding, 0.08);
    assert_eq!(r.document.background.shadow, 0.36);
    assert!(r.source.is_some());
}

#[test]
fn state_v1() {
    let s: State = load_v1(FormatKind::State);
    assert_eq!(s.counter, 42);
    assert_eq!(s.last_region.map(|r| (r.w, r.h)), Some((640, 480)));
    assert_eq!(s.recent_wallpapers.len(), 2);
    assert_eq!(s.inspector_sections.get("camera"), Some(&false));
    assert_eq!(
        s.deck_layout.get("top_right").map(String::as_str),
        Some("dismiss")
    );
}

#[test]
fn state_v1_migrates_its_region_into_the_target_capture_previous_repeats() {
    let value: serde_json::Value = serde_json::from_str(&fixture("state", "v1.json")).unwrap();
    let Migrated::Current(v2) = migrate(FormatKind::State, 1, value).unwrap() else {
        panic!("v1 is older than this build");
    };
    let s = serde_json::from_value::<Envelope<State>>(v2).unwrap().body;
    let region = LastRegion {
        output: "DP-2".into(),
        x: 100,
        y: 200,
        w: 640,
        h: 480,
    };
    assert_eq!(s.last_target, Some(LastTarget::Region(region.clone())));
    assert_eq!(s.last_region, Some(region));
    assert_eq!(s.counter, 42);
}

#[test]
fn state_v2_reads_each_kind_of_target() {
    let src = fixture("state", "v2.json");
    let env: Envelope<State> = serde_json::from_str(&src).unwrap();
    assert_eq!(env.version, FormatKind::State.version());
    assert_eq!(
        env.body.last_target,
        Some(LastTarget::Window {
            id: "94".into(),
            app: "foot".into(),
            title: "~/src".into(),
        })
    );
    let output: LastTarget = serde_json::from_str(r#"{"output":"DP-1"}"#).unwrap();
    assert_eq!(output, LastTarget::Output("DP-1".into()));
    let written = serde_json::to_value(&env).unwrap();
    assert_eq!(written["last_target"]["window"]["id"], "94");
}

#[test]
fn every_format_kind_has_a_v1_fixture_with_its_own_name() {
    for kind in FormatKind::ALL {
        let header: Header = serde_json::from_str(&fixture(dir_of(kind), "v1.json")).unwrap();
        assert_eq!(header.format, kind.name());
        assert_eq!(header.version, 1);
    }
}

#[test]
fn unknown_fields_are_tolerated_in_every_body() {
    fn tolerant<T: DeserializeOwned>(kind: FormatKind) {
        let mut v: serde_json::Value =
            serde_json::from_str(&fixture(dir_of(kind), "v1.json")).unwrap();
        v["field_from_a_later_minor"] = serde_json::json!({"nested": [1, 2]});
        serde_json::from_value::<Envelope<T>>(v).unwrap_or_else(|e| panic!("{}: {e}", kind.name()));
    }
    tolerant::<StylePreset>(FormatKind::Style);
    tolerant::<PresetLibrary>(FormatKind::BgPresets);
    tolerant::<AnnotationDocument>(FormatKind::Document);
    tolerant::<RecordingCapture>(FormatKind::Capture);
    tolerant::<RecordingInput>(FormatKind::Input);
    tolerant::<RecordingProject>(FormatKind::Project);
    tolerant::<Recovery>(FormatKind::Recovery);
    tolerant::<State>(FormatKind::State);
}

#[test]
fn an_empty_body_is_a_valid_default_for_every_kind() {
    fn empty<T: DeserializeOwned + PartialEq + std::fmt::Debug + Default>() {
        let body: T = serde_json::from_str("{}").unwrap();
        assert_eq!(body, T::default());
    }
    empty::<PresetLibrary>();
    empty::<AnnotationDocument>();
    empty::<RecordingInput>();
    empty::<State>();
    empty::<StylePreset>();
}

#[test]
fn a_recording_takes_system_sound_and_no_microphone_unless_told_otherwise() {
    let r: es_model::config::Recording = toml::from_str("fps = 30").unwrap();
    assert!(r.system_audio, "a silent recording surprises the user");
    assert!(r.mic.is_empty(), "the microphone is opt-in");
}
