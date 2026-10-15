//! Tests of how the home is chosen.

use std::path::Path;

use super::*;

#[cfg(windows)]
const ROOT: &str = "C:\\Users\\person";
#[cfg(not(windows))]
const ROOT: &str = "/home/person";

fn os(value: &str) -> Option<OsString> {
    Some(OsString::from(value))
}

#[test]
fn the_home_is_the_user_home_joined_with_the_ghostlight_directory() {
    let home = resolve(None, os(ROOT)).unwrap();
    assert_eq!(home, Path::new(ROOT).join(".ghostlight"));
}

#[test]
fn the_test_override_names_the_home_itself() {
    let home = resolve(os(ROOT), os("ignored")).unwrap();
    assert_eq!(home, Path::new(ROOT));
}

#[test]
fn a_relative_home_is_refused() {
    assert_eq!(
        resolve(None, os("relative")).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        resolve(os("relative"), os(ROOT)).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn a_missing_user_home_is_not_found() {
    assert_eq!(
        resolve(None, None).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
}
