//! Integration tests for G16 `policy explain`: spawns the built binary
//! (`env!("CARGO_BIN_EXE_browser-mcp")`, the pattern `tests/mcp_protocol.rs` line 22 uses) and
//! checks its stdout/stderr/exit code directly, since the renderer's own byte-for-byte
//! correctness is already pinned by `governance::explain`'s inline unit tests and this file's
//! golden comparison; what only an integration test can prove is the CLI wiring itself (exit
//! codes, that nothing but the rendering reaches stdout, that an invalid file never produces a
//! best-effort rendering).

use std::path::Path;
use std::process::Command;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn strip_cr(s: &str) -> String {
    s.chars().filter(|&c| c != '\r').collect()
}

fn run_explain(file: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_browser-mcp"))
        .arg("policy")
        .arg("explain")
        .arg(file)
        .output()
        .expect("spawn browser-mcp")
}

/// Golden equality, one case per committed example (Required behavior section 6 / Verification
/// item 4). Compares after stripping every `\r` byte from BOTH sides so a git `autocrlf`
/// checkout cannot break the suite; the renderer itself emits no `\r` at all (pinned by the
/// inline `determinism_and_line_endings` unit test).
#[test]
fn golden_equality_for_every_committed_example() {
    for name in ["enterprise-healthcare", "qa-staging", "research-read-only"] {
        let example = repo_root().join("examples").join(format!("{name}.json"));
        let golden_path = repo_root()
            .join("tests")
            .join("fixtures")
            .join("explain")
            .join(format!("{name}.txt"));
        let golden = std::fs::read_to_string(&golden_path)
            .unwrap_or_else(|e| panic!("{golden_path:?}: {e}"));

        let output = run_explain(&example);
        assert!(
            output.status.success(),
            "{name}: exit {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        assert_eq!(
            strip_cr(&stdout),
            strip_cr(&golden),
            "{name}: rendering does not match the committed golden"
        );
    }
}

/// A manifest that parses as JSON but fails validation (an unsupported schema version) must
/// never produce a best-effort rendering: nonzero exit, empty stdout, a message on stderr.
#[test]
fn invalid_manifest_exits_nonzero_with_nothing_on_stdout() {
    let path = std::env::temp_dir().join(format!(
        "browser-mcp-policy-explain-invalid-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"{"schema":99,"name":"a","version":"1","grants":[]}"#,
    )
    .unwrap();

    let output = run_explain(&path);
    std::fs::remove_file(&path).ok();

    assert!(!output.status.success(), "must exit nonzero");
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(!output.stderr.is_empty(), "stderr must explain the failure");
}

/// A missing path exits nonzero with nothing on stdout.
#[test]
fn missing_file_exits_nonzero_with_nothing_on_stdout() {
    let path = std::env::temp_dir().join(format!(
        "browser-mcp-policy-explain-missing-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path); // ensure it does not exist

    let output = run_explain(&path);
    assert!(!output.status.success(), "must exit nonzero");
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
}
