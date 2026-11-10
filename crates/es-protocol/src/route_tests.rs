use super::*;

fn no_args() -> RouteArgs {
    RouteArgs::default()
}

fn path_args(p: &Path) -> RouteArgs {
    let mut a = RouteArgs::default();
    a.0.insert(PATH_ARG.into(), ArgValue::Path(p.to_owned()));
    a
}

fn variant_index(route: &Route) -> usize {
    match route {
        Route::CaptureRegion => 0,
        Route::CaptureFullscreen => 1,
        Route::CaptureWindow => 2,
        Route::CapturePrevious => 3,
        Route::CaptureTimed => 4,
        Route::Timed { .. } => 5,
        Route::CaptureRegionCopy => 6,
        Route::CaptureRegionSave => 7,
        Route::CaptureRegionAnnotate => 8,
        Route::CaptureRegionPin => 9,
        Route::CaptureScroll => 10,
        Route::CaptureScrollWindow => 35,
        Route::Ocr => 11,
        Route::OcrSingleLine => 12,
        Route::ColorPicker => 13,
        Route::Record => 14,
        Route::RecordArea => 15,
        Route::RecordStop => 16,
        Route::RecordPause => 17,
        Route::RecordRestart => 18,
        Route::RecordDiscard => 19,
        Route::CaptureBar => 20,
        Route::Home => 21,
        Route::Gallery => 22,
        Route::RestoreLast => 23,
        Route::PinLast => 24,
        Route::UnpinAll => 25,
        Route::Open(_) => 26,
        Route::EditClipboard => 27,
        Route::Settings(_) => 28,
        Route::DeckToggle => 29,
        Route::DeckSaveAll => 30,
        Route::DeckCloseAll => 31,
        Route::Quit => 32,
        Route::AgentRevoke => 33,
        Route::Agent(_) => 34,
    }
}

#[test]
fn all_lists_every_variant() {
    let seen: std::collections::BTreeSet<usize> = Route::all().iter().map(variant_index).collect();
    assert_eq!(
        seen,
        (0..=35).collect(),
        "a variant is missing from Route::all"
    );
}

#[test]
fn every_plain_route_round_trips_through_its_name() {
    for (name, route) in PLAIN {
        assert_eq!(route.as_str(), *name);
        assert_eq!(route.to_string(), *name);
        assert_eq!(&Route::parse(name, &no_args()).unwrap(), route);
    }
    assert_eq!(PLAIN.len(), 32);
}

#[test]
fn plain_scroll_is_the_region_start_and_the_window_start_is_its_alternate() {
    assert_eq!(
        Route::parse("capture/scroll", &no_args()).unwrap(),
        Route::CaptureScroll
    );
    assert_eq!(
        Route::parse("capture/scroll-window", &no_args()).unwrap(),
        Route::CaptureScrollWindow
    );
    assert!(Route::parse("capture/scroll-region", &no_args()).is_err());
    assert_eq!(Route::CaptureScroll.command_args(), ["capture", "scroll"]);
    assert_eq!(
        Route::CaptureScrollWindow.command_args(),
        ["capture", "scroll", "--window"]
    );
}

#[test]
fn settings_pages_round_trip() {
    let bare = Route::parse("settings", &no_args()).unwrap();
    assert_eq!(bare, Route::Settings(None));
    for page in SettingsPage::ALL {
        let r = Route::Settings(Some(page));
        assert_eq!(Route::parse(&r.as_str(), &no_args()).unwrap(), r);
    }
    assert!(Route::parse("settings/nope", &no_args()).is_err());
    assert!(Route::parse("settings/", &no_args()).is_err());
    for old in [
        "after-capture",
        "default-look",
        "compositor",
        "editor",
        "ocr",
    ] {
        assert!(
            Route::parse(&format!("settings/{old}"), &no_args()).is_err(),
            "{old}"
        );
    }
}

#[test]
fn unknown_routes_are_rejected() {
    for r in [
        "",
        "Capture/Region",
        "capture",
        "capture/region/",
        "/ocr",
        "exec",
        "quit ",
    ] {
        assert!(
            matches!(Route::parse(r, &no_args()), Err(Error::UnknownRoute(_))),
            "{r:?}"
        );
    }
}

#[test]
fn routes_other_than_open_take_no_arguments() {
    let mut args = RouteArgs::default();
    args.0
        .insert("output".into(), ArgValue::Str("/tmp/x.png".into()));
    for route in [
        "capture/region",
        "record",
        "settings",
        "settings/general",
        "quit",
    ] {
        assert!(
            matches!(Route::parse(route, &args), Err(Error::InvalidArgs { .. })),
            "{route}"
        );
    }
}

