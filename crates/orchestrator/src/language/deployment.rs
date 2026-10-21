//! The words the install side says: what `ghostlight deployment install`, `install`,
//! `uninstall`, `doctor` and `native-host` did, what they left alone and why they refused, and
//! the words for every registration's state, detail and evidence, for browsers and MCP clients
//! alike.
//!
//! The install code decides and the command line prints, and neither words anything itself
//! (ADR-0199, the development channel amendment; ADR-0203 Decision 8). Every sentence of the
//! reports and the text of every refusal a person can meet lives here, so a report says only what
//! was observed, one state is said with one word wherever it is printed, and the supported browser
//! families are named from the one browser table.

use std::path::Path;

use crate::install::command_path::{CommandPathReport, CommandPathState, OwnedEntry};
use crate::install::container::{
    checking_command, finishing_command, removing_command, AppContainer,
};
use crate::install::deployment::{EdgeReplacement, RESTORE_OPTION};
use crate::install::desktop_entry::{DesktopIntegrationReport, DesktopIntegrationState};
use crate::install::layout::Layout;
use crate::install::migration::{FlatpakPart, FlatpakResult, FlatpakStep};
use crate::install::native_host::{supported_browsers, BrowserRegistration, NativeHostState};
use crate::install::record::InstallRecord;
use crate::install::user_assets::UserAssetState;
use crate::install::{
    Attention, HarnessState, LauncherPin, NpmLauncher, ALL_CLIENTS_OPTION, FIRST_LAUNCHER_MAJOR,
};

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

/// What the install says for a folder Ghostlight 1.x left that it removed.
#[must_use]
pub fn leftover_removed(directory: &str) -> String {
    format!("Removed a folder Ghostlight 1.x left: {directory}")
}

/// What a whole uninstall's dry run says while Ghostlight runs.
pub const RUNNING_WOULD_STOP: &str = "Would stop the running Ghostlight.";

/// What an uninstall's dry run says of a browser whose Ghostlight registration it would remove.
#[must_use]
pub fn browser_would_remove(browser: &str) -> String {
    format!("Would remove Ghostlight's connection to {browser}.")
}

/// What an uninstall's dry run says of the `ghostlight` command entry it would remove.
#[must_use]
pub fn command_entry_would_remove(entry: &OwnedEntry) -> String {
    match entry {
        OwnedEntry::UserPath(directory) => {
            format!("Would remove {} from your PATH.", directory.display())
        }
        OwnedEntry::Link(link) => {
            format!("Would remove the ghostlight command at {}.", link.display())
        }
    }
}

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

/// What follows when the new service did not answer, said after each reason.
const NEXT_LAUNCH_INSTALLS_AGAIN: &str =
    "The installation record was not written, so the next launch installs again";

/// The new service ran but did not answer in time.
#[must_use]
pub fn service_did_not_start(bin: &str, seconds: u64) -> String {
    format!(
        "The Ghostlight service in {bin} did not start within {seconds} s. \
         {NEXT_LAUNCH_INSTALLS_AGAIN}"
    )
}

/// The new service's process could not be created, for the system's `reason`.
#[must_use]
pub fn service_could_not_start(bin: &str, reason: &str) -> String {
    format!(
        "The Ghostlight service in {bin} could not be started: {reason}. \
         {NEXT_LAUNCH_INSTALLS_AGAIN}"
    )
}

/// The new service stopped while starting, for the `reason` it recorded.
#[must_use]
pub fn service_stopped_because(bin: &str, reason: &str) -> String {
    format!(
        "The Ghostlight service in {bin} stopped while starting: {reason}. \
         {NEXT_LAUNCH_INSTALLS_AGAIN}"
    )
}

