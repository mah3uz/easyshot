#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Mp4,
    WebM,
    Gif,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Mp4, Format::WebM, Format::Gif];

    pub fn id(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::WebM => "webm",
            Format::Gif => "gif",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|f| f.id() == id)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    #[default]
    Original,
    P1080,
    P720,
    P480,
}

impl Size {
    pub const ALL: [Size; 4] = [Size::Original, Size::P1080, Size::P720, Size::P480];

    pub fn id(self) -> &'static str {
        match self {
            Size::Original => "original",
            Size::P1080 => "1080p",
            Size::P720 => "720p",
            Size::P480 => "480p",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.id() == id)
            .unwrap_or_default()
    }

    pub fn height(self) -> Option<u32> {
        match self {
            Size::Original => None,
            Size::P1080 => Some(1080),
            Size::P720 => Some(720),
            Size::P480 => Some(480),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    High,
    #[default]
    Balanced,
    Small,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::High, Quality::Balanced, Quality::Small];

    pub fn id(self) -> &'static str {
        match self {
            Quality::High => "high",
            Quality::Balanced => "balanced",
            Quality::Small => "small",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|q| q.id() == id)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    Natural,
    Vivid,
    Muted,
    Mono,
}

impl Look {
    pub const ALL: [Look; 4] = [Look::Natural, Look::Vivid, Look::Muted, Look::Mono];

    pub fn id(self) -> &'static str {
        match self {
            Look::Natural => "natural",
            Look::Vivid => "vivid",
            Look::Muted => "muted",
            Look::Mono => "mono",
        }
    }

    pub fn from_id(id: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|l| l.id() == id)
            .unwrap_or_default()
    }

    /// Contrast and saturation, 1.0 each for no change; the export and the preview both read this.
    pub fn tone(self) -> (f64, f64) {
        match self {
            Look::Natural => (1., 1.),
            Look::Vivid => (1.1, 1.4),
            Look::Muted => (0.95, 0.65),
            Look::Mono => (1.05, 0.),
        }
    }
}

pub const BRIGHTNESS_LIMIT: f64 = 0.3;

pub const VOLUME_LIMIT: f64 = 2.;
pub const SYSTEM_VOLUME: f64 = 0.5;
pub const MIC_VOLUME: f64 = 1.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportOptions {
    pub format: Format,
    pub size: Size,
    pub quality: Quality,
    pub look: Look,
    pub brightness: f64,
    pub system_volume: f64,
    pub mic_volume: f64,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: Format::default(),
            size: Size::default(),
            quality: Quality::default(),
            look: Look::default(),
            brightness: 0.,
            system_volume: SYSTEM_VOLUME,
            mic_volume: MIC_VOLUME,
        }
    }
}

impl ExportOptions {
    pub fn from_config(r: &es_model::config::Recording) -> Self {
        Self {
            format: Format::from_id(&r.export_format),
            size: Size::from_id(&r.export_size),
            quality: Quality::from_id(&r.export_quality),
            look: Look::from_id(&r.export_look),
            brightness: r
                .export_brightness
                .clamp(-BRIGHTNESS_LIMIT, BRIGHTNESS_LIMIT),
            system_volume: r.export_system_volume.clamp(0., VOLUME_LIMIT),
            mic_volume: r.export_mic_volume.clamp(0., VOLUME_LIMIT),
        }
    }

    pub fn store(&self, r: &mut es_model::config::Recording) {
        r.export_format = self.format.id().into();
        r.export_size = self.size.id().into();
        r.export_quality = self.quality.id().into();
        r.export_look = self.look.id().into();
        r.export_brightness = self.brightness;
        r.export_system_volume = self.system_volume;
        r.export_mic_volume = self.mic_volume;
    }

    /// Whether the picture itself changes, which rules out copying the stream.
    pub fn alters_picture(&self) -> bool {
        self.size != Size::Original || self.look != Look::Natural || self.brightness != 0.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_protocol::agent::{FORMATS, LOOKS, MAX_BRIGHTNESS_MILLI, QUALITIES, SIZES};

    #[test]
    fn the_ids_the_agent_command_accepts_are_the_ones_the_export_knows() {
        fn same(listed: &[&str], known: &[&str]) {
            assert_eq!(listed, known);
        }
        same(&FORMATS, &Format::ALL.map(Format::id));
        same(&SIZES, &Size::ALL.map(Size::id));
        same(&QUALITIES, &Quality::ALL.map(Quality::id));
        same(&LOOKS, &Look::ALL.map(Look::id));
        assert_eq!(MAX_BRIGHTNESS_MILLI as f64 / 1000., BRIGHTNESS_LIMIT);
    }
}
