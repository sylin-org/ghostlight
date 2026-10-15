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

#[test]
fn the_service_image_and_the_edge_are_two_files_with_the_platform_suffix() {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    assert_eq!(EDGE_EXECUTABLE_NAME, format!("ghostlight{suffix}"));
    assert_eq!(
        SERVICE_EXECUTABLE_NAME,
        format!("ghostlight-service{suffix}")
    );
}

mod verbatim {
    use std::path::{Path, PathBuf};

    use crate::home::{canonical, without_verbatim_prefix};

    fn stripped(text: &str) -> PathBuf {
        without_verbatim_prefix(Path::new(text))
    }

    #[test]
    fn a_verbatim_drive_path_loses_its_prefix() {
        assert_eq!(
            stripped(r"\\?\C:\Users\person\.ghostlight"),
            Path::new(r"C:\Users\person\.ghostlight")
        );
        assert_eq!(stripped(r"\\?\d:\x"), Path::new(r"d:\x"));
        assert_eq!(stripped(r"\\?\C:"), Path::new("C:"));
    }

    #[test]
    fn a_verbatim_share_path_becomes_a_share_path() {
        assert_eq!(
            stripped(r"\\?\UNC\server\share\x"),
            Path::new(r"\\server\share\x")
        );
        assert_eq!(
            stripped(r"\\?\unc\server\share"),
            Path::new(r"\\server\share")
        );
    }

    #[test]
    fn other_verbatim_forms_are_kept() {
        for text in [
            r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\x",
            r"\\?\GLOBALROOT\Device\x",
            r"\\?\C:x",
            r"\\?\UNCx\y",
            r"\\?\UNC",
            r"\\?\",
            r"\\.\C:\x",
        ] {
            assert_eq!(stripped(text), Path::new(text), "{text}");
        }
    }

    #[test]
    fn a_plain_path_is_kept() {
        for text in [r"C:\Users\person", r"\\server\share\x", "/home/person", ""] {
            assert_eq!(stripped(text), Path::new(text), "{text}");
        }
    }

    #[test]
    fn the_canonical_form_of_a_directory_is_stable() {
        let first = canonical(&std::env::temp_dir()).expect("canonical form");
        let second = canonical(&first).expect("canonical form of the canonical form");
        assert_eq!(first, second);
        assert!(first.is_absolute());
        assert!(first.is_dir());
        assert!(!first.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn a_missing_path_has_no_canonical_form() {
        let missing = std::env::temp_dir().join("ghostlight-canonical-missing");
        assert_eq!(
            canonical(&missing).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
