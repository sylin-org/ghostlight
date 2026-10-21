//! `ghostlight deployment install`: the one way executables reach the Ghostlight home.
//!
//! A package, the npm launcher, the get scripts and the development loop all run a
//! `ghostlight-service` with this command (ADR-0197 Decisions 13 and 14, ADR-0199 Decision 19 and
//! the development channel amendment). Under the install lock it:
//!
//! 1. checks the 2.0 break migration's destinations when a 1.x layout is found, so a conflict
//!    changes nothing, then moves the 1.x policy, managed-policy and client-location files into
//!    the home: no 1.x program holds them open, and the 2.0 service or client that the next
//!    steps let start must find them in place;
//! 2. reads `bin/install.json` and decides what to replace. A release refuses an older payload
//!    and a development installation, does nothing for the same intact version with an intact
//!    edge, rename-swaps the service image into `bin`, and replaces the edge only when the relay
//!    revision changes, `bin` still holds the pre-edge layout, or the edge in `bin` is missing or
//!    is not the one recorded. The payload is checked first, so a release install run from `bin`
//!    itself reports that, not a restore command that would fail the same way.
//!    `--development` places the same files the same way, but always swaps the service, whatever
//!    version is installed, also replaces an edge whose bytes the build changed (see
//!    [`EdgeReplacement::Rebuilt`]), and records the development channel. `--restore` does the
//!    same for this executable's own package and records the release channel: the one way back
//!    from a development installation, and the one allowed move down. The Ghostlight shell goes
//!    beside the service image when the payload has one and `bin` holds another;
//! 3. reconciles every registration to name `bin`, as soon as `bin` holds the executables and
//!    before anything old is stopped, so a browser or client that relaunches its host the moment
//!    the old one stops starts the new one;
//! 4. stops the 1.x images and looks again for a few rounds for any that respawned;
//! 5. asks the running service to quit (a control `quit` for replacement) and waits for its
//!    service lock to be free, and stops that exact image if it has not exited or is an earlier
//!    development build that cannot be asked; stops what runs from the retired development
//!    directory and removes that directory and its marker, which earlier development builds used;
//!    archives the 1.x audit log, then removes the 1.x lifecycle files, the installation record
//!    last; stops what runs from a 1.x `bin/v<version>` directory, archives the audit log it
//!    holds into `state`, and removes it (see [`version_directories`]);
//! 6. starts `bin/ghostlight-service serve` and writes `install.json` only once a service answers a
//!    control status with an instance other than the one that was replaced.
//!
//! There is no rollback: a service that does not start is reported, `install.json` stays as it
//! was, and the next launch installs again. `--plan` prints what would happen and changes
//! nothing. Every process start and stop goes through [`ProcessControl`], so tests never start a
//! process.

use std::env;
use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use ghostlight_relay::home::{EDGE_EXECUTABLE_NAME, SERVICE_EXECUTABLE_NAME, TEST_HOME_VARIABLE};
use ghostlight_relay::private;
use ghostlight_relay::protocol::{Control, QuitReason, StartReason};
use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use thiserror::Error;

use super::entry::{canonical_path, same_path};
use super::layout::{retired_connector_names, Layout, DESKTOP_EXECUTABLE_NAME};
use super::migration::{self, FlatpakStep, Legacy, LegacyRoots, MigrationPlan, Move};
use super::reconcile::{PendingRegistration, ReconcileReport, Registrations};
use super::record::{self, Channel, InstallRecord, PayloadRefused, Standing};
use super::swap;
use super::version_directories;
use crate::cli::client;
use crate::language::deployment as words;
use crate::service::lock::ServiceLock;

/// How long an install waits for another install to finish. An install can itself spend the
/// quit wait and the ready wait.
const INSTALL_LOCK_WAIT: Duration = Duration::from_secs(60);

/// Pause between attempts to take the install lock, between checks of a quitting service and
/// between looks for a 1.x program that respawned.
const POLL: Duration = Duration::from_millis(25);

/// How many times, after the 1.x images are stopped, the installer looks again for a program that
/// was started from one of them in the meantime.
const LEGACY_RESPAWN_ROUNDS: u32 = 3;

/// How long the running service has to finish in-flight work and exit after `quit`.
const QUIT_WAIT: Duration = Duration::from_secs(10);

/// How long a started service has to answer a control status.
const READY_WAIT: Duration = Duration::from_secs(30);

/// Pause between asks for a control status.
const READY_POLL: Duration = Duration::from_millis(100);

/// How long a started service has before the installer may conclude it stopped: no process runs
/// its image and nothing answers. The new process runs its image well within this.
const STOPPED_GRACE: Duration = Duration::from_secs(2);

/// The relay revision of the development builds that preceded relay 4. They hold no service lock
/// and cannot be asked to quit, so the installer stops their exact images instead.
const EARLIER_DEVELOPMENT_RELAY: &str = "3";

/// The option that installs a payload as the development channel.
pub const DEVELOPMENT_OPTION: &str = "--development";

/// The option that returns a development installation to a release.
pub const RESTORE_OPTION: &str = "--restore";

/// The option that prints the install plan and changes nothing.
pub const PLAN_OPTION: &str = "--plan";

/// What an install puts where.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// A release: the payload becomes the installed service in `bin`, unless an equal or newer
    /// one is there or the installation is a development build.
    Release,
    /// A development build: the payload becomes the installed service in `bin` every time and
    /// the installation is recorded as the development channel.
    Development,
    /// Return to a release: this executable's own payload becomes the installed service in `bin`
    /// whatever version or channel is installed, and the installation is recorded as a release.
    /// The one allowed move down.
    Restore,
}

/// One `deployment install` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The directory holding the payload's executables; `None` is this executable's own
    /// directory.
    pub payload: Option<PathBuf>,
    /// What to install.
    pub mode: Mode,
    /// Print the plan and change nothing.
    pub plan_only: bool,
}

/// The build the running installer belongs to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Build {
    /// Product version.
    pub version: String,
    /// Relay revision its edge speaks.
    pub relay_revision: String,
    /// SHA-256 of its service image.
    pub image_sha256: String,
}

impl Build {
    /// The running executable's build.
    pub fn current() -> io::Result<Self> {
        Ok(Self {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            relay_revision: ghostlight_relay::RELAY.to_string(),
            image_sha256: record::sha256_file(&env::current_exe()?)?,
        })
    }

    /// The record `install.json` holds once this build serves from `bin` as `channel`, with the
    /// edge it leaves there.
    fn record(&self, edge_sha256: Option<String>, channel: Channel) -> InstallRecord {
        InstallRecord {
            service_version: self.version.clone(),
            relay_revision: self.relay_revision.clone(),
            service_sha256: self.image_sha256.clone(),
            edge_sha256,
            channel,
        }
    }
}

/// Starting and stopping the service. The installer never starts or stops a process any other
/// way, so tests drive the whole install with a fake.
pub trait ProcessControl {
    /// The instance of the service of `layout`'s home that answers a control status, when one
    /// does.
    fn instance(&self, layout: &Layout) -> Option<String>;
    /// Ask the service of `layout`'s home to quit for replacement and wait up to 10 s for its
    /// service lock to be free. Returns whether no service of that home runs afterwards.
    fn quit(&self, layout: &Layout) -> bool;
    /// Stop every process running one of `images` and wait for it to exit.
    fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()>;
    /// Whether any process runs one of `images`.
    fn is_running(&self, images: &[PathBuf]) -> bool;
    /// The programs of an older installation that still run in `layout`'s home. The answering
    /// service `service_pid` is spared unless it runs from the retired development directory or
    /// from a 1.x version directory.
    fn strays(&self, layout: &Layout, service_pid: Option<u32>) -> Vec<StrayProcess>;
    /// Stop `strays` and wait for them to exit. Returns the ones that would not stop.
    fn stop_strays(&self, strays: &[StrayProcess]) -> Vec<StrayProcess>;
    /// Start `bin/ghostlight-service serve --reason update` in `layout`'s home, detached, without waiting
    /// for it.
    fn start_serve(&self, layout: &Layout) -> io::Result<()>;
    /// Wait up to 30 s for the service of `layout`'s home to answer an accepted control status
    /// whose instance is not `previous`, and stop waiting early once no service runs.
    fn wait_ready(&self, layout: &Layout, previous: Option<&str>) -> Readiness;
}

/// How the wait for a started service ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Readiness {
    /// A new service answered.
    Answered,
    /// No process runs the service image and nothing answers: the service stopped.
    Stopped,
    /// A service ran but did not answer within the ready wait.
    Silent,
}

/// Why a service the installer started did not answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartFailure {
    /// The process could not be created; the system's reason.
    Spawn(String),
    /// The service stopped; the reason it recorded, when it recorded one.
    Stopped(Option<String>),
    /// The service ran but did not answer within the ready wait.
    Silent,
}

/// The real processes of this user.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemProcessControl;

impl ProcessControl for SystemProcessControl {
    fn instance(&self, layout: &Layout) -> Option<String> {
        client::status_at(layout.home())
            .ok()
            .map(|status| status.instance)
    }

    fn quit(&self, layout: &Layout) -> bool {
        let earlier = InstallRecord::read(&layout.install_record())
            .ok()
            .flatten()
            .is_some_and(|record| record.relay_revision == EARLIER_DEVELOPMENT_RELAY);
        if earlier {
            return false;
        }
        let lock = layout.service_lock();
        if !ServiceLock::is_free(&lock) {
            // The answer only says the service heard; the lock says when it has gone.
            let _ = client::control_at(
                layout.home(),
                Control::Quit {
                    reason: QuitReason::Replace,
                },
            );
        }
        let images = running_images(layout);
        let deadline = Instant::now() + QUIT_WAIT;
        loop {
            if ServiceLock::is_free(&lock) && !self.is_running(&images) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL);
        }
    }

    fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()> {
        migration::stop_legacy_images(&other_than_running_installer(images))
    }

    fn is_running(&self, images: &[PathBuf]) -> bool {
        migration::any_running_from(&other_than_running_installer(images))
    }

    fn strays(&self, layout: &Layout, service_pid: Option<u32>) -> Vec<StrayProcess> {
        stray_processes(layout, service_pid)
    }

    fn stop_strays(&self, strays: &[StrayProcess]) -> Vec<StrayProcess> {
        stop_strays(strays)
    }

    fn start_serve(&self, layout: &Layout) -> io::Result<()> {
        ghostlight_relay::spawn::spawn_serve(
            &layout.service_executable(),
            layout.home(),
            StartReason::Update,
        )
        .map(|_| ())
    }

    fn wait_ready(&self, layout: &Layout, previous: Option<&str>) -> Readiness {
        let started = Instant::now();
        let image = [layout.service_executable()];
        loop {
            let answered = client::status_at(layout.home())
                .is_ok_and(|status| Some(status.instance.as_str()) != previous);
            if answered {
                return Readiness::Answered;
            }
            let waited = started.elapsed();
            if waited >= STOPPED_GRACE && !migration::any_running_from(&image) {
                return Readiness::Stopped;
            }
            if waited >= READY_WAIT {
                return Readiness::Silent;
            }
            std::thread::sleep(READY_POLL);
        }
    }
}

/// `images` without the running installer's own executable, which cannot be stopped.
fn other_than_running_installer(images: &[PathBuf]) -> Vec<PathBuf> {
    let own = env::current_exe().map(|path| canonical_path(&path)).ok();
    images
        .iter()
        .filter(|image| {
            own.as_ref()
                .is_none_or(|own| !same_path(&canonical_path(image), own))
        })
        .cloned()
        .collect()
}

/// The kinds of program an older Ghostlight installation leaves running.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrayKind {
    /// A connector from the layout before the edge: the MCP or browser connector.
    RetiredConnector,
    /// A service image a deploy moved aside whose process is not the service that answers.
    ReplacedService,
    /// A program still running from the directory earlier 2.0 development builds ran in, which
    /// an install removes.
    RetiredDevelopment,
    /// A program still running from a 1.x `bin/v<version>` directory, which an install removes.
    RetiredVersionDirectory,
}

/// A process of an older installation that is still running.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StrayProcess {
    /// The process id.
    pub pid: u32,
    /// The executable the process runs, which may no longer exist under that name.
    pub image: PathBuf,
    /// What kind of leftover it is.
    pub kind: StrayKind,
}

/// How long a stopped stray is given to exit.
const STRAY_STOP_WAIT: Duration = Duration::from_secs(5);

/// What Linux appends to the executable path of a process whose file has been removed.
const REMOVED_IMAGE_MARKER: &str = " (deleted)";

/// The executable a process runs, as the process table reports it, without the marker Linux
/// adds once the file has been removed, so a process whose image a deploy deleted keeps its name.
fn running_image(reported: &Path) -> PathBuf {
    reported
        .to_str()
        .and_then(|text| text.strip_suffix(REMOVED_IMAGE_MARKER))
        .map_or_else(|| reported.to_path_buf(), PathBuf::from)
}

/// The processes of this home that an older installation left running.
///
/// `service_pid` is the service that answers, when one does: it is spared unless it runs from the
/// retired development directory or from a 1.x version directory. The process table is read once.
#[must_use]
pub fn stray_processes(layout: &Layout, service_pid: Option<u32>) -> Vec<StrayProcess> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        stray_refresh().with_exe(UpdateKind::Always),
    );
    let own_pid = std::process::id();
    let mut strays: Vec<StrayProcess> = system
        .processes()
        .values()
        .filter_map(|process| {
            let image = running_image(process.exe()?);
            let pid = process.pid().as_u32();
            classify(&image, pid, layout, service_pid, own_pid).map(|kind| StrayProcess {
                pid,
                image,
                kind,
            })
        })
        .collect();
    strays.sort_by_key(|stray| stray.pid);
    strays
}

/// What to read of each process when looking for strays or checking that one is gone.
fn stray_refresh() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing().without_tasks()
}

/// Whether the process running `image` is a leftover of an older installation.
///
/// Only executables under the Ghostlight home count, so a program of the same name elsewhere is
/// left alone. Anything running from the retired development directory or from a 1.x version
/// directory is a leftover; elsewhere the name decides, so a process whose file has since been
/// removed is still found. The current edge, the shell and the current service image are never
/// strays, nor is the process asking. The retired directories come first, so the answering
/// service is a stray when it runs from one.
fn classify(
    image: &Path,
    pid: u32,
    layout: &Layout,
    service_pid: Option<u32>,
    own_pid: u32,
) -> Option<StrayKind> {
    if pid == own_pid || !lies_under(image, layout.home()) {
        return None;
    }
    if lies_under(image, &layout.retired_development_directory()) {
        return Some(StrayKind::RetiredDevelopment);
    }
    if version_directories::find(layout)
        .iter()
        .any(|directory| lies_under(image, directory))
    {
        return Some(StrayKind::RetiredVersionDirectory);
    }
    let file_name = image.file_name()?.to_str()?;
    let (base, is_moved_aside) = match swap::moved_aside(file_name) {
        Some((base, _)) => (base, true),
        None => (file_name, false),
    };
    if retired_connector_names()
        .iter()
        .any(|name| same_file_name(name, base))
    {
        return Some(StrayKind::RetiredConnector);
    }
    (is_moved_aside && same_file_name(SERVICE_EXECUTABLE_NAME, base) && Some(pid) != service_pid)
        .then_some(StrayKind::ReplacedService)
}

/// Whether two file names are the same file name on this platform.
fn same_file_name(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

/// Whether `image` lies under `home`, however either is spelled. The image's file may be gone.
fn lies_under(image: &Path, home: &Path) -> bool {
    let resolved = image
        .parent()
        .zip(image.file_name())
        .map(|(directory, name)| canonical_path(directory).join(name));
    let homes = [home.to_path_buf(), canonical_path(home)];
    std::iter::once(image)
        .chain(resolved.as_deref())
        .flat_map(Path::ancestors)
        .any(|ancestor| homes.iter().any(|home| same_path(ancestor, home)))
}

/// What became of the strays one search found.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StrayStop {
    /// The strays that are gone now.
    pub stopped: Vec<StrayProcess>,
    /// The strays that would not stop.
    pub stuck: Vec<StrayProcess>,
}

