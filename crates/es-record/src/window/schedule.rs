const SECOND: u128 = 1_000_000_000;

/// Constant-rate ticks on a clock in nanoseconds. Tick 0 is shown from `start`; a pause cuts its
/// span out of the timeline, so file time stays continuous across it.
#[derive(Debug, Clone)]
pub(crate) struct Schedule {
    start: u64,
    fps: u32,
    shift: u64,
    paused_at: Option<u64>,
    next: u64,
}

impl Schedule {
    /// Tick 0 has already been pushed (the pre-roll frame).
    pub fn new(start: u64, fps: u32) -> Self {
        Schedule {
            start,
            fps: fps.max(1),
            shift: 0,
            paused_at: None,
            next: 1,
        }
    }

    /// File time of tick `n`, exact for any `n` instead of accumulating a rounded period.
    pub fn pts(&self, n: u64) -> u64 {
        (u128::from(n) * SECOND / u128::from(self.fps)) as u64
    }

    pub fn duration(&self, n: u64) -> u64 {
        self.pts(n + 1) - self.pts(n)
    }

    pub fn next(&self) -> u64 {
        self.next
    }

    pub fn deadline(&self) -> u64 {
        self.start + self.shift + self.pts(self.next)
    }

    pub fn paused(&self) -> bool {
        self.paused_at.is_some()
    }

    pub fn pause(&mut self, now: u64) {
        if self.paused_at.is_none() {
            self.paused_at = Some(now);
        }
    }

    pub fn resume(&mut self, now: u64) {
        if let Some(at) = self.paused_at.take() {
            self.shift += now.saturating_sub(at);
        }
    }

    /// Moves past the tick just pushed. When the push ran past later deadlines, jumps to the
    /// newest tick that is due and returns how many were skipped.
    pub fn advance(&mut self, now: u64) -> u64 {
        self.next += 1;
        let elapsed = now.saturating_sub(self.start + self.shift);
        let due = (u128::from(elapsed) * u128::from(self.fps) / SECOND) as u64;
        if due > self.next {
            let skipped = due - self.next;
            self.next = due;
            skipped
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn ticks_follow_the_clock_and_file_time_has_no_rounding_drift() {
        let mut s = Schedule::new(5_000 * MS, 30);
        assert_eq!(s.deadline(), 5_000 * MS + 33_333_333);
        for _ in 0..299 {
            let at = s.deadline();
            assert_eq!(s.advance(at), 0);
        }
        assert_eq!(s.next(), 300);
        assert_eq!(s.pts(300), 10_000 * MS);
        assert_eq!(s.deadline(), 15_000 * MS);
        let total: u64 = (0..300).map(|n| s.duration(n)).sum();
        assert_eq!(total, 10_000 * MS, "durations tile the file with no gap");
    }

    #[test]
    fn a_pause_is_cut_out_of_file_time() {
        let mut s = Schedule::new(0, 10);
        let at = s.deadline();
        s.advance(at);
        assert_eq!(s.deadline(), 200 * MS);
        s.pause(150 * MS);
        assert!(s.paused());
        s.resume(2_150 * MS);
        assert!(!s.paused());
        assert_eq!(s.deadline(), 2_200 * MS, "the tick keeps its phase");
        assert_eq!(
            s.advance(2_200 * MS),
            0,
            "nothing is skipped over the pause"
        );
        assert_eq!(
            s.pts(s.next()),
            300 * MS,
            "file time continues where it stopped"
        );
    }

    #[test]
    fn a_push_that_blocks_past_later_deadlines_skips_to_the_newest_due_tick() {
        let mut s = Schedule::new(0, 10);
        assert_eq!(s.advance(450 * MS), 2, "ticks 2 and 3 are skipped");
        assert_eq!(s.next(), 4);
        assert!(
            s.deadline() <= 450 * MS,
            "the newest due tick goes out at once"
        );
        assert_eq!(s.advance(450 * MS), 0);
        assert_eq!(s.deadline(), 500 * MS);
    }
}
