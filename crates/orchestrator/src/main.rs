//! The `ghostlight-service` executable: the service, its desktop workbench and every typed
//! command (ADR-0199 Decision 1). It is a console-subsystem program: a typed command writes to the
//! terminal it runs in, and a start with no console to join shows none (the build script embeds the
//! manifest that says so, ADR-0199 Decision 4).
//!
//! Before any command runs, the entry rule decides whether this copy does the job, hands it to
//! the installed service, hands the edge's relay verbs to the installed edge, or installs from
//! its own package first (ADR-0197 Decisions 10 and 13, ADR-0199 Decision 2 and the development
//! channel amendment).

use ghostlight_service::cli::sign_out::listen_unless_serving;
use ghostlight_service::cli::start::{start, ProcessLauncher, Start};
use ghostlight_service::install::layout::Layout;

fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let _session = listen_unless_serving(&arguments);
    let current_exe = std::env::current_exe()?;
    match start(
        arguments,
        &current_exe,
        &Layout::current()?,
        &ProcessLauncher,
    )? {
        Start::Dispatch(mode) => ghostlight_service::cli::dispatch(mode),
        Start::Exit(code) => std::process::exit(code),
    }
}
