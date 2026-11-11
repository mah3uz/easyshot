use es_app::history::History;
use es_app::platform_ui::ClipboardSink;
use es_app::settings_model::SettingsModel;
use es_app::share::{LiveTargets, Shares, SystemKeyring};
use es_store::Paths;
use gpui::{App, AppContext as _, Entity};
use std::rc::Rc;
use std::sync::Arc;

pub fn engine(
    paths: &Paths,
    settings: &Entity<SettingsModel>,
    history: &Entity<History>,
    clipboard: Rc<dyn ClipboardSink>,
    cx: &mut App,
) -> Entity<Shares> {
    let targets = LiveTargets::new(
        paths.clone(),
        env!("EASYSHOT_VERSION"),
        Arc::new(SystemKeyring),
    );
    let (settings, history) = (settings.clone(), history.clone());
    cx.new(|cx| Shares::new(paths, settings, history, clipboard, Arc::new(targets), cx))
}
