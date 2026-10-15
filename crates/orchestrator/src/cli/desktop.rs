//! Opening Ghostlight, serving, and the commands that need the service.
//!
//! Nothing here serves in the foreground except `serve` itself. A person's start (`open` or a
//! bare launch) and a script's `call` find the service through a control opening, and when none
//! answers they start one detached, as a `person` or a `command` start (ADR-0199 Decision 10),
//! and then wait for it: `open` until the window is shown, `call` until the service is ready.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use ghostlight_relay::protocol::StartReason;

const SERVICE_READY_TIMEOUT: Duration = Duration::from_secs(30);
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);
// Native window construction can outlast service publication without indicating a failure.
const WORKBENCH_ACTIVATION_TIMEOUT: Duration = Duration::from_secs(15);
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
    /// Start the service for `reason` without waiting for it.
    fn start(&self, reason: StartReason) -> io::Result<()>;
}

/// The real starter: `bin/ghostlight serve --reason <reason>`, detached.
#[derive(Clone, Copy, Debug, Default)]
pub struct Detached;

impl Starter for Detached {
    fn start(&self, reason: StartReason) -> io::Result<()> {
        ghostlight_relay::spawn::start_service(reason)
    }
}

/// Run the service and its backgrounded desktop in this process, for a start made for `reason`,
/// until a quit.
///
/// The service never reveals a window on its own. It exits 0 without a word when another service
/// already serves or when Quit holds a browser or recover start back, and it stops by itself,
/// with a nonzero status and a diagnostics event, when it is not accepting openings in time.
pub fn serve(reason: StartReason) -> anyhow::Result<()> {
    settle_serve(crate::desktop::run(reason))
}

/// Turn how a `serve` start ended into its exit: finding the service already running, or being
/// held back by Quit, is success.
pub fn settle_serve(ended: anyhow::Result<()>) -> anyhow::Result<()> {
    match ended {
        Err(error)
            if error.is::<crate::service::AuthorityAlreadyRunning>()
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
        Ok(false) => return finish_activation(wait_for_workbench_activation(), None),
        Err(_) => {}
    }
    starter.start(StartReason::Person)?;
    finish_activation(wait_for_workbench_activation(), None)
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
pub fn finish_activation(
    activation: ActivationState,
    start_error: Option<anyhow::Error>,
) -> anyhow::Result<()> {
    match activation {
        ActivationState::Activated => Ok(()),
        ActivationState::Unavailable => anyhow::bail!(WORKBENCH_OPEN_TIMEOUT_MESSAGE),
        ActivationState::Unreachable => {
            Err(start_error.unwrap_or_else(|| anyhow::anyhow!(WORKBENCH_OPEN_TIMEOUT_MESSAGE)))
        }
    }
}

/// Invoke one tool, or a batch of them, against the local authority.
///
/// A script that runs before anything else has started gets a service rather than an error: the
/// command line starts one as a `command` start.
pub fn run_call() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args()
        .skip(1)
        .skip_while(|argument| argument != "call")
        .skip(1)
        .collect();
    let command = match crate::cli::parse(&arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    reach_service(&Detached)?;
    let mut out = std::io::stdout().lock();
    let code = crate::cli::run(command, &mut out);
    std::process::exit(code);
}

/// Make sure a service answers, starting one as a `command` start through `starter` when none
/// does.
pub fn reach_service(starter: &dyn Starter) -> anyhow::Result<()> {
    if crate::cli::client::readiness().is_err() {
        starter.start(StartReason::Command)?;
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
    use crate::workbench::{
        WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
    };

    #[test]
    fn serve_settles_a_refusal_to_start_as_success_and_any_other_failure_as_failure() {
        assert!(settle_serve(Err(crate::service::AuthorityAlreadyRunning.into())).is_ok());
        assert!(settle_serve(Err(crate::service::QuitLatched.into())).is_ok());
        assert!(settle_serve(Err(anyhow::anyhow!("native desktop construction failed"))).is_err());
        assert!(settle_serve(Ok(())).is_ok());
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
        fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
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
        fn start(&self, reason: StartReason) -> io::Result<()> {
            self.reasons.borrow_mut().push(reason);
            let host = crate::service::ServiceHost::start(&self.home).map_err(io::Error::other)?;
            host.publish_ready().map_err(io::Error::other)?;
            host.workbench.attach_presentation(self.reveals.clone());
            *self.host.borrow_mut() = Some(host);
            Ok(())
        }
    }

    #[test]
    fn open_with_no_service_starts_one_for_a_person_and_shows_it() {
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

    #[test]
    fn a_call_with_no_service_starts_one_as_a_command_start() {
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

    #[test]
    fn open_waits_for_delayed_native_presentation_and_reveals_once() {
        struct DelayedPresentation(AtomicUsize);

        impl WorkbenchPresentationPort for DelayedPresentation {
            fn reveal(&self) -> Result<(), WorkbenchPresentationError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn notify(&self, _: WorkbenchNotification) -> Result<(), WorkbenchPresentationError> {
                Ok(())
            }
        }

        let home = crate::service::test_home::TestHome::new("delayed-presentation");
        let host = crate::service::ServiceHost::start(home.path()).unwrap();
        host.publish_ready().unwrap();
        assert!(!crate::cli::client::activate_workbench().unwrap());
        let timeout_started = std::time::Instant::now();
        let timeout =
            wait_for_workbench_activation_until(timeout_started + Duration::from_millis(100));
        assert_eq!(timeout, ActivationState::Unavailable);
        assert!(timeout_started.elapsed() < Duration::from_secs(1));
        let message = finish_activation(timeout, None).unwrap_err().to_string();
        assert!(message.contains("did not open in time"));
        assert!(message.contains("ghostlight open"));
        assert!(!message.contains("stop"));
        let presentation = Arc::new(DelayedPresentation(AtomicUsize::new(0)));
        let delayed_presentation = presentation.clone();
        let workbench = host.workbench.clone();
        let ready = std::thread::spawn(move || {
            // A cold WebView can become ready after the old one-second activation budget.
            std::thread::sleep(Duration::from_secs(2));
            workbench.attach_presentation(delayed_presentation);
        });
        let result = wait_for_workbench_activation();
        ready.join().unwrap();
        drop(host);
        drop(home);
        assert_eq!(result, ActivationState::Activated);
        assert_eq!(presentation.0.load(Ordering::SeqCst), 1);
    }
}
