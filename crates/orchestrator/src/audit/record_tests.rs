//! Tests of the audit line: its readers, its payload bounds and the golden lines that freeze it.

use super::*;
use crate::language::outcome::Outcome;
use crate::language::self_check::golden::{check_golden_with, golden_text, repository_path};
use crate::work::result::{Effect, Status};

/// The environment variable that rewrites the audit line goldens instead of comparing them.
const UPDATE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_GOLDEN_UPDATE";

/// A navigate that succeeded, relative to the repository root.
const SUCCESS_GOLDEN: &str = "tests/golden/audit/success.json";

/// A click that policy refused, relative to the repository root.
const REFUSAL_GOLDEN: &str = "tests/golden/audit/refusal.json";

/// A two-step flow: one child receipt, then the parent, relative to the repository root.
const FLOW_GOLDEN: &str = "tests/golden/audit/flow.json";

/// A flow step whose arguments never decoded, under an organization's signed policy, relative to
/// the repository root.
const NOT_STARTED_GOLDEN: &str = "tests/golden/audit/not-started.json";

/// The time every golden line carries.
const GOLDEN_TIME_MS: u64 = 1_790_000_000_000;

/// The connection every golden line carries.
const GOLDEN_CONNECTION: &str = "connection_00000000000000000000000000000001";

/// The authority every golden line carries.
const GOLDEN_AUTHORITY: &str = "authority_00000000000000000000000000000001";

/// Every key anywhere in the record, so a payload cannot hide one level down.
fn keys(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, nested) in object {
                found.push(key.clone());
                keys(nested, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                keys(item, found);
            }
        }
        _ => {}
    }
}

/// Fix every value that differs between runs, after the production constructors ran.
fn fixed(mut record: AuditRecord, duration_ms: u64) -> AuditRecord {
    record.timestamp_ms = GOLDEN_TIME_MS;
    record.authority = GOLDEN_AUTHORITY.into();
    record.duration_ms = duration_ms;
    if let Some(provenance) = &mut record.provenance {
        provenance.connection_id = Some(GOLDEN_CONNECTION.into());
        provenance.observed_at_ms = Some(GOLDEN_TIME_MS);
    }
    record
}

/// The connection evidence a record carries, as the service attaches it: an MCP connection whose
/// client claimed to be Claude Code, started by Claude Desktop's installed app.
fn golden_connection() -> crate::provenance::ConnectionEvidence {
    crate::provenance::ConnectionEvidence::new(
        crate::provenance::Attribution {
            source: Some(crate::provenance::Source {
                agent: Some("claude-desktop".into()),
                image: "claude.exe".into(),
            }),
            ..crate::provenance::Attribution::accepted(crate::intake::session::IntakeChannel::Mcp)
        },
        Some("claude-code"),
    )
}

/// An all-open snapshot, the decision for `url` and the evidence it left.
fn permitted(
    requirements: CapabilitySet,
    url: &str,
) -> (AuthoritySnapshot, Decision, evidence::PermissionTrace) {
    let snapshot = crate::governance::GovernanceFacade::new(None)
        .snapshot()
        .as_ref()
        .clone();
    let (decision, check) = snapshot.authorize_with_evidence(requirements, Some(url));
    let mut trace = evidence::PermissionTrace::default();
    trace.record(check);
    (snapshot, decision, trace)
}

fn navigate_succeeded() -> AuditRecord {
    let (snapshot, decision, trace) = permitted(CapabilitySet::READ, "https://example.com/");
    let connection = golden_connection();
    let mut record = AuditRecord::now(
        "invocation_00000000000000000000000000000001",
        "workspace_00000000000000000000000000000001",
        "browser_navigate",
        CapabilitySet::READ,
        snapshot.id(),
        decision,
        &Status::Succeeded.stored_name(),
        &Effect::Applied.stored_name(),
        &Outcome::PageNavigated {
            host: Some("example.com".into()),
        }
        .audit(),
        0,
    )
    .with_provenance(Some(connection.attribution()))
    .with_policy(&snapshot, decision)
    .with_observation(Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        ..Observed::default()
    })
    .with_browser(Some("Chromium".into()))
    .with_browser_id(Some("browser_7d1c0a9e5b2f4c38a1e6d0b9f3a2c5e8".into()));
    record.permissions = trace;
    fixed(record, 412)
}

