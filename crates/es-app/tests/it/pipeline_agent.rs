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
fn an_agent_capture_to_a_file_only_writes_that_file_and_nothing_else(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let out = rig.save_dir.join("shot.png");
    std::fs::create_dir_all(&rig.save_dir).unwrap();
    let options = es_app::agent_capture::Options {
        out: Some(out.clone()),
        ui: false,
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert_eq!(saved.path, out);
    assert_eq!((saved.width, saved.height), (4, 2));
    assert_eq!(image::open(&out).unwrap().to_rgba8().dimensions(), (4, 2));
    assert_eq!(rig.history_len(cx), 0, "no library entry");
    assert_eq!(rig.deck_len(cx), 0);
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
    assert_eq!(rig.history_len(cx), 1);
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
    };
    let saved = rig.agent_save(cx, options).unwrap();
    assert_eq!(saved.path, out);
    assert!(out.exists());
    assert_eq!(rig.deck_len(cx), 1);
}
