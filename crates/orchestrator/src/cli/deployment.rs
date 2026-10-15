//! `ghostlight deployment install` on the command line: run the installer and say what it did.
//!
//! `--plan` prints the plan as one JSON document. Otherwise each step that changed something gets
//! one line, and a service that did not start is an error, so the exit code tells a script that
//! the next launch will install again. Every sentence is in [`crate::language::deployment`]; this
//! module chooses which to print and where.

use crate::install::container;
use crate::install::deployment::{self, Mode, Outcome, Request, ServiceDidNotStart};
use crate::install::entry::same_path;
use crate::install::layout::Layout;
use crate::install::migration::FlatpakStep;
use crate::install::reconcile::ReconcileReport;
use crate::language::deployment::{self as words, Placed};
use crate::language::environment;

/// Where a line of the report goes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stream {
    /// Standard output: what happened.
    Output,
    /// Standard error: something a person should look at.
    Problem,
}

/// One line of the report.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Line {
    stream: Stream,
    text: String,
}

impl Line {
    fn output(text: String) -> Self {
        Self {
            stream: Stream::Output,
            text,
        }
    }

    fn problem(text: String) -> Self {
        Self {
            stream: Stream::Problem,
            text,
        }
    }
}

/// Run `request` and report it on standard output.
pub fn run(request: &Request) -> anyhow::Result<()> {
    let outcome = deployment::run(request)?;
    if request.plan_only {
        println!("{}", serde_json::to_string_pretty(&outcome.plan)?);
        return Ok(());
    }
    let layout = Layout::current()?;
    for line in report(&outcome, &layout) {
        match line.stream {
            Stream::Output => println!("{}", line.text),
            Stream::Problem => eprintln!("{}", line.text),
        }
    }
    if outcome.service_started == Some(false) {
        return Err(ServiceDidNotStart(layout.bin()).into());
    }
    Ok(())
}

/// One line for each thing `outcome` changed, saying only what was observed.
fn report(outcome: &Outcome, layout: &Layout) -> Vec<Line> {
    let plan = &outcome.plan;
    let mut lines = Vec::new();
    if plan.migration.has_earlier_installation() {
        lines.push(Line::output(words::moved_earlier_installation(
            &layout.home().display().to_string(),
        )));
    }
    if plan.unchanged {
        lines.push(Line::output(words::already_installed(
            &plan.version,
            &layout.bin().display().to_string(),
        )));
        return lines;
    }
    for placed in &plan.swaps {
        lines.push(Line::output(words::installed_file(
            &placed.name,
            &placed.directory.display().to_string(),
        )));
    }
    lines.extend(service_line(outcome, layout));
    lines.extend(deletion_lines(outcome, layout));
    if outcome.service_started == Some(true) {
        lines.push(Line::output(words::SERVICE_STARTED.to_owned()));
    }
    if let Some(registrations) = &outcome.registrations {
        lines.extend(registration_lines(registrations, layout));
    }
    lines.extend(outcome.flatpak.iter().map(flatpak_line));
    lines
}

/// Where the installed build serves from, said only once a service was seen answering.
fn service_line(outcome: &Outcome, layout: &Layout) -> Option<Line> {
    let placed = match outcome.plan.mode {
        Mode::Development => Placed::Development,
        Mode::Restore => Placed::Release,
        Mode::Release => return None,
    };
    let service = layout.service_executable().display().to_string();
    match outcome.service_started {
        Some(true) => Some(Line::output(words::serves_from(placed, &service))),
        Some(false) => Some(Line::problem(words::placed_without_service(
            placed, &service,
        ))),
        None => None,
    }
}

/// What became of the directory and the marker earlier development builds used.
fn deletion_lines(outcome: &Outcome, layout: &Layout) -> Vec<Line> {
    let marker = layout.retired_development_marker();
    outcome
        .plan
        .deletions
        .iter()
        .map(|path| {
            let is_marker = same_path(path, &marker);
            let kept = outcome.left_behind.contains(path);
            let shown = path.display().to_string();
            match (is_marker, kept) {
                (false, false) => Line::output(words::removed_development_directory(&shown)),
                (true, false) => Line::output(words::removed_development_marker(&shown)),
                (false, true) => Line::problem(words::kept_development_directory(&shown)),
                (true, true) => Line::problem(words::kept_development_marker(&shown)),
            }
        })
        .collect()
}

/// What became of one file the earlier Flatpak browser route left in the home.
fn flatpak_line(step: &FlatpakStep) -> Line {
    let text = words::flatpak_line(step);
    if words::flatpak_failed(step) {
        Line::problem(text)
    } else {
        Line::output(text)
    }
}

