//! Tests for the edge's diagnostics writer.
//!
//! The shape of a line is the bridge's to pin, against the record the service reads
//! (`crates/bridge/src/diagnostics.rs`, the shared schema test); these tests prove when the
//! writer writes and how much.

use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::tests::Scratch;

/// A home in a scratch directory, with the diagnostics marker and directory the writer uses.
struct LogHome(Scratch);

impl LogHome {
    fn new() -> Self {
        Self(Scratch::named("log"))
    }

    fn marker(&self) -> PathBuf {
        self.0.join(home::DIAGNOSTICS_MARKER_NAME)
    }

    fn directory(&self) -> PathBuf {
        self.0.join(home::DIAGNOSTICS_DIRECTORY_NAME)
    }

    fn log(&self) -> PathBuf {
        self.directory()
            .join(format!("{}.jsonl", Component::EdgeMcp.as_str()))
    }

    fn write(&self, detail: &str) -> io::Result<()> {
        append(
            &self.marker(),
            &self.directory(),
            1234,
            Component::EdgeMcp,
            Event::ServiceConnected,
            detail,
        )
    }

    /// The detail of every line in the log, in order.
    fn details(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .unwrap()
            .lines()
            .map(|line| {
                let line: Value = serde_json::from_str(line).unwrap();
                line["detail"].as_str().unwrap().to_owned()
            })
            .collect()
    }
}

/// Diagnostics are off unless a person turns them on: while the marker file is absent nothing
/// is written, not even the directory; while it exists each call appends one line; and removing
/// it stops the writing at the next call.
#[test]
fn diagnostics_are_written_only_while_the_marker_exists() {
    let home = LogHome::new();
    home.write("before").unwrap();
    assert!(
        !home.directory().exists(),
        "nothing is written without the marker"
    );
    fs::write(home.marker(), "").unwrap();
    home.write("first").unwrap();
    home.write("second").unwrap();
    assert_eq!(home.details(), ["first", "second"]);
    fs::remove_file(home.marker()).unwrap();
    home.write("after").unwrap();
    assert_eq!(
        home.details(),
        ["first", "second"],
        "removing the marker stops the writing"
    );
}

/// A diagnostics line and its file stay bounded and private: a detail keeps at most
/// `MAX_DETAIL_CHARS` characters, and a file that reached `MAX_FILE_BYTES`, the size at which
/// the service rotates it, takes no more lines. On Unix the first line creates a file only the
/// user can read, in a directory closed to everyone else.
#[test]
fn a_diagnostics_line_and_its_file_stay_bounded_and_private() {
    let home = LogHome::new();
    fs::write(home.marker(), "").unwrap();
    home.write(&"x".repeat(MAX_DETAIL_CHARS * 3)).unwrap();
    assert_eq!(home.details()[0].chars().count(), MAX_DETAIL_CHARS);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&home.log()), 0o600, "the file is the user's alone");
        assert_eq!(
            mode(&home.directory()),
            0o700,
            "the directory is closed to everyone else"
        );
    }

    let full = vec![b' '; usize::try_from(MAX_FILE_BYTES).unwrap()];
    fs::write(home.log(), &full).unwrap();
    home.write("refused").unwrap();
    assert_eq!(
        fs::metadata(home.log()).unwrap().len(),
        MAX_FILE_BYTES,
        "a full file takes no more lines"
    );
}
