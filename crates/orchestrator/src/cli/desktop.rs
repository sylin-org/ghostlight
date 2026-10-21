//! Opening Ghostlight, serving, and the commands that need the service.
//!
//! Nothing here serves in the foreground except `serve` itself. A person's start (`open` or a
//! bare launch) and a script's `call` find the service through a control opening, and when none
//! answers they start one detached, as a `person` or a `command` start (ADR-0199 Decision 10),
//! and then wait for it: `open` until the window is shown, `call` until the service is ready.
//! A service that reaches no display, such as one an agent started over SSH, cannot bring its
//! window up, so `open`, which a person runs on their own desktop, starts the shell itself there.
//!
//! `serve` runs the service in this process until a quit. It has no window of its own: it binds
//! its endpoint as soon as it has started (ADR-0199 Decision 5), and the shell that shows the
//! window and the tray is started by presence (`service::presence`). A start first relaunches
//! itself once, so the process that serves is never a living child of its starter (ADR-0199
//! amendment of 2026-10-21).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::os::session;
use ghostlight_relay::protocol::StartReason;

use crate::cli::parse::{ServeStart, RELAUNCHED_OPTION};
use crate::install::layout::Layout;
use crate::install::record;
use crate::language::environment::SESSION_LISTENER_FAILED;
use crate::language::workbench::WindowReason;
use crate::service::presence::{self, LaunchRefusal};
use crate::service::{Quit, ServiceHost, StartTerms, World};

const SERVICE_READY_TIMEOUT: Duration = Duration::from_secs(30);
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);
/// How long `open` waits for the window after it asked the service to show it. The service starts
/// the shell when none is attached, which can take a few seconds on a cold start; the service
/// answers within its own, shorter wait.
const WORKBENCH_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(15);
/// The status a `serve` start exits with when it never bound its endpoint in time.
pub const READY_STALLED_EXIT: i32 = 3;
/// The command word that runs one browser tool from the command line.
const CALL_COMMAND: &str = "call";
/// The usage error for a `call` argument that is not valid UTF-8.
const CALL_ARGUMENTS_NOT_UTF8: &str = "call arguments must be valid UTF-8";
const WORKBENCH_OPEN_TIMEOUT_MESSAGE: &str =
    "The Ghostlight workbench did not open in time. Run 'ghostlight open' to try again.";

/// Activation outcome when querying an active service instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationState {
    /// The workbench was successfully activated or revealed.
    Activated,
    /// Service responded but workbench presentation did not become available in time.
    Unavailable,
    /// Service endpoint could not be contacted.
    Unreachable,
}

/// How the command line starts the service when none answers.
pub trait Starter {
    /// Whether the calling process runs with an elevated token. A service started from one
    /// refuses to run, so nothing is started on its behalf.
    fn elevated(&self) -> io::Result<bool>;

    /// Start the service for `reason` without waiting for it.
    fn start(&self, reason: StartReason) -> io::Result<()>;
}

/// Start the service for `reason` through `starter`, unless this process is elevated, in which
/// case nothing is started and the person is told why at once.
fn start_service(starter: &dyn Starter, reason: StartReason) -> anyhow::Result<()> {
    crate::service::refuse_elevated_token(|| starter.elevated())?;
    starter.start(reason)?;
    Ok(())
}

/// The real starter: `bin/ghostlight serve --reason <reason>`, detached.
#[derive(Clone, Copy, Debug, Default)]
pub struct Detached;

impl Starter for Detached {
    fn elevated(&self) -> io::Result<bool> {
        ghostlight_win_peer::is_elevated()
    }

    fn start(&self, reason: StartReason) -> io::Result<()> {
        ghostlight_relay::spawn::start_service(reason)
    }
}

