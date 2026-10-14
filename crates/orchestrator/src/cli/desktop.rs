//! Desktop workbench lifecycle, window activation, and local execution runners.
//!
//! This module coordinates process activation, single-instance workbench window
//! presentation, runtime readiness waiting, policy inspection, and local `call` execution.

use std::thread;
use std::time::{Duration, Instant};

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

/// Start the orchestrator desktop application or activate an existing running instance.
pub fn start_or_activate_desktop() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    if desktop_bus_start(std::env::var_os("DBUS_STARTER_BUS_TYPE").as_deref()) {
        // A bus launch requests existence, never Open. In a cold-start race a losing
        // process must exit at the lifetime lease instead of revealing the winner's UI.
        return crate::desktop::run(crate::desktop::Startup::interactive());
    }
    match crate::cli::client::activate_workbench() {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(), None),
        Err(_) => {}
    }
    match crate::desktop::run(crate::desktop::Startup::interactive()) {
        Ok(()) => Ok(()),
        Err(start_error) if start_error.is::<crate::service::AuthorityAlreadyRunning>() => {
            finish_activation(wait_for_workbench_activation(), Some(start_error))
        }
        Err(start_error) => Err(start_error),
    }
}

/// Start the service and its backgrounded desktop for a connector, then run until quit.
///
/// This is the start a connector makes, so it differs from a human start in three ways. It never
/// reveals a window. It exits 0 without a word when another service holds the lease, once it has
/// asked for the lease for a second. And it stops by itself, with a nonzero status and a
/// diagnostics event, when discovery is not published within the discovery window.
pub fn serve() -> anyhow::Result<()> {
    let started = Instant::now();
    settle_serve(crate::desktop::run(crate::desktop::Startup::serve(started)))
}

/// Turn how a `serve` start ended into its exit: finding the service already running is success.
pub fn settle_serve(ended: anyhow::Result<()>) -> anyhow::Result<()> {
    match ended {
        Err(error) if error.is::<crate::service::AuthorityAlreadyRunning>() => Ok(()),
        other => other,
    }
}

/// Check if a Linux D-Bus starter invocation specifies a session bus.
#[cfg(target_os = "linux")]
pub fn desktop_bus_start(bus_type: Option<&std::ffi::OsStr>) -> bool {
    bus_type.is_some_and(|value| value == "session")
}

/// Request an existing Ghostlight service or newly started service to reveal its window.
pub fn open_desktop() -> anyhow::Result<()> {
    match crate::cli::client::activate_workbench() {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(), None),
        Err(_) => {}
    }
    ghostlight_relay::spawn::start_service()?;
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
/// command line starts `serve` the way a connector does.
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
    if crate::cli::client::readiness().is_err() {
        ghostlight_relay::spawn::start_service()?;
        wait_for_service()?;
    }
    let mut out = std::io::stdout().lock();
    let code = crate::cli::run(command, &mut out);
    std::process::exit(code);
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
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn session_bus_launch_means_start_without_workbench_reveal() {
        use std::ffi::OsStr;
        assert!(super::desktop_bus_start(Some(OsStr::new("session"))));
        assert!(!super::desktop_bus_start(Some(OsStr::new("system"))));
        assert!(!super::desktop_bus_start(None));
    }

    #[test]
    fn serve_exits_quietly_while_another_service_holds_the_lease() {
        let directory = std::env::temp_dir().join(format!(
            "ghostlight-serve-decision-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&directory).unwrap();
        let running = crate::service::ServiceHost::start(&directory).unwrap();

        let started = Instant::now();
        let contended =
            crate::service::ServiceHost::start_waiting(&directory, Duration::from_millis(200))
                .err()
                .expect("the lease is held");
        assert!(
            started.elapsed() >= Duration::from_millis(200),
            "a serve start asks for the lease for its whole wait"
        );
        assert!(settle_serve(Err(contended)).is_ok());

        // Only a held lease is success; any other end of a start is still an error.
        assert!(settle_serve(Err(anyhow::anyhow!("native desktop construction failed"))).is_err());
        assert!(settle_serve(Ok(())).is_ok());

        // A lease released inside the wait is taken, not reported as held.
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(running);
        });
        let taken =
            crate::service::ServiceHost::start_waiting(&directory, Duration::from_secs(5)).unwrap();
        release.join().unwrap();
        drop(taken);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_serve_start_waits_for_the_lease_and_a_human_start_does_not() {
        let started = Instant::now();
        let serve = crate::desktop::Startup::serve(started);
        assert_eq!(serve.lease_wait, Duration::from_secs(1));
        assert_eq!(
            serve.discovery_deadline,
            Some(started + crate::service::lifetime::DISCOVERY_WINDOW)
        );
        let human = crate::desktop::Startup::interactive();
        assert_eq!(human.lease_wait, Duration::ZERO);
        assert!(human.discovery_deadline.is_none());
    }

    #[test]
    fn open_waits_for_delayed_native_presentation_and_reveals_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        use crate::workbench::{
            WorkbenchNotification, WorkbenchPresentationError, WorkbenchPresentationPort,
        };

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