/// The new service stopped while starting and recorded no reason.
#[must_use]
pub fn service_stopped(bin: &str) -> String {
    format!("The Ghostlight service in {bin} stopped while starting. {NEXT_LAUNCH_INSTALLS_AGAIN}")
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

/// The install replaces the edge in `bin`, and why.
#[must_use]
pub fn edge_replaced(reason: EdgeReplacement) -> String {
    let because = match reason {
        EdgeReplacement::PreEdgeLayout => {
            "bin still holds an older layout that kept its connectors there"
        }
        EdgeReplacement::Unrecorded => "nothing records which edge is installed",
        EdgeReplacement::RelayRevision => "this build speaks another relay revision",
        EdgeReplacement::Missing => "the edge in bin is missing",
        EdgeReplacement::Altered => "the edge in bin is not the one the record names",
        EdgeReplacement::Rebuilt => "this development build changed it",
        EdgeReplacement::LeavingDevelopment => "the release replaces the development build's edge",
    };
    format!("The edge is replaced because {because}.")
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

/// What a dry run of an install would do for `client`, whose entry is in `state`, needing
/// `attention` for the cause inspection saw when it is in [`HarnessState::NeedsAttention`]. It
/// says what the real run would change, and that nothing would when nothing would.
#[must_use]
pub fn client_would_install(
    client: &str,
    state: HarnessState,
    attention: Option<Attention>,
) -> String {
    match state {
        HarnessState::NotDetected | HarnessState::Available => {
            format!("Would install Ghostlight for {client}.")
        }
        HarnessState::Updatable => format!("Would update Ghostlight's entry for {client}."),
        HarnessState::Installed => {
            format!("{client} already has the current Ghostlight entry; nothing would change.")
        }
        HarnessState::NeedsAttention => client_would_be_left(client, attention),
    }
}

/// What a dry run of a removal would do for `client`, whose own settings are in `state`, needing
/// `attention` for the cause inspection saw, and whose other settings copies at `owned_copies`
/// hold an entry Ghostlight owns. It says what Uninstall changes: the entry in the client's own
/// settings when it holds one, and each other copy by its path, so a change made in another copy
/// is never said as nothing. Nothing would change only when no copy holds Ghostlight's entry.
#[must_use]
pub fn client_would_remove(
    client: &str,
    state: HarnessState,
    attention: Option<Attention>,
    owned_copies: &[String],
) -> String {
    let here = state.holds_owned_entry();
    match (here, owned_copies.is_empty()) {
        (true, true) => format!("Would remove Ghostlight from {client}."),
        (true, false) => format!(
            "Would remove Ghostlight from {client} and from {}.",
            settings_copies("its", owned_copies)
        ),
        (false, false) => format!(
            "Would remove Ghostlight from {}.",
            settings_copies(&format!("{client}'s"), owned_copies)
        ),
        (false, true) if state == HarnessState::NeedsAttention => {
            client_would_be_left(client, attention)
        }
        (false, true) => format!("{client} has no Ghostlight entry; nothing would change."),
    }
}

/// The other copies of a client's settings at `copies`, whose owner is said as `owner` (`its`, or
/// the client's name in the possessive).
fn settings_copies(owner: &str, copies: &[String]) -> String {
    match copies {
        [copy] => format!("another copy of {owner} settings, at {copy}"),
        _ => format!(
            "other copies of {owner} settings, at {}",
            copies.join(" and ")
        ),
    }
}

/// A dry run leaves `client` as it is because it needs attention, for the cause inspection saw.
/// Without a known cause the line names none.
fn client_would_be_left(client: &str, attention: Option<Attention>) -> String {
    match attention {
        Some(Attention::Launcher(LauncherPin::Earlier)) => format!(
            "Would leave {client} as it is: its entry starts Ghostlight's npm launcher pinned to \
             a release before {FIRST_LAUNCHER_MAJOR}.0."
        ),
        Some(Attention::Launcher(LauncherPin::Unpinned | LauncherPin::Pinned)) => {
            format!("Would leave {client} as it is: its entry starts Ghostlight's npm launcher.")
        }
        Some(Attention::ForeignEntry) => format!(
            "Would leave {client} as it is: the entry under Ghostlight's name is not Ghostlight's."
        ),
        Some(Attention::Malformed) => {
            format!("Would leave {client} as it is: its settings file could not be parsed.")
        }
        Some(Attention::Unreadable) => {
            format!("Would leave {client} as it is: its settings file could not be read.")
        }
        None => format!("Would leave {client} as it is."),
    }
}

/// Why an install or an uninstall left an entry that starts Ghostlight's npm launcher.
pub const LAUNCHER_ENTRY_LEFT: &str = "the `ghostlight` entry starts Ghostlight's npm launcher; \
                                       Ghostlight left it as it is, and Fix points it at the \
                                       installed Ghostlight";

/// What a client's entry that starts Ghostlight's npm `launcher` is. Ghostlight cannot ask npm
/// what the launcher runs (ADR-0028), so the entry is never said to be current.
#[must_use]
pub fn launcher_detail(launcher: &NpmLauncher) -> String {
    let package = &launcher.package;
    match launcher.pin {
        LauncherPin::Unpinned => "This entry starts Ghostlight's npm launcher, and npm chooses \
                                  the version at each launch, so Ghostlight cannot say it is \
                                  current. It was left as it is."
            .into(),
        LauncherPin::Earlier => format!(
            "This entry starts Ghostlight's npm launcher pinned to {package}, a release before \
             {FIRST_LAUNCHER_MAJOR}.0 that cannot work with this Ghostlight. It was left as it is."
        ),
        LauncherPin::Pinned => format!(
            "This entry starts Ghostlight's npm launcher pinned to {package}. Ghostlight cannot \
             ask npm what that runs, so it cannot say it is current. It was left as it is."
        ),
    }
}

/// What inspection found where Ghostlight's entry belongs when it is the npm `launcher`, beside
/// the installed `edge` Ghostlight maintains there.
#[must_use]
pub fn launcher_evidence(launcher: &NpmLauncher, edge: &Path) -> String {
    format!(
        "Found Ghostlight's npm launcher, `npx {}`, where Ghostlight's entry belongs; Ghostlight \
         maintains `{}` there and changed nothing.",
        launcher.package,
        edge.display()
    )
}

/// Fix pointed `client`'s npm launcher entry at the installed Ghostlight.
#[must_use]
pub fn launcher_fixed(client: &str) -> String {
    format!(
        "{client} now starts the installed Ghostlight instead of the npm launcher. Restart or \
         reconnect it to load the tools."
    )
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

/// What `doctor` says of where a client's settings are when that is not simply the plain path:
/// `path`, where Ghostlight's entry goes, is the Microsoft Store edition's own copy when
/// `packaged`, and `other` is another copy of the same settings that exists. None for settings
/// found only at their plain path.
#[must_use]
pub fn client_settings_line(path: &str, packaged: bool, other: Option<&str>) -> Option<String> {
    let place = if packaged {
        format!("{path}, the Microsoft Store edition's own copy")
    } else {
        path.to_owned()
    };
    match other {
        Some(other) => Some(format!(
            "Ghostlight's entry goes in {place}; another copy of these settings is at {other}."
        )),
        None => packaged.then(|| format!("Settings: {place}.")),
    }
}

/// What a run inside the app container named `app` says of `client`, whose configuration that
/// container keeps private: the entry an install would write, or a removal would remove, is left
/// as it is, and the same command run from a regular terminal changes it.
#[must_use]
pub fn client_left_private(client: &str, app: &str, install: bool) -> String {
    let (left, act, command) = if install {
        ("is not written from here", "write", "install")
    } else {
        ("stays in place", "remove", "uninstall")
    };
    format!(
        "{client}: Windows keeps this configuration private to {app}, so its Ghostlight entry \
         {left}. To {act} it, run Ghostlight's {command} from a regular terminal."
    )
}

/// The refusal of a `--client` value that is neither an agent's id nor a host's, naming every
/// value accepted: the agent ids, then the host ids.
#[must_use]
pub fn unknown_client(value: &str, agents: &[&str], hosts: &[&str]) -> String {
    format!(
        "unknown MCP client '{value}'; expected an agent id ({}) or a host id ({})",
        agents.join(", "),
        hosts.join(", ")
    )
}

/// The refusal of a `--client` value that names the agent called `agent` when none of its hosts
/// is found here, naming each host's id, any of which selects that host, and the option that
/// selects every host.
#[must_use]
pub fn no_client_host_found(value: &str, agent: &str, hosts: &[&str]) -> String {
    format!(
        "no host of {agent} was found for '{value}'; name one by its host id ({}), or add \
         {ALL_CLIENTS_OPTION} to select every host of {agent}",
        hosts.join(", ")
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

/// Why a file Ghostlight writes as its own was left alone: a link sits where it belongs.
#[must_use]
pub fn link_refused(path: &str) -> String {
    format!("{path} is a link, and Ghostlight does not write its own files through a link")
}

/// Names joined as the list a sentence ends on: `a, b, c, or d`.
fn either_of(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_owned(),
        [first, second] => format!("{first} or {second}"),
        [head @ .., last] => format!("{}, or {last}", head.join(", ")),
    }
}

/// The ids of the supported browser families, as `--browser` accepts them and in the browser
/// table's order: `chrome, edge, brave, or chromium`.
#[must_use]
pub fn browser_ids() -> String {
    either_of(
        &supported_browsers()
            .iter()
            .map(|browser| browser.id)
            .collect::<Vec<_>>(),
    )
}

/// The supported browser families by their short names, in the browser table's order:
/// `Chrome, Edge, Brave, or Chromium`.
#[must_use]
pub fn browser_families() -> String {
    either_of(
        &supported_browsers()
            .iter()
            .map(|browser| browser.short)
            .collect::<Vec<_>>(),
    )
}

/// The refusal of a `--browser` value that names no supported family.
#[must_use]
pub fn unknown_browser(id: &str) -> String {
    format!("unknown browser '{id}'; expected {}", browser_ids())
}

/// The step that gives a computer a browser Ghostlight can start: one of the supported families,
/// installed as a native package.
#[must_use]
pub fn install_a_native_browser() -> String {
    format!("Install {} as a native package", browser_families())
}

/// What `--browser` selects, as the help names it.
#[must_use]
pub fn browser_option_help() -> String {
    format!("Select {}", browser_families())
}

/// The refusal of an install whose named browser is not installed, after the package's own
/// `detail`.
#[must_use]
pub fn browser_not_installed(detail: &str) -> String {
    format!(
        "{detail} {}, then run Ghostlight install again.",
        install_a_native_browser()
    )
}

/// The refusal of an install that found no supported browser installed as a native package.
#[must_use]
pub fn no_native_browser() -> String {
    format!(
        "No supported native browser was detected. {}, then run Ghostlight install again.",
        install_a_native_browser()
    )
}

/// The words a person reads for a browser's native-host registration in `state`. `doctor`, the
/// install's reports and `ghostlight native-host` all say a state with these words.
#[must_use]
pub const fn browser_state(state: NativeHostState) -> &'static str {
    match state {
        NativeHostState::Missing => "not registered",
        NativeHostState::Current => "registered",
        NativeHostState::Updatable => "registered, needs an update",
        NativeHostState::OwnedElsewhere => "registered to another installation",
        NativeHostState::NeedsAttention => "needs attention",
    }
}

/// What `doctor` says of a registration whose state only another app can see.
pub const NOT_VISIBLE: &str = "cannot be checked from here";

/// The line that names the Ghostlight program browsers start.
#[must_use]
pub fn program_line(program: &Path) -> String {
    format!("Ghostlight program: {}", program.display())
}

/// One browser's package, registration state and detail, as the reports print it.
#[must_use]
pub fn browser_line(browser: &BrowserRegistration) -> String {
    format!(
        "Browser: {} -- {} -- {} -- {}",
        browser.name,
        browser.package_detail,
        browser_state(browser.state),
        browser.detail
    )
}

/// One browser's line inside `container`: its package, and in place of its registration state
/// that the state cannot be checked from here, because the registry read is that app's own.
#[must_use]
pub fn browser_line_not_visible(browser: &BrowserRegistration, container: &AppContainer) -> String {
    format!(
        "Browser: {} -- {} -- {NOT_VISIBLE} -- {}",
        browser.name,
        browser.package_detail,
        registry_view_detail(container)
    )
}

/// What `ghostlight native-host` did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHostRun {
    /// It read every registration and changed nothing.
    Checked,
    /// It registered every browser it could.
    Installed,
    /// It wrote the manifest, and an app container kept the registry values it needed private.
    Prepared,
    /// It removed every registration Ghostlight owns.
    Uninstalled,
}

/// The first line `ghostlight native-host` prints: what it did, and whether a registration
/// changed.
#[must_use]
pub fn native_host_ran(run: NativeHostRun, changed: bool) -> String {
    let verb = match run {
        NativeHostRun::Checked => "checked",
        NativeHostRun::Installed => "installed",
        NativeHostRun::Prepared => "prepared",
        NativeHostRun::Uninstalled => "uninstalled",
    };
    format!("Ghostlight native host {verb}; changed: {changed}")
}

/// The lines an install prints when it left `registrations` unwritten because `container` keeps
/// them private.
#[must_use]
pub fn kept_private_lines(
    container: &AppContainer,
    registrations: &[String],
    layout: &Layout,
) -> Vec<String> {
    vec![
        format!(
            "Windows keeps these registrations private to {}, so no other app sees them: {}.",
            container.app,
            registrations.join(", ")
        ),
        format!(
            "To finish them, run this from a regular terminal: {}",
            finishing_command(layout)
        ),
    ]
}

/// The line `doctor` prints in place of a registration state it cannot see.
#[must_use]
pub fn cannot_see_line(container: &AppContainer, layout: &Layout) -> String {
    format!(
        "This terminal runs inside {}, and Windows gives that app its own view of the registry \
         and AppData, so Ghostlight cannot see what other apps see there. To check the real \
         registrations, run this from a regular terminal: {}",
        container.app,
        checking_command(layout)
    )
}

/// The line `doctor` prints for an MCP client whose configuration only another app can see.
#[must_use]
pub fn client_cannot_see_line(client: &str) -> String {
    format!(
        "MCP client: {client} -- {NOT_VISIBLE} -- Windows gives the app this terminal runs in \
         its own view of this configuration."
    )
}

/// What `doctor` and the install dry run say of a browser registration inside `container`, in
/// place of its state: the registry they read is that app's own.
#[must_use]
pub fn registry_view_detail(container: &AppContainer) -> String {
    format!(
        "{}'s own view of the registry; run ghostlight doctor from a regular terminal.",
        container.app
    )
}

/// The refusal when an uninstall would only change the private view.
#[must_use]
pub fn uninstall_refusal(container: &AppContainer, layout: &Layout) -> String {
    format!(
        "This terminal runs inside {}, and Windows gives that app its own view of the registry \
         and AppData, so removing registrations here would not remove them for other apps. Run \
         this from a regular terminal: {}",
        container.app,
        removing_command(layout)
    )
}

/// What the reports call the Start-menu shortcut.
pub const START_MENU_SHORTCUT: &str = "Start-menu shortcut";

/// What the reports call the Uninstall key.
pub const INSTALLED_APPS_ENTRY: &str = "installed-apps entry";

/// What the reports call the Uninstall key when writing it replaces the one another installation
/// left, an earlier installer's or another home's. It reads after both "Updated the" and "Would
/// update the".
pub const INSTALLED_APPS_ENTRY_REPLACING_OTHER: &str =
    "installed-apps entry, replacing another installation's";

/// The words a person reads for the per-user `ghostlight` command entry in `state`.
#[must_use]
pub const fn command_state(state: CommandPathState) -> &'static str {
    match state {
        CommandPathState::NotApplicable => "not used on this installation",
        CommandPathState::Missing => "not installed",
        CommandPathState::Current => "installed",
        CommandPathState::Updatable => "installed, needs an update",
        CommandPathState::NeedsAttention => "needs attention",
        CommandPathState::PrivateView => "private to an app",
    }
}

/// The line that names the `ghostlight` command, with its entry's detail when this installation
/// keeps one.
#[must_use]
pub fn command_line(report: &CommandPathReport) -> String {
    if report.state == CommandPathState::NotApplicable {
        // Still name the executable: a person on a system package or on a test home needs the path
        // as much as anyone else does.
        return format!("Command: {}", report.executable.display());
    }
    format!(
        "Command: {} -- {}",
        report.executable.display(),
        report.detail
    )
}

/// The words a person reads for the per-user documentation and completion files in `state`.
#[must_use]
pub const fn documentation_state(state: UserAssetState) -> &'static str {
    match state {
        UserAssetState::NotApplicable => "not used on this installation",
        UserAssetState::Missing => "not installed",
        UserAssetState::Current => "installed",
        UserAssetState::NeedsAttention => "needs attention",
    }
}

