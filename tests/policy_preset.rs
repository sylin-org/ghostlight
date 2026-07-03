//! Integration tests for G18 `config preset`: spawns the built binary
//! (`env!("CARGO_BIN_EXE_browser-mcp")`, the `tests/mcp_protocol.rs` pattern).
//!
//! Deliberately `--dry-run` ONLY: `config preset` without `--dry-run` writes to the real
//! per-platform user config file (`load::user_config_path`, not overridable via environment for
//! a spawned child process), so a non-dry-run integration test would mutate the operator's own
//! `browser-mcp` configuration outside the repository -- unacceptable side effect for an
//! automated test run. The write path itself (missing/preserve/corrupt-file behavior, and that
//! the stored value is always the underscore form) is covered by
//! `governance::config::presets`'s own inline unit tests against temp paths, matching the
//! established pattern `governance::config::cli`'s own `write_user_value` tests already use for
//! the identical concern.

use std::process::{Command, Output};

fn run_preset(preset: &str, dry_run: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_browser-mcp"));
    cmd.arg("config").arg("preset").arg(preset);
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.output().expect("spawn browser-mcp")
}

#[test]
fn dry_run_never_writes_and_ends_with_the_exact_last_line() {
    let output = run_preset("safe", true);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(stdout.starts_with("Preset change: "), "{stdout}");
    assert!(stdout.contains("User config file: "), "{stdout}");
    assert!(
        stdout.trim_end().ends_with("Dry run: nothing written."),
        "{stdout}"
    );
}

/// Required test 1 (g18 doc, Tests item 1): the CLI hyphen spelling and its underscore alias
/// select the identical preset. Proven without touching any real file: both dry-run invocations
/// must resolve the SAME candidate preset against the SAME on-disk state, so their diffs (and
/// therefore their full stdout) are byte-identical.
#[test]
fn hyphen_spelling_and_underscore_alias_select_the_same_preset() {
    let hyphen = run_preset("fully-open", true);
    let underscore = run_preset("fully_open", true);
    assert!(hyphen.status.success());
    assert!(underscore.status.success());
    assert_eq!(hyphen.stdout, underscore.stdout);
}

#[test]
fn every_preset_spelling_dry_runs_successfully() {
    for name in ["fully-open", "safe", "restricted"] {
        let output = run_preset(name, true);
        assert!(
            output.status.success(),
            "{name}: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(&format!("-> {name}\n")), "{name}: {stdout}");
    }
}

#[test]
fn unknown_preset_name_is_a_clap_usage_error() {
    let output = run_preset("bogus", true);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fully-open"), "{stderr}");
    assert!(stderr.contains("safe"), "{stderr}");
    assert!(stderr.contains("restricted"), "{stderr}");
}
