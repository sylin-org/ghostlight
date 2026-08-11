//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

use std::ffi::OsString;
use std::path::Path;
use std::thread;
use std::time::Duration;

const ACTIVATION_RETRY_COUNT: usize = 20;
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaunchMode {
    Headless,
    Background,
    Show,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationState {
    Activated,
    Unavailable,
    Unreachable,
}

fn main() -> anyhow::Result<()> {
    match launch_mode(std::env::args_os().skip(1)) {
        LaunchMode::Headless => ghostlight::service::run_forever(),
        LaunchMode::Background => ghostlight::desktop::run(false),
        LaunchMode::Show => show_or_start(),
    }
}

fn show_or_start() -> anyhow::Result<()> {
    let runtime = ghostlight_bridge::runtime::runtime_file();
    match ghostlight::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    match ghostlight::desktop::run(true) {
        Ok(()) => Ok(()),
        Err(start_error) => {
            finish_activation(wait_for_workbench_activation(&runtime), Some(start_error))
        }
    }
}

fn wait_for_workbench_activation(runtime: &Path) -> ActivationState {
    let mut presentation_seen = false;
    for _ in 0..ACTIVATION_RETRY_COUNT {
        match ghostlight::service::request_workbench_activation(runtime) {
            Ok(true) => return ActivationState::Activated,
            Ok(false) => presentation_seen = true,
            Err(_) => {}
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
    if presentation_seen {
        ActivationState::Unavailable
    } else {
        ActivationState::Unreachable
    }
}

fn finish_activation(
    activation: ActivationState,
    start_error: Option<anyhow::Error>,
) -> anyhow::Result<()> {
    match activation {
        ActivationState::Activated => Ok(()),
        ActivationState::Unavailable => anyhow::bail!(
            "Ghostlight is running without a desktop workbench; stop the explicit headless engine before opening the desktop"
        ),
        ActivationState::Unreachable => Err(start_error.unwrap_or_else(|| {
            anyhow::anyhow!("the running Ghostlight authority could not be reached")
        })),
    }
}

fn launch_mode(arguments: impl IntoIterator<Item = OsString>) -> LaunchMode {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--headless") {
        LaunchMode::Headless
    } else if arguments.iter().any(|argument| argument == "--background") {
        LaunchMode::Background
    } else {
        LaunchMode::Show
    }
}

#[cfg(test)]
mod tests {
    use super::{launch_mode, LaunchMode};

    #[test]
    fn launch_modes_keep_human_and_adapter_intent_distinct() {
        assert_eq!(launch_mode(Vec::new()), LaunchMode::Show);
        assert_eq!(launch_mode(["--show".into()]), LaunchMode::Show);
        assert_eq!(launch_mode(["--background".into()]), LaunchMode::Background);
        assert_eq!(launch_mode(["--headless".into()]), LaunchMode::Headless);
        assert_eq!(
            launch_mode(["--background".into(), "--headless".into()]),
            LaunchMode::Headless
        );
    }
}
