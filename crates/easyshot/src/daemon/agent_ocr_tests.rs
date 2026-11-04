use super::*;
use crate::daemon::agent::gate::tests::{GateRig, ME};
use es_app::events::Origin;
use es_app::jobs::{JobId, Jobs};
use es_backend::Caps;
use es_protocol::agent::OcrInput;
use es_protocol::{AgentRoute, JobStatus};
use gpui::TestAppContext;
use image::Rgba;
use rxing::{BarcodeFormat, Writer, qrcode::QRCodeWriter};
use std::cell::{Cell, RefCell};
use std::sync::Mutex;

fn qr(payload: &str, side: i32) -> RgbaImage {
    let m = QRCodeWriter
        .encode(payload, &BarcodeFormat::QR_CODE, side, side)
        .unwrap();
    RgbaImage::from_fn(m.getWidth(), m.getHeight(), |x, y| {
        let v = if m.get(x, y) { 0 } else { 255 };
        Rgba([v, v, v, 255])
    })
}

fn without_tesseract() -> Recognizer {
    Arc::new(|image, options| {
        let options = OcrOptions {
            tesseract_path: Some("/nonexistent/tesseract".into()),
            ..options.clone()
        };
        es_ocr::recognize(image, &options)
    })
}

#[test]
fn a_qr_code_is_read_and_placed_without_tesseract() {
    let mut canvas = RgbaImage::from_pixel(400, 300, Rgba([255; 4]));
    image::imageops::overlay(&mut canvas, &qr("https://x.test/a", 200), 120, 60);
    let info = read(&canvas, &["eng".into()], &without_tesseract()).unwrap();
    assert_eq!(info.text, "https://x.test/a");
    assert!(info.lines.is_empty());
    let [code] = info.qr.as_slice() else {
        panic!("{:?}", info.qr)
    };
    assert_eq!(code.text, "https://x.test/a");
    assert!(
        (120..=200).contains(&code.x) && (60..=140).contains(&code.y),
        "{code:?}"
    );
    assert!(
        code.width > 80 && code.x + code.width as i32 <= 330,
        "{code:?}"
    );
}

#[test]
fn text_without_tesseract_and_without_a_code_is_unsupported_with_the_install_hint() {
    let blank = RgbaImage::from_pixel(120, 60, Rgba([255; 4]));
    let Fail(code, message, hint) =
        read(&blank, &["eng".into()], &without_tesseract()).unwrap_err();
    assert_eq!(
        hint.as_deref(),
        Some(
            "Reading text needs Tesseract. Install it with \
             `sudo pacman -S tesseract tesseract-data-eng`."
        )
    );
    assert_eq!(code, ErrorCode::Unsupported);
    assert!(message.contains("install the tesseract"), "{message}");
}

#[test]
fn lines_become_whole_pixel_boxes_and_the_languages_reach_the_recognizer() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let fake: Recognizer = Arc::new(move |_, options| {
        log.lock().unwrap().extend(options.languages.clone());
        Ok(Recognized {
            text: "Hello there".into(),
            lines: vec![es_ocr::Line {
                text: "Hello there".into(),
                rect: Rect::new(10.4, 20.5, 70.6, 32.0),
                confidence: 89.875,
            }],
            ..Recognized::default()
        })
    });
    let image = RgbaImage::new(100, 50);
    let info = read(&image, &["deu".into(), "eng".into()], &fake).unwrap();
    assert_eq!(*seen.lock().unwrap(), ["deu", "eng"]);
    let [line] = info.lines.as_slice() else {
        panic!("{:?}", info.lines)
    };
    assert_eq!(
        (line.x, line.y, line.width, line.height, line.confidence),
        (10, 21, 60, 12, 89.88)
    );
    assert_eq!(info.text, "Hello there");
}

#[test]
fn a_file_must_exist_and_be_a_png_or_a_jpeg_whatever_its_name_says() {
    let dir = tempfile::tempdir().unwrap();
    let path = |name: &str| dir.path().join(name);
    let image = RgbaImage::from_pixel(8, 6, Rgba([10, 20, 30, 255]));
    image.save(path("a.png")).unwrap();
    image::DynamicImage::ImageRgba8(image.clone())
        .to_rgb8()
        .save(path("b.jpg"))
        .unwrap();
    assert_eq!(load_image(&path("a.png")).unwrap().dimensions(), (8, 6));
    assert_eq!(load_image(&path("b.jpg")).unwrap().dimensions(), (8, 6));
    std::fs::write(path("fake.png"), "not an image").unwrap();
    std::fs::create_dir(path("dir.png")).unwrap();
    let code = |p: &str| load_image(&path(p)).unwrap_err().0;
    assert_eq!(code("missing.png"), ErrorCode::NotFound);
    assert_eq!(code("fake.png"), ErrorCode::Failed);
    assert_eq!(code("dir.png"), ErrorCode::InvalidArguments);
}

#[gpui::test]
fn the_ocr_route_runs_its_runner_only_for_a_request_the_gate_lets_in(cx: &mut TestAppContext) {
    let runs = std::rc::Rc::new(Cell::new(0));
    let asked = std::rc::Rc::new(RefCell::new(None));
    let (counted, record) = (runs.clone(), asked.clone());
    let runner: OcrRunner = Rc::new(move |spec, _, cx| {
        counted.set(counted.get() + 1);
        *record.borrow_mut() = Some(spec);
        cx.spawn(async move |_| AgentReply {
            ocr: Some(Box::new(OcrInfo {
                text: "hi".into(),
                lines: Vec::new(),
                qr: Vec::new(),
                source: OcrSource::default(),
            })),
            ..AgentReply::ok(ReplyKind::Ocr)
        })
    });
    let ask = |origin: Origin, cx: &mut TestAppContext| {
        let (jobs, finished) = Jobs::new();
        let sources = super::super::Sources {
            caps: Caps::all(),
            outputs: Arc::new(Vec::new),
            windows: Arc::new(Vec::new),
        };
        let rig = GateRig::permissive(cx);
        let caller = es_desktop::dbus::Caller {
            uid: Some(ME),
            pid: None,
        };
        rig.gate.callers().borrow_mut().insert(JobId(5), caller);
        let gate = rig.gate.clone();
        let handler = super::super::handler_with(sources, jobs, gate, None, Some(runner.clone()));
        let spec = OcrSpec::new(OcrInput::File("/tmp/a.png".into()));
        cx.update(|cx| handler(JobId(5), AgentRoute::Ocr(spec), origin, cx));
        cx.run_until_parked();
        let done = finished.try_recv().expect("the job finishes");
        (
            done.status,
            serde_json::from_str::<serde_json::Value>(done.detail.as_deref().unwrap()).unwrap(),
        )
    };
    let (status, reply) = ask(Origin::DBus, cx);
    assert_eq!(status, JobStatus::Ok, "{reply}");
    assert_eq!(
        (reply["kind"].as_str(), reply["text"].as_str()),
        (Some("ocr"), Some("hi"))
    );
    assert_eq!(runs.get(), 1);
    assert!(asked.borrow().is_some());
    let (status, refused) = ask(Origin::Tray, cx);
    assert_eq!(status, JobStatus::Error);
    assert_eq!(refused["error"]["code"], "access_denied");
    assert_eq!(runs.get(), 1, "a refused request reads nothing");
}
