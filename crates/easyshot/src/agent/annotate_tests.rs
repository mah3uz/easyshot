use super::*;
use crate::cli::{Cli, Request};
use clap::Parser;
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

const OPS: &str = r##"{"version":1,"ops":[
  {"op":"rect","x":20,"y":20,"w":150,"h":80,"color":"#2f80ed","width":4},
  {"op":"ellipse","x":190,"y":20,"w":120,"h":80,"color":"#27ae6080","fill":true},
  {"op":"arrow","from":[60,255],"to":[200,160],"color":"#eb5757","width":5},
  {"op":"text","x":215,"y":135,"text":"Click here","size":26,"color":"#ffffff","background":"#000000c0"},
  {"op":"counter","x":60,"y":255},
  {"op":"counter","x":420,"y":60},
  {"op":"counter","x":420,"y":120,"n":7,"color":"#9b51e0"},
  {"op":"highlight","x":20,"y":124,"w":160,"h":18},
  {"op":"blur","x":330,"y":200,"w":120,"h":36},
  {"op":"pixelate","x":330,"y":250,"w":120,"h":30,"amount":0.6}
]}"##;

const BACKGROUND: &str = r##"{"version":1,"ops":[
  {"op":"arrow","from":[40,40],"to":[200,120],"color":"#f2994a","width":6},
  {"op":"counter","x":40,"y":40}
],"background":{"gradient":["#4568dc","#b06ab3"],"padding":40,"corners":16,"shadow":0.5}}"##;

fn fixture() -> RgbaImage {
    let mut img = RgbaImage::from_fn(480, 300, |_, y| {
        let v = 236 - (y / 4) as u8;
        Rgba([v, v.saturating_add(6), v.saturating_add(14), 255])
    });
    for (top, right) in [(128, 150), (135, 120)] {
        for x in 24..right {
            for y in top..top + 3 {
                img.put_pixel(x, y, Rgba([40, 40, 48, 255]));
            }
        }
    }
    for line in 0..4u32 {
        for x in 334..446 {
            for y in 204 + line * 8..208 + line * 8 {
                img.put_pixel(x, y, Rgba([30, 30, 36, 255]));
            }
        }
    }
    for line in 0..3u32 {
        for x in 334..446 {
            for y in 254 + line * 9..259 + line * 9 {
                let on = (x / 7 + line) % 2 == 0;
                img.put_pixel(
                    x,
                    y,
                    if on {
                        Rgba([200, 40, 40, 255])
                    } else {
                        Rgba([30, 30, 36, 255])
                    },
                );
            }
        }
    }
    img
}

