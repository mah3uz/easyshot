use crate::error::BackendError;
use crate::probe::Provider;
use crate::traits::{BindHelper, PointerQuery, WindowList, WindowRules};

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "wlroots",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "wlroots",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const RULES: Provider<dyn WindowRules> = Provider {
    name: "wlroots",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "wlroots",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};