/// Find the strays of `layout`'s home through `control` and stop them.
///
/// `service_pid` is the service that answers, when one does. This is the one way an install, a
/// deploy and `doctor --fix` stop leftovers, so they say and do the same.
pub fn stop_found_strays(
    control: &dyn ProcessControl,
    layout: &Layout,
    service_pid: Option<u32>,
) -> StrayStop {
    let found = control.strays(layout, service_pid);
    if found.is_empty() {
        return StrayStop::default();
    }
    let stuck = control.stop_strays(&found);
    let stopped = found
        .into_iter()
        .filter(|stray| !stuck.contains(stray))
        .collect();
    StrayStop { stopped, stuck }
}

/// Stop each stray and wait for it to exit. Returns the ones that would not stop.
///
/// A process that is already gone, or whose pid now belongs to another program, counts as
/// stopped. The calling process is never stopped.
#[must_use]
pub fn stop_strays(strays: &[StrayProcess]) -> Vec<StrayProcess> {
    let own_pid = std::process::id();
    let (own, others): (Vec<_>, Vec<_>) = strays.iter().partition(|stray| stray.pid == own_pid);
    let mut stuck: Vec<StrayProcess> = own.into_iter().cloned().collect();
    if others.is_empty() {
        return stuck;
    }
    let pids: Vec<Pid> = others
        .iter()
        .map(|stray| Pid::from_u32(stray.pid))
        .collect();
    let mut system = System::new();
    let refresh = |system: &mut System| {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            stray_refresh().with_exe(UpdateKind::Always),
        );
    };
    let runs = |system: &System, stray: &StrayProcess| {
        system
            .process(Pid::from_u32(stray.pid))
            .is_some_and(|process| {
                process.status() != sysinfo::ProcessStatus::Zombie
                    && process
                        .exe()
                        .is_some_and(|exe| same_path(&running_image(exe), &stray.image))
            })
    };
    refresh(&mut system);
    for stray in &others {
        if runs(&system, stray) {
            if let Some(process) = system.process(Pid::from_u32(stray.pid)) {
                process.kill();
            }
        }
    }
    let deadline = Instant::now() + STRAY_STOP_WAIT;
    loop {
        refresh(&mut system);
        let running: Vec<&StrayProcess> = others
            .iter()
            .copied()
            .filter(|stray| runs(&system, stray))
            .collect();
        if running.is_empty() {
            return stuck;
        }
        if Instant::now() >= deadline {
            stuck.extend(running.into_iter().cloned());
            return stuck;
        }
        std::thread::sleep(POLL);
    }
}

/// One executable placed by a rename-swap.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Swap {
    /// The directory it goes into.
    pub directory: PathBuf,
    /// Its file name there.
    pub name: String,
    /// The payload file it is copied from.
    pub source: PathBuf,
}

/// Why an install replaces the edge in `bin`. The plan prints it as the sentence a person reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeReplacement {
    /// `bin` still holds an older layout, where the service image had the edge's name and the
    /// connectors sat beside it.
    PreEdgeLayout,
    /// There is no installation record, or it names no edge.
    Unrecorded,
    /// The payload's edge speaks another relay revision than the recorded one (ADR-0197
    /// Decision 14).
    RelayRevision,
    /// The edge in `bin` is missing.
    Missing,
    /// The edge in `bin` is not the one the record names.
    Altered,
    /// A development install whose payload edge differs byte for byte from the installed one.
    /// Cargo relinks the edge only when its code or dependencies change, so a development edge
    /// follows its build and an unchanged one stays (ADR-0197, amendment of 2026-10-18).
    Rebuilt,
    /// A release install or a restore over a development install whose payload edge differs from
    /// the installed one: the way back from development brings back the release's edge too.
    LeavingDevelopment,
}

impl Serialize for EdgeReplacement {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&words::edge_replaced(*self))
    }
}

/// Everything one install will do, in order. `--plan` prints it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InstallPlan {
    /// What is installed.
    pub mode: Mode,
    /// The version the payload carries.
    pub version: String,
    /// The 2.0 break migration, carried out around the swap.
    pub migration: MigrationPlan,
    /// The same version is installed and intact: nothing else happens.
    pub unchanged: bool,
    /// Executables placed by rename-swap: the service image, the shell beside it when the payload
    /// has one and `bin` holds another, and the edge where it changes.
    pub swaps: Vec<Swap>,
    /// Why the edge is among the swaps; absent, and not printed, when it stays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_replaced: Option<EdgeReplacement>,
    /// Whether the running service is replaced by a new `serve`.
    pub restart: bool,
    /// The record written once the new service answers.
    pub record: Option<InstallRecord>,
    /// Images stopped: the old installation's, the retired development directory's, and the
    /// running service's if it does not quit.
    pub processes_to_stop: Vec<PathBuf>,
    /// Files and directories removed once `bin` holds the new executables: the retired
    /// development directory and its marker, where an earlier development build ran, and the 1.x
    /// `bin/v<version>` directories.
    pub deletions: Vec<PathBuf>,
    /// The audit logs of the 1.x version directories, each moved into `state` before its
    /// directory is removed.
    pub version_audit_archives: Vec<Move>,
    /// Registrations the reconcile will write.
    pub registrations_to_rewrite: Vec<PendingRegistration>,
}

/// What an install did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    /// The plan that was carried out.
    pub plan: InstallPlan,
    /// Whether the new service answered; `None` when no service was started.
    pub service_started: Option<bool>,
    /// Why the new service did not answer, when `service_started` is `Some(false)`.
    pub start_failure: Option<StartFailure>,
    /// What the reconcile changed; `None` when nothing was installed.
    pub registrations: Option<ReconcileReport>,
    /// What became of the files the 1.x Flatpak route left in the home.
    pub flatpak: Vec<FlatpakStep>,
    /// Entries of the plan's deletions that could not be removed. The next install tries again;
    /// this one is not failed by them.
    pub left_behind: Vec<PathBuf>,
    /// The audit logs of the 1.x version directories that this install moved into `state`, as
    /// observed: the plan lists what it intends, this lists what happened.
    pub archived_version_audits: Vec<Move>,
    /// Programs of an older installation that would not stop. The next install, deploy or
    /// `doctor --fix` tries again; this one is not failed by them.
    pub strays_left: Vec<StrayProcess>,
    /// The folders 1.x left that this install removed (`migration::retire_leftovers`).
    pub retired_leftovers: Vec<PathBuf>,
}

/// A release package found an installation that a development build made. The refusal names the
/// release carriers that restore: a package's own directory is a temporary stage that is gone
/// once its carrier exits.
#[derive(Debug, Error)]
#[error("{}", words::development_installed())]
pub struct DevelopmentInstalled;

/// The installation record says a development build is installed and the executable a command
/// goes to is gone, so there is nothing to hand the command to. The restore route is the only
/// way forward.
#[derive(Debug, Error)]
#[error("{}", words::development_executable_missing(&.0.display().to_string()))]
pub struct DevelopmentExecutableMissing(pub PathBuf);

/// Why a payload directory cannot be installed.
#[derive(Debug, Error)]
pub enum PayloadError {
    /// An executable is missing from the payload.
    #[error("{}", words::payload_missing(&.0.display().to_string()))]
    Missing(PathBuf),
    /// The payload already is the directory it would be installed into.
    #[error("{}", words::payload_in_place(&.0.display().to_string()))]
    InPlace(PathBuf),
    /// The payload's service is another build than the running installer.
    #[error("{}", words::payload_other_build(&.0.display().to_string()))]
    OtherBuild(PathBuf),
}

/// Everything an install reads and acts through.
pub struct Context<'a> {
    /// The Ghostlight home.
    pub layout: Layout,
    /// The running installer's build.
    pub build: Build,
    /// The directory holding the payload.
    pub payload: PathBuf,
    /// The 1.x layout found.
    pub legacy: Legacy,
    /// Process starts and stops.
    pub control: &'a dyn ProcessControl,
    /// The registrations to reconcile.
    pub registrations: &'a Registrations,
}

/// Run `request` for this user's home with the real processes and registrations.
pub fn run(request: &Request) -> anyhow::Result<Outcome> {
    run_in(Layout::current()?, request)
}

/// Run `request` for the home of `layout` with the real processes and registrations.
fn run_in(layout: Layout, request: &Request) -> anyhow::Result<Outcome> {
    let own_directory = env::current_exe()?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::other("the running executable has no directory"))?;
    let build = Build::current()?;
    let payload = match &request.payload {
        None => own_directory,
        Some(payload) => {
            let service = payload.join(SERVICE_EXECUTABLE_NAME);
            if record::sha256_file(&service).ok().as_deref() != Some(&build.image_sha256) {
                return Err(PayloadError::OtherBuild(payload.clone()).into());
            }
            payload.clone()
        }
    };
    // A test home never reads the real profile's 1.x files.
    let roots = if env::var_os(TEST_HOME_VARIABLE).is_some() {
        LegacyRoots::within(layout.home())
    } else {
        LegacyRoots::from_environment()?
    };
    let legacy = Legacy::discover(layout.home(), roots)?;
    let registrations = Registrations::discover();
    let context = Context {
        layout,
        build,
        payload,
        legacy,
        control: &SystemProcessControl,
        registrations: &registrations,
    };
    install(&context, request)
}

/// Install this executable's own package as a release into the home of `layout`, for a carrier
/// package.
pub fn install_carrier(layout: &Layout) -> anyhow::Result<()> {
    let outcome = run_in(
        layout.clone(),
        &Request {
            payload: None,
            mode: Mode::Release,
            plan_only: false,
        },
    )?;
    match ServiceDidNotStart::of(&outcome, layout) {
        Some(failure) => Err(failure.into()),
        None => Ok(()),
    }
}

/// The new service did not answer: where it was installed and why, as far as the installer saw.
#[derive(Debug, Error)]
#[error("{}", self.sentence())]
pub struct ServiceDidNotStart {
    /// The directory the service was installed in.
    pub bin: PathBuf,
    /// Why it did not answer.
    pub failure: StartFailure,
}

impl ServiceDidNotStart {
    /// The failure an install's `outcome` reports, when its service did not answer.
    #[must_use]
    pub fn of(outcome: &Outcome, layout: &Layout) -> Option<Self> {
        (outcome.service_started == Some(false)).then(|| Self {
            bin: layout.bin(),
            failure: outcome
                .start_failure
                .clone()
                .unwrap_or(StartFailure::Silent),
        })
    }

    /// The sentence a person reads.
    fn sentence(&self) -> String {
        let bin = self.bin.display().to_string();
        match &self.failure {
            StartFailure::Spawn(reason) => words::service_could_not_start(&bin, reason),
            StartFailure::Stopped(Some(reason)) => words::service_stopped_because(&bin, reason),
            StartFailure::Stopped(None) => words::service_stopped(&bin),
            StartFailure::Silent => words::service_did_not_start(&bin, READY_WAIT.as_secs()),
        }
    }
}

/// The reason the service of `layout`'s home recorded when its start failed, if it did.
fn recorded_start_failure(layout: &Layout) -> Option<String> {
    let text = fs::read_to_string(layout.start_failure()).ok()?;
    let reason = text.trim();
    (!reason.is_empty()).then(|| reason.to_owned())
}

/// Plan `request` and, unless it only asks for the plan, carry it out under the install lock.
pub fn install(context: &Context<'_>, request: &Request) -> anyhow::Result<Outcome> {
    if request.plan_only {
        return Ok(Outcome {
            plan: plan(context, request.mode)?,
            service_started: None,
            start_failure: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
            archived_version_audits: Vec::new(),
            strays_left: Vec::new(),
            retired_leftovers: Vec::new(),
        });
    }
    let _guard = hold_install_lock(&context.layout.install_lock())?;
    let plan = plan(context, request.mode)?;
    execute(context, plan)
}

/// Work out what installing in `mode` does. Nothing is changed.
pub fn plan(context: &Context<'_>, mode: Mode) -> anyhow::Result<InstallPlan> {
    let layout = &context.layout;
    let migration = migration::plan(layout.home(), &context.legacy)?;
    let installed = InstallRecord::read(&layout.install_record())?;
    let retired = RetiredDevelopment::find(layout);
    let version_directories = version_directories::find(layout);
    let mut deletions = retired.deletions.clone();
    deletions.extend(version_directories.iter().cloned());
    // The break migration archives the log of a 1.x state directory that is itself a version
    // directory; it is not archived a second time.
    let version_audit_archives = version_directories::archives(layout, &version_directories)
        .into_iter()
        .filter(|transfer| {
            !migration
                .audit_archive
                .as_ref()
                .is_some_and(|archive| same_path(&archive.from, &transfer.from))
        })
        .collect();
    let mut plan = InstallPlan {
        mode,
        version: context.build.version.clone(),
        migration,
        unchanged: false,
        swaps: Vec::new(),
        edge_replaced: None,
        restart: true,
        record: None,
        processes_to_stop: Vec::new(),
        deletions,
        version_audit_archives,
        registrations_to_rewrite: Vec::new(),
    };
    validate_payload(&context.payload, &layout.bin())?;
    let channel = if mode == Mode::Development {
        Channel::Development
    } else {
        Channel::Release
    };
    let edge = edge_replacement(context, installed.as_ref(), channel)?;
    let shell = shell_changes(context);
    match mode {
        Mode::Release => {
            if installed
                .as_ref()
                .is_some_and(|record| record.channel == Channel::Development)
            {
                return Err(DevelopmentInstalled.into());
            }
            let image = installed_image(layout)?;
            let payload_record = context.build.record(None, Channel::Release);
            match record::decide(installed.as_ref(), &payload_record, image.as_deref()) {
                Standing::Newer => {
                    let installed = installed.as_ref().expect("a newer standing has a record");
                    return Err(PayloadRefused::new(installed, &payload_record).into());
                }
                // Only an intact set is left alone: a missing or changed edge or shell is damage,
                // and the install puts it back.
                Standing::Same
                    if edge.is_none()
                        && !shell
                        && plan.migration.is_empty()
                        && retired.is_empty()
                        && version_directories.is_empty() =>
                {
                    plan.unchanged = true;
                    plan.restart = false;
                    return Ok(plan);
                }
                Standing::Same if edge.is_none() && !shell => {}
                Standing::Absent | Standing::Older | Standing::Damaged | Standing::Same => {
                    place_in_bin(context, &mut plan, installed.as_ref(), edge, shell, channel)?;
                }
            }
        }
        Mode::Development | Mode::Restore => {
            place_in_bin(context, &mut plan, installed.as_ref(), edge, shell, channel)?;
        }
    }
    plan.processes_to_stop = plan.migration.images_to_stop.clone();
    for image in running_images(layout)
        .into_iter()
        .chain(retired.images.iter().cloned())
    {
        if !plan.processes_to_stop.contains(&image) {
            plan.processes_to_stop.push(image);
        }
    }
    plan.registrations_to_rewrite = context.registrations.pending();
    Ok(plan)
}

