// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Deterministic local policy validation, explanation, and audit-free simulation.

use std::fs;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::{managed, manifest, migrate, GovernanceFacade};
use crate::language::{migration, policy as words};

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
        _ => Err(anyhow!(words::USAGE)),
    }
}

/// Run one policy inspection without starting a browser or writing audit.
pub fn run(command: &Command, out: &mut impl Write) -> Result<()> {
    match command {
        Command::Validate(path) => writeln!(out, "{}", words::valid(&load(path)?))?,
        Command::Explain(path) => {
            for line in words::explanation(&load(path)?) {
                writeln!(out, "{line}")?;
            }
        }
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
    match migrate::migrate_file(path, migrate::Access::Kept) {
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

fn simulate(policy: &Path, audit: &Path, out: &mut impl Write) -> Result<()> {
    let manifest = load(policy)?;
    let facade = GovernanceFacade::new(Some(policy.to_path_buf()));
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
                writeln!(out, "{}", words::history_gap(gap.unconfirmed_receipts))?;
                continue;
            }
            crate::audit::AuditLine::Unreadable => {
                writeln!(out, "{}", words::unreadable_line(index))?;
                continue;
            }
            crate::audit::AuditLine::Empty => continue,
        };
        records += 1;
        let requirements = record.capabilities;
        let decision = snapshot.replay(
            record.channel,
            requirements,
            record.observed.host.as_deref(),
        );
        if !decision.allowed {
            denied += 1;
            let line = words::would_deny(&words::WouldDeny {
                line: index,
                tool: &record.tool,
                requirements: &requirements.label(),
                host: record.observed.host.as_deref(),
                rule: decision.policy_rule().unwrap_or(decision.reason.as_str()),
                denial_id: decision.denial_id().as_deref(),
            });
            writeln!(out, "{line}")?;
        }
    }
    writeln!(out, "{}", words::simulated(records, &manifest, denied))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{run, words, Command};
    use crate::governance::{
        manifest, AuthoritySnapshot, CapabilitySet, Decision, GovernanceFacade, PolicyRule,
    };
    use crate::intake::session::IntakeChannel;
    use crate::scratch::Scratch;

    /// What `policy simulate` prints for `policy` over the history `lines`, with the policy as
    /// enforcement would read it.
    fn simulate(policy: &str, lines: &str) -> (String, manifest::Manifest, AuthoritySnapshot) {
        let path = Scratch::file("policy.json");
        let audit = Scratch::file("audit.jsonl");
        fs::write(&path, policy).unwrap();
        fs::write(&audit, lines).unwrap();
        let mut out = Vec::new();
        run(
            &Command::Simulate {
                policy: path.to_path_buf(),
                audit: audit.to_path_buf(),
            },
            &mut out,
        )
        .unwrap();
        let enforced = GovernanceFacade::new(Some(path.to_path_buf()))
            .snapshot()
            .as_ref()
            .clone();
        (
            String::from_utf8(out).unwrap(),
            manifest::parse(policy, "draft").unwrap(),
            enforced,
        )
    }

    /// The line simulate prints for a recorded `browser_click` on example.com that `decision`
    /// refuses under `rule`.
    fn click_denied(line: usize, decision: Decision, rule: PolicyRule) -> String {
        assert_eq!(decision.policy_rule(), Some(rule.as_str()));
        words::would_deny(&words::WouldDeny {
            line,
            tool: "browser_click",
            requirements: &CapabilitySet::ACTION.label(),
            host: Some("example.com"),
            rule: rule.as_str(),
            denial_id: decision.denial_id().as_deref(),
        })
    }

    /// 9c705f86 (arch-1 P3, G-06), ADR-0121 Decision 3 and docs/trust/faq.md ("previews a
    /// candidate policy against recorded audit"): `policy simulate` reads recorded audit lines,
    /// the documented golden ones included, and decides each as enforcement would: on its own
    /// channel first, then its capabilities on its host. A line that names no channel is MCP work.
    #[test]
    fn simulate_decides_each_recorded_line_as_enforcement_would() {
        let mut goldens = String::new();
        for golden in ["success", "refusal"] {
            let text = fs::read_to_string(crate::language::self_check::golden::repository_path(
                &format!("tests/golden/audit/{golden}.json"),
            ))
            .unwrap();
            let line: serde_json::Value = serde_json::from_str(&text).unwrap();
            goldens.push_str(&format!("{line}\n"));
        }
        let read_only = r#"{"schema":4,"name":"read only","version":"4","grants":[{"id":"research","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#;
        let (said, draft, enforced) = simulate(read_only, &goldens);
        let refused = enforced.authorize_landing(CapabilitySet::ACTION, "https://example.com");
        assert_eq!(
            said,
            format!(
                "{}\n{}\n",
                click_denied(2, refused, PolicyRule::Capability),
                words::simulated(2, &draft, 1)
            )
        );

        let mcp_off = r#"{"schema":4,"name":"no agents","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}],"config":[{"key":"channels.mcp.enabled","value":false,"level":"mandatory"}]}"#;
        let line = |channel: &str| {
            format!(
                r#"{{"timestamp_ms":1,"invocation":"i","workspace":"w","tool":"browser_click","capabilities":["action"],"authority":"a","allowed":true,"reason":"permitted","status":"succeeded","effect":"applied","observed":{{"host":"example.com"}}{channel}}}"#
            )
        };
        let lines = [
            line(r#","channel":"mcp""#),
            line(r#","channel":"cli""#),
            line(""),
        ]
        .join("\n");
        let (said, draft, enforced) = simulate(mcp_off, &lines);
        let (refused, _) = enforced
            .channel_refusal(IntakeChannel::Mcp)
            .expect("the draft turns MCP off");
        assert_eq!(
            said,
            format!(
                "{}\n{}\n{}\n",
                click_denied(1, refused, PolicyRule::Channel),
                click_denied(3, refused, PolicyRule::Channel),
                words::simulated(3, &draft, 2)
            )
        );
    }
}
