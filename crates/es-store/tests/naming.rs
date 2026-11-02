#![allow(clippy::unwrap_used)]
use es_model::State;
use es_store::ExportFormat;
use es_store::export::jpeg_quality_percent;
use es_store::naming::*;
use std::path::Path;
use time::OffsetDateTime;

fn ctx() -> Context {
    Context {
        date: OffsetDateTime::from_unix_timestamp(1_758_115_802).unwrap(),
        kind: Kind::Screenshot,
        counter: 7,
    }
}

#[test]
fn default_template_renders_the_documented_name() {
    let name = file_name(DEFAULT_TEMPLATE, "png", &ctx());
    assert_eq!(name, "easyshot_2025-09-17-13-30-02.png");
}

#[test]
fn static_text_and_random_tokens() {
    let name = file_name("hello-{hex:8}", "png", &ctx());
    let random = name
        .strip_prefix("hello-")
        .unwrap()
        .strip_suffix(".png")
        .unwrap();
    assert_eq!(random.len(), 8);
    assert!(
        random
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    );
    let b62 = render("{base62:6}", &ctx(), "");
    assert!(b62.len() == 6 && b62.chars().all(|c| c.is_ascii_alphanumeric()));
    let digits = render("{digits:4}", &ctx(), "");
    assert!(digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()));
    assert_eq!(render("{hex:0}", &ctx(), "").len(), 1, "clamps up");
    assert_eq!(render("{hex:999}", &ctx(), "").len(), 32, "clamps down");
    assert_eq!(
        render("{hex:abc}", &ctx(), "").len(),
        8,
        "non-numeric falls back"
    );
    assert_eq!(render("{date:yyyy}", &ctx(), ""), "2025");
    let uuid = render("{uuid}", &ctx(), "");
    assert!(uuid.len() == 36 && uuid == uuid.to_lowercase());
    assert_ne!(
        render("{hex:32}", &ctx(), ""),
        render("{hex:32}", &ctx(), "")
    );
}

#[test]
fn date_patterns() {
    assert_eq!(render("{date:dd-MM-yyyy}", &ctx(), ""), "17-09-2025");
    assert_eq!(render("{date:yyyy.MM.dd}", &ctx(), ""), "2025.09.17");
    assert_eq!(render("{time:HHmmss}", &ctx(), ""), "133002");
    assert_eq!(render("{date:EEE MMM d}", &ctx(), ""), "Wed Sep 17");
    assert_eq!(render("{time:h a}", &ctx(), ""), "1 PM");
    assert_eq!(render("{date:'week' yy}", &ctx(), ""), "week 25");
}

#[test]
fn extension_is_added_once() {
    assert_eq!(file_name("shot.{ext}", "png", &ctx()), "shot.png");
    assert_eq!(render("shot.{ext}", &ctx(), "png"), "shot.png");
    assert_eq!(file_name("shot", "png", &ctx()), "shot.png");
    assert_eq!(file_name("clip", "mp4", &ctx()), "clip.mp4");
    assert_eq!(file_name("shot.png", "png", &ctx()), "shot.png");
    assert_eq!(file_name("shot.PNG", "png", &ctx()), "shot.PNG");
}

#[test]
fn unknown_and_unclosed_braces_survive() {
    assert_eq!(file_name("a{nope}b", "png", &ctx()), "a{nope}b.png");
    assert_eq!(file_name("a{date", "png", &ctx()), "a{date.png");
    assert_eq!(render("{x{kind}", &ctx(), ""), "{xScreenshot");
    assert_eq!(token_names("a{nope}{kind}"), vec!["kind"]);
}

#[test]
fn nothing_typed_can_reach_the_file_system_illegally() {
    assert_eq!(file_name("a/b:c", "png", &ctx()), "abc.png");
    assert_eq!(file_name("...secret", "png", &ctx()), "secret.png");
    assert_eq!(file_name("  spaced  ", "png", &ctx()), "spaced.png");
    assert_eq!(file_name("a\nb\tc", "png", &ctx()), "abc.png");
    assert_eq!(
        file_name(&"x".repeat(500), "png", &ctx()).chars().count(),
        204
    );
}

#[test]
fn empty_or_all_punctuation_template_falls_back_to_the_default() {
    for t in ["", "   ", "///", "..."] {
        let name = file_name(t, "png", &ctx());
        assert!(name.starts_with("easyshot_"), "{t:?} -> {name}");
        assert_ne!(name, ".png");
    }
}