/// Run the service for `start`, until a quit.
///
/// A start a starter made relaunches itself once (ADR-0199 amendment of 2026-10-21). It refuses
/// an elevated token, so that refusal still reaches whoever ran `serve`, then starts this image
/// again with [`RELAUNCHED_OPTION`] and exits 0. The relaunched copy serves. The service is
/// therefore never a living child of its starter: an agent host that ends its MCP server by
/// killing the server's process tree, as the Copilot CLI does, cannot reach it, and on Linux it
/// belongs to init.
///
/// The service shows nothing itself. It exits 0 without a word when another service already
/// serves or when Quit holds a browser or recover start back, and it stops by itself, with a
/// nonzero status and a diagnostics event, when it is not accepting openings in time.
pub fn serve(start: ServeStart) -> anyhow::Result<()> {
    let home = ghostlight_relay::home::home()?;
    let ended = if start.relaunched {
        serve_in(
            &home,
            start.reason,
            listen_for_session_end,
            |home, reason| {
                ServiceHost::start(home, StartTerms::for_reason(reason), World::system())
            },
        )
    } else {
        relaunch(
            &home,
            start.reason,
            ghostlight_win_peer::is_elevated,
            spawn_relaunched,
        )
    };
    record_start_failure(&home, &ended);
    settle_serve(ended)
}

/// Refuse an elevated token as a serving start does, then have `spawn` start the copy that
/// serves for `reason` in `home`, and return without waiting for it.
fn relaunch(
    home: &Path,
    reason: StartReason,
    elevated: impl FnOnce() -> io::Result<bool>,
    spawn: impl FnOnce(&Path, StartReason) -> io::Result<()>,
) -> anyhow::Result<()> {
    crate::service::refuse_elevated_token(elevated)?;
    spawn(home, reason).map_err(|error| anyhow::anyhow!("relaunch the service: {error}"))
}

/// Start this image as `serve --reason <reason>` with [`RELAUNCHED_OPTION`], detached, in
/// `home`.
fn spawn_relaunched(home: &Path, reason: StartReason) -> io::Result<()> {
    let image = std::env::current_exe()?;
    let [serve, option, word] = ghostlight_relay::spawn::serve_arguments(reason);
    let arguments = [serve, option, word, OsStr::new(RELAUNCHED_OPTION)];
    ghostlight_relay::os::spawn_detached(&image, &arguments, home).map(drop)
}

/// Leave the reason a start failed in the home's start-failure record, where its starter reads
/// it: a started service's output goes nowhere. Meeting the running service and being held back
/// by Quit are not failures and leave nothing.
fn record_start_failure(home: &Path, ended: &anyhow::Result<()>) {
    let Err(error) = ended else {
        return;
    };
    if error.is::<crate::service::ServiceAlreadyRunning>()
        || error.is::<crate::service::QuitLatched>()
    {
        return;
    }
    let reason = format!("{error:#}\n");
    let _ = record::atomic_replace(&Layout::new(home).start_failure(), reason.as_bytes());
}

/// Listen for the end of the session from this process's start.
///
/// On the final signal the fallback exit is armed first. A service that exists then says goodbye
/// with `session_end` on every stream and leaves through its quit; a start that has no service
/// yet holds nothing and leaves at once.
fn listen_for_session_end(ending: Arc<OnceLock<Quit>>) -> io::Result<session::SessionListener> {
    session::listen(move |event| session::stop_on_final(event, || end_session(ending.get())))
}

/// End this process because the session is ending: through the quit path when the service
/// exists, at once when it does not.
fn end_session(quit: Option<&Quit>) {
    match quit {
        Some(quit) => quit.session_end(),
        None => std::process::exit(0),
    }
}