/// Carry out `plan`.
///
/// The migration's destinations are checked before any swap, so a conflict leaves every
/// executable as it was. The 1.x policy, managed-policy and client-location files are copied
/// before anything is swapped, and their originals stay: the registrations that follow let a 2.0
/// service or a client start, and they must find the user's policy and located clients in place,
/// while the 1.x service that is still running keeps reading its own policy from the original.
/// The registrations are written as soon as `bin` holds the new executables and before any old
/// program is stopped: an old connector that a browser relaunches from a registration that still
/// named it would write the 1.x installation record again. Once the last old program has stopped
/// and cannot write them again, the originals are retired (a file a 1.x program rewrote after
/// the copy is published over the copy first), the 1.x audit log, which an old service writes,
/// is archived, the 1.x lifecycle files are deleted, and what the 1.x Flatpak route left in the
/// home is removed or restored, each file reported. A migrating plan always restarts: a plan
/// with nothing to place and a migration to run never reaches the unchanged shortcut.
///
/// The retired development directory and marker go the same way: once `bin` holds the new
/// executables and the registrations name them, the service is asked to quit (it may be running
/// from that directory), whatever else runs from there is stopped, and the directory and the
/// marker are removed. The 1.x version directories follow the same order and are removed last,
/// after the migration has archived its own audit log: the log each holds is archived first, and
/// a directory whose log could not be archived stays. An entry that cannot be removed is
/// reported in [`Outcome::left_behind`]; it does not fail the install.
pub fn execute(context: &Context<'_>, plan: InstallPlan) -> anyhow::Result<Outcome> {
    let layout = &context.layout;
    if plan.unchanged {
        // The same version is in place, but what 1.x left still goes.
        let retired_leftovers = migration::retire_leftovers(&plan.migration);
        return Ok(Outcome {
            plan,
            service_started: None,
            start_failure: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
            archived_version_audits: Vec::new(),
            strays_left: Vec::new(),
            retired_leftovers,
        });
    }
    let migrating = !plan.migration.is_empty();
    debug_assert!(
        plan.restart || !migrating,
        "a migration is finished only by the restart"
    );
    let mut copied = Vec::new();
    let mut flatpak = Vec::new();
    let mut left_behind = Vec::new();
    let mut archived_version_audits = Vec::new();
    let mut strays_left = Vec::new();
    let mut start_failure = None;
    if migrating {
        migration::prepare(&plan.migration)?;
        copied = migration::copy_state(&plan.migration)?;
    }
    for placed in &plan.swaps {
        swap::rename_swap(&placed.directory, &placed.name, &placed.source)?;
    }
    let registrations = context
        .registrations
        .reconcile(layout, &context.build.version);
    stop_legacy(context.control, &legacy_images(&plan))?;
    let service_started = if plan.restart {
        let replaced = context.control.instance(layout);
        if !context.control.quit(layout) {
            context.control.stop_image(&running_images(layout))?;
        }
        // A program that will not stop is reported; it blocks nothing here.
        strays_left = stop_found_strays(context.control, layout, None).stuck;
        if migrating {
            migration::retire_state(&copied)?;
            migration::archive_audit(&plan.migration)?;
            migration::finish(&plan.migration)?;
            flatpak = migration::retire_flatpak_route(&plan.migration);
        }
        (left_behind, archived_version_audits) = remove_deletions(layout, &plan);
        // A reason left by an earlier failed start must not be read as this one's.
        let _ = fs::remove_file(layout.start_failure());
        start_failure = match context.control.start_serve(layout) {
            Err(error) => Some(StartFailure::Spawn(error.to_string())),
            Ok(()) => match context.control.wait_ready(layout, replaced.as_deref()) {
                Readiness::Answered => None,
                Readiness::Stopped => Some(StartFailure::Stopped(recorded_start_failure(layout))),
                Readiness::Silent => Some(StartFailure::Silent),
            },
        };
        Some(start_failure.is_none())
    } else {
        None
    };
    if service_started == Some(true) {
        if let Some(record) = &plan.record {
            record.write(&layout.install_record())?;
        }
    }
    // After a migration's own steps, so a policy folder whose files it just retired is empty.
    let retired_leftovers = migration::retire_leftovers(&plan.migration);
    Ok(Outcome {
        plan,
        service_started,
        start_failure,
        registrations: Some(registrations),
        flatpak,
        left_behind,
        archived_version_audits,
        strays_left,
        retired_leftovers,
    })
}

/// The 1.x images to stop once this install has placed its own executables.
///
/// A 1.x installation may have served from `bin` itself. The swap moved those images aside, so
/// the programs still running them are found under their `.old-<n>` names; the executables at
/// the `bin` paths are now this install's and must not be stopped.
fn legacy_images(plan: &InstallPlan) -> Vec<PathBuf> {
    let placed = &plan.swaps;
    let mut images = Vec::new();
    for image in &plan.migration.images_to_stop {
        let displaced = placed
            .iter()
            .find(|swap| same_path(image, &swap.directory.join(&swap.name)));
        match displaced {
            Some(swap) => images.extend(
                swap::images(&swap.directory, &swap.name)
                    .into_iter()
                    .filter(|moved| !same_path(moved, image)),
            ),
            None => images.push(image.clone()),
        }
    }
    images
}

/// Stop the 1.x programs running `images`, then look again for a few rounds for any that were
/// started from them in the meantime and stop those too.
fn stop_legacy(control: &dyn ProcessControl, images: &[PathBuf]) -> anyhow::Result<()> {
    if images.is_empty() {
        return Ok(());
    }
    let stop = || {
        control
            .stop_image(images)
            .context(words::OLD_PROGRAMS_WOULD_NOT_STOP)
    };
    stop()?;
    for _ in 0..LEGACY_RESPAWN_ROUNDS {
        std::thread::sleep(POLL);
        if control.is_running(images) {
            stop()?;
        }
    }
    Ok(())
}

/// The swap placing the payload's service in `bin`.
fn service_swap(context: &Context<'_>) -> Swap {
    Swap {
        directory: context.layout.bin(),
        name: SERVICE_EXECUTABLE_NAME.to_owned(),
        source: context.payload.join(SERVICE_EXECUTABLE_NAME),
    }
}

/// The swap placing the payload's shell in `bin`.
fn desktop_swap(context: &Context<'_>) -> Swap {
    Swap {
        directory: context.layout.bin(),
        name: DESKTOP_EXECUTABLE_NAME.to_owned(),
        source: context.payload.join(DESKTOP_EXECUTABLE_NAME),
    }
}

/// The swap placing the payload's edge in `bin`.
fn edge_swap(context: &Context<'_>) -> Swap {
    Swap {
        directory: context.layout.bin(),
        name: EDGE_EXECUTABLE_NAME.to_owned(),
        source: context.payload.join(EDGE_EXECUTABLE_NAME),
    }
}

/// Whether the payload has a shell that `bin` does not already hold.
fn shell_changes(context: &Context<'_>) -> bool {
    let source = context.payload.join(DESKTOP_EXECUTABLE_NAME);
    let installed = context.layout.bin().join(DESKTOP_EXECUTABLE_NAME);
    source.is_file()
        && !matches!(
            (record::sha256_file(&source), record::sha256_file(&installed)),
            (Ok(payload), Ok(installed)) if payload == installed
        )
}

/// Why an install recording `channel` replaces the edge in `bin`, or `None` when the edge
/// stays: `bin` still holds the pre-edge layout, nothing records the installed edge, the edge
/// speaks another relay revision, the edge there is missing or is not the one the record says
/// was installed, or, on the development channel only, the payload's edge is another build than
/// the installed one. A release keeps an intact edge of its relay revision whatever its bytes.
fn edge_replacement(
    context: &Context<'_>,
    installed: Option<&InstallRecord>,
    channel: Channel,
) -> io::Result<Option<EdgeReplacement>> {
    if migration::pre_edge_layout(&context.layout) {
        return Ok(Some(EdgeReplacement::PreEdgeLayout));
    }
    let Some(installed) = installed else {
        return Ok(Some(EdgeReplacement::Unrecorded));
    };
    if installed.relay_revision != context.build.relay_revision {
        return Ok(Some(EdgeReplacement::RelayRevision));
    }
    let Some(recorded) = installed.edge_sha256.as_deref() else {
        return Ok(Some(EdgeReplacement::Unrecorded));
    };
    match record::sha256_file(&context.layout.edge()) {
        Ok(sha) if sha != recorded => return Ok(Some(EdgeReplacement::Altered)),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some(EdgeReplacement::Missing));
        }
        Err(error) => return Err(error),
    }
    let leaving_development =
        channel != Channel::Development && installed.channel == Channel::Development;
    if (channel == Channel::Development || leaving_development)
        && record::sha256_file(&context.payload.join(EDGE_EXECUTABLE_NAME))? != recorded
    {
        return Ok(Some(if leaving_development {
            EdgeReplacement::LeavingDevelopment
        } else {
            EdgeReplacement::Rebuilt
        }));
    }
    Ok(None)
}

/// Add the swaps that place the payload in `bin` to `plan`, and set the record `bin` holds once
/// they are done: the service always, the shell when `shell` says the payload has one `bin` does
/// not ([`shell_changes`]), and the edge when `edge` gives a reason (the edge it leaves is the
/// payload's, or the installed one).
fn place_in_bin(
    context: &Context<'_>,
    plan: &mut InstallPlan,
    installed: Option<&InstallRecord>,
    edge: Option<EdgeReplacement>,
    shell: bool,
    channel: Channel,
) -> anyhow::Result<()> {
    plan.swaps.push(service_swap(context));
    if shell {
        plan.swaps.push(desktop_swap(context));
    }
    plan.edge_replaced = edge;
    let edge_sha256 = if edge.is_some() {
        plan.swaps.push(edge_swap(context));
        Some(record::sha256_file(
            &context.payload.join(EDGE_EXECUTABLE_NAME),
        )?)
    } else {
        installed.and_then(|record| record.edge_sha256.clone())
    };
    plan.record = Some(context.build.record(edge_sha256, channel));
    Ok(())
}

/// What earlier 2.0 development builds left in the home: the images that may still run from the
/// retired development directory, and the directory and marker to remove.
struct RetiredDevelopment {
    /// Every service, edge and shell image in the directory, with the images earlier swaps moved
    /// aside.
    images: Vec<PathBuf>,
    /// The directory and the marker, as far as they exist.
    deletions: Vec<PathBuf>,
}

impl RetiredDevelopment {
    /// Look in `layout`'s home.
    fn find(layout: &Layout) -> Self {
        let directory = layout.retired_development_directory();
        let marker = layout.retired_development_marker();
        let images = [
            SERVICE_EXECUTABLE_NAME,
            EDGE_EXECUTABLE_NAME,
            DESKTOP_EXECUTABLE_NAME,
        ]
        .into_iter()
        .flat_map(|name| swap::images(&directory, name))
        .collect();
        let deletions = [directory, marker]
            .into_iter()
            .filter(|path| fs::symlink_metadata(path).is_ok())
            .collect();
        Self { images, deletions }
    }

    /// Whether nothing is left to clean up.
    fn is_empty(&self) -> bool {
        self.deletions.is_empty()
    }
}

/// Remove what `plan` lists as deletions: the retired development directory and its marker, and
/// the 1.x version directories after their audit logs are archived. Returns the entries that
/// could not be removed and the audit logs that were archived. The programs that ran from there were stopped by pid with the other
/// strays, and one that would not stop is reported with them, so a removal that fails here names
/// only what stayed.
fn remove_deletions(layout: &Layout, plan: &InstallPlan) -> (Vec<PathBuf>, Vec<Move>) {
    let (versions, others): (Vec<PathBuf>, Vec<PathBuf>) = plan
        .deletions
        .iter()
        .cloned()
        .partition(|path| version_directories::is_version_directory(layout, path));
    let mut kept: Vec<PathBuf> = others
        .into_iter()
        .filter(|path| {
            let removed = if path.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            removed.is_err_and(|error| error.kind() != io::ErrorKind::NotFound)
        })
        .collect();
    let retired = version_directories::retire(&versions, &plan.version_audit_archives);
    kept.extend(retired.kept);
    (kept, retired.archived)
}

/// Every image a running service of this home may have been started from: the ones in `bin` and
/// the service images of the retired development directory.
fn running_images(layout: &Layout) -> Vec<PathBuf> {
    let mut images = swap::images(&layout.bin(), SERVICE_EXECUTABLE_NAME);
    images.extend(swap::images(
        &layout.retired_development_directory(),
        SERVICE_EXECUTABLE_NAME,
    ));
    images
}

