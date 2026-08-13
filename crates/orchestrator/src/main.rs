//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::thread;
use std::time::Duration;

const ACTIVATION_RETRY_COUNT: usize = 20;
const ACTIVATION_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaunchMode {
    Headless,
    Desktop,
    /// The command-line intake. A script asked for work, not for a window (ADR-0105).
    Call,
    /// The narrow package-facing Chromium registration seam (ADR-0115).
    NativeHost(NativeHostCommand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeHostCommand {
    Check,
    Install,
    Uninstall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActivationState {
    Activated,
    Unavailable,
    Unreachable,
}

fn main() -> anyhow::Result<()> {
    match launch_mode(std::env::args_os().skip(1))? {
        LaunchMode::Headless => ghostlight::service::run_forever(),
        LaunchMode::Desktop => start_or_activate_desktop(),
        LaunchMode::Call => run_call(),
        LaunchMode::NativeHost(command) => run_native_host(command),
    }
}

fn run_native_host(command: NativeHostCommand) -> anyhow::Result<()> {
    use ghostlight::install::native_host::{NativeHostRegistry, NativeHostState};

    let registry = NativeHostRegistry::discover();
    let (verb, changed, report, migration) = match command {
        NativeHostCommand::Check => ("checked", false, registry.check()?, None),
        NativeHostCommand::Install => {
            let result = registry.install();
            let migration = ghostlight::install::migration::retire_obsolete_supervisor();
            let result = result?;
            ("installed", result.changed, result.report, Some(migration))
        }
        NativeHostCommand::Uninstall => {
            let result = registry.uninstall()?;
            ("uninstalled", result.changed, result.report, None)
        }
    };
    println!("Ghostlight native host {verb}; changed: {changed}");
    println!("Connector: {}", report.connector.display());
    for browser in report.browsers {
        let state = match browser.state {
            NativeHostState::Missing => "missing",
            NativeHostState::Current => "current",
            NativeHostState::Updatable => "updatable",
            NativeHostState::NeedsAttention => "needs attention",
        };
        println!("{}: {state} -- {}", browser.name, browser.detail);
    }
    if let Some(migration) = migration {
        for removed in migration.removed {
            println!("Retired: {removed}");
        }
        for preserved in migration.preserved {
            println!("Preserved: {preserved}");
        }
        for warning in migration.warnings {
            eprintln!("Migration warning: {warning}");
        }
    }
    Ok(())
}

/// Invoke one tool, or a batch of them, against the local authority.
///
/// Demand-start applies here exactly as it does to a connector: a script that runs before anything
/// else has started gets an authority rather than an error.
fn run_call() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args()
        .skip(1)
        .skip_while(|argument| argument != "call")
        .skip(1)
        .collect();
    let command = match ghostlight::cli::parse(&arguments) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let runtime = ghostlight_bridge::runtime::runtime_file();
    if ghostlight_bridge::runtime::read_runtime(&runtime).is_err() {
        let _ = ghostlight_bridge::lifecycle::request_orchestrator_start();
        wait_for_runtime(&runtime);
    }
    let mut out = std::io::stdout().lock();
    let code = ghostlight::cli::run(command, &runtime, &mut out);
    std::process::exit(code);
}

fn wait_for_runtime(runtime: &Path) {
    for _ in 0..ACTIVATION_RETRY_COUNT {
        if ghostlight_bridge::runtime::read_runtime(runtime).is_ok() {
            return;
        }
        thread::sleep(ACTIVATION_RETRY_DELAY);
    }
}

fn start_or_activate_desktop() -> anyhow::Result<()> {
    let runtime = ghostlight_bridge::runtime::runtime_file();
    match ghostlight::service::request_workbench_activation(&runtime) {
        Ok(true) => return Ok(()),
        Ok(false) => return finish_activation(wait_for_workbench_activation(&runtime), None),
        Err(_) => {}
    }
    match ghostlight::desktop::run() {
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

fn launch_mode(arguments: impl IntoIterator<Item = OsString>) -> anyhow::Result<LaunchMode> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|argument| argument == "native-host")
    {
        let command = match arguments.get(1).and_then(|argument| argument.to_str()) {
            Some("check") => NativeHostCommand::Check,
            Some("install") => NativeHostCommand::Install,
            Some("uninstall") => NativeHostCommand::Uninstall,
            _ => anyhow::bail!("usage: ghostlight native-host <check|install|uninstall>"),
        };
        if arguments.len() != 2 {
            anyhow::bail!("usage: ghostlight native-host <check|install|uninstall>");
        }
        Ok(LaunchMode::NativeHost(command))
    } else if arguments
        .iter()
        .any(|argument| argument == OsStr::new("call"))
    {
        Ok(LaunchMode::Call)
    } else if arguments.iter().any(|argument| argument == "--headless") {
        Ok(LaunchMode::Headless)
    } else {
        Ok(LaunchMode::Desktop)
    }
}

#[cfg(test)]
mod tests {
    use super::{launch_mode, LaunchMode, NativeHostCommand};

    #[test]
    fn launch_modes_keep_desktop_headless_and_call_intents_distinct() {
        assert_eq!(launch_mode(Vec::new()).unwrap(), LaunchMode::Desktop);
        assert_eq!(
            launch_mode(["--headless".into()]).unwrap(),
            LaunchMode::Headless
        );
        assert_eq!(launch_mode(["call".into()]).unwrap(), LaunchMode::Call);
        assert_eq!(
            launch_mode(["native-host".into(), "check".into()]).unwrap(),
            LaunchMode::NativeHost(NativeHostCommand::Check)
        );
        assert!(launch_mode(["native-host".into(), "guess".into()]).is_err());
    }
}