/// Serve from the Ghostlight home `home` until a quit, with `listen` hearing the end of the
/// session and `start` making the service.
///
/// In order: the session listener is installed, so a start still waiting for the service lock
/// leaves at once; the service starts under its lock; the quit signal is installed; the ready
/// limit is armed; and only then the endpoint is bound, so a quit that arrives the moment the
/// first peer can connect is heard. The process then waits for the quit signal.
fn serve_in<L>(
    home: &Path,
    reason: StartReason,
    listen: impl FnOnce(Arc<OnceLock<Quit>>) -> io::Result<L>,
    start: impl FnOnce(&Path, StartReason) -> anyhow::Result<ServiceHost>,
) -> anyhow::Result<()> {
    let ending = Arc::new(OnceLock::<Quit>::new());
    let _session_listener = listen(Arc::clone(&ending))
        .map_err(|error| anyhow::anyhow!("{SESSION_LISTENER_FAILED} {error}"))?;
    let host = start(home, reason)?;
    let _ = ending.set(host.quit_handle());
    let (quit_sender, quit_received) = mpsc::channel::<()>();
    host.install_quit_signal(Arc::new(move || {
        let _ = quit_sender.send(());
    }));
    host.expect_ready_by(host.ready_deadline(), || {
        std::process::exit(READY_STALLED_EXIT)
    })?;
    host.publish_ready()?;
    // This start answers now, so a reason an earlier one recorded no longer applies.
    let _ = fs::remove_file(Layout::new(home).start_failure());
    eprintln!("Ghostlight {} ready", env!("CARGO_PKG_VERSION"));
    // A quit signal that is never made, because the service was dropped, also ends the wait.
    let _ = quit_received.recv();
    Ok(())
}

/// Turn how a `serve` start ended into its exit: finding the service already running, or being
/// held back by Quit, is success.
pub fn settle_serve(ended: anyhow::Result<()>) -> anyhow::Result<()> {
    match ended {
        Err(error)
            if error.is::<crate::service::ServiceAlreadyRunning>()
                || error.is::<crate::service::QuitLatched>() =>
        {
            Ok(())
        }
        other => other,
    }
}

/// Bring Ghostlight's window forward, starting the service first when none answers.
///
/// A bare launch and `open` are a person's start: the service is started as a `person` start,
/// which a Quit does not hold back.
pub fn open_desktop() -> anyhow::Result<()> {
    open_with(&Detached)
}

/// Like [`open_desktop`], starting the service through `starter`.
pub fn open_with(starter: &dyn Starter) -> anyhow::Result<()> {
    match crate::cli::client::activate_workbench() {
        Ok(true) => return Ok(()),
        Ok(false) => {
            bring_window_here()?;
            return finish_activation(wait_for_workbench_activation());
        }
        Err(_) => {}
    }
    start_service(starter, StartReason::Person)?;
    finish_activation(wait_for_workbench_activation())
}

/// What `open` does when the running service brought no window forward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoWindow {
    /// Wait for the window the service is bringing up.
    Wait,
    /// The service reaches no display and this process does: start the shell here, as the
    /// Applications entry does. It attaches to that service and opens its window.
    StartShellHere,
    /// No window can come from here either: say why at once.
    Refuse(&'static str),
}

/// Decide [`NoWindow`] from the reason the service gives for having no window and whether a
/// window this process starts could be seen.
fn after_no_window(reason: Option<WindowReason>, display_here: bool) -> NoWindow {
    match reason {
        Some(WindowReason::NoDisplay) if display_here => NoWindow::StartShellHere,
        Some(reason @ WindowReason::NoDisplay) => {
            reason.sentence().map_or(NoWindow::Wait, NoWindow::Refuse)
        }
        _ => NoWindow::Wait,
    }
}

/// When the service has no window because it reaches no display, start the shell from this
/// process, or say why no window can come. Otherwise the service is bringing one up.
fn bring_window_here() -> anyhow::Result<()> {
    let reason = crate::cli::client::status()
        .ok()
        .and_then(|status| status.window);
    match after_no_window(reason, presence::display_here().is_ok()) {
        NoWindow::Wait => Ok(()),
        NoWindow::Refuse(sentence) => anyhow::bail!(sentence),
        NoWindow::StartShellHere => {
            let home = ghostlight_relay::home::home()?;
            match presence::start_shell(&Layout::new(&home), false) {
                // A shell the system would not start leaves the wait to say the window did not
                // open, as before.
                Ok(()) | Err(LaunchRefusal::Failed) => Ok(()),
                Err(LaunchRefusal::NoShellFile) => {
                    anyhow::bail!(WindowReason::NoShellFile.sentence().unwrap_or_default())
                }
            }
        }
    }
}

