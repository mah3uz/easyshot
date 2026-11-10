use std::ops::Range;

const SECOND: u128 = 1_000_000_000;
pub(crate) const RATE: u64 = 48_000;

/// The sample at `ns` of running time.
pub(crate) fn sample_at(ns: u64) -> u64 {
    ((u128::from(ns) * u128::from(RATE) + SECOND / 2) / SECOND) as u64
}

pub(crate) fn time_of(sample: u64) -> u64 {
    ((u128::from(sample) * SECOND + u128::from(RATE) / 2) / u128::from(RATE)) as u64
}

/// Which samples of running time go into the file: from the first video frame on, without the
/// paused spans, up to the stop. Kept samples are renumbered so the file has no gaps.
#[derive(Debug, Default)]
pub(crate) struct Timeline {
    start: Option<u64>,
    pauses: Vec<(u64, u64)>,
    /// Where a pause that has not ended, or the stop, began.
    open: Option<u64>,
    stopped: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Cut {
    Drop,
    /// The recorded samples of the buffer, which play from file sample `at` with no gap between them.
    Keep {
        pieces: Vec<Range<u64>>,
        at: u64,
    },
    /// The buffer starts after the stop.
    End,
}

impl Timeline {
    pub fn start(&mut self, running: u64) {
        self.start = Some(sample_at(running));
    }

    pub fn pause(&mut self, running: u64) {
        if self.open.is_none() {
            self.open = Some(sample_at(running));
        }
    }

    pub fn resume(&mut self, running: u64) {
        if self.stopped {
            return;
        }
        if let Some(p) = self.open.take() {
            self.pauses.push((p, sample_at(running).max(p)));
        }
    }

    /// While paused, the stop is where the pause began.
    pub fn stop(&mut self, running: u64) {
        self.pause(running);
        self.stopped = true;
    }

    /// What to record of `len` samples starting at running-time sample `at`.
    pub fn cut(&self, at: u64, len: u64) -> Cut {
        let Some(start) = self.start else {
            return Cut::Drop;
        };
        if self.stopped && self.open.is_some_and(|end| at >= end) {
            return Cut::End;
        }
        let end = at + len;
        let (mut lo, mut skipped) = (start, start);
        let mut pieces = Vec::new();
        let mut first = None;
        for pause in self.pauses.iter().map(Some).chain([None]) {
            let hi = match pause {
                Some(&(p, _)) => p,
                None => self.open.unwrap_or(u64::MAX),
            };
            let (a, b) = (at.max(lo), end.min(hi));
            if a < b {
                first.get_or_insert(a - skipped);
                pieces.push(a..b);
            }
            if let Some(&(p, q)) = pause {
                skipped += q - p;
                lo = q;
            }
        }
        match first {
            Some(at) => Cut::Keep { pieces, at },
            None => Cut::Drop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn keep(pieces: &[(u64, u64)], at: u64) -> Cut {
        Cut::Keep {
            pieces: pieces.iter().map(|&(a, b)| a..b).collect(),
            at,
        }
    }

    #[test]
    fn sound_before_the_first_frame_is_dropped_and_the_file_starts_at_it() {
        let mut t = Timeline::default();
        assert_eq!(
            t.cut(0, 480),
            Cut::Drop,
            "nothing before the start is known"
        );
        t.start(500 * MS);
        assert_eq!(sample_at(500 * MS), 24_000);
        assert_eq!(t.cut(23_000, 480), Cut::Drop);
        assert_eq!(t.cut(23_800, 480), keep(&[(24_000, 24_280)], 0));
        assert_eq!(t.cut(24_280, 480), keep(&[(24_280, 24_760)], 280));
    }

    #[test]
    fn a_pause_is_cut_to_the_sample_and_the_sound_after_it_follows_without_a_gap() {
        let mut t = Timeline::default();
        t.start(0);
        t.pause(1_000 * MS);
        assert_eq!(t.cut(47_800, 480), keep(&[(47_800, 48_000)], 47_800));
        assert_eq!(t.cut(48_280, 480), Cut::Drop, "paused");
        t.resume(3_000 * MS);
        assert_eq!(
            t.cut(143_800, 480),
            keep(&[(144_000, 144_280)], 48_000),
            "file time goes on where the pause began"
        );
        t.pause(4_000 * MS);
        t.resume(5_000 * MS);
        assert_eq!(
            t.cut(191_900, 48_200),
            keep(&[(191_900, 192_000), (240_000, 240_100)], 95_900),
            "one buffer across a whole pause keeps both ends, back to back"
        );
    }

    #[test]
    fn after_the_stop_sound_ends_and_a_stop_while_paused_ends_at_the_pause() {
        let mut t = Timeline::default();
        t.start(0);
        t.stop(1_000 * MS);
        assert_eq!(t.cut(47_800, 480), keep(&[(47_800, 48_000)], 47_800));
        assert_eq!(t.cut(48_000, 480), Cut::End);
        let mut t = Timeline::default();
        t.start(0);
        t.pause(1_000 * MS);
        t.stop(2_000 * MS);
        t.resume(3_000 * MS);
        assert_eq!(t.cut(48_000, 480), Cut::End);
    }

    #[test]
    fn samples_and_times_round_trip() {
        for s in [0, 1, 479, 48_000, 123_456_789] {
            assert_eq!(sample_at(time_of(s)), s);
        }
    }
}
