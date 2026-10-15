//! Tests of the audit line: its readers, its payload bounds and the golden lines that freeze it.

use super::*;
use crate::governance::Capability;
use crate::language::outcome::Outcome;
use crate::language::self_check::golden::{check_golden_with, golden_text, repository_path};

/// The environment variable that rewrites the audit line goldens instead of comparing them.
const UPDATE_VARIABLE: &str = "GHOSTLIGHT_AUDIT_GOLDEN_UPDATE";

/// A navigate that succeeded, relative to the repository root.
const SUCCESS_GOLDEN: &str = "tests/golden/audit/success.json";

/// A click that policy refused, relative to the repository root.
const REFUSAL_GOLDEN: &str = "tests/golden/audit/refusal.json";

/// A two-step flow: one child receipt, then the parent, relative to the repository root.
const FLOW_GOLDEN: &str = "tests/golden/audit/flow.json";

/// The time every golden line carries.
const GOLDEN_TIME_MS: u64 = 1_790_000_000_000;

/// The connection every golden line carries.
const GOLDEN_CONNECTION: &str = "connection_00000000000000000000000000000001";

/// The authority every golden line carries.
const GOLDEN_AUTHORITY: &str = "authority_00000000000000000000000000000001";

/// The key an unknown-field test adds to every object of a line.
const FUTURE_FIELD: &str = "future_field";

fn sample_record() -> AuditRecord {
    AuditRecord::now(
        "invocation_x",
        "workspace_x",
        "browser_fill_form",
        CapabilitySet::READ.union(CapabilitySet::WRITE),
        "authority_x",
        Decision::permitted(),
        "succeeded",
        "applied",
        &crate::language::outcome::Outcome::TextRead {
            words: 3,
            host: None,
        }
        .audit(),
        1200,
    )
}

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

#[test]
fn audit_record_has_no_payload_fields() {
    let record = sample_record().with_observation(Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        count: Some(3),
        width: Some(1280),
        height: Some(720),
    });
    let value = serde_json::to_value(record).unwrap();
    let mut found = Vec::new();
    keys(&value, &mut found);
    // The walk must reach nested keys, or it would pass any payload hidden one level down.
    assert!(found.contains(&"observed".to_owned()));
    assert!(
        found.contains(&"host".to_owned()),
        "the walk stops at the top"
    );
    for forbidden in [
        "url",
        "text",
        "content",
        "selector",
        "value",
        "screenshot",
        "dialog",
        "path",
        "query",
    ] {
        assert!(
            !found.contains(&forbidden.to_owned()),
            "the record grew a {forbidden} field"
        );
    }
}

#[test]
fn new_audit_records_serialize_the_complete_set_without_a_highest_capability() {
    let value = serde_json::to_value(sample_record()).unwrap();
    assert_eq!(value["capabilities"], serde_json::json!(["read", "write"]));
    assert!(value.get("capability").is_none());
}

#[test]
fn audit_reads_lines_written_with_peer_observation_without_reemitting_it() {
    use crate::intake::session::IntakeChannel;

    let connection = format!("connection_{}", uuid::Uuid::new_v4().simple());
    let mut encoded = serde_json::to_value(sample_record()).unwrap();
    encoded["peer_image"] = "client.exe".into();
    encoded["provenance"] = serde_json::json!({
        "connection_id": connection,
        "observed_at_ms": 7,
        "channel": "mcp",
        "peer": { "state": "observed", "executable": "client.exe" },
        "signature": "not_checked",
    });
    let record: AuditRecord =
        serde_json::from_value(encoded).expect("lines with peer observation stay readable");
    let provenance = record.provenance.as_ref().expect("provenance survives");
    assert_eq!(provenance.channel, IntakeChannel::Mcp);
    assert_eq!(provenance.observed_at_ms, Some(7));
    assert_eq!(
        provenance.connection_id.as_deref(),
        Some(connection.as_str())
    );

    let rewritten = serde_json::to_value(&record).unwrap();
    let mut found = Vec::new();
    keys(&rewritten, &mut found);
    for retired in ["peer_image", "peer", "signature", "executable"] {
        assert!(
            !found.contains(&retired.to_owned()),
            "the record re-emitted {retired}"
        );
    }
}