/// The line about the documentation and completion files in `state`, with their `detail`.
#[must_use]
pub fn documentation_line(state: UserAssetState, detail: &str) -> String {
    format!("Documentation: {} -- {detail}", documentation_state(state))
}

/// The words a person reads for the Linux Applications entry in `state`.
#[must_use]
pub const fn applications_state(state: DesktopIntegrationState) -> &'static str {
    match state {
        DesktopIntegrationState::NotApplicable => "not used on this installation",
        DesktopIntegrationState::Missing => "not installed",
        DesktopIntegrationState::Current => "installed",
        DesktopIntegrationState::Updatable => "installed, needs an update",
        DesktopIntegrationState::NeedsAttention => "needs attention",
    }
}

/// What the report says of the Applications entry on a platform that has none.
pub const APPLICATIONS_ELSEWHERE: &str =
    "Linux application-menu integration does not apply on this platform.";

/// What the report says of the Applications entry in `state`. An updatable entry that already
/// starts this installation lacks only its icon (`icon_only`), and is said so.
#[must_use]
pub const fn applications_detail(state: DesktopIntegrationState, icon_only: bool) -> &'static str {
    match state {
        DesktopIntegrationState::NotApplicable => {
            "Desktop integration is owned by the operating system package."
        }
        DesktopIntegrationState::Missing => "Ghostlight is not present in Applications.",
        DesktopIntegrationState::Current => {
            "Applications opens this installation's Ghostlight window."
        }
        DesktopIntegrationState::Updatable if icon_only => {
            "The owned Applications entry starts this installation, but its icon is missing."
        }
        DesktopIntegrationState::Updatable => {
            "The owned Applications entry points at an older installation."
        }
        DesktopIntegrationState::NeedsAttention => {
            "A foreign file or symlink occupies Ghostlight's Applications location; it was left \
             untouched."
        }
    }
}

