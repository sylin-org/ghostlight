//! The `ghostlight diagnostics` command: read-only insight into the shared process
//! diagnostics log, plus the two person-facing actuations (ADR-0145).
//!
//! Everything here reads local files or actuates the marker; nothing demand-starts the
//! authority, following the doctor rule.

use anyhow::bail;
use ghostlight_bridge::diagnostics::{
    marker_path, prune_directory, resolve, set_marker, Activation, Component, ENV_DIR,
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
    let runtime = ghostlight_bridge::runtime::runtime_file();
    match command {
        Command::Path => run_path(&runtime),
        Command::Show {
            last,
            component,
            op,
            json,
        } => run_show(
            &runtime,
            last.as_deref(),
            component.as_deref(),
            op.as_deref(),
            *json,
        ),
        Command::Prune => run_prune(&runtime),
        Command::On => {
            let activation = set_marker(&runtime, true)?;
            report(&activation, &marker_path(&runtime));
            Ok(())
        }
        Command::Off => {
            let activation = set_marker(&runtime, false)?;
            report(&activation, &marker_path(&runtime));
            Ok(())
        }
    }
}

fn effective(runtime: &std::path::Path) -> Activation {
    let explicit = std::env::var_os(ENV_DIR).map(PathBuf::from);
    resolve(explicit, runtime)
}

/// The folder this command should act on: the active directory, or the default one when off,
/// so retained logs stay readable and prunable after diagnostics are turned off.
fn target_directory(runtime: &std::path::Path, activation: &Activation) -> PathBuf {
    activation
        .directory()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ghostlight_bridge::diagnostics::default_directory(runtime))
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

fn run_path(runtime: &std::path::Path) -> anyhow::Result<()> {
    let activation = effective(runtime);
    report(&activation, &marker_path(runtime));
    println!(
        "Log folder: {}",
        target_directory(runtime, &activation).display()
    );
    Ok(())
}

fn run_prune(runtime: &std::path::Path) -> anyhow::Result<()> {
    let activation = effective(runtime);
    let directory = target_directory(runtime, &activation);
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
    runtime: &std::path::Path,
    last: Option<&str>,
    component: Option<&str>,
    op: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let activation = effective(runtime);
    let off = matches!(activation, Activation::Off);
    let directory = target_directory(runtime, &activation);
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
    let mut records: Vec<serde_json::Value> = Vec::new();
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
            let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
                malformed += 1;
                continue;
            };
            if let Some(cutoff) = cutoff {
                let ts = record
                    .get("ts_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                if (ts as u128) < cutoff {
                    continue;
                }
            }
            if let Some(component) = component {
                if record.get("component").and_then(serde_json::Value::as_str) != Some(component) {
                    continue;
                }
            }
            if let Some(op) = op {
                if record.get("op").and_then(serde_json::Value::as_str) != Some(op) {
                    continue;
                }
            }
            records.push(record);
        }
    }
    records.sort_by_key(|record| {
        record
            .get("ts_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }
    if records.is_empty() {
        println!("No diagnostics records match in {}.", directory.display());
        return Ok(());
    }
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    for record in &records {
        let ts = record
            .get("ts_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let moment = time::OffsetDateTime::from_unix_timestamp_nanos((ts as i128) * 1_000_000)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            .to_offset(offset);
        println!(
            "{:02}-{:02} {:02}:{:02}:{:02}.{:03} {:<17} {:5} {:28} {}{}",
            moment.month() as u8,
            moment.day(),
            moment.hour(),
            moment.minute(),
            moment.second(),
            moment.millisecond(),
            record
                .get("component")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?"),
            record
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?"),
            record
                .get("event")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?"),
            record
                .get("op")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            record
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
        );
    }
    if malformed > 0 {
        println!("({malformed} unparseable lines skipped)");
    }
    for component in [
        Component::Orchestrator,
        Component::McpConnector,
        Component::BrowserConnector,
    ] {
        let present = records.iter().any(|record| {
            record.get("component").and_then(serde_json::Value::as_str) == Some(component.as_str())
        });
        if !present {
            println!(
                "No {} records in range; its diagnostics were probably off.",
                component.as_str()
            );
        }
    }
    Ok(())
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
