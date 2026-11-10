use es_app::toast::{IconName, Toast, show_toast};
use gpui::App;

pub(super) enum Feature {
    CloudShare,
    #[allow(dead_code)] // no entry point yet; the surface is shared
    VideoStudio,
}

pub(super) fn coming_soon(feature: Feature, cx: &mut App) {
    let (title, icon) = match feature {
        Feature::CloudShare => ("toast-coming-soon-cloud-share", IconName::Info),
        Feature::VideoStudio => ("toast-coming-soon-video-studio", IconName::Info),
    };
    show_toast(Toast::new(title, icon), cx);
}