/// The line about the Applications entry `report` names at `entry`.
#[must_use]
pub fn applications_line(report: &DesktopIntegrationReport, entry: &Path) -> String {
    format!(
        "Applications: {} -- {} -- {}",
        applications_state(report.state),
        report.detail,
        entry.display()
    )
}

/// The words a person reads for an MCP client's Ghostlight entry in `state`.
#[must_use]
pub const fn client_state(state: HarnessState) -> &'static str {
    match state {
        HarnessState::NotDetected => "not detected",
        HarnessState::Available => "detected, not connected",
        HarnessState::Installed => "connected",
        HarnessState::Updatable => "connected, needs an update",
        HarnessState::NeedsAttention => "needs attention",
    }
}

/// What a client's detail says when its configuration could not be read.
pub const CLIENT_UNREADABLE: &str = "This configuration could not be read; it was left untouched.";

/// What a client's detail says when an entry Ghostlight did not write holds its name.
pub const CLIENT_FOREIGN_ENTRY: &str =
    "A foreign entry holds this target's Ghostlight registration; it was left untouched.";

/// The names a client's entry that starts the retired MCP connector is said with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetiredEntry<'a> {
    /// The retired connector the entry starts.
    pub connector: &'a str,
    /// The edge Set up points the entry at.
    pub edge: &'a str,
    /// The argument that makes the edge the MCP relay.
    pub argument: &'a str,
}

