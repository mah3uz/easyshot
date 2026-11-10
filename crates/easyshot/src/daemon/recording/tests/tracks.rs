use super::*;
use es_model::recording::AudioTrack;

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
    assert_eq!(
        rig.stored::<RecordingCapture>().audio_tracks,
        [AudioTrack::System, AudioTrack::Mic]
    );
}