#[test]
fn counter_rendering_and_detection() {
    assert_eq!(render("{counter}", &ctx(), ""), "7");
    assert_eq!(render("{counter:3}", &ctx(), ""), "007");
    assert_eq!(render("{counter:1}", &ctx(), ""), "7");
    assert!(uses_counter("shot-{counter}"));
    assert!(!uses_counter("shot-{hex:8}"));
    assert!(!uses_counter("shot-{uuid}"));
    assert!(!uses_counter(DEFAULT_TEMPLATE));
}

#[test]
fn kind_token() {
    let mut c = ctx();
    assert_eq!(render("{kind}", &c, ""), "Screenshot");
    c.kind = Kind::Recording;
    assert_eq!(render("{kind}", &c, ""), "Recording");
}

#[test]
fn naming_a_capture_spends_one_number_and_only_when_the_template_uses_it() {
    let date = ctx().date;
    let mut state = State::default();
    assert_eq!(state.counter, 1);
    let first = next_file_name(
        "shot-{counter:3}",
        "png",
        Kind::Screenshot,
        date,
        &mut state,
    );
    assert_eq!(first, "shot-001.png");
    assert_eq!(state.counter, 2);
    let second = next_file_name(
        "shot-{counter:3}",
        "png",
        Kind::Screenshot,
        date,
        &mut state,
    );
    assert_eq!(second, "shot-002.png");
    assert_eq!(state.counter, 3);

    let preview = file_name(
        "shot-{counter:3}",
        "png",
        &Context {
            counter: state.counter,
            ..ctx()
        },
    );
    assert_eq!(preview, "shot-003.png");
    assert_eq!(state.counter, 3, "previewing spends nothing");

    next_file_name("shot-{hex:8}", "png", Kind::Screenshot, date, &mut state);
    assert_eq!(state.counter, 3);

    state.counter = 0;
    assert_eq!(
        next_file_name("{counter}", "png", Kind::Screenshot, date, &mut state),
        "1.png"
    );
}

#[test]
fn a_second_capture_with_the_same_name_never_overwrites_the_first() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.png"), b"").unwrap();
    let take = |name: &str, sep: &str| {
        unique_path(name, dir.path(), sep, exists)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(take("b.png", " "), "b.png");
    assert_eq!(take("a.png", " "), "a 1.png");
    assert_eq!(take("a.png", "-"), "a-1.png");

    std::fs::write(dir.path().join("c.preview.png"), b"").unwrap();
    let companion = unique_path("c.png", dir.path(), "-", |p: &Path| {
        exists(p) || exists(&p.with_extension("preview.png"))
    });
    assert_eq!(companion.file_name().unwrap(), "c-1.png");
}

#[test]
fn collisions_skip_every_taken_number_and_finally_fall_back_to_a_uuid() {
    let dir = Path::new("/nowhere");
    let p = unique_path("a.png", dir, " ", |p| {
        let n = p.file_name().unwrap().to_string_lossy();
        n == "a.png" || n == "a 1.png" || n == "a 2.png"
    });
    assert_eq!(p.file_name().unwrap(), "a 3.png");
    let all = unique_path("a.png", dir, " ", |_| true);
    let n = all.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        n.starts_with("a ") && n.ends_with(".png") && n.len() > 20,
        "{n}"
    );
}

#[test]
fn every_offered_export_format_has_an_extension_that_maps_back() {
    for f in ExportFormat::ALL {
        assert_eq!(ExportFormat::from_extension(f.extension()), Some(f));
        assert_eq!(ExportFormat::from_setting(f.setting_name()), f);
        assert!(f.mime().starts_with("image/"));
    }
    assert_eq!(ExportFormat::Jpeg.extension(), "jpg");
    assert_eq!(
        ExportFormat::from_extension("JPEG"),
        Some(ExportFormat::Jpeg)
    );
}

#[test]
fn a_dropped_or_unknown_stored_format_falls_back_to_png() {
    assert_eq!(ExportFormat::from_setting("webp"), ExportFormat::Png);
    assert_eq!(ExportFormat::from_setting(""), ExportFormat::Png);
    assert_eq!(ExportFormat::from_setting("jpeg"), ExportFormat::Jpeg);
    assert_eq!(ExportFormat::from_extension("heic"), None);
    assert_eq!(jpeg_quality_percent(0.9), 90);
    assert_eq!(jpeg_quality_percent(7.0), 100);
    assert_eq!(jpeg_quality_percent(f64::NAN), 90);
}
