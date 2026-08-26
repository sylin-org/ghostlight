// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic local policy validation, explanation, and audit-free simulation.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::{managed, manifest, AuditRecord, CapabilitySet, GovernanceFacade};
use crate::language::{capability_map, RequestRestrictions};

/// One local policy inspection command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// Parse and validate one strict schema-3 manifest.
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
    /// Create keys or signed organization policy bundles without browser work.
    Author(managed::cli::Command),
}

/// Parse arguments after the policy command name.
pub fn parse(arguments: &[String]) -> Result<Command> {
    match arguments {
        [action, policy] if action == "validate" => Ok(Command::Validate(policy.into())),
        [action, policy] if action == "explain" => Ok(Command::Explain(policy.into())),
        [action, policy, audit] if action == "simulate" => Ok(Command::Simulate {
            policy: policy.into(),
            audit: audit.into(),
        }),
        [action, ..] if matches!(action.as_str(), "keygen" | "pubkey" | "sign" | "publish") => {
            managed::cli::parse(arguments).map(Command::Author)
        }
        _ => Err(anyhow!(
            "usage: ghostlight policy <validate|explain> <policy.json>\n       ghostlight policy simulate <policy.json> <audit.jsonl>\n       ghostlight policy <keygen|pubkey|sign|publish> ..."
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
                "Valid schema-3 policy: {} {} ({})",
                policy.name,
                policy.version,
                short_hash(&policy.hash)
            )?;
        }
        Command::Explain(path) => explain(&load(path)?, out)?,
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

fn explain(policy: &manifest::Manifest, out: &mut impl Write) -> Result<()> {
    writeln!(out, "Policy: {} {}", policy.name, policy.version)?;
    writeln!(out, "Identity: {}", short_hash(&policy.hash))?;
    writeln!(out, "Mode: {}", policy.mode.unwrap_or_default().as_str())?;
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
            let mode = grant.mode.or(policy.mode).unwrap_or_default();
            writeln!(out, "; mode {}", mode.as_str())?;
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
    let snapshot = facade.snapshot(&RequestRestrictions::default());
    let file = fs::File::open(audit).with_context(|| format!("read audit {}", audit.display()))?;
    let mut records = 0_u64;
    let mut denied = 0_u64;
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.with_context(|| format!("read audit line {}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: AuditRecord = serde_json::from_str(&line)
            .with_context(|| format!("decode audit line {}", index + 1))?;
        records += 1;
        let requirements = record.requirements();
        let decision = decide_record(&snapshot, requirements, record.observed.host.as_deref());
        if !decision.allowed || decision.observed {
            denied += 1;
            let host = record.observed.host.as_deref().unwrap_or("no host");
            writeln!(
                out,
                "Would deny line {}: {} [{}] on {} -- {}{}",
                index + 1,
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

fn decide_record(
    snapshot: &super::AuthoritySnapshot,
    requirements: CapabilitySet,
    host: Option<&str>,
) -> super::Decision {
    host.map_or_else(
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

    use super::{parse, run, Command};

    fn temporary(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-policy-inspection-{name}-{}",
            uuid::Uuid::new_v4()
        ))
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
        let policy = temporary("policy.json");
        let audit = temporary("audit.jsonl");
        fs::write(
            &policy,
            r#"{"schema":3,"name":"read only","version":"4","grants":[{"id":"research","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        fs::write(
            &audit,
            r#"{"timestamp_ms":1,"invocation":"i","workspace":"w","tool":"browser_click","capabilities":["action"],"authority":"a","allowed":true,"reason":"permitted","status":"succeeded","effect":"applied","observed":{"host":"example.com"}}"#,
        )
        .unwrap();

        let mut explanation = Vec::new();
        run(&Command::Explain(policy.clone()), &mut explanation).unwrap();
        let explanation = String::from_utf8(explanation).unwrap();
        assert!(explanation.contains("research: read on example.com"));
        assert!(explanation.contains("browser_fill_form (submit): read + action + write"));

        let mut simulation = Vec::new();
        run(
            &Command::Simulate {
                policy: policy.clone(),
                audit: audit.clone(),
            },
            &mut simulation,
        )
        .unwrap();
        let simulation = String::from_utf8(simulation).unwrap();
        assert!(simulation.contains("Would deny line 1: browser_click [action]"));
        assert!(simulation.contains("1 would be denied"));
        let _ = fs::remove_file(policy);
        let _ = fs::remove_file(audit);
    }
}
