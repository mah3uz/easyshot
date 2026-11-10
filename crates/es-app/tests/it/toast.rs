use es_app::toast::{Countdown, ERROR_DURATION, TOAST_DURATION, Toast, UNDO_DURATION};
use std::time::Duration;

const SECOND: Duration = Duration::from_secs(1);

#[test]
fn an_error_lasts_longer_than_a_notice_and_less_than_an_undo() {
    assert_eq!(
        Toast::error("toast-open-unsupported").duration,
        ERROR_DURATION
    );
    assert_eq!(ERROR_DURATION, Duration::from_secs(5));
    assert_eq!(TOAST_DURATION, Duration::from_millis(2500));
    assert!(ERROR_DURATION < UNDO_DURATION);
}

#[test]
fn hovering_holds_the_time_and_leaving_resumes_with_what_was_left() {
    let mut countdown = Countdown::new(ERROR_DURATION);
    assert!(!countdown.advance(2 * SECOND, false));
    assert!(
        !countdown.advance(60 * SECOND, true),
        "still shown while hovered"
    );
    assert_eq!(countdown.remaining(), 3 * SECOND);
    assert!(!countdown.advance(2 * SECOND, false));
    assert!(
        countdown.advance(SECOND, false),
        "closes after the remaining time"
    );
}
