//! Narrow retirement of recognized pre-1.0 resident-supervisor artifacts.

#[cfg(unix)]
use std::env;
#[cfg(target_os = "macos")]
use std::ffi::OsStr;
#[cfg(unix)]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use std::process::Command;

use serde::Serialize;

#[cfg(windows)]
const WINDOWS_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const WINDOWS_VALUE_NAME: &str = "Ghostlight Service";
#[cfg(windows)]
const WINDOWS_TASK_NAME: &str = "Ghostlight Service";
#[cfg(all(unix, not(target_os = "macos")))]
const LINUX_UNIT_NAME: &str = "ghostlight.service";
#[cfg(target_os = "macos")]
const MACOS_LABEL: &str = "org.sylin.ghostlight.service";

/// Content-free account of obsolete artifacts removed or deliberately preserved.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MigrationReport {
    /// Recognized Ghostlight artifacts that were removed.
    pub removed: Vec<String>,
    /// Ambiguous or foreign artifacts that were left untouched.
    pub preserved: Vec<String>,
    /// Best-effort operating-system refresh failures.
    pub warnings: Vec<String>,
}

impl MigrationReport {
    /// Whether this run retired at least one old lifecycle artifact.
    #[must_use]
    pub fn changed(&self) -> bool {
        !self.removed.is_empty()
    }
}

/// Remove only recognized 0.8 resident-supervisor artifacts for the current user.
///
/// ADR-0104 demand-start replaced these mechanisms. This function never installs a successor and
/// never removes a command or definition that does not prove the old Ghostlight shape.
#[must_use]
pub fn retire_obsolete_supervisor() -> MigrationReport {
    #[cfg(windows)]
    {
        retire_windows()
    }
    #[cfg(target_os = "macos")]
    {
        retire_macos()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        retire_linux()
    }
    #[cfg(not(any(windows, unix)))]
    {
        MigrationReport::default()
    }
}

