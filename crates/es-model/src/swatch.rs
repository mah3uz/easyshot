use es_geom::Rgba;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Swatch {
    pub id: String,
    pub title: String,
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Default for Swatch {
    fn default() -> Self {
        Self::red()
    }
}

macro_rules! palette {
    ($($name:ident, $title:literal, $r:expr, $g:expr, $b:expr;)*) => {
        impl Swatch {
            $(pub fn $name() -> Self {
                Self::opaque(stringify!($name), $title, $r, $g, $b)
            })*

            pub fn palette() -> Vec<Swatch> {
                vec![$(Self::$name()),*]
            }
        }
    };
}

palette! {
    black, "Black", 0.02, 0.02, 0.024;
    red, "Red", 0.97, 0.22, 0.20;
    orange, "Orange", 1.0, 0.53, 0.08;
    yellow, "Yellow", 1.0, 0.82, 0.18;
    green, "Green", 0.18, 0.72, 0.36;
    turquoise, "Turquoise", 0.20, 0.77, 0.72;
    blue, "Blue", 0.18, 0.48, 1.0;
    purple, "Purple", 0.55, 0.30, 0.95;
    pink, "Pink", 1.0, 0.18, 0.43;
    white, "White", 0.96, 0.96, 0.96;
}

impl Swatch {
    fn opaque(id: &str, title: &str, red: f64, green: f64, blue: f64) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            red,
            green,
            blue,
            alpha: 1.0,
        }
    }

    pub fn by_id(id: &str) -> Option<Swatch> {
        Self::palette().into_iter().find(|s| s.id == id)
    }

    /// The id encodes the 0..=255 channels so a custom colour round-trips by id alone.
    pub fn custom(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            id: format!("custom-{r}-{g}-{b}-{a}"),
            title: "Custom".into(),
            red: f64::from(r) / 255.0,
            green: f64::from(g) / 255.0,
            blue: f64::from(b) / 255.0,
            alpha: f64::from(a) / 255.0,
        }
    }

    pub fn to_rgba(&self) -> Rgba {
        Rgba::new(
            self.red as f32,
            self.green as f32,
            self.blue as f32,
            self.alpha as f32,
        )
    }

    pub fn luminance(&self) -> f64 {
        0.299 * self.red + 0.587 * self.green + 0.114 * self.blue
    }

    /// A numbered badge on a light swatch uses dark text.
    pub fn prefers_dark_text(&self) -> bool {
        self.luminance() > 0.68
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_has_ten_swatches_and_red_is_default() {
        assert_eq!(Swatch::palette().len(), 10);
        assert_eq!(Swatch::default().id, "red");
        assert_eq!(Swatch::by_id("turquoise").map(|s| s.blue), Some(0.72));
        assert_eq!(Swatch::by_id("nope"), None);
    }

    #[test]
    fn badge_text_flips_on_light_swatches() {
        assert!(Swatch::white().prefers_dark_text());
        assert!(Swatch::yellow().prefers_dark_text());
        assert!(!Swatch::black().prefers_dark_text());
        assert!(!Swatch::red().prefers_dark_text());
    }

    #[test]
    fn custom_id_encodes_the_channels() {
        let s = Swatch::custom(255, 0, 128, 255);
        assert_eq!(s.id, "custom-255-0-128-255");
        assert_eq!(s.alpha, 1.0);
    }

    #[test]
    fn alpha_defaults_to_opaque_when_absent() {
        let s: Swatch =
            serde_json::from_str(r#"{"id":"x","title":"X","red":0.1,"green":0.2,"blue":0.3}"#)
                .unwrap();
        assert_eq!(s.alpha, 1.0);
    }
}