/// The SHA-256 of the service image in `bin`, when there is one.
fn installed_image(layout: &Layout) -> io::Result<Option<String>> {
    match record::sha256_file(&layout.service_executable()) {
        Ok(sha) => Ok(Some(sha)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Require the service and the edge in `payload`, which must not be `destination` itself.
fn validate_payload(payload: &Path, destination: &Path) -> Result<(), PayloadError> {
    for name in [SERVICE_EXECUTABLE_NAME, EDGE_EXECUTABLE_NAME] {
        let path = payload.join(name);
        if !path.is_file() {
            return Err(PayloadError::Missing(path));
        }
    }
    if same_path(&canonical_path(payload), &canonical_path(destination)) {
        return Err(PayloadError::InPlace(destination.to_owned()));
    }
    Ok(())
}

/// Take the install lock at `path`, waiting up to [`INSTALL_LOCK_WAIT`] for another install.
fn hold_install_lock(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        private::create_dir_all(parent)?;
    }
    let file = private::file_options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let deadline = Instant::now() + INSTALL_LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(POLL);
            }
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    words::INSTALL_LOCK_BUSY,
                ))
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;

    use super::super::migration::{
        LegacyInstallation, MigrationError, AUDIT_FILE_NAME, HARNESS_LOCATIONS_NAME,
        LEGACY_DEPLOY_LOCK_NAME, LEGACY_RECORD_NAME, LEGACY_STARTUP_NAME, USER_POLICY_FILE_NAME,
    };
    use super::super::native_host::NativeHostRegistry;
    use super::super::HarnessRegistry;
    use super::*;
    use crate::intake::listener::{self, control};
    use crate::intake::opening::read_opening;
    use crate::scratch::Scratch;
    use ghostlight_relay::endpoint::Listener;
    use ghostlight_relay::protocol::{self, Reply, Role};

    /// Version of the payload under test.
    const VERSION: &str = "2.1.0";

    /// Relay revision of the payload under test.
    const RELAY: &str = "4";

    /// What the payload's edge holds.
    fn edge_text(relay: &str) -> String {
        format!("edge {relay}")
    }

    /// The image names a 1.x installation held: the service under the product's name, which is
    /// now the edge's, and both connectors.
    fn legacy_image_names() -> Vec<String> {
        std::iter::once(EDGE_EXECUTABLE_NAME.to_owned())
            .chain(retired_connector_names())
            .collect()
    }

    /// What the fake process control was asked to do.
    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Quit,
        Stop(Vec<PathBuf>),
        Running,
        StopStrays(Vec<u32>),
        Start,
        Wait,
    }

    /// What the disk showed at the moment of one request.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Sighting {
        /// A native-host manifest already names the installed edge.
        registered: bool,
        /// The 1.x installation record still exists.
        record_present: bool,
        /// The 1.x user policy still exists where the 1.x service reads it.
        policy_original_present: bool,
        /// The user policy already exists where 2.0 keeps it.
        policy_copy_present: bool,
    }

    /// Process control that records every request and starts nothing.
    struct FakeControl {
        quits: bool,
        answers: bool,
        /// When the new service does not answer: whether it stopped (rather than staying
        /// silent), and the reason it records in the home while stopping, if any.
        stops_recording: Option<Option<String>>,
        /// The system's reason the new service's process could not be created, if it cannot.
        spawn_error: Option<String>,
        /// The instance of the running service the installer replaces, when there is one.
        instance: Option<String>,
        /// What each wait for the new service was told to treat as stale.
        waited_for: RefCell<Vec<Option<String>>>,
        /// How many looks for a respawned 1.x program find one.
        respawns: Cell<u32>,
        /// The manifest, the edge it must name and the 1.x record to look at on every request.
        witness: Option<(PathBuf, PathBuf, PathBuf)>,
        /// The 1.x user policy and its 2.0 place, to look at on every request.
        policy_witness: Option<(PathBuf, PathBuf)>,
        /// Files a 1.x program writes while it is being stopped.
        writes_on_stop: Vec<(PathBuf, String)>,
        /// How many of the next requests to stop an image fail.
        failing_stops: Cell<u32>,
        /// The programs of an older installation the fake finds running.
        strays: Vec<StrayProcess>,
        /// Whether the programs of an older installation refuse to stop.
        strays_stay: bool,
        /// A directory whose presence the fake notes each time it is asked to stop strays.
        watched_directory: Option<PathBuf>,
        /// Whether `watched_directory` existed at each stop of strays.
        watched_present: RefCell<Vec<bool>>,
        /// A file whose text the fake reads at the quit.
        quit_sees: Option<PathBuf>,
        /// The text of `quit_sees` when the quit was asked, if the file was there.
        seen_at_quit: RefCell<Option<String>>,
        calls: RefCell<Vec<Call>>,
        sightings: RefCell<Vec<Sighting>>,
    }

    impl FakeControl {
        fn new() -> Self {
            Self {
                quits: true,
                answers: true,
                stops_recording: None,
                spawn_error: None,
                instance: None,
                waited_for: RefCell::new(Vec::new()),
                respawns: Cell::new(0),
                witness: None,
                policy_witness: None,
                writes_on_stop: Vec::new(),
                failing_stops: Cell::new(0),
                strays: Vec::new(),
                strays_stay: false,
                watched_directory: None,
                watched_present: RefCell::new(Vec::new()),
                quit_sees: None,
                seen_at_quit: RefCell::new(None),
                calls: RefCell::new(Vec::new()),
                sightings: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }

        fn sightings(&self) -> Vec<Sighting> {
            self.sightings.borrow().clone()
        }

        /// Record `call` and what the disk shows now.
        fn note(&self, call: Call) {
            if let Some((manifest, connector, record)) = &self.witness {
                self.sightings.borrow_mut().push(Sighting {
                    registered: fs::read_to_string(manifest).is_ok_and(|text| {
                        serde_json::from_str::<serde_json::Value>(&text)
                            .ok()
                            .and_then(|value| value["path"].as_str().map(PathBuf::from))
                            .and_then(|path| fs::canonicalize(path).ok())
                            .is_some_and(|path| Some(path) == fs::canonicalize(connector).ok())
                    }),
                    record_present: record.is_file(),
                    policy_original_present: self
                        .policy_witness
                        .as_ref()
                        .is_some_and(|(original, _)| original.is_file()),
                    policy_copy_present: self
                        .policy_witness
                        .as_ref()
                        .is_some_and(|(_, copy)| copy.is_file()),
                });
            }
            self.calls.borrow_mut().push(call);
        }
    }

    impl ProcessControl for FakeControl {
        fn quit(&self, _layout: &Layout) -> bool {
            if let Some(path) = &self.quit_sees {
                *self.seen_at_quit.borrow_mut() = fs::read_to_string(path).ok();
            }
            self.note(Call::Quit);
            self.quits
        }

        fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()> {
            self.note(Call::Stop(images.to_vec()));
            if self.failing_stops.get() > 0 {
                self.failing_stops.set(self.failing_stops.get() - 1);
                anyhow::bail!("Could not stop Ghostlight process");
            }
            for (path, text) in &self.writes_on_stop {
                fs::write(path, text).unwrap();
            }
            Ok(())
        }

        fn is_running(&self, _images: &[PathBuf]) -> bool {
            self.note(Call::Running);
            let remaining = self.respawns.get();
            self.respawns.set(remaining.saturating_sub(1));
            remaining > 0
        }

        fn strays(&self, _layout: &Layout, _service_pid: Option<u32>) -> Vec<StrayProcess> {
            self.strays.clone()
        }

        fn stop_strays(&self, strays: &[StrayProcess]) -> Vec<StrayProcess> {
            if let Some(directory) = &self.watched_directory {
                self.watched_present.borrow_mut().push(directory.is_dir());
            }
            self.note(Call::StopStrays(
                strays.iter().map(|stray| stray.pid).collect(),
            ));
            if self.strays_stay {
                strays.to_vec()
            } else {
                Vec::new()
            }
        }

        fn start_serve(&self, _layout: &Layout) -> io::Result<()> {
            self.note(Call::Start);
            match &self.spawn_error {
                Some(reason) => Err(io::Error::other(reason.clone())),
                None => Ok(()),
            }
        }

        fn instance(&self, _layout: &Layout) -> Option<String> {
            self.instance.clone()
        }

        fn wait_ready(&self, layout: &Layout, previous: Option<&str>) -> Readiness {
            self.note(Call::Wait);
            self.waited_for
                .borrow_mut()
                .push(previous.map(str::to_owned));
            if self.answers {
                return Readiness::Answered;
            }
            match &self.stops_recording {
                None => Readiness::Silent,
                Some(reason) => {
                    if let Some(reason) = reason {
                        fs::create_dir_all(layout.run()).unwrap();
                        fs::write(layout.start_failure(), format!("{reason}\n")).unwrap();
                    }
                    Readiness::Stopped
                }
            }
        }
    }

    /// A temporary home, a payload directory and isolated registrations.
    struct Fixture {
        scratch: Scratch,
        layout: Layout,
        payload: PathBuf,
        registrations: Registrations,
        legacy_policy: Option<PathBuf>,
        /// The directory a 1.x installation served from, holding its three executables.
        legacy_serving: Option<PathBuf>,
        /// The directory the 1.x installation record names as holding its audit log.
        legacy_state: Option<PathBuf>,
        /// The directory that holds the browser manifests the registrations write.
        hosts: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let scratch = Scratch::new();
            let root = fs::canonicalize(scratch.path()).unwrap();
            let layout = Layout::new(root.join("home"));
            let payload = root.join("payload");
            fs::create_dir_all(&payload).unwrap();
            Self::write_payload(&payload, "service");
            let hosts = root.join("hosts");
            let registrations = Registrations::isolated(
                NativeHostRegistry::isolated(&hosts, &layout.edge()),
                HarnessRegistry::isolated(&root.join("clients"), &layout.edge(), &layout.state()),
            );
            Self {
                scratch,
                layout,
                payload,
                registrations,
                legacy_policy: None,
                legacy_serving: None,
                legacy_state: None,
                hosts,
            }
        }

        fn write_payload(payload: &Path, service: &str) {
            fs::write(payload.join(SERVICE_EXECUTABLE_NAME), service).unwrap();
            fs::write(payload.join(EDGE_EXECUTABLE_NAME), edge_text(RELAY)).unwrap();
        }

        fn build(&self, version: &str) -> Build {
            Build {
                version: version.to_owned(),
                relay_revision: RELAY.to_owned(),
                image_sha256: record::sha256_file(&self.payload.join(SERVICE_EXECUTABLE_NAME))
                    .unwrap(),
            }
        }

        /// Put a 1.x installation record in the home and its executables in `serving`.
        fn plant_legacy_installation(&mut self, serving: &Path) {
            fs::create_dir_all(serving).unwrap();
            for name in legacy_image_names() {
                fs::write(serving.join(&name), format!("1.x {name}")).unwrap();
            }
            fs::create_dir_all(self.layout.home()).unwrap();
            fs::write(self.legacy_record(), "{}").unwrap();
            self.legacy_serving = Some(serving.to_owned());
        }

        fn legacy_record(&self) -> PathBuf {
            self.layout.home().join(LEGACY_RECORD_NAME)
        }

        /// A control that looks at the manifest and the 1.x record on every request.
        fn watching_control(&self) -> FakeControl {
            let manifest = NativeHostRegistry::isolated(&self.hosts, &self.layout.edge())
                .isolated_manifest("chrome");
            let mut control = FakeControl::new();
            control.witness = Some((manifest, self.layout.edge(), self.legacy_record()));
            control
        }

        fn legacy(&self) -> Legacy {
            Legacy {
                installation: (self.legacy_policy.is_some()
                    || self.legacy_serving.is_some()
                    || self.legacy_state.is_some())
                .then(|| LegacyInstallation {
                    policy_directory: self.legacy_policy.clone(),
                    serving_directory: self.legacy_serving.clone(),
                    state_directory: self.legacy_state.clone(),
                    ..LegacyInstallation::default()
                }),
                roots: LegacyRoots::within(&self.scratch.path().join("profile")),
                registrations: Vec::new(),
            }
        }

        fn context<'a>(&'a self, build: Build, control: &'a FakeControl) -> Context<'a> {
            Context {
                layout: self.layout.clone(),
                build,
                payload: self.payload.clone(),
                legacy: self.legacy(),
                control,
                registrations: &self.registrations,
            }
        }

        fn run(&self, mode: Mode, version: &str, control: &FakeControl) -> anyhow::Result<Outcome> {
            install(
                &self.context(self.build(version), control),
                &Request {
                    payload: None,
                    mode,
                    plan_only: false,
                },
            )
        }

        /// Whether the manifest the registrations wrote names the installed edge.
        fn manifest_names_bin_edge(&self) -> bool {
            let manifest = NativeHostRegistry::isolated(&self.hosts, &self.layout.edge())
                .isolated_manifest("chrome");
            let value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
            fs::canonicalize(value["path"].as_str().unwrap()).unwrap()
                == fs::canonicalize(self.layout.edge()).unwrap()
        }

        fn read(&self, path: PathBuf) -> String {
            fs::read_to_string(path).unwrap()
        }

        fn record(&self) -> Option<InstallRecord> {
            InstallRecord::read(&self.layout.install_record()).unwrap()
        }

        /// Every file under the home, with its contents, to show that nothing changed.
        fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
            fn walk(directory: &Path, found: &mut Vec<(PathBuf, Vec<u8>)>) {
                let Ok(entries) = fs::read_dir(directory) else {
                    return;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, found);
                    } else {
                        found.push((path.clone(), fs::read(&path).unwrap()));
                    }
                }
            }
            let mut found = Vec::new();
            walk(self.scratch.path(), &mut found);
            found.sort();
            found
        }
    }

    /// The names of the files a plan places, in order.
    fn placed_names(plan: &InstallPlan) -> Vec<&str> {
        plan.swaps
            .iter()
            .map(|placed| placed.name.as_str())
            .collect()
    }

    /// Put what an earlier 2.0 development build left in the home: a service, an edge and a shell
    /// in the retired directory, an image a swap moved aside, and the marker. Returns the images.
    fn plant_retired_development(fixture: &Fixture) -> Vec<PathBuf> {
        let directory = fixture.layout.retired_development_directory();
        fs::create_dir_all(&directory).unwrap();
        let mut images: Vec<PathBuf> = [
            SERVICE_EXECUTABLE_NAME,
            EDGE_EXECUTABLE_NAME,
            DESKTOP_EXECUTABLE_NAME,
        ]
        .iter()
        .map(|name| directory.join(name))
        .collect();
        images.push(directory.join(format!("{SERVICE_EXECUTABLE_NAME}.old-1")));
        for image in &images {
            fs::write(image, "development image").unwrap();
        }
        fs::write(fixture.layout.retired_development_marker(), "{}").unwrap();
        images
    }

    /// Put what the 1.x launchers left in `bin`: `v1.3.5` with an audit log and runtime files,
    /// `v1.3.6` with executables, and two entries that are not version directories.
    fn plant_version_directories(fixture: &Fixture) -> (PathBuf, PathBuf) {
        let bin = fixture.layout.bin();
        let first = bin.join("v1.3.5");
        let second = bin.join("v1.3.6");
        for directory in [&first, &second] {
            fs::create_dir_all(directory).unwrap();
            for name in legacy_image_names() {
                fs::write(directory.join(name), "1.x image").unwrap();
            }
        }
        fs::write(first.join(AUDIT_FILE_NAME), VERSION_AUDIT).unwrap();
        for name in [
            "ghostlight-runtime.json",
            "ghostlight-runtime.lock",
            "ghostlight-runtime.startup",
        ] {
            fs::write(first.join(name), "runtime").unwrap();
        }
        for name in NOT_VERSION_NAMES {
            fs::create_dir_all(bin.join(name)).unwrap();
            fs::write(bin.join(name).join("keep"), "keep").unwrap();
        }
        fs::write(bin.join("v1.3.7"), "a file").unwrap();
        (first, second)
    }

    /// What `v1.3.5` held as its audit log.
    const VERSION_AUDIT: &[u8] = b"{\"event\":\"1.x\"}\n";

    /// Directories in `bin` whose names are not a `v` and a semantic version.
    const NOT_VERSION_NAMES: [&str; 3] = ["vnext", "v1.4", "1.3.5"];

    /// The entries of `bin` that are not version directories, to show they stay.
    fn untouched_entries(fixture: &Fixture) -> Vec<PathBuf> {
        let bin = fixture.layout.bin();
        let mut entries: Vec<PathBuf> = NOT_VERSION_NAMES
            .iter()
            .map(|name| bin.join(name).join("keep"))
            .collect();
        entries.push(bin.join("v1.3.7"));
        entries
    }

    /// A 1.x installation that served from `old-1x` and kept its policy in `old-policy`, and the
    /// place its user policy is read from.
    fn plant_legacy_policy(fixture: &mut Fixture) -> PathBuf {
        fixture.plant_legacy_installation(&fixture.scratch.path().join("old-1x"));
        let policy = fixture.scratch.path().join("old-policy");
        fs::create_dir_all(&policy).unwrap();
        fs::write(policy.join(USER_POLICY_FILE_NAME), "user policy").unwrap();
        fixture.legacy_policy = Some(policy.clone());
        policy.join(USER_POLICY_FILE_NAME)
    }

    /// Run an install that fails to stop the 1.x service after the copies were made, and return
    /// the 1.x original and the copy of the user policy it left.
    fn failed_run_after_the_copy(fixture: &mut Fixture) -> (PathBuf, PathBuf) {
        let original = plant_legacy_policy(fixture);
        let copy = fixture.layout.policy().join(USER_POLICY_FILE_NAME);
        let control = FakeControl::new();
        control.failing_stops.set(1);
        let error = fixture.run(Mode::Release, VERSION, &control).unwrap_err();
        assert!(error.to_string().contains("Could not stop"), "{error}");
        assert_eq!(fixture.read(original.clone()), "user policy");
        assert_eq!(fixture.read(copy.clone()), "user policy");
        (original, copy)
    }

    /// A stand-in for the running service: it answers `answers` control openings on the home of
    /// `layout`, handing each opening to `seen`, and returns once they are all answered.
    fn fake_service(
        layout: &Layout,
        answers: usize,
        seen: impl Fn(&protocol::Opening) + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        fs::create_dir_all(layout.home()).unwrap();
        let listener = Listener::bind_first(layout.home()).unwrap();
        std::thread::spawn(move || {
            for _ in 0..answers {
                let connection = listener.accept().unwrap();
                let opening = read_opening(&connection, Duration::from_secs(5)).unwrap();
                seen(&opening);
                let reply = Reply::accepted("2.0.0".to_owned(), "instance_new".to_owned(), None);
                listener::accept(&connection, &reply).unwrap();
                listener::answer_control(
                    &connection,
                    &control::Status {
                        service: "2.0.0".to_owned(),
                        instance: "instance_new".to_owned(),
                        pid: std::process::id(),
                        started_by: StartReason::Update,
                        connections: None,
                        workbench: None,
                        window: None,
                    },
                )
                .unwrap();
            }
        })
    }

    /// What the plan places, as file name and directory in order.
    fn placed(plan: &InstallPlan) -> Vec<(String, PathBuf)> {
        plan.swaps
            .iter()
            .map(|swap| (swap.name.clone(), swap.directory.clone()))
            .collect()
    }

    /// Put a shell in the fixture's payload.
    fn add_shell(fixture: &Fixture, text: &str) {
        fs::write(fixture.payload.join(DESKTOP_EXECUTABLE_NAME), text).unwrap();
    }

    /// Install the release with a shell, `damage` the shell in `bin`, and install the same
    /// version again: the shell is put back beside the service, the intact edge stays, and the
    /// next install of that version is a no-op again.
    fn assert_a_same_version_install_restores_the_shell(damage: impl Fn(&Path)) {
        let fixture = Fixture::new();
        add_shell(&fixture, "shell");
        fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        let shell = fixture.layout.bin().join(DESKTOP_EXECUTABLE_NAME);
        damage(&shell);

        let outcome = fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        assert!(!outcome.plan.unchanged);
        assert_eq!(
            placed_names(&outcome.plan),
            [SERVICE_EXECUTABLE_NAME, DESKTOP_EXECUTABLE_NAME]
        );
        assert_eq!(fixture.read(shell), "shell");
        assert_eq!(outcome.service_started, Some(true));

        let outcome = fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        assert!(outcome.plan.unchanged);
    }

    /// A temporary home with its `bin` and retired development directories.
    fn stray_home() -> (Scratch, Layout) {
        let scratch = Scratch::new();
        let root = fs::canonicalize(scratch.path()).unwrap();
        let layout = Layout::new(root.join("home"));
        fs::create_dir_all(layout.bin()).unwrap();
        fs::create_dir_all(layout.retired_development_directory()).unwrap();
        (scratch, layout)
    }

    /// The stray kind of a process running `image`, which need not exist.
    fn kind_of(
        layout: &Layout,
        image: &Path,
        pid: u32,
        service_pid: Option<u32>,
    ) -> Option<StrayKind> {
        classify(image, pid, layout, service_pid, std::process::id())
    }

    /// The system program the real-process test copies under a retired connector's name, with the
    /// arguments that keep it waiting, without a console, for about a minute if the test never
    /// stops it.
    #[cfg(windows)]
    fn sleeper() -> (PathBuf, &'static [&'static str]) {
        let windows = env::var_os("SystemRoot").expect("Windows sets SystemRoot");
        let image = Path::new(&windows).join("System32").join("PING.EXE");
        (image, &["-n", "60", "127.0.0.1"])
    }

    /// The system program the real-process test copies under a retired connector's name, with the
    /// arguments that keep it waiting for about a minute if the test never stops it.
    #[cfg(not(windows))]
    fn sleeper() -> (PathBuf, &'static [&'static str]) {
        (PathBuf::from("/bin/sleep"), &["60"])
    }

    /// How long a test waits for the stand-in to show in the process table.
    const STAND_IN_APPEARS_WITHIN: Duration = Duration::from_secs(20);

    /// How many times starting the stand-in is tried when the kernel reports its file busy.
    const STAND_IN_START_ATTEMPTS: u32 = 20;

    /// Pause between those tries.
    const STAND_IN_START_PAUSE: Duration = Duration::from_millis(50);

    /// A started stand-in, killed and reaped when the test ends however it ends.
    struct StandIn(std::process::Child);

    impl Drop for StandIn {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Copy the system [`sleeper`] to `image` and start the copy waiting.
    ///
    /// Another test thread may fork between the copy and the start and hand the copy's open
    /// descriptor to its child for a moment, and then the kernel refuses to execute the file as
    /// busy. That passes, so the start is tried again a bounded number of times.
    fn start_stand_in(image: &Path) -> StandIn {
        let (sleeper, arguments) = sleeper();
        fs::copy(sleeper, image).unwrap();
        let mut attempts = 1;
        loop {
            let started = std::process::Command::new(image)
                .args(arguments)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            match started {
                Ok(child) => return StandIn(child),
                Err(error)
                    if error.kind() == io::ErrorKind::ExecutableFileBusy
                        && attempts < STAND_IN_START_ATTEMPTS =>
                {
                    attempts += 1;
                    std::thread::sleep(STAND_IN_START_PAUSE);
                }
                Err(error) => panic!("the stand-in did not start: {error}"),
            }
        }
    }

    /// The stray of `layout`'s home that runs as process `pid`, once the table shows it.
    fn wait_for_stray(layout: &Layout, pid: u32) -> StrayProcess {
        let deadline = Instant::now() + STAND_IN_APPEARS_WITHIN;
        loop {
            if let Some(found) = stray_processes(layout, None)
                .into_iter()
                .find(|stray| stray.pid == pid)
            {
                return found;
            }
            assert!(
                Instant::now() < deadline,
                "the stand-in never showed as a stray"
            );
            std::thread::sleep(POLL);
        }
    }

    /// Protects ACCEPTANCE 14 and promise 1 (ADR-0197 Decision 14): an install acts on what `bin`
    /// holds. A fresh install places the set and records it once the service answers; the same version
    /// is left alone unless its edge or shell is missing or changed, which an install puts back; a
    /// damaged service image is repaired; an older payload, or one missing an executable, is refused
    /// and changes nothing; and a payload's shell lands in `bin` beside the service.
    #[test]
    fn an_install_places_repairs_or_refuses_by_what_bin_holds() {
        {
            let fixture = Fixture::new();
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            assert_eq!(
                fixture.record(),
                Some(InstallRecord {
                    service_version: VERSION.to_owned(),
                    relay_revision: RELAY.to_owned(),
                    service_sha256: fixture.build(VERSION).image_sha256,
                    edge_sha256: Some(record::sha256_file(&fixture.layout.edge()).unwrap()),
                    channel: Channel::Release,
                })
            );
            assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
            assert!(outcome.registrations.is_some());
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let before = fixture.snapshot();
            let mut control = FakeControl::new();
            control.strays = vec![StrayProcess {
                pid: 4242,
                image: fixture
                    .layout
                    .bin()
                    .join(retired_connector_names()[0].as_str()),
                kind: StrayKind::RetiredConnector,
            }];
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert!(outcome.plan.unchanged);
            assert!(control.calls().is_empty());
            assert_eq!(fixture.snapshot(), before);
        }
        {
            for damage in [None, Some("another edge")] {
                let fixture = Fixture::new();
                fixture
                    .run(Mode::Release, VERSION, &FakeControl::new())
                    .unwrap();
                match damage {
                    None => fs::remove_file(fixture.layout.edge()).unwrap(),
                    Some(text) => fs::write(fixture.layout.edge(), text).unwrap(),
                }
                let control = FakeControl::new();
                let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
                assert!(!outcome.plan.unchanged, "{damage:?}");
                assert!(
                    placed_names(&outcome.plan).contains(&EDGE_EXECUTABLE_NAME),
                    "{damage:?}"
                );
                assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
                assert_eq!(outcome.service_started, Some(true));
                let recorded = fixture.record().unwrap().edge_sha256;
                assert_eq!(
                    recorded,
                    Some(record::sha256_file(&fixture.layout.edge()).unwrap())
                );
                // Repaired, so the next identical install is a no-op again.
                let outcome = fixture
                    .run(Mode::Release, VERSION, &FakeControl::new())
                    .unwrap();
                assert!(outcome.plan.unchanged, "{damage:?}");
            }
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let before = fixture.snapshot();
            let control = FakeControl::new();
            let error = fixture.run(Mode::Release, "2.0.0", &control).unwrap_err();
            assert!(error.is::<PayloadRefused>(), "{error}");
            assert!(control.calls().is_empty());
            assert_eq!(fixture.snapshot(), before);
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            fs::write(fixture.layout.service_executable(), "damaged").unwrap();
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(!outcome.plan.unchanged);
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
        }
        {
            let fixture = Fixture::new();
            fs::remove_file(fixture.payload.join(EDGE_EXECUTABLE_NAME)).unwrap();
            let error = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<PayloadError>(),
                Some(PayloadError::Missing(_))
            ));
        }
        {
            let fixture = Fixture::new();
            add_shell(&fixture, "shell");
            let layout = &fixture.layout;
            let named = |name: &str| (name.to_owned(), layout.bin());

            let control = FakeControl::new();
            let planned = plan(
                &fixture.context(fixture.build(VERSION), &control),
                Mode::Development,
            )
            .unwrap();
            assert_eq!(
                placed(&planned),
                [
                    named(SERVICE_EXECUTABLE_NAME),
                    named(DESKTOP_EXECUTABLE_NAME),
                    named(EDGE_EXECUTABLE_NAME),
                ]
            );
            assert!(!layout.retired_development_directory().exists());

            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(
                fixture.read(layout.bin().join(DESKTOP_EXECUTABLE_NAME)),
                "shell"
            );

            // A different shell is swapped; the intact edge stays.
            add_shell(&fixture, "next shell");
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(
                placed(&outcome.plan),
                [
                    named(SERVICE_EXECUTABLE_NAME),
                    named(DESKTOP_EXECUTABLE_NAME)
                ]
            );
            assert_eq!(
                fixture.read(layout.bin().join(DESKTOP_EXECUTABLE_NAME)),
                "next shell"
            );

            // The same shell is left in place.
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(placed(&outcome.plan), [named(SERVICE_EXECUTABLE_NAME)]);
        }
        {
            let fixture = Fixture::new();
            add_shell(&fixture, "shell");
            let control = FakeControl::new();
            let planned = plan(
                &fixture.context(fixture.build(VERSION), &control),
                Mode::Release,
            )
            .unwrap();
            let bin = fixture.layout.bin();
            assert_eq!(
                placed(&planned),
                [
                    (SERVICE_EXECUTABLE_NAME.to_owned(), bin.clone()),
                    (DESKTOP_EXECUTABLE_NAME.to_owned(), bin.clone()),
                    (EDGE_EXECUTABLE_NAME.to_owned(), bin.clone()),
                ]
            );
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(fixture.read(bin.join(DESKTOP_EXECUTABLE_NAME)), "shell");
        }
        {
            let fixture = Fixture::new();
            for mode in [Mode::Release, Mode::Development] {
                let control = FakeControl::new();
                let planned =
                    plan(&fixture.context(fixture.build(VERSION), &control), mode).unwrap();
                assert!(
                    planned
                        .swaps
                        .iter()
                        .all(|swap| swap.name != DESKTOP_EXECUTABLE_NAME),
                    "{mode:?}"
                );
            }
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(!fixture.layout.bin().join(DESKTOP_EXECUTABLE_NAME).exists());
        }
        {
            assert_a_same_version_install_restores_the_shell(|shell| {
                fs::remove_file(shell).unwrap()
            });
        }
        {
            assert_a_same_version_install_restores_the_shell(|shell| {
                fs::write(shell, "another shell").unwrap();
            });
        }
        {
            // The second `Same` arm: the set is the same version, but what earlier builds left
            // still goes, so the install does not take the unchanged shortcut.
            let fixture = Fixture::new();
            add_shell(&fixture, "shell");
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            fs::remove_file(fixture.layout.bin().join(DESKTOP_EXECUTABLE_NAME)).unwrap();
            fs::create_dir_all(fixture.layout.retired_development_directory()).unwrap();

            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(placed_names(&outcome.plan).contains(&DESKTOP_EXECUTABLE_NAME));
            assert_eq!(
                fixture.read(fixture.layout.bin().join(DESKTOP_EXECUTABLE_NAME)),
                "shell"
            );
            assert!(!fixture.layout.retired_development_directory().exists());
        }
    }

    /// Protects ADR-0197 Decision 14 and defect e3e861b7: the edge, which clients and browsers hold
    /// open, is replaced only when it must be. A release keeps it while the relay revision matches and
    /// replaces it when the revision changes or `bin` holds the pre-edge layout, whose connectors go
    /// after an exact image stop; a development install replaces it only when its build changed it or
    /// no record vouches for it; a restore brings back the release's edge; and on Windows a running
    /// edge is moved aside without failing the deploy.
    #[test]
    fn the_edge_is_replaced_only_when_its_relay_record_or_development_build_says() {
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let recorded = fixture.record().unwrap().edge_sha256;
            Fixture::write_payload(&fixture.payload, "next service");
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "rebuilt edge").unwrap();
            let outcome = fixture
                .run(Mode::Release, "2.2.0", &FakeControl::new())
                .unwrap();
            assert_eq!(placed_names(&outcome.plan), [SERVICE_EXECUTABLE_NAME]);
            assert_eq!(outcome.plan.edge_replaced, None);
            assert_eq!(
                fixture.read(fixture.layout.service_executable()),
                "next service"
            );
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            let record = fixture.record().unwrap();
            assert_eq!(record.service_version, "2.2.0");
            assert_eq!(record.edge_sha256, recorded);
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            Fixture::write_payload(&fixture.payload, "next service");
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "next edge").unwrap();
            let control = FakeControl::new();
            let build = Build {
                relay_revision: "5".to_owned(),
                ..fixture.build("2.2.0")
            };
            let outcome = install(
                &fixture.context(build, &control),
                &Request {
                    payload: None,
                    mode: Mode::Release,
                    plan_only: false,
                },
            )
            .unwrap();
            assert_eq!(
                outcome.plan.edge_replaced,
                Some(EdgeReplacement::RelayRevision)
            );
            assert_eq!(fixture.read(fixture.layout.edge()), "next edge");
            let record = fixture.record().unwrap();
            assert_eq!(record.relay_revision, "5");
            assert_eq!(
                record.edge_sha256,
                Some(record::sha256_file(&fixture.layout.edge()).unwrap())
            );
        }
        {
            let fixture = Fixture::new();
            let bin = fixture.layout.bin();
            fs::create_dir_all(&bin).unwrap();
            // An earlier development build: the service under the edge's name, both connectors, and
            // a record that names no edge.
            fs::write(fixture.layout.edge(), "pre-edge service").unwrap();
            for name in retired_connector_names() {
                fs::write(bin.join(name), "retired connector").unwrap();
            }
            InstallRecord {
                service_version: VERSION.to_owned(),
                relay_revision: RELAY.to_owned(),
                service_sha256: "0".repeat(64),
                edge_sha256: None,
                channel: Channel::Release,
            }
            .write(&fixture.layout.install_record())
            .unwrap();
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            let calls = control.calls();
            let Call::Stop(images) = &calls[0] else {
                panic!("{calls:?}");
            };
            let mut expected = vec![bin.join(format!("{EDGE_EXECUTABLE_NAME}.old-1"))];
            expected.extend(retired_connector_names().map(|name| bin.join(name)));
            assert_eq!(images, &expected);
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
            for name in retired_connector_names() {
                assert!(!bin.join(&name).exists(), "{name}");
            }
            assert_eq!(
                fixture.record().unwrap().edge_sha256,
                Some(record::sha256_file(&fixture.layout.edge()).unwrap())
            );
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let recorded = fixture.record().unwrap();
            assert_eq!(recorded.channel, Channel::Release);
            Fixture::write_payload(&fixture.payload, "development service");
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(placed_names(&outcome.plan), [SERVICE_EXECUTABLE_NAME]);
            assert_eq!(outcome.plan.edge_replaced, None);
            let printed = serde_json::to_value(&outcome.plan).unwrap();
            assert!(printed.get("edge_replaced").is_none(), "{printed}");
            assert_eq!(
                fixture.read(fixture.layout.service_executable()),
                "development service"
            );
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            let record = fixture.record().unwrap();
            assert_eq!(record.channel, Channel::Development);
            assert_eq!(record.edge_sha256, recorded.edge_sha256);
            assert_eq!(
                record.service_sha256,
                record::sha256_file(&fixture.layout.service_executable()).unwrap()
            );
            assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
            assert!(!fixture.layout.retired_development_directory().exists());
            assert!(!fixture.layout.retired_development_marker().exists());
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            let first = fixture.record().unwrap().edge_sha256;
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "rebuilt edge").unwrap();
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(
                placed_names(&outcome.plan),
                [SERVICE_EXECUTABLE_NAME, EDGE_EXECUTABLE_NAME]
            );
            assert_eq!(outcome.plan.edge_replaced, Some(EdgeReplacement::Rebuilt));
            let printed = serde_json::to_value(&outcome.plan).unwrap();
            assert_eq!(
                printed["edge_replaced"],
                words::edge_replaced(EdgeReplacement::Rebuilt)
            );
            // The same swap and restart a relay revision change gets, and nothing more.
            assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
            assert_eq!(fixture.read(fixture.layout.edge()), "rebuilt edge");
            let record = fixture.record().unwrap();
            assert_eq!(record.channel, Channel::Development);
            assert_eq!(
                record.edge_sha256,
                Some(record::sha256_file(&fixture.layout.edge()).unwrap())
            );
            assert_ne!(record.edge_sha256, first);

            // A deploy right after it, whose build did not change the edge, replaces no edge.
            Fixture::write_payload(&fixture.payload, "next service");
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "rebuilt edge").unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(placed_names(&outcome.plan), [SERVICE_EXECUTABLE_NAME]);
            assert_eq!(outcome.plan.edge_replaced, None);
            assert_eq!(fixture.record().unwrap().edge_sha256, record.edge_sha256);
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "release edge").unwrap();
            let outcome = fixture
                .run(Mode::Restore, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(
                outcome.plan.edge_replaced,
                Some(EdgeReplacement::LeavingDevelopment)
            );
            assert_eq!(fixture.read(fixture.layout.edge()), "release edge");
            assert_eq!(fixture.record().unwrap().channel, Channel::Release);

            // Once back on the release, a restore of the same release keeps its intact edge.
            let again = fixture
                .run(Mode::Restore, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(placed_names(&again.plan), [SERVICE_EXECUTABLE_NAME]);
            assert_eq!(again.plan.edge_replaced, None);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;

            /// The sharing Windows gives a running image: it may be read and renamed, not written.
            const RUNNING_IMAGE_SHARING: u32 = 0x1 | 0x4;

            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            let running = fs::OpenOptions::new()
                .read(true)
                .share_mode(RUNNING_IMAGE_SHARING)
                .open(fixture.layout.edge())
                .unwrap();
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "rebuilt edge").unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(outcome.plan.edge_replaced, Some(EdgeReplacement::Rebuilt));
            assert_eq!(fixture.read(fixture.layout.edge()), "rebuilt edge");
            let moved_aside = fixture
                .layout
                .bin()
                .join(format!("{EDGE_EXECUTABLE_NAME}.old-1"));
            assert_eq!(fixture.read(moved_aside), edge_text(RELAY));
            drop(running);
        }
        {
            let fixture = Fixture::new();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(
                placed_names(&outcome.plan),
                [SERVICE_EXECUTABLE_NAME, EDGE_EXECUTABLE_NAME]
            );
            assert_eq!(
                outcome.plan.edge_replaced,
                Some(EdgeReplacement::Unrecorded)
            );
            assert!(outcome
                .plan
                .swaps
                .iter()
                .all(|placed| placed.directory == fixture.layout.bin()));
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            let record = fixture.record().unwrap();
            assert_eq!(record.channel, Channel::Development);
            assert_eq!(
                record.edge_sha256,
                Some(record::sha256_file(&fixture.layout.edge()).unwrap())
            );
            assert!(!fixture.layout.retired_development_directory().exists());
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let recorded = fixture.record().unwrap().edge_sha256;
            fs::write(fixture.layout.edge(), "changed edge").unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(
                placed_names(&outcome.plan),
                [SERVICE_EXECUTABLE_NAME, EDGE_EXECUTABLE_NAME]
            );
            assert_eq!(outcome.plan.edge_replaced, Some(EdgeReplacement::Altered));
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
            assert_eq!(fixture.record().unwrap().edge_sha256, recorded);

            fs::remove_file(fixture.layout.edge()).unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert!(placed_names(&outcome.plan).contains(&EDGE_EXECUTABLE_NAME));
            assert_eq!(outcome.plan.edge_replaced, Some(EdgeReplacement::Missing));
            assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            Fixture::write_payload(&fixture.payload, "release service");
            fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "release edge").unwrap();
            let build = Build {
                relay_revision: "5".to_owned(),
                ..fixture.build("2.0.0")
            };
            install(
                &fixture.context(build, &FakeControl::new()),
                &Request {
                    payload: None,
                    mode: Mode::Restore,
                    plan_only: false,
                },
            )
            .unwrap();
            assert_eq!(fixture.read(fixture.layout.edge()), "release edge");
            let record = fixture.record().unwrap();
            assert_eq!(record.relay_revision, "5");
            assert_eq!(record.channel, Channel::Release);
        }
    }

    /// Protects the ADR-0199 amendment that brings the development channel into 2.0: a development
    /// install takes any version and swaps and restarts every time, a release package refuses to
    /// install over it and changes nothing, an in-place run reports its own payload, and only
    /// `--restore` leaves it, installing a release even from an older payload or into an empty home.
    #[test]
    fn the_development_channel_installs_every_build_and_only_restore_leaves_it() {
        {
            let fixture = Fixture::new();
            for round in 0..3 {
                Fixture::write_payload(&fixture.payload, &format!("build {round}"));
                let control = FakeControl::new();
                let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
                assert!(!outcome.plan.unchanged, "round {round}");
                assert_eq!(outcome.service_started, Some(true), "round {round}");
                assert_eq!(
                    fixture.read(fixture.layout.service_executable()),
                    format!("build {round}")
                );
                assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
            }
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, "2.5.0", &FakeControl::new())
                .unwrap();
            for version in ["2.0.0", "3.0.0"] {
                Fixture::write_payload(&fixture.payload, version);
                fixture
                    .run(Mode::Development, version, &FakeControl::new())
                    .unwrap();
                assert_eq!(fixture.record().unwrap().service_version, version);
                assert_eq!(fixture.read(fixture.layout.service_executable()), version);
            }
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            let before = fixture.snapshot();
            for version in ["2.0.0", VERSION, "9.9.9"] {
                let control = FakeControl::new();
                let error = fixture.run(Mode::Release, version, &control).unwrap_err();
                assert!(error.is::<DevelopmentInstalled>(), "{error}");
                let message = error.to_string();
                assert!(
                    message.contains("npx ghostlight deployment install --restore"),
                    "{message}"
                );
                assert!(message.contains("get.ps1"), "{message}");
                assert!(message.contains("get.sh"), "{message}");
                assert!(
                    !message.contains(&fixture.payload.display().to_string()),
                    "a staged payload is gone once its carrier exits: {message}"
                );
                assert!(error.to_string().contains("development build"), "{error}");
                assert!(control.calls().is_empty());
                assert_eq!(fixture.snapshot(), before, "{version}");
            }
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            let before = fixture.snapshot();
            // The installed edge hands `ghostlight deployment install` to bin/ghostlight-service,
            // so the payload is bin itself.
            let control = FakeControl::new();
            let in_place = Context {
                payload: fixture.layout.bin(),
                ..fixture.context(fixture.build(VERSION), &control)
            };
            let error = plan(&in_place, Mode::Release).unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<PayloadError>(),
                    Some(PayloadError::InPlace(_))
                ),
                "{error}"
            );
            assert!(!error.to_string().contains("--restore"), "{error}");
            assert_eq!(fixture.snapshot(), before);
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Development, "2.5.0", &FakeControl::new())
                .unwrap();
            Fixture::write_payload(&fixture.payload, "release service");
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Restore, "2.0.0", &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(!outcome.plan.unchanged);
            assert_eq!(placed_names(&outcome.plan), [SERVICE_EXECUTABLE_NAME]);
            assert_eq!(
                fixture.read(fixture.layout.service_executable()),
                "release service"
            );
            let record = fixture.record().unwrap();
            assert_eq!(record.service_version, "2.0.0");
            assert_eq!(record.channel, Channel::Release);
            assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);

            // The installation is a release again: the same package is nothing to do.
            let outcome = fixture
                .run(Mode::Release, "2.0.0", &FakeControl::new())
                .unwrap();
            assert!(outcome.plan.unchanged);
        }
        {
            let fixture = Fixture::new();
            let outcome = fixture
                .run(Mode::Restore, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
            assert_eq!(fixture.record().unwrap().channel, Channel::Release);
        }
    }

    /// Protects defects 26d6edba, 64164332, 83aa61f0 and 80810b08: a 1.x installation is replaced
    /// without losing anything the person had. Registrations name `bin` before any old program stops,
    /// a client the person located is reconciled, the state moves into the home, and every legacy
    /// program, respawned or served from `bin`, is stopped by its image before its record goes. The
    /// 1.x policy stays where the 1.x service reads it until the last stop, a save made while it stops
    /// or after a failed run wins, a diverged policy is reported without blocking the next install, a
    /// failed migration swaps nothing, and the next install finishes a failed run by the same rule.
    #[test]
    fn a_1x_installation_is_replaced_with_registrations_first_and_its_state_and_policy_kept() {
        {
            let mut fixture = Fixture::new();
            let policy = fixture.scratch.path().join("old-policy");
            fs::create_dir_all(&policy).unwrap();
            fs::write(policy.join(USER_POLICY_FILE_NAME), "user policy").unwrap();
            fixture.legacy_policy = Some(policy.clone());
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(!outcome.plan.migration.is_empty());
            assert_eq!(
                fixture.read(fixture.layout.policy().join(USER_POLICY_FILE_NAME)),
                "user policy"
            );
            assert!(!policy.join(USER_POLICY_FILE_NAME).exists());
            assert!(fixture.layout.service_executable().is_file());
        }
        {
            let mut fixture = Fixture::new();
            fixture.plant_legacy_installation(&fixture.scratch.path().join("old-1x"));
            let control = fixture.watching_control();
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            let calls = control.calls();
            let sightings = control.sightings();
            assert_eq!(calls.len(), sightings.len());
            let stops: Vec<usize> = (0..calls.len())
                .filter(|&index| matches!(calls[index], Call::Stop(_) | Call::Quit))
                .collect();
            assert!(!stops.is_empty(), "{calls:?}");
            for index in stops {
                assert!(sightings[index].registered, "{calls:?} at {index}");
            }
            assert!(fixture.manifest_names_bin_edge());
        }
        {
            let mut fixture = Fixture::new();
            fixture.plant_legacy_installation(&fixture.scratch.path().join("old-1x"));
            let located = fixture
                .scratch
                .path()
                .join("portable")
                .join("claude_desktop_config.json");
            let old_connector = fixture
                .scratch
                .path()
                .join("old-1x")
                .join(&retired_connector_names()[0]);
            fs::create_dir_all(located.parent().unwrap()).unwrap();
            fs::write(
                &located,
                serde_json::json!({"mcpServers": {"ghostlight": {"command": old_connector}}})
                    .to_string(),
            )
            .unwrap();
            // The 1.x file the user's located paths were kept in, where the 1.x layout had it.
            let locations = LegacyRoots::within(&fixture.scratch.path().join("profile"))
                .harness_locations_directory()
                .join(HARNESS_LOCATIONS_NAME);
            fs::create_dir_all(locations.parent().unwrap()).unwrap();
            fs::write(
                &locations,
                serde_json::json!({"claude-desktop": {"config": located}}).to_string(),
            )
            .unwrap();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let written: serde_json::Value =
                serde_json::from_str(&fixture.read(located.clone())).unwrap();
            let command = PathBuf::from(
                written["mcpServers"]["ghostlight"]["command"]
                    .as_str()
                    .unwrap(),
            );
            assert_eq!(
                fs::canonicalize(command).unwrap(),
                fs::canonicalize(fixture.layout.edge()).unwrap()
            );
            assert_eq!(
                written["mcpServers"]["ghostlight"]["args"],
                serde_json::json!(["mcp"])
            );
            assert!(!locations.exists());
            let kept: serde_json::Value = serde_json::from_str(
                &fixture.read(fixture.layout.state().join(HARNESS_LOCATIONS_NAME)),
            )
            .unwrap();
            assert_eq!(
                PathBuf::from(kept["claude-desktop"]["config"].as_str().unwrap()),
                located
            );
        }
        {
            let mut fixture = Fixture::new();
            let original = plant_legacy_policy(&mut fixture);
            let copy = fixture.layout.policy().join(USER_POLICY_FILE_NAME);
            let mut control = fixture.watching_control();
            control.policy_witness = Some((original.clone(), copy.clone()));
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            let calls = control.calls();
            let sightings = control.sightings();
            let start = calls.iter().position(|call| *call == Call::Start).unwrap();
            assert!(start > 0, "{calls:?}");
            // Through the swaps, the reconcile and every stop, the 1.x service has its policy and
            // a 2.0 service would find the copy.
            for (call, sighting) in calls.iter().zip(&sightings).take(start) {
                assert!(sighting.policy_original_present, "{call:?}");
                assert!(sighting.policy_copy_present, "{call:?}");
            }
            assert!(!sightings[start].policy_original_present);
            assert_eq!(fixture.read(copy), "user policy");
            assert!(!original.exists());
        }
        {
            let mut fixture = Fixture::new();
            let original = plant_legacy_policy(&mut fixture);
            let mut control = FakeControl::new();
            control.writes_on_stop = vec![(original.clone(), "saved while stopping".to_owned())];
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert!(!original.exists());
            assert_eq!(
                fixture.read(fixture.layout.policy().join(USER_POLICY_FILE_NAME)),
                "saved while stopping"
            );
        }
        {
            let mut fixture = Fixture::new();
            let original = plant_legacy_policy(&mut fixture);
            let copy = fixture.layout.policy().join(USER_POLICY_FILE_NAME);
            let mut control = FakeControl::new();
            control.writes_on_stop = vec![
                (original.clone(), "saved by the 1.x service".to_owned()),
                (copy.clone(), "saved by a 2.0 service".to_owned()),
            ];
            let error = fixture.run(Mode::Release, VERSION, &control).unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<MigrationError>(),
                    Some(MigrationError::Conflict { .. })
                ),
                "{error}"
            );
            assert_eq!(fixture.record(), None);
            let aside = original.with_file_name(format!("{USER_POLICY_FILE_NAME}.pre-2.0"));
            assert_eq!(fixture.read(aside), "saved by the 1.x service");
            assert!(!original.exists());

            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(fixture.read(copy), "saved by a 2.0 service");
            assert!(!fixture.legacy_record().exists());
        }
        {
            let mut fixture = Fixture::new();
            let (original, copy) = failed_run_after_the_copy(&mut fixture);
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(!original.exists());
            assert_eq!(fixture.read(copy), "user policy");
        }
        {
            let mut fixture = Fixture::new();
            let (original, copy) = failed_run_after_the_copy(&mut fixture);
            fs::write(&original, "saved by the 1.x service").unwrap();
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(!original.exists());
            assert_eq!(fixture.read(copy), "saved by the 1.x service");
        }
        {
            let mut fixture = Fixture::new();
            let (original, copy) = failed_run_after_the_copy(&mut fixture);
            fs::write(&copy, "written by a 2.0 service").unwrap();
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(!original.exists());
            assert_eq!(fixture.read(copy), "written by a 2.0 service");
        }
        {
            let mut fixture = Fixture::new();
            let (original, copy) = failed_run_after_the_copy(&mut fixture);
            fs::write(&original, "saved by the 1.x service").unwrap();
            fs::write(&copy, "written by a 2.0 service").unwrap();
            let error = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<MigrationError>(),
                    Some(MigrationError::Conflict { .. })
                ),
                "{error}"
            );
            let aside = original.with_file_name(format!("{USER_POLICY_FILE_NAME}.pre-2.0"));
            assert_eq!(fixture.read(aside), "saved by the 1.x service");
            assert_eq!(fixture.read(copy.clone()), "written by a 2.0 service");

            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(fixture.read(copy), "written by a 2.0 service");
            assert!(!fixture.legacy_record().exists());
        }
        {
            let mut fixture = Fixture::new();
            let original = plant_legacy_policy(&mut fixture);
            let copy = fixture.layout.policy().join(USER_POLICY_FILE_NAME);
            fs::create_dir_all(fixture.layout.policy()).unwrap();
            fs::write(&copy, "somebody else's file").unwrap();
            let error = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<MigrationError>(),
                    Some(MigrationError::Conflict { .. })
                ),
                "{error}"
            );
            assert_eq!(fixture.read(original), "user policy");
            assert_eq!(fixture.read(copy), "somebody else's file");
        }
        {
            let mut fixture = Fixture::new();
            fixture.plant_legacy_installation(&fixture.scratch.path().join("old-1x"));
            let home = fixture.layout.home().to_owned();
            fs::write(home.join(AUDIT_FILE_NAME), "old audit").unwrap();
            fs::write(home.join(LEGACY_STARTUP_NAME), "").unwrap();
            fs::write(home.join(LEGACY_DEPLOY_LOCK_NAME), "").unwrap();
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(outcome.plan.restart);
            let archive = fixture.layout.state().join("audit-pre-2.0.jsonl");
            assert_eq!(fixture.read(archive), "old audit");
            for gone in [
                AUDIT_FILE_NAME,
                LEGACY_STARTUP_NAME,
                LEGACY_DEPLOY_LOCK_NAME,
            ] {
                assert!(!home.join(gone).exists(), "{gone}");
            }

            // The same version is already installed, and a 1.x leftover still forces the full run.
            fs::write(home.join(LEGACY_STARTUP_NAME), "").unwrap();
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert!(!outcome.plan.unchanged);
            assert!(outcome.plan.restart);
            assert!(!home.join(LEGACY_STARTUP_NAME).exists());
            assert!(control.calls().contains(&Call::Start));
        }
        {
            let mut fixture = Fixture::new();
            let serving = fixture.scratch.path().join("old-1x");
            fixture.plant_legacy_installation(&serving);
            let control = FakeControl::new();
            control.respawns.set(1);
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            let images: Vec<PathBuf> = legacy_image_names()
                .iter()
                .map(|name| serving.join(name))
                .collect();
            let mut expected = vec![Call::Stop(images.clone())];
            expected.extend([
                Call::Running,
                Call::Stop(images),
                Call::Running,
                Call::Running,
            ]);
            expected.extend([Call::Quit, Call::Start, Call::Wait]);
            assert_eq!(control.calls(), expected);
        }
        {
            let mut fixture = Fixture::new();
            fixture.plant_legacy_installation(&fixture.scratch.path().join("old-1x"));
            let control = fixture.watching_control();
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(!fixture.legacy_record().exists());
            let calls = control.calls();
            let sightings = control.sightings();
            let start = calls.iter().position(|call| *call == Call::Start).unwrap();
            for (call, sighting) in calls.iter().zip(&sightings).take(start) {
                assert!(sighting.record_present, "{call:?}");
            }
            assert!(!sightings[start].record_present);
        }
        {
            let mut fixture = Fixture::new();
            fixture.plant_legacy_installation(&fixture.layout.bin());
            let control = FakeControl::new();
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            let calls = control.calls();
            let Call::Stop(images) = &calls[0] else {
                panic!("{calls:?}");
            };
            // The 1.x service sat under the name the edge now has, so the install moved it aside.
            let moved = fixture
                .layout
                .bin()
                .join(format!("{EDGE_EXECUTABLE_NAME}.old-1"));
            assert!(images.contains(&moved), "{images:?}");
            // The connectors were never replaced, so they are stopped where they are, and removed.
            for name in retired_connector_names() {
                let connector = fixture.layout.bin().join(name);
                assert!(images.contains(&connector), "{images:?}");
                assert!(!connector.exists());
            }
            for installed in [fixture.layout.service_executable(), fixture.layout.edge()] {
                assert!(!images.contains(&installed), "{images:?}");
            }
        }
        {
            let mut fixture = Fixture::new();
            let policy = fixture.scratch.path().join("old-policy");
            fs::create_dir_all(&policy).unwrap();
            fs::write(policy.join(USER_POLICY_FILE_NAME), "old policy").unwrap();
            fs::create_dir_all(fixture.layout.policy()).unwrap();
            fs::write(
                fixture.layout.policy().join(USER_POLICY_FILE_NAME),
                "new policy",
            )
            .unwrap();
            fixture.legacy_policy = Some(policy);
            let control = FakeControl::new();
            assert!(fixture.run(Mode::Release, VERSION, &control).is_err());
            assert!(!fixture.layout.bin().exists());
            assert!(control.calls().is_empty());
        }
    }

    /// Protects ACCEPTANCE 14 and defects 31f2c835 and bd29366c: a replace runs in one order. The
    /// service is asked to quit, and stopped by its exact image if it does not; what an older
    /// installation left running is stopped next, once and by pid, and one that will not stop is
    /// reported without blocking; the new service starts after that, with the new shell already in
    /// `bin`. The install is recorded only when an instance other than the replaced one answers, and a
    /// service that never answers, stops while starting or cannot be spawned is named with its own
    /// reason, never one an earlier start left.
    #[test]
    fn a_replace_quits_the_service_stops_leftovers_and_records_only_a_new_instance_that_answers() {
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            control.strays = vec![StrayProcess {
                pid: 4242,
                image: fixture
                    .layout
                    .bin()
                    .join(retired_connector_names()[0].as_str()),
                kind: StrayKind::RetiredConnector,
            }];
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(
                control.calls(),
                [
                    Call::Quit,
                    Call::StopStrays(vec![4242]),
                    Call::Start,
                    Call::Wait
                ]
            );
        }
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            let stray = StrayProcess {
                pid: 4242,
                image: fixture.layout.bin().join(&retired_connector_names()[1]),
                kind: StrayKind::RetiredConnector,
            };
            control.strays = vec![stray.clone()];
            control.strays_stay = true;
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(outcome.strays_left, [stray]);
            assert_eq!(
                control.calls(),
                [
                    Call::Quit,
                    Call::StopStrays(vec![4242]),
                    Call::Start,
                    Call::Wait
                ]
            );
        }
        {
            let layout = Layout::new("home");
            let stray = |pid: u32, name: &str| StrayProcess {
                pid,
                image: layout.bin().join(name),
                kind: StrayKind::RetiredConnector,
            };
            let mut control = FakeControl::new();
            assert_eq!(
                stop_found_strays(&control, &layout, Some(9)),
                StrayStop::default()
            );
            assert!(control.calls().is_empty());

            control.strays = vec![stray(1, "one"), stray(2, "two")];
            let stop = stop_found_strays(&control, &layout, Some(9));
            assert_eq!(stop.stopped, control.strays);
            assert!(stop.stuck.is_empty());

            control.strays_stay = true;
            let stop = stop_found_strays(&control, &layout, None);
            assert!(stop.stopped.is_empty());
            assert_eq!(stop.stuck, control.strays);
        }
        {
            let fixture = Fixture::new();
            let control = FakeControl::new();
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert!(!control
                .calls()
                .iter()
                .any(|call| matches!(call, Call::StopStrays(_))));
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            Fixture::write_payload(&fixture.payload, "next service");
            let mut control = FakeControl::new();
            control.quits = false;
            fixture.run(Mode::Release, "2.2.0", &control).unwrap();
            let calls = control.calls();
            let Call::Stop(images) = &calls[1] else {
                panic!("{calls:?}");
            };
            assert!(images.contains(
                &fixture
                    .layout
                    .bin()
                    .join(format!("{SERVICE_EXECUTABLE_NAME}.old-1"))
            ));
            assert_eq!(calls[2..], [Call::Start, Call::Wait]);
        }
        {
            let fixture = Fixture::new();
            let images = plant_retired_development(&fixture);
            let mut control = FakeControl::new();
            control.strays = vec![StrayProcess {
                pid: 41,
                image: images[1].clone(),
                kind: StrayKind::RetiredDevelopment,
            }];
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert!(outcome.strays_left.is_empty());
            assert!(outcome.left_behind.is_empty());
            assert_eq!(
                control.calls(),
                [
                    Call::Quit,
                    Call::StopStrays(vec![41]),
                    Call::Start,
                    Call::Wait
                ]
            );
        }
        {
            let fixture = Fixture::new();
            let images = plant_retired_development(&fixture);
            let stuck = StrayProcess {
                pid: 41,
                image: images[1].clone(),
                kind: StrayKind::RetiredDevelopment,
            };
            let mut control = FakeControl::new();
            control.strays = vec![stuck.clone()];
            control.strays_stay = true;
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(outcome.strays_left, [stuck]);
            assert!(!fixture.layout.retired_development_directory().exists());
        }
        {
            let fixture = Fixture::new();
            let images = plant_retired_development(&fixture);
            let mut control = FakeControl::new();
            control.quits = false;
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            let calls = control.calls();
            let Call::Stop(first) = &calls[1] else {
                panic!("{calls:?}");
            };
            assert!(first.contains(&images[0]), "{first:?}");
            assert!(!fixture.layout.retired_development_directory().exists());
        }
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            control.answers = false;
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(false));
            assert_eq!(outcome.start_failure, Some(StartFailure::Silent));
            assert!(fixture.layout.service_executable().is_file());
            assert_eq!(fixture.record(), None);
            let failure = ServiceDidNotStart::of(&outcome, &fixture.layout).unwrap();
            assert!(failure.to_string().contains("did not start within 30 s"));
        }
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            control.answers = false;
            control.stops_recording = Some(Some("bind the relay endpoint: no room".to_owned()));
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(
                outcome.start_failure,
                Some(StartFailure::Stopped(Some(
                    "bind the relay endpoint: no room".to_owned()
                )))
            );
            let sentence = ServiceDidNotStart::of(&outcome, &fixture.layout)
                .unwrap()
                .to_string();
            assert!(sentence.contains("stopped while starting: bind the relay endpoint: no room"));
            assert!(!sentence.contains("30 s"), "a stop is not a timeout");
            assert_eq!(fixture.record(), None);
        }
        {
            let fixture = Fixture::new();
            fs::create_dir_all(fixture.layout.run()).unwrap();
            fs::write(fixture.layout.start_failure(), "an earlier reason\n").unwrap();
            let mut control = FakeControl::new();
            control.answers = false;
            control.stops_recording = Some(None);
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(outcome.start_failure, Some(StartFailure::Stopped(None)));
            let sentence = ServiceDidNotStart::of(&outcome, &fixture.layout)
                .unwrap()
                .to_string();
            assert!(sentence.contains("stopped while starting."), "{sentence}");
        }
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            control.spawn_error = Some("Permission denied (os error 13)".to_owned());
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(
                outcome.start_failure,
                Some(StartFailure::Spawn(
                    "Permission denied (os error 13)".to_owned()
                ))
            );
            assert!(
                !control.calls().contains(&Call::Wait),
                "nothing is waited for when nothing was started"
            );
            let sentence = ServiceDidNotStart::of(&outcome, &fixture.layout)
                .unwrap()
                .to_string();
            assert!(sentence.contains("could not be started: Permission denied (os error 13)"));
        }
        {
            let fixture = Fixture::new();
            let mut control = FakeControl::new();
            control.instance = Some("instance_old".to_owned());
            fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(
                control.waited_for.borrow().as_slice(),
                [Some("instance_old".to_owned())]
            );

            let fresh = Fixture::new();
            let control = FakeControl::new();
            fresh.run(Mode::Release, VERSION, &control).unwrap();
            assert_eq!(control.waited_for.borrow().as_slice(), [None]);
        }
        {
            // A shell that is told to hand over compares its own image with the one in `bin`, so the
            // swap must come before the quit.
            let fixture = Fixture::new();
            add_shell(&fixture, "shell");
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();

            add_shell(&fixture, "next shell");
            let mut control = FakeControl::new();
            control.quit_sees = Some(fixture.layout.bin().join(DESKTOP_EXECUTABLE_NAME));
            fixture.run(Mode::Development, VERSION, &control).unwrap();

            assert_eq!(control.calls().first(), Some(&Call::Quit));
            assert_eq!(
                control.seen_at_quit.borrow().as_deref(),
                Some("next shell"),
                "the quit finds the new shell already in bin"
            );
        }
    }

    /// Protects defect 5b818ffc: the retired development layout and the 1.x version directories are
    /// listed by `--plan`, which changes nothing, and removed after the `bin` swap by every channel,
    /// the same release included, after any program running from them stops. Each directory's log is
    /// archived once under a name that says where it came from, a log that cannot be archived keeps its
    /// directory and is reported, an entry that cannot be removed is reported without failing the
    /// install, and a second install has nothing left to do.
    #[test]
    fn older_layouts_are_listed_by_the_plan_and_retired_after_the_swap() {
        {
            let fixture = Fixture::new();
            let images = plant_retired_development(&fixture);
            let directory = fixture.layout.retired_development_directory();
            let marker = fixture.layout.retired_development_marker();
            let before = fixture.snapshot();

            let planned = plan(
                &fixture.context(fixture.build(VERSION), &FakeControl::new()),
                Mode::Development,
            )
            .unwrap();
            assert_eq!(planned.deletions, [directory.clone(), marker.clone()]);
            for image in &images {
                assert!(planned.processes_to_stop.contains(image), "{image:?}");
            }
            assert_eq!(fixture.snapshot(), before, "a plan changes nothing");

            let control = fixture.watching_control();
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(outcome.left_behind.is_empty());
            assert!(!directory.exists());
            assert!(!marker.exists());
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");

            // The scan stops what runs from the directory, so no second stop by image follows.
            assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
            for sighting in control.sightings() {
                assert!(
                    sighting.registered,
                    "registrations name bin before any stop"
                );
            }
            assert!(fixture.manifest_names_bin_edge());
        }
        {
            let fixture = Fixture::new();
            fs::create_dir_all(fixture.layout.home()).unwrap();
            fs::write(fixture.layout.retired_development_marker(), "{}").unwrap();
            let control = FakeControl::new();
            fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert!(!fixture.layout.retired_development_marker().exists());
            assert!(!control
                .calls()
                .iter()
                .any(|call| matches!(call, Call::Stop(_))));

            fs::create_dir_all(fixture.layout.retired_development_directory()).unwrap();
            fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert!(!fixture.layout.retired_development_directory().exists());
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            plant_retired_development(&fixture);
            let control = FakeControl::new();
            let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
            assert!(!outcome.plan.unchanged);
            assert!(outcome.plan.swaps.is_empty());
            assert!(!fixture.layout.retired_development_directory().exists());
            assert!(!fixture.layout.retired_development_marker().exists());
            assert_eq!(control.calls()[0], Call::Quit);
        }
        {
            let fixture = Fixture::new();
            let (first, second) = plant_version_directories(&fixture);
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert!(outcome.left_behind.is_empty());
            assert_eq!(outcome.plan.deletions, [first.clone(), second.clone()]);
            assert!(!first.exists());
            assert!(!second.exists());
            assert_eq!(
                fs::read(fixture.layout.state().join("audit-v1.3.5.jsonl")).unwrap(),
                VERSION_AUDIT
            );
            assert_eq!(
                outcome.archived_version_audits,
                [Move {
                    from: first.join(AUDIT_FILE_NAME),
                    to: fixture.layout.state().join("audit-v1.3.5.jsonl"),
                }]
            );
            for kept in untouched_entries(&fixture) {
                assert!(kept.is_file(), "{}", kept.display());
            }
            assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
        }
        {
            for mode in [Mode::Release, Mode::Restore] {
                let fixture = Fixture::new();
                let (first, second) = plant_version_directories(&fixture);
                let outcome = fixture.run(mode, VERSION, &FakeControl::new()).unwrap();
                assert!(outcome.left_behind.is_empty(), "{mode:?}");
                assert!(!first.exists() && !second.exists(), "{mode:?}");
                assert!(fixture.layout.state().join("audit-v1.3.5.jsonl").is_file());

                let again = fixture.run(mode, VERSION, &FakeControl::new()).unwrap();
                assert!(again.plan.deletions.is_empty(), "{mode:?}");
                assert!(again.plan.version_audit_archives.is_empty(), "{mode:?}");
            }
        }
        {
            let fixture = Fixture::new();
            fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            let (first, second) = plant_version_directories(&fixture);
            let outcome = fixture
                .run(Mode::Release, VERSION, &FakeControl::new())
                .unwrap();
            assert!(!outcome.plan.unchanged);
            assert!(outcome.plan.swaps.is_empty());
            assert!(!first.exists() && !second.exists());
        }
        {
            let fixture = Fixture::new();
            let (first, second) = plant_version_directories(&fixture);
            let before = fixture.snapshot();
            let control = FakeControl::new();
            let outcome = install(
                &fixture.context(fixture.build(VERSION), &control),
                &Request {
                    payload: None,
                    mode: Mode::Development,
                    plan_only: true,
                },
            )
            .unwrap();
            assert_eq!(outcome.plan.deletions, [first.clone(), second]);
            assert_eq!(
                outcome.plan.version_audit_archives,
                [Move {
                    from: first.join(AUDIT_FILE_NAME),
                    to: fixture.layout.state().join("audit-v1.3.5.jsonl"),
                }]
            );
            assert_eq!(fixture.snapshot(), before);
            assert!(control.calls().is_empty());
        }
        {
            let fixture = Fixture::new();
            let (first, second) = plant_version_directories(&fixture);
            let mut control = FakeControl::new();
            control.watched_directory = Some(second.clone());
            control.strays = vec![StrayProcess {
                pid: 52,
                image: second.join(EDGE_EXECUTABLE_NAME),
                kind: StrayKind::RetiredVersionDirectory,
            }];
            let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
            assert!(outcome.strays_left.is_empty());
            assert_eq!(
                control.calls(),
                [
                    Call::Quit,
                    Call::StopStrays(vec![52]),
                    Call::Start,
                    Call::Wait
                ]
            );
            assert_eq!(*control.watched_present.borrow(), [true]);
            assert!(!first.exists() && !second.exists());
        }
        {
            let mut fixture = Fixture::new();
            let (first, second) = plant_version_directories(&fixture);
            fixture.legacy_state = Some(first.clone());
            fs::create_dir_all(fixture.layout.home()).unwrap();
            fs::write(fixture.legacy_record(), "{}").unwrap();
            let log = first.join(AUDIT_FILE_NAME);
            let migration_archive = Move {
                from: log.clone(),
                to: fixture.layout.state().join("audit-pre-2.0.jsonl"),
            };

            let planned = install(
                &fixture.context(fixture.build(VERSION), &FakeControl::new()),
                &Request {
                    payload: None,
                    mode: Mode::Development,
                    plan_only: true,
                },
            )
            .unwrap();
            assert_eq!(
                planned.plan.migration.audit_archive,
                Some(migration_archive)
            );
            assert!(planned.plan.version_audit_archives.is_empty());

            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert!(outcome.left_behind.is_empty());
            assert!(outcome.archived_version_audits.is_empty());
            assert_eq!(
                fs::read(fixture.layout.state().join("audit-pre-2.0.jsonl")).unwrap(),
                VERSION_AUDIT
            );
            assert!(!fixture.layout.state().join("audit-v1.3.5.jsonl").exists());
            assert!(!first.exists() && !second.exists());
        }
        {
            let fixture = Fixture::new();
            let (first, second) = plant_version_directories(&fixture);
            fs::create_dir_all(fixture.layout.state()).unwrap();
            let archive = fixture.layout.state().join("audit-v1.3.5.jsonl");
            fs::write(&archive, "an earlier archive").unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(outcome.left_behind, std::slice::from_ref(&first));
            assert!(outcome.archived_version_audits.is_empty());
            assert_eq!(
                fs::read(first.join(AUDIT_FILE_NAME)).unwrap(),
                VERSION_AUDIT
            );
            assert_eq!(fixture.read(archive), "an earlier archive");
            assert!(!second.exists());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;

            let fixture = Fixture::new();
            let images = plant_retired_development(&fixture);
            // A program running from an image holds it open without sharing delete.
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&images[0])
                .unwrap();
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert_eq!(outcome.service_started, Some(true));
            assert_eq!(
                outcome.left_behind,
                [fixture.layout.retired_development_directory()]
            );
            assert!(!fixture.layout.retired_development_marker().exists());
            drop(held);

            // The next install finishes the removal.
            let outcome = fixture
                .run(Mode::Development, VERSION, &FakeControl::new())
                .unwrap();
            assert!(outcome.left_behind.is_empty());
            assert!(!fixture.layout.retired_development_directory().exists());
        }
        {
            let mut fixture = Fixture::new();
            let policy = fixture.scratch.path().join("old-policy");
            fs::create_dir_all(&policy).unwrap();
            fs::write(policy.join(USER_POLICY_FILE_NAME), "user policy").unwrap();
            fixture.legacy_policy = Some(policy);
            let before = fixture.snapshot();
            let control = FakeControl::new();
            let outcome = install(
                &fixture.context(fixture.build(VERSION), &control),
                &Request {
                    payload: None,
                    mode: Mode::Development,
                    plan_only: true,
                },
            )
            .unwrap();
            assert!(!outcome.plan.migration.is_empty());
            assert_eq!(outcome.plan.swaps.len(), 2);
            assert!(serde_json::to_value(&outcome.plan).is_ok());
            assert!(control.calls().is_empty());
            assert_eq!(fixture.snapshot(), before);
        }
    }

    /// Protects the quit-for-replacement wire contract and defect bd29366c: the installer asks the
    /// running service to quit for replacement and waits for its lock, a home with no service has
    /// nothing to wait for, an earlier development build that cannot be asked is not, the new service
    /// is ready only when an instance other than the old one answers, and a wait for a service no
    /// process runs ends after the grace, not the ready wait.
    #[test]
    fn the_installer_quits_the_service_for_replacement_and_waits_for_a_new_instance() {
        {
            let fixture = Fixture::new();
            let layout = &fixture.layout;
            let lock = ServiceLock::try_acquire(&layout.service_lock())
                .unwrap()
                .expect("the service lock");
            let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let record = std::sync::Arc::clone(&asked);
            let service = fake_service(layout, 1, move |opening| {
                record.lock().unwrap().push((opening.role, opening.control));
            });
            // The service exits once it has answered the quit, and its lock goes with it.
            let released = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let exits = {
                let released = std::sync::Arc::clone(&released);
                std::thread::spawn(move || {
                    service.join().unwrap();
                    released.store(true, std::sync::atomic::Ordering::SeqCst);
                    drop(lock);
                })
            };
            assert!(SystemProcessControl.quit(layout));
            assert!(
                released.load(std::sync::atomic::Ordering::SeqCst),
                "the installer waited for the answer, not for the lock"
            );
            exits.join().unwrap();
            assert_eq!(
                asked.lock().unwrap().as_slice(),
                [(
                    Role::Control,
                    Some(Control::Quit {
                        reason: QuitReason::Replace
                    })
                )]
            );
        }
        {
            let fixture = Fixture::new();
            assert!(SystemProcessControl.quit(&fixture.layout));
        }
        {
            let fixture = Fixture::new();
            let record = |relay: &str| InstallRecord {
                service_version: VERSION.to_owned(),
                relay_revision: relay.to_owned(),
                service_sha256: "0".repeat(64),
                edge_sha256: None,
                channel: Channel::Release,
            };
            record(EARLIER_DEVELOPMENT_RELAY)
                .write(&fixture.layout.install_record())
                .unwrap();
            assert!(
                !SystemProcessControl.quit(&fixture.layout),
                "the caller is told to stop the image"
            );
            record(RELAY)
                .write(&fixture.layout.install_record())
                .unwrap();
            assert!(SystemProcessControl.quit(&fixture.layout));
        }
        {
            let fixture = Fixture::new();
            let layout = &fixture.layout;
            let service = fake_service(layout, 2, |opening| {
                assert_eq!(opening.control, Some(Control::Status));
            });
            assert_eq!(
                SystemProcessControl.instance(layout).as_deref(),
                Some("instance_new")
            );
            assert_eq!(
                SystemProcessControl.wait_ready(layout, Some("instance_old")),
                Readiness::Answered
            );
            service.join().unwrap();
        }
        {
            let fixture = Fixture::new();
            let started = Instant::now();
            assert_eq!(
                SystemProcessControl.wait_ready(&fixture.layout, None),
                Readiness::Stopped
            );
            let waited = started.elapsed();
            assert!(waited >= STOPPED_GRACE, "{waited:?}");
            assert!(waited < READY_WAIT / 2, "{waited:?}");
        }
    }

    /// Protects defects 31f2c835 and a663e83d: a stray is a program an older installation of this home
    /// left running, named by its image even when its file is gone, and of its own kind when it runs
    /// from the retired development directory or a 1.x version directory. The current programs, the
    /// service that answers, programs elsewhere and the caller never are. A real retired connector is
    /// found and stopped by its exact pid, and a pid that now runs another image is left alone.
    #[test]
    fn only_this_homes_leftover_programs_are_strays_and_only_their_exact_image_is_stopped() {
        {
            let (_scratch, layout) = stray_home();
            for name in retired_connector_names() {
                let present = layout.bin().join(&name);
                fs::write(&present, "connector").unwrap();
                let moved = layout.bin().join(format!("{name}.old-2"));
                let gone = layout.bin().join(format!("{name}.old-9"));
                for image in [&present, &moved, &gone] {
                    assert_eq!(
                        kind_of(&layout, image, 7001, None),
                        Some(StrayKind::RetiredConnector),
                        "{}",
                        image.display()
                    );
                }
            }
        }
        {
            let (_scratch, layout) = stray_home();
            let [connector, _] = retired_connector_names();
            let removed_connector = layout.bin().join(format!("{connector} (deleted)"));
            let removed_service = layout
                .bin()
                .join(format!("{SERVICE_EXECUTABLE_NAME}.old-1 (deleted)"));
            let connector_image = running_image(&removed_connector);
            let service_image = running_image(&removed_service);
            assert_eq!(connector_image, layout.bin().join(&connector));
            assert_eq!(
                kind_of(&layout, &connector_image, 7010, None),
                Some(StrayKind::RetiredConnector)
            );
            assert_eq!(
                kind_of(&layout, &service_image, 7011, Some(7012)),
                Some(StrayKind::ReplacedService)
            );
            let present = layout.bin().join(&connector);
            assert_eq!(running_image(&present), present);
        }
        {
            let (_scratch, layout) = stray_home();
            let old = layout
                .bin()
                .join(format!("{SERVICE_EXECUTABLE_NAME}.old-1"));
            assert_eq!(
                kind_of(&layout, &old, 7002, Some(7003)),
                Some(StrayKind::ReplacedService)
            );
            assert_eq!(
                kind_of(&layout, &old, 7002, None),
                Some(StrayKind::ReplacedService)
            );
            assert_eq!(kind_of(&layout, &old, 7002, Some(7002)), None);
        }
        {
            let (scratch, layout) = stray_home();
            let mut current = Vec::new();
            let directory = layout.bin();
            for name in [
                EDGE_EXECUTABLE_NAME,
                SERVICE_EXECUTABLE_NAME,
                DESKTOP_EXECUTABLE_NAME,
            ] {
                current.push(directory.join(name));
            }
            current.push(directory.join(format!("{EDGE_EXECUTABLE_NAME}.old-1")));
            current.push(directory.join(format!("{DESKTOP_EXECUTABLE_NAME}.old-1")));
            for image in &current {
                assert_eq!(
                    kind_of(&layout, image, 7004, None),
                    None,
                    "{}",
                    image.display()
                );
            }
            let elsewhere = fs::canonicalize(scratch.path()).unwrap().join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            for name in retired_connector_names() {
                let image = elsewhere.join(&name);
                assert_eq!(kind_of(&layout, &image, 7005, None), None);
            }
            let image = elsewhere.join(format!("{SERVICE_EXECUTABLE_NAME}.old-1"));
            assert_eq!(kind_of(&layout, &image, 7005, None), None);
        }
        {
            let (scratch, layout) = stray_home();
            let directory = layout.retired_development_directory();
            for name in [
                EDGE_EXECUTABLE_NAME.to_owned(),
                SERVICE_EXECUTABLE_NAME.to_owned(),
                DESKTOP_EXECUTABLE_NAME.to_owned(),
                format!("{SERVICE_EXECUTABLE_NAME}.old-1"),
                format!("{EDGE_EXECUTABLE_NAME}.old-3"),
                retired_connector_names()[0].clone(),
                "helper".to_owned(),
            ] {
                let image = directory.join(&name);
                assert_eq!(
                    kind_of(&layout, &image, 7020, Some(7021)),
                    Some(StrayKind::RetiredDevelopment),
                    "{name}"
                );
            }
            let answering = directory.join(SERVICE_EXECUTABLE_NAME);
            assert_eq!(
                kind_of(&layout, &answering, 7024, Some(7024)),
                Some(StrayKind::RetiredDevelopment),
                "the answering service is not spared when it runs from the retired directory"
            );
            let beside = directory.with_file_name("development-notes").join("helper");
            assert_eq!(kind_of(&layout, &beside, 7022, None), None);
            let elsewhere = fs::canonicalize(scratch.path())
                .unwrap()
                .join("dev")
                .join(EDGE_EXECUTABLE_NAME);
            assert_eq!(kind_of(&layout, &elsewhere, 7023, None), None);
            assert_eq!(
                serde_json::to_value(StrayKind::RetiredDevelopment).unwrap(),
                serde_json::json!("retired_development")
            );
        }
        {
            let (_scratch, layout) = stray_home();
            let directory = layout.bin().join("v1.3.6");
            fs::create_dir_all(&directory).unwrap();
            fs::create_dir_all(layout.bin().join("vnext")).unwrap();
            for name in [
                EDGE_EXECUTABLE_NAME.to_owned(),
                SERVICE_EXECUTABLE_NAME.to_owned(),
                retired_connector_names()[1].clone(),
                "helper".to_owned(),
            ] {
                let image = directory.join(&name);
                assert_eq!(
                    kind_of(&layout, &image, 7030, Some(7031)),
                    Some(StrayKind::RetiredVersionDirectory),
                    "{name}"
                );
            }
            let answering = directory.join(SERVICE_EXECUTABLE_NAME);
            assert_eq!(
                kind_of(&layout, &answering, 7032, Some(7032)),
                Some(StrayKind::RetiredVersionDirectory),
                "the answering service is not spared when it runs from a version directory"
            );
            let beside = layout.bin().join("vnext").join(SERVICE_EXECUTABLE_NAME);
            assert_eq!(kind_of(&layout, &beside, 7033, None), None);
            assert_eq!(
                serde_json::to_value(StrayKind::RetiredVersionDirectory).unwrap(),
                "retired_version_directory"
            );
        }
        {
            let (_scratch, layout) = stray_home();
            let image = layout.bin().join(&retired_connector_names()[0]);
            assert_eq!(
                classify(&image, 7006, &layout, None, 7006),
                None,
                "the caller"
            );
            assert_eq!(
                classify(&image, 7006, &layout, None, 7007),
                Some(StrayKind::RetiredConnector)
            );
        }
        {
            assert!(stop_strays(&[]).is_empty());
            let caller = StrayProcess {
                pid: std::process::id(),
                image: PathBuf::from("caller"),
                kind: StrayKind::ReplacedService,
            };
            assert_eq!(stop_strays(std::slice::from_ref(&caller)), [caller]);
        }
        {
            let (_scratch, layout) = stray_home();
            let image = layout.bin().join(&retired_connector_names()[0]);
            let mut stand_in = start_stand_in(&image);
            let pid = stand_in.0.id();

            let stray = wait_for_stray(&layout, pid);
            assert_eq!(stray.kind, StrayKind::RetiredConnector);
            assert!(same_path(&stray.image, &image), "{}", stray.image.display());

            // The pid now belongs to another program than the one the scan saw: leave it alone.
            let other = StrayProcess {
                image: layout.bin().join(&retired_connector_names()[1]),
                ..stray.clone()
            };
            assert!(stop_strays(&[other]).is_empty());
            assert!(
                stand_in.0.try_wait().unwrap().is_none(),
                "a pid running another image was stopped"
            );

            assert!(stop_strays(&[stray]).is_empty());
            let deadline = Instant::now() + STRAY_STOP_WAIT;
            while stand_in.0.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline, "the stray is still running");
                std::thread::sleep(POLL);
            }
            assert!(stray_processes(&layout, None)
                .iter()
                .all(|found| found.pid != pid));
        }
    }
}