fn click_refused_by_policy() -> AuditRecord {
    let policy = crate::scratch::Scratch::file("policy.json");
    std::fs::write(
        &policy,
        r#"{"schema":4,"name":"research","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
    )
    .unwrap();
    let snapshot = crate::governance::GovernanceFacade::new(Some(policy.to_path_buf())).snapshot();
    drop(policy);
    let (decision, check) =
        snapshot.authorize_with_evidence(CapabilitySet::ACTION, Some("https://example.com/"));
    assert!(!decision.allowed);
    let connection = golden_connection();
    let mut record = AuditRecord::now(
        "invocation_00000000000000000000000000000002",
        "workspace_00000000000000000000000000000001",
        "browser_click",
        CapabilitySet::ACTION,
        snapshot.id(),
        decision,
        "blocked",
        "none",
        &crate::language::outcome::Refusal::AuthorityBlocked {
            code: crate::governance::ReasonCode::CapabilityDenied,
            host: Some("example.com".into()),
            origin: None,
        }
        .audit(),
        0,
    )
    .with_provenance(Some(connection.attribution()))
    .with_policy(&snapshot, decision)
    .with_observation(Observed {
        host: Some("example.com".into()),
        ..Observed::default()
    })
    .with_browser(Some("Chromium".into()));
    record.permissions.record(check);
    fixed(record, 3)
}

fn flow_ran() -> Vec<AuditRecord> {
    use crate::language::composition::{CompositionProgress, EffectCounts, StepCounts};
    use crate::language::coverage::Coverage;
    use crate::language::history::{CompositionKind, StepReceipt};

    let (snapshot, decision, trace) = permitted(CapabilitySet::READ, "https://example.com/");
    let connection = golden_connection();
    let invocation = "invocation_00000000000000000000000000000003";
    let workspace = "workspace_00000000000000000000000000000001";
    let mut child = AuditRecord::now(
        invocation,
        workspace,
        "browser_read",
        CapabilitySet::READ,
        snapshot.id(),
        decision,
        "succeeded",
        "none",
        &Outcome::TextRead {
            words: 120,
            host: Some("example.com".into()),
        }
        .audit()
        .with_coverage(&Coverage {
            inspected_documents: 1,
            ..Coverage::default()
        }),
        0,
    )
    .with_provenance(Some(connection.attribution()))
    .with_policy(&snapshot, decision)
    .with_observation(Observed {
        host: Some("example.com".into()),
        count: Some(120),
        ..Observed::default()
    })
    .with_browser(Some("Chromium".into()));
    child.step = Some(StepReceipt {
        parent: CompositionKind::Flow,
        position: 1,
        total: 2,
        preparation_failed: false,
    });
    child.permissions = trace;

    let progress = CompositionProgress {
        counts: StepCounts {
            total: 2,
            succeeded: 2,
            ..StepCounts::default()
        },
        effects: EffectCounts {
            none: 1,
            applied: 1,
            ..EffectCounts::default()
        },
        ..CompositionProgress::default()
    };
    let parent = AuditRecord::now(
        invocation,
        workspace,
        CompositionKind::Flow.tool(),
        CapabilitySet::EMPTY,
        snapshot.id(),
        Decision::permitted(),
        "succeeded",
        "applied",
        &Outcome::CompositionRan(progress)
            .audit()
            .with_tools(["browser_read", "browser_click"]),
        0,
    )
    .with_provenance(Some(connection.attribution()))
    .with_browser(Some("Chromium".into()));
    vec![fixed(child, 38), fixed(parent, 951)]
}

