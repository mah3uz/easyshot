use es_anno::StylePatch;
use es_model::{StylePreset, Tool};
use es_ui::components::EditorButton;
use gpui::{Context, Div};

use super::{Control, ToolRow, style_bar::Facets, track};
use crate::strings::fl;

impl ToolRow {
    /// Puts the options the panel shows for `facets` back to the defaults, as one undo step on a selection.
    pub(super) fn reset_style(&mut self, facets: Facets, cx: &mut Context<Self>) {
        let d = StylePreset::default();
        let mut patches = Vec::new();
        if facets.colour {
            patches.push(StylePatch::Swatch(d.swatch));
        }
        if facets.stroke {
            patches.push(StylePatch::StrokeWidth(d.stroke_width));
        }
        if facets.strength {
            patches.push(StylePatch::RedactionDensity(d.redaction_density));
        }
        if facets.text {
            patches.extend([
                StylePatch::FontFamily(d.text_font_family),
                StylePatch::FontSize(d.text_font_size),
                StylePatch::Bold(d.text_bold),
                StylePatch::Italic(d.text_italic),
                StylePatch::Underline(d.text_underline),
                StylePatch::Align(d.text_align),
                StylePatch::TextFill(d.text_fill),
                StylePatch::TextBorder(d.text_border),
            ]);
        }
        if facets.heads {
            patches.extend([
                StylePatch::ArrowheadStart(d.arrowhead_start),
                StylePatch::ArrowheadEnd(d.arrowhead_end),
            ]);
        }
        if facets.dash {
            patches.push(StylePatch::Dash(d.dash));
        }
        let gesture = self.fresh_gesture();
        for patch in patches {
            self.apply_style(patch, Some(gesture), cx);
        }
        if facets.mode && self.doc.read(cx).editor().highlighter_straight() {
            self.pick_tool(Tool::Highlighter, cx);
        }
    }
}

pub(super) fn button(row: &ToolRow, facets: Facets, cx: &mut Context<ToolRow>) -> Div {
    let weak = cx.entity().downgrade();
    track(
        &row.drawn,
        Control::ResetStyle,
        EditorButton::new("reset-style")
            .label(fl!("editor-reset-style"))
            .tooltip(fl!("editor-reset-style-help"))
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |r, cx| r.reset_style(facets, cx));
            }),
    )
}
