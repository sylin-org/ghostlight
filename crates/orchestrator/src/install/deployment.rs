//! `ghostlight deployment install`: the one way executables reach the Ghostlight home.
//!
//! A package, the npm launcher, the get scripts and the development loop all run a `ghostlight`
//! with this command (ADR-0197 Decisions 13 to 15). Under the install lock it:
//!
//! 1. checks the 2.0 break migration's destinations when a 1.x layout is found, so a conflict
//!    changes nothing, then moves the 1.x policy, managed-policy and client-location files into
//!    the home: no 1.x program holds them open, and the 2.0 service or client that the next
//!    steps let start must find them in place;
//! 2. reads `bin/install.json` and decides what to replace: a release refuses an older payload,
//!    does nothing for the same intact version, rename-swaps the service image into `bin`, and
//!    replaces the connectors only when the relay revision changes; `--development` swaps the
//!    service into `dev` and writes the development marker (installing into `bin` as well when
//!    `bin` speaks another relay revision); `--restore` removes the marker;
//! 3. reconciles every registration to name `bin`, as soon as `bin` holds the executables and
//!    before anything old is stopped, so a browser or client that relaunches a connector the
//!    moment the old one stops starts the new one;
//! 4. stops the 1.x images and looks again for a few rounds for any that respawned;
//! 5. asks the running service to quit and stops that exact image if it has not exited, archives
//!    the 1.x audit log, then removes the 1.x lifecycle files, the installation record last;
//! 6. starts `bin/ghostlight serve` and writes `install.json` only once the new service
//!    publishes.
//!
//! There is no rollback: a service that does not start is reported, `install.json` stays as it
//! was, and the next launch installs again. `--plan` prints what would happen and changes
//! nothing. Every process start and stop goes through [`ProcessControl`], so tests never start a
//! process.

use std::env;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context as _;
use ghostlight_relay::home::{
    LEASE_FILE_NAME, RUNTIME_FILE_NAME, SERVICE_EXECUTABLE_NAME, TEST_HOME_VARIABLE,
};
use ghostlight_relay::opening::{ControlOp, Reply, Stream};
use ghostlight_relay::runtime::RuntimeFile;
use serde::Serialize;
use thiserror::Error;

use super::entry::{canonical_path, same_path};
use super::layout::{Layout, BROWSER_CONNECTOR_EXECUTABLE_NAME, MCP_CONNECTOR_EXECUTABLE_NAME};
use super::marker::DevelopmentMarker;
use super::migration::{self, FlatpakStep, Legacy, LegacyRoots, MigrationPlan};
use super::reconcile::{PendingRegistration, ReconcileReport, Registrations};
use super::record::{self, InstallRecord, PayloadRefused, Standing};
use super::swap;

/// The connectors one package carries beside the service.
const CONNECTORS: [&str; 2] = [
    MCP_CONNECTOR_EXECUTABLE_NAME,
    BROWSER_CONNECTOR_EXECUTABLE_NAME,
];

/// How long an install waits for another install to finish. An install can itself spend the
/// quit wait and the publish wait.
const INSTALL_LOCK_WAIT: Duration = Duration::from_secs(60);

/// Pause between attempts to take the install lock, between checks of a quitting service and
/// between looks for a 1.x program that respawned.
const POLL: Duration = Duration::from_millis(25);

/// How many times, after the 1.x images are stopped, the installer looks again for a program that
/// was started from one of them in the meantime.
const LEGACY_RESPAWN_ROUNDS: u32 = 3;

/// How long the running service has to finish in-flight work and exit after `quit`.
const QUIT_WAIT: Duration = Duration::from_secs(10);

/// How long a started service has to publish its runtime file.
const PUBLISH_WAIT: Duration = Duration::from_secs(30);

/// Pause between checks for a published runtime file.
const PUBLISH_POLL: Duration = Duration::from_millis(100);

/// What an install puts where.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// A release: the payload becomes the installed service in `bin`.
    Release,
    /// A development build: the payload serves from `dev`, selected by the marker.
    Development,
    /// Deselect the development build so the installed service serves again. Offline: nothing
    /// is copied.
    Restore,
}

/// One `deployment install` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The directory holding the payload's three executables; `None` is this executable's own
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
    /// Relay revision its connectors speak.
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

    /// The record `install.json` holds once this build serves from `bin`.
    fn record(&self) -> InstallRecord {
        InstallRecord {
            service_version: self.version.clone(),
            relay_revision: self.relay_revision.clone(),
            service_sha256: self.image_sha256.clone(),
        }
    }
}

