//! Desktop workbench lifecycle, window activation, and local execution runners.
//!
//! This module coordinates process activation, single-instance workbench window
//! presentation, runtime readiness waiting, policy inspection, and local `call` execution.

use std::path::Path;
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
    if ghostlight_bridge::lifecycle::production_deployment_in_progress()? {
        anyhow::bail!("Ghostlight is being updated; it will be available when deployment finishes");
    }
    if std::env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_none() {
        let runtime = ghostlight_bridge::installation::production_runtime()?;
        let selection = ghostlight_bridge::installation::read(&runtime)?;
        if selection.as_ref().is_none_or(|record| {
            record.development_directory.is_none()
                && record
                    .release_directory
                    .as_ref()
                    .is_some_and(|release| release != record.directory())
        }) {
            let _selection = ghostlight_bridge::installation::startup_selection()?;
        }
        let selected = ghostlight_bridge::installation::selected_executable()?;
        if std::fs::canonicalize(selected)? != std::fs::canonicalize(std::env::current_exe()?)? {
            return open_desktop();
        }
    }
    #[cfg(target_os = "linux")]
    if desktop_bus_start(std::env::var_os("DBUS_STARTER_BUS_TYPE").as_deref()) {
        // A bus launch requests existence, never Open. In a cold-start race a losing
        // process must exit at the lifetime lease instead of revealing the winner's UI.
        return crate::desktop::run();
    }
    let runtime = ghostlight_bridge::runtime::runtime_file()?;
    match crate::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    match crate::desktop::run() {
        Ok(()) => Ok(()),
        Err(start_error) if start_error.is::<crate::service::AuthorityAlreadyRunning>() => {
            finish_activation(wait_for_workbench_activation(&runtime), Some(start_error))
        }
        Err(start_error) => Err(start_error),
    }
}

/// Check if a Linux D-Bus starter invocation specifies a session bus.
#[cfg(target_os = "linux")]
pub fn desktop_bus_start(bus_type: Option<&std::ffi::OsStr>) -> bool {
    bus_type.is_some_and(|value| value == "session")
}

/// Request an existing Ghostlight service or newly demand-started service to reveal its window.
pub fn open_desktop() -> anyhow::Result<()> {
    use ghostlight_bridge::lifecycle::StartDisposition;

    let runtime = ghostlight_bridge::runtime::runtime_file()?;
    match crate::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    if ghostlight_bridge::lifecycle::request_orchestrator_start()?
        == StartDisposition::DeploymentInProgress
    {
        anyhow::bail!("Ghostlight is being updated; open it again when the update finishes");
    }
    finish_activation(wait_for_workbench_activation(&runtime), None)
}

/// Wait until the workbench presentation attaches and activates or times out.
pub fn wait_for_workbench_activation(runtime: &Path) -> ActivationState {
    wait_for_workbench_activation_until(runtime, Instant::now() + WORKBENCH_ACTIVATION_TIMEOUT)
}

/// Wait until the workbench presentation activates or the given deadline expires.
pub fn wait_for_workbench_activation_until(runtime: &Path, deadline: Instant) -> ActivationState {
    let mut presentation_seen = false;
    loop {
        match crate::service::request_workbench_activation(runtime) {
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
/// Demand-start applies here exactly as it does to a connector: a script that runs before anything
/// else has started gets an authority rather than an error.
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
    let runtime = match ghostlight_bridge::runtime::runtime_file() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    if ghostlight_bridge::lifecycle::request_orchestrator_start()?
        == ghostlight_bridge::lifecycle::StartDisposition::DeploymentInProgress
    {
        anyhow::bail!("Ghostlight is being updated; retry when deployment finishes");
    }
    wait_for_runtime(&runtime)?;
    let mut out = std::io::stdout().lock();
    let code = crate::cli::run(command, &runtime, &mut out);
    std::process::exit(code);
}

/// Run local policy governance inspection commands.
pub fn run_policy(command: &crate::governance::inspection::Command) -> anyhow::Result<()> {
    let mut out = std::io::stdout().lock();
    crate::governance::inspection::run(command, &mut out)
}

/// Wait until the local runtime file indicates readiness.
pub fn wait_for_runtime(runtime: &Path) -> anyhow::Result<()> {
    let deadline = Instant::now() + SERVICE_READY_TIMEOUT;
    while Instant::now() < deadline {
        if crate::service::request_readiness(runtime).is_ok() {
            return Ok(());
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
    anyhow::bail!("The selected Ghostlight authority did not become ready in time; run ghostlight doctor for its deployment details")
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

        let directory = std::env::temp_dir().join(format!(
            "ghostlight-delayed-presentation-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&directory).unwrap();
        let runtime = directory.join("runtime.json");
        let host = crate::service::ServiceHost::start(&runtime).unwrap();
        host.publish_ready().unwrap();
        assert!(!crate::service::request_workbench_activation(&runtime).unwrap());
        let timeout_started = std::time::Instant::now();
        let timeout = wait_for_workbench_activation_until(
            &runtime,
            timeout_started + Duration::from_millis(100),
        );
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
        let result = wait_for_workbench_activation(&runtime);
        ready.join().unwrap();
        drop(host);
        std::fs::remove_dir_all(directory).unwrap();
        assert_eq!(result, ActivationState::Activated);
        assert_eq!(presentation.0.load(Ordering::SeqCst), 1);
    }
}
