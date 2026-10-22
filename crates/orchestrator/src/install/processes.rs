//! The installer's processes: one reading of the process table, one way to stop processes and
//! wait for them to exit, and the exact-image match a replace stops by.
//!
//! Which processes to stop is each caller's question, and so is what a stop that fails means. A
//! replace stops the processes of the images it replaces, matched by their exact files
//! ([`stop_images`]), and fails the install when one will not stop. The stray sweep finds the
//! leftovers of a home by place and name and reports the ones that will not stop, blocking
//! nothing ([`super::deployment::stop_strays`]). Both stop through [`stop_and_wait`].

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use sysinfo::{
    Pid, Process, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind,
};

/// How long the processes of a replaced image are given to exit.
const STOP_WAIT: Duration = Duration::from_secs(10);

/// How often a stop looks again whether its processes have exited.
const EXIT_POLL: Duration = Duration::from_millis(25);

/// What a refresh reads of each process: whether it runs, and its executable.
fn with_executables() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .without_tasks()
        .with_exe(UpdateKind::Always)
}

/// The processes of this machine, with their executables.
#[must_use]
pub(super) fn process_table() -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, with_executables());
    system
}

/// Look at `pids` again, with their executables.
fn refresh(system: &mut System, pids: &[Pid]) {
    system.refresh_processes_specifics(ProcessesToUpdate::Some(pids), true, with_executables());
}

/// What a stop attempt on one process came to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StopOutcome {
    /// The kill was delivered; wait for the process to exit.
    Stopping,
    /// The kill failed because the process had already exited, which is what stopping wants.
    AlreadyGone,
    /// The kill failed and the process is still running.
    Failed,
}

/// Classify one stop attempt. A process can exit on its own between the process snapshot and
/// the kill (a connector the browser is cycling does), and that must not fail the stop.
pub(super) fn stop_outcome(killed: bool, gone: bool) -> StopOutcome {
    if killed {
        StopOutcome::Stopping
    } else if gone {
        StopOutcome::AlreadyGone
    } else {
        StopOutcome::Failed
    }
}

/// What one [`stop_and_wait`] came to.
#[derive(Debug, Default)]
pub(super) struct Stopped {
    /// The processes whose kill failed while they ran on.
    pub(super) refused: Vec<Pid>,
    /// The processes still running when the wait ended.
    pub(super) running: Vec<Pid>,
}

/// Kill each of `pids` that still runs and that `is_ours` says is still the process meant, then
/// wait up to `wait` for each to exit, looking every [`EXIT_POLL`]. A process that is gone, or a
/// zombie, has exited.
pub(super) fn stop_and_wait(
    pids: &[Pid],
    wait: Duration,
    is_ours: impl Fn(&Process) -> bool,
) -> Stopped {
    let mut system = System::new();
    let runs = |system: &System, pid: Pid| {
        system
            .process(pid)
            .is_some_and(|process| process.status() != ProcessStatus::Zombie && is_ours(process))
    };
    refresh(&mut system, pids);
    let mut refused = Vec::new();
    for &pid in pids {
        if runs(&system, pid) && !system.process(pid).is_some_and(Process::kill) {
            refused.push(pid);
        }
    }
    if !refused.is_empty() {
        refresh(&mut system, pids);
        refused.retain(|&pid| stop_outcome(false, !runs(&system, pid)) == StopOutcome::Failed);
    }
    let deadline = Instant::now() + wait;
    loop {
        let running: Vec<Pid> = pids
            .iter()
            .copied()
            .filter(|pid| !refused.contains(pid) && runs(&system, *pid))
            .collect();
        if running.is_empty() || Instant::now() >= deadline {
            return Stopped { refused, running };
        }
        thread::sleep(EXIT_POLL);
        refresh(&mut system, pids);
    }
}

/// Stop every process running one of `images`, and wait up to [`STOP_WAIT`] for them to exit.
///
/// This is the bounded fallback for services that predate `quit`, and the way the break
/// migration quiesces the old installation: an old connector must not start an old service while
/// the files move. It refuses to stop the running installer, and fails when a process will not
/// stop.
pub fn stop_images(images: &[PathBuf]) -> anyhow::Result<()> {
    if images.is_empty() {
        return Ok(());
    }
    let pids = running_from(&process_table(), images);
    anyhow::ensure!(
        pids.iter().all(|pid| pid.as_u32() != std::process::id()),
        "Cannot replace the running installer"
    );
    let stopped = stop_and_wait(&pids, STOP_WAIT, |_| true);
    if let Some(pid) = stopped.refused.first() {
        anyhow::bail!("Could not stop Ghostlight process {pid}");
    }
    anyhow::ensure!(
        stopped.running.is_empty(),
        "Ghostlight processes did not finish their replacement"
    );
    Ok(())
}

/// Whether any process other than the running installer is running one of `images`.
#[must_use]
pub fn any_running_from(images: &[PathBuf]) -> bool {
    !images.is_empty()
        && running_from(&process_table(), images)
            .iter()
            .any(|pid| pid.as_u32() != std::process::id())
}

/// The processes of `system` whose executable is one of `images`, matched by their exact files.
fn running_from(system: &System, images: &[PathBuf]) -> Vec<Pid> {
    let images: Vec<PathBuf> = images
        .iter()
        .filter_map(|image| fs::canonicalize(image).ok())
        .collect();
    system
        .processes()
        .values()
        .filter(|process| {
            process
                .exe()
                .and_then(|path| fs::canonicalize(path).ok())
                .is_some_and(|executable| images.contains(&executable))
        })
        .map(Process::pid)
        .collect()
}
