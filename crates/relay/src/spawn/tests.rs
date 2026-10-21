//! Tests of the backoff schedule.

use super::*;

/// Relay-protocol-4 section 10 and ACCEPTANCE C13: an edge that finds no service tries again on
/// the backoff schedule. The first wait is the vocabulary's first wait, each wait doubles the one
/// before up to the cap, and the cap holds, so a missing service costs at most one look and one
/// start each cap's worth of time. Once a service answers, the schedule starts over.
#[test]
fn the_backoff_doubles_from_its_first_wait_to_its_cap_and_starts_over() {
    let mut backoff = Backoff::new();
    let waits: Vec<Duration> = (0..9).map(|_| backoff.next_wait()).collect();
    assert_eq!(waits[0], BACKOFF_FIRST);
    for pair in waits.windows(2) {
        assert_eq!(pair[1], (pair[0] * 2).min(BACKOFF_MAX), "{waits:?}");
    }
    assert_eq!(
        waits.last(),
        Some(&BACKOFF_MAX),
        "the cap is reached and held"
    );

    backoff.reset();
    assert_eq!(backoff.next_wait(), BACKOFF_FIRST);
    assert_eq!(backoff.next_wait(), BACKOFF_FIRST * 2);
}
