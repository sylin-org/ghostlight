//! `ghostlight deployment install` on the command line: run the installer and say what it did.
//!
//! `--plan` prints the plan as one JSON document. Otherwise each step that changed something gets
//! one line, and a service that did not start is an error, so the exit code tells a script that
//! the next launch will install again. Every sentence is in [`crate::language::deployment`]; this
//! module chooses which to print and where.

use crate::install::deployment::{self, Mode, Outcome, Request, ServiceDidNotStart};
use crate::install::entry::same_path;
use crate::install::layout::Layout;
use crate::install::migration::FlatpakStep;
use crate::install::reconcile::ReconcileReport;
use crate::install::version_directories;
use crate::language::deployment::{self as words, Placed};
use crate::language::environment;
use crate::language::workbench as stray_words;

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
    if let Some(failure) = ServiceDidNotStart::of(&outcome, &layout) {
        return Err(failure.into());
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
    lines.extend(
        plan.edge_replaced
            .map(|reason| Line::output(words::edge_replaced(reason))),
    );
    lines.extend(service_line(outcome, layout));
    lines.extend(deletion_lines(outcome, layout));
    lines.extend(outcome.strays_left.iter().map(|stray| {
        Line::problem(stray_words::stray_not_stopped_line(
            &stray.image.display().to_string(),
        ))
    }));
    if outcome.service_started == Some(true) {
        lines.push(Line::output(words::SERVICE_STARTED.to_owned()));
    }
    if let Some(registrations) = &outcome.registrations {
        lines.extend(registration_lines(registrations, layout));
    }
    lines.extend(outcome.flatpak.iter().map(flatpak_line));
    lines.extend(
        outcome.retired_leftovers.iter().map(|directory| {
            Line::output(words::leftover_removed(&directory.display().to_string()))
        }),
    );
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

/// What became of the directory and the marker earlier development builds used and of the 1.x
/// version directories, with the archive each removed directory's audit log went to, as
/// observed.
fn deletion_lines(outcome: &Outcome, layout: &Layout) -> Vec<Line> {
    let marker = layout.retired_development_marker();
    let mut lines = Vec::new();
    for path in &outcome.plan.deletions {
        let shown = path.display().to_string();
        if outcome.left_behind.contains(path) {
            lines.push(Line::problem(words::kept_entry(&shown)));
        } else if same_path(path, &marker) {
            lines.push(Line::output(words::removed_development_marker(&shown)));
        } else if version_directories::is_version_directory(layout, path) {
            lines.push(Line::output(words::removed_version_directory(&shown)));
            lines.extend(
                version_directories::archive_of(&outcome.archived_version_audits, path).map(
                    |archive| {
                        Line::output(words::archived_version_audit(
                            &archive.to.display().to_string(),
                        ))
                    },
                ),
            );
        } else {
            lines.push(Line::output(words::removed_development_directory(&shown)));
        }
    }
    lines
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
            words::kept_private_lines(container, &report.kept_private, layout)
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
            edge_replaced: None,
            restart: true,
            record: None,
            processes_to_stop: Vec::new(),
            deletions,
            version_audit_archives: Vec::new(),
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
                start_failure: None,
                registrations: None,
                flatpak: Vec::new(),
                left_behind,
                archived_version_audits: Vec::new(),
                strays_left: Vec::new(),
                retired_leftovers: Vec::new(),
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
    fn a_removed_or_kept_directory_and_marker_are_each_named_by_their_own_path() {
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
            vec![directory.clone(), marker],
        );
        let kept_marker = kept
            .iter()
            .find(|line| line.text.contains(&shown_marker))
            .unwrap();
        assert_eq!(kept_marker.stream, Stream::Problem);
        assert_eq!(
            kept_marker.text,
            format!("Could not remove {shown_marker}. The next install tries again.")
        );
        assert!(kept.iter().all(|line| !line.text.contains("still runs")));
        let shown_directory = directory.display().to_string();
        // The marker's path starts with the directory's, so match the whole sentence.
        let kept_directory =
            format!("Could not remove {shown_directory}. The next install tries again.");
        assert!(
            kept.iter()
                .any(|line| line.stream == Stream::Problem && line.text == kept_directory),
            "{kept:?}"
        );
    }

    #[test]
    fn a_removed_or_kept_version_directory_is_named_with_the_archive_of_its_log() {
        use crate::install::migration::Move;

        let layout = Layout::new("home");
        let first = layout.bin().join("v1.3.5");
        let second = layout.bin().join("v1.3.6");
        let archive = layout.state().join("audit-v1.3.5.jsonl");
        let outcome = |left_behind: Vec<PathBuf>| Outcome {
            plan: plan(
                Mode::Development,
                &layout,
                vec![first.clone(), second.clone()],
            ),
            service_started: Some(true),
            start_failure: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind,
            archived_version_audits: vec![Move {
                from: first.join("audit.jsonl"),
                to: archive.clone(),
            }],
            strays_left: Vec::new(),
            retired_leftovers: Vec::new(),
        };

        let removed = report(&outcome(Vec::new()), &layout);
        let removed_first = format!(
            "Removed {}, where an earlier Ghostlight kept one of its versions.",
            first.display()
        );
        let kept_log = format!("Kept its audit log as {}.", archive.display());
        let start = removed
            .iter()
            .position(|line| line.text == removed_first)
            .unwrap_or_else(|| panic!("{removed:?}"));
        assert_eq!(removed[start + 1].text, kept_log);
        assert!(removed.iter().any(|line| line
            .text
            .starts_with(&format!("Removed {}", second.display()))));
        assert_eq!(
            removed.iter().filter(|line| line.text == kept_log).count(),
            1
        );
        assert!(removed.iter().all(|line| line.stream == Stream::Output));

        let kept = report(&outcome(vec![second.clone()]), &layout);
        let sentence = format!(
            "Could not remove {}. The next install tries again.",
            second.display()
        );
        assert!(
            kept.iter()
                .any(|line| line.stream == Stream::Problem && line.text == sentence),
            "{kept:?}"
        );
    }

    #[test]
    fn a_stray_that_would_not_stop_is_named_as_a_problem() {
        use crate::install::deployment::{StrayKind, StrayProcess};

        let layout = Layout::new("home");
        let image = layout.bin().join("ghostlight-mcp-connector");
        let outcome = Outcome {
            plan: plan(Mode::Release, &layout, Vec::new()),
            service_started: Some(true),
            start_failure: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
            archived_version_audits: Vec::new(),
            strays_left: vec![StrayProcess {
                pid: 7,
                image: image.clone(),
                kind: StrayKind::RetiredConnector,
            }],
            retired_leftovers: Vec::new(),
        };
        let lines = report(&outcome, &layout);
        let sentence = format!("Could not stop {}. Close it yourself.", image.display());
        let named: Vec<&Line> = lines.iter().filter(|line| line.text == sentence).collect();
        assert_eq!(named.len(), 1, "{lines:?}");
        assert_eq!(named[0].stream, Stream::Problem);
    }

    #[test]
    fn a_release_install_says_nothing_about_serving_from() {
        let layout = Layout::new("home");
        let lines = report_of(Mode::Release, &layout, Vec::new(), Some(true), Vec::new());
        assert!(lines.iter().all(|line| !line.text.contains("serves")));
    }

    #[test]
    fn a_replaced_edge_is_said_once_with_its_reason_and_a_kept_one_not_at_all() {
        use crate::install::deployment::EdgeReplacement;

        let layout = Layout::new("home");
        let outcome = |edge_replaced: Option<EdgeReplacement>| Outcome {
            plan: InstallPlan {
                edge_replaced,
                ..plan(Mode::Development, &layout, Vec::new())
            },
            service_started: Some(true),
            start_failure: None,
            registrations: None,
            flatpak: Vec::new(),
            left_behind: Vec::new(),
            archived_version_audits: Vec::new(),
            strays_left: Vec::new(),
            retired_leftovers: Vec::new(),
        };
        let replaced = report(&outcome(Some(EdgeReplacement::Rebuilt)), &layout);
        let sentence = "The edge is replaced because this development build changed it.";
        let said: Vec<&Line> = replaced
            .iter()
            .filter(|line| line.text.contains("edge"))
            .collect();
        assert_eq!(said.len(), 1, "{replaced:?}");
        assert_eq!(said[0].text, sentence);
        assert_eq!(said[0].stream, Stream::Output);

        let kept = report(&outcome(None), &layout);
        assert!(
            kept.iter().all(|line| !line.text.contains("edge")),
            "{kept:?}"
        );
    }
}
