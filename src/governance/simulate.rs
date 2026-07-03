//! Policy simulation: replay recorded audit events through a candidate manifest (ADR-0020
//! commitment 3, g17; the classification step updated for ADR-0022 Decision 8).
//!
//! Because the audit flight recorder ships before enforcement (ADR-0018), an organization can
//! baseline real agent traffic in observe mode, then test a candidate manifest against actual
//! recorded usage instead of guessing what will break. This module replays each evaluable
//! record through the SAME pure decision function live enforcement uses
//! ([`crate::governance::enforcement::check_call`]) -- there is no second, parallel evaluator
//! here, so the preview cannot disagree with production by construction.
//!
//! Two honest limitations, stated plainly rather than glossed over: an audit record carries
//! only the parser-normalized host (shared format doc section 6.1), never the full URL, so a
//! `scheme/<scheme>` denial or anything path-dependent cannot be reproduced from a record --
//! only a resolved host or "no page" (`domain: null`) round-trips. And simulation only ever
//! covers what was actually recorded; it says nothing about traffic that never happened.
//!
//! Mode (manifest-level or per-grant) is ignored entirely: simulation always reports the
//! enforce view, since the whole point is finding out what WOULD be blocked before flipping a
//! manifest from observe to enforce. Any deny-shaped verdict (`Decision::Deny` or
//! `Decision::ShadowDeny`) counts as a would-deny; the sacred-domain list is always empty
//! (a candidate manifest simulation must be reproducible in CI, where no local
//! `content.security.sacred_domains` exists), so rule `sacred` can never appear in a report.
//!
//! Old (`rw`-era) audit files remain replayable: this module never trusted the recorded `rw`
//! (or, before that, `decision`/`grant_id`) value; it always re-derives the bound capability
//! requirement set from `requires_fn` and replays the action fresh.

use std::collections::BTreeMap;
use std::path::Path;

use crate::governance::enforcement::check_call;
use crate::governance::manifest::document::{parse_manifest, Manifest, ManifestError};
use crate::governance::ports::{
    Capability, Decision, Denial, EffectiveMode, GoverningResource, HostRuleOutcome,
};

/// Why [`run_simulate`] could not produce an outcome. A malformed LINE inside the replay file
/// is never an error (it is a not-evaluable record, counted and reported); only failure to
/// read a file at all, or a manifest that fails validation, stops the run.
#[derive(Debug, thiserror::Error)]
pub enum SimulateError {
    #[error("failed to read manifest '{path}': {source}")]
    ManifestIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("failed to read replay file '{path}': {source}")]
    ReplayIo {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// The rendered report plus the one number the exit code depends on.
pub struct SimulateOutcome {
    /// The full plain-ASCII report (Required behavior section 5 format), ready to print.
    pub report: String,
    /// Count of would-deny events (exit code 2 when nonzero, per `src/main.rs`).
    pub would_deny: u64,
}

/// Load the candidate manifest and the replay file, run the simulation, and render the report.
/// `replay_path_display` is the path exactly as given on the command line (the report echoes
/// it verbatim, per Required behavior section 5). `domain_pattern_valid` is the manifest
/// loader's real host-pattern checker; `requires_fn`/`evaluate_host` are the same function
/// pointers live enforcement injects into [`check_call`] -- all three the "known integration
/// point" shape used everywhere else in `governance/`, so this domain-agnostic core module
/// never names `browser::`/`transport::` directly (the a7 arch-test).
pub fn run_simulate(
    manifest_path: &Path,
    replay_path: &Path,
    domain_pattern_valid: fn(&str) -> bool,
    requires_fn: fn(&str, Option<&str>) -> Option<&'static [Capability]>,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
) -> Result<SimulateOutcome, SimulateError> {
    let manifest_text =
        std::fs::read_to_string(manifest_path).map_err(|e| SimulateError::ManifestIo {
            path: manifest_path.display().to_string(),
            source: e,
        })?;
    let manifest = parse_manifest(
        &manifest_text,
        &manifest_path.display().to_string(),
        domain_pattern_valid,
    )?;

    let replay_text =
        std::fs::read_to_string(replay_path).map_err(|e| SimulateError::ReplayIo {
            path: replay_path.display().to_string(),
            source: e,
        })?;

    let report = simulate_lines(
        &manifest,
        numbered_lines(&replay_text),
        requires_fn,
        evaluate_host,
    );
    let would_deny = report.would_deny;
    let text = render_report(&manifest, &replay_path.display().to_string(), &report);
    Ok(SimulateOutcome {
        report: text,
        would_deny,
    })
}

/// Split replay text into 1-based `(line_number, line)` pairs, tolerating a trailing `\r` per
/// line (Required behavior section 4).
fn numbered_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| (i + 1, line.strip_suffix('\r').unwrap_or(line)))
}

