//! Tests of the edge's route over every shape of arguments and of where the edge runs.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use super::{route, Route};

#[cfg(windows)]
const BIN: &str = "C:\\Users\\person\\.ghostlight\\bin";
#[cfg(not(windows))]
const BIN: &str = "/home/person/.ghostlight/bin";

#[cfg(windows)]
const ELSEWHERE: &str = "C:\\Users\\person\\Downloads";
#[cfg(not(windows))]
const ELSEWHERE: &str = "/home/person/Downloads";

#[cfg(windows)]
const EDGE: &str = "ghostlight-edge.exe";
#[cfg(not(windows))]
const EDGE: &str = "ghostlight-edge";

const EXTENSION: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

fn run(this_exe: PathBuf, values: &[&str]) -> Route {
    route(
        &this_exe,
        &PathBuf::from(BIN),
        OsStr::new(EDGE),
        &args(values),
    )
}

fn installed(values: &[&str]) -> Route {
    run(PathBuf::from(BIN).join(EDGE), values)
}

#[test]
fn no_arguments_run_the_service_image() {
    assert_eq!(installed(&[]), Route::Service);
}

#[test]
fn the_mcp_verb_runs_the_connector_with_or_without_more_arguments() {
    assert_eq!(installed(&["mcp"]), Route::Mcp);
    assert_eq!(installed(&["mcp", "--verbose"]), Route::Mcp);
}

#[test]
fn the_mcp_verb_is_case_sensitive() {
    assert_eq!(installed(&["MCP"]), Route::Service);
    assert_eq!(installed(&["Mcp"]), Route::Service);
}

#[test]
fn an_extension_origin_runs_the_browser_connector() {
    let expected = Route::Browser {
        origin: EXTENSION.to_owned(),
    };
    assert_eq!(installed(&[EXTENSION]), expected);
    assert_eq!(installed(&[EXTENSION, "--parent-window=0"]), expected);
}

#[test]
fn a_near_miss_of_the_origin_runs_the_service_image() {
    assert_eq!(installed(&["chrome-extension:abc"]), Route::Service);
    assert_eq!(installed(&["chrome-extension"]), Route::Service);
    assert_eq!(installed(&["moz-extension://abc/"]), Route::Service);
    assert_eq!(installed(&["--chrome-extension://abc/"]), Route::Service);
}

#[test]
fn only_the_first_argument_selects_the_route() {
    assert_eq!(installed(&["serve", "mcp"]), Route::Service);
    assert_eq!(installed(&["serve", EXTENSION]), Route::Service);
    assert_eq!(installed(&["mcp", EXTENSION]), Route::Mcp);
}

#[test]
fn an_unknown_verb_runs_the_service_image() {
    assert_eq!(installed(&["status", "--json"]), Route::Service);
}

#[test]
fn a_copy_outside_the_bin_directory_is_a_carrier() {
    for values in [&[][..], &["mcp"], &[EXTENSION], &["status"]] {
        assert_eq!(
            run(PathBuf::from(ELSEWHERE).join(EDGE), values),
            Route::Carrier {
                directory: PathBuf::from(ELSEWHERE)
            }
        );
    }
}

#[test]
fn another_file_name_in_the_bin_directory_is_a_carrier() {
    assert_eq!(
        run(PathBuf::from(BIN).join("renamed"), &["mcp"]),
        Route::Carrier {
            directory: PathBuf::from(BIN)
        }
    );
}

#[test]
fn a_subdirectory_of_the_bin_directory_is_a_carrier() {
    let directory = PathBuf::from(BIN).join("old");
    assert_eq!(run(directory.join(EDGE), &[]), Route::Carrier { directory });
}

#[cfg(windows)]
#[test]
fn the_file_name_ignores_ascii_case_on_windows() {
    assert_eq!(
        run(PathBuf::from(BIN).join("GHOSTLIGHT-EDGE.EXE"), &["mcp"]),
        Route::Mcp
    );
}

#[cfg(not(windows))]
#[test]
fn the_file_name_is_exact_off_windows() {
    assert!(matches!(
        run(PathBuf::from(BIN).join("GHOSTLIGHT-EDGE"), &[]),
        Route::Carrier { .. }
    ));
}
