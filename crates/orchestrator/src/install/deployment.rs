//! `ghostlight deployment install`: the one way executables reach the Ghostlight home.
//!
//! A package, the npm launcher, the get scripts and the development loop all run a `ghostlight`
//! with this command (ADR-0197 Decisions 13 to 15). Under the install lock it:
//!
//! 1. carries out the 2.0 break migration when a 1.x layout is found, before anything is swapped;
//! 2. reads `bin/install.json` and decides what to replace: a release refuses an older payload,
//!    does nothing for the same intact version, rename-swaps the service image into `bin`, and
//!    replaces the connectors only when the relay revision changes; `--development` swaps the
//!    service into `dev` and writes the development marker (installing into `bin` as well when
//!    `bin` speaks another relay revision); `--restore` removes the marker;
//! 3. asks the running service to quit, stops that exact image if it has not exited, starts
//!    `bin/ghostlight serve` and writes `install.json` only once the new service publishes;
//! 4. reconciles every registration to name `bin`.
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
use super::migration::{self, Legacy, LegacyRoots, MigrationPlan};
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

/// Pause between attempts to take the install lock and between checks of a quitting service.
const POLL: Duration = Duration::from_millis(25);

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
        let own = env::current_exe().map(|path| canonical_path(&path)).ok();
        let others: Vec<PathBuf> = images
            .iter()
            .filter(|image| {
                own.as_ref()
                    .is_none_or(|own| !same_path(&canonical_path(image), own))
            })
            .cloned()
            .collect();
        migration::stop_legacy_images(&others)
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
    /// The 2.0 break migration, carried out first.
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
/// The migration runs before any swap, so a failed move leaves every executable as it was.
pub fn execute(context: &Context<'_>, plan: InstallPlan) -> anyhow::Result<Outcome> {
    let layout = &context.layout;
    if !plan.migration.is_empty() {
        migration::execute(&plan.migration)?;
    }
    if plan.unchanged {
        return Ok(Outcome {
            plan,
            service_started: None,
            registrations: None,
        });
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
    let service_started = plan.restart.then(|| {
        let stale = runtime_token(layout);
        if !context.control.quit(layout) {
            context.control.stop_image(&running_images(layout))?;
        }
        Ok::<bool, anyhow::Error>(
            context.control.start_serve(layout).is_ok()
                && context.control.wait_published(layout, stale.as_deref()),
        )
    });
    let service_started = service_started.transpose()?;
    if service_started == Some(true) {
        if let Some(record) = &plan.record {
            record.write(&layout.install_record())?;
        }
    }
    let registrations = context
        .registrations
        .reconcile(layout, &context.build.version);
    Ok(Outcome {
        plan,
        service_started,
        registrations: Some(registrations),
    })
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
    use std::cell::RefCell;

    use super::super::migration::{LegacyInstallation, USER_POLICY_FILE_NAME};
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
        Start,
        Wait,
    }

    /// Process control that records every request and starts nothing.
    struct FakeControl {
        quits: bool,
        publishes: bool,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeControl {
        fn new() -> Self {
            Self {
                quits: true,
                publishes: true,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl ProcessControl for FakeControl {
        fn quit(&self, _layout: &Layout) -> bool {
            self.calls.borrow_mut().push(Call::Quit);
            self.quits
        }

        fn stop_image(&self, images: &[PathBuf]) -> anyhow::Result<()> {
            self.calls.borrow_mut().push(Call::Stop(images.to_vec()));
            Ok(())
        }

        fn start_serve(&self, _layout: &Layout) -> io::Result<()> {
            self.calls.borrow_mut().push(Call::Start);
            Ok(())
        }

        fn wait_published(&self, _layout: &Layout, _stale: Option<&str>) -> bool {
            self.calls.borrow_mut().push(Call::Wait);
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
    }

    impl Fixture {
        fn new() -> Self {
            let scratch = Scratch::new();
            let root = fs::canonicalize(scratch.path()).unwrap();
            let layout = Layout::new(root.join("home"));
            let payload = root.join("payload");
            fs::create_dir_all(&payload).unwrap();
            Self::write_payload(&payload, "service");
            let registrations = Registrations::isolated(
                NativeHostRegistry::isolated(&root.join("hosts"), &layout.browser_connector()),
                HarnessRegistry::isolated(&root.join("clients"), &layout.mcp_connector()),
            );
            Self {
                scratch,
                layout,
                payload,
                registrations,
                legacy_policy: None,
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

        fn legacy(&self) -> Legacy {
            Legacy {
                installation: self
                    .legacy_policy
                    .as_ref()
                    .map(|policy| LegacyInstallation {
                        policy_directory: Some(policy.clone()),
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
    fn a_legacy_layout_migrates_before_anything_is_swapped() {
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
