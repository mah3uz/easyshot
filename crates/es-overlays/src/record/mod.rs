pub mod bar;
pub mod control;
pub mod options;
pub mod placement;
pub mod session;

pub use bar::{BarEvent, CaptureBar};
pub use control::{Confirm, ControlBar, ControlEvent};
pub use options::{OptionsEvent, RecordingOptions, RecordingRequest, RecordingTarget};
pub use placement::{Placement, Recorded, control_bar, highlight_strips};
pub use session::{Phase, Stopwatch, TrayItem, TrayMenu, format_elapsed, tray_notice};

use crate::capture::paint::black;
use gpui::{Context, Div, IntoElement, ParentElement as _, Render, Styled as _, Window, div, px};

pub(crate) const CELL: f32 = 0.62;
const DIM: f32 = 0.3;

/// Each character in a fixed cell, so a running time never shifts sideways.
pub(crate) fn digits(text: &str, size: f32, cell: f32) -> Div {
    text.chars()
        .fold(div().flex().flex_none().text_size(px(size)), |row, c| {
            let width = if c.is_ascii_digit() {
                cell
            } else {
                cell * 0.55
            };
            row.child(
                div()
                    .flex()
                    .justify_center()
                    .w(px(size * width))
                    .child(c.to_string()),
            )
        })
}

/// One strip of the dim around a recorded area.
pub struct Dim;

impl Render for Dim {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(black(DIM))
    }
}
