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
//!    and a development installation, does nothing for the same intact version, rename-swaps the
//!    service image into `bin`, and replaces the edge only when the relay revision changes, `bin`
//!    still holds the pre-edge layout, or the edge in `bin` is not the one recorded.
//!    `--development` places the same files the same way, but always swaps the service, whatever
//!    version is installed, and records the development channel. `--restore` does the same for
//!    this executable's own package and records the release channel: the one way back from a
//!    development installation, and the one allowed move down. The Ghostlight shell goes beside
//!    the service image when the payload has one and `bin` holds another;
//! 3. reconciles every registration to name `bin`, as soon as `bin` holds the executables and
//!    before anything old is stopped, so a browser or client that relaunches its host the moment
//!    the old one stops starts the new one;
//! 4. stops the 1.x images and looks again for a few rounds for any that respawned;
//! 5. asks the running service to quit (a control `quit` for replacement) and waits for its
//!    service lock to be free, and stops that exact image if it has not exited or is an earlier
//!    development build that cannot be asked; stops what runs from the retired development
//!    directory and removes that directory and its marker, which earlier development builds used;
//!    archives the 1.x audit log, then removes the 1.x lifecycle files, the installation record
//!    last;
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
use super::layout::{Layout, DESKTOP_EXECUTABLE_NAME};
use super::migration::{self, FlatpakStep, Legacy, LegacyRoots, MigrationPlan};
use super::reconcile::{PendingRegistration, ReconcileReport, Registrations};
use super::record::{self, Channel, InstallRecord, PayloadRefused, Standing};
use super::swap;
use crate::cli::client;
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

/// The relay revision of the development builds that preceded relay 4. They hold no service lock
/// and cannot be asked to quit, so the installer stops their exact images instead.
const EARLIER_DEVELOPMENT_RELAY: &str = "3";

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
    /// The programs of an older installation that still run in `layout`'s home, other than the
    /// service `service_pid`.
    fn strays(&self, layout: &Layout, service_pid: Option<u32>) -> Vec<StrayProcess>;
    /// Stop `strays` and wait for them to exit. Returns the ones that would not stop.
    fn stop_strays(&self, strays: &[StrayProcess]) -> Vec<StrayProcess>;
    /// Start `bin/ghostlight-service serve --reason update` in `layout`'s home, detached, without waiting
    /// for it.
    fn start_serve(&self, layout: &Layout) -> io::Result<()>;
    /// Wait up to 30 s for the service of `layout`'s home to answer an accepted control status
    /// whose instance is not `previous`.
    fn wait_ready(&self, layout: &Layout, previous: Option<&str>) -> bool;
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

    fn wait_ready(&self, layout: &Layout, previous: Option<&str>) -> bool {
        let deadline = Instant::now() + READY_WAIT;
        loop {
            let answered = client::status_at(layout.home())
                .is_ok_and(|status| Some(status.instance.as_str()) != previous);
            if answered {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
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

/// The text the swap puts before the number in the name of an image it moves aside.
const MOVED_ASIDE_MARKER: &str = ".old-";

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
/// `service_pid` is the service that answers, when one does: its image is never a stray. The
/// process table is read once.
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
/// left alone. The name decides, so a process whose file has since been removed is still found.
/// The current edge, the shell and the current service image are never strays, nor is the
/// process asking.
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
    let file_name = image.file_name()?.to_str()?;
    let (base, moved_aside) = match file_name.rsplit_once(MOVED_ASIDE_MARKER) {
        Some((base, number))
            if !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            (base, true)
        }
        _ => (file_name, false),
    };
    if migration::retired_connector_names()
        .iter()
        .any(|name| same_file_name(name, base))
    {
        return Some(StrayKind::RetiredConnector);
    }
    (moved_aside && same_file_name(SERVICE_EXECUTABLE_NAME, base) && Some(pid) != service_pid)
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
    /// Whether the running service is replaced by a new `serve`.
    pub restart: bool,
    /// The record written once the new service answers.
    pub record: Option<InstallRecord>,
    /// Images stopped: the old installation's, the retired development directory's, and the
    /// running service's if it does not quit.
    pub processes_to_stop: Vec<PathBuf>,
    /// Files and directories removed once `bin` holds the new executables: the retired
    /// development directory and its marker, where an earlier development build ran.
    pub deletions: Vec<PathBuf>,
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
    /// What the reconcile changed; `None` when nothing was installed.
    pub registrations: Option<ReconcileReport>,
    /// What became of the files the 1.x Flatpak route left in the home.
    pub flatpak: Vec<FlatpakStep>,
    /// Entries of the plan's deletions that could not be removed, because a program still runs
    /// from them. The next install tries again; this one is not failed by them.
    pub left_behind: Vec<PathBuf>,
}

/// A release package found an installation that a development build made.
#[derive(Debug, Error)]
#[error(
    "The installed Ghostlight is a development build, so this package did not replace it. To \
     return to a release, run: {restore}"
)]
pub struct DevelopmentInstalled {
    /// The command that installs this package as a release over the development build.
    pub restore: String,
}