/// Starting and stopping the service. The installer never starts or stops a process any other
/// way, so tests drive the whole install with a fake.
pub trait ProcessControl {
    /// Ask the service of `layout`'s home to quit and wait up to 10 s for it to exit. Returns
    /// whether no service of that home runs afterwards.
    fn quit(&self, layout: &Layout) -> bool;
    /// Stop every process running one of `images` and wait for it to exit.
    fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()>;
    /// Whether any process runs one of `images`.
    fn is_running(&self, images: &[PathBuf]) -> bool;
    /// Start `bin/ghostlight serve` in `layout`'s home, detached, without waiting for it.
    fn start_serve(&self, layout: &Layout) -> io::Result<()>;
    /// Wait up to 30 s for a service to publish a runtime file whose token is not `stale`.
    fn wait_published(&self, layout: &Layout, stale: Option<&str>) -> bool;
}

/// The real processes of this user.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemProcessControl;

impl ProcessControl for SystemProcessControl {
    fn quit(&self, layout: &Layout) -> bool {
        let lease = layout.home().join(LEASE_FILE_NAME);
        if lease_free(&lease) {
            return true;
        }
        if let Ok((mut stream, Reply::Accepted { .. })) = ghostlight_relay::client::connect(
            Stream::Control {
                op: ControlOp::Quit,
            },
            ghostlight_relay::client::REPLY_DEADLINE,
        ) {
            // The answer only says the service heard; the lease says when it has gone.
            let _ = stream.set_read_timeout(Some(ghostlight_relay::client::REPLY_DEADLINE));
            let _ = ghostlight_relay::frame::read_frame_raw(&mut stream);
        }
        let deadline = Instant::now() + QUIT_WAIT;
        while Instant::now() < deadline {
            if lease_free(&lease) {
                return true;
            }
            std::thread::sleep(POLL);
        }
        lease_free(&lease)
    }

    fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()> {
        migration::stop_legacy_images(&other_than_running_installer(images))
    }

    fn is_running(&self, images: &[PathBuf]) -> bool {
        migration::any_running_from(&other_than_running_installer(images))
    }

    fn start_serve(&self, layout: &Layout) -> io::Result<()> {
        ghostlight_relay::spawn::spawn_serve(&layout.service_executable(), layout.home()).map(drop)
    }

