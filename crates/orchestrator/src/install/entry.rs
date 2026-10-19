//! The entry rule: what a `ghostlight` process does before it does its job.
//!
//! Any copy of the service image can be started: from `bin`, or from a package or a build
//! directory that only carries it. [`decide`] answers, from the process's own path, its role and
//! the state of the installation, whether to run, hand over to another executable, install first,
//! or refuse (ADR-0197 Decisions 10 and 13, ADR-0199 Decisions 2 and 19 and the development
//! channel amendment). It is pure: the caller gathers the facts and performs the answer. Links
//! are not followed here, so the caller resolves every path it passes with [`canonical_path`]
//! first.
//!
//! The paths it compares are compared by Ghostlight's one same-location rule, which lives here
//! for every installer part (ADR-0203 Decision 11.2): [`same_path`] by spelling,
//! [`same_location`] once links are resolved, and [`lies_within`] for a path below a directory.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::record::Standing;

/// What a process was started to do, as far as the entry rule cares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    /// `deployment`: the installer itself. It runs from wherever its payload is.
    Deployment,
    /// `serve`: the service an edge starts.
    Serve,
    /// `mcp` and a Chrome extension origin: the edge's own verbs. A service image never runs
    /// them; it hands them to the installed edge, installing first when it is a carrier.
    Relay,
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
    /// The fixed service executable, `home/bin/ghostlight-service`.
    pub installed_service: PathBuf,
    /// The fixed edge, `home/bin/ghostlight`.
    pub installed_edge: PathBuf,
    /// Whether the installed record is the development channel. A copy that is not the installed
    /// service never installs over it.
    pub development_channel: bool,
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
    /// place, or the installed edge for the edge's own verbs.
    CarrierInstall,
    /// Do not run.
    Refuse(Refusal),
}

/// Why a process may not run.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum Refusal {
    /// Only the installed service serves, so there is one service per user by construction.
    #[error("only the installed Ghostlight service can serve")]
    ServeFromElsewhere,
}

/// Decide what the process at `current_exe` does for `role`.
///
/// `current_exe` and `facts.installed_service` must both be canonical ([`canonical_path`]). A
/// process never forwards to itself: a target that names the running executable ends in
/// [`Entry::Run`].
///
/// 1. The installer always runs.
/// 2. Only the installed service serves.
/// 3. Inspection runs from anywhere.
/// 4. Any other launch from a copy that is not the installed service is a carrier. It installs
///    when the installed set is absent, older or damaged, unless the installed set is the
///    development channel, which a carrier never installs over. Otherwise it hands over: the
///    edge's own verbs to the installed edge, everything else to the installed service.
#[must_use]
pub fn decide(current_exe: &Path, role: Role, facts: &Facts) -> Entry {
    if role == Role::Deployment {
        return Entry::Run;
    }
    let installed = same_path(current_exe, &facts.installed_service);
    let needs_install = matches!(
        facts.standing,
        Standing::Absent | Standing::Older | Standing::Damaged
    ) && !installed
        && !facts.development_channel;
    match role {
        Role::Deployment | Role::Inspect => Entry::Run,
        Role::Serve if installed => Entry::Run,
        Role::Serve => Entry::Refuse(Refusal::ServeFromElsewhere),
        Role::Relay if needs_install => Entry::CarrierInstall,
        Role::Relay => Entry::Forward(facts.installed_edge.clone()),
        Role::Launch if needs_install => Entry::CarrierInstall,
        Role::Launch => forward_unless_self(current_exe, &facts.installed_service),
    }
}

