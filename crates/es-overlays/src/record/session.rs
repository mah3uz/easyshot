use crate::capture::strings::fl;
use es_protocol::Route;
use es_record::Clock;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Recording,
    Paused,
    Finishing,
}

impl Phase {
    pub fn busy(self) -> bool {
        matches!(self, Phase::Starting | Phase::Finishing)
    }
}

/// Elapsed recording time: wall time since the start minus every pause.
pub struct Stopwatch {
    clock: Arc<dyn Clock>,
    started: Option<Duration>,
    paused_since: Option<Duration>,
    paused_total: Duration,
}

impl Stopwatch {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Stopwatch {
            clock,
            started: None,
            paused_since: None,
            paused_total: Duration::ZERO,
        }
    }

    pub fn start(&mut self) {
        self.started = Some(self.clock.now());
        self.paused_since = None;
        self.paused_total = Duration::ZERO;
    }

    pub fn pause(&mut self) {
        if self.started.is_some() && self.paused_since.is_none() {
            self.paused_since = Some(self.clock.now());
        }
    }

    pub fn resume(&mut self) {
        if let Some(since) = self.paused_since.take() {
            self.paused_total += self.clock.now().saturating_sub(since);
        }
    }

    pub fn elapsed(&self) -> Duration {
        let Some(started) = self.started else {
            return Duration::ZERO;
        };
        let now = self.clock.now();
        let paused = self.paused_total + self.paused_since.map_or(Duration::ZERO, |s| now - s);
        now.saturating_sub(started).saturating_sub(paused)
    }
}

/// `mm:ss`, or `h:mm:ss` past the hour.
pub fn format_elapsed(d: Duration) -> String {
    let s = d.as_secs();
    match s / 3600 {
        0 => format!("{:02}:{:02}", s / 60, s % 60),
        h => format!("{h}:{:02}:{:02}", s / 60 % 60, s % 60),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayItem {
    pub route: Route,
    pub label: String,
    pub enabled: bool,
}

/// What the tray shows while a recording has no bar on screen: the red dot, the time and the
/// controls, in that order at the top of its menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenu {
    pub elapsed: String,
    pub phase: Phase,
    pub items: Vec<TrayItem>,
}

impl TrayMenu {
    pub fn new(phase: Phase, elapsed: Duration) -> Self {
        let enabled = !phase.busy();
        let pause = if phase == Phase::Paused {
            fl!("rec-bar-resume-help")
        } else {
            fl!("rec-bar-pause-help")
        };
        let items = [
            (Route::RecordStop, fl!("rec-bar-stop")),
            (Route::RecordPause, pause),
            (Route::RecordRestart, fl!("rec-bar-start-over-help")),
            (Route::RecordDiscard, fl!("rec-bar-discard-help")),
        ]
        .into_iter()
        .map(|(route, label)| TrayItem {
            route,
            label,
            enabled,
        })
        .collect();
        TrayMenu {
            elapsed: format_elapsed(elapsed),
            phase,
            items,
        }
    }
}

/// Shown for a few seconds when a full-display recording starts with nowhere to put the bar.
pub fn tray_notice(shortcut: &str) -> String {
    fl!("rec-tray-notice", shortcut = shortcut)
}
