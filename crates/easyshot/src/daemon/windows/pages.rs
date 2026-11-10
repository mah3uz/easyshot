use es_protocol::SettingsPage;
use es_shell::settings;

pub(super) fn page_of(page: SettingsPage) -> settings::Page {
    use settings::Page;
    match page {
        SettingsPage::Home => Page::Home,
        SettingsPage::General => Page::General,
        SettingsPage::Capture => Page::Capture,
        SettingsPage::Overlay => Page::Overlay,
        SettingsPage::Recording => Page::Recording,
        SettingsPage::Shortcuts => Page::Shortcuts,
        SettingsPage::Agents => Page::Agents,
        SettingsPage::About => Page::About,
    }
}
