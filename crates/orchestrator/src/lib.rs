//! Ghostlight's single mutable, domain-driven browser orchestrator.

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
compile_error!("Ghostlight supports Windows and Linux only.");

pub mod browser;
pub mod cli;
pub mod desktop;
pub mod diagnostics;
pub mod events;
pub mod governance;
pub mod install;
pub mod language;
pub mod presentation;
pub mod service;
pub mod work;
pub mod workbench;
pub mod workspace;
