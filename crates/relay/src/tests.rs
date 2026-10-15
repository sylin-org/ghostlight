//! Checks that the crate version major is the relay revision.

use super::RELAY;

/// The crate version major is the relay revision, so a published edge names its revision.
#[test]
fn crate_version_major_equals_relay_revision() {
    let major: u16 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    assert_eq!(major, RELAY);
}
