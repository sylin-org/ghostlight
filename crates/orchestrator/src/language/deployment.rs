//! The words `ghostlight deployment install` says: what it did, what it left alone, and why it
//! refused.
//!
//! The install code decides and the command line prints, and neither words anything itself
//! (ADR-0199, the development channel amendment). Every sentence of the report and the text of
//! every refusal a person can meet lives here, so the report says only what was observed and the
//! way back from a development build is named the same way wherever it is needed.

use crate::install::deployment::RESTORE_OPTION;
use crate::install::migration::{FlatpakPart, FlatpakResult, FlatpakStep};

/// Where the get scripts are published.
const GET_SCRIPT_ROOT: &str = "https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts";

/// The switch of `get.ps1` that installs its release over a development installation.
const GET_POWERSHELL_RESTORE_SWITCH: &str = "-Restore";

/// What the install says after it started the service.
pub const SERVICE_STARTED: &str = "The Ghostlight service started.";

/// The install lock was held by another install for the whole wait.
pub const INSTALL_LOCK_BUSY: &str = "Ghostlight installation is still changing; retry shortly";

/// The installer could not stop the programs of an older installation.
pub const OLD_PROGRAMS_WOULD_NOT_STOP: &str = "Could not stop the old Ghostlight programs";

/// Which build an install put in `bin`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placed {
    /// A build the development loop installed.
    Development,
    /// A release, installed or restored.
    Release,
}

impl Placed {
    /// The noun phrase the report uses for the build.
    const fn noun(self) -> &'static str {
        match self {
            Self::Development => "development build",
            Self::Release => "release",
        }
    }
}

/// The way back from a development build to a release, as a person can run it.
///
/// A release carrier installs its own package, and the staging of a carrier is gone once it
/// exits, so the commands name the carriers and not a file inside one.
#[must_use]
pub fn restore_route() -> String {
    format!(
        "To return to a release, run one of these:\n  \
         npx ghostlight deployment install {RESTORE_OPTION}\n  \
         PowerShell: & ([scriptblock]::Create((irm {GET_SCRIPT_ROOT}/get.ps1))) \
         {GET_POWERSHELL_RESTORE_SWITCH}\n  \
         macOS and Linux: curl -fsSL {GET_SCRIPT_ROOT}/get.sh | sh -s -- {RESTORE_OPTION}"
    )
}

/// A release package found a development build installed and left it alone.
#[must_use]
pub fn development_installed() -> String {
    format!(
        "The installed Ghostlight is a development build, so this package did not replace it. {}",
        restore_route()
    )
}

/// The record says a development build is installed and one of its executables is gone.
#[must_use]
pub fn development_executable_missing(executable: &str) -> String {
    format!(
        "The installed Ghostlight is a development build, but {executable} is missing. {}",
        restore_route()
    )
}

/// A payload is older than what is installed.
#[must_use]
pub fn payload_refused(installed: &str, payload: &str) -> String {
    format!(
        "Ghostlight {installed} is already installed and this package carries the older \
         {payload}, so nothing was changed"
    )
}

/// An executable is missing from the payload.
#[must_use]
pub fn payload_missing(path: &str) -> String {
    format!("The Ghostlight package is missing {path}")
}

/// The payload is the directory it would be installed into.
#[must_use]
pub fn payload_in_place(directory: &str) -> String {
    format!("Run the installation from the package's own directory, not from {directory}")
}

/// The payload's service is another build than the installer that was asked to install it.
#[must_use]
pub fn payload_other_build(directory: &str) -> String {
    format!(
        "{directory} holds another build of Ghostlight; run that build's own deployment install"
    )
}

/// The new service did not answer in time.
#[must_use]
pub fn service_did_not_start(bin: &str, seconds: u64) -> String {
    format!(
        "The Ghostlight service in {bin} did not start within {seconds} s. The installation \
         record was not written, so the next launch installs again"
    )
}

/// The files of an earlier Ghostlight installation were moved into the home.
#[must_use]
pub fn moved_earlier_installation(home: &str) -> String {
    format!("Moved the files of the earlier Ghostlight installation into {home}.")
}