#[test]
fn open_requires_an_existing_image_and_canonicalises() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.PNG");
    std::fs::write(&png, [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
    let jpg = dir.path().join("b.jpg");
    std::fs::write(&jpg, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
    let fake = dir.path().join("c.png");
    std::fs::write(&fake, b"hello").unwrap();
    let text = dir.path().join("d.txt");
    std::fs::write(&text, [0x89, b'P', b'N', b'G']).unwrap();
    let mismatch = dir.path().join("e.png");
    std::fs::write(&mismatch, [0xFF, 0xD8, 0xFF]).unwrap();

    let roundabout = dir.path().join(".").join("a.PNG");
    let r = Route::parse("open", &path_args(&roundabout)).unwrap();
    assert_eq!(r, Route::Open(png.canonicalize().unwrap()));
    assert!(Route::parse("open", &path_args(&jpg)).is_ok());
    for bad in [&fake, &text, &dir.path().join("missing.png"), dir.path()] {
        assert!(
            matches!(
                Route::parse("open", &path_args(bad)),
                Err(Error::InvalidArgs { .. })
            ),
            "{bad:?}"
        );
    }
    assert!(Route::parse("open", &path_args(&mismatch)).is_ok());
}

#[test]
fn open_accepts_videos_by_extension_and_container() {
    let dir = tempfile::tempdir().unwrap();
    let heads: [(&str, &[u8]); 5] = [
        ("a.mp4", b"\0\0\0\x18ftypmp42"),
        ("b.MOV", b"\0\0\0\x14ftypqt  "),
        ("c.webm", &[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
        ("d.mkv", &[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
        ("e.gif", b"GIF89a"),
    ];
    for (name, head) in heads {
        let file = dir.path().join(name);
        std::fs::write(&file, head).unwrap();
        let r = Route::parse("open", &path_args(&file)).unwrap();
        assert_eq!(r, Route::Open(file.canonicalize().unwrap()), "{name}");
    }
    let fake = dir.path().join("f.mp4");
    std::fs::write(&fake, b"not a video at all").unwrap();
    assert!(Route::parse("open", &path_args(&fake)).is_err());
}

#[test]
fn open_refuses_a_fifo_without_blocking() {
    use rustix::fs::{CWD, FileType, Mode, mknodat};
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("trap.png");
    mknodat(CWD, &fifo, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let args = path_args(&fifo);
    std::thread::spawn(move || {
        let _ = tx.send(Route::parse("open", &args));
    });
    let parsed = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("parsing a FIFO must not block");
    assert!(matches!(parsed, Err(Error::InvalidArgs { .. })));
}

#[test]
fn a_link_never_records_without_a_selection() {
    for (_, route) in URL_ROUTES {
        assert_ne!(route, &Route::Record);
    }
}

#[test]
fn open_rejects_missing_wrong_or_extra_arguments() {
    assert!(Route::parse("open", &no_args()).is_err());
    let mut args = RouteArgs::default();
    args.0.insert(PATH_ARG.into(), ArgValue::Bool(true));
    assert!(Route::parse("open", &args).is_err());
    let mut args = RouteArgs::default();
    args.0.insert("other".into(), ArgValue::Str("x".into()));
    assert!(Route::parse("open", &args).is_err());
}

#[test]
fn open_round_trips_through_name_and_args() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    std::fs::write(&png, [0x89, b'P', b'N', b'G']).unwrap();
    let r = Route::parse("open", &path_args(&png)).unwrap();
    assert_eq!(r.as_str(), "open");
    assert_eq!(Route::parse(&r.as_str(), &r.args()).unwrap(), r);
}

#[test]
fn url_routes_lists_every_allowed_url() {
    let urls: Vec<_> = Route::url_routes().collect();
    assert_eq!(urls.len(), URL_ROUTES.len());
    for url in urls {
        assert!(Route::from_url(&url).is_ok(), "{url}");
    }
}

#[test]
fn url_accepts_exactly_the_nine_routes() {
    for (name, route) in URL_ROUTES {
        assert_eq!(
            &Route::from_url(&format!("easyshot://{name}")).unwrap(),
            route
        );
    }
    assert_eq!(URL_ROUTES.len(), 9);
    assert_eq!(
        Route::from_url("easyshot://settings").unwrap(),
        Route::Settings(None)
    );
    assert_eq!(Route::from_url("easyshot://home").unwrap(), Route::Home);
}

#[test]
fn url_scheme_and_host_ignore_case_but_the_path_does_not() {
    assert_eq!(
        Route::from_url("EASYSHOT://CAPTURE/region").unwrap(),
        Route::CaptureRegion
    );
    assert_eq!(
        Route::from_url("EasyShot://RECORD").unwrap(),
        Route::RecordArea
    );
    assert!(Route::from_url("easyshot://capture/REGION").is_err());
}

#[test]
fn url_rejects_everything_outside_the_allowlist() {
    for url in [
        "https://capture/region",
        "easyshot:record",
        "easyshot:///record",
        "easyshot://",
        "easyshot://unknown",
        "easyshot://capture",
        "easyshot://capture/region/extra",
        "easyshot://capture/region/",
        "easyshot://record/",
        "easyshot://capture/region?output=/tmp/test.png",
        "easyshot://record?",
        "easyshot://record#start",
        "easyshot://record#",
        "easyshot://user@record",
        "easyshot://user:password@record",
        "easyshot://record:123",
        "easyshot://capture/../region",
        "easyshot://settings/anything",
        "easyshot://settings/general",
        "easyshot://home/",
        "easyshot://home/gallery",
        "easyshot://home?tab=settings",
        "easyshot://gallery",
        "easyshot://capture/previous",
        "easyshot://quit",
        "easyshot://open",
        " easyshot://record",
        "easyshot://record ",
        "",
    ] {
        assert!(
            matches!(Route::from_url(url), Err(Error::UrlNotAllowed(_))),
            "{url:?}"
        );
    }
}
