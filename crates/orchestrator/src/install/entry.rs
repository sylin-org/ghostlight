//! The entry rule: what a `ghostlight` process does before it does its job.
//!
//! Any copy of the executable can be started: from `bin`, from a development build, or from a
//! package that only carries it. [`decide`] answers, from the process's own path, its role, the
//! development marker and the state of the installation, whether to run, hand over to another
//! executable, install first, or refuse (ADR-0197 Decisions 10, 13 and 15). It is pure: the
//! caller gathers the facts and performs the answer.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::marker::DevelopmentMarker;
use super::record::Standing;

/// What a process was started to do, as far as the entry rule cares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// `deployment`: the installer itself. It runs from wherever its payload is.
    Deployment,
    /// `serve`: the service a connector starts.
    Serve,
    /// Any role that uses the service or changes the installation: the workbench, `open`,
    /// `call`, `install`, `uninstall`, `policy`, `diagnostics` and `native-host`.
    Launch,
    /// `doctor`, `status`, `help` and `--version`: they report and never install.
    Inspect,
}

/// What the caller learned about the machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Facts {
    /// The fixed service executable, `home/bin/ghostlight`.
    pub installed_service: PathBuf,
    /// Whether the executable the development marker names exists.
    pub development_executable_exists: bool,
    /// How the installed copy stands against this process's own package.
    pub standing: Standing,
}

/// What the process does next.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Entry {
    /// Do the job from this executable.
    Run,
    /// Run this executable with the same arguments instead.
    Forward(PathBuf),
    /// Install from this executable's own directory, then run the installed service in its
    /// place.
    CarrierInstall,
    /// Do not run.
    Refuse(Refusal),
}

/// Why a process may not run.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum Refusal {
    /// Only the installed service or the development build serves, so there is one service per
    /// user by construction.
    #[error("only the installed Ghostlight service or the development build can serve")]
    ServeFromElsewhere,
}

/// Decide what the process at `current_exe` does for `role`.
///
/// 1. The installer always runs.
/// 2. While a development marker names an existing executable, that executable does everything
///    else: it runs, and every other copy forwards to it. A marker whose executable is gone is
///    ignored, so a deleted build cannot lock out the installed one.
/// 3. Without a marker only the installed service serves. Inspection runs from anywhere.
/// 4. Any other launch from a copy that is not the installed service is a carrier: it installs
///    when the installed service is absent, older or damaged, and otherwise hands over.
#[must_use]
pub fn decide(
    current_exe: &Path,
    role: Role,
    marker: Option<&DevelopmentMarker>,
    facts: &Facts,
) -> Entry {
    if role == Role::Deployment {
        return Entry::Run;
    }
    if let Some(marker) = marker.filter(|_| facts.development_executable_exists) {
        return if same_path(current_exe, &marker.executable) {
            Entry::Run
        } else {
            Entry::Forward(marker.executable.clone())
        };
    }
    let installed = same_path(current_exe, &facts.installed_service);
    match role {
        Role::Deployment | Role::Inspect => Entry::Run,
        Role::Serve if installed => Entry::Run,
        Role::Serve => Entry::Refuse(Refusal::ServeFromElsewhere),
        Role::Launch if installed => Entry::Run,
        Role::Launch => match facts.standing {
            Standing::Absent | Standing::Older | Standing::Damaged => Entry::CarrierInstall,
            Standing::Same | Standing::Newer => Entry::Forward(facts.installed_service.clone()),
        },
    }
}

