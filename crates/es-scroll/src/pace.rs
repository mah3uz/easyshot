use std::time::Duration;

const FIRST_DELAY_MS: f64 = 10.0;
const DELAY_GROWTH: f64 = 1.5;
const MAX_DELAY_MS: f64 = 80.0;
// Moving content may never settle, and every tick waits on this.
const MAX_WAIT_MS: f64 = 200.0;
const MAX_FAILED_STEPS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleStep {
    Settled,
    Retry(Duration),
    GaveUp,
}

#[derive(Debug, Clone, Default)]
pub struct Settle {
    tries: u32,
    waited_ms: f64,
}

impl Settle {
    pub fn new() -> Self {
        Self::default()
    }

    /// `unchanged` says whether this frame matches the one before; the first frame has none.
    pub fn observe(&mut self, unchanged: bool) -> SettleStep {
        self.tries += 1;
        if unchanged && self.tries > 1 {
            return SettleStep::Settled;
        }
        if self.waited_ms >= MAX_WAIT_MS {
            return SettleStep::GaveUp;
        }
        let ms = (FIRST_DELAY_MS * DELAY_GROWTH.powi(self.tries as i32 - 1)).min(MAX_DELAY_MS);
        self.waited_ms += ms;
        SettleStep::Retry(Duration::from_secs_f64(ms / 1000.0))
    }
}

pub fn auto_scroll_step(area_height: u32, speed: u32) -> u32 {
    (area_height.saturating_mul(speed) / 8).max(1)
}

#[derive(Debug, Clone, Default)]
pub struct AutoScrollStop {
    failed: u32,
}

impl AutoScrollStop {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, appended: bool) -> bool {
        self.failed = if appended { 0 } else { self.failed + 1 };
        self.failed >= MAX_FAILED_STEPS
    }
}