impl DevelopmentInstalled {
    /// The refusal that names `executable`, the service image of the refused package.
    fn new(executable: &Path) -> Self {
        let name = executable.display().to_string();
        let name = if name.contains(char::is_whitespace) {
            format!("\"{name}\"")
        } else {
            name
        };
        Self {
            restore: format!("{name} deployment install {RESTORE_OPTION}"),
        }
    }
}

/// The option that returns a development installation to a release.
const RESTORE_OPTION: &str = "--restore";

/// Why a payload directory cannot be installed.
#[derive(Debug, Error)]
pub enum PayloadError {
    /// An executable is missing from the payload.
    #[error("The Ghostlight package is missing {0}")]
    Missing(PathBuf),
    /// The payload already is the directory it would be installed into.
    #[error("Run the installation from the package's own directory, not from {0}")]
    InPlace(PathBuf),
    /// The payload's service is another build than the running installer.
    #[error("{0} holds another build of Ghostlight; run that build's own deployment install")]
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
    match outcome.service_started {
        Some(false) => Err(ServiceDidNotStart(layout.bin()).into()),
        _ => Ok(()),
    }
}

/// The new service did not answer in time.
#[derive(Debug, Error)]
#[error(
    "The Ghostlight service in {0} did not start within 30 s. The installation record was not \
     written, so the next launch installs again"
)]
pub struct ServiceDidNotStart(pub PathBuf);