#[cfg(unix)]
fn home_directory() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn command_is_old_ghostlight_service(command: &str) -> bool {
    let command = command.trim();
    let (program, arguments) = if let Some(rest) = command.strip_prefix('"') {
        let Some(end) = rest.find('"') else {
            return false;
        };
        (&rest[..end], rest[end + 1..].trim())
    } else {
        let Some((program, arguments)) = command.split_once(char::is_whitespace) else {
            return false;
        };
        (program, arguments.trim())
    };
    let program_name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let owned_program = program_name.eq_ignore_ascii_case("ghostlight")
        || program_name.eq_ignore_ascii_case("ghostlight.exe");
    let arguments = arguments.split_whitespace().collect::<Vec<_>>();
    let owned_arguments = arguments == ["service"]
        || arguments.len() == 3
            && arguments[0] == "--instance"
            && !arguments[1].starts_with('-')
            && arguments[2] == "service";
    owned_program && owned_arguments
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn definition_is_old_ghostlight_service(contents: &str, marker: &str) -> bool {
    contents.contains(marker)
        && contents.lines().any(|line| {
            line.trim()
                .strip_prefix("ExecStart=")
                .is_some_and(command_is_old_ghostlight_service)
        })
}

#[cfg(windows)]
fn retire_windows() -> MigrationReport {
    use std::io;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let mut report = MigrationReport::default();
    let root = RegKey::predef(HKEY_CURRENT_USER);
    match root.open_subkey_with_flags(WINDOWS_RUN_KEY, KEY_READ | KEY_WRITE) {
        Ok(key) => match key.get_value::<String, _>(WINDOWS_VALUE_NAME) {
            Ok(command) if command_is_old_ghostlight_service(&command) => {
                match key.delete_value(WINDOWS_VALUE_NAME) {
                    Ok(()) => report
                        .removed
                        .push("Windows per-user Ghostlight Service Run value".into()),
                    Err(error) => report.warnings.push(format!(
                        "could not remove the old Windows Run value: {error}"
                    )),
                }
            }
            Ok(_) => report
                .preserved
                .push("foreign Windows Run value named Ghostlight Service".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => report.warnings.push(format!(
                "could not inspect the old Windows Run value: {error}"
            )),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => report
            .warnings
            .push(format!("could not inspect the Windows Run key: {error}")),
    }

    match Command::new("schtasks")
        .args(["/query", "/tn", WINDOWS_TASK_NAME, "/xml"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let xml = String::from_utf8_lossy(&output.stdout);
            let normalized = xml.replace("\r\n", "").replace("\n", "");
            let owned = normalized
                .split("<Command>")
                .nth(1)
                .and_then(|tail| tail.split("</Command>").next())
                .zip(
                    normalized
                        .split("<Arguments>")
                        .nth(1)
                        .and_then(|tail| tail.split("</Arguments>").next()),
                )
                .is_some_and(|(program, arguments)| {
                    command_is_old_ghostlight_service(&format!("\"{program}\" {arguments}"))
                });
            if owned {
                match Command::new("schtasks")
                    .args(["/delete", "/tn", WINDOWS_TASK_NAME, "/f"])
                    .status()
                {
                    Ok(status) if status.success() => report
                        .removed
                        .push("legacy Ghostlight Service scheduled task".into()),
                    Ok(status) => report.warnings.push(format!(
                        "could not remove the legacy scheduled task (exit {:?})",
                        status.code()
                    )),
                    Err(error) => report.warnings.push(format!(
                        "could not invoke schtasks to remove the legacy task: {error}"
                    )),
                }
            } else {
                report
                    .preserved
                    .push("foreign scheduled task named Ghostlight Service".into());
            }
        }
        Ok(_) => {}
        Err(error) => report
            .warnings
            .push(format!("could not inspect legacy scheduled tasks: {error}")),
    }
    report
}

#[cfg(all(unix, not(target_os = "macos")))]
fn retire_linux() -> MigrationReport {
    let mut report = MigrationReport::default();
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_directory().join(".config"));
    let unit = config.join("systemd/user").join(LINUX_UNIT_NAME);
    let link = config
        .join("systemd/user/default.target.wants")
        .join(LINUX_UNIT_NAME);
    match fs::read_to_string(&unit) {
        Ok(contents)
            if definition_is_old_ghostlight_service(
                &contents,
                "Description=Ghostlight Hub service",
            ) =>
        {
            match fs::remove_file(&unit) {
                Ok(()) => {
                    report
                        .removed
                        .push("obsolete systemd user unit ghostlight.service".into());
                    if link.exists() {
                        match fs::remove_file(&link) {
                            Ok(()) => report.removed.push(
                                "obsolete systemd user enablement for ghostlight.service".into(),
                            ),
                            Err(error) => report.warnings.push(format!(
                                "could not remove the old systemd enablement: {error}"
                            )),
                        }
                    }
                    match Command::new("systemctl")
                        .args(["--user", "daemon-reload"])
                        .status()
                    {
                        Ok(status) if status.success() => {}
                        Ok(status) => report.warnings.push(format!(
                            "systemctl --user daemon-reload exited {:?}",
                            status.code()
                        )),
                        Err(error) => report.warnings.push(format!(
                            "could not refresh the systemd user manager: {error}"
                        )),
                    }
                }
                Err(error) => report.warnings.push(format!(
                    "could not remove the old systemd user unit: {error}"
                )),
            }
        }
        Ok(_) => report
            .preserved
            .push("foreign systemd user unit named ghostlight.service".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report.warnings.push(format!(
            "could not inspect the old systemd user unit: {error}"
        )),
    }
    report
}

#[cfg(target_os = "macos")]
fn retire_macos() -> MigrationReport {
    let mut report = MigrationReport::default();
    let plist = home_directory()
        .join("Library/LaunchAgents")
        .join(format!("{MACOS_LABEL}.plist"));
    match fs::read_to_string(&plist) {
        Ok(contents)
            if contents.contains(&format!("<string>{MACOS_LABEL}</string>"))
                && contents.contains("<string>service</string>")
                && contents
                    .split("<string>")
                    .filter_map(|tail| tail.split("</string>").next())
                    .any(|value| {
                        Path::new(value)
                            .file_stem()
                            .and_then(OsStr::to_str)
                            .is_some_and(|stem| stem.eq_ignore_ascii_case("ghostlight"))
                    }) =>
        {
            match fs::remove_file(&plist) {
                Ok(()) => report
                    .removed
                    .push("obsolete Ghostlight launchd agent".into()),
                Err(error) => report
                    .warnings
                    .push(format!("could not remove the old launchd agent: {error}")),
            }
        }
        Ok(_) => report
            .preserved
            .push("foreign launchd agent using the Ghostlight label".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report
            .warnings
            .push(format!("could not inspect the old launchd agent: {error}")),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_old_service_command_shape_is_owned() {
        assert!(command_is_old_ghostlight_service(
            r#""C:\Users\u\.ghostlight\bin\0.8.0\ghostlight.exe" service"#
        ));
        assert!(command_is_old_ghostlight_service(
            "/home/u/.ghostlight/bin/ghostlight --instance qa service"
        ));
        assert!(command_is_old_ghostlight_service(
            r#""C:\Users\u\.ghostlight\bin\0.8.0\GhOsTlIgHt.ExE" service"#
        ));
        assert!(!command_is_old_ghostlight_service(
            r#""C:\Ghostlight\ghostlight.cmd" service"#
        ));
        assert!(!command_is_old_ghostlight_service(
            r#""C:\Ghostlight\ghostlight-old.exe" service"#
        ));
        assert!(!command_is_old_ghostlight_service(
            r#""C:\other\runner.exe" service"#
        ));
        assert!(!command_is_old_ghostlight_service(
            r#""C:\Ghostlight\ghostlight.exe" --headless"#
        ));
        assert!(!command_is_old_ghostlight_service(
            r#""C:\Ghostlight\ghostlight.exe" service extra"#
        ));
    }

    #[test]
    fn linux_definition_requires_both_identity_and_owned_command() {
        let owned = "[Unit]\nDescription=Ghostlight Hub service\n[Service]\nExecStart=/opt/ghostlight service\n";
        assert!(definition_is_old_ghostlight_service(
            owned,
            "Description=Ghostlight Hub service"
        ));
        assert!(!definition_is_old_ghostlight_service(
            &owned.replace("/opt/ghostlight", "/opt/other"),
            "Description=Ghostlight Hub service"
        ));
        assert!(!definition_is_old_ghostlight_service(
            &owned.replace("Ghostlight Hub service", "Another service"),
            "Description=Ghostlight Hub service"
        ));
    }
}
