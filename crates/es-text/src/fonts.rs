use std::collections::HashSet;
use std::path::Path;

use cosmic_text::fontdb::{Database, ID};
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

const BUNDLED_NAMES: [&str; 3] = ["Inter", "Inter Tight", "Nunito"];

pub struct FontSet {
    db: Database,
}

impl FontSet {
    pub fn bundled() -> Self {
        Self::from_data(BUNDLED.iter().map(|b| b.to_vec()))
    }

    /// The bundled faces and every font fontconfig lists.
    pub fn installed() -> Self {
        Self::bundled().adding(Database::load_system_fonts)
    }

    pub fn from_data(fonts: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let mut db = Database::new();
        for data in fonts {
            db.load_font_data(data);
        }
        Self { db }
    }

    /// Adds the fonts in `dir` as installed families.
    pub fn with_dir(self, dir: &Path) -> Self {
        self.adding(|db| db.load_fonts_dir(dir))
    }

    /// An installed copy of a bundled family is dropped, so a bundled family always draws its own files.
    fn adding(mut self, load: impl FnOnce(&mut Database)) -> Self {
        let before: HashSet<ID> = self.db.faces().map(|f| f.id).collect();
        load(&mut self.db);
        let shadowing: Vec<ID> = self
            .db
            .faces()
            .filter(|f| !before.contains(&f.id))
            .filter(|f| {
                f.families
                    .iter()
                    .any(|(n, _)| BUNDLED_NAMES.contains(&n.as_str()))
            })
            .map(|f| f.id)
            .collect();
        for id in shadowing {
            self.db.remove_face(id);
        }
        self
    }

    pub(crate) fn into_system(self) -> FontSystem {
        FontSystem::new_with_locale_and_db("en-US".to_owned(), self.db)
    }
}

/// Every name a face answers to, and each installed face's first name sorted for a picker.
pub(crate) fn installed_families(db: &Database) -> (HashSet<String>, Vec<String>) {
    let mut known = HashSet::new();
    let mut listed = HashSet::new();
    for face in db.faces() {
        for (name, _) in &face.families {
            known.insert(name.clone());
        }
        if let Some((name, _)) = face.families.first()
            && !BUNDLED_NAMES.contains(&name.as_str())
        {
            listed.insert(name.clone());
        }
    }
    let mut listed: Vec<String> = listed.into_iter().collect();
    listed.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    (known, listed)
}

fn bundled_name(family: &FontFamily) -> &'static str {
    match family {
        FontFamily::Compact => BUNDLED_NAMES[1],
        FontFamily::Rounded => BUNDLED_NAMES[2],
        FontFamily::Pro | FontFamily::System(_) => BUNDLED_NAMES[0],
    }
}

/// A family the font set lacks draws in the default bundled family.
pub(crate) fn resolve<'a>(family: &'a FontFamily, known: &HashSet<String>) -> &'a str {
    match family {
        FontFamily::System(name) if known.contains(name) => name,
        other => bundled_name(other),
    }
}

pub(crate) fn attrs(name: &str, bold: bool, italic: bool) -> Attrs<'_> {
    Attrs::new()
        .family(Family::Name(name))
        .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
        .style(if italic { Style::Italic } else { Style::Normal })
}
