// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic local policy validation, explanation, and audit-free simulation.

use std::fs;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::{managed, manifest, migrate, GovernanceFacade};
use crate::language::{capability_map, migration};

/// Every `ghostlight policy` subcommand, in help order.
///
/// This is the list the shell completions offer after `policy`; a test holds them to it, and
/// another holds [`parse`] to accepting each one.
pub const SUBCOMMANDS: &[&str] = &[
    "validate", "migrate", "explain", "simulate", "keygen", "pubkey", "sign", "publish",
];

/// One local policy inspection command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Parse and validate one strict schema-4 manifest.
    Validate(PathBuf),
    /// Render one manifest and the exact product capability map.
    Explain(PathBuf),
    /// Replay content-minimized audit records through the production decision engine.
    Simulate {
        /// Candidate policy manifest.
        policy: PathBuf,
        /// Existing Ghostlight audit JSONL.
        audit: PathBuf,
    },
    /// Rewrite a schema-3 manifest as schema 4 where the meaning is kept.
    Migrate(PathBuf),
    /// Create keys or signed organization policy bundles without browser work.
    Author(managed::cli::Command),
}

/// Parse arguments after the policy command name.
pub fn parse(arguments: &[String]) -> Result<Command> {
    match arguments {
        [action, policy] if action == "validate" => Ok(Command::Validate(policy.into())),
        [action, policy] if action == "explain" => Ok(Command::Explain(policy.into())),
        [action, policy] if action == "migrate" => Ok(Command::Migrate(policy.into())),
        [action, policy, audit] if action == "simulate" => Ok(Command::Simulate {
            policy: policy.into(),
            audit: audit.into(),
        }),
        [action, ..] if matches!(action.as_str(), "keygen" | "pubkey" | "sign" | "publish") => {
            managed::cli::parse(arguments).map(Command::Author)
        }
        _ => Err(anyhow!(
            "usage: ghostlight policy <validate|explain|migrate> <policy.json>\n       ghostlight policy simulate <policy.json> <audit.jsonl>\n       ghostlight policy <keygen|pubkey|sign|publish> ..."
        )),
    }
}

/// Run one policy inspection without starting a browser or writing audit.
pub fn run(command: &Command, out: &mut impl Write) -> Result<()> {
    match command {
        Command::Validate(path) => {
            let policy = load(path)?;
            writeln!(
                out,
                "Valid schema-4 policy: {} {} ({})",
                policy.name,
                policy.version,
                short_hash(&policy.hash)
            )?;
        }
        Command::Explain(path) => explain(&load(path)?, out)?,
        Command::Migrate(path) => migrate(path, out)?,
        Command::Simulate { policy, audit } => simulate(policy, audit, out)?,
        Command::Author(command) => managed::cli::run(command, out)?,
    }
    Ok(())
}

fn load(path: &Path) -> Result<manifest::Manifest> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read policy {}", path.display()))?;
    manifest::parse(&text, &path.display().to_string()).map_err(anyhow::Error::new)
}

fn migrate(path: &Path, out: &mut impl Write) -> Result<()> {
    match migrate::migrate_file(path) {
        Ok(None) => writeln!(out, "{}", migration::already_current(path))?,
        Ok(Some((backup, moved))) => {
            writeln!(
                out,
                "{}",
                migration::migrated(path, &backup, &moved.removed)
            )?;
        }
        Err(migrate::MigrateError::Refused(reasons)) => {
            return Err(anyhow!(migration::kept(path, &reasons)));
        }
        Err(error) => return Err(anyhow!(error)),
    }
    Ok(())
}

fn explain(policy: &manifest::Manifest, out: &mut impl Write) -> Result<()> {
    writeln!(out, "Policy: {} {}", policy.name, policy.version)?;
    writeln!(out, "Identity: {}", short_hash(&policy.hash))?;
    if let Some(identity) = &policy.identity {
        if let Some(principal) = &identity.principal {
            writeln!(out, "Principal: {principal}")?;
        }
        if let Some(groups) = &identity.groups {
            if !groups.is_empty() {
                writeln!(out, "Groups: {}", groups.join(", "))?;
            }
        }
    }
    if policy.grants.is_empty() {
        writeln!(
            out,
            "Grants: none. Governed browser work is denied by default."
        )?;
    } else {
        writeln!(out, "Grants:")?;
        for grant in &policy.grants {
            let allowed = grant.allowed_set().label();
            let hosts = if grant.hosts.allow.is_empty() {
                "no hosts".into()
            } else {
                grant.hosts.allow.join(", ")
            };
            write!(out, "  {}: {allowed} on {hosts}", grant.id)?;
            if !grant.hosts.deny.is_empty() {
                write!(out, "; except {}", grant.hosts.deny.join(", "))?;
            }
            writeln!(out)?;
            if let Some(description) = &grant.description {
                writeln!(out, "    {description}")?;
            }
        }
    }
    if !policy.config.is_empty() {
        writeln!(out, "Settings:")?;
        for setting in &policy.config {
            writeln!(
                out,
                "  {} = {} ({})",
                setting.key,
                setting.value,
                setting.level.as_str()
            )?;
        }
    }
    writeln!(out, "Capability map:")?;
    for entry in capability_map::DIRECTORY {
        let variant = entry
            .variant
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        writeln!(
            out,
            "  {}{}: {} -- {}",
            entry.tool,
            variant,
            entry.requirements.label(),
            entry.description
        )?;
    }
    Ok(())
}

