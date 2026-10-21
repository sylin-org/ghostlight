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

/// ADR-0197 Decision 10: there is one home per user. It is the user's home joined with the
/// Ghostlight directory, or the test override, which names the home itself. A home that is
/// relative, empty or missing is an error rather than a guess, so no default root ever falls
/// back to the working directory.
#[test]
fn the_home_is_the_users_ghostlight_directory_or_the_override_and_never_a_guess() {
    assert_eq!(
        resolve(None, os(ROOT)).unwrap(),
        Path::new(ROOT).join(HOME_DIRECTORY_NAME)
    );
    assert_eq!(resolve(os(ROOT), os("ignored")).unwrap(), Path::new(ROOT));
    for (test_home, user_home) in [
        (None, os("relative")),
        (None, os("")),
        (os("relative"), os(ROOT)),
    ] {
        assert_eq!(
            resolve(test_home.clone(), user_home.clone())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput,
            "{test_home:?} {user_home:?}"
        );
    }
    assert_eq!(
        resolve(None, None).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
}
