use es_app::settings_model::SettingsModel;
use es_desktop::{RecordingSound, Sound};
use gpui::{App, Entity};
use std::sync::Arc;

#[cfg(not(feature = "sound"))]
struct Silent;

#[cfg(not(feature = "sound"))]
impl Sound for Silent {
    fn shutter(&self) {}
}

/// Test mode counts shutters and never plays one.
pub fn player(test_mode: bool) -> Arc<dyn Sound> {
    if test_mode {
        return Arc::new(RecordingSound::default());
    }
    #[cfg(feature = "sound")]
    return Arc::new(es_desktop::RodioSound);
    #[cfg(not(feature = "sound"))]
    Arc::new(Silent)
}

pub fn play_after_capture(settings: &Entity<SettingsModel>, sound: &dyn Sound, cx: &App) {
    if settings.read(cx).config().general.play_sound {
        sound.shutter();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_store::ConfigFile;
    use gpui::{AppContext as _, TestAppContext};

    #[gpui::test]
    fn the_shutter_follows_the_sound_setting(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let config = ConfigFile::load(&dir.path().join("config.toml")).unwrap();
        let settings = cx.update(|cx| cx.new(|_| SettingsModel::new(config)));
        let sound = RecordingSound::default();
        cx.update(|cx| play_after_capture(&settings, &sound, cx));
        assert_eq!(sound.shutters(), 1, "on by default");
        settings.update(cx, |s, cx| s.update(|c| c.general.play_sound = false, cx));
        cx.update(|cx| play_after_capture(&settings, &sound, cx));
        assert_eq!(sound.shutters(), 1, "off plays nothing");
    }
}
