//! Orchestrator-owned installation selection for package setup and the development loop.

use std::env;
use std::io;

use ghostlight_bridge::installation::{self, Installation, Selection};

/// Select this executable set while preserving development custody during package installation.
pub fn select(selection: Selection) -> io::Result<Option<Installation>> {
    if env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some() {
        // Explicit fixtures retain their private election; never edit the production record.
        return Ok(None);
    }
    let runtime = installation::production_runtime()?;
    let guard = installation::lock(&runtime)?;
    let current = env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| io::Error::other("executable has no directory"))?;
    let record = installation::select_locked(&runtime, directory, selection)?;
    drop(guard);
    Ok(Some(record))
}

/// An inactive package may remove its artifacts, but cannot unregister the user's active route.
pub fn owns_registration() -> io::Result<bool> {
    if env::var_os("GHOSTLIGHT_RUNTIME_FILE").is_some() {
        return Ok(true);
    }
    let runtime = installation::production_runtime()?;
    let Some(record) = installation::read(&runtime)? else {
        return Ok(true);
    };
    let current = env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| io::Error::other("executable has no directory"))?;
    Ok(std::fs::canonicalize(directory)? == std::fs::canonicalize(record.directory())?)
}
