#![allow(clippy::unwrap_used)]
use es_model::{AnnotationDocument, LastTarget, StylePreset};
use es_store::{Error, Paths, document, presets, state};
use std::os::unix::fs::PermissionsExt;

fn newer(kind: &str) -> String {
    format!(r#"{{"format":"{kind}","version":7,"future_field":{{"x":1}}}}"#)
}

fn paths_with_dirs(root: &std::path::Path) -> Paths {
    let paths = Paths::rooted(root);
    paths.ensure_all().unwrap();
    paths
}

#[test]
fn state_round_trips_through_the_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let mut f = state::open(&paths).unwrap();
    assert_eq!(f.value.counter, 1);
    f.value.counter = 42;
    f.save().unwrap();
    let text = std::fs::read_to_string(paths.state_file()).unwrap();
    assert!(text.contains("\"format\": \"easyshot.state\""));
    assert_eq!(state::open(&paths).unwrap().value.counter, 42);
    let mode = std::fs::metadata(paths.state_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn a_v1_state_opens_with_its_region_as_the_last_target_and_saves_as_v2() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    std::fs::write(
        paths.state_file(),
        r#"{"format":"easyshot.state","version":1,"counter":5,
            "last_region":{"output":"DP-2","x":1,"y":2,"w":30,"h":40}}"#,
    )
    .unwrap();
    let f = state::open(&paths).unwrap();
    assert_eq!(f.newer_version(), None);
    let Some(LastTarget::Region(region)) = &f.value.last_target else {
        panic!("no region target: {:?}", f.value.last_target);
    };
    assert_eq!(
        (region.output.as_str(), region.w, region.h),
        ("DP-2", 30, 40)
    );
    f.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(paths.state_file()).unwrap()).unwrap();
    assert_eq!(written["version"], 2);
    assert_eq!(written["counter"], 5);
    assert_eq!(written["last_target"]["region"]["output"], "DP-2");
}

#[test]
fn newer_state_refuses_writes_and_the_file_is_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    let body = newer("easyshot.state");
    std::fs::write(paths.state_file(), &body).unwrap();
    let f = state::open(&paths).unwrap();
    assert_eq!(f.newer_version(), Some(7));
    assert!(matches!(f.save(), Err(Error::NewerVersion { version: 7 })));
    assert_eq!(std::fs::read_to_string(paths.state_file()).unwrap(), body);
}

#[test]
fn style_preset_round_trips_and_newer_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let mut f = presets::open_style(&paths).unwrap();
    f.value = StylePreset {
        stroke_width: 9.0,
        ..StylePreset::default()
    };
    f.save().unwrap();
    assert_eq!(presets::open_style(&paths).unwrap().value.stroke_width, 9.0);
    std::fs::write(paths.style_file(), newer("easyshot.style")).unwrap();
    assert!(presets::open_style(&paths).unwrap().save().is_err());
}

#[test]
fn a_damaged_background_library_is_copied_to_recovery_and_replaced_by_an_empty_one() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    std::fs::write(paths.bg_presets_file(), "{ not json").unwrap();
    let f = presets::open_backgrounds(&paths).unwrap();
    assert!(f.value.presets.is_empty());
    assert_eq!(
        std::fs::read_to_string(paths.bg_presets_recovery_file()).unwrap(),
        "{ not json"
    );
}

#[test]
fn background_library_keeps_added_presets() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let mut f = presets::open_backgrounds(&paths).unwrap();
    let id = f.value.add("Sunset", Default::default());
    f.save().unwrap();
    let again = presets::open_backgrounds(&paths).unwrap();
    assert_eq!(again.value.get(id).map(|p| p.name.as_str()), Some("Sunset"));
}

fn harbour() -> es_model::BackgroundGradient {
    let mut g = es_model::BackgroundGradient {
        title: "Harbour".into(),
        colors: vec![
            es_model::BackgroundColor::custom(0.0, 0.6, 0.8),
            es_model::BackgroundColor::custom(0.9, 0.6, 0.1),
            es_model::BackgroundColor::custom(0.4, 0.1, 0.6),
        ],
        ..Default::default()
    };
    g.set_angle_degrees(90.0);
    g
}

#[test]
fn user_gradients_round_trip_in_their_own_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let mut f = presets::open_gradients(&paths).unwrap();
    let id = f.value.save(harbour());
    f.save().unwrap();
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(paths.gradients_file()).unwrap()).unwrap();
    assert_eq!(
        (&written["format"], &written["version"]),
        (&"easyshot.gradients".into(), &1.into())
    );
    let again = presets::open_gradients(&paths).unwrap();
    assert_eq!(again.value.get(&id), Some(&f.value.gradients[0]));
    assert_eq!(again.value.gradients[0].colors.len(), 3);
}

#[test]
fn newer_gradients_are_read_only_and_a_damaged_file_is_kept_for_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    let body = newer("easyshot.gradients");
    std::fs::write(paths.gradients_file(), &body).unwrap();
    let f = presets::open_gradients(&paths).unwrap();
    assert!(matches!(f.save(), Err(Error::NewerVersion { version: 7 })));
    assert_eq!(
        std::fs::read_to_string(paths.gradients_file()).unwrap(),
        body
    );

    std::fs::write(paths.gradients_file(), "{ not json").unwrap();
    let f = presets::open_gradients(&paths).unwrap();
    assert!(f.value.gradients.is_empty());
    assert_eq!(
        std::fs::read_to_string(paths.gradients_recovery_file()).unwrap(),
        "{ not json"
    );
}

#[test]
fn a_file_of_another_format_is_rejected_not_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    std::fs::write(
        paths.state_file(),
        r#"{"format":"easyshot.history","version":1}"#,
    )
    .unwrap();
    assert!(matches!(
        state::open(&paths),
        Err(Error::WrongFormat { .. })
    ));
}

#[test]
fn a_syntax_error_reports_line_and_column() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_with_dirs(dir.path());
    std::fs::write(paths.state_file(), "{\n  \"format\": ,\n}").unwrap();
    assert!(matches!(
        state::open(&paths),
        Err(Error::Parse { line: 2, .. })
    ));
}

#[test]
fn sidecar_sits_next_to_the_image_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("shot.png");
    assert_eq!(
        document::sidecar_path(&image),
        dir.path().join("shot.png.easyshot")
    );
    assert!(document::open(&image).is_none());
    let doc = AnnotationDocument {
        base_image_file_name: "shot.base.png".into(),
        ..Default::default()
    };
    document::create(&image, doc).save().unwrap();
    let back = document::open(&image).unwrap();
    assert_eq!(back.value.base_image_file_name, "shot.base.png");
}

#[test]
fn newer_sidecar_opens_read_only_and_unreadable_sidecar_is_no_document() {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("shot.png");
    let side = document::sidecar_path(&image);
    std::fs::write(&side, newer("easyshot.document")).unwrap();
    let f = document::open(&image).unwrap();
    assert!(matches!(f.save(), Err(Error::NewerVersion { version: 7 })));
    std::fs::write(&side, "garbage").unwrap();
    assert!(document::open(&image).is_none());
    assert_eq!(std::fs::read_to_string(&side).unwrap(), "garbage");
}