/// Wait until the workbench presentation attaches and activates or times out.
pub fn wait_for_workbench_activation() -> ActivationState {
    wait_for_workbench_activation_until(Instant::now() + WORKBENCH_ACTIVATION_TIMEOUT)
}

/// Wait until the workbench presentation activates or the given deadline expires.
pub fn wait_for_workbench_activation_until(deadline: Instant) -> ActivationState {
    let mut presentation_seen = false;
    loop {
        match crate::cli::client::activate_workbench() {
            Ok(true) => return ActivationState::Activated,
            Ok(false) => presentation_seen = true,
            Err(_) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(ACTIVATION_RETRY_DELAY.min(remaining));
    }
    if presentation_seen {
        ActivationState::Unavailable
    } else {
        ActivationState::Unreachable
    }
}

/// Translate activation state into a final result or user-facing error message.
pub fn finish_activation(activation: ActivationState) -> anyhow::Result<()> {
    match activation {
        ActivationState::Activated => Ok(()),
        ActivationState::Unavailable | ActivationState::Unreachable => {
            anyhow::bail!(WORKBENCH_OPEN_TIMEOUT_MESSAGE)
        }
    }
}

/// Invoke one tool, or a batch of them, against the local authority.
///
/// A script that runs before anything else has started gets a service rather than an error: the
/// command line starts one as a `command` start.
pub fn run_call() -> anyhow::Result<()> {
    let command = match call_arguments(std::env::args_os())
        .and_then(|arguments| crate::cli::parse(&arguments).map_err(|error| error.to_string()))
    {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(super::EXIT_USAGE);
        }
    };
    reach_service(&Detached)?;
    let mut out = std::io::stdout().lock();
    let code = crate::cli::run(command, &mut out);
    std::process::exit(code);
}

/// The arguments after `call` in the process's `arguments`, its own name first. One that is not
/// valid UTF-8 is a usage error, never a panic.
fn call_arguments(arguments: impl IntoIterator<Item = OsString>) -> Result<Vec<String>, String> {
    arguments
        .into_iter()
        .skip(1)
        .skip_while(|argument| argument != CALL_COMMAND)
        .skip(1)
        .map(|argument| {
            argument
                .into_string()
                .map_err(|_| CALL_ARGUMENTS_NOT_UTF8.to_owned())
        })
        .collect()
}

/// Make sure a service answers, starting one as a `command` start through `starter` when none
/// does.
pub fn reach_service(starter: &dyn Starter) -> anyhow::Result<()> {
    if crate::cli::client::readiness().is_err() {
        start_service(starter, StartReason::Command)?;
        wait_for_service()?;
    }
    Ok(())
}

/// Run local policy governance inspection commands.
pub fn run_policy(command: &crate::governance::inspection::Command) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    crate::governance::inspection::run(command, &mut out)
}

