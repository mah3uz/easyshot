use std::{ops::Deref, sync::Arc};

use es_geom::Rgba;
use gpui::{FontFeatures, FontWeight, Hsla, Pixels, Styled, px};

use crate::convert::hsla;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    /// The desktop's colour scheme as the caller read it; `None` is "no preference".
    System(Option<Appearance>),
}

impl ThemeMode {
    /// Dark is the app's own look, so a desktop with no preference gets dark.
    pub fn appearance(self) -> Appearance {
        match self {
            Self::Light | Self::System(Some(Appearance::Light)) => Appearance::Light,
            Self::Dark | Self::System(Some(Appearance::Dark) | None) => Appearance::Dark,
        }
    }
}

const fn hex(rgb: u32) -> Rgba {
    Rgba::new(
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
        1.0,
    )
}

const fn alpha(rgb: u32, a: f32) -> Rgba {
    let c = hex(rgb);
    Rgba::new(c.r, c.g, c.b, a)
}

const fn white(a: f32) -> Rgba {
    alpha(0xFFFFFF, a)
}

const fn black(a: f32) -> Rgba {
    alpha(0x000000, a)
}

macro_rules! colours {
    ($($field:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Colours<C> {
            $(pub $field: C,)*
        }

        impl<C: Copy> Colours<C> {
            pub fn map<D>(&self, f: impl Fn(C) -> D) -> Colours<D> {
                Colours { $($field: f(self.$field),)* }
            }

            pub fn named(&self) -> Vec<(&'static str, C)> {
                vec![$((stringify!($field), self.$field),)*]
            }
        }
    };
}

colours!(
    window,
    workspace,
    inspector,
    track,
    value_box,
    segment_track,
    fill,
    selected_segment,
    dropdown,
    separator,
    border,
    accent,
    selection,
    accent_tint,
    on_accent,
    text,
    text_secondary,
    text_tertiary,
    destructive,
    success,
    hover_fill,
    hover_border,
    press_fill,
    bordered_fill,
    bordered_border,
);

pub const DARK: Colours<Rgba> = Colours {
    window: hex(0x1B1B1B),
    workspace: hex(0x1B1B1B),
    inspector: hex(0x2B2B2B),
    track: hex(0x363636),
    value_box: hex(0x363636),
    segment_track: hex(0x363636),
    fill: hex(0x494949),
    selected_segment: hex(0x494949),
    dropdown: hex(0x444444),
    separator: hex(0x464646),
    border: white(0.10),
    accent: hex(0x008EFB),
    selection: hex(0x3182ED),
    accent_tint: hex(0x456383),
    on_accent: hex(0xFFFFFF),
    text: white(0.92),
    text_secondary: white(0.55),
    text_tertiary: white(0.26),
    destructive: hex(0xFF453A),
    success: hex(0x32D74B),
    hover_fill: white(0.10),
    hover_border: white(0.25),
    press_fill: white(0.16),
    bordered_fill: white(0.05),
    bordered_border: white(0.15),
};

pub const LIGHT: Colours<Rgba> = Colours {
    window: hex(0xFFFFFF),
    workspace: hex(0xECECEC),
    inspector: hex(0xEAEAEA),
    track: hex(0xE0E0E0),
    value_box: hex(0xD3D3D3),
    segment_track: hex(0xE0E0E0),
    fill: hex(0xD2D2D2),
    selected_segment: hex(0xD2D2D2),
    dropdown: hex(0xFFFFFF),
    separator: black(0.10),
    border: black(0.10),
    accent: hex(0x1184F6),
    selection: hex(0x3182ED),
    accent_tint: alpha(0x1184F6, 0.12),
    on_accent: hex(0xFFFFFF),
    text: black(0.88),
    text_secondary: black(0.50),
    text_tertiary: black(0.26),
    destructive: hex(0xD70015),
    success: hex(0x248A3D),
    hover_fill: black(0.10),
    hover_border: black(0.25),
    press_fill: black(0.16),
    bordered_fill: black(0.05),
    bordered_border: black(0.15),
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub inspector_padding: Pixels,
    pub section_padding: Pixels,
    pub header_spacing: Pixels,
    pub row_spacing: Pixels,
    pub group_label_spacing: Pixels,
    pub control_height: Pixels,
    pub control_inset: Pixels,
    pub slider_height: Pixels,
    pub slider_value_width: Pixels,
    pub label_column: Pixels,
    pub header_height: Pixels,
    pub button_height: Pixels,
    pub button_padding_x: Pixels,
    pub tool_button_padding_x: Pixels,
    pub field_radius: Pixels,
    pub tile_radius: Pixels,
    pub button_radius: Pixels,
    pub slider_radius: Pixels,
    pub card_radius: Pixels,
    pub popover_radius: Pixels,
    pub hairline: Pixels,
    pub focus_ring: Pixels,
}

pub const METRICS: Metrics = Metrics {
    inspector_padding: px(12.),
    section_padding: px(12.),
    header_spacing: px(10.),
    row_spacing: px(8.),
    group_label_spacing: px(7.),
    control_height: px(24.),
    control_inset: px(2.),
    slider_height: px(32.),
    slider_value_width: px(60.),
    label_column: px(58.),
    header_height: px(38.),
    button_height: px(32.),
    button_padding_x: px(10.),
    tool_button_padding_x: px(6.),
    field_radius: px(5.),
    tile_radius: px(6.),
    button_radius: px(6.),
    slider_radius: px(8.),
    card_radius: px(10.),
    popover_radius: px(12.),
    hairline: px(1.),
    focus_ring: px(2.),
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: Pixels,
    pub weight: FontWeight,
    pub tabular: bool,
}

impl TextStyle {
    const fn new(size: f32, weight: FontWeight) -> Self {
        Self {
            size: px(size),
            weight,
            tabular: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypeScale {
    pub title: TextStyle,
    pub tool_button: TextStyle,
    pub footer: TextStyle,
    pub section_header: TextStyle,
    pub label: TextStyle,
    pub value: TextStyle,
    pub segmented: TextStyle,
}

pub const TYPE: TypeScale = TypeScale {
    title: TextStyle::new(13., FontWeight::SEMIBOLD),
    tool_button: TextStyle::new(12., FontWeight::MEDIUM),
    footer: TextStyle::new(12., FontWeight::NORMAL),
    section_header: TextStyle::new(11., FontWeight::SEMIBOLD),
    label: TextStyle::new(11., FontWeight::NORMAL),
    value: TextStyle {
        tabular: true,
        ..TextStyle::new(11., FontWeight::MEDIUM)
    },
    segmented: TextStyle::new(11., FontWeight::MEDIUM),
};

pub trait Typeset: Styled + Sized {
    fn typeset(self, style: TextStyle) -> Self {
        let this = self.text_size(style.size).font_weight(style.weight);
        if style.tabular {
            this.font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        } else {
            this
        }
    }
}

impl<E: Styled> Typeset for E {}

#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub appearance: Appearance,
    pub colours: Colours<Hsla>,
    pub metrics: Metrics,
    pub type_scale: TypeScale,
}

impl Tokens {
    pub fn new(appearance: Appearance) -> Self {
        let source = match appearance {
            Appearance::Light => &LIGHT,
            Appearance::Dark => &DARK,
        };
        Self {
            appearance,
            colours: source.map(hsla),
            metrics: METRICS,
            type_scale: TYPE,
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        Self::new(mode.appearance())
    }
}

impl Deref for Tokens {
    type Target = Colours<Hsla>;

    fn deref(&self) -> &Self::Target {
        &self.colours
    }
}
