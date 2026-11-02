use crate::error::BackendError;
use crate::probe::Provider;
use crate::traits::{BindHelper, PointerQuery, WindowList, WindowRules};
use es_model::platform::CursorSource;

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "hyprland",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "hyprland",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const RULES: Provider<dyn WindowRules> = Provider {
    name: "hyprland",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "hyprland",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const CURSOR: Provider<dyn CursorSource> = Provider {
    name: "hyprland",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};