/// Whether two executable paths name the same file, ignoring ASCII case on Windows.
fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const ROOT: &str = "C:\\Users\\person\\.ghostlight";
    #[cfg(not(windows))]
    const ROOT: &str = "/home/person/.ghostlight";

    fn path(relative: &str) -> PathBuf {
        let mut path = PathBuf::from(ROOT);
        for part in relative.split('/') {
            path.push(part);
        }
        path
    }

    fn facts(standing: Standing) -> Facts {
        Facts {
            installed_service: path("bin/ghostlight"),
            development_executable_exists: true,
            standing,
        }
    }

    fn marker() -> DevelopmentMarker {
        DevelopmentMarker {
            executable: path("dev/ghostlight"),
        }
    }

    fn check(
        name: &str,
        current_exe: &Path,
        role: Role,
        marker: Option<DevelopmentMarker>,
        facts: Facts,
        expected: Entry,
    ) {
        assert_eq!(
            decide(current_exe, role, marker.as_ref(), &facts),
            expected,
            "{name}"
        );
    }

    #[test]
    fn decisions_follow_the_entry_rule() {
        let installed = path("bin/ghostlight");
        let development = path("dev/ghostlight");
        let carrier = path("package/ghostlight");
        let missing_build = Facts {
            development_executable_exists: false,
            ..facts(Standing::Same)
        };
        check(
            "the installer runs from a carrier",
            &carrier,
            Role::Deployment,
            None,
            facts(Standing::Absent),
            Entry::Run,
        );
        check(
            "the installer ignores the marker",
            &carrier,
            Role::Deployment,
            Some(marker()),
            facts(Standing::Same),
            Entry::Run,
        );
        check(
            "the installed service runs",
            &installed,
            Role::Launch,
            None,
            facts(Standing::Same),
            Entry::Run,
        );
        check(
            "the installed service serves",
            &installed,
            Role::Serve,
            None,
            facts(Standing::Same),
            Entry::Run,
        );
        check(
            "a carrier serving is refused",
            &carrier,
            Role::Serve,
            None,
            facts(Standing::Same),
            Entry::Refuse(Refusal::ServeFromElsewhere),
        );
        check(
            "a development build serving without its marker is refused",
            &development,
            Role::Serve,
            None,
            facts(Standing::Same),
            Entry::Refuse(Refusal::ServeFromElsewhere),
        );
        check(
            "an absent install is installed by the carrier",
            &carrier,
            Role::Launch,
            None,
            facts(Standing::Absent),
            Entry::CarrierInstall,
        );
        check(
            "an older install is installed over by the carrier",
            &carrier,
            Role::Launch,
            None,
            facts(Standing::Older),
            Entry::CarrierInstall,
        );
        check(
            "a damaged install is installed over by the carrier",
            &carrier,
            Role::Launch,
            None,
            facts(Standing::Damaged),
            Entry::CarrierInstall,
        );
        check(
            "the same install is run, not reinstalled",
            &carrier,
            Role::Launch,
            None,
            facts(Standing::Same),
            Entry::Forward(installed.clone()),
        );
        check(
            "a newer install is run, not downgraded",
            &carrier,
            Role::Launch,
            None,
            facts(Standing::Newer),
            Entry::Forward(installed.clone()),
        );
        check(
            "inspection never installs",
            &carrier,
            Role::Inspect,
            None,
            facts(Standing::Absent),
            Entry::Run,
        );
        check(
            "the marker forwards the installed service",
            &installed,
            Role::Serve,
            Some(marker()),
            facts(Standing::Same),
            Entry::Forward(development.clone()),
        );
        check(
            "the marker forwards a carrier",
            &carrier,
            Role::Launch,
            Some(marker()),
            facts(Standing::Absent),
            Entry::Forward(development.clone()),
        );
        check(
            "the marker forwards inspection",
            &installed,
            Role::Inspect,
            Some(marker()),
            facts(Standing::Same),
            Entry::Forward(development.clone()),
        );
        check(
            "the marked build runs every role",
            &development,
            Role::Serve,
            Some(marker()),
            facts(Standing::Absent),
            Entry::Run,
        );
        check(
            "the marked build runs a launch",
            &development,
            Role::Launch,
            Some(marker()),
            facts(Standing::Absent),
            Entry::Run,
        );
        check(
            "a marker naming a deleted build is ignored",
            &installed,
            Role::Serve,
            Some(marker()),
            missing_build.clone(),
            Entry::Run,
        );
        check(
            "a carrier ignores a marker naming a deleted build",
            &carrier,
            Role::Launch,
            Some(marker()),
            missing_build,
            Entry::Forward(installed.clone()),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_without_regard_to_case() {
        let shouted = PathBuf::from(ROOT.to_uppercase())
            .join("BIN")
            .join("GHOSTLIGHT");
        assert_eq!(
            decide(&shouted, Role::Serve, None, &facts(Standing::Same)),
            Entry::Run
        );
    }
}
