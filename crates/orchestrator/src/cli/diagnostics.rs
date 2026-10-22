//! The `ghostlight diagnostics` command: read-only insight into the shared process
//! diagnostics log, plus the two person-facing actuations (ADR-0145).
//!
//! Everything here reads local files or actuates the marker; nothing demand-starts the
//! service, following the doctor rule.

use anyhow::bail;
use ghostlight_bridge::diagnostics::{
    marker_path, prune_directory, resolve, set_marker, Activation, Component, Record, EDGE_LOGS,
    ENV_DIR,
};
use std::path::{Path, PathBuf};

/// One `ghostlight diagnostics` invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Path,
    Show {
        last: Option<String>,
        component: Option<String>,
        op: Option<String>,
        json: bool,
    },
    Prune,
    On,
    Off,
}

/// Parse the words after `ghostlight diagnostics`.
pub fn parse(arguments: &[String]) -> anyhow::Result<Command> {
    let usage = "usage: ghostlight diagnostics <path|show|prune|on|off>";
    let Some(action) = arguments.first() else {
        bail!(usage);
    };
    match action.as_str() {
        "path" => {
            if arguments.len() != 1 {
                bail!("usage: ghostlight diagnostics path");
            }
            Ok(Command::Path)
        }
        "prune" => {
            if arguments.len() != 1 {
                bail!("usage: ghostlight diagnostics prune");
            }
            Ok(Command::Prune)
        }
        "on" | "off" => {
            if arguments.len() != 1 {
                bail!("usage: ghostlight diagnostics {action}");
            }
            if action == "on" {
                Ok(Command::On)
            } else {
                Ok(Command::Off)
            }
        }
        "show" => {
            let mut last = None;
            let mut component = None;
            let mut op = None;
            let mut json = false;
            let mut remaining = arguments[1..].iter();
            while let Some(argument) = remaining.next() {
                let mut take_value = |name: &str| -> anyhow::Result<String> {
                    remaining.next().cloned().ok_or_else(|| {
                        anyhow::anyhow!("ghostlight diagnostics show {name} needs a value")
                    })
                };
                match argument.as_str() {
                    "--json" => json = true,
                    "--last" => last = Some(take_value("--last")?),
                    "--component" => component = Some(take_value("--component")?),
                    "--op" => op = Some(take_value("--op")?),
                    other => bail!("unknown diagnostics show option {other}"),
                }
            }
            Ok(Command::Show {
                last,
                component,
                op,
                json,
            })
        }
        other => bail!("unknown diagnostics action {other}; {usage}"),
    }
}

/// Run one parsed command.
pub fn run(command: &Command) -> anyhow::Result<()> {
    let home = ghostlight_relay::home::home()?;
    match command {
        Command::Path => run_path(&home),
        Command::Show {
            last,
            component,
            op,
            json,
        } => run_show(
            &home,
            last.as_deref(),
            component.as_deref(),
            op.as_deref(),
            *json,
        ),
        Command::Prune => run_prune(&home),
        Command::On => {
            let activation = set_marker(&home, true)?;
            report(&activation, &marker_path(&home));
            Ok(())
        }
        Command::Off => {
            let activation = set_marker(&home, false)?;
            report(&activation, &marker_path(&home));
            Ok(())
        }
    }
}

fn effective(home: &std::path::Path) -> Activation {
    let explicit = std::env::var_os(ENV_DIR).map(PathBuf::from);
    resolve(explicit, home)
}

/// The folder this command should act on: the active directory, or the default one when off,
/// so retained logs stay readable and prunable after diagnostics are turned off.
fn target_directory(home: &std::path::Path, activation: &Activation) -> PathBuf {
    activation
        .directory()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ghostlight_bridge::diagnostics::default_directory(home))
}

fn report(activation: &Activation, marker: &std::path::Path) {
    match activation {
        Activation::Explicit { directory } => println!(
            "Diagnostics are on (explicit). Log directory: {}",
            directory.display()
        ),
        Activation::Marker { directory } => println!(
            "Diagnostics are on (marker). Log directory: {}",
            directory.display()
        ),
        Activation::Off => println!(
            "Diagnostics are off. The {} variable or a {} file turns them on.",
            ENV_DIR,
            marker.display()
        ),
    }
}

fn run_path(home: &std::path::Path) -> anyhow::Result<()> {
    let activation = effective(home);
    report(&activation, &marker_path(home));
    println!(
        "Log folder: {}",
        target_directory(home, &activation).display()
    );
    Ok(())
}

