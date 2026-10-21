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

/// ADR-0199 Decision 2, frozen with the relay: an edge that does not run from `<home>/bin` under
/// the edge's file name is a carrier of the service image beside it, whatever its arguments; the
/// file name compares ASCII case-insensitively on Windows and exactly elsewhere. An installed
/// edge reads only its first argument: exactly `mcp` is the MCP relay, a Chrome extension origin
/// is the browser relay, and anything else, a near miss included, runs the service image.
#[test]
fn the_route_reads_where_the_edge_runs_and_then_only_its_first_argument() {
    let browser = Route::Browser {
        origin: EXTENSION.to_owned(),
    };
    let cases: [(&[&str], Route); 15] = [
        (&[], Route::Service),
        (&["mcp"], Route::Mcp),
        (&["mcp", "--verbose"], Route::Mcp),
        (&["mcp", EXTENSION], Route::Mcp),
        (&["MCP"], Route::Service),
        (&["Mcp"], Route::Service),
        (&[EXTENSION], browser.clone()),
        (&[EXTENSION, "--parent-window=0"], browser),
        (&["chrome-extension:abc"], Route::Service),
        (&["chrome-extension"], Route::Service),
        (&["moz-extension://abc/"], Route::Service),
        (&["--chrome-extension://abc/"], Route::Service),
        (&["serve", "mcp"], Route::Service),
        (&["serve", EXTENSION], Route::Service),
        (&["status", "--json"], Route::Service),
    ];
    for (arguments, expected) in cases {
        assert_eq!(installed(arguments), expected, "{arguments:?}");
    }

    for arguments in [&[][..], &["mcp"], &[EXTENSION], &["status"]] {
        assert_eq!(
            run(PathBuf::from(ELSEWHERE).join(EDGE), arguments),
            Route::Carrier {
                directory: PathBuf::from(ELSEWHERE)
            },
            "{arguments:?}"
        );
    }
    assert_eq!(
        run(PathBuf::from(BIN).join("renamed"), &["mcp"]),
        Route::Carrier {
            directory: PathBuf::from(BIN)
        },
        "another file name in bin"
    );
    let old = PathBuf::from(BIN).join("old");
    assert_eq!(
        run(old.join(EDGE), &[]),
        Route::Carrier { directory: old },
        "a directory below bin"
    );
    let shouted = run(PathBuf::from(BIN).join(EDGE.to_ascii_uppercase()), &["mcp"]);
    if cfg!(windows) {
        assert_eq!(
            shouted,
            Route::Mcp,
            "the file name ignores ASCII case on Windows"
        );
    } else {
        assert!(
            matches!(shouted, Route::Carrier { .. }),
            "the file name is exact elsewhere"
        );
    }
}
