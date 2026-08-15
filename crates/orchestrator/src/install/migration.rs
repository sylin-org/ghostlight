//! Narrow retirement of recognized pre-1.0 resident-supervisor artifacts.

#[cfg(target_os = "linux")]
use std::env;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(any(windows, target_os = "linux"))]
use std::process::Command;

use serde::Serialize;

#[cfg(windows)]
const WINDOWS_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const WINDOWS_VALUE_NAME: &str = "Ghostlight Service";
#[cfg(windows)]
const WINDOWS_TASK_NAME: &str = "Ghostlight Service";
#[cfg(target_os = "linux")]
const LINUX_UNIT_NAME: &str = "ghostlight.service";

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
    #[cfg(target_os = "linux")]
    {
        retire_linux()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        MigrationReport::default()
    }
}

#[cfg(target_os = "linux")]
fn home_directory() -> PathBuf {
    // Linux-only: HOME is the one variable that means this on this platform. USERPROFILE is a
    // Windows convention; checking it here first was confusing at best, and under WSL interop
    // that leaks Windows environment variables through, a Windows-style path could silently win
    // over the correct Linux HOME, misdirecting where the systemd cleanup below looks.
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

#[cfg(any(windows, target_os = "linux", test))]
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

#[cfg(any(target_os = "linux", test))]
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

#[cfg(target_os = "linux")]
fn retire_linux() -> MigrationReport {
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_directory().join(".config"));
    retire_linux_at(&config, |arguments| {
        let status = Command::new("systemctl")
            .args(arguments)
            .status()
            .map_err(|error| format!("could not invoke systemctl: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("systemctl exited {:?}", status.code()))
        }
    })
}

#[cfg(target_os = "linux")]
fn retire_linux_at(
    config: &Path,
    mut systemctl: impl FnMut(&[&str]) -> Result<(), String>,
) -> MigrationReport {
    let mut report = MigrationReport::default();
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
            if let Err(error) = systemctl(&["--user", "stop", LINUX_UNIT_NAME]) {
                report
                    .warnings
                    .push(format!("could not stop the old systemd user unit: {error}"));
            }
            if fs::symlink_metadata(&link).is_ok() {
                match fs::remove_file(&link) {
                    Ok(()) => report
                        .removed
                        .push("obsolete systemd user enablement for ghostlight.service".into()),
                    Err(error) => report.warnings.push(format!(
                        "could not remove the old systemd enablement: {error}"
                    )),
                }
            }
            match fs::remove_file(&unit) {
                Ok(()) => {
                    report
                        .removed
                        .push("obsolete systemd user unit ghostlight.service".into());
                    if let Err(error) = systemctl(&["--user", "daemon-reload"]) {
                        report.warnings.push(format!(
                            "could not refresh the systemd user manager: {error}"
                        ));
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

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use std::fs;
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::symlink;
    #[cfg(target_os = "linux")]
    use std::time::SystemTime;

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

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_retirement_stops_the_service_and_removes_its_enablement() {
        let config = std::env::temp_dir().join(format!(
            "ghostlight-migration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let unit = config.join("systemd/user").join(LINUX_UNIT_NAME);
        let link = config
            .join("systemd/user/default.target.wants")
            .join(LINUX_UNIT_NAME);
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::write(
            &unit,
            "[Unit]\nDescription=Ghostlight Hub service\n[Service]\nExecStart=/home/u/.ghostlight/bin/v0.8.0/ghostlight service\n",
        )
        .unwrap();
        symlink(&unit, &link).unwrap();
        let mut commands: Vec<Vec<String>> = Vec::new();

        let report = retire_linux_at(&config, |arguments| {
            commands.push(arguments.iter().map(|value| (*value).to_owned()).collect());
            Ok(())
        });

        assert_eq!(
            commands,
            vec![
                vec!["--user", "stop", LINUX_UNIT_NAME],
                vec!["--user", "daemon-reload"],
            ]
        );
        assert!(!unit.exists());
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(report.warnings.is_empty());
        assert_eq!(report.removed.len(), 2);
        fs::remove_dir_all(config).unwrap();
    }
}
