use crate::probe::Provider;
use crate::traits::{BindHelper, PointerQuery, WindowList};

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "niri",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "niri",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "niri",
    probe: |_| todo!(),
    build: |_| todo!(),
};