fn simulate(policy: &Path, audit: &Path, out: &mut impl Write) -> Result<()> {
    let manifest = load(policy)?;
    let facade = GovernanceFacade::new(Some(policy.to_path_buf()), None);
    let snapshot = facade.snapshot();
    let file = fs::File::open(audit).with_context(|| format!("read audit {}", audit.display()))?;
    let mut records = 0_u64;
    let mut denied = 0_u64;
    let mut reader = BufReader::new(file);
    let mut index = 0;
    while let Some(line) = crate::audit::read_line(&mut reader)? {
        index += 1;
        let record = match line {
            crate::audit::AuditLine::Receipt(record) => record,
            crate::audit::AuditLine::Gap(gap) => {
                writeln!(
                    out,
                    "History gap: {} receipts were not confirmed saved.",
                    gap.unconfirmed_receipts
                )?;
                continue;
            }
            crate::audit::AuditLine::Unreadable => {
                writeln!(out, "Unreadable history entry at line {index}; skipped.")?;
                continue;
            }
            crate::audit::AuditLine::Empty => continue,
        };
        records += 1;
        let requirements = record.capabilities;
        let decision = decide_record(&snapshot, &record);
        if !decision.allowed {
            denied += 1;
            let host = record.observed.host.as_deref().unwrap_or("no host");
            writeln!(
                out,
                "Would deny line {}: {} [{}] on {} -- {}{}",
                index,
                record.tool,
                requirements.label(),
                host,
                decision.policy_rule().unwrap_or(decision.reason.as_str()),
                decision
                    .denial_id()
                    .map(|id| format!(" ({id})"))
                    .unwrap_or_default()
            )?;
        }
    }
    writeln!(
        out,
        "Simulated {records} record(s) against {} {}: {denied} would be denied.",
        manifest.name, manifest.version
    )?;
    Ok(())
}

/// Decide one recorded request as the candidate would have: by its channel first, as the executor
/// does, then by its capabilities and host.
fn decide_record(
    snapshot: &super::AuthoritySnapshot,
    record: &crate::audit::AuditRecord,
) -> super::Decision {
    if let Some((decision, _)) = record
        .channel
        .and_then(|channel| snapshot.channel_refusal(channel))
    {
        return decision;
    }
    let requirements = record.capabilities;
    record.observed.host.as_deref().map_or_else(
        || snapshot.authorize_requirements(requirements),
        |host| snapshot.authorize_landing(requirements, &format!("https://{host}")),
    )
}

