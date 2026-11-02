use cosmic_text::fontdb::Database;
use cosmic_text::{Attrs, Family, FontSystem, Style, Weight};

use crate::FontFamily;

/// Every bundled face; the interface registers the same bytes with its own text system.
pub const BUNDLED: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Bold.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Italic.ttf"),
    include_bytes!("../../../assets/fonts/Inter-BoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/InterTight-Regular.ttf"),
    include_bytes!("../../../assets/fonts/InterTight-Bold.ttf"),
    include_bytes!("../../../assets/fonts/InterTight-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/InterTight-Italic.ttf"),
    include_bytes!("../../../assets/fonts/InterTight-BoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/Nunito[wght].ttf"),
    include_bytes!("../../../assets/fonts/Nunito-Italic[wght].ttf"),
];

pub struct FontSet {
    db: Database,
}

impl FontSet {
    pub fn bundled() -> Self {
        Self::from_data(BUNDLED.iter().map(|b| b.to_vec()))
    }

    pub fn from_data(fonts: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let mut db = Database::new();
        for data in fonts {
            db.load_font_data(data);
        }
        Self { db }
    }

    pub(crate) fn into_system(self) -> FontSystem {
        FontSystem::new_with_locale_and_db("en-US".to_owned(), self.db)
    }
}

pub(crate) fn family_name(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Pro => "Inter",
        FontFamily::Compact => "Inter Tight",
        FontFamily::Rounded => "Nunito",
    }
}

pub(crate) fn attrs(family: FontFamily, bold: bool, italic: bool) -> Attrs<'static> {
    Attrs::new()
        .family(Family::Name(family_name(family)))
        .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
        .style(if italic { Style::Italic } else { Style::Normal })
}
