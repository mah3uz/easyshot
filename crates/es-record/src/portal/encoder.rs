use crate::Codec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoder {
    pub element: &'static str,
    pub codec: Codec,
    pub props: &'static [(&'static str, &'static str)],
}

pub const ENCODERS: &[Encoder] = &[
    Encoder {
        element: "vah264enc",
        codec: Codec::H264,
        props: &[],
    },
    Encoder {
        element: "nvh264enc",
        codec: Codec::H264,
        props: &[],
    },
    Encoder {
        element: "x264enc",
        codec: Codec::H264,
        props: &[("tune", "zerolatency"), ("speed-preset", "veryfast")],
    },
    Encoder {
        element: "openh264enc",
        codec: Codec::H264,
        props: &[],
    },
    Encoder {
        element: "vp9enc",
        codec: Codec::Vp9,
        props: &[("deadline", "1"), ("cpu-used", "8")],
    },
];

pub fn pick_encoder(has_element: impl Fn(&str) -> bool) -> Option<Encoder> {
    ENCODERS.iter().copied().find(|e| has_element(e.element))
}
