use crate::probe::Provider;
use crate::traits::{BindHelper, PointerQuery, WindowList, WindowRules};
use es_model::platform::CursorSource;

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "hyprland",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "hyprland",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const RULES: Provider<dyn WindowRules> = Provider {
    name: "hyprland",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "hyprland",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const CURSOR: Provider<dyn CursorSource> = Provider {
    name: "hyprland",
    probe: |_| todo!(),
    build: |_| todo!(),
};
