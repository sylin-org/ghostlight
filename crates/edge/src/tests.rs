//! Tests of the edge's own surface: its version and the private diagnostics it writes.

/// The crate version major is the relay revision, so a published edge names its revision.
#[test]
fn crate_version_major_equals_relay_revision() {
    let major: u16 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap();
    assert_eq!(major, ghostlight_relay::RELAY);
}

#[cfg(unix)]
mod diagnostics {
    //! The diagnostics files an edge role writes are private to the user.

    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use ghostlight_relay::home::{
        DIAGNOSTICS_DIRECTORY_NAME, DIAGNOSTICS_MARKER_NAME, TEST_HOME_VARIABLE,
    };
    use ghostlight_relay::log::Event as LogEvent;

    use crate::record_mcp;

    /// The permission bits of `path`.
    fn mode(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_mcp_relay_writes_a_private_log_in_a_private_directory() {
        let home = std::env::temp_dir().join(format!("ghostlight-edge-log-{}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(DIAGNOSTICS_MARKER_NAME), "").unwrap();
        std::env::set_var(TEST_HOME_VARIABLE, &home);
        record_mcp(LogEvent::ProcessStarted, "");
        std::env::remove_var(TEST_HOME_VARIABLE);
        let directory = home.join(DIAGNOSTICS_DIRECTORY_NAME);
        let log = directory.join("edge-mcp.jsonl");
        assert!(log.is_file());
        assert_eq!(mode(&log), 0o600);
        assert_eq!(mode(&directory), 0o700);
        let _ = fs::remove_dir_all(&home);
    }
}
