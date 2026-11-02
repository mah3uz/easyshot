use std::time::Duration;

const FIRST_DELAY_MS: f64 = 10.0;
const DELAY_GROWTH: f64 = 1.5;
const MAX_DELAY_MS: f64 = 80.0;
const MAX_TRIES: u32 = 12;
const MAX_FAILED_STEPS: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleStep {
    Settled,
    Retry(Duration),
    GaveUp,
}

#[derive(Debug, Clone, Default)]
pub struct Settle {
    last: Option<u64>,
    tries: u32,
}

impl Settle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, hash: u64) -> SettleStep {
        self.tries += 1;
        if self.last == Some(hash) {
            return SettleStep::Settled;
        }
        self.last = Some(hash);
        if self.tries >= MAX_TRIES {
            return SettleStep::GaveUp;
        }
        let ms = (FIRST_DELAY_MS * DELAY_GROWTH.powi(self.tries as i32 - 1)).min(MAX_DELAY_MS);
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
