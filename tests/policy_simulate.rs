//! Integration tests for G17 `policy simulate`: spawns the built binary
//! (`env!("CARGO_BIN_EXE_ghostlight")`, the pattern `tests/mcp_protocol.rs` uses) and checks
//! its stdout/stderr/exit code directly. The pure replay/report core already has its own inline
//! unit tests in `governance::simulate`; what only an integration test can prove is the CLI
//! wiring itself (exit codes, argument parsing, that operational errors never print a partial
//! report).

use std::path::Path;
use std::process::{Command, Output};

fn run_simulate(manifest: &str, replay: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ghostlight"))
        .arg("policy")
        .arg("simulate")
        .arg(manifest)
        .arg("--replay")
        .arg(replay)
        .output()
        .expect("spawn ghostlight")
}

const PERMISSIVE: &str = "tests/fixtures/simulate/manifest-permissive.json";
const RESTRICTIVE: &str = "tests/fixtures/simulate/manifest-restrictive.json";
const REPLAY: &str = "tests/fixtures/simulate/audit.jsonl";

/// Item 1: permissive manifest over the fixture -- zero would-denies, no group section, the
/// exact not-evaluable lines the fixture was authored to produce.
#[test]
fn permissive_manifest_yields_zero_would_denies() {
    let output = run_simulate(PERMISSIVE, REPLAY);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");

    assert!(
        stdout.contains("result: no would-denies (exit 0)"),
        "{stdout}"
    );
    assert!(stdout.contains("would allow: 9"), "{stdout}");
    assert!(stdout.contains("would deny: 0"), "{stdout}");
    assert!(stdout.contains("not evaluable: 4"), "{stdout}");
    assert!(!stdout.contains("would-deny groups"), "{stdout}");

    for line in [
        "line 11: unknown tool: teleport",
        "line 12: unknown action: fly",
        "line 13: computer action missing",
        "line 14: malformed json",
    ] {
        assert!(stdout.contains(line), "missing {line:?} in:\n{stdout}");
    }
}

/// Item 2 (golden test): restrictive manifest over the fixture. Exact totals arithmetic, exact
/// group lines (by substring), denial id shape, sort order, and the folded `computer` count.
#[test]
fn restrictive_manifest_golden() {
    let output = run_simulate(RESTRICTIVE, REPLAY);
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");

    assert!(stdout.contains("total actions: 13"), "{stdout}");
    assert!(stdout.contains("would allow: 4"), "{stdout}");
    assert!(stdout.contains("would deny: 5"), "{stdout}");
    assert!(stdout.contains("not evaluable: 4"), "{stdout}");
    assert_eq!(4 + 5 + 4, 13, "totals arithmetic must hold");

    let expected_groups = [
        "count=1 grant=- domain=unknown.example tool=read_page rule=unmatched_domain",
        "count=3 grant=docs-read domain=docs.example.com tool=computer rule=capability",
        "count=1 grant=forms-noscript domain=forms.example.net tool=javascript_tool rule=capability",
    ];
    for group in expected_groups {
        assert!(
            stdout.contains(group),
            "missing group {group:?} in:\n{stdout}"
        );
    }

    // Every denial id in the report is D- followed by exactly 8 lowercase hex characters.
    for line in stdout.lines().filter(|l| l.starts_with("count=")) {
        let denial = line
            .split("denial=")
            .nth(1)
            .unwrap_or_else(|| panic!("group line missing denial=: {line}"));
        assert!(denial.starts_with("D-"), "{line}");
        let hex = &denial[2..];
        assert_eq!(hex.len(), 8, "{line}");
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{line}"
        );
    }

    // Sort order: group lines appear in the exact order listed above.
    let positions: Vec<usize> = expected_groups
        .iter()
        .map(|g| stdout.find(g).unwrap())
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "group lines out of order: {positions:?}\n{stdout}"
    );

    for line in [
        "line 11: unknown tool: teleport",
        "line 12: unknown action: fly",
        "line 13: computer action missing",
        "line 14: malformed json",
    ] {
        assert!(stdout.contains(line), "missing {line:?} in:\n{stdout}");
    }

    assert!(
        stdout.contains("result: 5 would-denies (exit 2)"),
        "{stdout}"
    );
}

/// Item 3: determinism. Running the golden command twice yields byte-identical stdout.
#[test]
fn restrictive_manifest_is_deterministic() {
    let first = run_simulate(RESTRICTIVE, REPLAY);
    let second = run_simulate(RESTRICTIVE, REPLAY);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.status.code(), second.status.code());
}

/// Item 4: operational errors all exit 1 with no report on stdout.
#[test]
fn nonexistent_replay_path_exits_one_naming_the_path() {
    let missing = "tests/fixtures/simulate/does-not-exist.jsonl";
    let output = run_simulate(PERMISSIVE, missing);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(missing),
        "stderr does not name the path: {stderr}"
    );
}

#[test]
fn manifest_with_invalid_json_exits_one() {
    let path = std::env::temp_dir().join(format!(
        "ghostlight-policy-simulate-badjson-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, "{not valid json").unwrap();

    let output = run_simulate(path.to_str().unwrap(), REPLAY);
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
}

#[test]
fn structurally_invalid_manifest_exits_one() {
    let path = std::env::temp_dir().join(format!(
        "ghostlight-policy-simulate-badgrant-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"{"schema":3,"name":"x","version":"1","grants":[{"id":"g","hosts":{"allow":["example.com"]},"allowed":["mutate"]}]}"#,
    )
    .unwrap();

    let output = run_simulate(path.to_str().unwrap(), REPLAY);
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
}

/// Item 6 (conditional): `examples/` exists in this tree, so every committed example manifest
/// must never exit 1 when simulated against the fixture replay (0 and 2 are both acceptable --
/// only an operational error is disallowed).
#[test]
fn every_committed_example_manifest_never_errors_out() {
    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let entries = std::fs::read_dir(&examples_dir).expect("examples/ exists");

    let mut checked = 0;
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let output = run_simulate(path.to_str().unwrap(), REPLAY);
        assert_ne!(
            output.status.code(),
            Some(1),
            "{path:?} errored: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        checked += 1;
    }
    assert!(checked > 0, "expected at least one example manifest");
}
