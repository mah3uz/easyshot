#![allow(clippy::unwrap_used, clippy::print_stdout)]

use es_scroll::{AutoScrollStop, Frame, Settle, SettleStep, auto_scroll_step, frame_hash};
use std::time::Duration;

#[test]
fn settle_backs_off_then_gives_up_after_twelve_tries() {
    let mut s = Settle::new();
    let mut delays = Vec::new();
    for h in 0..11u64 {
        match s.observe(h) {
            SettleStep::Retry(d) => delays.push(d.as_micros()),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        delays,
        [
            10_000, 15_000, 22_500, 33_750, 50_625, 75_937, 80_000, 80_000, 80_000, 80_000, 80_000
        ]
    );
    assert_eq!(s.observe(11), SettleStep::GaveUp);
}

#[test]
fn settle_needs_two_identical_hashes() {
    let mut s = Settle::new();
    assert_eq!(s.observe(5), SettleStep::Retry(Duration::from_millis(10)));
    assert_eq!(s.observe(6), SettleStep::Retry(Duration::from_millis(15)));
    assert_eq!(s.observe(6), SettleStep::Settled);
}

#[test]
fn hash_sees_every_byte() {
    let a = Frame::new(3, 3, vec![0; 36]).unwrap();
    let mut b = a.clone();
    b.data[35] = 1;
    assert_ne!(frame_hash(&a), frame_hash(&b));
    assert_eq!(frame_hash(&a), frame_hash(&a.clone()));
}

#[test]
fn auto_scroll_step_scales_with_area_and_speed() {
    assert_eq!(auto_scroll_step(800, 3), 300);
    assert_eq!(auto_scroll_step(300, 1), 37);
    assert_eq!(auto_scroll_step(4, 1), 1);
    assert_eq!(auto_scroll_step(0, 4), 1);
}

#[test]
fn auto_scroll_stops_after_eight_failed_steps_in_a_row() {
    let mut s = AutoScrollStop::new();
    for _ in 0..7 {
        assert!(!s.record(false));
    }
    assert!(!s.record(true));
    for _ in 0..7 {
        assert!(!s.record(false));
    }
    assert!(s.record(false));
}

#[test]
#[ignore]
fn shift_timing_1080p() {
    use es_scroll::{Exclusions, shift};
    let (w, h) = (1920usize, 1080usize);
    let mut x = 88172645463325252u64;
    let mut page = Vec::with_capacity(w * (h + 300) * 4);
    for _ in 0..w * (h + 300) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        page.extend_from_slice(&[(x >> 8) as u8, (x >> 16) as u8, (x >> 24) as u8, 255]);
    }
    let cut = |off: usize| Frame::new(w, h, page[off * w * 4..(off + h) * w * 4].to_vec()).unwrap();
    let (a, b) = (cut(0), cut(300));
    let t = std::time::Instant::now();
    for _ in 0..20 {
        assert_eq!(shift(&a, &b, Exclusions::default()), Some(300));
    }
    println!("shift 1920x1080: {:?} per call", t.elapsed() / 20);
}
