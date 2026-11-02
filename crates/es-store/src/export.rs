#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 2] = [ExportFormat::Png, ExportFormat::Jpeg];

    /// Anything not offered, such as a stored format that was dropped, is PNG.
    pub fn from_setting(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "jpeg" | "jpg" => Self::Jpeg,
            _ => Self::Png,
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            _ => None,
        }
    }

    pub fn setting_name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }

    pub fn is_lossy(self) -> bool {
        self == Self::Jpeg
    }
}

/// JPEG quality percent from the 0..=1 setting.
pub fn jpeg_quality_percent(setting: f64) -> u8 {
    if setting.is_nan() {
        return 90;
    }
    (setting.clamp(0.01, 1.0) * 100.0).round() as u8
}