/// The line of a flow step that never started, written under an organization's signed policy, so
/// it carries the policy's publish sequence.
fn step_not_started() -> AuditRecord {
    use crate::governance::managed::ManagedPaths;
    use crate::language::history::{CompositionKind, StepReceipt};

    let root = crate::scratch::Scratch::named("audit-golden-managed");
    let snapshot = crate::governance::GovernanceFacade::with_managed_paths(
        None,
        ManagedPaths::signed(
            &root,
            r#"{"schema":4,"name":"org","version":"1","grants":[{"id":"org","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        ),
    )
    .snapshot();
    let connection = golden_connection();
    let record = AuditRecord::not_started(
        "invocation_00000000000000000000000000000004",
        "workspace_00000000000000000000000000000001",
        "browser_read",
        &snapshot,
        connection.attribution(),
        StepReceipt {
            parent: CompositionKind::Flow,
            position: 1,
            total: 2,
            preparation_failed: true,
        },
    );
    assert!(record.policy_seq.is_some(), "the signed policy is in force");
    fixed(record, 0)
}

/// Every line a golden file holds, as the JSON values the service wrote.
fn golden_lines(relative: &str) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(repository_path(relative)).unwrap();
    match serde_json::from_str::<serde_json::Value>(&text).unwrap() {
        serde_json::Value::Array(lines) => lines,
        line => vec![line],
    }
}

/// Read one line through the production reader.
fn read(value: &serde_json::Value) -> AuditRecord {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    let Some(AuditLine::Receipt(record)) = read_line(&mut bytes.as_slice()).unwrap() else {
        panic!("the line did not read as a receipt: {value}");
    };
    *record
}

/// Contract: the audit line is frozen at version 2 and documented for collectors
/// (docs/guides/siem-integration.md). Each kind of line the service writes matches its golden: a
/// navigate that succeeded, a click policy refused, a flow's child and parent, and a flow step
/// that never started under an organization's signed policy.
#[test]
fn every_audit_line_matches_its_documented_golden() {
    for (golden, line) in [
        (SUCCESS_GOLDEN, golden_text(&navigate_succeeded())),
        (REFUSAL_GOLDEN, golden_text(&click_refused_by_policy())),
        (FLOW_GOLDEN, golden_text(&flow_ran())),
        (NOT_STARTED_GOLDEN, golden_text(&step_not_started())),
    ] {
        check_golden_with(golden, &line, UPDATE_VARIABLE, "the audit line");
    }
}