/// Wait until a service answers a readiness control opening.
pub fn wait_for_service() -> anyhow::Result<()> {
    let deadline = Instant::now() + SERVICE_READY_TIMEOUT;
    while Instant::now() < deadline {
        if crate::cli::client::readiness().is_ok() {
            return Ok(());
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
    anyhow::bail!("The Ghostlight service did not become ready in time; run ghostlight doctor for its installation details")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;
    use crate::scratch::Scratch;

    use crate::workbench::{
        PageAddress, WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    };

    /// An argument that is not valid UTF-8 on this system.
    fn not_utf8() -> OsString {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xD800])
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xFF])
        }
    }

    /// A starter that starts a real service in the test home and remembers why it was asked.
    struct StartsAService {
        home: std::path::PathBuf,
        reasons: RefCell<Vec<StartReason>>,
        host: RefCell<Option<crate::service::ServiceHost>>,
        reveals: Arc<Reveals>,
    }

    /// Counts the times the workbench was asked to come forward.
    #[derive(Default)]
    struct Reveals(AtomicUsize);

    impl WorkbenchPresentationPort for Reveals {
        fn reveal(&self, _address: Option<PageAddress>) -> Result<(), WorkbenchPresentationError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn notify(&self, _: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
            Ok(())
        }
    }

    impl StartsAService {
        fn new(home: &crate::service::test_home::TestHome) -> Self {
            Self {
                home: home.path().to_path_buf(),
                reasons: RefCell::default(),
                host: RefCell::default(),
                reveals: Arc::default(),
            }
        }
    }

    impl Starter for StartsAService {
        fn elevated(&self) -> io::Result<bool> {
            Ok(false)
        }

        fn start(&self, reason: StartReason) -> io::Result<()> {
            self.reasons.borrow_mut().push(reason);
            let host = crate::service::ServiceHost::start_for_test(&self.home)
                .map_err(io::Error::other)?;
            host.publish_ready().map_err(io::Error::other)?;
            host.workbench.attach_presentation(self.reveals.clone());
            *self.host.borrow_mut() = Some(host);
            Ok(())
        }
    }

    /// A starter whose process is elevated, and which fails the test if asked to start anything.
    struct Elevated(RefCell<usize>);

    impl Starter for Elevated {
        fn elevated(&self) -> io::Result<bool> {
            Ok(true)
        }

        fn start(&self, _: StartReason) -> io::Result<()> {
            *self.0.borrow_mut() += 1;
            Ok(())
        }
    }

    /// Protects the service's start lifecycle and defect bd29366c: a start relaunches itself
    /// for its reason and the copy serves, an elevated start is refused before it relaunches, the
    /// session listener is installed before the service starts and the endpoint binds after, a refusal
    /// to start settles as success and any other failure as failure, and a failed start leaves its
    /// reason for the installer while a service that publishes removes the one an earlier start left.
    #[test]
    fn a_service_start_relaunches_for_its_reason_and_leaves_why_it_ended() {
        {
            let home = Scratch::named("relaunch");
            let mut relaunched = None;
            relaunch(
                home.path(),
                StartReason::Agent,
                || Ok(false),
                |directory, reason| {
                    relaunched = Some((directory.to_path_buf(), reason));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                relaunched,
                Some((home.path().to_path_buf(), StartReason::Agent))
            );
        }
        {
            let home = Scratch::named("relaunch-elevated");
            let mut relaunched = false;
            let refused = relaunch(
                home.path(),
                StartReason::Agent,
                || Ok(true),
                |_, _| {
                    relaunched = true;
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(
                format!("{refused:#}"),
                crate::language::environment::ELEVATED_SERVICE
            );
            assert!(!relaunched, "nothing was started");
        }
        {
            use crate::cli::client::control_at;
            use ghostlight_relay::protocol::{Control, QuitReason};

            let home = crate::service::test_home::TestHome::new("serve-order");
            let order = std::sync::Mutex::new(Vec::<&str>::new());
            let path = home.path().to_path_buf();
            let probe = path.clone();
            let quitter = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(20);
                while Instant::now() < deadline {
                    if control_at(
                        &probe,
                        Control::Quit {
                            reason: QuitReason::Replace,
                        },
                    )
                    .is_ok()
                    {
                        return true;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                false
            });
            let served = serve_in(
                &path,
                StartReason::Agent,
                |_| {
                    order.lock().unwrap().push("listener");
                    assert!(
                        control_at(&path, Control::Readiness).is_err(),
                        "nothing is bound before the listener is installed"
                    );
                    Ok(())
                },
                |home, _| {
                    order.lock().unwrap().push("start");
                    ServiceHost::start_for_test(home)
                },
            );
            assert!(quitter.join().unwrap(), "the endpoint was bound and quit");
            served.unwrap();
            assert_eq!(*order.lock().unwrap(), ["listener", "start"]);
        }
        {
            assert!(settle_serve(Err(crate::service::ServiceAlreadyRunning.into())).is_ok());
            assert!(settle_serve(Err(crate::service::QuitLatched.into())).is_ok());
            assert!(settle_serve(Err(anyhow::anyhow!("the endpoint could not be bound"))).is_err());
            assert!(settle_serve(Ok(())).is_ok());
        }
        {
            let home = crate::service::test_home::TestHome::new("start-failure");
            let record = Layout::new(home.path()).start_failure();
            record_start_failure(
                home.path(),
                &Err(crate::service::ServiceAlreadyRunning.into()),
            );
            record_start_failure(home.path(), &Err(crate::service::QuitLatched.into()));
            record_start_failure(home.path(), &Ok(()));
            assert!(!record.exists());
            let failed = Err(anyhow::anyhow!("path must be shorter than SUN_LEN")
                .context("bind the relay endpoint"));
            record_start_failure(home.path(), &failed);
            assert_eq!(
                fs::read_to_string(&record).unwrap(),
                "bind the relay endpoint: path must be shorter than SUN_LEN\n"
            );
        }
        {
            let home = crate::service::test_home::TestHome::new("start-failure-cleared");
            let path = home.path().to_path_buf();
            let record = Layout::new(&path).start_failure();
            fs::create_dir_all(record.parent().unwrap()).unwrap();
            fs::write(&record, "an earlier reason\n").unwrap();
            // The service answers once its listener runs, which can be before `serve_in` removes the
            // record, so the record is looked at only after `serve_in` returns: it removes the record
            // before it waits for the quit.
            let probe = path.clone();
            let quitter = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(20);
                while Instant::now() < deadline {
                    if crate::cli::client::status_at(&probe).is_ok() {
                        let _ = crate::cli::client::control_at(
                            &probe,
                            ghostlight_relay::protocol::Control::Quit {
                                reason: ghostlight_relay::protocol::QuitReason::Replace,
                            },
                        );
                        return true;
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                false
            });
            serve_in(
                &path,
                StartReason::Command,
                |_| Ok(()),
                |home, _| ServiceHost::start_for_test(home),
            )
            .unwrap();
            assert!(quitter.join().unwrap(), "the service answered");
            assert!(
                !record.exists(),
                "a service that published removed the earlier start's reason"
            );
        }
    }

    /// Protects ACCEPTANCE's open and call journeys: call takes the words after `call` and refuses an
    /// argument that is not UTF-8; open starts the shell where the person is when the service reaches
    /// no display; open with no service starts one for a person and shows it, and call starts one as a
    /// command start; an elevated open or call starts nothing and says why at once; and open waits for
    /// a shell still starting and reveals the window once.
    #[test]
    fn open_and_call_start_a_service_for_the_person_and_reveal_the_window_once() {
        {
            let arguments = ["ghostlight", "call", "browser_tabs", "{}"].map(OsString::from);
            assert_eq!(
                call_arguments(arguments),
                Ok(vec!["browser_tabs".to_owned(), "{}".to_owned()])
            );
        }
        {
            let arguments = vec![
                OsString::from("ghostlight"),
                OsString::from("call"),
                not_utf8(),
            ];
            assert_eq!(
                call_arguments(arguments),
                Err(CALL_ARGUMENTS_NOT_UTF8.to_owned())
            );
        }
        {
            assert_eq!(
                after_no_window(Some(WindowReason::NoDisplay), true),
                NoWindow::StartShellHere
            );
            assert_eq!(
                after_no_window(Some(WindowReason::NoDisplay), false),
                NoWindow::Refuse("No Ghostlight window: no display is reachable from the service.")
            );
            // Any other reason, or none yet, is a window the service is bringing up.
            for reason in [
                None,
                Some(WindowReason::Tray),
                Some(WindowReason::DidNotStay),
                Some(WindowReason::NoShellFile),
            ] {
                assert_eq!(after_no_window(reason, true), NoWindow::Wait);
            }
        }
        {
            let home = crate::service::test_home::TestHome::new("open-starts");
            let starter = StartsAService::new(&home);
            open_with(&starter).unwrap();
            assert_eq!(*starter.reasons.borrow(), [StartReason::Person]);
            assert_eq!(starter.reveals.0.load(Ordering::SeqCst), 1);

            // With a service already answering, open starts nothing and shows it again.
            open_with(&starter).unwrap();
            assert_eq!(*starter.reasons.borrow(), [StartReason::Person]);
            assert_eq!(starter.reveals.0.load(Ordering::SeqCst), 2);
        }
        {
            let home = crate::service::test_home::TestHome::new("call-starts");
            let starter = StartsAService::new(&home);
            reach_service(&starter).unwrap();
            assert_eq!(*starter.reasons.borrow(), [StartReason::Command]);
            reach_service(&starter).unwrap();
            assert_eq!(
                *starter.reasons.borrow(),
                [StartReason::Command],
                "a service that answers is not started again"
            );
        }
        {
            let home = crate::service::test_home::TestHome::new("elevated-start");
            let starter = Elevated(RefCell::new(0));
            let began = Instant::now();
            let opened = open_with(&starter).unwrap_err();
            assert_eq!(
                opened.to_string(),
                crate::language::environment::ELEVATED_SERVICE
            );
            let called = reach_service(&starter).unwrap_err();
            assert_eq!(
                called.to_string(),
                crate::language::environment::ELEVATED_SERVICE
            );
            assert_eq!(*starter.0.borrow(), 0, "no service was started");
            assert!(began.elapsed() < Duration::from_secs(5), "no waiting");
            drop(home);
        }
        {
            /// A shell still starting: it refuses the first reveal and carries out every later one.
            struct DelayedPresentation(AtomicUsize);

            impl WorkbenchPresentationPort for DelayedPresentation {
                fn reveal(
                    &self,
                    _address: Option<PageAddress>,
                ) -> Result<(), WorkbenchPresentationError> {
                    match self.0.fetch_add(1, Ordering::SeqCst) {
                        0 => Err(WorkbenchPresentationError::Unavailable),
                        _ => Ok(()),
                    }
                }

                fn notify(
                    &self,
                    _: WorkbenchNotification,
                ) -> Result<(), WorkbenchPresentationError> {
                    Ok(())
                }
            }

            let home = crate::service::test_home::TestHome::new("delayed-presentation");
            let host = crate::service::ServiceHost::start_for_test(home.path()).unwrap();
            host.publish_ready().unwrap();
            assert!(!crate::cli::client::activate_workbench().unwrap());
            let timeout_started = std::time::Instant::now();
            let timeout =
                wait_for_workbench_activation_until(timeout_started + Duration::from_millis(100));
            assert_eq!(timeout, ActivationState::Unavailable);
            assert!(timeout_started.elapsed() < Duration::from_secs(1));
            let message = finish_activation(timeout).unwrap_err().to_string();
            assert!(message.contains("did not open in time"));
            assert!(message.contains("ghostlight open"));
            assert!(!message.contains("stop"));
            let presentation = Arc::new(DelayedPresentation(AtomicUsize::new(0)));
            host.workbench.attach_presentation(presentation.clone());
            let result = wait_for_workbench_activation();
            drop(host);
            drop(home);
            assert_eq!(result, ActivationState::Activated);
            // One refused while the shell started, then the one reveal: the wait stops there.
            assert_eq!(presentation.0.load(Ordering::SeqCst), 2);
        }
    }
}
