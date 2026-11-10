use std::rc::Rc;

use es_anno::crop::AspectPreset;
use es_ui::{
    components::{ButtonKind, EditorButton},
    icons::IconName,
    theme::tokens,
};
use gpui::{App, Context, Div, ParentElement, Styled, Window, div, px};

use super::{Control, ToolOptions, section};
use crate::strings::fl;
use crate::toolbar::{
    menus::{choice_menu, menu_trigger},
    track,
};

pub fn aspect_title(aspect: AspectPreset) -> String {
    match aspect {
        AspectPreset::Freeform => fl!("editor-crop-aspect-freeform"),
        AspectPreset::Original => fl!("editor-crop-aspect-original"),
        AspectPreset::Square => fl!("editor-aspect-1-1"),
        AspectPreset::Landscape16x9 => fl!("editor-aspect-16-9"),
        AspectPreset::Portrait9x16 => fl!("editor-aspect-9-16"),
        AspectPreset::Landscape4x3 => fl!("editor-aspect-4-3"),
        AspectPreset::Landscape3x2 => fl!("editor-aspect-3-2"),
    }
}

/// The ratio menu, then Reset, Cancel and Apply, acting on the crop in progress.
pub(super) fn content(page: &ToolOptions, cx: &mut Context<ToolOptions>) -> Div {
    let t = tokens(cx).clone();
    let crop = page.crop.clone();
    let aspect = crop
        .read(cx)
        .state()
        .map(|c| c.aspect())
        .unwrap_or_default();
    let trigger = menu_trigger(
        "crop-aspect",
        fl!("editor-crop-aspect-help"),
        Some(IconName::Ratio),
        Some(aspect_title(aspect).into()),
        false,
        cx,
    )
    .w_full()
    .bg(t.track);
    let choices = AspectPreset::ALL
        .iter()
        .map(|&a| (a, aspect_title(a), None))
        .collect();
    let pick = {
        let crop = crop.clone();
        Rc::new(move |a: AspectPreset, _: &mut Window, cx: &mut App| {
            crop.update(cx, |c, cx| c.set_aspect(a, cx));
        })
    };
    let (reset, cancel) = (crop.clone(), crop.clone());
    let buttons = div()
        .flex()
        .gap(px(8.))
        .child(track(
            &page.drawn,
            Control::CropReset,
            EditorButton::new("crop-reset")
                .label(fl!("editor-crop-reset"))
                .tip(fl!("editor-crop-reset-help"))
                .on_click(move |_, _, cx| reset.update(cx, |c, cx| c.reset(cx))),
        ))
        .child(div().flex_1())
        .child(track(
            &page.drawn,
            Control::CropCancel,
            EditorButton::new("crop-cancel")
                .label(fl!("editor-crop-cancel"))
                .on_click(move |_, _, cx| cancel.update(cx, |c, cx| c.cancel(cx))),
        ))
        .child(track(
            &page.drawn,
            Control::CropApply,
            EditorButton::new("crop-apply")
                .kind(ButtonKind::Prominent)
                .label(fl!("editor-crop-apply"))
                .on_click(move |_, _, cx| {
                    crop.update(cx, |c, cx| {
                        c.apply(cx);
                    })
                }),
        ));
    div()
        .flex()
        .flex_col()
        .child(section(
            fl!("editor-crop-aspect-help"),
            track(
                &page.drawn,
                Control::CropAspect,
                choice_menu(trigger, choices, Some(aspect), pick),
            )
            .w_full(),
            &t,
        ))
        .child(buttons)
}
