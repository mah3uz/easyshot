use super::{Feature, coming_soon, record};
use gpui::TestAppContext;

#[gpui::test]
fn every_ask_reaches_the_one_panel_with_its_feature(cx: &mut TestAppContext) {
    let asked = cx.update(record);
    cx.update(|cx| {
        coming_soon(Feature::VideoStudio, cx);
        coming_soon(Feature::VideoStudio, cx);
    });
    assert_eq!(
        *asked.borrow(),
        [Feature::VideoStudio, Feature::VideoStudio]
    );
}

#[cfg(feature = "player")]
#[gpui::test]
fn open_in_studio_in_the_trim_viewer_asks_for_video_studio(cx: &mut TestAppContext) {
    use crate::daemon::recording::ui::on_player_event;
    use es_overlays::player::PlayerEvent;
    let asked = cx.update(record);
    cx.update(|cx| on_player_event(&PlayerEvent::OpenStudio, None, cx));
    assert_eq!(*asked.borrow(), [Feature::VideoStudio]);
}
