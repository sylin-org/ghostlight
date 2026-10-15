//! Whether a typed command joins the console of the terminal it was typed in.
//!
//! The release service image is a GUI-subsystem program and so owns no console. A command the
//! edge hands it must still write to the person's terminal and receive Ctrl-C there, so it
//! attaches to its parent's console before anything runs. A detached service, the bare launch
//! and `open` never do: a service must not join a console, and a person's launch prints nothing
//! (ADR-0199 Decision 4). The desktop shell track deletes this module when the image becomes a
//! console-subsystem program.

use std::ffi::OsString;

use crate::cli::parse::{self, LaunchMode};

/// Whether the command in `arguments` writes to the person's console. An argument list that does
/// not parse is an error message for the person, so it counts.
#[must_use]
pub fn wants_parent_console(arguments: &[OsString]) -> bool {
    match parse::launch_mode(arguments.iter().cloned()) {
        Ok(mode) => mode_wants_parent_console(&mode),
        Err(_) => true,
    }
}

/// Whether `mode` writes to the person's console.
fn mode_wants_parent_console(mode: &LaunchMode) -> bool {
    !matches!(
        mode,
        LaunchMode::Serve(_) | LaunchMode::Desktop | LaunchMode::Open
    )
}

/// Attach to the parent's console when the command in `arguments` writes there. Failures are
/// ignored: with no parent console the command runs as it would have.
#[cfg(all(not(debug_assertions), target_os = "windows"))]
pub fn attach_for_typed_command(arguments: &[OsString]) {
    if wants_parent_console(arguments) {
        ghostlight_win_peer::attach_parent_console();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wants(arguments: &[&str]) -> bool {
        let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
        wants_parent_console(&arguments)
    }

    #[test]
    fn a_detached_service_and_a_launch_stay_off_the_console() {
        assert!(!wants(&["serve", "--reason", "agent"]));
        assert!(!wants(&[]));
        assert!(!wants(&["open"]));
    }

    #[test]
    fn every_typed_command_joins_the_console() {
        for arguments in [
            &["doctor"][..],
            &["doctor", "--fix"],
            &["status"],
            &["status", "--json"],
            &["--version"],
            &["--help"],
            &["install"],
            &["uninstall"],
            &["call", "browser_tabs"],
            &["policy", "validate"],
            &["diagnostics", "show"],
            &["mcp"],
            &["deployment", "install"],
        ] {
            assert!(wants(arguments), "{arguments:?}");
        }
    }

    #[test]
    fn arguments_that_do_not_parse_still_reach_the_console() {
        assert!(wants(&["deployment"]));
        assert!(wants(&["no-such-command"]));
    }
}
