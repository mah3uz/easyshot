use crate::error::BackendError;
use crate::probe::Provider;
use crate::traits::{BindHelper, PointerQuery, WindowList};

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "niri",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "niri",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "niri",
    probe: |_| None,
    build: |_| Err(BackendError::Unavailable("not detected".into())),
};