/// What a client's detail says of the client in `state` whose entry Ghostlight may write: whether
/// the edge is `edge_ready`, and the `retired` connector its owned entry starts, when it starts
/// one. A client that needs attention is said by its cause instead, so its detail here is empty.
#[must_use]
pub fn client_detail(
    state: HarnessState,
    edge_ready: bool,
    retired: Option<RetiredEntry<'_>>,
) -> String {
    match (state, retired) {
        (HarnessState::Updatable, Some(retired)) => format!(
            "This entry starts {}, which Ghostlight no longer installs. Set up points it at {} {}.",
            retired.connector, retired.edge, retired.argument
        ),
        (HarnessState::Installed, _) => "Ghostlight is registered for this user context.".into(),
        (HarnessState::Updatable, None) => {
            "Ghostlight's saved registration needs an update.".into()
        }
        (HarnessState::Available, _) if edge_ready => {
            "Detected and ready for an explicit Ghostlight registration.".into()
        }
        (HarnessState::Available, _) => "Detected, but the Ghostlight program is missing.".into(),
        (HarnessState::NotDetected, _) if edge_ready => {
            "Not detected. You can prepare its user configuration before installing it.".into()
        }
        (HarnessState::NotDetected, _) => {
            "Not detected, and the Ghostlight program is missing.".into()
        }
        (HarnessState::NeedsAttention, _) => String::new(),
    }
}

/// A client's `detail`, followed by the note that a path a person located is gone.
#[must_use]
pub fn with_located_path_missing(detail: &str) -> String {
    format!("{detail} A previously located path is missing; the normal location is shown.")
}

/// The number of arguments an entry's command carries.
fn argument_count(count: usize) -> String {
    match count {
        0 => "no arguments".into(),
        1 => "1 argument".into(),
        many => format!("{many} arguments"),
    }
}

/// What inspection found where Ghostlight's entry belongs: another `program` with `arguments`
/// arguments, beside the installed `edge` Ghostlight maintains there.
#[must_use]
pub fn other_program_evidence(program: &str, arguments: usize, edge: &Path) -> String {
    format!(
        "Found another program, `{program}` with {}, where Ghostlight's entry belongs; \
         Ghostlight maintains `{}` there and changed nothing.",
        argument_count(arguments),
        edge.display()
    )
}

