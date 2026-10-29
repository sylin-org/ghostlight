//! `ghostlight update`: the version check the person starts from the command line, and
//! `--install` to install the newer release it finds (ADR-0206).
//!
//! The command runs in its own process, outside the service, so it reads the authority from this
//! computer's policy files as the service does at its start, and it sees no browser: a release in
//! another major.minor block, which needs a newer extension, is installed from the window, which
//! knows the extension each browser runs.

use crate::governance::GovernanceFacade;
use crate::install::layout::Layout;
use crate::install::update::{self, Finding, Https, Version};
use crate::language::update as words;

/// Check for a newer release, and install it when `install` is set.
///
/// # Errors
///
/// When the authority turns the check off, and the check's, download's or start's failure, each
/// in the words the person reads. Nothing is replaced on any of them.
pub fn run(install: bool) -> anyhow::Result<()> {
    if !GovernanceFacade::from_environment()
        .snapshot()
        .allows_update_check()
    {
        anyhow::bail!(words::TURNED_OFF);
    }
    let installed = Version::parse(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| anyhow::anyhow!(words::UNEXPECTED))?;
    let layout = Layout::current()?;
    println!("{}", words::CHECKING);
    let fetch = Https::new();
    let release = match update::check(&fetch, installed, update::installed_channel(&layout), &[])? {
        Finding::Current { .. } => {
            println!("{}", words::current(&installed.to_string()));
            return Ok(());
        }
        Finding::Development { latest } => {
            println!("{}", words::development(&latest.to_string()));
            return Ok(());
        }
        Finding::NeedsExtension { release, .. } => {
            println!("{}", words::needs_extension(&release.version.to_string()));
            return Ok(());
        }
        Finding::Newer { release, .. } => release,
    };
    let latest = release.version.to_string();
    println!("{}", words::newer(&latest));
    if !release.notes.trim().is_empty() {
        println!("\n{}\n", release.notes.trim());
    }
    if !install {
        println!("{}", words::INSTALL_HINT);
        return Ok(());
    }
    let target = update::this_target().ok_or(update::UpdateError::NoImages)?;
    let staged = update::stage(
        &fetch,
        &release,
        target,
        cfg!(windows),
        &layout.home().join(update::STAGING_DIRECTORY),
    )?;
    // Started detached, as the window starts it: the installer stops the service image this
    // command runs from, so it must not be this command's child.
    update::start(&staged).map_err(update::UpdateError::from)?;
    println!("{}", words::installing_from_command_line(&latest));
    Ok(())
}
