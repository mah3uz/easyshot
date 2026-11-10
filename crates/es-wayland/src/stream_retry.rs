use wayland_client::WEnum;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::FailureReason;

const MAX_RETRIES: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AfterFailure {
    RequestNow,
    RetryLater,
    Stopped,
    GiveUp,
}

/// Counts failed frames in a row that brought no change of constraints. Hyprland fails every
/// frame of a window while it resizes or changes monitor, each with new constraints just before
/// or after the failure, so only failures without such a change mean the stream cannot go on.
#[derive(Debug, Default)]
pub(crate) struct Retries {
    failures: u32,
}

impl Retries {
    pub fn ready(&mut self) {
        self.failures = 0;
    }

    pub fn constraints(&mut self, changed: bool) {
        if changed {
            self.failures = 0;
        }
    }

    pub fn failed(
        &mut self,
        reason: WEnum<FailureReason>,
        newer_constraints: bool,
    ) -> AfterFailure {
        match reason {
            WEnum::Value(FailureReason::Stopped) => AfterFailure::Stopped,
            WEnum::Value(FailureReason::BufferConstraints) if newer_constraints => {
                AfterFailure::RequestNow
            }
            _ => {
                self.failures += 1;
                if self.failures > MAX_RETRIES {
                    AfterFailure::GiveUp
                } else {
                    AfterFailure::RetryLater
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONSTRAINTS: WEnum<FailureReason> = WEnum::Value(FailureReason::BufferConstraints);

    #[test]
    fn a_window_resizing_for_many_frames_does_not_end_the_stream() {
        let mut retries = Retries::default();
        // Hyprland's order as a window changed monitor: new constraints just before or just after each failure.
        for i in 0..200 {
            if i % 2 == 0 {
                retries.constraints(true);
                assert_eq!(retries.failed(CONSTRAINTS, true), AfterFailure::RequestNow);
            } else {
                assert_eq!(retries.failed(CONSTRAINTS, false), AfterFailure::RetryLater);
                retries.constraints(true);
            }
        }
    }

    #[test]
    fn failures_without_changed_constraints_end_the_stream_after_the_retries() {
        let mut retries = Retries::default();
        for _ in 0..MAX_RETRIES {
            assert_eq!(retries.failed(CONSTRAINTS, false), AfterFailure::RetryLater);
            retries.constraints(false);
        }
        assert_eq!(retries.failed(CONSTRAINTS, false), AfterFailure::GiveUp);
    }

    #[test]
    fn a_ready_frame_restarts_the_count_and_stopped_ends_at_once() {
        let mut retries = Retries::default();
        let unknown = WEnum::Value(FailureReason::Unknown);
        for _ in 0..MAX_RETRIES {
            assert_eq!(retries.failed(unknown, false), AfterFailure::RetryLater);
        }
        retries.ready();
        assert_eq!(retries.failed(unknown, false), AfterFailure::RetryLater);
        assert_eq!(
            retries.failed(WEnum::Value(FailureReason::Stopped), false),
            AfterFailure::Stopped
        );
    }
}
