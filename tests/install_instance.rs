// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0044: `--instance <n> install` plans a full per-instance stack (a binary copy Chrome
//! launches by name, an instance-isolated native host + dirs, and a suffixed supervisor), while
//! the default install stays byte-identical. Since ADR-0065 there is no dev-special install path:
//! `dev` is an ordinary named instance like any other (named instances are a test-isolation seam,
//! not a workflow). Drives `install --dry-run` as a subprocess (writes nothing, runs no external
//! command) and inspects the printed plan. `--all-browsers`/`--all-clients` force a deterministic
//! plan regardless of what is installed on the test machine.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ghostlight")
}

fn install_plan(instance: Option<&str>) -> String {
    let mut cmd = Command::new(bin());
    if let Some(n) = instance {
        cmd.arg("--instance").arg(n);
    }
    let out = cmd
        .args([
            "install",
            "--dry-run",
            "--all-browsers",
            "--all-clients",
            "--extension-id",
            &"a".repeat(32),
        ])
        .output()
        .expect("run ghostlight install --dry-run");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn default_install_plan_is_byte_identical_and_places_no_copy() {
    let plan = install_plan(None);
    assert!(
        plan.contains("Ghostlight Service"),
        "default supervisor is the unsuffixed name: {plan}"
    );
    assert!(
        !plan.contains("(dev)") && !plan.contains("ghostlight-dev"),
        "the default plan carries no instance suffix anywhere: {plan}"
    );
    assert!(
        !plan.contains("instance binary"),
        "the default instance places no per-instance binary copy: {plan}"
    );
}

#[test]
fn dev_install_plan_is_an_ordinary_named_instance_plan() {
    // ADR-0065: there is no dev-special install path. A `dev` install plans exactly what any
    // named instance plans (ADR-0044): a `ghostlight-relay-dev` copy Chrome launches by name, a
    // suffixed native host, client entries, AND the suffixed supervisor -- no pinned skip line.
    let plan = install_plan(Some("dev"));
    assert!(
        plan.contains("instance binary") && plan.contains("ghostlight-relay-dev"),
        "the dev plan copies its own per-instance relay binary: {plan}"
    );
    assert!(
        plan.contains("org.sylin.ghostlight.dev"),
        "the dev plan registers its own suffixed native host: {plan}"
    );
    assert!(
        plan.contains("(client)"),
        "the dev plan still registers MCP-client entries: {plan}"
    );
    assert!(
        plan.contains("Ghostlight Service (dev)"),
        "the dev plan registers a suffixed supervisor like any named instance: {plan}"
    );
    assert!(
        !plan.contains("runs its service in a terminal"),
        "the ADR-0048 dev supervisor skip line is gone: {plan}"
    );
}

#[test]
fn a_named_non_dev_instance_still_plans_the_full_stack() {
    // ADR-0044: every named instance plans the full per-instance stack (copy launched by name,
    // isolated host, suffixed supervisor).
    let plan = install_plan(Some("qa"));
    assert!(
        plan.contains("instance binary") && plan.contains("ghostlight-relay-qa"),
        "a qa plan copies a per-instance relay binary: {plan}"
    );
    assert!(
        plan.contains("org.sylin.ghostlight.qa"),
        "a qa plan uses a suffixed native-host name: {plan}"
    );
    assert!(
        plan.contains("Ghostlight Service (qa)"),
        "a qa plan registers a suffixed supervisor: {plan}"
    );
}

#[test]
fn no_supervisor_flag_plans_no_supervisor_steps() {
    // ADR-0046 dev loop: --no-supervisor skips OS auto-start registration entirely, so an
    // auto-started dev service never holds the exe lock during a rebuild.
    let out = Command::new(bin())
        .args([
            "install",
            "--dry-run",
            "--no-supervisor",
            "--all-browsers",
            "--all-clients",
            "--extension-id",
            &"a".repeat(32),
        ])
        .output()
        .expect("run ghostlight install --dry-run --no-supervisor");
    let plan = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        plan.contains("(skipped: --no-supervisor)"),
        "the supervisor section is skipped: {plan}"
    );
    for os_cmd in ["schtasks", "launchctl", "systemctl"] {
        assert!(
            !plan.contains(os_cmd),
            "no supervisor OS command is planned ({os_cmd}): {plan}"
        );
    }
}