/// The path `path` names once links, `.` and `..` are resolved, without the `\?\` prefix
/// Windows puts on resolved paths. A path that cannot be resolved is returned unchanged.
///
/// This is how the entry rule's inputs are made comparable: the running executable (which Linux
/// reports with links resolved) and the installed service both go through it before [`decide`]
/// sees them.
#[must_use]
pub fn canonical_path(path: &Path) -> PathBuf {
    ghostlight_relay::home::canonical(path).unwrap_or_else(|_| path.to_path_buf())
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

/// Whether two paths are spelled as one path on this system: [`spelled_alike`] for the system
/// this build runs on. For paths already resolved with [`canonical_path`], such as the entry
/// rule's executables; [`same_location`] resolves them itself.
#[must_use]
pub fn same_path(left: &Path, right: &Path) -> bool {
    spelled_alike(left, right, cfg!(windows))
}

/// Whether two paths are spelled as one path on Windows (`windows`) or on another system.
///
/// On Windows the comparison ignores case, the difference between `/` and `\`, a trailing
/// separator and the verbatim prefix. Elsewhere it compares the paths' components, so a trailing
/// or doubled `/` and an inner `.` do not matter and case does.
#[must_use]
pub(crate) fn spelled_alike(left: &Path, right: &Path, windows: bool) -> bool {
    if windows {
        windows_spelling(left) == windows_spelling(right)
    } else {
        left == right
    }
}

/// Whether two paths name the same location: Ghostlight's one same-location rule (ADR-0203
/// Decision 11.2), for the system this build runs on.
///
/// They do when they are [`spelled_alike`], or when they are once links, `.` and `..` are
/// resolved ([`canonical_path`]). A path that cannot be resolved is compared as it is spelled.
#[must_use]
pub fn same_location(left: &Path, right: &Path) -> bool {
    same_location_on(left, right, cfg!(windows))
}

/// [`same_location`] for paths of Windows (`windows`) or of another system.
#[must_use]
pub(crate) fn same_location_on(left: &Path, right: &Path, windows: bool) -> bool {
    spelled_alike(left, right, windows)
        || spelled_alike(&canonical_path(left), &canonical_path(right), windows)
}

/// Whether `path` is `directory` or lies below it, spelled as [`spelled_alike`] reads paths of
/// Windows (`windows`) or of another system. Links are not resolved.
#[must_use]
pub(crate) fn lies_within(path: &Path, directory: &Path, windows: bool) -> bool {
    if !windows {
        return path.starts_with(directory);
    }
    let path = windows_spelling(path);
    let directory = windows_spelling(directory);
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|rest| rest.starts_with('\\'))
}