    fn wait_published(&self, layout: &Layout, stale: Option<&str>) -> bool {
        let deadline = Instant::now() + PUBLISH_WAIT;
        loop {
            if fresh_runtime_token(layout, stale).is_some() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(PUBLISH_POLL);
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

/// Whether nothing holds the service lease at `path`. Taking it for a moment is the test; it is
/// released at once.
fn lease_free(path: &Path) -> bool {
    matches!(
        ghostlight_relay::server::Lease::try_acquire(path),
        Ok(Some(_))
    )
}

/// The token of the runtime file in `layout`'s home, when there is one.
fn runtime_token(layout: &Layout) -> Option<String> {
    RuntimeFile::read(&layout.home().join(RUNTIME_FILE_NAME))
        .ok()
        .flatten()
        .map(|runtime| runtime.token)
}

/// The runtime token in `layout`'s home when it is not `stale`.
fn fresh_runtime_token(layout: &Layout, stale: Option<&str>) -> Option<String> {
    runtime_token(layout).filter(|token| Some(token.as_str()) != stale)
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

/// What happens to the development marker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerChange {
    /// Write the marker naming this development executable.
    Write(PathBuf),
    /// Remove the marker.
    Remove,
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
    /// Service images placed by rename-swap.
    pub swaps: Vec<Swap>,
    /// Connectors replaced because the relay revision changes.
    pub connector_replacements: Vec<Swap>,
    /// The development marker change.
    pub marker: Option<MarkerChange>,
    /// Whether the running service is replaced by a new `serve`.
    pub restart: bool,
    /// The record written once the new service publishes.
    pub record: Option<InstallRecord>,
    /// Images stopped: the old installation's, and the running service's if it does not quit.
    pub processes_to_stop: Vec<PathBuf>,
    /// Registrations the reconcile will write.
    pub registrations_to_rewrite: Vec<PendingRegistration>,
}

/// What an install did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    /// The plan that was carried out.
    pub plan: InstallPlan,
    /// Whether the new service published; `None` when no service was started.
    pub service_started: Option<bool>,
    /// What the reconcile changed; `None` when nothing was installed.
    pub registrations: Option<ReconcileReport>,
    /// What became of the files the 1.x Flatpak route left in the home.
    pub flatpak: Vec<FlatpakStep>,
}

/// Why `--restore` would not run.
#[derive(Debug, Error)]
pub enum RestoreRefused {
    /// Nothing records a release in `bin`.
    #[error("No release is recorded in {0}, so there is nothing to restore")]
    NoRelease(PathBuf),
    /// The restore was not run by the release service in `bin`.
    #[error("Run the restore with the installed service: {0} deployment install --restore")]
    NotInstalledService(PathBuf),
    /// The connectors in `bin` speak another relay revision than the service there.
    #[error(
        "The connectors in {bin} speak relay revision {connectors} and the service there speaks \
         {service}, so the installed service could not serve them. Install a release first"
    )]
    RelayMismatch {
        /// The `bin` directory.
        bin: PathBuf,
        /// Relay revision recorded for the connectors.
        connectors: String,
        /// Relay revision of the service.
        service: String,
    },
}

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
            if request.mode != Mode::Restore
                && record::sha256_file(&service).ok().as_deref() != Some(&build.image_sha256)
            {
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

/// The new service did not publish in time.
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
    let payload_record = context.build.record();
    let mut plan = InstallPlan {
        mode,
        version: context.build.version.clone(),
        migration,
        unchanged: false,
        swaps: Vec::new(),
        connector_replacements: Vec::new(),
        marker: None,
        restart: true,
        record: None,
        processes_to_stop: Vec::new(),
        registrations_to_rewrite: Vec::new(),
    };
    let relay_changes = installed.as_ref().map(|record| &record.relay_revision)
        != Some(&context.build.relay_revision);
    match mode {
        Mode::Release => {
            validate_payload(&context.payload, &layout.bin())?;
            let image = installed_image(layout)?;
            match record::decide(installed.as_ref(), &payload_record, image.as_deref()) {
                Standing::Newer => {
                    let installed = installed.as_ref().expect("a newer standing has a record");
                    return Err(PayloadRefused::new(installed, &payload_record).into());
                }
                Standing::Same if plan.migration.is_empty() => {
                    plan.unchanged = true;
                    plan.restart = false;
                    return Ok(plan);
                }
                Standing::Same => {}
                Standing::Absent | Standing::Older | Standing::Damaged => {
                    plan.swaps.push(service_swap(context, layout.bin()));
                    if relay_changes {
                        plan.connector_replacements = connector_swaps(context, &layout.bin());
                    }
                    plan.record = Some(payload_record);
                }
            }
        }
        Mode::Development => {
            validate_payload(&context.payload, &layout.dev())?;
            plan.swaps.push(service_swap(context, layout.dev()));
            plan.marker = Some(MarkerChange::Write(layout.development_executable()));
            if relay_changes {
                // Connectors start bin/ghostlight serve, which forwards to the development
                // build, so bin must speak the payload's relay revision.
                plan.swaps.push(service_swap(context, layout.bin()));
                plan.connector_replacements = connector_swaps(context, &layout.bin());
                plan.record = Some(payload_record);
            }
        }
        Mode::Restore => {
            check_restore(context, installed.as_ref())?;
            plan.marker = Some(MarkerChange::Remove);
        }
    }
    plan.processes_to_stop = plan.migration.images_to_stop.clone();
    for image in running_images(layout) {
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
pub fn execute(context: &Context<'_>, plan: InstallPlan) -> anyhow::Result<Outcome> {
    let layout = &context.layout;
    if plan.unchanged {
        return Ok(Outcome {
            plan,
            service_started: None,
            registrations: None,
            flatpak: Vec::new(),
        });
    }
    let migrating = !plan.migration.is_empty();
    debug_assert!(
        plan.restart || !migrating,
        "a migration is finished only by the restart"
    );
    let mut copied = Vec::new();
    let mut flatpak = Vec::new();
    if migrating {
        migration::prepare(&plan.migration)?;
        copied = migration::copy_state(&plan.migration)?;
    }
    for placed in plan.swaps.iter().chain(&plan.connector_replacements) {
        swap::rename_swap(&placed.directory, &placed.name, &placed.source)?;
    }
    match &plan.marker {
        Some(MarkerChange::Write(executable)) => DevelopmentMarker {
            executable: executable.clone(),
        }
        .write(&layout.development_marker())?,
        Some(MarkerChange::Remove) => DevelopmentMarker::remove(&layout.development_marker())?,
        None => {}
    }
    let registrations = context
        .registrations
        .reconcile(layout, &context.build.version);
    stop_legacy(context.control, &legacy_images(&plan))?;
    let service_started = if plan.restart {
        let stale = runtime_token(layout);
        if !context.control.quit(layout) {
            context.control.stop_image(&running_images(layout))?;
        }
        if migrating {
            migration::retire_state(&copied)?;
            migration::archive_audit(&plan.migration)?;
            migration::finish(&plan.migration)?;
            flatpak = migration::retire_flatpak_route(&plan.migration);
        }
        Some(
            context.control.start_serve(layout).is_ok()
                && context.control.wait_published(layout, stale.as_deref()),
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
    })
}

/// The 1.x images to stop once this install has placed its own executables.
///
/// A 1.x installation may have served from `bin` itself. The swap moved those images aside, so
/// the programs still running them are found under their `.old-<n>` names; the executables at
/// the `bin` paths are now this install's and must not be stopped.
fn legacy_images(plan: &InstallPlan) -> Vec<PathBuf> {
    let placed: Vec<&Swap> = plan
        .swaps
        .iter()
        .chain(&plan.connector_replacements)
        .collect();
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

/// The swap placing the payload's service in `directory`.
fn service_swap(context: &Context<'_>, directory: PathBuf) -> Swap {
    Swap {
        directory,
        name: SERVICE_EXECUTABLE_NAME.to_owned(),
        source: context.payload.join(SERVICE_EXECUTABLE_NAME),
    }
}

/// The swaps placing the payload's connectors in `bin`.
fn connector_swaps(context: &Context<'_>, bin: &Path) -> Vec<Swap> {
    CONNECTORS
        .iter()
        .map(|name| Swap {
            directory: bin.to_owned(),
            name: (*name).to_owned(),
            source: context.payload.join(name),
        })
        .collect()
}

/// Every image a running service of this home may have been started from.
fn running_images(layout: &Layout) -> Vec<PathBuf> {
    let mut images = swap::images(&layout.bin(), SERVICE_EXECUTABLE_NAME);
    images.extend(swap::images(&layout.dev(), SERVICE_EXECUTABLE_NAME));
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

/// Refuse a restore unless the installed service runs it and its connectors match it.
fn check_restore(
    context: &Context<'_>,
    installed: Option<&InstallRecord>,
) -> Result<(), RestoreRefused> {
    let layout = &context.layout;
    let installed = installed.ok_or_else(|| RestoreRefused::NoRelease(layout.bin()))?;
    let image = record::sha256_file(&layout.service_executable()).ok();
    if image.as_deref() != Some(context.build.image_sha256.as_str()) {
        return Err(RestoreRefused::NotInstalledService(
            layout.service_executable(),
        ));
    }
    if installed.relay_revision != context.build.relay_revision {
        return Err(RestoreRefused::RelayMismatch {
            bin: layout.bin(),
            connectors: installed.relay_revision.clone(),
            service: context.build.relay_revision.clone(),
        });
    }
    Ok(())
}

/// Require the three executables in `payload`, which must not be `destination` itself.
fn validate_payload(payload: &Path, destination: &Path) -> Result<(), PayloadError> {
    for name in std::iter::once(SERVICE_EXECUTABLE_NAME).chain(CONNECTORS) {
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
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
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

    use super::super::migration::{
        FlatpakLeftovers, FlatpakPart, FlatpakResult, LegacyInstallation, MigrationError,
        AUDIT_FILE_NAME, HARNESS_LOCATIONS_NAME, LEGACY_DEPLOY_LOCK_NAME, LEGACY_RECORD_NAME,
        LEGACY_STARTUP_NAME, USER_POLICY_FILE_NAME,
    };
    use super::super::native_host::NativeHostRegistry;
    use super::super::scratch::Scratch;
    use super::super::HarnessRegistry;
    use super::*;

    /// Version of the payload under test.
    const VERSION: &str = "2.1.0";

    /// Relay revision of the payload under test.
    const RELAY: &str = "3";

    /// What the fake process control was asked to do.
    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Call {
        Quit,
        Stop(Vec<PathBuf>),
        Running,
        Start,
        Wait,
    }

    /// What the disk showed at the moment of one request.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Sighting {
        /// A native-host manifest already names the installed browser connector.
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
        publishes: bool,
        /// How many looks for a respawned 1.x program find one.
        respawns: Cell<u32>,
        /// The manifest, the connector it must name and the 1.x record to look at on every
        /// request.
        witness: Option<(PathBuf, PathBuf, PathBuf)>,
        /// The 1.x user policy and its 2.0 place, to look at on every request.
        policy_witness: Option<(PathBuf, PathBuf)>,
        /// Files a 1.x program writes while it is being stopped.
        writes_on_stop: Vec<(PathBuf, String)>,
        /// How many of the next requests to stop an image fail.
        failing_stops: Cell<u32>,
        calls: RefCell<Vec<Call>>,
        sightings: RefCell<Vec<Sighting>>,
    }

    impl FakeControl {
        fn new() -> Self {
            Self {
                quits: true,
                publishes: true,
                respawns: Cell::new(0),
                witness: None,
                policy_witness: None,
                writes_on_stop: Vec::new(),
                failing_stops: Cell::new(0),
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

        fn start_serve(&self, _layout: &Layout) -> io::Result<()> {
            self.note(Call::Start);
            Ok(())
        }

        fn wait_published(&self, _layout: &Layout, _stale: Option<&str>) -> bool {
            self.note(Call::Wait);
            self.publishes
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
                NativeHostRegistry::isolated(&hosts, &layout.browser_connector()),
                HarnessRegistry::isolated(
                    &root.join("clients"),
                    &layout.mcp_connector(),
                    &layout.state(),
                ),
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
            for name in CONNECTORS {
                fs::write(payload.join(name), format!("{name} {RELAY}")).unwrap();
            }
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
            for name in std::iter::once(SERVICE_EXECUTABLE_NAME).chain(CONNECTORS) {
                fs::write(serving.join(name), format!("1.x {name}")).unwrap();
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
            let manifest =
                NativeHostRegistry::isolated(&self.hosts, &self.layout.browser_connector())
                    .isolated_manifest("chrome");
            let mut control = FakeControl::new();
            control.witness = Some((
                manifest,
                self.layout.browser_connector(),
                self.legacy_record(),
            ));
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

        /// Whether the manifest the registrations wrote names the installed browser connector.
        fn manifest_names_bin_connector(&self) -> bool {
            let manifest =
                NativeHostRegistry::isolated(&self.hosts, &self.layout.browser_connector())
                    .isolated_manifest("chrome");
            let value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
            fs::canonicalize(value["path"].as_str().unwrap()).unwrap()
                == fs::canonicalize(self.layout.browser_connector()).unwrap()
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
    fn a_fresh_install_places_everything_and_records_it_once_the_service_publishes() {
        let fixture = Fixture::new();
        let control = FakeControl::new();
        let outcome = fixture.run(Mode::Release, VERSION, &control).unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
        for name in CONNECTORS {
            assert_eq!(
                fixture.read(fixture.layout.bin().join(name)),
                format!("{name} {RELAY}")
            );
        }
        assert_eq!(
            fixture.record(),
            Some(InstallRecord {
                service_version: VERSION.to_owned(),
                relay_revision: RELAY.to_owned(),
                service_sha256: fixture.build(VERSION).image_sha256,
            })
        );
        assert_eq!(control.calls(), [Call::Quit, Call::Start, Call::Wait]);
        assert!(outcome.registrations.is_some());
    }

    #[test]
    fn the_same_version_is_left_alone() {
        let fixture = Fixture::new();
        fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        let before = fixture.snapshot();
        let control = FakeControl::new();
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
    fn connectors_stay_when_the_relay_revision_matches() {
        let fixture = Fixture::new();
        fixture
            .run(Mode::Release, VERSION, &FakeControl::new())
            .unwrap();
        Fixture::write_payload(&fixture.payload, "next service");
        for name in CONNECTORS {
            fs::write(fixture.payload.join(name), "rebuilt connector").unwrap();
        }
        let outcome = fixture
            .run(Mode::Release, "2.2.0", &FakeControl::new())
            .unwrap();
        assert!(outcome.plan.connector_replacements.is_empty());
        assert_eq!(
            fixture.read(fixture.layout.service_executable()),
            "next service"
        );
        for name in CONNECTORS {
            assert_eq!(
                fixture.read(fixture.layout.bin().join(name)),
                format!("{name} {RELAY}")
            );
        }
        assert_eq!(fixture.record().unwrap().service_version, "2.2.0");
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

    #[test]
    fn development_installs_dev_the_marker_and_bin_at_the_break() {
        let fixture = Fixture::new();
        let outcome = fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert_eq!(
            fixture.read(fixture.layout.development_executable()),
            "service"
        );
        assert_eq!(
            DevelopmentMarker::read(&fixture.layout.development_marker()).unwrap(),
            Some(DevelopmentMarker {
                executable: fixture.layout.development_executable(),
            })
        );
        assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
        for name in CONNECTORS {
            assert!(fixture.layout.bin().join(name).is_file());
        }
        assert_eq!(fixture.record().unwrap().relay_revision, RELAY);

        // With bin on the same relay revision, the next development build leaves bin alone.
        Fixture::write_payload(&fixture.payload, "dev service");
        let outcome = fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        assert_eq!(outcome.plan.swaps.len(), 1);
        assert_eq!(
            fixture.read(fixture.layout.development_executable()),
            "dev service"
        );
        assert_eq!(fixture.read(fixture.layout.service_executable()), "service");
    }

    #[test]
    fn restore_removes_the_marker_and_refuses_a_relay_mismatch() {
        let fixture = Fixture::new();
        fixture
            .run(Mode::Development, VERSION, &FakeControl::new())
            .unwrap();
        // The bin service runs the restore: its build is the bin image.
        let control = FakeControl::new();
        let outcome = install(
            &fixture.context(fixture.build(VERSION), &control),
            &Request {
                payload: None,
                mode: Mode::Restore,
                plan_only: false,
            },
        )
        .unwrap();
        assert_eq!(outcome.service_started, Some(true));
        assert!(!fixture.layout.development_marker().exists());

        let mut record = fixture.record().unwrap();
        record.relay_revision = "2".to_owned();
        record.write(&fixture.layout.install_record()).unwrap();
        let before = fixture.snapshot();
        let control = FakeControl::new();
        let error = install(
            &fixture.context(fixture.build(VERSION), &control),
            &Request {
                payload: None,
                mode: Mode::Restore,
                plan_only: false,
            },
        )
        .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<RestoreRefused>(),
                Some(RestoreRefused::RelayMismatch { .. })
            ),
            "{error}"
        );
        assert!(control.calls().is_empty());
        assert_eq!(fixture.snapshot(), before);
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
                "path": "/home/user/.ghostlight/bin/ghostlight-browser-connector",
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
        assert!(fixture.manifest_names_bin_connector());
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
            .join(MCP_CONNECTOR_EXECUTABLE_NAME);
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
            fs::canonicalize(fixture.layout.mcp_connector()).unwrap()
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
        let images = vec![
            serving.join(SERVICE_EXECUTABLE_NAME),
            serving.join(MCP_CONNECTOR_EXECUTABLE_NAME),
            serving.join(BROWSER_CONNECTOR_EXECUTABLE_NAME),
        ];
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
        let moved = |name: &str| fixture.layout.bin().join(format!("{name}.old-1"));
        assert!(
            images.contains(&moved(SERVICE_EXECUTABLE_NAME)),
            "{images:?}"
        );
        for name in CONNECTORS {
            assert!(images.contains(&moved(name)), "{images:?}");
        }
        for installed in [
            fixture.layout.service_executable(),
            fixture.layout.mcp_connector(),
            fixture.layout.browser_connector(),
        ] {
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
    fn a_service_that_never_publishes_leaves_the_record_unwritten() {
        let fixture = Fixture::new();
        let mut control = FakeControl::new();
        control.publishes = false;
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
        assert_eq!(outcome.plan.connector_replacements.len(), CONNECTORS.len());
        assert!(serde_json::to_value(&outcome.plan).is_ok());
        assert!(control.calls().is_empty());
        assert_eq!(fixture.snapshot(), before);
    }

    #[test]
    fn a_payload_missing_an_executable_is_refused() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.payload.join(BROWSER_CONNECTOR_EXECUTABLE_NAME)).unwrap();
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
}