/// One would-deny group's accumulated state (Required behavior section 5): a count and the
/// group's (necessarily constant) denial id.
struct GroupInfo {
    count: u64,
    denial_id: String,
}

/// The pure report core (Required behavior section 2): counts, deny groups, and the
/// not-evaluable list, computed from an iterator of numbered lines so it is unit-testable
/// without touching a file. `BTreeMap` keyed by the group tuple for deterministic,
/// byte-wise-ascending grouping; no `HashMap` iteration order reaches the output.
struct Report {
    would_allow: u64,
    would_deny: u64,
    not_evaluable: u64,
    groups: BTreeMap<(String, String, String, String), GroupInfo>,
    not_evaluable_lines: Vec<(usize, String)>,
}

fn simulate_lines<'a>(
    manifest: &Manifest,
    lines: impl Iterator<Item = (usize, &'a str)>,
    requires_fn: fn(&str, Option<&str>) -> Option<&'static [Capability]>,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
) -> Report {
    let mut would_allow = 0u64;
    let mut would_deny = 0u64;
    let mut not_evaluable = 0u64;
    let mut groups: BTreeMap<(String, String, String, String), GroupInfo> = BTreeMap::new();
    let mut not_evaluable_lines = Vec::new();

    for (line_number, line) in lines {
        if line.trim().is_empty() {
            continue;
        }
        match evaluate_line(manifest, line, requires_fn, evaluate_host) {
            LineOutcome::Allow => would_allow += 1,
            LineOutcome::Deny {
                domain,
                tool,
                denial,
            } => {
                would_deny += 1;
                let key = (
                    denial.grant_id.clone().unwrap_or_else(|| "-".to_string()),
                    domain.unwrap_or_else(|| "-".to_string()),
                    tool,
                    denial.rule.clone(),
                );
                groups
                    .entry(key)
                    .and_modify(|g| g.count += 1)
                    .or_insert(GroupInfo {
                        count: 1,
                        denial_id: denial.denial_id.clone(),
                    });
            }
            LineOutcome::NotEvaluable(reason) => {
                not_evaluable += 1;
                not_evaluable_lines.push((line_number, reason));
            }
        }
    }

    Report {
        would_allow,
        would_deny,
        not_evaluable,
        groups,
        not_evaluable_lines,
    }
}

enum LineOutcome {
    Allow,
    Deny {
        domain: Option<String>,
        tool: String,
        denial: Denial,
    },
    NotEvaluable(String),
}

/// Evaluate one non-empty replay line per the Required behavior section 4 bucket table, in
/// the exact order specified there. The recorded `capability`/`decision`/`grant_id`/
/// `denial_id` (or, on an old rw-era line, `rw`) and every other field besides
/// `tool`/`action`/`domain` are read by nobody: simulate replays the action under the
/// CANDIDATE manifest, never trusts or compares against the original decision. Old rw-era
/// audit lines replay identically to capability-era ones, since only `tool`/`action`/`domain`
/// are ever read.
fn evaluate_line(
    manifest: &Manifest,
    line: &str,
    requires_fn: fn(&str, Option<&str>) -> Option<&'static [Capability]>,
    evaluate_host: fn(&str, &[String], &[String]) -> HostRuleOutcome,
) -> LineOutcome {
    let value: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return LineOutcome::NotEvaluable("malformed json".to_string()),
    };
    let Some(obj) = value.as_object() else {
        return LineOutcome::NotEvaluable("malformed json".to_string());
    };

    let Some(tool) = obj.get("tool").and_then(|v| v.as_str()) else {
        return LineOutcome::NotEvaluable("missing field: tool".to_string());
    };

    let domain: Option<String> = match obj.get("domain") {
        None => return LineOutcome::NotEvaluable("missing field: domain".to_string()),
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(_) => return LineOutcome::NotEvaluable("missing field: domain".to_string()),
    };

    let action: Option<String> = match obj.get("action") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(_) => return LineOutcome::NotEvaluable("missing field: action".to_string()),
    };

    let Some(requires) = requires_fn(tool, action.as_deref()) else {
        return LineOutcome::NotEvaluable(if tool != "computer" {
            format!("unknown tool: {tool}")
        } else if let Some(a) = &action {
            format!("unknown action: {a}")
        } else {
            "computer action missing".to_string()
        });
    };

    let resource = match &domain {
        Some(host) => GoverningResource::Resource(host.clone()),
        None => GoverningResource::None,
    };
    // Mode is ignored entirely (module doc): `manifest_mode: None`, `config_mode: Enforce`
    // means the mode switch can only ever fire on a per-grant `mode` override, which the
    // match arm below collapses into the same would-deny bucket regardless.
    let decision = check_call(
        &manifest.grants,
        tool,
        action.as_deref(),
        requires,
        &resource,
        &manifest.hash,
        evaluate_host,
        None,
        EffectiveMode::Enforce,
    );
    match decision {
        Decision::Allow { .. } => LineOutcome::Allow,
        Decision::Deny(denial) | Decision::ShadowDeny(denial) => LineOutcome::Deny {
            domain,
            tool: tool.to_string(),
            denial,
        },
    }
}