/// The key every Windows spelling of one path shares: backslashes, no verbatim prefix, no
/// trailing separator, lowercase.
fn windows_spelling(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    strip_verbatim(&text).trim_end_matches('\\').to_lowercase()
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
    use std::fs;

    use super::*;
    use crate::scratch::Scratch;

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
            installed_service: path("bin/ghostlight-service"),
            installed_edge: path("bin/ghostlight"),
            development_channel: false,
            standing,
        }
    }

    fn development(standing: Standing) -> Facts {
        Facts {
            development_channel: true,
            ..facts(standing)
        }
    }

    fn check(name: &str, current_exe: &Path, role: Role, facts: Facts, expected: Entry) {
        assert_eq!(decide(current_exe, role, &facts), expected, "{name}");
    }

    #[test]
    fn decisions_follow_the_entry_rule() {
        let installed = path("bin/ghostlight-service");
        let edge = path("bin/ghostlight");
        let build = path("target/release/ghostlight-service");
        let carrier = path("package/ghostlight-service");
        check(
            "the installer runs from a carrier",
            &carrier,
            Role::Deployment,
            facts(Standing::Absent),
            Entry::Run,
        );
        check(
            "the installer runs over a development channel",
            &carrier,
            Role::Deployment,
            development(Standing::Same),
            Entry::Run,
        );
        check(
            "the installed service runs",
            &installed,
            Role::Launch,
            facts(Standing::Same),
            Entry::Run,
        );
        check(
            "the installed service serves",
            &installed,
            Role::Serve,
            facts(Standing::Same),
            Entry::Run,
        );
        check(
            "the installed service serves a development channel",
            &installed,
            Role::Serve,
            development(Standing::Same),
            Entry::Run,
        );
        check(
            "a carrier serving is refused",
            &carrier,
            Role::Serve,
            facts(Standing::Same),
            Entry::Refuse(Refusal::ServeFromElsewhere),
        );
        check(
            "a build directory serving is refused whatever the channel",
            &build,
            Role::Serve,
            development(Standing::Same),
            Entry::Refuse(Refusal::ServeFromElsewhere),
        );
        check(
            "an absent install is installed by the carrier",
            &carrier,
            Role::Launch,
            facts(Standing::Absent),
            Entry::CarrierInstall,
        );
        check(
            "an older install is installed over by the carrier",
            &carrier,
            Role::Launch,
            facts(Standing::Older),
            Entry::CarrierInstall,
        );
        check(
            "a damaged install is installed over by the carrier",
            &carrier,
            Role::Launch,
            facts(Standing::Damaged),
            Entry::CarrierInstall,
        );
        check(
            "the same install is run, not reinstalled",
            &carrier,
            Role::Launch,
            facts(Standing::Same),
            Entry::Forward(installed.clone()),
        );
        check(
            "a newer install is run, not downgraded",
            &carrier,
            Role::Launch,
            facts(Standing::Newer),
            Entry::Forward(installed.clone()),
        );
        check(
            "inspection never installs",
            &carrier,
            Role::Inspect,
            facts(Standing::Absent),
            Entry::Run,
        );
        for (name, standing) in [
            ("absent", Standing::Absent),
            ("older", Standing::Older),
            ("damaged", Standing::Damaged),
        ] {
            check(
                &format!("a carrier handed the edge's verbs installs over an {name} install"),
                &carrier,
                Role::Relay,
                facts(standing),
                Entry::CarrierInstall,
            );
        }
        for standing in [Standing::Same, Standing::Newer] {
            check(
                "a carrier hands the edge's verbs to the installed edge",
                &carrier,
                Role::Relay,
                facts(standing),
                Entry::Forward(edge.clone()),
            );
        }
        check(
            "the installed service hands the edge's verbs to the installed edge",
            &installed,
            Role::Relay,
            facts(Standing::Same),
            Entry::Forward(edge.clone()),
        );
    }

    #[test]
    fn a_carrier_never_installs_over_a_development_channel() {
        let installed = path("bin/ghostlight-service");
        let edge = path("bin/ghostlight");
        let carrier = path("package/ghostlight-service");
        for standing in [
            Standing::Absent,
            Standing::Older,
            Standing::Damaged,
            Standing::Same,
            Standing::Newer,
        ] {
            check(
                "a carrier runs the installed service",
                &carrier,
                Role::Launch,
                development(standing),
                Entry::Forward(installed.clone()),
            );
            check(
                "a carrier hands the edge's verbs to the installed edge",
                &carrier,
                Role::Relay,
                development(standing),
                Entry::Forward(edge.clone()),
            );
        }
        check(
            "a build directory runs the installed service",
            &path("target/release/ghostlight-service"),
            Role::Launch,
            development(Standing::Older),
            Entry::Forward(installed),
        );
        check(
            "the installed service hands the edge's verbs to the installed edge",
            &path("bin/ghostlight-service"),
            Role::Relay,
            development(Standing::Same),
            Entry::Forward(edge),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_compare_without_regard_to_spelling() {
        let shouted = PathBuf::from(ROOT.to_uppercase())
            .join("BIN")
            .join("GHOSTLIGHT-SERVICE");
        let slashed = PathBuf::from(format!(
            "{}/bin/ghostlight-service",
            ROOT.replace('\\', "/")
        ));
        let verbatim = PathBuf::from(format!("\\\\?\\{ROOT}\\bin\\ghostlight-service"));
        let trailing = PathBuf::from(format!("{ROOT}\\bin\\ghostlight-service\\"));
        for spelling in [&shouted, &slashed, &verbatim, &trailing] {
            assert_eq!(
                decide(spelling, Role::Serve, &facts(Standing::Same)),
                Entry::Run,
                "{spelling:?}"
            );
        }
        check(
            "a service named in another spelling is never forwarded to",
            &path("bin/ghostlight-service"),
            Role::Launch,
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
        fs::write(real.join("bin").join("ghostlight-service"), b"x").unwrap();
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let running = canonical_path(&real.join("bin").join("ghostlight-service"));
        let facts = Facts {
            installed_service: canonical_path(&link.join("bin").join("ghostlight-service")),
            installed_edge: canonical_path(&link.join("bin").join("ghostlight")),
            development_channel: false,
            standing: Standing::Same,
        };
        assert_eq!(decide(&running, Role::Serve, &facts), Entry::Run);
        assert_eq!(decide(&running, Role::Launch, &facts), Entry::Run);
    }
}