#[test]
fn audit_reads_unrecognized_failure_payloads_without_reemitting_them() {
    for unrecognized in [
        serde_json::json!({"reason":"browser_primitive_failed","detail":"PRIVATE_EXCEPTION"}),
        serde_json::json!({"steps":[{"result":{"text":"PRIVATE_READ"}}]}),
        serde_json::json!({"reason":"PRIVATE_REASON"}),
        serde_json::json!(["PRIVATE_ARRAY"]),
    ] {
        let mut encoded = serde_json::to_value(sample_record()).unwrap();
        encoded["refusal_facts"] = unrecognized;
        let record: AuditRecord = serde_json::from_value(encoded)
            .expect("a line with unknown failure facts stays readable");
        assert!(!serde_json::to_string(&record).unwrap().contains("PRIVATE_"));
        assert_eq!(record.invocation, "invocation_x");
        assert_eq!(
            record.capabilities,
            CapabilitySet::READ.union(CapabilitySet::WRITE)
        );
    }
}

#[test]
fn audit_reads_lines_that_carry_the_removed_attention_state() {
    let mut encoded = serde_json::to_value(sample_record()).unwrap();
    encoded["allowed"] = serde_json::json!(false);
    encoded["reason"] = serde_json::json!("runtime_attention");
    encoded["status"] = serde_json::json!("attention_required");
    encoded["refusal_facts"] = serde_json::json!({"reason":"attention_required"});
    encoded["composition"] = serde_json::json!({
        "counts": {
            "total": 3, "succeeded": 1, "failed": 0, "blocked": 0, "cancelled": 0,
            "attention_required": 1, "unknown": 0, "not_started": 0, "not_run": 1
        },
        "effects": {"none": 1, "applied": 1, "partial": 0, "unknown": 0},
        "stopped": true,
        "issue": {"step": 2, "cause": "attention_required"}
    });
    let mut line = serde_json::to_vec(&encoded).unwrap();
    line.push(b'\n');
    let Some(crate::audit::AuditLine::Receipt(record)) =
        crate::audit::read_line(&mut line.as_slice()).unwrap()
    else {
        panic!("a line written by the attention state must still read as a receipt");
    };
    assert!(!record.allowed);
    assert_eq!(record.reason, ReasonCode::RuntimeHold);
    assert_eq!(record.status, "attention_required");
    assert_eq!(record.refusal_facts, None);
    let progress = record.composition.expect("the account survives");
    assert_eq!(progress.counts.total, 3);
    assert_eq!(progress.counts.succeeded, 1);
    assert_eq!(progress.counts.not_run, 1);
    let rewritten = serde_json::to_string(&*record).unwrap();
    assert!(!rewritten.contains("runtime_attention"));
    assert!(!rewritten.contains("attention_required\":1"));
}

#[test]
fn typed_audit_failures_round_trip_with_policy_and_measurements() {
    use crate::language::outcome::Refusal;
    let record = AuditRecord::now(
        "invocation_failure",
        "workspace_failure",
        "browser_execute",
        Capability::Execute,
        "authority_failure",
        Decision::permitted(),
        "unknown",
        "unknown",
        &Refusal::DeadlineExpired {
            before_dispatch: false,
            timeout_adjustable: true,
        }
        .audit(),
        123,
    )
    .with_observation(Observed {
        host: Some("example.com".into()),
        ..Observed::default()
    });
    let encoded = serde_json::to_string(&record).unwrap();
    assert_eq!(
        serde_json::from_str::<AuditRecord>(&encoded).unwrap(),
        record
    );
    assert_eq!(
        record.refusal_facts,
        Some(crate::language::audit::AuditRefusal::DeadlineExpired {
            before_dispatch: false
        })
    );
}