/// What inspection found where Ghostlight's entry belongs: an entry without a usable command,
/// beside the installed `edge` Ghostlight maintains there.
#[must_use]
pub fn no_command_evidence(edge: &Path) -> String {
    format!(
        "Found an entry without a usable command where Ghostlight's entry belongs; Ghostlight \
         maintains `{}` there and changed nothing.",
        edge.display()
    )
}

/// The sentence a person reads for a client whose file Ghostlight never edits, named `file` when
/// its configuration has a file name.
#[must_use]
pub fn manual_setup(file: Option<&str>) -> String {
    format!(
        "Ghostlight does not edit this client's file. Copy setup and paste it into {}.",
        file.unwrap_or("its configuration file")
    )
}

/// The setup a person copies when the client's own form of it could not be written: the
/// `command` that starts Ghostlight as a local stdio MCP server.
#[must_use]
pub fn manual_setup_command(command: &str) -> String {
    format!("Use this command as a local stdio MCP server:\n{command}")
}

/// What a person reads when a client's configuration cannot be parsed. It names the `line` of
/// the parse error and nothing the file says.
#[must_use]
pub fn configuration_malformed(line: Option<usize>) -> String {
    match line {
        Some(line) => {
            format!("This configuration could not be parsed at line {line}; it was left untouched.")
        }
        None => "This configuration could not be parsed; it was left untouched.".into(),
    }
}

/// An install wrote `client`'s entry.
#[must_use]
pub fn client_registered(client: &str) -> String {
    format!("Ghostlight was registered with {client}. Restart or reconnect it to load the tools.")
}

/// An install found `client`'s entry current.
#[must_use]
pub fn client_already_current(client: &str) -> String {
    format!("{client} already has the current Ghostlight registration.")
}

/// An uninstall removed Ghostlight from `client`.
#[must_use]
pub fn client_removed(client: &str) -> String {
    format!("Ghostlight was removed from {client}. Restart or reconnect it to unload the tools.")
}

/// An uninstall found no entry of Ghostlight's in `client`.
#[must_use]
pub fn client_had_no_entry(client: &str) -> String {
    format!("{client} had no owned Ghostlight registration to remove.")
}

/// Fix replaced the entry that held Ghostlight's name in `client`.
#[must_use]
pub fn client_fixed(client: &str) -> String {
    format!(
        "Ghostlight replaced the incorrect entry for {client}. Restart or reconnect it to load \
         the tools."
    )
}

/// A path a person located for `client` is kept for its Set up.
#[must_use]
pub fn client_location_kept(client: &str) -> String {
    format!("Ghostlight will use the located {client} path after you choose Set up.")
}

/// A client's change failed with nothing Ghostlight may show of the failure.
pub const CLIENT_CHANGE_FAILED: &str = "The target could not be changed.";

/// What setting up every detected client did: `set_up` new entries and `updated` older ones,
/// with how many clients still need attention and how many `failed`.
#[must_use]
pub fn setup_message(
    set_up: usize,
    updated: usize,
    needs_attention: usize,
    failed: usize,
) -> String {
    let changed = set_up + updated;
    let mut message = if changed == 0 {
        "Everything Ghostlight can set up is already ready.".to_owned()
    } else {
        format!(
            "Ghostlight set up {set_up} and updated {updated} MCP integration{}.",
            if changed == 1 { "" } else { "s" }
        )
    };
    if changed > 0 {
        message.push_str(" Restart or reconnect those clients to load the tools.");
    }
    if needs_attention > 0 {
        message.push_str(&format!(
            " {needs_attention} target{} still need{} attention.",
            if needs_attention == 1 { "" } else { "s" },
            if needs_attention == 1 { "s" } else { "" }
        ));
    }
    if failed > 0 {
        message.push_str(&format!(
            " {failed} target{} could not be changed.",
            if failed == 1 { "" } else { "s" }
        ));
    }
    message
}

/// The refusal of a client id outside the roster.
#[must_use]
pub fn unknown_client_id(id: &str) -> String {
    format!("unknown supported harness `{id}`")
}

/// The refusal of a download for a client with no official download here.
#[must_use]
pub fn no_client_download(id: &str) -> String {
    format!("supported harness `{id}` has no official download for this platform")
}

/// The refusal of a client change while the installed edge is missing.
pub const EDGE_MISSING: &str = "the Ghostlight program is missing";

/// A client's configuration could not be read, for the system's `reason`.
#[must_use]
pub fn configuration_unreadable(reason: &str) -> String {
    format!("could not read harness configuration: {reason}")
}

/// A client's configuration could not be written, for the system's `reason`.
#[must_use]
pub fn configuration_unwritable(reason: &str) -> String {
    format!("could not write harness configuration: {reason}")
}

/// The refusal of a change while an entry Ghostlight did not write holds its name.
pub const FOREIGN_ENTRY_LEFT: &str =
    "a foreign `ghostlight` entry is present; Ghostlight left it untouched";

/// The refusal of a Fix whose entry changed since it was offered.
pub const NOT_FIXABLE: &str =
    "the Ghostlight entry changed and is no longer eligible for Fix; re-check integrations";

