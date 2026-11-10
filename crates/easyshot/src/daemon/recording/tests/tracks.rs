use super::*;

#[gpui::test]
async fn capture_json_lists_the_sound_tracks_system_first(cx: &mut TestAppContext) {
    let rig = rig_with(cx, FakeRecorder::default());
    let mut start = display();
    start.request.system_audio = true;
    start.request.mic = Some("default".into());
    cx.update(|cx| rig.recordings.start(start, cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();
    let folder = rig.folders().remove(0);
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("capture.json")).unwrap()).unwrap();
    assert_eq!(
        capture["audio_tracks"],
        serde_json::json!(["system", "mic"])
    );
}
