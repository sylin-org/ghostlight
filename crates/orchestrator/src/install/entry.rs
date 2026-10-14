//! The entry rule: what a `ghostlight` process does before it does its job.
//!
//! Any copy of the executable can be started: from `bin`, from a development build, or from a
//! package that only carries it. [`decide`] answers, from the process's own path, its role, the
//! development marker and the state of the installation, whether to run, hand over to another
//! executable, install first, or refuse (ADR-0197 Decisions 10, 13 and 15). It is pure: the
//! caller gathers the facts and performs the answer. Links are not followed here, so the caller
//! resolves every path it passes with [`canonical_path`] first.

use std::fs;
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
///
/// Every path is canonical, as [`canonical_path`] returns it.
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
/// `current_exe`, the marker's executable and `facts.installed_service` must all be canonical
/// ([`canonical_path`]). A process never forwards to itself: a target that names the running
/// executable ends in [`Entry::Run`].
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
        return forward_unless_self(current_exe, &marker.executable);
    }
    let installed = same_path(current_exe, &facts.installed_service);
    match role {
        Role::Deployment | Role::Inspect => Entry::Run,
        Role::Serve if installed => Entry::Run,
        Role::Serve => Entry::Refuse(Refusal::ServeFromElsewhere),
        Role::Launch => match facts.standing {
            Standing::Absent | Standing::Older | Standing::Damaged if !installed => {
                Entry::CarrierInstall
            }
            _ => forward_unless_self(current_exe, &facts.installed_service),
        },
    }
}

/// The path `path` names once links, `.` and `..` are resolved, without the `\?\` prefix
/// Windows puts on resolved paths. A path that cannot be resolved is returned unchanged.
///
/// This is how the entry rule's inputs are made comparable: the running executable (which Linux
/// reports with links resolved), the installed service and the marker's executable all go
/// through it before [`decide`] sees them.
#[must_use]
pub fn canonical_path(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(resolved) if cfg!(windows) => PathBuf::from(strip_verbatim(&resolved.to_string_lossy())),
        Ok(resolved) => resolved,
        Err(_) => path.to_path_buf(),
    }
}

/// Forward to `target`, or run when `target` is the running executable, so no spelling of a path
/// can make a process start itself again.
fn forward_unless_self(current_exe: &Path, target: &Path) -> Entry {
    if same_path(current_exe, target) {
        Entry::Run
    } else {
        Entry::Forward(target.to_path_buf())
    }
}

/// Whether two canonical executable paths name the same file. On Windows the comparison ignores
/// ASCII case, the difference between `/` and `\`, a trailing separator and the verbatim prefix.
#[must_use]
pub fn same_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        comparable(left) == comparable(right)
    } else {
        left == right
    }
}

/// The key two Windows spellings of one path share.
fn comparable(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    strip_verbatim(&text)
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

/// The text of a Windows path without its verbatim prefix.
fn strip_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(VERBATIM_UNC_PREFIX) {
        format!("\\\\{rest}")
    } else if let Some(rest) = text.strip_prefix(VERBATIM_PREFIX) {
        rest.to_owned()
    } else {
        text.to_owned()
    }
}

/// Prefix Windows puts on a resolved local path.
const VERBATIM_PREFIX: &str = "\\\\?\\";

/// Prefix Windows puts on a resolved network path.
const VERBATIM_UNC_PREFIX: &str = "\\\\?\\UNC\\";

#[cfg(test)]
mod tests {
    use super::super::scratch::Scratch;
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
    fn windows_paths_compare_without_regard_to_spelling() {
        let installed = path("bin/ghostlight");
        let shouted = PathBuf::from(ROOT.to_uppercase())
            .join("BIN")
            .join("GHOSTLIGHT");
        let slashed = PathBuf::from(format!("{}/bin/ghostlight", ROOT.replace('\\', "/")));
        let verbatim = PathBuf::from(format!("\\\\?\\{ROOT}\\bin\\ghostlight"));
        let trailing = PathBuf::from(format!("{ROOT}\\bin\\ghostlight\\"));
        for spelling in [&shouted, &slashed, &verbatim, &trailing] {
            assert_eq!(
                decide(spelling, Role::Serve, None, &facts(Standing::Same)),
                Entry::Run,
                "{spelling:?}"
            );
        }
        for spelling in [&shouted, &slashed, &verbatim] {
            let named_by_marker = DevelopmentMarker {
                executable: spelling.clone(),
            };
            check(
                "a marker naming the running build in another spelling runs it",
                &installed,
                Role::Launch,
                Some(named_by_marker),
                facts(Standing::Absent),
                Entry::Run,
            );
        }
        check(
            "a service named in another spelling is never forwarded to",
            &installed,
            Role::Launch,
            None,
            Facts {
                installed_service: shouted,
                ..facts(Standing::Same)
            },
            Entry::Run,
        );
    }

    #[test]
    fn verbatim_prefixes_are_stripped_from_resolved_windows_paths() {
        assert_eq!(strip_verbatim("\\\\?\\C:\\ghost\\bin"), "C:\\ghost\\bin");
        assert_eq!(
            strip_verbatim("\\\\?\\UNC\\server\\share\\bin"),
            "\\\\server\\share\\bin"
        );
        assert_eq!(strip_verbatim("C:\\ghost\\bin"), "C:\\ghost\\bin");
    }

    #[test]
    fn a_canonical_path_resolves_dots_and_keeps_an_unresolvable_path() {
        let scratch = Scratch::new();
        let bin = scratch.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let image = bin.join("ghostlight");
        fs::write(&image, b"x").unwrap();
        let roundabout = bin.join("..").join("bin").join("ghostlight");
        assert_eq!(canonical_path(&roundabout), canonical_path(&image));
        assert!(!canonical_path(&image)
            .to_string_lossy()
            .starts_with(VERBATIM_PREFIX));
        let absent = bin.join("absent");
        assert_eq!(canonical_path(&absent), absent);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_home_names_the_same_executable() {
        let scratch = Scratch::new();
        let real = scratch.path().join("real");
        fs::create_dir_all(real.join("bin")).unwrap();
        fs::write(real.join("bin").join("ghostlight"), b"x").unwrap();
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let running = canonical_path(&real.join("bin").join("ghostlight"));
        let facts = Facts {
            installed_service: canonical_path(&link.join("bin").join("ghostlight")),
            development_executable_exists: false,
            standing: Standing::Same,
        };
        assert_eq!(decide(&running, Role::Serve, None, &facts), Entry::Run);
        assert_eq!(decide(&running, Role::Launch, None, &facts), Entry::Run);
    }
}
