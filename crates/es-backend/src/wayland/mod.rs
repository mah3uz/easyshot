use crate::probe::Provider;
use crate::traits::{Capture, Clipboard, VirtualInput, WindowList};

pub const EXT_IMAGE_COPY: Provider<dyn Capture> = Provider {
    name: "ext-image-copy",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const WLR_SCREENCOPY: Provider<dyn Capture> = Provider {
    name: "wlr-screencopy",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const FOREIGN_TOPLEVEL_NO_GEOMETRY: Provider<dyn WindowList> = Provider {
    name: "foreign-toplevel",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const DATA_CONTROL: Provider<dyn Clipboard> = Provider {
    name: "data-control",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const DATA_DEVICE: Provider<dyn Clipboard> = Provider {
    name: "data-device",
    probe: |_| todo!(),
    build: |_| todo!(),
};

pub const VIRTUAL_POINTER: Provider<dyn VirtualInput> = Provider {
    name: "virtual-pointer",
    probe: |_| todo!(),
    build: |_| todo!(),
};