struct Rig {
    dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        fixture().save(dir.path().join("in.png")).unwrap();
        Rig { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn run(&self, ops: &str, out: &str, force: bool) -> Result<Done, Failure> {
        perform(
            &self.path("in.png"),
            ops,
            &self.path(out),
            force,
            &StylePreset::default(),
        )
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn assert_golden(name: &str, actual: &Path) {
    let produced = image::open(actual).unwrap().to_rgba8();
    let golden = golden_dir().join(name);
    if std::env::var_os("EASYSHOT_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        produced.save(&golden).unwrap();
        return;
    }
    let expected = image::open(&golden)
        .unwrap_or_else(|e| panic!("{}: {e}", golden.display()))
        .to_rgba8();
    assert_eq!(produced.dimensions(), expected.dimensions(), "{name}: size");
    let off = produced
        .pixels()
        .zip(expected.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(x, y)| x.abs_diff(y) > 6))
        .count();
    let allowed = produced.pixels().len() / 500;
    assert!(
        off <= allowed,
        "{name}: {off} pixels differ by more than 6 (allowed {allowed})"
    );
}

#[test]
fn every_op_renders_to_the_picture_checked_by_eye() {
    let rig = Rig::new();
    let done = rig.run(OPS, "all.png", false).unwrap();
    assert_eq!((done.width, done.height, done.ops), (480, 300, 10));
    assert_golden("annotate-all-ops.png", &rig.path("all.png"));
}

#[test]
fn a_background_grows_the_canvas_and_matches_its_picture() {
    let rig = Rig::new();
    let done = rig.run(BACKGROUND, "bg.png", false).unwrap();
    assert_eq!((done.width, done.height), (560, 380));
    assert_golden("annotate-background.png", &rig.path("bg.png"));
}

#[test]
fn an_op_changes_only_the_pixels_it_names() {
    let rig = Rig::new();
    let ops = r##"{"version":1,"ops":[{"op":"rect","x":100,"y":100,"w":50,"h":50,"color":"#ff0000","width":4}]}"##;
    rig.run(ops, "one.png", false).unwrap();
    let before = fixture();
    let after = image::open(rig.path("one.png")).unwrap().to_rgba8();
    assert_eq!(before.get_pixel(10, 10), after.get_pixel(10, 10));
    assert_eq!(before.get_pixel(125, 125), after.get_pixel(125, 125));
    let edge = after.get_pixel(100, 125);
    assert!(edge[0] > 200 && edge[1] < 80, "{edge:?}");
}

fn refused(ops: &str) -> String {
    let rig = Rig::new();
    let Err(f) = rig.run(ops, "out.png", false) else {
        panic!("accepted: {ops}");
    };
    assert_eq!(f.code, ErrorCode::InvalidArguments, "{}", f.message);
    assert!(!rig.path("out.png").exists(), "a refusal writes nothing");
    f.message
}

#[test]
fn a_bad_op_is_refused_with_its_index_and_field() {
    let first = r##"{"op":"rect","x":1,"y":1,"w":5,"h":5}"##;
    for (op, field) in [
        (r#"{"op":"circle","x":1}"#, "circle"),
        (
            r##"{"op":"arrow","from":[0,0],"to":[5,5],"colour":"#fff"}"##,
            "colour",
        ),
        (
            r##"{"op":"rect","x":1,"y":1,"w":5,"h":5,"color":"red"}"##,
            "color",
        ),
        (r#"{"op":"rect","x":1,"y":1,"h":5}"#, "w"),
        (r#"{"op":"rect","x":1,"y":1,"w":0,"h":5}"#, "w"),
        (r#"{"op":"arrow","from":[1,1],"to":[1,1]}"#, "from and to"),
        (r#"{"op":"text","x":1,"y":1,"text":""}"#, "text"),
        (r#"{"op":"counter","x":1,"y":1,"n":0}"#, "n"),
        (
            r#"{"op":"blur","x":1,"y":1,"w":5,"h":5,"amount":2}"#,
            "amount",
        ),
        (r#"{"op":"arrow","from":[1],"to":[5,5]}"#, "from"),
        (r#"{"x":1}"#, "op"),
        (
            r#"{"op":"arrow","from":[0,0],"to":[5,5],"head":"fancy"}"#,
            "head",
        ),
    ] {
        let message = refused(&format!(r#"{{"version":1,"ops":[{first},{op}]}}"#));
        assert!(message.contains("ops[1]"), "{op}: {message}");
        assert!(message.contains(field), "{op}: {message}");
    }
}

#[test]
fn the_file_itself_is_checked_before_any_op() {
    assert!(refused(r#"{"version":2,"ops":[]}"#).contains("version"));
    assert!(refused(r#"{"ops":[]}"#).contains("version"));
    assert!(refused(r#"{"version":1,"ops":[],"extra":1}"#).contains("extra"));
    assert!(refused(r#"not json"#).contains("ops file"));
    assert!(
        refused(
            r##"{"version":1,"ops":[],"background":{"color":"#000","gradient":["#000","#fff"]}}"##
        )
        .contains("background")
    );
    assert!(
        refused(r##"{"version":1,"ops":[],"background":{"color":"#000000","padding":-4}}"##)
            .contains("padding")
    );
}

#[test]
fn an_existing_file_is_kept_unless_forced() {
    let rig = Rig::new();
    std::fs::write(rig.path("out.png"), b"mine").unwrap();
    let ops = r#"{"version":1,"ops":[]}"#;
    let Err(f) = rig.run(ops, "out.png", false) else {
        panic!("replaced a file without --force");
    };
    assert_eq!(f.code, ErrorCode::InvalidArguments);
    assert!(f.message.contains("--force"), "{}", f.message);
    assert_eq!(std::fs::read(rig.path("out.png")).unwrap(), b"mine");
    rig.run(ops, "out.png", true).unwrap();
    assert!(image::open(rig.path("out.png")).is_ok());
}

#[test]
fn the_output_must_be_a_png_in_a_folder_that_exists() {
    let rig = Rig::new();
    let ops = r#"{"version":1,"ops":[]}"#;
    for out in ["out.jpg", "missing/out.png"] {
        let Err(f) = rig.run(ops, out, false) else {
            panic!("{out} accepted");
        };
        assert_eq!(f.code, ErrorCode::InvalidArguments, "{out}: {}", f.message);
    }
}

#[test]
fn a_missing_image_is_not_found_and_a_text_file_is_not_an_image() {
    let rig = Rig::new();
    let ops = r#"{"version":1,"ops":[]}"#;
    let out = rig.path("out.png");
    let style = StylePreset::default();
    let Err(f) = perform(&rig.path("nope.png"), ops, &out, false, &style) else {
        panic!("a missing image was accepted");
    };
    assert_eq!(f.code, ErrorCode::NotFound);
    let gone = rig.path("nope.png");
    assert_eq!(
        f.hint,
        Some(format!(
            "There is no file at {}. Check the path and try again.",
            gone.display()
        ))
    );
    std::fs::write(rig.path("notes.png"), "hello").unwrap();
    let Err(f) = perform(&rig.path("notes.png"), ops, &out, false, &style) else {
        panic!("a text file was accepted");
    };
    assert_eq!(f.code, ErrorCode::InvalidArguments);
    assert!(
        f.hint
            .unwrap()
            .ends_with("isn't an image easyshot can read. Use a PNG or JPEG file."),
    );
}

#[test]
fn counters_number_themselves_after_the_highest_one() {
    let ops = r#"{"version":1,"ops":[
        {"op":"counter","x":50,"y":50},{"op":"counter","x":100,"y":50,"n":5},
        {"op":"counter","x":150,"y":50}]}"#;
    let (doc, count) = annotate_ops::build(ops, (480, 300), &StylePreset::default()).unwrap();
    assert_eq!(count, 3);
    let values: Vec<u32> = doc
        .shapes
        .iter()
        .filter_map(|s| match &s.kind {
            es_model::ShapeKind::Numbered(p) => Some(p.value),
            _ => None,
        })
        .collect();
    assert_eq!(values, [1, 5, 6]);
}

#[test]
fn defaults_come_from_the_saved_style() {
    let style = StylePreset {
        swatch: es_model::Swatch::custom(0, 0, 255, 255),
        stroke_width: 9.0,
        ..StylePreset::default()
    };
    let ops = r#"{"version":1,"ops":[{"op":"rect","x":1,"y":1,"w":50,"h":50}]}"#;
    let (doc, _) = annotate_ops::build(ops, (900, 600), &style).unwrap();
    let es_model::ShapeKind::Geo(p) = &doc.shapes[0].kind else {
        panic!("not a rectangle");
    };
    assert_eq!(p.swatch.id, "custom-0-0-255-255");
    assert_eq!(p.stroke_width, 9.0);
}

#[test]
fn the_command_line_builds_an_annotate_call_that_needs_no_daemon() {
    let parse = |args: &[&str]| Cli::try_parse_from(args).unwrap().request().unwrap();
    let want = AnnotateCall {
        image: "a.png".into(),
        ops: "ops.json".into(),
        out: "b.png".into(),
        force: true,
        json: true,
    };
    for head in ["agent", "ai"] {
        let got = parse(&[
            "easyshot", head, "annotate", "a.png", "--ops", "ops.json", "--out", "b.png",
            "--force", "--json",
        ]);
        assert_eq!(got, Request::Annotate(want.clone()));
    }
    assert!(Cli::try_parse_from(["easyshot", "agent", "annotate", "a.png"]).is_err());
}

#[test]
fn json_and_text_answers_and_exit_codes() {
    let rig = Rig::new();
    let ops_file = rig.path("ops.json");
    std::fs::write(&ops_file, OPS).unwrap();
    let call = |out: &str, json| AnnotateCall {
        image: rig.path("in.png"),
        ops: ops_file.clone(),
        out: rig.path(out),
        force: false,
        json,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_annotate(&call("a.png", true), &mut out, &mut err), 0);
    let body: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        body,
        json!({"ok": true, "kind": "annotate", "path": rig.path("a.png"),
               "width": 480, "height": 300, "ops": 10})
    );
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_annotate(&call("a.png", true), &mut out, &mut err), 4);
    let body: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(body["kind"], "annotate");
    assert_eq!(body["error"]["code"], "invalid_arguments");
    assert!(err.is_empty());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_annotate(&call("b.png", false), &mut out, &mut err), 0);
    assert_eq!(
        String::from_utf8(out).unwrap().trim(),
        rig.path("b.png").to_string_lossy()
    );
    let missing = AnnotateCall {
        ops: rig.path("none.json"),
        ..call("c.png", true)
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run_annotate(&missing, &mut out, &mut err), 5);
}
