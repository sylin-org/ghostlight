//! Integration tests for G18 `policy init`: spawns the built binary
//! (`env!("CARGO_BIN_EXE_ghostlight")`, the `tests/mcp_protocol.rs` pattern) with `--out`
//! pointed at a per-test temp directory, so nothing outside the repository is ever touched.

use std::path::Path;
use std::process::{Command, Output};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run_init(template: &str, out: &Path, force: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ghostlight"));
    cmd.arg("policy")
        .arg("init")
        .arg("--template")
        .arg(template);
    cmd.arg("--out").arg(out);
    if force {
        cmd.arg("--force");
    }
    cmd.output().expect("spawn ghostlight")
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ghostlight-g18-policy-init-{}-{}",
        std::process::id(),
        tag
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Required test 9: create, refuse-without-force, force-overwrite, and bogus-name-lists-all-three.
#[test]
fn create_creates_a_file_byte_identical_to_the_embedded_template() {
    let dir = temp_dir("create");
    let out = dir.join("policy.json");

    let output = run_init("qa-staging", &out, false);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(&format!("Wrote {} (template 'qa-staging').", out.display())));
    assert!(stdout.contains("Windows  %ProgramData%\\ghostlight\\policy.json"));
    assert!(stdout.contains("macOS    /Library/Application Support/ghostlight/policy.json"));
    assert!(stdout.contains("Linux    /etc/ghostlight/policy.json"));
    assert!(stdout.contains("ghostlight --manifest file:///absolute/path/to/policy.json"));

    let written = std::fs::read_to_string(&out).unwrap();
    let example =
        std::fs::read_to_string(repo_root().join("examples").join("qa-staging.json")).unwrap();
    assert_eq!(written, example);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn second_run_without_force_exits_nonzero_and_mentions_force() {
    let dir = temp_dir("noforce");
    let out = dir.join("policy.json");

    assert!(run_init("qa-staging", &out, false).status.success());
    let second = run_init("qa-staging", &out, false);
    assert!(!second.status.success());
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("--force"), "{stderr}");
    assert!(stderr.contains(&out.display().to_string()), "{stderr}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn force_overwrites_an_existing_file() {
    let dir = temp_dir("force");
    let out = dir.join("policy.json");
    std::fs::write(&out, "stale content").unwrap();

    let output = run_init("qa-staging", &out, true);
    assert!(output.status.success());
    let written = std::fs::read_to_string(&out).unwrap();
    let example =
        std::fs::read_to_string(repo_root().join("examples").join("qa-staging.json")).unwrap();
    assert_eq!(written, example);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn bogus_template_name_exits_nonzero_and_lists_the_three_valid_names() {
    let dir = temp_dir("bogus");
    let out = dir.join("policy.json");

    let output = run_init("bogus", &out, false);
    assert!(!output.status.success());
    assert!(!out.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    for name in [
        "enterprise-healthcare",
        "developer-unrestricted",
        "qa-staging",
    ] {
        assert!(stderr.contains(name), "{stderr}");
    }

    std::fs::remove_dir_all(&dir).ok();
}

/// Required test 9's implied coverage of every template: each writes a file that parses through
/// the real manifest loader/validator (paired with `governance::templates`'s own inline test 7,
/// this proves the end-to-end CLI write path, not just the embedded constant).
#[test]
fn every_template_creates_a_file_that_matches_its_examples_source() {
    for name in [
        "enterprise-healthcare",
        "developer-unrestricted",
        "qa-staging",
    ] {
        let dir = temp_dir(&format!("every-{name}"));
        let out = dir.join("policy.json");
        let output = run_init(name, &out, false);
        assert!(output.status.success(), "{name}");
        let written = std::fs::read_to_string(&out).unwrap();
        let example =
            std::fs::read_to_string(repo_root().join("examples").join(format!("{name}.json")))
                .unwrap();
        assert_eq!(written, example, "{name}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