/// ACCEPTANCE CT5 (which names this file), G10 and CR: a line an earlier build wrote still reads
/// and shows in history, and nothing that build wrote and this one retired is written back: the
/// fields before audit v2, observed peer identity, the attention state, arbitrary failure facts
/// a line may carry, and a line written before connection sources or claimed agents were kept.
#[test]
fn a_line_an_earlier_build_wrote_still_reads_without_writing_back_what_it_retired() {
    let golden = golden_lines(SUCCESS_GOLDEN).remove(0);
    let expected = read(&golden);

    // A line written before audit v2.
    let mut line = golden.clone();
    line["capability"] = "read".into();
    line["peer_image"] = "client.exe".into();
    line["policy_observed"] = false.into();
    line["policy_mode"] = "enforce".into();
    line["coverage"] = serde_json::json!({
        "targeted": false, "inspected_documents": 1, "excluded_documents": 0,
        "page_excluded_documents": 0, "unavailable_documents": 0,
        "limited_by_size": false, "masked_regions": 0
    });
    let check = &mut line["permissions"]["checks"][0];
    check["request_restricted"] = false.into();
    check["request_evaluated"] = false.into();
    check["observed"] = false.into();
    let path = crate::scratch::Scratch::file("audit-earlier.jsonl");
    std::fs::write(&path, format!("{line}\n")).unwrap();
    let projection = WorkbenchProjection::default();
    projection.load_history(&path).unwrap();
    drop(path);
    assert_eq!(projection.audit_health().unreadable_entries, 0);
    let history = crate::workbench::WorkbenchFacade::new(
        projection,
        crate::workspace::WorkspaceStore::default(),
        crate::governance::GovernanceFacade::new(None),
        std::sync::Arc::new(crate::browser::RelayBrowserPort::new(
            "audit-record-test".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
    .look()
    .activity()
    .history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].tool, "browser_navigate");
    assert_eq!(history[0].status, "succeeded");
    assert_eq!(history[0].requirements, CapabilitySet::READ);
    let rewritten = serde_json::to_string(&read(&line)).unwrap();
    for retired in [
        "\"capability\"",
        "peer_image",
        "policy_observed",
        "policy_mode",
        "masked_regions",
        "request_restricted",
        "request_evaluated",
    ] {
        assert!(!rewritten.contains(retired), "{retired} was written back");
    }

    // A line written while connections carried observed peer identity.
    let mut line = golden.clone();
    line["peer_image"] = "client.exe".into();
    line["provenance"] = serde_json::json!({
        "connection_id": GOLDEN_CONNECTION,
        "observed_at_ms": 7,
        "channel": "mcp",
        "peer": { "state": "observed", "executable": "client.exe" },
        "signature": "not_checked",
    });
    let record = read(&line);
    let provenance = record.provenance.as_ref().expect("provenance survives");
    assert_eq!(provenance.channel, IntakeChannel::Mcp);
    assert_eq!(provenance.observed_at_ms, Some(7));
    assert_eq!(provenance.connection_id.as_deref(), Some(GOLDEN_CONNECTION));
    let mut found = Vec::new();
    keys(&serde_json::to_value(&record).unwrap(), &mut found);
    for retired in ["peer_image", "peer", "signature", "executable"] {
        assert!(
            !found.contains(&retired.to_owned()),
            "the record re-emitted {retired}"
        );
    }

    // A line written while the attention state existed.
    let mut line = golden.clone();
    line["allowed"] = serde_json::json!(false);
    line["reason"] = serde_json::json!("runtime_attention");
    line["status"] = serde_json::json!("attention_required");
    line["refusal_facts"] = serde_json::json!({"reason":"attention_required"});
    line["composition"] = serde_json::json!({
        "counts": {
            "total": 3, "succeeded": 1, "failed": 0, "blocked": 0, "cancelled": 0,
            "attention_required": 1, "unknown": 0, "not_started": 0, "not_run": 1
        },
        "effects": {"none": 1, "applied": 1, "partial": 0, "unknown": 0},
        "stopped": true,
        "issue": {"step": 2, "cause": "attention_required"}
    });
    let record = read(&line);
    assert!(!record.allowed);
    assert_eq!(record.reason, ReasonCode::RuntimeHold);
    assert_eq!(record.status, "attention_required");
    assert_eq!(record.refusal_facts, None);
    let progress = record.composition.as_ref().expect("the account survives");
    assert_eq!(progress.counts.total, 3);
    assert_eq!(progress.counts.succeeded, 1);
    assert_eq!(progress.counts.not_run, 1);
    let rewritten = serde_json::to_string(&record).unwrap();
    assert!(!rewritten.contains("runtime_attention"));
    assert!(!rewritten.contains("attention_required\":1"));

    // A line carrying failure facts this build does not recognize.
    for unrecognized in [
        serde_json::json!({"reason":"browser_primitive_failed","detail":"PRIVATE_EXCEPTION"}),
        serde_json::json!({"steps":[{"result":{"text":"PRIVATE_READ"}}]}),
        serde_json::json!({"reason":"PRIVATE_REASON"}),
        serde_json::json!(["PRIVATE_ARRAY"]),
    ] {
        let mut line = golden.clone();
        line["refusal_facts"] = unrecognized;
        let record = read(&line);
        assert!(!serde_json::to_string(&record).unwrap().contains("PRIVATE_"));
        assert_eq!(record.invocation, expected.invocation);
        assert_eq!(record.capabilities, expected.capabilities);
    }

    // A line written before connection sources, and one before claimed agents, were kept.
    for kept_later in ["source", "claimed_agent"] {
        let mut line = golden.clone();
        line["provenance"]
            .as_object_mut()
            .unwrap()
            .remove(kept_later)
            .expect("the golden line carries it");
        let record = read(&line);
        let provenance = record.provenance.as_ref().expect("provenance survives");
        assert_eq!(provenance.connection_id.as_deref(), Some(GOLDEN_CONNECTION));
        match kept_later {
            "source" => assert_eq!(provenance.source, None),
            _ => assert_eq!(provenance.claimed_agent, None),
        }
        assert_eq!(serde_json::to_value(&record).unwrap(), line, "{kept_later}");
    }
}