/// What the reconcile changed and what it left for a person.
fn registration_lines(report: &ReconcileReport, layout: &Layout) -> Vec<Line> {
    let mut lines = Vec::new();
    if !report.browsers.is_empty() {
        lines.push(Line::output(words::browsers_registered(&report.browsers)));
    }
    for client in &report.clients_rewritten {
        lines.push(Line::output(words::client_rewritten(client)));
    }
    for entry in &report.clients_unchanged {
        lines.push(Line::output(match &entry.found {
            Some(found) => words::client_keeps_entry(&entry.client, found),
            None => words::client_entry_unchangeable(&entry.client),
        }));
    }
    for entry in &report.platform {
        lines.push(Line::output(words::platform_entry_updated(entry)));
    }
    if report.needs_new_terminal {
        lines.push(Line::output(environment::OPEN_NEW_TERMINAL.to_owned()));
    }
    if let Some(container) = &report.container {
        lines.extend(
            container::kept_private_lines(container, &report.kept_private, layout)
                .into_iter()
                .map(Line::output),
        );
    }
    lines.extend(
        report
            .failures
            .iter()
            .map(|failure| Line::problem(words::registration_failed(failure))),
    );
    lines
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::install::deployment::{InstallPlan, Swap};
    use crate::install::migration::MigrationPlan;

    fn plan(mode: Mode, layout: &Layout, deletions: Vec<PathBuf>) -> InstallPlan {
        InstallPlan {
            mode,
            version: "2.0.0".to_owned(),
            migration: MigrationPlan::default(),
            unchanged: false,
            swaps: vec![Swap {
                directory: layout.bin(),
                name: "ghostlight-service".to_owned(),
                source: PathBuf::from("payload"),
            }],
            restart: true,
            record: None,
            processes_to_stop: Vec::new(),
            deletions,
            registrations_to_rewrite: Vec::new(),
        }
    }

    fn report_of(
        mode: Mode,
        layout: &Layout,
        deletions: Vec<PathBuf>,
        started: Option<bool>,
        left_behind: Vec<PathBuf>,
    ) -> Vec<Line> {
        report(
            &Outcome {
                plan: plan(mode, layout, deletions),
                service_started: started,
                registrations: None,
                flatpak: Vec::new(),
                left_behind,
            },
            layout,
        )
    }

    fn texts(lines: &[Line]) -> Vec<&str> {
        lines.iter().map(|line| line.text.as_str()).collect()
    }

    #[test]
    fn a_build_is_said_to_serve_only_once_a_service_answered() {
        let layout = Layout::new("home");
        let service = layout.service_executable().display().to_string();
        for (mode, noun) in [
            (Mode::Development, "development build"),
            (Mode::Restore, "release"),
        ] {
            let started = report_of(mode, &layout, Vec::new(), Some(true), Vec::new());
            let sentence = format!("The {noun} now serves from {service}.");
            assert!(texts(&started).contains(&sentence.as_str()), "{started:?}");
            let failed = report_of(mode, &layout, Vec::new(), Some(false), Vec::new());
            assert!(
                failed.iter().all(|line| !line.text.contains("now serves")),
                "{failed:?}"
            );
            assert!(
                failed
                    .iter()
                    .any(|line| line.stream == Stream::Problem && line.text.contains(&service)),
                "{failed:?}"
            );
        }
    }

    #[test]
    fn the_marker_and_the_directory_each_get_their_own_sentence() {
        let layout = Layout::new("home");
        let directory = layout.retired_development_directory();
        let marker = layout.retired_development_marker();
        let deletions = vec![directory.clone(), marker.clone()];
        let shown_marker = marker.display().to_string();

        let removed = report_of(
            Mode::Development,
            &layout,
            deletions.clone(),
            Some(true),
            Vec::new(),
        );
        let directory_sentence = format!(
            "Removed {}, where earlier development builds ran.",
            directory.display()
        );
        assert!(
            texts(&removed).contains(&directory_sentence.as_str()),
            "{removed:?}"
        );
        let marker_line = removed
            .iter()
            .find(|line| line.text.contains(&shown_marker))
            .unwrap();
        assert!(!marker_line
            .text
            .contains("where earlier development builds ran"));
        assert_eq!(marker_line.stream, Stream::Output);

        let kept = report_of(
            Mode::Development,
            &layout,
            deletions,
            Some(true),
            vec![directory, marker],
        );
        let kept_marker = kept
            .iter()
            .find(|line| line.text.contains(&shown_marker))
            .unwrap();
        assert_eq!(kept_marker.stream, Stream::Problem);
        assert!(!kept_marker.text.contains("program still runs"));
        assert!(
            kept.iter()
                .any(|line| line.stream == Stream::Problem
                    && line.text.contains("program still runs"))
        );
    }

    #[test]
    fn a_release_install_says_nothing_about_serving_from() {
        let layout = Layout::new("home");
        let lines = report_of(Mode::Release, &layout, Vec::new(), Some(true), Vec::new());
        assert!(lines.iter().all(|line| !line.text.contains("serves")));
    }
}
