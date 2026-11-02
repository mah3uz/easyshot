use std::rc::Rc;

use es_anno::crop::{AspectPreset, ImageOp};
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton},
    icons::IconName,
    theme::tokens,
    tokens::{TextStyle, Typeset},
};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled,
    div, px,
};

use super::EditorWindow;
use crate::session::crop::CropModel;
use crate::strings::fl;
use crate::toolbar::menus::{choice_menu, menu_trigger};
use crate::toolbar::{FlipAxis, Rotation, ToolRowEvent};

const ASPECT_WIDTH: gpui::Pixels = px(110.);

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

pub fn image_op(event: ToolRowEvent) -> Option<ImageOp> {
    match event {
        ToolRowEvent::Rotate(Rotation::Left) => Some(ImageOp::RotateLeft),
        ToolRowEvent::Rotate(Rotation::Right) => Some(ImageOp::RotateRight),
        ToolRowEvent::Flip(FlipAxis::Horizontal) => Some(ImageOp::FlipHorizontal),
        ToolRowEvent::Flip(FlipAxis::Vertical) => Some(ImageOp::FlipVertical),
        _ => None,
    }
}

impl EditorWindow {
    pub fn is_cropping(&self, cx: &App) -> bool {
        self.session()
            .is_some_and(|s| s.crop.read(cx).is_cropping())
    }

    pub(crate) fn with_crop(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut CropModel, &mut Context<CropModel>),
    ) {
        if let Some(s) = self.session() {
            s.crop.update(cx, f);
        }
    }

    /// Aspect menu, Reset, Cancel and Crop, standing in for Save and Export.
    pub(super) fn crop_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let aspect = self
            .session()
            .and_then(|s| s.crop.read(cx).state().map(|c| c.aspect()))
            .unwrap_or_default();
        let trigger = menu_trigger(
            "crop-aspect",
            fl!("editor-crop-aspect-help"),
            Some(IconName::Ratio),
            Some(aspect_title(aspect).into()),
            false,
            cx,
        )
        .w(ASPECT_WIDTH);
        let choices = AspectPreset::ALL
            .iter()
            .map(|&a| (a, aspect_title(a), None))
            .collect();
        let this = cx.entity().downgrade();
        let pick = Rc::new(move |a: AspectPreset, _: &mut gpui::Window, cx: &mut App| {
            let _ = this.update(cx, |w, cx| w.with_crop(cx, |c, cx| c.set_aspect(a, cx)));
        });
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(choice_menu(trigger, choices, Some(aspect), pick))
            .child(
                EditorButton::new("crop-reset")
                    .label(fl!("editor-crop-reset"))
                    .tooltip(fl!("editor-crop-reset-help"))
                    .on_click(cx.listener(|w, _, _, cx| w.with_crop(cx, |c, cx| c.reset(cx)))),
            )
            .child(
                EditorButton::new("crop-cancel")
                    .label(fl!("editor-crop-cancel"))
                    .on_click(cx.listener(|w, _, _, cx| w.with_crop(cx, |c, cx| c.cancel(cx)))),
            )
            .child(
                EditorButton::new("crop-apply")
                    .kind(ButtonKind::Prominent)
                    .label(fl!("editor-crop-apply"))
                    .on_click(cx.listener(|w, _, _, cx| {
                        w.with_crop(cx, |c, cx| {
                            c.apply(cx);
                        })
                    })),
            )
            .into_any_element()
    }

    /// The output size while cropping, where Copy otherwise sits.
    pub(crate) fn crop_badge(&self, cx: &App) -> Option<AnyElement> {
        let (w, h) = self.session()?.crop.read(cx).state()?.output_size();
        let t = tokens(cx);
        let text = fl!("editor-crop-size", width = w, height = h);
        Some(
            div()
                .id("crop-size")
                .flex_none()
                .px(px(12.))
                .py(px(7.))
                .rounded(px(8.))
                .border(t.metrics.hairline)
                .border_color(t.border)
                .text_color(t.text_secondary)
                .typeset(TextStyle {
                    tabular: true,
                    weight: FontWeight::MEDIUM,
                    ..t.type_scale.footer
                })
                .accessible(Role::Status, text.clone())
                .child(text)
                .into_any_element(),
        )
    }
}
