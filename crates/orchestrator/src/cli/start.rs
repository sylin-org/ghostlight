//! The entry rule performed: what `main` does before it dispatches a command.
//!
//! [`start`] gathers the facts [`crate::install::entry::decide`] needs from the Ghostlight home,
//! asks it, and carries the answer out through a [`Launcher`]: run here, forward to the
//! development build or the installed service, hand the edge's own verbs to the installed edge,
//! install from a carrier package first, or refuse to serve (ADR-0197 Decisions 10, 13 and 15,
//! ADR-0199 Decision 2). The launcher is the only part that starts processes, so tests drive the
//! real rule with a fake one.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use ghostlight_relay::os::session;
use ghostlight_relay::protocol::StartReason;

use crate::cli::parse::{self, LaunchMode};
use crate::install::entry::{self, canonical_path, Entry, Facts, Role};
use crate::install::layout::Layout;
use crate::install::marker::DevelopmentMarker;
use crate::install::record::{self, InstallRecord, Standing};

/// Exit code of a forwarded process that ended without one (killed by a signal).
const EXIT_WITHOUT_CODE: i32 = 1;

/// What `main` does after the entry rule.
#[derive(Debug, Eq, PartialEq)]
pub enum Start {
    /// Run the parsed command in this process.
    Dispatch(LaunchMode),
    /// Another executable did the job; exit with this code.
    Exit(i32),
}

/// The side effects of the entry rule: starting other executables and installing.
pub trait Launcher {
    /// Run `executable` with `arguments` and inherited standard streams, wait for it, and
    /// return its exit code.
    fn run(&self, executable: &Path, arguments: &[OsString]) -> io::Result<i32>;
    /// Start `executable serve --reason <reason>` detached with `home` as its working directory,
    /// without waiting.
    fn serve_detached(&self, executable: &Path, home: &Path, reason: StartReason)
        -> io::Result<()>;
    /// Install this executable's package as a release into the home of `layout`.
    fn install(&self, layout: &Layout) -> Result<()>;
}

/// The launcher `main` uses: real processes and the real installer.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessLauncher;

impl Launcher for ProcessLauncher {
    fn run(&self, executable: &Path, arguments: &[OsString]) -> io::Result<i32> {
        if session::session_ending() {
            return Err(io::Error::other(session::START_REFUSED));
        }
        let status = Command::new(executable).args(arguments).status()?;
        Ok(status.code().unwrap_or(EXIT_WITHOUT_CODE))
    }

    fn serve_detached(
        &self,
        executable: &Path,
        home: &Path,
        reason: StartReason,
    ) -> io::Result<()> {
        ghostlight_relay::spawn::spawn_serve(executable, home, reason).map(|_| ())
    }

    fn install(&self, layout: &Layout) -> Result<()> {
        crate::install::deployment::install_carrier(layout)
    }
}

/// Apply the entry rule to a process at `current_exe` started with `arguments`.
///
/// `layout` is the Ghostlight home. A command that does not parse is never installed for: it
/// still forwards while a development build is marked, so a newer build can answer it, and is
/// otherwise reported here.
pub fn start(
    arguments: Vec<OsString>,
    current_exe: &Path,
    layout: &Layout,
    launcher: &dyn Launcher,
) -> Result<Start> {
    let parsed = parse::launch_mode(arguments.iter().cloned());
    let role = parsed.as_ref().map_or(Role::Inspect, role_of);
    let current_exe = canonical_path(current_exe);
    let installed_service = canonical_path(&layout.service_executable());
    let installed_edge = canonical_path(&layout.edge());
    let marker = DevelopmentMarker::read(&layout.development_marker())
        .ok()
        .flatten()
        .map(|marker| DevelopmentMarker {
            executable: canonical_path(&marker.executable),
        });
    // Only a launch from a copy other than the installed service consults the standing, so the
    // installed image is hashed only then.
    let launching = matches!(role, Role::Launch | Role::Relay);
    let standing = if launching && !entry::same_path(&current_exe, &installed_service) {
        standing(layout, &current_exe)?
    } else {
        Standing::Same
    };
    let facts = Facts {
        installed_service,
        installed_edge,
        development_executable_exists: marker
            .as_ref()
            .is_some_and(|marker| marker.executable.is_file()),
        standing,
    };
    match entry::decide(&current_exe, role, marker.as_ref(), &facts) {
        Entry::Run => Ok(Start::Dispatch(parsed?)),
        Entry::Forward(target) => match parsed {
            Ok(LaunchMode::Serve(reason)) => {
                launcher.serve_detached(&target, layout.home(), reason)?;
                Ok(Start::Exit(0))
            }
            _ => Ok(Start::Exit(launcher.run(&target, &arguments)?)),
        },
        Entry::CarrierInstall => {
            launcher.install(layout)?;
            let installed = if role == Role::Relay {
                layout.edge()
            } else {
                layout.service_executable()
            };
            Ok(Start::Exit(launcher.run(&installed, &arguments)?))
        }
        Entry::Refuse(refusal) => Err(refusal.into()),
    }
}

