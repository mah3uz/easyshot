use es_anno::crop::ImageOp;
use es_ui::{
    a11y::{Accessible, Role},
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
use crate::toolbar::{FlipAxis, Rotation, ToolRowEvent};

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