/// The record has exactly one URL-shaped field and it is a host.
///
/// What the executor puts in it is guarded where it is extracted, in `work`; this pins that
/// there is nowhere else for the rest of a URL to travel.
#[test]
fn an_observation_has_one_place_for_a_host_and_none_for_the_rest_of_a_url() {
    let record = sample_record().with_observation(Observed {
        host: Some("example.com".into()),
        readiness: Some("complete".into()),
        count: Some(3),
        width: Some(1280),
        height: Some(720),
    });
    let encoded = serde_json::to_value(&record).unwrap();
    let observed = encoded["observed"].as_object().unwrap();
    assert_eq!(observed["host"], "example.com");
    let mut text: Vec<&str> = observed
        .iter()
        .filter(|(_, value)| value.is_string())
        .map(|(key, _)| key.as_str())
        .collect();
    text.sort_unstable();
    assert_eq!(
        text,
        ["host", "readiness"],
        "an observation grew another field a URL could travel in"
    );
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

/// The connection evidence a record carries, as the service attaches it.
fn golden_connection() -> crate::provenance::ConnectionEvidence {
    crate::provenance::ConnectionEvidence::new(
        "golden client",
        crate::intake::session::IntakeChannel::Mcp,
    )
}

/// An all-open snapshot, the decision for `url` and the evidence it left.
fn permitted(
    requirements: CapabilitySet,
    url: &str,
) -> (AuthoritySnapshot, Decision, evidence::PermissionTrace) {
    let snapshot = crate::governance::GovernanceFacade::new(None, None).snapshot();
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
        "succeeded",
        "none",
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
    .with_browser(Some("Chromium".into()));
    record.permissions = trace;
    fixed(record, 412)
}

fn click_refused_by_policy() -> AuditRecord {
    let home =
        std::env::temp_dir().join(format!("ghostlight-audit-golden-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&home).unwrap();
    let policy = home.join("policy.json");
    std::fs::write(
        &policy,
        r#"{"schema":4,"name":"research","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
    )
    .unwrap();
    let snapshot = crate::governance::GovernanceFacade::new(Some(policy), None).snapshot();
    std::fs::remove_dir_all(&home).unwrap();
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
            reason: crate::language::outcome::BlockedReason::Capability,
            host: Some("example.com".into()),
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

#[test]
fn the_success_line_matches_its_golden() {
    check_golden_with(
        SUCCESS_GOLDEN,
        &golden_text(&navigate_succeeded()),
        UPDATE_VARIABLE,
        "the audit line",
    );
}

#[test]
fn the_refusal_line_matches_its_golden() {
    check_golden_with(
        REFUSAL_GOLDEN,
        &golden_text(&click_refused_by_policy()),
        UPDATE_VARIABLE,
        "the audit line",
    );
}

#[test]
fn the_flow_lines_match_their_golden() {
    check_golden_with(
        FLOW_GOLDEN,
        &golden_text(&flow_ran()),
        UPDATE_VARIABLE,
        "the audit line",
    );
}

/// Every line a golden file holds, as the JSON values the service wrote.
fn golden_lines(relative: &str) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(repository_path(relative)).unwrap();
    match serde_json::from_str::<serde_json::Value>(&text).unwrap() {
        serde_json::Value::Array(lines) => lines,
        line => vec![line],
    }
}

/// Add one unknown key to the value and to every object inside it.
fn with_future_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.values_mut().for_each(with_future_fields);
            object.insert(FUTURE_FIELD.into(), serde_json::json!({"payload": 1}));
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(with_future_fields),
        _ => {}
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

#[test]
fn a_line_with_unknown_fields_at_every_level_reads_as_the_same_record() {
    for golden in [SUCCESS_GOLDEN, REFUSAL_GOLDEN, FLOW_GOLDEN] {
        for line in golden_lines(golden) {
            let expected = read(&line);
            let mut extended = line.clone();
            with_future_fields(&mut extended);
            assert_ne!(extended, line, "{golden}: the extension changed nothing");
            let record = read(&extended);
            assert_eq!(record, expected, "{golden}");
            assert!(
                !serde_json::to_string(&record)
                    .unwrap()
                    .contains(FUTURE_FIELD),
                "{golden}: an unknown field was written back"
            );
            assert_eq!(serde_json::to_value(&record).unwrap(), line, "{golden}");
        }
    }
}

#[test]
fn a_line_written_before_audit_v2_reads_and_shows_in_history() {
    let mut line = golden_lines(SUCCESS_GOLDEN).remove(0);
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

    let path = std::env::temp_dir().join(format!(
        "ghostlight-audit-earlier-{}.jsonl",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, format!("{line}\n")).unwrap();
    let projection = WorkbenchProjection::default();
    projection.load_history(&path).unwrap();
    std::fs::remove_file(path).unwrap();

    assert_eq!(projection.audit_health().unreadable_entries, 0);
    let history = crate::workbench::WorkbenchFacade::new(
        projection,
        crate::workspace::WorkspaceStore::default(),
        crate::governance::GovernanceFacade::new(None, None),
        std::sync::Arc::new(crate::browser::RelayBrowserPort::new(
            "audit-record-test".into(),
            crate::browser::adapter::chromium::protocol4::assets::page_assets(),
        )),
        crate::diagnostics::DiagnosticsHub::for_tests(),
    )
    .snapshot()
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
}