/// The refusal of a located `path` that is neither the client's configuration nor an executable.
#[must_use]
pub fn located_path_invalid(path: &Path) -> String {
    format!(
        "the selected path is not a usable harness executable or configuration: {}",
        path.display()
    )
}

/// Ghostlight's record of located paths could not be read, for `reason`.
#[must_use]
pub fn location_state_malformed(reason: &str) -> String {
    format!("Ghostlight's harness location state is malformed: {reason}")
}

/// The first line `doctor` prints, naming this build's `version`.
#[must_use]
pub fn doctor_heading(version: &str) -> String {
    format!("Ghostlight {version} diagnostics")
}

/// The line that names the Ghostlight home.
#[must_use]
pub fn home_line(home: &Path) -> String {
    format!("Home: {}", home.display())
}

/// The line that names what the installation record says is installed, or that it says nothing.
#[must_use]
pub fn installed_line(record: Option<&InstallRecord>) -> String {
    match record {
        Some(record) => format!(
            "Installed: Ghostlight {} -- relay {}",
            record.service_version, record.relay_revision
        ),
        None => "Installed: nothing recorded in the Ghostlight home".into(),
    }
}

/// The line that names the environment Ghostlight runs in, by its `label` and `location`.
#[must_use]
pub fn environment_line(label: &str, location: &str) -> String {
    format!("Environment: {label} -- {location}")
}

/// The line that says the environment's `caveat`.
#[must_use]
pub fn environment_caveat_line(caveat: &str) -> String {
    format!("Environment: {caveat}")
}

/// The line that says the running service's readiness, by its `word` and `detail`.
#[must_use]
pub fn readiness_line(word: &str, detail: &str) -> String {
    format!("Readiness: {word} -- {detail}")
}

/// The line `doctor` prints when a service runs and did not report its readiness.
pub const READINESS_UNREPORTED: &str =
    "Readiness: unavailable -- The running Ghostlight service did not report its current state.";

/// The line about one installed executable at `path`, and whether it is there.
#[must_use]
pub fn binary_line(path: &Path, ready: bool) -> String {
    format!(
        "Binary: {} -- {}",
        path.display(),
        if ready { "ready" } else { "missing" }
    )
}

/// The line that says the registrations below cannot be seen from here, and why.
#[must_use]
pub fn registrations_line(cannot_see: &str) -> String {
    format!("Registrations: {cannot_see}")
}

/// The line about one MCP client: its `label`, its entry's `state` and its `detail`.
#[must_use]
pub fn client_line(label: &str, state: HarnessState, detail: &str) -> String {
    format!("MCP client: {label} -- {} -- {detail}", client_state(state))
}

/// The line about process diagnostics: the `layer` that turned them on or off, and, when they
/// write into `directory`, how many bytes of log `used` there.
#[must_use]
pub fn diagnostics_line(layer: &str, used: u64, directory: Option<&str>) -> String {
    match directory {
        Some(directory) => {
            format!("Process diagnostics: {layer} -- {used} bytes of log in {directory}")
        }
        None => {
            format!("Process diagnostics: {layer} -- run ghostlight diagnostics on to turn them on")
        }
    }
}

