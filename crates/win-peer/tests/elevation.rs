//! The process token's elevation can be read on every platform.

/// Whether the token is elevated depends on how the tests were started; reading it must not fail.
#[test]
fn the_elevation_of_this_process_can_be_read() {
    assert!(ghostlight_win_peer::is_elevated().is_ok());
}