/// The installed version is this one and nothing needed repair.
#[must_use]
pub fn already_installed(version: &str, bin: &str) -> String {
    format!("Ghostlight {version} is already installed in {bin}.")
}

/// An executable was placed in a directory.
#[must_use]
pub fn installed_file(name: &str, directory: &str) -> String {
    format!("Installed {name} in {directory}.")
}

/// A service that was observed answering is the one in `service`.
#[must_use]
pub fn serves_from(placed: Placed, service: &str) -> String {
    format!("The {} now serves from {service}.", placed.noun())
}

/// The files are in place and no service answered.
#[must_use]
pub fn placed_without_service(placed: Placed, service: &str) -> String {
    format!(
        "The {} is in place at {service}, but no service answered from it.",
        placed.noun()
    )
}

/// The directory earlier development builds ran from was removed.
#[must_use]
pub fn removed_development_directory(path: &str) -> String {
    format!("Removed {path}, where earlier development builds ran.")
}

/// The marker earlier development builds used to select a build was removed.
#[must_use]
pub fn removed_development_marker(path: &str) -> String {
    format!("Removed {path}, the marker earlier development builds used to pick a build.")
}

/// The directory an earlier 1.x Ghostlight kept one version of itself in was removed.
#[must_use]
pub fn removed_version_directory(path: &str) -> String {
    format!("Removed {path}, where an earlier Ghostlight kept one of its versions.")
}

/// The audit log of a removed 1.x version directory is kept in the state directory.
#[must_use]
pub fn archived_version_audit(archive: &str) -> String {
    format!("Kept its audit log as {archive}.")
}

/// An entry the install removes, such as the retired development layout or a 1.x version
/// directory, could not be removed. The cause is not known to the installer, so the sentence
/// does not name one.
#[must_use]
pub fn kept_entry(path: &str) -> String {
    format!("Could not remove {path}. The next install tries again.")
}

/// Browsers whose connection was registered.
#[must_use]
pub fn browsers_registered(browsers: &[String]) -> String {
    format!("Browser connection registered for {}.", browsers.join(", "))
}

/// A client's Ghostlight entry now starts the installed edge.
#[must_use]
pub fn client_rewritten(client: &str) -> String {
    format!("{client} now starts the installed Ghostlight edge.")
}

/// A client keeps an entry under Ghostlight's name that Ghostlight did not write.
#[must_use]
pub fn client_keeps_entry(client: &str, found: &str) -> String {
    format!("{client} keeps its own Ghostlight entry ({found}); it was not changed.")
}

/// A client has an entry under Ghostlight's name that Ghostlight cannot change.
#[must_use]
pub fn client_entry_unchangeable(client: &str) -> String {
    format!("{client} has a Ghostlight entry Ghostlight cannot change; it was not changed.")
}

/// Why a client's entry is not changed from here: the app container this process runs in, named
/// `app`, keeps the client's configuration private. Inspection says it as the client's detail and
/// every change Ghostlight is asked for there is refused with it.
#[must_use]
pub fn client_kept_private(app: &str) -> String {
    format!(
        "Windows keeps this configuration private to {app}, so a change made from here would not \
         reach other apps. To change it, run Ghostlight's install from a regular terminal."
    )
}

/// A platform entry was written.
#[must_use]
pub fn platform_entry_updated(entry: &str) -> String {
    format!("Updated the {entry}.")
}

/// A platform entry a dry run would write.
#[must_use]
pub fn platform_entry_would_update(entry: &str) -> String {
    format!("Would update the {entry}.")
}

/// A platform entry of this installation was removed.
#[must_use]
pub fn platform_entry_removed(entry: &str) -> String {
    format!("Removed the {entry}.")
}

/// A platform entry a dry run would remove.
#[must_use]
pub fn platform_entry_would_remove(entry: &str) -> String {
    format!("Would remove the {entry}.")
}

/// A platform entry that does not name this installation was left as it is.
#[must_use]
pub fn platform_entry_kept(entry: &str) -> String {
    format!("Kept the {entry}; it does not name this Ghostlight installation.")
}

