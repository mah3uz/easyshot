use std::time::Duration;

use gpui::{App, SharedString, StatefulInteractiveElement};
use gpui_component::tooltip::{Tooltip, TooltipDefaults};

/// The rest on a control before its tooltip shows, the same for every control, the next one too.
pub const SHOW_DELAY: Duration = Duration::from_millis(1500);
const READ_BASE: Duration = Duration::from_secs(2);
const READ_PER_CHAR: Duration = Duration::from_millis(60);
const READ_CAP: Duration = Duration::from_secs(8);

/// How long a tooltip with this text stays up before it hides by itself.
pub fn read_time(text: &str) -> Duration {
    let chars = u32::try_from(text.chars().count()).unwrap_or(u32::MAX);
    READ_BASE
        .saturating_add(READ_PER_CHAR.saturating_mul(chars))
        .min(READ_CAP)
}

/// The only way a control gets a tooltip.
pub trait Tip: StatefulInteractiveElement + Sized {
    fn tip(self, text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        self.tooltip_show_delay(SHOW_DELAY)
            .tooltip_hide_after(read_time(&text))
            .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
    }
}

impl<E: StatefulInteractiveElement> Tip for E {}

/// Kit widgets that draw their own tooltip (the colour picker's button) cannot hide it on a timer,
/// so they get the same rest and never stay warm.
pub fn install(cx: &mut App) {
    TooltipDefaults::new()
        .with_show_delay(SHOW_DELAY)
        .with_grace_period(Duration::ZERO)
        .install(cx);
}
