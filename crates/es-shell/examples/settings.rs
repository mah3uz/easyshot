use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_shell::settings::{self, SettingsDeps, SystemClipboard};
use es_store::{ConfigFile, Paths};
use gpui::AppContext;
use std::rc::Rc;

fn main() {
    let root = std::env::temp_dir().join("easyshot-settings-example");
    let paths = Paths::rooted(&root);
    if let Err(error) = paths.ensure_all() {
        eprintln!("{error}");
        return;
    }
    let file = match ConfigFile::load(&paths.config_file()) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(move |cx| {
            es_ui::components::gallery::init(cx);
            let deps = SettingsDeps {
                settings: cx.new(|_| SettingsModel::new(file)),
                history: None,
                caps: Caps::CAPTURE_OUTPUT | Caps::LAYER_SHELL,
                binds: None,
                paths: paths.clone(),
                clipboard: Rc::new(SystemClipboard),
                outputs: Vec::new(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            };
            if let Err(error) = settings::open(deps, cx) {
                eprintln!("{error}");
            }
        });
}
