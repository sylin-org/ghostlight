//! Opt-in private-session-bus proof for the closed activation mechanism, not live acceptance.

#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use ghostlight_bridge::desktop_activation::{
    claim_ready_name, registration_bytes, registration_path, request_registered_start, BUS_NAME,
};

const STAGE: &str = "GHOSTLIGHT_ACTIVATION_TEST_STAGE";
const ROOT: &str = "GHOSTLIGHT_ACTIVATION_TEST_ROOT";
const TEST: &str = "private_bus_starts_exact_registration_and_reuses_one_name";

#[test]
#[ignore = "requires dbus-run-session; run this test explicitly with --ignored"]
fn private_bus_starts_exact_registration_and_reuses_one_name() {
    match std::env::var(STAGE).as_deref() {
        Ok("owner") => owner(),
        Ok("bus") => bus(),
        _ => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.tmp")
                .join(format!("private-activation-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join(".local/share")).unwrap();
            let status = Command::new("dbus-run-session")
                .arg("--")
                .arg(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", TEST, "--nocapture"])
                .env(STAGE, "bus")
                .env(ROOT, &root)
                .env("XDG_DATA_HOME", root.join(".local/share"))
                .status()
                .expect("dbus-run-session is available for the explicit private-bus gate");
            assert!(
                status.success(),
                "private-bus test failed; evidence retained in {root:?}"
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(std::env::var_os(ROOT).expect("fixture root is set by parent"))
}

fn owner() {
    let root = root();
    let executable = root.join("bin with spaces/ghostlight");
    fs::write(root.join("desktop-ready"), std::process::id().to_string()).unwrap();
    let lease = claim_ready_name(&root, &executable).unwrap().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(lease);
}

fn bus() {
    let root = root();
    let executable = root.join("bin with spaces/ghostlight");
    let registration = registration_path(&root);
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::create_dir_all(registration.parent().unwrap()).unwrap();
    let runner = std::env::current_exe().unwrap();
    let quoted_runner = runner.to_str().unwrap().replace('\'', "'\\''");
    // This is an isolated test fixture, never an installed product wrapper. D-Bus invokes
    // its exact no-argument path, and the fixture re-enters this test as the name owner.
    fs::write(&executable, format!(
        "#!/bin/sh\nexport {STAGE}=owner\nexec '{quoted_runner}' --ignored --exact {TEST} --nocapture\n"
    )).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&registration, registration_bytes(&executable).unwrap()).unwrap();
    let connection = zbus::blocking::Connection::session().unwrap();
    let proxy = zbus::blocking::fdo::DBusProxy::new(&connection).unwrap();
    proxy.reload_config().unwrap();
    assert!(!proxy.name_has_owner(BUS_NAME.try_into().unwrap()).unwrap());
    request_registered_start(&root, &executable).unwrap();
    let owner_pid = proxy
        .get_connection_unix_process_id(BUS_NAME.try_into().unwrap())
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("desktop-ready")).unwrap(),
        owner_pid.to_string()
    );
    request_registered_start(&root, &executable).unwrap();
    assert_eq!(
        proxy
            .get_connection_unix_process_id(BUS_NAME.try_into().unwrap())
            .unwrap(),
        owner_pid
    );
    assert!(request_registered_start(&root, &root.join("other/ghostlight")).is_err());
    fs::write(root.join("stop"), b"stop").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while proxy.name_has_owner(BUS_NAME.try_into().unwrap()).unwrap() {
        assert!(
            Instant::now() < deadline,
            "fixture owner did not release name"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