/// The entry role of a parsed command.
fn role_of(mode: &LaunchMode) -> Role {
    match mode {
        LaunchMode::DeploymentInstall(_) => Role::Deployment,
        LaunchMode::Serve(_) => Role::Serve,
        LaunchMode::Relay => Role::Relay,
        LaunchMode::Doctor { .. }
        | LaunchMode::Status { .. }
        | LaunchMode::Help
        | LaunchMode::Version => Role::Inspect,
        _ => Role::Launch,
    }
}

/// How the installation in `layout` stands against the package `current_exe` belongs to.
fn standing(layout: &Layout, current_exe: &Path) -> io::Result<Standing> {
    let installed = InstallRecord::read(&layout.install_record())?;
    let installed_image = match record::sha256_file(&layout.service_executable()) {
        Ok(sha) => Some(sha),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let payload = InstallRecord {
        service_version: env!("CARGO_PKG_VERSION").to_owned(),
        relay_revision: ghostlight_relay::RELAY.to_string(),
        service_sha256: record::sha256_file(current_exe)?,
        edge_sha256: None,
    };
    Ok(record::decide(
        installed.as_ref(),
        &payload,
        installed_image.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::install::scratch::Scratch;

    /// Exit code the fake launcher reports for every run.
    const FORWARDED_EXIT: i32 = 7;

    /// What the fake launcher was asked to do.
    #[derive(Debug, Eq, PartialEq)]
    enum Call {
        Run(PathBuf, Vec<OsString>),
        Serve(PathBuf, StartReason),
        Install(PathBuf),
    }

    #[derive(Default)]
    struct FakeLauncher(RefCell<Vec<Call>>);

    impl Launcher for FakeLauncher {
        fn run(&self, executable: &Path, arguments: &[OsString]) -> io::Result<i32> {
            self.0
                .borrow_mut()
                .push(Call::Run(executable.to_path_buf(), arguments.to_vec()));
            Ok(FORWARDED_EXIT)
        }

        fn serve_detached(
            &self,
            executable: &Path,
            _home: &Path,
            reason: StartReason,
        ) -> io::Result<()> {
            self.0
                .borrow_mut()
                .push(Call::Serve(executable.to_path_buf(), reason));
            Ok(())
        }

        fn install(&self, layout: &Layout) -> Result<()> {
            self.0.borrow_mut().push(Call::Install(layout.bin()));
            Ok(())
        }
    }

    /// A temporary Ghostlight home with a carrier package outside it.
    struct Home {
        scratch: Scratch,
        layout: Layout,
    }

    impl Home {
        fn new() -> Self {
            let scratch = Scratch::new();
            let layout = Layout::new(scratch.path().join("home"));
            fs::create_dir_all(layout.bin()).unwrap();
            Self { scratch, layout }
        }

        fn executable(&self, path: PathBuf, bytes: &[u8]) -> PathBuf {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            canonical_path(&path)
        }

        fn carrier(&self) -> PathBuf {
            self.executable(
                self.scratch
                    .path()
                    .join("package")
                    .join("ghostlight-service"),
                b"carrier",
            )
        }

        fn installed(&self) -> PathBuf {
            self.executable(self.layout.service_executable(), b"installed")
        }

        fn edge(&self) -> PathBuf {
            self.executable(self.layout.edge(), b"edge")
        }

        fn development(&self) -> PathBuf {
            let development = self.executable(self.layout.development_executable(), b"development");
            DevelopmentMarker {
                executable: development.clone(),
            }
            .write(&self.layout.development_marker())
            .unwrap();
            development
        }

        fn record(&self, version: &str) {
            InstallRecord {
                service_version: version.to_owned(),
                relay_revision: ghostlight_relay::RELAY.to_string(),
                service_sha256: record::sha256_file(&self.layout.service_executable()).unwrap(),
                edge_sha256: None,
            }
            .write(&self.layout.install_record())
            .unwrap();
        }

        fn start(&self, current_exe: &Path, arguments: &[&str]) -> (Result<Start>, Vec<Call>) {
            let launcher = FakeLauncher::default();
            let result = start(
                arguments.iter().map(OsString::from).collect(),
                current_exe,
                &self.layout,
                &launcher,
            );
            (result, launcher.0.into_inner())
        }
    }

    fn os(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_marked_development_build_receives_every_command_with_the_same_arguments() {
        let home = Home::new();
        let carrier = home.carrier();
        let development = home.development();
        let (result, calls) = home.start(&carrier, &["call", "--catalog"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(
            calls,
            vec![Call::Run(development.clone(), os(&["call", "--catalog"]))]
        );
        let (result, calls) = home.start(&development, &["call", "--catalog"]);
        assert!(matches!(result.unwrap(), Start::Dispatch(LaunchMode::Call)));
        assert!(calls.is_empty());
    }

    #[test]
    fn serve_is_forwarded_detached_with_its_reason_and_the_parent_exits_zero() {
        let home = Home::new();
        let installed = home.installed();
        let development = home.development();
        for reason in [
            StartReason::Agent,
            StartReason::Browser,
            StartReason::Update,
        ] {
            let (result, calls) = home.start(&installed, &["serve", "--reason", reason.as_str()]);
            assert_eq!(result.unwrap(), Start::Exit(0));
            assert_eq!(calls, vec![Call::Serve(development.clone(), reason)]);
        }
    }

    #[test]
    fn serve_without_a_reason_is_a_usage_error_the_development_build_reports() {
        let home = Home::new();
        let installed = home.installed();
        let development = home.development();
        let (result, calls) = home.start(&installed, &["serve"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(calls, vec![Call::Run(development, os(&["serve"]))]);
    }

    #[test]
    fn a_carrier_installs_when_nothing_is_installed_and_then_runs_the_installed_service() {
        let home = Home::new();
        let carrier = home.carrier();
        let (result, calls) = home.start(&carrier, &["open"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(
            calls,
            vec![
                Call::Install(home.layout.bin()),
                Call::Run(home.layout.service_executable(), os(&["open"])),
            ]
        );
    }

    #[test]
    fn a_carrier_installs_over_an_older_service_and_hands_over_to_the_same_one() {
        let home = Home::new();
        let carrier = home.carrier();
        let installed = home.installed();
        home.record("0.0.1");
        let (result, calls) = home.start(&carrier, &["install"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(calls[0], Call::Install(home.layout.bin()));
        home.record(env!("CARGO_PKG_VERSION"));
        let (result, calls) = home.start(&carrier, &["install"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(calls, vec![Call::Run(installed, os(&["install"]))]);
    }

    #[test]
    fn a_carrier_older_than_the_installed_service_hands_over_without_installing() {
        let home = Home::new();
        let carrier = home.carrier();
        let installed = home.installed();
        home.record("999.0.0");
        let (result, calls) = home.start(&carrier, &["open"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(calls, vec![Call::Run(installed, os(&["open"]))]);
    }

    #[test]
    fn the_edges_verbs_go_to_the_installed_edge_from_any_service_image() {
        let home = Home::new();
        let carrier = home.carrier();
        let installed = home.installed();
        let edge = home.edge();
        home.record(env!("CARGO_PKG_VERSION"));
        let development = home.development();
        for arguments in [
            &["mcp"][..],
            &["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"],
            &[
                "chrome-extension://abcdefghijklmnopabcdefghijklmnop/",
                "--parent-window=4",
            ],
        ] {
            for current in [&carrier, &installed, &development] {
                let (result, calls) = home.start(current, arguments);
                assert_eq!(
                    result.unwrap(),
                    Start::Exit(FORWARDED_EXIT),
                    "{arguments:?}"
                );
                assert_eq!(
                    calls,
                    vec![Call::Run(edge.clone(), os(arguments))],
                    "{arguments:?}"
                );
            }
        }
    }

    #[test]
    fn a_carrier_asked_for_a_relay_installs_first_and_then_runs_the_installed_edge() {
        let home = Home::new();
        let carrier = home.carrier();
        let (result, calls) = home.start(&carrier, &["mcp"]);
        assert_eq!(result.unwrap(), Start::Exit(FORWARDED_EXIT));
        assert_eq!(
            calls,
            vec![
                Call::Install(home.layout.bin()),
                Call::Run(home.layout.edge(), os(&["mcp"])),
            ]
        );
    }

    #[test]
    fn inspection_never_installs() {
        let home = Home::new();
        let carrier = home.carrier();
        for arguments in [
            &["doctor"][..],
            &["status"],
            &["--help"],
            &["help"],
            &["--version"],
        ] {
            let (result, calls) = home.start(&carrier, arguments);
            assert!(
                matches!(result.unwrap(), Start::Dispatch(_)),
                "{arguments:?}"
            );
            assert!(calls.is_empty(), "{arguments:?}");
        }
        let (result, calls) = home.start(&carrier, &["nonsense"]);
        assert!(result.is_err());
        assert!(calls.is_empty());
    }

    #[test]
    fn serve_from_any_other_path_is_refused() {
        let home = Home::new();
        let carrier = home.carrier();
        home.installed();
        let (result, calls) = home.start(&carrier, &["serve", "--reason", "agent"]);
        let error = result.unwrap_err();
        assert!(error.is::<entry::Refusal>(), "{error}");
        assert!(calls.is_empty());
    }

    #[test]
    fn the_installed_service_runs_its_own_commands() {
        let home = Home::new();
        let installed = home.installed();
        for (arguments, mode) in [
            (
                &["serve", "--reason", "person"][..],
                LaunchMode::Serve(StartReason::Person),
            ),
            (&["open"], LaunchMode::Open),
        ] {
            let (result, calls) = home.start(&installed, arguments);
            assert_eq!(result.unwrap(), Start::Dispatch(mode));
            assert!(calls.is_empty());
        }
    }
}
