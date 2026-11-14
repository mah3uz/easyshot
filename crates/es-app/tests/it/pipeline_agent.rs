use super::*;

impl Rig {
    fn agent_save(
        &self,
        cx: &mut TestAppContext,
        options: es_app::agent_capture::Options,
    ) -> Result<es_app::agent_capture::Saved> {
        let task = cx.update(|cx| {
            es_app::agent_capture::save(
                &self.deps,
                options,
                es_model::CaptureKind::Window,
                frame(),
                cx,
            )
        });
        cx.run_until_parked();
        cx.foreground_executor().block_test(task)
    }

    fn history_len(&self, cx: &mut TestAppContext) -> usize {
        self.history.read_with(cx, |h, _| h.entries().len())
    }
}

#[gpui::test]
fn an_agent_copy_with_out_is_extra_because_the_user_reviews_captures_in_the_gallery(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let out = rig.save_dir.join("shot.png");
    std::fs::create_dir_all(&rig.save_dir).unwrap();
    let options = es_app::agent_capture::Options {
        out: Some(out.clone()),
        ui: false,
        ..Default::default()
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert_eq!(saved.path, out);
    assert_eq!((saved.width, saved.height), (4, 2));
    assert_eq!(image::open(&out).unwrap().to_rgba8().dimensions(), (4, 2));
    assert_eq!(rig.history_len(cx), 1, "the library entry is still made");
    assert!(
        rig.history
            .read_with(cx, |h, _| h.entries().iter().all(|e| e.file != out)),
        "the library file is not the agent's copy"
    );
    assert_eq!(rig.deck_len(cx), 0, "nothing is shown on screen");
    assert!(rig.toast_titles().is_empty());
    assert_eq!(files(&rig.save_dir), [out], "nothing else was saved");
}

#[gpui::test]
fn a_quiet_agent_capture_is_kept_in_the_library_even_when_cards_wait_to_be_saved(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.general.auto_save = true);
    let saved = rig
        .agent_save(cx, es_app::agent_capture::Options::default())
        .unwrap();
    assert_eq!(rig.history_len(cx), 1, "the user reviews it in the gallery");
    assert!(saved.path.exists());
    assert!(
        rig.history
            .read_with(cx, |h, _| h.by_file(&saved.path).is_some())
    );
    assert_eq!(rig.deck_len(cx), 0, "no card");
    assert!(rig.toast_titles().is_empty(), "no toast");
    assert_eq!(files(&rig.save_dir).len(), 1, "auto-save is honoured");
}

#[gpui::test]
fn an_agent_capture_with_the_interface_shows_its_card_and_can_also_write_a_file(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let out = rig.save_dir.join("shot.png");
    std::fs::create_dir_all(&rig.save_dir).unwrap();
    let options = es_app::agent_capture::Options {
        out: Some(out.clone()),
        ui: true,
        ..Default::default()
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert_eq!(saved.path, out);
    assert!(out.exists());
    assert_eq!(rig.deck_len(cx), 1);
}

fn black_copy() -> Arc<Frame> {
    let black = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 0, 0, 255]));
    Arc::new(es_app::agent_redact::frame_of(&frame(), &black))
}

fn is_color(path: &std::path::Path, rgba: [u8; 4]) -> bool {
    image::open(path)
        .unwrap()
        .to_rgba8()
        .pixels()
        .all(|p| p.0 == rgba)
}

#[gpui::test]
fn a_quiet_agent_capture_with_a_redacted_copy_keeps_only_the_redacted_pixels(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let out = rig.save_dir.join("shot.png");
    std::fs::create_dir_all(&rig.save_dir).unwrap();
    let options = es_app::agent_capture::Options {
        out: Some(out.clone()),
        ui: false,
        redacted: Some(black_copy()),
    };
    rig.agent_save(cx, options).unwrap();
    assert!(is_color(&out, [0, 0, 0, 255]), "the --out file");

    let options = es_app::agent_capture::Options {
        redacted: Some(black_copy()),
        ..Default::default()
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert!(
        is_color(&saved.path, [0, 0, 0, 255]),
        "the library copy made for the agent"
    );
    assert_eq!(rig.history_len(cx), 2, "one library entry per capture");
}

#[gpui::test]
fn with_the_interface_the_user_keeps_the_original_and_the_agent_gets_the_redacted_copy(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.configure(cx, |c| c.capture.keep_in_deck_until_saved = false);
    let out = rig.save_dir.join("shot.png");
    std::fs::create_dir_all(&rig.save_dir).unwrap();
    let options = es_app::agent_capture::Options {
        out: Some(out.clone()),
        ui: true,
        redacted: Some(black_copy()),
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert_eq!(saved.path, out);
    assert!(is_color(&out, [0, 0, 0, 255]), "the --out file");
    assert_eq!(rig.deck_len(cx), 1, "the card is the user's");

    let options = es_app::agent_capture::Options {
        ui: true,
        redacted: Some(black_copy()),
        ..Default::default()
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert!(is_color(&saved.path, [0, 0, 0, 255]), "the agent's file");
    let library = rig.history.read_with(cx, |h, _| h.entries().to_vec());
    assert!(
        library.iter().all(|e| e.file != saved.path),
        "the agent's file is not the user's"
    );
    assert_eq!(library.len(), 2);
    for entry in library {
        assert!(is_color(&entry.file, [0xFF, 0x80, 0, 255]), "the original");
    }
}
