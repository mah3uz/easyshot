use crate::probe::Provider;
use crate::traits::{
    BindHelper, Capture, Clipboard, PointerQuery, VirtualInput, WindowList, WindowRules,
};
use crate::{desktop, wayland};
use es_desktop::{Appearance, FilePicker, Notifier, Shortcuts, Tray};
use es_model::platform::CursorSource;
use es_record::Recorder;

#[cfg(feature = "hyprland")]
use crate::hyprland;
#[cfg(feature = "niri")]
use crate::niri;
#[cfg(feature = "wlroots")]
use crate::wlroots;

pub static CAPTURE: &[Provider<dyn Capture>] = &[
    wayland::EXT_IMAGE_COPY,
    wayland::WLR_SCREENCOPY,
    desktop::PORTAL_SCREENSHOT,
];

pub static WINDOWS: &[Provider<dyn WindowList>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::WINDOWS,
    #[cfg(feature = "wlroots")]
    wlroots::WINDOWS,
    #[cfg(feature = "niri")]
    niri::WINDOWS,
    wayland::FOREIGN_TOPLEVEL_NO_GEOMETRY,
];

pub static POINTER: &[Provider<dyn PointerQuery>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::POINTER,
    #[cfg(feature = "wlroots")]
    wlroots::POINTER,
    #[cfg(feature = "niri")]
    niri::POINTER,
];

pub static RULES: &[Provider<dyn WindowRules>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::RULES,
    #[cfg(feature = "wlroots")]
    wlroots::RULES,
];

pub static BINDS: &[Provider<dyn BindHelper>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::BINDS,
    #[cfg(feature = "wlroots")]
    wlroots::BINDS,
    #[cfg(feature = "niri")]
    niri::BINDS,
];

pub static CLIPBOARD: &[Provider<dyn Clipboard>] = &[wayland::DATA_CONTROL, wayland::DATA_DEVICE];

pub static VIRTUAL_INPUT: &[Provider<dyn VirtualInput>] = &[wayland::VIRTUAL_POINTER];

pub static CURSOR: &[Provider<dyn CursorSource>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::CURSOR,
];

pub static SHORTCUTS: &[Provider<dyn Shortcuts>] =
    &[desktop::PORTAL_GLOBAL_SHORTCUTS, desktop::USER_BINDS];

pub static TRAY: &[Provider<dyn Tray>] = &[desktop::STATUS_NOTIFIER];

pub static NOTIFIER: &[Provider<dyn Notifier>] = &[desktop::FREEDESKTOP_NOTIFICATIONS];

pub static FILES: &[Provider<dyn FilePicker>] = &[desktop::PORTAL_FILE_CHOOSER];

pub static APPEARANCE: &[Provider<dyn Appearance>] = &[desktop::PORTAL_SETTINGS];

pub static RECORDERS: &[Provider<dyn Recorder>] = &[desktop::PORTAL_RECORDER];