/// Plan `request` and, unless it only asks for the plan, carry it out under the install lock.
pub fn install(context: &Context<'_>, request: &Request) -> anyhow::Result<Outcome> {
    if request.plan_only {
        return Ok(Outcome {
            plan: plan(context, request.mode)?,
            service_started: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
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
    let mut plan = InstallPlan {
        mode,
        version: context.build.version.clone(),
        migration,
        unchanged: false,
        swaps: Vec::new(),
        restart: true,
        record: None,
        processes_to_stop: Vec::new(),
        deletions: retired.deletions.clone(),
        registrations_to_rewrite: Vec::new(),
    };
    let edge_changes = edge_changes(context, installed.as_ref())?;
    match mode {
        Mode::Release => {
            if installed
                .as_ref()
                .is_some_and(|record| record.channel == Channel::Development)
            {
                let service = context.payload.join(SERVICE_EXECUTABLE_NAME);
                return Err(DevelopmentInstalled::new(&service).into());
            }
            validate_payload(&context.payload, &layout.bin())?;
            let image = installed_image(layout)?;
            let payload_record = context.build.record(None, Channel::Release);
            match record::decide(installed.as_ref(), &payload_record, image.as_deref()) {
                Standing::Newer => {
                    let installed = installed.as_ref().expect("a newer standing has a record");
                    return Err(PayloadRefused::new(installed, &payload_record).into());
                }
                Standing::Same if plan.migration.is_empty() && retired.is_empty() => {
                    plan.unchanged = true;
                    plan.restart = false;
                    return Ok(plan);
                }
                Standing::Same => {}
                Standing::Absent | Standing::Older | Standing::Damaged => {
                    place_in_bin(
                        context,
                        &mut plan,
                        installed.as_ref(),
                        edge_changes,
                        Channel::Release,
                    )?;
                }
            }
        }
        Mode::Development | Mode::Restore => {
            validate_payload(&context.payload, &layout.bin())?;
            let channel = if mode == Mode::Development {
                Channel::Development
            } else {
                Channel::Release
            };
            place_in_bin(
                context,
                &mut plan,
                installed.as_ref(),
                edge_changes,
                channel,
            )?;
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
/// marker are removed. An entry a running program keeps is reported in [`Outcome::left_behind`];
/// it does not fail the install.
pub fn execute(context: &Context<'_>, plan: InstallPlan) -> anyhow::Result<Outcome> {
    let layout = &context.layout;
    if plan.unchanged {
        return Ok(Outcome {
            plan,
            service_started: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
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
        let strays = context.control.strays(layout, None);
        if !strays.is_empty() {
            // A program that will not stop is left for `doctor` to name; it blocks nothing here.
            let _ = context.control.stop_strays(&strays);
        }
        left_behind = remove_retired_development(context.control, layout, &plan);
        if migrating {
            migration::retire_state(&copied)?;
            migration::archive_audit(&plan.migration)?;
            migration::finish(&plan.migration)?;
            flatpak = migration::retire_flatpak_route(&plan.migration);
        }
        Some(
            context.control.start_serve(layout).is_ok()
                && context.control.wait_ready(layout, replaced.as_deref()),
        )
    } else {
        None
    };
    if service_started == Some(true) {
        if let Some(record) = &plan.record {
            record.write(&layout.install_record())?;
        }
    }
    Ok(Outcome {
        plan,
        service_started,
        registrations: Some(registrations),
        flatpak,
        left_behind,
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
            .context("Could not stop the old Ghostlight programs")
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

/// Whether the install replaces the edge in `bin`: there is no record, the edge speaks another
/// relay revision, `bin` still holds the pre-edge layout, or the edge there is missing or is not
/// the one the record says was installed.
fn edge_changes(context: &Context<'_>, installed: Option<&InstallRecord>) -> io::Result<bool> {
    let relay_changes =
        installed.map(|record| &record.relay_revision) != Some(&context.build.relay_revision);
    if relay_changes || migration::pre_edge_layout(&context.layout) {
        return Ok(true);
    }
    let Some(recorded) = installed.and_then(|record| record.edge_sha256.as_deref()) else {
        return Ok(true);
    };
    match record::sha256_file(&context.layout.edge()) {
        Ok(sha) => Ok(sha != recorded),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

/// Add the swaps that place the payload in `bin` to `plan`, and set the record `bin` holds once
/// they are done: the service always, the shell when the payload has one `bin` does not, and the
/// edge when `edge_changes` (the edge it leaves is the payload's, or the installed one).
fn place_in_bin(
    context: &Context<'_>,
    plan: &mut InstallPlan,
    installed: Option<&InstallRecord>,
    edge_changes: bool,
    channel: Channel,
) -> anyhow::Result<()> {
    plan.swaps.push(service_swap(context));
    if shell_changes(context) {
        plan.swaps.push(desktop_swap(context));
    }
    let edge_sha256 = if edge_changes {
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

/// Stop what runs from the retired development directory and remove what `plan` lists as
/// deletions. Returns the entries that could not be removed, which are the ones a program still
/// runs from. A stop that fails is not an error here: the removal that follows names what stayed.
fn remove_retired_development(
    control: &dyn ProcessControl,
    layout: &Layout,
    plan: &InstallPlan,
) -> Vec<PathBuf> {
    if plan.deletions.is_empty() {
        return Vec::new();
    }
    let retired = RetiredDevelopment::find(layout);
    if !retired.images.is_empty() {
        let _ = control.stop_image(&retired.images);
    }
    plan.deletions
        .iter()
        .filter(|path| {
            let removed = if path.is_dir() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
            removed.is_err_and(|error| error.kind() != io::ErrorKind::NotFound)
        })
        .cloned()
        .collect()
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
                    "Ghostlight installation is still changing; retry shortly",
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
        retired_connector_names, FlatpakLeftovers, FlatpakPart, FlatpakResult, LegacyInstallation,
        MigrationError, AUDIT_FILE_NAME, HARNESS_LOCATIONS_NAME, LEGACY_DEPLOY_LOCK_NAME,
        LEGACY_RECORD_NAME, LEGACY_STARTUP_NAME, USER_POLICY_FILE_NAME,
    };
    use super::super::native_host::NativeHostRegistry;
    use super::super::scratch::Scratch;
    use super::super::HarnessRegistry;
    use super::*;
    use crate::intake::listener::{self, control};
    use crate::intake::opening::read_opening;
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
        calls: RefCell<Vec<Call>>,
        sightings: RefCell<Vec<Sighting>>,
    }

    impl FakeControl {
        fn new() -> Self {
            Self {
                quits: true,
                answers: true,
                instance: None,
                waited_for: RefCell::new(Vec::new()),
                respawns: Cell::new(0),
                witness: None,
                policy_witness: None,
                writes_on_stop: Vec::new(),
                failing_stops: Cell::new(0),
                strays: Vec::new(),
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
            self.note(Call::StopStrays(
                strays.iter().map(|stray| stray.pid).collect(),
            ));
            Vec::new()
        }

        fn start_serve(&self, _layout: &Layout) -> io::Result<()> {
            self.note(Call::Start);
            Ok(())
        }

        fn instance(&self, _layout: &Layout) -> Option<String> {
            self.instance.clone()
        }

        fn wait_ready(&self, _layout: &Layout, previous: Option<&str>) -> bool {
            self.note(Call::Wait);
            self.waited_for
                .borrow_mut()
                .push(previous.map(str::to_owned));
            self.answers
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
                installation: (self.legacy_policy.is_some() || self.legacy_serving.is_some()).then(
                    || LegacyInstallation {
                        policy_directory: self.legacy_policy.clone(),
                        serving_directory: self.legacy_serving.clone(),
                        ..LegacyInstallation::default()
                    },
                ),
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

    #[test]
    fn a_fresh_install_places_everything_and_records_it_once_the_service_answers() {
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

    #[test]
    fn a_restart_stops_what_an_older_installation_left_running_after_the_quit_and_before_the_start()
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

    #[test]
    fn an_install_that_finds_no_stray_stops_none() {
        let fixture = Fixture::new();
        let control = FakeControl::new();
        fixture.run(Mode::Release, VERSION, &control).unwrap();
        assert!(!control
            .calls()
            .iter()
            .any(|call| matches!(call, Call::StopStrays(_))));
    }

    #[test]
    fn the_same_version_is_left_alone() {
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

    #[test]
    fn an_older_payload_is_refused_and_changes_nothing() {
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

    #[test]
    fn a_damaged_service_image_is_repaired() {
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

    #[test]
    fn the_edge_stays_when_the_relay_revision_matches() {
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
        assert_eq!(
            outcome
                .plan
                .swaps
                .iter()
                .map(|placed| placed.name.as_str())
                .collect::<Vec<_>>(),
            [SERVICE_EXECUTABLE_NAME]
        );
        assert_eq!(
            fixture.read(fixture.layout.service_executable()),
            "next service"
        );
        assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
        let record = fixture.record().unwrap();
        assert_eq!(record.service_version, "2.2.0");
        assert_eq!(record.edge_sha256, recorded);
    }

    #[test]
    fn the_edge_is_replaced_when_the_relay_revision_changes() {
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
        install(
            &fixture.context(build, &control),
            &Request {
                payload: None,
                mode: Mode::Release,
                plan_only: false,
            },
        )
        .unwrap();
        assert_eq!(fixture.read(fixture.layout.edge()), "next edge");
        let record = fixture.record().unwrap();
        assert_eq!(record.relay_revision, "5");
        assert_eq!(
            record.edge_sha256,
            Some(record::sha256_file(&fixture.layout.edge()).unwrap())
        );
    }

    #[test]
    fn a_pre_edge_layout_is_replaced_and_its_connectors_go_after_an_exact_image_stop() {
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

    #[test]
    fn a_service_that_does_not_quit_is_stopped_by_its_exact_image() {
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

    #[test]
    fn a_development_install_swaps_the_service_into_bin_and_keeps_an_intact_edge() {
        let fixture = Fixture::new();
        fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        let recorded = fixture.record().unwrap();
        assert_eq!(recorded.channel, Channel::Release);
        Fixture::write_payload(&fixture.payload, "development service");
        fs::write(fixture.payload.join(EDGE_EXECUTABLE_NAME), "rebuilt edge").unwrap();
        let control = FakeControl::new();
        let outcome = fixture.run(Mode::Development, VERSION, &control).unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert_eq!(placed_names(&outcome.plan), [SERVICE_EXECUTABLE_NAME]);
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

    #[test]
    fn a_development_install_with_no_record_also_swaps_the_edge() {
        let fixture = Fixture::new();
        let outcome = fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert_eq!(
            placed_names(&outcome.plan),
            [SERVICE_EXECUTABLE_NAME, EDGE_EXECUTABLE_NAME]
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

    #[test]
    fn a_development_install_swaps_an_edge_that_is_not_the_recorded_one() {
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
        assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
        assert_eq!(fixture.record().unwrap().edge_sha256, recorded);

        fs::remove_file(fixture.layout.edge()).unwrap();
        let outcome = fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        assert!(placed_names(&outcome.plan).contains(&EDGE_EXECUTABLE_NAME));
        assert_eq!(fixture.read(fixture.layout.edge()), edge_text(RELAY));
    }

    #[test]
    fn a_development_install_of_the_same_version_swaps_and_restarts_every_time() {
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

    #[test]
    fn a_development_install_accepts_any_version() {
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

    #[test]
    fn a_release_install_over_a_development_installation_is_refused_and_changes_nothing() {
        let fixture = Fixture::new();
        fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        let before = fixture.snapshot();
        for version in ["2.0.0", VERSION, "9.9.9"] {
            let control = FakeControl::new();
            let error = fixture.run(Mode::Release, version, &control).unwrap_err();
            let refusal = error
                .downcast_ref::<DevelopmentInstalled>()
                .unwrap_or_else(|| panic!("{error}"));
            let service = fixture.payload.join(SERVICE_EXECUTABLE_NAME);
            assert!(
                refusal.restore.ends_with(&format!(
                    "{} deployment install --restore",
                    service.display()
                )),
                "{}",
                refusal.restore
            );
            assert!(error.to_string().contains("development build"), "{error}");
            assert!(control.calls().is_empty());
            assert_eq!(fixture.snapshot(), before, "{version}");
        }
    }

    #[test]
    fn a_restore_from_a_payload_older_than_the_development_installation_installs_a_release() {
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

    #[test]
    fn a_restore_replaces_the_edge_by_the_normal_rule_and_is_not_offline() {
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

    #[test]
    fn a_restore_with_nothing_installed_is_an_install() {
        let fixture = Fixture::new();
        let outcome = fixture
            .run(Mode::Restore, VERSION, &FakeControl::new())
            .unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
        assert_eq!(fixture.record().unwrap().channel, Channel::Release);
    }

    #[test]
    fn the_retired_development_layout_is_planned_and_removed_after_the_bin_swap() {
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

        let calls = control.calls();
        let Call::Stop(stopped) = &calls[1] else {
            panic!("{calls:?}");
        };
        let mut stopped = stopped.clone();
        stopped.sort();
        let mut expected = images;
        expected.sort();
        assert_eq!(stopped, expected);
        assert_eq!(calls[0], Call::Quit);
        assert_eq!(calls[2..], [Call::Start, Call::Wait]);
        for sighting in control.sightings() {
            assert!(
                sighting.registered,
                "registrations name bin before any stop"
            );
        }
        assert!(fixture.manifest_names_bin_edge());
    }

    #[test]
    fn a_service_still_running_from_the_retired_directory_is_stopped_by_its_exact_image() {
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

    #[test]
    fn a_marker_with_no_directory_and_a_directory_with_no_marker_are_each_removed() {
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

    #[test]
    fn a_same_release_with_a_retired_development_layout_still_cleans_it_up() {
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

    #[cfg(windows)]
    #[test]
    fn an_entry_a_running_program_keeps_is_reported_and_does_not_fail_the_install() {
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

    #[test]
    fn a_legacy_layout_moves_its_state_into_the_home() {
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

    #[test]
    fn the_flatpak_route_of_a_1x_install_is_removed_and_reported_once() {
        let fixture = Fixture::new();
        let route = FlatpakLeftovers::below(&fixture.scratch.path().join("profile"));
        fs::create_dir_all(route.manifest.parent().unwrap()).unwrap();
        fs::write(
            &route.manifest,
            serde_json::json!({
                "name": super::super::native_host::HOST_NAME,
                "description": "Ghostlight browser connector",
                "path": "/home/user/.ghostlight/bin/ghostlight",
                "type": "stdio",
            })
            .to_string(),
        )
        .unwrap();
        let outcome = fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        assert!(!outcome.plan.migration.is_empty());
        assert_eq!(outcome.flatpak.len(), 1);
        assert_eq!(outcome.flatpak[0].part, FlatpakPart::Manifest);
        assert_eq!(outcome.flatpak[0].result, FlatpakResult::Removed);
        assert!(!route.manifest.exists());
        let again = fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        assert!(again.plan.unchanged);
        assert!(again.flatpak.is_empty());
    }

    #[test]
    fn registrations_name_bin_before_any_old_program_is_stopped() {
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

    #[test]
    fn a_client_the_user_located_is_reconciled_to_bin() {
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

    #[test]
    fn the_1x_user_policy_stays_where_the_1x_service_reads_it_until_the_last_stop() {
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

    #[test]
    fn a_policy_saved_by_the_1x_service_while_it_stops_is_not_lost() {
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

    #[test]
    fn a_policy_that_diverged_is_reported_and_the_next_install_is_not_blocked() {
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

    #[test]
    fn a_run_that_failed_to_stop_the_1x_service_is_finished_by_the_next_install() {
        let mut fixture = Fixture::new();
        let (original, copy) = failed_run_after_the_copy(&mut fixture);
        let outcome = fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert!(!original.exists());
        assert_eq!(fixture.read(copy), "user policy");
    }

    #[test]
    fn a_1x_save_after_a_failed_run_wins_on_the_next_install() {
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

    #[test]
    fn a_2_0_rewrite_of_the_copy_after_a_failed_run_is_kept_on_the_next_install() {
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

    #[test]
    fn both_sides_changing_after_a_failed_run_is_reported_once_and_the_third_install_succeeds() {
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

    #[test]
    fn a_destination_that_was_never_copied_still_refuses_the_install() {
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

    #[test]
    fn every_migrating_run_restarts_archives_the_audit_log_and_removes_the_lifecycle_files() {
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

    #[test]
    fn a_respawned_legacy_program_is_stopped_again() {
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

    #[test]
    fn the_legacy_record_is_gone_after_the_last_stop() {
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

    #[test]
    fn a_legacy_service_that_served_from_bin_is_stopped_by_its_moved_image() {
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

    #[test]
    fn a_failed_migration_swaps_nothing() {
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

    #[test]
    fn a_service_that_never_answers_leaves_the_record_unwritten() {
        let fixture = Fixture::new();
        let mut control = FakeControl::new();
        control.answers = false;
        let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
        assert_eq!(outcome.service_started, Some(false));
        assert!(fixture.layout.service_executable().is_file());
        assert_eq!(fixture.record(), None);
    }

    #[test]
    fn a_plan_changes_nothing() {
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

    #[test]
    fn a_payload_missing_an_executable_is_refused() {
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

    #[test]
    fn a_held_install_lock_is_waited_for() {
        let scratch = Scratch::new();
        let lock = scratch.path().join("install.lock");
        let held = hold_install_lock(&lock).unwrap();
        let waiter = {
            let lock = lock.clone();
            std::thread::spawn(move || hold_install_lock(&lock).map(drop))
        };
        std::thread::sleep(Duration::from_millis(100));
        drop(held);
        waiter.join().unwrap().unwrap();
    }

    #[test]
    fn the_new_service_is_waited_for_by_the_instance_it_replaces() {
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
                    },
                )
                .unwrap();
            }
        })
    }

    #[test]
    fn the_installer_asks_the_service_to_quit_for_replacement_and_waits_for_its_lock() {
        let fixture = Fixture::new();
        let layout = &fixture.layout;
        let lock = ServiceLock::try_acquire(&layout.service_lock())
            .unwrap()
            .expect("the service lock");
        let held = std::sync::Mutex::new(Some(lock));
        let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = std::sync::Arc::clone(&asked);
        let delay = Duration::from_millis(300);
        let service = fake_service(layout, 1, move |opening| {
            record.lock().unwrap().push((opening.role, opening.control));
            // The service exits a moment after it hears the quit: its lock goes with it.
            let lock = held.lock().unwrap().take();
            let _ = std::thread::spawn(move || {
                std::thread::sleep(delay);
                drop(lock);
            });
        });
        let started = Instant::now();
        assert!(SystemProcessControl.quit(layout));
        assert!(
            started.elapsed() >= delay,
            "the installer waited for the lock, not for the answer"
        );
        service.join().unwrap();
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

    #[test]
    fn a_home_with_no_running_service_has_nothing_to_wait_for() {
        let fixture = Fixture::new();
        assert!(SystemProcessControl.quit(&fixture.layout));
    }

    #[test]
    fn an_earlier_development_build_cannot_be_asked_to_quit() {
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

    #[test]
    fn the_new_service_is_ready_when_an_instance_other_than_the_old_one_answers() {
        let fixture = Fixture::new();
        let layout = &fixture.layout;
        let service = fake_service(layout, 2, |opening| {
            assert_eq!(opening.control, Some(Control::Status));
        });
        assert_eq!(
            SystemProcessControl.instance(layout).as_deref(),
            Some("instance_new")
        );
        assert!(SystemProcessControl.wait_ready(layout, Some("instance_old")));
        service.join().unwrap();
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

    #[test]
    fn a_development_payload_with_a_shell_places_it_in_bin_when_bin_holds_another() {
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

    #[test]
    fn a_release_payload_with_a_shell_places_it_in_bin_beside_the_service() {
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

    #[test]
    fn a_payload_without_a_shell_installs_as_before_and_still_validates() {
        let fixture = Fixture::new();
        for mode in [Mode::Release, Mode::Development] {
            let control = FakeControl::new();
            let planned = plan(&fixture.context(fixture.build(VERSION), &control), mode).unwrap();
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

    #[test]
    fn a_retired_connector_under_the_home_is_a_stray_whether_or_not_its_file_exists() {
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

    #[test]
    fn a_process_whose_file_was_removed_is_still_named_by_its_image() {
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

    #[test]
    fn a_moved_aside_service_is_a_stray_unless_it_is_the_service_that_answers() {
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

    #[test]
    fn the_current_programs_and_programs_elsewhere_are_never_strays() {
        let (scratch, layout) = stray_home();
        let mut current = Vec::new();
        for directory in [layout.bin(), layout.retired_development_directory()] {
            for name in [
                EDGE_EXECUTABLE_NAME,
                SERVICE_EXECUTABLE_NAME,
                DESKTOP_EXECUTABLE_NAME,
            ] {
                current.push(directory.join(name));
            }
            current.push(directory.join(format!("{EDGE_EXECUTABLE_NAME}.old-1")));
            current.push(directory.join(format!("{DESKTOP_EXECUTABLE_NAME}.old-1")));
        }
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

    #[test]
    fn the_calling_process_is_never_a_stray() {
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

    #[test]
    fn stopping_nothing_and_stopping_the_caller_leave_everything_running() {
        assert!(stop_strays(&[]).is_empty());
        let caller = StrayProcess {
            pid: std::process::id(),
            image: PathBuf::from("caller"),
            kind: StrayKind::ReplacedService,
        };
        assert_eq!(stop_strays(std::slice::from_ref(&caller)), [caller]);
    }

    #[test]
    fn a_stray_is_described_for_doctor_json() {
        let stray = StrayProcess {
            pid: 12,
            image: PathBuf::from("a/b"),
            kind: StrayKind::RetiredConnector,
        };
        assert_eq!(
            serde_json::to_value(&stray).unwrap(),
            serde_json::json!({"pid": 12, "image": "a/b", "kind": "retired_connector"})
        );
        assert_eq!(
            serde_json::to_value(StrayKind::ReplacedService).unwrap(),
            serde_json::json!("replaced_service")
        );
    }
}
