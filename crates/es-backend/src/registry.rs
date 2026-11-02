use crate::backend::{ProviderReport, ReportEntry};
use crate::probe::{Probe, Provider};
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

pub static CLIPBOARD: &[Provider<dyn Clipboard>] = &[wayland::DATA_CONTROL];

pub static VIRTUAL_INPUT: &[Provider<dyn VirtualInput>] = &[wayland::VIRTUAL_POINTER];

pub static CURSOR: &[Provider<dyn CursorSource>] = &[
    #[cfg(feature = "hyprland")]
    hyprland::CURSOR,
    wayland::CURSOR_SESSION,
];

pub static SHORTCUTS: &[Provider<dyn Shortcuts>] =
    &[desktop::PORTAL_GLOBAL_SHORTCUTS, desktop::USER_BINDS];

pub static TRAY: &[Provider<dyn Tray>] = &[desktop::STATUS_NOTIFIER];

pub static NOTIFIER: &[Provider<dyn Notifier>] = &[desktop::FREEDESKTOP_NOTIFICATIONS];

pub static FILES: &[Provider<dyn FilePicker>] = &[desktop::PORTAL_FILE_CHOOSER];

pub static APPEARANCE: &[Provider<dyn Appearance>] = &[desktop::PORTAL_SETTINGS];

pub static RECORDERS: &[Provider<dyn Recorder>] = &[
    desktop::GSR_NATIVE,
    desktop::WF_RECORDER,
    desktop::PORTAL_RECORDER,
];

pub struct Registry {
    pub capture: &'static [Provider<dyn Capture>],
    pub windows: &'static [Provider<dyn WindowList>],
    pub pointer: &'static [Provider<dyn PointerQuery>],
    pub rules: &'static [Provider<dyn WindowRules>],
    pub binds: &'static [Provider<dyn BindHelper>],
    pub clipboard: &'static [Provider<dyn Clipboard>],
    pub virtual_input: &'static [Provider<dyn VirtualInput>],
    pub cursor: &'static [Provider<dyn CursorSource>],
    pub shortcuts: &'static [Provider<dyn Shortcuts>],
    pub tray: &'static [Provider<dyn Tray>],
    pub notifier: &'static [Provider<dyn Notifier>],
    pub files: &'static [Provider<dyn FilePicker>],
    pub appearance: &'static [Provider<dyn Appearance>],
    pub recorders: &'static [Provider<dyn Recorder>],
}

impl Registry {
    pub const EMPTY: Registry = Registry {
        capture: &[],
        windows: &[],
        pointer: &[],
        rules: &[],
        binds: &[],
        clipboard: &[],
        virtual_input: &[],
        cursor: &[],
        shortcuts: &[],
        tray: &[],
        notifier: &[],
        files: &[],
        appearance: &[],
        recorders: &[],
    };
}

impl Default for Registry {
    fn default() -> Self {
        Registry {
            capture: CAPTURE,
            windows: WINDOWS,
            pointer: POINTER,
            rules: RULES,
            binds: BINDS,
            clipboard: CLIPBOARD,
            virtual_input: VIRTUAL_INPUT,
            cursor: CURSOR,
            shortcuts: SHORTCUTS,
            tray: TRAY,
            notifier: NOTIFIER,
            files: FILES,
            appearance: APPEARANCE,
            recorders: RECORDERS,
        }
    }
}

pub fn force_var(capability: &str) -> String {
    format!("EASYSHOT_PROVIDER_{}", capability.to_ascii_uppercase())
}

fn candidates<'p, T: ?Sized>(
    capability: &'static str,
    list: &'p [Provider<T>],
    probe: &Probe,
    entry: &mut ReportEntry,
) -> Vec<&'p Provider<T>> {
    let var = force_var(capability);
    if let Some(forced) = probe.env.var(&var) {
        entry.forced = Some(forced.to_owned());
        let mut chosen = Vec::new();
        for p in list {
            if p.name == forced {
                chosen.push(p);
            } else {
                entry
                    .rejected
                    .push((p.name, format!("{var} selects {forced}")));
            }
        }
        if chosen.is_empty() {
            tracing::warn!("{var}={forced} names no {capability} provider");
        }
        return chosen;
    }
    let mut ranked: Vec<(u8, &Provider<T>)> = Vec::with_capacity(list.len());
    for p in list {
        match (p.probe)(probe) {
            Some(priority) => ranked.push((priority, p)),
            None => entry.rejected.push((p.name, "not detected".to_owned())),
        }
    }
    ranked.sort_by_key(|(priority, _)| std::cmp::Reverse(*priority));
    ranked.into_iter().map(|(_, p)| p).collect()
}

pub fn select<T: ?Sized>(
    capability: &'static str,
    list: &[Provider<T>],
    probe: &Probe,
    report: &mut ProviderReport,
) -> Option<Box<T>> {
    let mut entry = ReportEntry {
        capability,
        ..ReportEntry::default()
    };
    let mut built = None;
    for p in candidates(capability, list, probe, &mut entry) {
        if let Some(chosen) = entry.chosen {
            entry
                .rejected
                .push((p.name, format!("outranked by {chosen}")));
            continue;
        }
        match (p.build)(probe) {
            Ok(b) => {
                entry.chosen = Some(p.name);
                built = Some(b);
            }
            Err(e) => entry.rejected.push((p.name, e.to_string())),
        }
    }
    report.entries.push(entry);
    built
}

/// Every provider that probes and builds, best first; `chosen` names the first.
pub fn select_all<T: ?Sized>(
    capability: &'static str,
    list: &[Provider<T>],
    probe: &Probe,
    report: &mut ProviderReport,
) -> Vec<Box<T>> {
    let mut entry = ReportEntry {
        capability,
        ..ReportEntry::default()
    };
    let mut built = Vec::new();
    for p in candidates(capability, list, probe, &mut entry) {
        match (p.build)(probe) {
            Ok(b) => {
                entry.chosen.get_or_insert(p.name);
                built.push(b);
            }
            Err(e) => entry.rejected.push((p.name, e.to_string())),
        }
    }
    report.entries.push(entry);
    built
}
