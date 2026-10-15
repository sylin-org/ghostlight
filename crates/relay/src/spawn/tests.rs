//! Tests of the backoff schedule.

use super::*;

#[test]
fn the_backoff_starts_at_the_first_wait_and_doubles_to_the_cap() {
    let mut backoff = Backoff::new();
    let waits: Vec<u64> = (0..9)
        .map(|_| u64::try_from(backoff.next_wait().as_millis()).unwrap())
        .collect();
    assert_eq!(
        waits,
        [500, 1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]
    );
}

#[test]
fn a_reset_returns_to_the_first_wait() {
    let mut backoff = Backoff::new();
    for _ in 0..4 {
        backoff.next_wait();
    }
    backoff.reset();
    assert_eq!(backoff.next_wait(), BACKOFF_FIRST);
    assert_eq!(backoff.next_wait(), BACKOFF_FIRST * 2);
}