/// What `doctor --fix` says before it repairs.
pub const APPLYING_REPAIRS: &str = "Applying ownership-safe repairs.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::self_check::golden::repository_path;

    /// The two launchers, which word the way back themselves because they run before any service.
    const LAUNCHERS: [(&str, &str); 2] = [
        (
            "packaging/npm/bin/ghostlight.js",
            include_str!("../../../../packaging/npm/bin/ghostlight.js"),
        ),
        (
            "packaging/mcpb/server/launch.js",
            include_str!("../../../../packaging/mcpb/server/launch.js"),
        ),
    ];

    /// The PowerShell get script, whose switch the way back names.
    const GET_POWERSHELL: &str = "scripts/get.ps1";

    /// The POSIX get script, whose option the way back names.
    const GET_SH: &str = "scripts/get.sh";

    /// 6155aa14 (A-06, G4-06) and 55766ba5: the way back from a development build names each
    /// release carrier with a switch that carrier accepts and hands to the service, both
    /// launchers say it in the service's words, and every refusal that needs it ends with it.
    #[test]
    fn the_way_back_names_what_each_carrier_accepts() {
        let route = restore_route();
        for (name, source) in LAUNCHERS {
            let start = source
                .find("const RESTORE_ROUTE = [")
                .unwrap_or_else(|| panic!("{name} names no RESTORE_ROUTE"));
            let list = &source[start..];
            let list = &list[..list
                .find(r#"].join("\n");"#)
                .unwrap_or_else(|| panic!("{name} does not join RESTORE_ROUTE by lines"))];
            assert!(!list.contains('\\'), "{name} escapes a RESTORE_ROUTE line");
            let lines: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
            assert_eq!(lines.join("\n"), route, "{name}");
            let missing =
                development_executable_missing("${executable}").replace(&route, "${RESTORE_ROUTE}");
            assert!(source.contains(&format!("`{missing}")), "{name}: {missing}");
        }

        assert!(route.contains(&format!(
            "npx ghostlight deployment install {RESTORE_OPTION}"
        )));
        assert!(route.contains(&format!("get.ps1))) {GET_POWERSHELL_RESTORE_SWITCH}")));
        assert!(route.contains(&format!("get.sh | sh -s -- {RESTORE_OPTION}")));
        assert!(
            !route.contains("ghostlight-service"),
            "the route names a carrier, never a staged file inside one"
        );

        let powershell = std::fs::read_to_string(repository_path(GET_POWERSHELL))
            .expect("the PowerShell get script is readable");
        let switch = GET_POWERSHELL_RESTORE_SWITCH.trim_start_matches('-');
        assert!(
            powershell.contains(&format!("[switch]${switch}")),
            "{GET_POWERSHELL} accepts {GET_POWERSHELL_RESTORE_SWITCH}"
        );
        assert!(
            powershell.contains(&format!(
                "if (${switch}) {{ $deployment += \"{RESTORE_OPTION}\" }}"
            )),
            "{GET_POWERSHELL} hands {RESTORE_OPTION} to the service"
        );
        let posix = std::fs::read_to_string(repository_path(GET_SH))
            .expect("the POSIX get script is readable");
        assert!(
            posix.contains(&format!(
                "{RESTORE_OPTION}) restore=\"{RESTORE_OPTION}\" ;;"
            )),
            "{GET_SH} accepts {RESTORE_OPTION} and hands it to the service"
        );

        let missing = development_executable_missing("bin/ghostlight-service");
        assert!(missing.contains("bin/ghostlight-service is missing"));
        assert!(missing.ends_with(&route));
        assert!(development_installed().ends_with(&route));
    }

    /// d6c55ef3 and 41a3f8bf: a dry run says per client what the real install or uninstall would
    /// do, in words and never a state's identifier: an update, nothing for a current entry, each
    /// other settings copy a removal changes, and the cause inspection saw for an entry it leaves.
    #[test]
    fn a_dry_run_says_what_the_real_run_would_change() {
        use HarnessState::{Available, Installed, NeedsAttention, NotDetected, Updatable};

        const NOTHING: &str = "nothing would change.";
        let install = |state| client_would_install("Cline (CLI)", state, None);
        let remove = |state| client_would_remove("Cline (CLI)", state, None, &[]);
        for state in [Available, NotDetected] {
            assert_eq!(install(state), "Would install Ghostlight for Cline (CLI).");
            assert!(remove(state).ends_with(NOTHING), "{state:?}");
        }
        assert_eq!(
            install(Updatable),
            "Would update Ghostlight's entry for Cline (CLI)."
        );
        assert!(install(Installed).ends_with(NOTHING));
        for state in [Installed, Updatable] {
            assert_eq!(remove(state), "Would remove Ghostlight from Cline (CLI).");
        }
        for state in [Available, NotDetected, Updatable, Installed, NeedsAttention] {
            for sentence in [install(state), remove(state)] {
                assert!(
                    !sentence.contains(&format!("{state:?}")),
                    "no internal name: {sentence}"
                );
            }
        }

        let left = [
            (
                Some(Attention::ForeignEntry),
                "Would leave Kiro as it is: the entry under Ghostlight's name is not Ghostlight's."
                    .to_owned(),
            ),
            (
                Some(Attention::Malformed),
                "Would leave Kiro as it is: its settings file could not be parsed.".to_owned(),
            ),
            (
                Some(Attention::Unreadable),
                "Would leave Kiro as it is: its settings file could not be read.".to_owned(),
            ),
            (
                Some(Attention::Launcher(LauncherPin::Unpinned)),
                "Would leave Kiro as it is: its entry starts Ghostlight's npm launcher.".to_owned(),
            ),
            (
                Some(Attention::Launcher(LauncherPin::Earlier)),
                format!(
                    "Would leave Kiro as it is: its entry starts Ghostlight's npm launcher pinned \
                     to a release before {FIRST_LAUNCHER_MAJOR}.0."
                ),
            ),
            (None, "Would leave Kiro as it is.".to_owned()),
        ];
        for (attention, line) in left {
            assert_eq!(
                client_would_install("Kiro", NeedsAttention, attention),
                line
            );
            assert_eq!(
                client_would_remove("Kiro", NeedsAttention, attention, &[]),
                line
            );
        }

        let plain = r"C:\Users\Pat\AppData\Roaming\Claude\claude_desktop_config.json".to_owned();
        let codex = r"C:\Users\Pat\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Roaming\Claude\claude_desktop_config.json".to_owned();
        let one = [plain.clone()];
        let elsewhere = format!(
            "Would remove Ghostlight from another copy of Claude Desktop's settings, at {plain}."
        );
        for (state, attention) in [
            (Available, None),
            (NotDetected, None),
            (NeedsAttention, Some(Attention::ForeignEntry)),
            (NeedsAttention, Some(Attention::Malformed)),
        ] {
            assert_eq!(
                client_would_remove("Claude Desktop", state, attention, &one),
                elsewhere,
                "{state:?}"
            );
        }
        assert_eq!(
            client_would_remove("Claude Desktop", Installed, None, &one),
            format!(
                "Would remove Ghostlight from Claude Desktop and from another copy of its \
                 settings, at {plain}."
            )
        );
        assert_eq!(
            client_would_remove(
                "Claude Desktop",
                Available,
                None,
                &[plain.clone(), codex.clone()]
            ),
            format!(
                "Would remove Ghostlight from other copies of Claude Desktop's settings, at \
                 {plain} and {codex}."
            )
        );
    }
}