fn run_prune(home: &std::path::Path) -> anyhow::Result<()> {
    let activation = effective(home);
    let directory = target_directory(home, &activation);
    let report = prune_directory(&directory);
    println!(
        "Pruned {} log files; kept {} files ({} bytes) in {}.",
        report.deleted,
        report.kept,
        report.kept_bytes,
        directory.display()
    );
    Ok(())
}

fn run_show(
    home: &std::path::Path,
    last: Option<&str>,
    component: Option<&str>,
    op: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let activation = effective(home);
    let off = matches!(activation, Activation::Off);
    let directory = target_directory(home, &activation);
    if off {
        println!(
            "Diagnostics are off; showing the retained records in {}.",
            directory.display()
        );
    }
    let window = last.map(parse_window).transpose()?;
    let cutoff = window.map(|window| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        now.saturating_sub(window)
    });
    // Each line typed for filtering and display, and as written for `--json`, which prints it whole.
    let mut records: Vec<(Record, serde_json::Value)> = Vec::new();
    let mut malformed = 0usize;
    let entries = std::fs::read_dir(&directory)?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Only diagnostics logs, never audit.jsonl or anything else sharing the folder.
        if ghostlight_bridge::diagnostics::parse_log_name(&name).is_none() {
            continue;
        }
        let content = match std::fs::read_to_string(entry.path()) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for line in content.lines() {
            let Some((record, written)) = serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|written| {
                    serde_json::from_value::<Record>(written.clone())
                        .ok()
                        .map(|record| (record, written))
                })
            else {
                malformed += 1;
                continue;
            };
            if cutoff.is_some_and(|cutoff| u128::from(record.ts_ms) < cutoff) {
                continue;
            }
            if component.is_some_and(|component| record.component != component) {
                continue;
            }
            if op.is_some_and(|op| record.op.as_deref() != Some(op)) {
                continue;
            }
            records.push((record, written));
        }
    }
    records.sort_by_key(|(record, _)| record.ts_ms);
    if json {
        let written: Vec<&serde_json::Value> = records.iter().map(|(_, written)| written).collect();
        println!("{}", serde_json::to_string_pretty(&written)?);
        return Ok(());
    }
    if records.is_empty() {
        println!("No diagnostics records match in {}.", directory.display());
        return Ok(());
    }
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    for (record, _) in &records {
        let moment =
            time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(record.ts_ms) * 1_000_000)
                .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
                .to_offset(offset);
        println!(
            "{:02}-{:02} {:02}:{:02}:{:02}.{:03} {:<17} {:5} {:28} {}",
            moment.month() as u8,
            moment.day(),
            moment.hour(),
            moment.minute(),
            moment.second(),
            moment.millisecond(),
            or_unknown(&record.component),
            record.level.as_deref().unwrap_or_default(),
            or_unknown(&record.event),
            op_and_detail(record.op.as_deref().unwrap_or(""), &record.detail),
        );
    }
    if malformed > 0 {
        println!("({malformed} unparseable lines skipped)");
    }
    // A component the filter left out is absent by request, not because its diagnostics were off.
    let wanted = component;
    for component in writers().filter(|known| wanted.is_none_or(|wanted| wanted == *known)) {
        let present = records
            .iter()
            .any(|(record, _)| record.component == component);
        if !present {
            println!("No {component} records in range; its diagnostics were probably off.");
        }
    }
    Ok(())
}

/// The name of every component that writes into the diagnostics directory: the service, then
/// each edge log as `ghostlight_relay::log` spells it.
fn writers() -> impl Iterator<Item = &'static str> {
    std::iter::once(Component::Orchestrator.as_str())
        .chain(EDGE_LOGS.into_iter().map(|log| log.as_str()))
}

/// A name a line left out shows as "?".
fn or_unknown(name: &str) -> &str {
    if name.is_empty() {
        "?"
    } else {
        name
    }
}

/// The end of a shown line: the operation, then the detail, with a space between when there are
/// both. An invocation id and a tool name must not read as one word.
fn op_and_detail(op: &str, detail: &str) -> String {
    if op.is_empty() || detail.is_empty() {
        format!("{op}{detail}")
    } else {
        format!("{op} {detail}")
    }
}

/// Parse a window like `30s`, `10m`, `2h`, or `1d` into milliseconds.
fn parse_window(text: &str) -> anyhow::Result<u128> {
    let digits: String = text
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    let unit = &text[digits.len()..];
    let value: u128 = digits.parse().map_err(|_| {
        anyhow::anyhow!("cannot parse --last window {text}; use a number with s, m, h, or d")
    })?;
    if value == 0 {
        bail!("--last must be greater than zero");
    }
    let seconds: u128 = match unit {
        "s" | "" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        other => bail!("unknown --last unit {other}; use s, m, h, or d"),
    };
    Ok(value * seconds * 1000)
}