fn short_hash(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{parse, run, Command, SUBCOMMANDS};
    use crate::scratch::Scratch;

    /// Each listed subcommand is one the parser accepts, and the list names every one it accepts.
    #[test]
    fn every_listed_subcommand_parses() {
        let samples: &[&[&str]] = &[
            &["validate", "policy.json"],
            &["migrate", "policy.json"],
            &["explain", "policy.json"],
            &["simulate", "policy.json", "audit.jsonl"],
            &["keygen", "keys"],
            &["pubkey", "ed25519.seed"],
            &[
                "sign",
                "policy.json",
                "--ed25519-seed",
                "s",
                "--sequence",
                "1",
            ],
            &[
                "publish",
                "policy.json",
                "--ed25519-seed",
                "s",
                "--source",
                "policy.json",
            ],
        ];
        let named: Vec<&str> = samples.iter().map(|sample| sample[0]).collect();
        assert_eq!(named, SUBCOMMANDS);
        for sample in samples {
            let arguments: Vec<String> = sample.iter().map(|word| (*word).to_owned()).collect();
            if let Err(error) = parse(&arguments) {
                panic!("policy {} does not parse: {error}", sample[0]);
            }
        }
    }

    #[test]
    fn commands_are_small_and_unambiguous() {
        assert_eq!(
            parse(&["validate".into(), "policy.json".into()]).unwrap(),
            Command::Validate("policy.json".into())
        );
        assert_eq!(
            parse(&[
                "simulate".into(),
                "policy.json".into(),
                "audit.jsonl".into()
            ])
            .unwrap(),
            Command::Simulate {
                policy: "policy.json".into(),
                audit: "audit.jsonl".into()
            }
        );
        assert!(parse(&["guess".into()]).is_err());
    }

    #[test]
    fn explain_and_simulate_share_the_production_policy_vocabulary() {
        let policy = Scratch::file("policy.json");
        let audit = Scratch::file("audit.jsonl");
        fs::write(
            &policy,
            r#"{"schema":4,"name":"read only","version":"4","grants":[{"id":"research","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        fs::write(
            &audit,
            r#"{"timestamp_ms":1,"invocation":"i","workspace":"w","tool":"browser_click","capabilities":["action"],"authority":"a","allowed":true,"reason":"permitted","status":"succeeded","effect":"applied","observed":{"host":"example.com"}}"#,
        )
        .unwrap();

        let mut explanation = Vec::new();
        run(&Command::Explain(policy.to_path_buf()), &mut explanation).unwrap();
        let explanation = String::from_utf8(explanation).unwrap();
        assert!(explanation.contains("research: read on example.com"));
        assert!(explanation.contains("browser_fill_form (submit): read + action + write"));

        let mut simulation = Vec::new();
        run(
            &Command::Simulate {
                policy: policy.to_path_buf(),
                audit: audit.to_path_buf(),
            },
            &mut simulation,
        )
        .unwrap();
        let simulation = String::from_utf8(simulation).unwrap();
        assert!(simulation.contains("Would deny line 1: browser_click [action]"));
        assert!(simulation.contains("1 would be denied"));
    }

    #[test]
    fn simulate_refuses_the_recorded_requests_of_a_channel_the_candidate_turns_off() {
        let policy = Scratch::file("policy.json");
        let audit = Scratch::file("audit.jsonl");
        fs::write(
            &policy,
            r#"{"schema":4,"name":"no agents","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
        let line = |channel: &str| {
            format!(
                r#"{{"timestamp_ms":1,"invocation":"i","workspace":"w","tool":"browser_click","capabilities":["action"],"authority":"a","allowed":true,"reason":"permitted","status":"succeeded","effect":"applied","observed":{{"host":"example.com"}},"channel":"{channel}"}}"#
            )
        };
        let lines = [line("mcp"), line("cli")].join("\n");
        fs::write(&audit, lines).unwrap();

        let mut simulation = Vec::new();
        run(
            &Command::Simulate {
                policy: policy.to_path_buf(),
                audit: audit.to_path_buf(),
            },
            &mut simulation,
        )
        .unwrap();
        let simulation = String::from_utf8(simulation).unwrap();
        assert!(
            simulation
                .contains("Would deny line 1: browser_click [action] on example.com -- channel"),
            "{simulation}"
        );
        assert!(!simulation.contains("Would deny line 2"), "{simulation}");
        assert!(simulation.contains("1 would be denied"), "{simulation}");
    }

    #[test]
    fn simulate_replays_the_golden_audit_lines() {
        let policy = Scratch::file("policy.json");
        let audit = Scratch::file("audit.jsonl");
        fs::write(
            &policy,
            r#"{"schema":4,"name":"read only","version":"4","grants":[{"id":"research","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        let mut lines = String::new();
        for golden in ["success", "refusal"] {
            let text = fs::read_to_string(crate::language::self_check::golden::repository_path(
                &format!("tests/golden/audit/{golden}.json"),
            ))
            .unwrap();
            let line: serde_json::Value = serde_json::from_str(&text).unwrap();
            lines.push_str(&format!(
                "{line}
"
            ));
        }
        fs::write(&audit, lines).unwrap();

        let mut simulation = Vec::new();
        run(
            &Command::Simulate {
                policy: policy.to_path_buf(),
                audit: audit.to_path_buf(),
            },
            &mut simulation,
        )
        .unwrap();
        let simulation = String::from_utf8(simulation).unwrap();
        assert!(simulation.contains("Would deny line 2: browser_click [action] on example.com"));
        assert!(simulation.contains("Simulated 2 record(s)"));
        assert!(simulation.contains("1 would be denied"));
    }
}
