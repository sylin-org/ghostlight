//! `ghostlight deployment install` on the command line: run the installer and say what it did.
//!
//! `--plan` prints the plan as one JSON document. Otherwise each step that changed something gets
//! one line, and a service that did not start is an error, so the exit code tells a script that
//! the next launch will install again.

use crate::install::container;
use crate::install::deployment::{self, MarkerChange, Mode, Outcome, Request, ServiceDidNotStart};
use crate::install::layout::Layout;
use crate::install::reconcile::ReconcileReport;

/// Run `request` and report it on standard output.
pub fn run(request: &Request) -> anyhow::Result<()> {
    let outcome = deployment::run(request)?;
    if request.plan_only {
        println!("{}", serde_json::to_string_pretty(&outcome.plan)?);
        return Ok(());
    }
    let layout = Layout::current()?;
    report(&outcome, &layout);
    if outcome.service_started == Some(false) {
        return Err(ServiceDidNotStart(layout.bin()).into());
    }
    Ok(())
}

/// Print one line for each thing `outcome` changed.
fn report(outcome: &Outcome, layout: &Layout) {
    let plan = &outcome.plan;
    if !plan.migration.is_empty() {
        println!(
            "Moved the files of the earlier Ghostlight installation into {}.",
            layout.home().display()
        );
    }
    if plan.unchanged {
        println!(
            "Ghostlight {} is already installed in {}.",
            plan.version,
            layout.bin().display()
        );
        return;
    }
    for placed in plan.swaps.iter().chain(&plan.connector_replacements) {
        println!(
            "Installed {} in {}.",
            placed.name,
            placed.directory.display()
        );
    }
    match (plan.mode, &plan.marker) {
        (Mode::Development, Some(MarkerChange::Write(executable))) => println!(
            "The development build {} now serves in place of {}.",
            executable.display(),
            layout.service_executable().display()
        ),
        (Mode::Restore, _) => println!(
            "The installed service {} serves again.",
            layout.service_executable().display()
        ),
        _ => {}
    }
    if outcome.service_started == Some(true) {
        println!("The Ghostlight service started.");
    }
    if let Some(registrations) = &outcome.registrations {
        report_registrations(registrations, layout);
    }
}

/// Print what the reconcile changed and what it left for a person.
fn report_registrations(report: &ReconcileReport, layout: &Layout) {
    if !report.browsers.is_empty() {
        println!(
            "Browser connection registered for {}.",
            report.browsers.join(", ")
        );
    }
    for client in &report.clients_rewritten {
        println!("{client} now starts the installed MCP connector.");
    }
    for entry in &report.clients_unchanged {
        match &entry.found {
            Some(found) => println!(
                "{} keeps its own Ghostlight entry ({found}); it was not changed.",
                entry.client
            ),
            None => println!(
                "{} has a Ghostlight entry Ghostlight cannot change; it was not changed.",
                entry.client
            ),
        }
    }
    for entry in &report.platform {
        println!("Updated the {entry}.");
    }
    if let Some(container) = &report.container {
        for line in container::kept_private_lines(container, &report.kept_private, layout) {
            println!("{line}");
        }
    }
    for failure in &report.failures {
        eprintln!("Could not update a registration: {failure}");
    }
}
