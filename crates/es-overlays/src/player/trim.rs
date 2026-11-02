pub const MIN_TRIM_SECS: f64 = 0.12;

/// The kept span of a video, in seconds; it never shrinks below `MIN_TRIM_SECS`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    duration: f64,
    start: f64,
    end: f64,
}

impl Trim {
    pub fn new(duration: f64) -> Self {
        let duration = duration.max(0.);
        Trim {
            duration,
            start: 0.,
            end: duration,
        }
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn start(&self) -> f64 {
        self.start
    }

    pub fn end(&self) -> f64 {
        self.end
    }

    pub fn kept(&self) -> f64 {
        self.end - self.start
    }

    pub fn is_whole(&self) -> bool {
        self.start <= 0. && self.end >= self.duration
    }

    pub fn set_start(&mut self, secs: f64) {
        self.start = secs.clamp(0., (self.end - MIN_TRIM_SECS).max(0.));
    }

    pub fn set_end(&mut self, secs: f64) {
        self.end = secs.clamp(
            (self.start + MIN_TRIM_SECS).min(self.duration),
            self.duration,
        );
    }
}

/// `mm:ss.cc`, the hundredths kept so a trim of a fraction of a second still reads differently.
pub fn clock(secs: f64) -> String {
    let hundredths = (secs.max(0.) * 100.).round() as u64;
    format!(
        "{:02}:{:02}.{:02}",
        hundredths / 6000,
        hundredths / 100 % 60,
        hundredths % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_stop_short_of_each_other() {
        let mut t = Trim::new(5.);
        t.set_start(4.99);
        assert!((t.start() - (5. - MIN_TRIM_SECS)).abs() < 1e-9);
        t.set_end(0.);
        assert!((t.kept() - MIN_TRIM_SECS).abs() < 1e-9);
        t.set_start(-3.);
        t.set_end(9.);
        assert!(t.is_whole());
    }

    #[test]
    fn a_clip_shorter_than_the_minimum_keeps_all_of_itself() {
        let mut t = Trim::new(0.05);
        t.set_start(0.04);
        t.set_end(0.);
        assert_eq!((t.start(), t.end()), (0., 0.05));
    }

    #[test]
    fn clock_reads_minutes_seconds_and_hundredths() {
        assert_eq!(clock(0.), "00:00.00");
        assert_eq!(clock(2.4), "00:02.40");
        assert_eq!(clock(61.999), "01:02.00");
    }
}