/// Render the exact section-5 report text: blocks separated by one blank line, exactly one
/// trailing newline, `\n` only.
fn render_report(manifest: &Manifest, replay_path_display: &str, report: &Report) -> String {
    let mut blocks = vec![format!(
        "policy simulate\nmanifest: {} {} sha256={}\nreplay: {replay_path_display}",
        manifest.name, manifest.version, manifest.hash
    )];
    blocks.push(format!(
        "total actions: {}\nwould allow: {}\nwould deny: {}\nnot evaluable: {}",
        report.would_allow + report.would_deny + report.not_evaluable,
        report.would_allow,
        report.would_deny,
        report.not_evaluable
    ));

    if report.would_deny > 0 {
        let mut lines = vec!["would-deny groups (grant, domain, tool):".to_string()];
        for ((grant, domain, tool, rule), info) in &report.groups {
            lines.push(format!(
                "count={} grant={grant} domain={domain} tool={tool} rule={rule} denial={}",
                info.count, info.denial_id
            ));
        }
        blocks.push(lines.join("\n"));
    }

    if report.not_evaluable > 0 {
        let mut lines = vec!["not evaluable:".to_string()];
        for (line_number, reason) in &report.not_evaluable_lines {
            lines.push(format!("line {line_number}: {reason}"));
        }
        blocks.push(lines.join("\n"));
    }

    blocks.push(if report.would_deny == 0 {
        "result: no would-denies (exit 0)".to_string()
    } else {
        format!("result: {} would-denies (exit 2)", report.would_deny)
    });

    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::{Grant, HostRules};

    fn stub_requires(tool: &str, action: Option<&str>) -> Option<&'static [Capability]> {
        match (tool, action) {
            ("computer", Some("screenshot")) => Some(&[Capability::Read]),
            ("computer", Some("left_click")) => Some(&[Capability::Action]),
            ("read_page", None) => Some(&[Capability::Read]),
            ("navigate", None) => Some(&[Capability::Read]),
            ("javascript_tool", None) => Some(&[Capability::Execute]),
            ("update_plan", None) => Some(&[]),
            _ => None,
        }
    }

    fn stub_evaluate_host(host: &str, allow: &[String], deny: &[String]) -> HostRuleOutcome {
        fn matches(pattern: &str, host: &str) -> bool {
            pattern == "*"
                || match pattern.strip_prefix("*.") {
                    Some(suffix) => host.ends_with(&format!(".{suffix}")),
                    None => pattern == host,
                }
        }
        let allowed = allow.iter().any(|p| matches(p, host));
        let denied = deny.iter().any(|p| matches(p, host));
        match (allowed, denied) {
            (false, false) => HostRuleOutcome::Unmatched,
            (true, false) => HostRuleOutcome::Allowed,
            (false, true) | (true, true) => HostRuleOutcome::Denied,
        }
    }

    fn sample_manifest(grants: Vec<Grant>) -> Manifest {
        Manifest {
            schema: 3,
            name: "t".to_string(),
            version: "1".to_string(),
            mode: None,
            identity: None,
            grants,
            config: Vec::new(),
            hash: "h".repeat(64),
        }
    }

    fn grant(id: &str, allow_hosts: &[&str], allowed: &[Capability]) -> Grant {
        Grant {
            id: id.to_string(),
            hosts: HostRules {
                allow: allow_hosts.iter().map(|d| d.to_string()).collect(),
                deny: Vec::new(),
            },
            allowed: allowed.to_vec(),
            description: None,
            mode: None,
        }
    }

    fn run(manifest: &Manifest, lines: &[&str]) -> Report {
        let numbered: Vec<(usize, &str)> =
            lines.iter().enumerate().map(|(i, l)| (i + 1, *l)).collect();
        simulate_lines(
            manifest,
            numbered.into_iter(),
            stub_requires,
            stub_evaluate_host,
        )
    }

    #[test]
    fn empty_replay_is_all_zeros() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[]);
        assert_eq!(r.would_allow, 0);
        assert_eq!(r.would_deny, 0);
        assert_eq!(r.not_evaluable, 0);
    }

    #[test]
    fn whitespace_only_lines_are_not_counted() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &["", "   ", "\t"]);
        assert_eq!(r.would_allow + r.would_deny + r.not_evaluable, 0);
    }

    #[test]
    fn bucket_table_malformed_json() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &["not json at all"]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "malformed json".to_string())]
        );
    }

    #[test]
    fn bucket_table_non_object_json() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &["[1,2,3]"]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "malformed json".to_string())]
        );
    }

    #[test]
    fn bucket_table_missing_tool() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"domain":null}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "missing field: tool".to_string())]
        );
    }

    #[test]
    fn bucket_table_missing_domain_key() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"read_page"}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "missing field: domain".to_string())]
        );
    }

    #[test]
    fn bucket_table_domain_wrong_type() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"read_page","domain":42}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "missing field: domain".to_string())]
        );
    }

    #[test]
    fn bucket_table_action_wrong_type() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"computer","domain":null,"action":42}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "missing field: action".to_string())]
        );
    }

    #[test]
    fn bucket_table_unknown_tool() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"teleport","domain":null}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "unknown tool: teleport".to_string())]
        );
    }

    #[test]
    fn bucket_table_computer_action_missing() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"computer","domain":null,"action":null}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "computer action missing".to_string())]
        );
    }

    #[test]
    fn bucket_table_computer_unknown_action() {
        let m = sample_manifest(vec![]);
        let r = run(&m, &[r#"{"tool":"computer","domain":null,"action":"fly"}"#]);
        assert_eq!(
            r.not_evaluable_lines,
            vec![(1, "unknown action: fly".to_string())]
        );
    }

    #[test]
    fn bucket_table_evaluable_allow_and_deny() {
        let m = sample_manifest(vec![grant("g", &["example.com"], &[Capability::Read])]);
        let allow = run(&m, &[r#"{"tool":"read_page","domain":"example.com"}"#]);
        assert_eq!(allow.would_allow, 1);
        let deny = run(
            &m,
            &[r#"{"tool":"javascript_tool","domain":"example.com"}"#],
        );
        assert_eq!(deny.would_deny, 1);
    }

    #[test]
    fn totals_arithmetic_holds() {
        let m = sample_manifest(vec![grant("g", &["example.com"], &[Capability::Read])]);
        let r = run(
            &m,
            &[
                r#"{"tool":"read_page","domain":"example.com"}"#,
                r#"{"tool":"navigate","domain":"example.com"}"#,
                "not json",
            ],
        );
        assert_eq!(r.would_allow + r.would_deny + r.not_evaluable, 3);
    }

    #[test]
    fn group_sort_order_dash_entries_sort_first() {
        let m = sample_manifest(vec![grant("g", &["example.com"], &[Capability::Read])]);
        let r = run(
            &m,
            &[
                r#"{"tool":"javascript_tool","domain":"example.com"}"#,
                r#"{"tool":"javascript_tool","domain":null}"#,
            ],
        );
        // "-" (no grant / no domain) sorts before "example.com"/"g" lexicographically.
        let keys: Vec<_> = r.groups.keys().cloned().collect();
        assert!(keys[0].0 == "-" || keys[0].1 == "-", "keys: {keys:?}");
    }

    /// Required test 5 (same-logic pin): calling the pure decision function directly with the
    /// same manifest and call facts a would-deny replay record produces must return the
    /// IDENTICAL `grant_id`/`denial_id` the simulation report groups by. This is what proves
    /// simulate and live enforcement share exactly one code path, not two that happen to agree
    /// today.
    #[test]
    fn simulate_and_the_decision_function_agree_on_the_same_call() {
        let m = sample_manifest(vec![grant(
            "docs-read",
            &["docs.example.com"],
            &[Capability::Read],
        )]);
        let line = r#"{"tool":"javascript_tool","domain":"docs.example.com"}"#;

        let report = run(&m, &[line]);
        let ((grant, domain, tool, rule), info) = report.groups.iter().next().expect("one group");
        assert_eq!(grant, "docs-read");
        assert_eq!(domain, "docs.example.com");
        assert_eq!(tool, "javascript_tool");
        assert_eq!(rule, "capability");

        let direct = check_call(
            &m.grants,
            "javascript_tool",
            None,
            &[Capability::Execute],
            &GoverningResource::Resource("docs.example.com".to_string()),
            &m.hash,
            stub_evaluate_host,
            None,
            EffectiveMode::Enforce,
        );
        match direct {
            Decision::Deny(d) => {
                assert_eq!(d.grant_id.as_deref(), Some(grant.as_str()));
                assert_eq!(d.denial_id, info.denial_id);
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }
}