/// A registration could not be removed.
#[must_use]
pub fn removal_failed(failure: &str) -> String {
    format!("Could not remove a registration: {failure}")
}

/// A registration could not be reconciled.
#[must_use]
pub fn registration_failed(failure: &str) -> String {
    format!("Could not update a registration: {failure}")
}

/// What became of one file the earlier Flatpak browser route left in the home. A step that
/// failed is the one a person should see as a problem: [`flatpak_failed`] says which.
#[must_use]
pub fn flatpak_line(step: &FlatpakStep) -> String {
    let what = match step.part {
        FlatpakPart::Manifest => "the Flatpak browser connection",
        FlatpakPart::Registration => "the Flatpak browser start-up registration",
        FlatpakPart::PermissionOverride => "the Flatpak browser permission",
        FlatpakPart::Custody => "the record of the Flatpak browser permission",
    };
    let path = step.path.display();
    match &step.result {
        FlatpakResult::Removed => format!("Removed {what} ({path}); Ghostlight no longer uses it."),
        FlatpakResult::Restored => format!("Restored {what} ({path}) to how it was before."),
        FlatpakResult::LeftForeign => {
            format!("Left {what} ({path}) as it is; it is not Ghostlight's.")
        }
        FlatpakResult::LeftChanged => {
            format!("Left {what} ({path}) as it is; it changed after Ghostlight set it.")
        }
        FlatpakResult::Failed(reason) => {
            format!("Could not update {what} ({path}): {reason}. The next install tries again.")
        }
    }
}

/// Whether `step` is one whose line reports a failure.
#[must_use]
pub fn flatpak_failed(step: &FlatpakStep) -> bool {
    matches!(step.result, FlatpakResult::Failed(_))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn the_way_back_names_each_release_carrier_with_its_own_switch() {
        let route = restore_route();
        assert!(route.contains("npx ghostlight deployment install --restore"));
        assert!(route.contains("get.ps1))) -Restore"));
        assert!(route.contains("get.sh | sh -s -- --restore"));
        assert!(!route.contains("ghostlight-service"));
    }

    #[test]
    fn a_missing_development_service_is_named_with_the_same_route() {
        let line = development_executable_missing("bin/ghostlight-service");
        assert!(line.contains("bin/ghostlight-service is missing"));
        assert!(line.ends_with(&restore_route()));
        assert!(development_installed().ends_with(&restore_route()));
    }

    #[test]
    fn a_service_that_did_not_answer_is_not_said_to_serve() {
        assert_eq!(
            serves_from(Placed::Development, "bin/s"),
            "The development build now serves from bin/s."
        );
        assert_eq!(
            serves_from(Placed::Release, "bin/s"),
            "The release now serves from bin/s."
        );
        assert!(!placed_without_service(Placed::Release, "bin/s").contains("serves"));
    }

    #[test]
    fn a_flatpak_failure_reads_as_a_failure() {
        let step = FlatpakStep {
            part: FlatpakPart::Manifest,
            path: PathBuf::from("m.json"),
            result: FlatpakResult::Failed("busy".into()),
        };
        assert!(flatpak_failed(&step));
        assert_eq!(
            flatpak_line(&step),
            "Could not update the Flatpak browser connection (m.json): busy. The next install tries again."
        );
    }

    #[test]
    fn every_sentence_is_ascii() {
        let sentences = [
            restore_route(),
            development_installed(),
            development_executable_missing("s"),
            payload_refused("2.1.0", "2.0.0"),
            payload_missing("p"),
            payload_in_place("d"),
            payload_other_build("d"),
            service_did_not_start("bin", 30),
            removed_development_marker("m"),
            removed_version_directory("d"),
            archived_version_audit("a"),
            kept_entry("m"),
            platform_entry_removed("e"),
            platform_entry_would_remove("e"),
            platform_entry_kept("e"),
            removal_failed("f"),
        ];
        assert!(sentences.iter().all(|sentence| sentence.is_ascii()));
    }
}
