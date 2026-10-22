//! The service: Ghostlight's product authority, and everything else the `ghostlight-service`
//! executable is (docs/1.0/ARCHITECTURE.md).
//!
//! One part has three names here. The directory is `crates/orchestrator`, the package is
//! `ghostlight-service` and this library `ghostlight_service`, and the process-diagnostics
//! component, with its log files, is `orchestrator`. All three are the service. None is renamed:
//! the ADRs, the install layout and the diagnostics log names read them.
//!
//! The product authority, which every model-requested call crosses once:
//!
//! - [`service`]: the persistent service, its lifetime, its one relay endpoint and the streams it
//!   serves.
//! - [`intake`]: how a caller's request enters the service: the opening, the MCP engine and the
//!   shell's stream contract.
//! - [`language`]: every word Ghostlight accepts and every word it says.
//! - [`work`]: the one executor and its completion path.
//! - [`workspace`]: the workspace aggregate, its handles, ownership, generations and leases.
//! - [`governance`]: the policy authority and its final-boundary decisions.
//! - [`browser`]: the browser port, and the service half of each browser adapter.
//! - [`presentation`]: content-free feedback in the browser.
//! - [`audit`], [`events`], [`provenance`] and [`diagnostics`]: the audit writer, the closed domain
//!   events, connection evidence and the process-diagnostics hub.
//!
//! The rest of the executable:
//!
//! - [`cli`]: the command line, every typed command from its arguments to its exit.
//! - [`install`]: everything that puts Ghostlight on a computer, keeps it current and takes it
//!   off.
//! - [`agent`]: who an agent is and what the window shows for it.
//! - [`workbench`]: the desktop workbench the UI's window shows, served on loopback.

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
compile_error!("Ghostlight supports Windows and Linux only.");

pub mod agent;
pub mod audit;
pub mod browser;
pub mod cli;
pub mod diagnostics;
pub mod events;
pub mod governance;
pub mod install;
pub mod intake;
pub mod language;
pub mod presentation;
pub mod provenance;
#[cfg(test)]
pub(crate) mod scratch;
pub mod service;
pub(crate) mod sync;
pub mod work;
pub mod workbench;
pub mod workspace;
