use es_model::{
    AgentRecord, AnnotationDocument, AudioTrack, BucketKeys, Envelope, FontFamily, FormatKind,
    GradientLibrary, Header, LastRegion, LastTarget, Migrated, PresetLibrary, RecordingCapture,
    RecordingInput, RecordingProject, Recovery, ShapeKind, ShareDelete, ShareKind, ShareSource,
    ShareState, ShareTarget, Shares, State, StylePreset, Tool, migrate,
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
fn gradients_v1_keeps_every_stop_and_the_axis() {
    let lib: GradientLibrary = load_v1(FormatKind::Gradients);
    assert_eq!(lib.gradients.len(), 1);
    let harbour = &lib.gradients[0];
    assert!(harbour.is_user());
    assert_eq!(harbour.title, "Harbour");
    assert_eq!(harbour.colors.len(), 3);
    assert_eq!(harbour.angle_degrees(), 90.0);
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
fn document_v2_keeps_a_highlighter_stroke() {
    let src = fixture("document", "v2.json");
    let env: Envelope<AnnotationDocument> = serde_json::from_str(&src).unwrap();
    assert_eq!(env.version, 2);
    let ShapeKind::Highlighter(h) = &env.body.shapes[0].kind else {
        panic!("a highlighter");
    };
    assert_eq!((h.points.len(), h.stroke_width), (2, 24.0));
    assert_eq!(env.body.shapes[0].tool(), Tool::Highlighter);
    let again: Envelope<AnnotationDocument> =
        serde_json::from_str(&serde_json::to_string(&env).unwrap()).unwrap();
    assert_eq!(again, env);
}

#[test]
fn a_v2_document_migrates_and_its_text_has_no_box() {
    let src = fixture("document", "v2.json");
    let value: serde_json::Value = serde_json::from_str(&src).unwrap();
    let Migrated::Current(v) = migrate(FormatKind::Document, 2, value).unwrap() else {
        panic!("migrates");
    };
    assert_eq!(v["version"], FormatKind::Document.version());
    let env: Envelope<AnnotationDocument> = serde_json::from_value(v).unwrap();
    assert_eq!(env.body.shapes.len(), 1);
    let old = r#"{"id":"00000000-0000-4000-8000-000000000031","x":0.0,"y":0.0,"rotation":0.0,"opacity":1.0,"kind":{"type":"text","text":"Hi"}}"#;
    let shape: es_model::AnnoShape = serde_json::from_str(old).unwrap();
    let ShapeKind::Text(t) = shape.kind else {
        panic!("text");
    };
    assert!(t.fill.is_none() && t.border.is_none());
}

#[test]
fn document_v3_keeps_a_text_box() {
    let src = fixture("document", "v3.json");
    let env: Envelope<AnnotationDocument> = serde_json::from_str(&src).unwrap();
    assert_eq!(env.version, 3);
    let ShapeKind::Text(t) = &env.body.shapes[0].kind else {
        panic!("text");
    };
    assert_eq!(t.fill.as_ref().map(|f| f.id.as_str()), Some("blue"));
    assert_eq!(t.border.as_ref().map(|b| b.width), Some(4.0));
    assert_eq!(t.padding, 12.0);
    let again: Envelope<AnnotationDocument> =
        serde_json::from_str(&serde_json::to_string(&env).unwrap()).unwrap();
    assert_eq!(again, env);
}

/// An installed family is tagged by name, so it can never collide with a bundled one.
#[test]
fn document_v4_names_an_installed_family_beside_a_bundled_one() {
    assert_eq!(FormatKind::Document.version(), 4);
    let src = fixture("document", "v4.json");
    let env: Envelope<AnnotationDocument> = serde_json::from_str(&src).unwrap();
    assert_eq!(env.version, FormatKind::Document.version());
    let families: Vec<_> = env
        .body
        .shapes
        .iter()
        .map(|s| match &s.kind {
            ShapeKind::Text(t) => t.font_family.clone(),
            _ => panic!("text"),
        })
        .collect();
    assert_eq!(
        families,
        [FontFamily::System("Noto Serif".into()), FontFamily::Compact]
    );
    let written = serde_json::to_value(&env).unwrap();
    assert_eq!(
        written["shapes"][0]["kind"]["font_family"],
        serde_json::json!({"system": "Noto Serif"})
    );
    assert_eq!(written["shapes"][1]["kind"]["font_family"], "compact");
    let again: Envelope<AnnotationDocument> = serde_json::from_value(written).unwrap();
    assert_eq!(again, env);
}

#[test]
fn a_v3_document_opens_as_the_version_with_its_bundled_family() {
    let src = fixture("document", "v3.json");
    let value: serde_json::Value = serde_json::from_str(&src).unwrap();
    let Migrated::Current(v) = migrate(FormatKind::Document, 3, value).unwrap() else {
        panic!("migrates");
    };
    assert_eq!(v["version"], 4);
    let env: Envelope<AnnotationDocument> = serde_json::from_value(v).unwrap();
    let ShapeKind::Text(t) = &env.body.shapes[0].kind else {
        panic!("text");
    };
    assert_eq!(t.font_family, FontFamily::Pro);
}

#[test]
fn the_saved_style_remembers_an_installed_family() {
    assert_eq!(FormatKind::Style.version(), 2);
    let s = StylePreset {
        text_font_family: FontFamily::System("Noto Serif".into()),
        ..StylePreset::default()
    };
    let env = Envelope::new(FormatKind::Style, s);
    let written = serde_json::to_string(&env).unwrap();
    let back: Envelope<StylePreset> = serde_json::from_str(&written).unwrap();
    assert_eq!(back.version, 2);
    assert_eq!(
        back.body.text_font_family,
        FontFamily::System("Noto Serif".into())
    );
    let value = serde_json::from_str(&fixture("style", "v1.json")).unwrap();
    let Migrated::Current(v) = migrate(FormatKind::Style, 1, value).unwrap() else {
        panic!("migrates");
    };
    assert_eq!(v["version"], 2);
}

#[test]
fn a_v1_document_migrates_to_the_version() {
    let src = fixture("document", "v1.json");
    let value: serde_json::Value = serde_json::from_str(&src).unwrap();
    let Migrated::Current(v) = migrate(FormatKind::Document, 1, value).unwrap() else {
        panic!("migrates");
    };
    assert_eq!(v["version"], FormatKind::Document.version());
    let env: Envelope<AnnotationDocument> = serde_json::from_value(v).unwrap();
    assert_eq!(env.body.shapes.len(), 7);
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
fn recording_capture_v2_lists_the_sound_tracks_in_file_order() {
    let src = fixture("capture", "v2.json");
    let env: Envelope<RecordingCapture> = serde_json::from_str(&src).unwrap();
    assert_eq!(env.version, FormatKind::Capture.version());
    assert_eq!(env.body.audio_tracks, [AudioTrack::System, AudioTrack::Mic]);
    let old: RecordingCapture = load_v1(FormatKind::Capture);
    assert!(old.audio_tracks.is_empty());
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
}

#[test]
fn agent_v1() {
    let a: AgentRecord = load_v1(FormatKind::Agent);
    assert!(a.always.is_some());
    assert_eq!(a.log.len(), 2);
    assert_eq!(a.log[0].comm.as_deref(), Some("claude"));
    assert_eq!(a.log[1].result, "access_denied");
}

#[test]
fn shares_v1_keeps_both_targets_sources_and_delete_rights() {
    let s: Shares = load_v1(FormatKind::Shares);
    assert_eq!(s.entries.len(), 2);
    let video = &s.entries[0];
    assert_eq!(video.kind, ShareKind::Video);
    assert_eq!(video.days, Some(3));
    assert_eq!(video.delete, ShareDelete::FreeToken("tok-abc".into()));
    assert!(matches!(video.source, ShareSource::Recording(_)));
    let image = &s.entries[1];
    assert_eq!(
        image.target,
        ShareTarget::Bucket {
            label: "my-bucket".into()
        }
    );
    assert_eq!((image.days, image.expires_at), (None, None));
    assert_eq!(image.delete, ShareDelete::Bucket);
    assert_eq!(image.state, ShareState::Uploading);
}

#[test]
fn bucket_keys_v1() {
    let k: BucketKeys = load_v1(FormatKind::BucketKeys);
    assert_eq!(k.access_key_id, "AKIAEXAMPLEKEY");
    assert_eq!(k.secret_access_key, "example-secret-not-real");
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
    tolerant::<GradientLibrary>(FormatKind::Gradients);
    tolerant::<AnnotationDocument>(FormatKind::Document);
    tolerant::<RecordingCapture>(FormatKind::Capture);
    tolerant::<RecordingInput>(FormatKind::Input);
    tolerant::<RecordingProject>(FormatKind::Project);
    tolerant::<Recovery>(FormatKind::Recovery);
    tolerant::<State>(FormatKind::State);
    tolerant::<AgentRecord>(FormatKind::Agent);
}

#[test]
fn an_empty_body_is_a_valid_default_for_every_kind() {
    fn empty<T: DeserializeOwned + PartialEq + std::fmt::Debug + Default>() {
        let body: T = serde_json::from_str("{}").unwrap();
        assert_eq!(body, T::default());
    }
    empty::<PresetLibrary>();
    empty::<GradientLibrary>();
    empty::<AnnotationDocument>();
    empty::<RecordingInput>();
    empty::<State>();
    empty::<StylePreset>();
    empty::<AgentRecord>();
}

#[test]
fn a_recording_takes_system_sound_and_no_microphone_unless_told_otherwise() {
    let r: es_model::config::Recording = toml::from_str("fps = 30").unwrap();
    assert!(r.system_audio, "a silent recording surprises the user");
    assert!(r.mic.is_empty(), "the microphone is opt-in");
}

#[test]
fn a_config_without_the_whole_screen_bar_key_shows_it() {
    let r: es_model::config::Recording = toml::from_str("fps = 30").unwrap();
    assert!(r.whole_screen_bar);
    let off: es_model::config::Recording = toml::from_str("whole_screen_bar = false").unwrap();
    assert!(!off.whole_screen_bar);
}
