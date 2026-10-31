use crate::probe::Provider;
use crate::traits::{Capture};
use es_desktop::Appearance;
use es_desktop::FilePicker;
use es_desktop::Notifier;
use es_record::Recorder;
use es_desktop::Shortcuts;
use es_desktop::Tray;

pub const PORTAL_SCREENSHOT: Provider<dyn Capture> = Provider {
    name: "portal-screenshot",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const PORTAL_GLOBAL_SHORTCUTS: Provider<dyn Shortcuts> = Provider {
    name: "portal-global-shortcuts",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const USER_BINDS: Provider<dyn Shortcuts> = Provider {
    name: "user-binds",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const STATUS_NOTIFIER: Provider<dyn Tray> = Provider {
    name: "status-notifier",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const FREEDESKTOP_NOTIFICATIONS: Provider<dyn Notifier> = Provider {
    name: "freedesktop-notifications",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const PORTAL_FILE_CHOOSER: Provider<dyn FilePicker> = Provider {
    name: "portal-file-chooser",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const PORTAL_SETTINGS: Provider<dyn Appearance> = Provider {
    name: "portal-settings",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const PORTAL_RECORDER: Provider<dyn Recorder> = Provider {
    name: "portal-recorder",
    probe: |_| todo!(),
    build: |_| todo!(),
};
