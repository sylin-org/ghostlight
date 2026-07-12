// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0044: `--instance <n> install` plans a full per-instance stack (a binary copy Chrome
//! launches by name, an instance-isolated native host + dirs, and a suffixed supervisor), while
//! the default install stays byte-identical. ADR-0048 D6: the reserved dev instance is the
//! exception -- its install is THIN (client entries only); every OTHER named instance keeps the
//! full stack. Drives `install --dry-run` as a subprocess (writes nothing, runs no external
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
fn dev_install_plan_registers_its_own_isolated_host() {
    // ADR-0064: the dev instance is isolated explicitly, so its install plans its OWN per-instance
    // stack (a `ghostlight-relay-dev` copy Chrome launches by name + a suffixed native host), just
    // like any other named instance -- NOT the pre-0064 thin shadow onto the default host. It still
    // skips the auto-start supervisor (a developer runs the dev service from a terminal).
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
        plan.contains("(skipped: the dev instance runs its service in a terminal; ADR-0048)"),
        "the dev supervisor section still prints the pinned skip line: {plan}"
    );
}

#[test]
fn a_named_non_dev_instance_still_plans_the_full_stack() {
    // ADR-0048 D6: only `dev` thins; every other named instance keeps ADR-0044's full
    // per-instance stack (copy launched by name, isolated host, suffixed supervisor).
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
