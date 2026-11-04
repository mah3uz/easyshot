use es_anno::StylePatch;
use es_model::FontFamily;
use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::{
    IndexPath,
    select::{SearchableVec, SelectEvent, SelectItem, SelectState},
};

use super::ToolOptions;
use crate::strings::fl;

const SEARCH_AFTER: usize = 12;

pub(super) type FontList = SelectState<SearchableVec<FontChoice>>;

#[derive(Clone)]
pub(super) struct FontChoice {
    family: FontFamily,
    title: SharedString,
}

impl SelectItem for FontChoice {
    type Value = FontFamily;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &FontFamily {
        &self.family
    }
}

/// The bundled families first, then the installed ones in the order given.
pub(super) fn font_choices(installed: &[String]) -> Vec<FontFamily> {
    [FontFamily::Pro, FontFamily::Compact, FontFamily::Rounded]
        .into_iter()
        .chain(installed.iter().cloned().map(FontFamily::System))
        .collect()
}

pub(super) fn family_title(family: &FontFamily, installed: bool) -> String {
    match family {
        FontFamily::Pro => fl!("editor-font-pro"),
        FontFamily::Compact => fl!("editor-font-compact"),
        FontFamily::Rounded => fl!("editor-font-rounded"),
        FontFamily::System(name) if installed => name.clone(),
        FontFamily::System(name) => fl!("editor-font-missing", family = name.clone()),
    }
}

pub(super) struct FontPicker {
    list: Entity<FontList>,
    shown: Option<FontFamily>,
}

impl ToolOptions {
    /// Built on first draw, since the list needs a window; follows the current family after that.
    pub(super) fn font_list(
        &mut self,
        current: &FontFamily,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<FontList> {
        if let Some(picker) = &mut self.fonts {
            if picker.shown.as_ref() != Some(current) {
                picker.shown = Some(current.clone());
                picker
                    .list
                    .update(cx, |l, cx| l.set_selected_value(current, window, cx));
            }
            return picker.list.clone();
        }
        let families = font_choices(&self.doc.read(cx).text().installed_families());
        let selected = families
            .iter()
            .position(|f| f == current)
            .map(|row| IndexPath::default().row(row));
        let searchable = families.len() > SEARCH_AFTER;
        let choices: Vec<FontChoice> = families
            .into_iter()
            .map(|family| FontChoice {
                title: family_title(&family, true).into(),
                family,
            })
            .collect();
        let list = cx.new(|cx| {
            SelectState::new(SearchableVec::new(choices), selected, window, cx)
                .searchable(searchable)
        });
        let on_pick = cx.subscribe(&list, |this, _, event: &SelectEvent<_>, cx| {
            if let SelectEvent::Confirm(Some(family)) = event {
                this.apply_style(StylePatch::FontFamily(family.clone()), None, cx);
            }
        });
        self._subscriptions.push(on_pick);
        self.fonts = Some(FontPicker {
            list: list.clone(),
            shown: Some(current.clone()),
        });
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_families_come_before_the_installed_ones_in_their_given_order() {
        let installed = ["Cantarell".to_owned(), "DejaVu Serif".to_owned()];
        assert_eq!(
            font_choices(&installed),
            [
                FontFamily::Pro,
                FontFamily::Compact,
                FontFamily::Rounded,
                FontFamily::System("Cantarell".into()),
                FontFamily::System("DejaVu Serif".into()),
            ]
        );
        assert_eq!(font_choices(&[]).len(), 3);
    }

    #[test]
    fn a_family_that_is_not_installed_says_so_and_an_installed_one_is_its_name() {
        let gone = FontFamily::System("Gone Sans".into());
        assert_eq!(family_title(&gone, false), "Gone Sans (not installed)");
        assert_eq!(family_title(&gone, true), "Gone Sans");
        assert_eq!(family_title(&FontFamily::Compact, false), "Inter Tight");
    }
}
