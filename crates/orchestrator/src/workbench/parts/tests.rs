//! Tests of the projection: what each audience sees of each part.

use std::collections::BTreeSet;

use super::*;
use crate::browser::adapter::RuntimeControlState;
use crate::governance::evidence::{PermissionCheck, PermissionTrace};
use crate::governance::{
    CapabilitySet, GovernanceFacade, ManagedPolicyContact, ManagedPolicyFreshness,
    ManagedPolicySource,
};
use crate::install::HarnessState;
use crate::intake::session::IntakeChannel;
use crate::language::audit_health::{Storage, StorageFailure};
use crate::language::composition::{
    CompositionProgress, EffectCounts, StepCause, StepCounts, StepIssue,
};
use crate::language::coverage::Coverage;
use crate::language::outcome::Observed;
use crate::language::workbench::ConnectionCounts;
use crate::provenance::ConnectionDetails;
use crate::workbench::{
    ConfigurationSummary, DiagnosticSeverity, HistoryStep, OperationPhase, OverviewSummary,
    StepHistoryState,
};

const EXCLUDED: &str = "private-frame.invalid";
const NEVER_TOUCH: &str = "never-touch.invalid";
const POLICY: &str = r#"{"schema":4,"name":"mine","version":"1","grants":[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["never-touch.invalid"],"level":"mandatory"}]}"#;
const HOME: &str = "/home/ann";

fn temp_policy() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ghostlight-parts-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&path, POLICY).unwrap();
    path
}

fn details() -> ConnectionDetails {
    ConnectionDetails {
        connection_id: Some("connection_1".into()),
        reported_application: Some("Claude Code".into()),
        channel: IntakeChannel::Mcp,
        explanation: "Declared by the client.".into(),
    }
}

fn history_item(invocation: &str, check: PermissionCheck) -> HistoryItem {
    HistoryItem {
        storage: Storage::Unconfirmed,
        storage_detail: "Some step history could not be saved.".into(),
        timestamp_ms: 5,
        invocation: invocation.into(),
        workspace: "workspace_1".into(),
        tool: "browser_read".into(),
        capability: "read".into(),
        requirements: CapabilitySet::READ,
        allowed: true,
        reason: "authority".into(),
        policy_tier: Some("user".into()),
        grant_id: Some("read".into()),
        denial_id: Some("denial_1".into()),
        status: "succeeded".into(),
        effect: "applied".into(),
        summary: "Read the page.".into(),
        duration_ms: 12,
        observed: Observed {
            host: Some("example.com".into()),
            readiness: Some("ready".into()),
            count: Some(1),
            width: Some(2),
            height: Some(3),
            rebound: false,
        },
        channel: Some(IntakeChannel::Mcp),
        provenance: Some(details()),
        browser: Some("Chrome".into()),
        complete: true,
        composition: Some(CompositionProgress {
            counts: StepCounts::default(),
            effects: EffectCounts::default(),
            stopped: true,
            issue: Some(StepIssue {
                step: 1,
                cause: StepCause::PolicyBlocked,
            }),
        }),
        steps: Vec::new(),
        permissions: PermissionTrace {
            checks: vec![check],
            truncated: false,
        },
        permission_explanations: vec!["Allowed by your rules.".into()],
    }
}

/// A snapshot with every field filled, so the keys of each part are all the keys it can have.
fn populated() -> (WorkbenchSnapshot, EffectiveAuthority, PathBuf) {
    let path = temp_policy();
    let governance = GovernanceFacade::new(Some(path.clone()), None);
    let (_, check) = governance
        .snapshot()
        .authorize_with_evidence(CapabilitySet::READ, Some("https://example.com/"));
    let mut item = history_item("invocation_1", check.clone());
    item.steps = vec![HistoryStep {
        position: 0,
        tool: Some("browser_read".into()),
        state: StepHistoryState::Recorded,
        record: Some(Box::new(history_item("invocation_2", check))),
    }];
    let snapshot = WorkbenchSnapshot {
        audit_health: AuditHealth {
            failure: Some(StorageFailure::Write),
            unconfirmed_receipts: 1,
            unreadable_entries: 2,
            history_unavailable: true,
        },
        audit_notice: "History could not be saved.".into(),
        document_coverage: vec![HumanCoverage {
            summary: "One frame was excluded.".into(),
            explanation: "Excluded by policy.".into(),
            invocation: "invocation_1".into(),
            coverage: Coverage {
                targeted: true,
                inspected_documents: 1,
                excluded_documents: 1,
                page_excluded_documents: 1,
                unavailable_documents: 1,
                limited_by_size: true,
            },
            excluded_hosts: vec![EXCLUDED.into(), "second.invalid".into()],
            proactive: true,
        }],
        seq: 9,
        generated_at_ms: 10,
        runtime_note: Some("Paused from the tray.".into()),
        service: ServiceSummary {
            version: "2.0.0".into(),
            started_at_ms: 11,
            runtime_state: RuntimeControlState::Held,
        },
        readiness: ReadinessSummary::resolve(&crate::language::readiness::ReadinessFacts {
            audit_required_unavailable: false,
            browser_connected: true,
            session_ended: false,
            paused: false,
            authority_invalid: false,
            working: false,
        }),
        overview: OverviewSummary {
            active_sessions: 1,
            active_operations: 1,
            connected_browsers: 1,
            blocked_in_history: 0,
        },
        sessions: vec![SessionSummary {
            id: "workspace_1".into(),
            client_label: "Claude Code".into(),
            connections: vec![details()],
            channel: IntakeChannel::Mcp,
            leased: true,
            tab_count: 2,
            held_tab_count: 1,
            active_operations: 1,
            browser: Some("Chrome".into()),
        }],
        operations: vec![OperationSummary {
            invocation: "invocation_3".into(),
            workspace: "workspace_1".into(),
            tool: "browser_read".into(),
            activity: "Reading page".into(),
            capability: "read".into(),
            started_at_ms: Some(12),
            phase: OperationPhase::Running,
            provenance: Some(details()),
        }],
        browsers: vec![BrowserInstanceSummary {
            id: "browser_1".into(),
            family: "Chrome".into(),
            adapter_version: Some("1.0.0".into()),
            connected: true,
        }],
        history: vec![item],
        diagnostics: vec![DiagnosticItem {
            id: "service".into(),
            label: "Orchestrator".into(),
            severity: DiagnosticSeverity::Passing,
            detail: "Ghostlight is accepting local connections.".into(),
        }],
        harnesses: vec![HarnessSummary {
            id: "cursor".into(),
            product_id: "cursor".into(),
            name: "Cursor".into(),
            label: "Cursor".into(),
            target: "Cursor".into(),
            icon: "cursor.svg".into(),
            state: HarnessState::NeedsAttention,
            detail: "Something else starts here.".into(),
            evidence: Some("A program named other, with 2 arguments.".into()),
            can_install: false,
            can_fix: true,
            can_uninstall: false,
            can_download: true,
            can_locate: true,
            private_view: true,
            config_path: "/home/ann/.cursor/mcp.json".into(),
            connector_command: "/home/ann/.ghostlight/bin/ghostlight mcp".into(),
            manual_setup: "{\"mcpServers\":{}}".into(),
        }],
        configuration: ConfigurationSummary {
            runtime_state: RuntimeControlState::Held,
            local_policy_configured: true,
            local_policy_active: true,
            local_policy_valid: true,
            managed_authority_configured: true,
            managed_authority_active: true,
            managed_authority_valid: true,
            managed_policy: ManagedPolicyPassport {
                configured: true,
                verified: true,
                freshness: ManagedPolicyFreshness::Fresh,
                sequence: Some(3),
                organization: Some("Example Org".into()),
                rationale: Some("Because.".into()),
                contacts: vec![ManagedPolicyContact {
                    kind: "email".into(),
                    value: "it@example.org".into(),
                    label: Some("IT".into()),
                }],
                source_class: ManagedPolicySource::Https,
                last_success_ms: Some(1),
                last_attempt_ms: Some(2),
            },
            policy: governance.policy_chip(),
        },
        controls: ControlsPart::of(&ControlsFacts {
            runtime_state: RuntimeControlState::Held,
            runtime_note: Some("Paused from the tray."),
            viewing: ViewingFacts::Served {
                watching: true,
                url: "http://127.0.0.1:41234/",
            },
            diagnostics: &DiagnosticsReport {
                layer: "marker",
                directory: Some("/home/ann/.ghostlight/logs".into()),
                used_bytes: 4,
            },
            counts: ConnectionCounts {
                mcp_clients: 1,
                command_line: 0,
                browsers: 1,
            },
            user_home: Some(Path::new(HOME)),
        }),
    };
    (snapshot, governance.effective_authority(), path)
}

/// Every key path in a value, with array elements folded into the name of their array.
fn key_paths(value: &Value) -> BTreeSet<String> {
    fn walk(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    out.insert(path.clone());
                    walk(inner, &path, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, &format!("{prefix}[]"), out);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    walk(value, "", &mut out);
    out
}

fn projection(audience: Audience) -> Projection {
    Projection::with_home(audience, Some(PathBuf::from(HOME)))
}

const STATUS_KEYS: &[&str] = &[
    "audit_health",
    "audit_health.failure",
    "audit_health.history_unavailable",
    "audit_health.unconfirmed_receipts",
    "audit_health.unreadable_entries",
    "audit_notice",
    "configuration",
    "configuration.managed_policy",
    "configuration.managed_policy.configured",
    "configuration.managed_policy.contacts",
    "configuration.managed_policy.contacts[].kind",
    "configuration.managed_policy.contacts[].label",
    "configuration.managed_policy.contacts[].value",
    "configuration.managed_policy.freshness",
    "configuration.managed_policy.last_attempt_ms",
    "configuration.managed_policy.last_success_ms",
    "configuration.managed_policy.organization",
    "configuration.managed_policy.rationale",
    "configuration.managed_policy.sequence",
    "configuration.managed_policy.source_class",
    "configuration.managed_policy.verified",
    "configuration.policy",
    "configuration.policy.detail",
    "configuration.policy.situation",
    "configuration.policy.tone",
    "readiness",
    "readiness.detail",
    "readiness.invites_control",
    "readiness.state",
    "readiness.tone",
    "readiness.word",
    "runtime_note",
    "service",
    "service.runtime_state",
    "service.started_at_ms",
    "service.version",
];

const CONNECTIONS_KEYS: &[&str] = &[
    "browsers",
    "browsers[].adapter_version",
    "browsers[].connected",
    "browsers[].family",
    "browsers[].id",
    "sessions",
    "sessions[].active_operations",
    "sessions[].browser",
    "sessions[].channel",
    "sessions[].client_label",
    "sessions[].connections",
    "sessions[].connections[].channel",
    "sessions[].connections[].connection_id",
    "sessions[].connections[].explanation",
    "sessions[].connections[].reported_application",
    "sessions[].id",
    "sessions[].tab_count",
];

const COVERAGE_KEYS: &[&str] = &[
    "document_coverage",
    "document_coverage[].coverage",
    "document_coverage[].coverage.excluded_documents",
    "document_coverage[].coverage.inspected_documents",
    "document_coverage[].coverage.limited_by_size",
    "document_coverage[].coverage.page_excluded_documents",
    "document_coverage[].coverage.targeted",
    "document_coverage[].coverage.unavailable_documents",
    "document_coverage[].excluded_host_count",
    "document_coverage[].explanation",
    "document_coverage[].invocation",
    "document_coverage[].proactive",
    "document_coverage[].summary",
];

/// What only the window sees of a document's coverage.
const COVERAGE_WINDOW_ONLY: &[&str] = &["document_coverage[].excluded_hosts"];

const OPERATION_KEYS: &[&str] = &[
    "operations",
    "operations[].activity",
    "operations[].capability",
    "operations[].invocation",
    "operations[].phase",
    "operations[].provenance",
    "operations[].provenance.channel",
    "operations[].provenance.connection_id",
    "operations[].provenance.explanation",
    "operations[].provenance.reported_application",
    "operations[].started_at_ms",
    "operations[].tool",
    "operations[].workspace",
];

/// The keys of one history record, relative to the record, without its steps.
const RECORD_KEYS: &[&str] = &[
    "allowed",
    "browser",
    "capability",
    "channel",
    "complete",
    "composition",
    "composition.counts",
    "composition.counts.blocked",
    "composition.counts.cancelled",
    "composition.counts.failed",
    "composition.counts.not_run",
    "composition.counts.not_started",
    "composition.counts.succeeded",
    "composition.counts.total",
    "composition.counts.unknown",
    "composition.effects",
    "composition.effects.applied",
    "composition.effects.none",
    "composition.effects.partial",
    "composition.effects.unknown",
    "composition.issue",
    "composition.issue.cause",
    "composition.issue.step",
    "composition.stopped",
    "denial_id",
    "duration_ms",
    "effect",
    "grant_id",
    "invocation",
    "observed",
    "observed.count",
    "observed.height",
    "observed.host",
    "observed.readiness",
    "observed.width",
    "permission_explanations",
    "permissions",
    "permissions.checks",
    "permissions.checks[].allowed",
    "permissions.checks[].host",
    "permissions.checks[].layers",
    "permissions.checks[].layers[].allowed",
    "permissions.checks[].layers[].grants",
    "permissions.checks[].layers[].tier",
    "permissions.checks[].reason",
    "permissions.checks[].requirements",
    "permissions.truncated",
    "policy_tier",
    "provenance",
    "provenance.channel",
    "provenance.connection_id",
    "provenance.explanation",
    "provenance.reported_application",
    "reason",
    "status",
    "steps",
    "storage",
    "storage_detail",
    "summary",
    "timestamp_ms",
    "tool",
    "workspace",
];

const STEP_KEYS: &[&str] = &[
    "history[].steps[].position",
    "history[].steps[].record",
    "history[].steps[].state",
    "history[].steps[].tool",
];

const INTEGRATION_KEYS: &[&str] = &[
    "harnesses",
    "harnesses[].can_download",
    "harnesses[].can_fix",
    "harnesses[].can_install",
    "harnesses[].can_locate",
    "harnesses[].can_uninstall",
    "harnesses[].config_path",
    "harnesses[].connector_command",
    "harnesses[].detail",
    "harnesses[].evidence",
    "harnesses[].icon",
    "harnesses[].id",
    "harnesses[].label",
    "harnesses[].manual_setup",
    "harnesses[].name",
    "harnesses[].product_id",
    "harnesses[].state",
    "harnesses[].target",
];

const DIAGNOSTIC_KEYS: &[&str] = &[
    "diagnostics",
    "diagnostics[].detail",
    "diagnostics[].id",
    "diagnostics[].label",
    "diagnostics[].severity",
];

const POLICY_KEYS: &[&str] = &[
    "audit",
    "audit.mode",
    "audit.organization_mode",
    "audit.source",
    "browser_startup",
    "browser_startup.decided_by",
    "browser_startup.organization_ceiling",
    "browser_startup.value",
    "capabilities",
    "capabilities[].capability",
    "capabilities[].covers",
    "capabilities[].decided_by",
    "capabilities[].detail",
    "capabilities[].label",
    "capabilities[].state",
    "ceilings",
    "headline",
    "layers",
    "layers[].document",
    "layers[].kind",
    "layers[].path",
    "layers[].policy_name",
    "layers[].rules",
    "layers[].rules[].allow",
    "layers[].rules[].allowed",
    "layers[].rules[].deny",
    "layers[].rules[].description",
    "layers[].rules[].id",
    "layers[].rules[].note",
    "layers[].settings",
    "layers[].settings[].key",
    "layers[].settings[].level",
    "layers[].settings[].value",
    "layers[].title",
    "layers[].version",
    "organization",
    "organization.contacts",
    "organization.contacts[].kind",
    "organization.contacts[].label",
    "organization.contacts[].value",
    "organization.name",
    "organization.statement",
    "organization.url",
    "passport",
    "passport.configured",
    "passport.contacts",
    "passport.freshness",
    "passport.last_attempt_ms",
    "passport.last_success_ms",
    "passport.organization",
    "passport.rationale",
    "passport.sequence",
    "passport.source_class",
    "passport.verified",
    "situation",
    "user_layer",
    "user_layer.blocked_reason",
    "user_layer.editable",
    "user_layer.path",
    "user_layer.source",
];

/// What only the Control policy has.
const POLICY_CONTROL_ONLY: &[&str] = &["etag"];

/// What only a watcher's policy has.
const POLICY_WATCH_ONLY: &[&str] = &["layers[].settings[].count"];

/// The keys every act of the Controls part has.
const AVAILABILITY_KEYS: &[&str] = &["available", "why_not"];

/// The Controls part has the same keys for every audience: only the values of its acts differ.
fn controls_keys() -> BTreeSet<String> {
    let mut keys = expected(&[&[
        "diagnostics",
        "diagnostics.acts",
        "diagnostics.folder",
        "diagnostics.label",
        "diagnostics.layer",
        "diagnostics.on",
        "diagnostics.used_bytes",
        "diagnostics.word",
        "notifications",
        "notifications.acts",
        "notifications.label",
        "notifications.word",
        "runtime",
        "runtime.acts",
        "runtime.label",
        "runtime.note",
        "runtime.state",
        "runtime.word",
        "service",
        "service.word",
        "viewing",
        "viewing.acts",
        "viewing.label",
        "viewing.url",
        "viewing.watch",
        "viewing.word",
        "you",
        "you.context",
        "you.sentence",
        "you.word",
    ]]);
    for act in [
        "diagnostics.acts.reveal_diagnostics",
        "diagnostics.acts.set_diagnostics",
        "notifications.acts.test_notification",
        "runtime.acts.end_session",
        "runtime.acts.hold",
        "runtime.acts.resume",
        "runtime.acts.start_session",
        "viewing.acts.set_watching",
    ] {
        keys.extend(under(act, AVAILABILITY_KEYS));
    }
    keys
}

fn expected(groups: &[&[&str]]) -> BTreeSet<String> {
    groups
        .iter()
        .flat_map(|group| group.iter().map(|key| (*key).to_owned()))
        .collect()
}

fn under(prefix: &str, keys: &[&str]) -> Vec<String> {
    let mut all = vec![prefix.to_owned()];
    all.extend(keys.iter().map(|key| format!("{prefix}.{key}")));
    all
}

fn activity_keys(audience: Audience) -> BTreeSet<String> {
    let mut keys = expected(&[COVERAGE_KEYS, OPERATION_KEYS, STEP_KEYS]);
    if audience == Audience::Control {
        keys.extend(expected(&[COVERAGE_WINDOW_ONLY]));
    }
    keys.insert("history".into());
    keys.extend(under("history[]", RECORD_KEYS));
    keys.extend(under("history[].steps[].record", RECORD_KEYS));
    keys.remove("history[]");
    keys
}

fn authority(path: &Path) -> EffectiveAuthority {
    let mut authority = GovernanceFacade::new(Some(path.to_owned()), None).effective_authority();
    authority.organization = Some(crate::governance::effective::OrganizationIdentity {
        name: "Example Org".into(),
        statement: Some("Because.".into()),
        url: Some("https://example.org/".into()),
        contacts: vec![crate::governance::effective::ContactView {
            kind: "email".into(),
            value: "it@example.org".into(),
            label: Some("IT".into()),
        }],
    });
    authority
}

#[test]
fn part_tokens_round_trip_and_unknown_tokens_are_errors() {
    for part in Part::ALL {
        assert_eq!(Part::from_token(part.token()), Ok(part));
    }
    assert_eq!(Part::from_token("policy"), Err(UnknownPart));
    assert_eq!(Part::from_token("Status"), Err(UnknownPart));
    assert_eq!(Part::from_token(""), Err(UnknownPart));
}

#[test]
fn each_part_holds_exactly_the_keys_classified_for_its_audience() {
    let (snapshot, _, path) = populated();
    for audience in [Audience::Control, Audience::Watch] {
        let projection = projection(audience);
        let shown = |part| key_paths(&projection.part(part, &snapshot));
        assert_eq!(
            shown(Part::Status),
            expected(&[STATUS_KEYS]),
            "{audience:?}"
        );
        assert_eq!(
            shown(Part::Connections),
            expected(&[CONNECTIONS_KEYS]),
            "{audience:?}"
        );
        assert_eq!(
            shown(Part::Activity),
            activity_keys(audience),
            "{audience:?}"
        );
        assert_eq!(
            shown(Part::Integrations),
            expected(&[INTEGRATION_KEYS]),
            "{audience:?}"
        );
        assert_eq!(
            shown(Part::Diagnostics),
            expected(&[DIAGNOSTIC_KEYS]),
            "{audience:?}"
        );
        assert_eq!(shown(Part::Controls), controls_keys(), "{audience:?}");
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn the_policy_holds_exactly_the_keys_classified_for_its_audience() {
    let (_, _, path) = populated();
    let authority = authority(&path);
    assert_eq!(
        key_paths(&projection(Audience::Control).policy(&authority)),
        expected(&[POLICY_KEYS, POLICY_CONTROL_ONLY])
    );
    assert_eq!(
        key_paths(&projection(Audience::Watch).policy(&authority)),
        expected(&[POLICY_KEYS, POLICY_WATCH_ONLY])
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn nothing_nobody_reads_is_sent_to_anyone() {
    let (snapshot, _, path) = populated();
    for audience in [Audience::Control, Audience::Watch] {
        let projection = projection(audience);
        let mut text = String::new();
        for part in Part::ALL {
            text.push_str(&projection.part(part, &snapshot).to_string());
        }
        for dropped in [
            "overview",
            "generated_at_ms",
            "private_view",
            "held_tab_count",
            "\"leased\"",
            "local_policy_configured",
            "managed_authority_active",
        ] {
            assert!(!text.contains(dropped), "{audience:?}: {dropped}");
        }
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_watcher_is_told_how_many_sites_are_excluded_and_not_which() {
    let (snapshot, _, path) = populated();
    let watcher = projection(Audience::Watch).part(Part::Activity, &snapshot);
    assert!(!watcher.to_string().contains(EXCLUDED));
    assert!(!watcher.to_string().contains("second.invalid"));
    assert_eq!(watcher["document_coverage"][0]["excluded_host_count"], 2);
    let window = projection(Audience::Control).part(Part::Activity, &snapshot);
    assert_eq!(
        window["document_coverage"][0]["excluded_hosts"],
        json!([EXCLUDED, "second.invalid"])
    );
    assert_eq!(window["document_coverage"][0]["excluded_host_count"], 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_watcher_is_told_how_many_sites_are_never_touched_and_gets_no_policy_text() {
    let (_, _, path) = populated();
    let authority = authority(&path);
    assert!(authority
        .ceilings
        .iter()
        .any(|line| line.contains(NEVER_TOUCH)));
    let watcher = projection(Audience::Watch).policy(&authority);
    let text = watcher.to_string();
    assert!(!text.contains(NEVER_TOUCH));
    assert_eq!(watcher["layers"][0]["document"], Value::Null);
    let setting = &watcher["layers"][0]["settings"][0];
    assert_eq!(setting["value"], "1");
    assert_eq!(setting["count"], 1);
    assert_eq!(
        watcher["ceilings"].as_array().unwrap().last().unwrap(),
        &json!(words::never_touch_count(1))
    );
    assert_eq!(watcher["ceilings"].as_array().unwrap().len(), 3);

    let window = projection(Audience::Control).policy(&authority);
    assert!(window["layers"][0]["document"]
        .as_str()
        .unwrap()
        .contains(NEVER_TOUCH));
    assert!(window.to_string().contains(NEVER_TOUCH));
    assert!(window["layers"][0]["settings"][0].get("count").is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn paths_are_home_relative_in_every_part_and_the_policy_for_both_audiences() {
    let (mut snapshot, _, path) = populated();
    let home = path.parent().unwrap().to_owned();
    let inside = |name: &str| home.join(name).to_string_lossy().into_owned();
    snapshot.harnesses[0].config_path = inside("cursor.json");
    snapshot.controls = ControlsPart::of(&ControlsFacts {
        runtime_state: RuntimeControlState::Active,
        runtime_note: None,
        viewing: ViewingFacts::NotServed,
        diagnostics: &DiagnosticsReport {
            layer: "marker",
            directory: Some(inside("logs")),
            used_bytes: 0,
        },
        counts: ConnectionCounts::default(),
        user_home: Some(&home),
    });
    let authority = authority(&path);
    for audience in [Audience::Control, Audience::Watch] {
        let projection = Projection::with_home(audience, Some(home.clone()));
        let sep = std::path::MAIN_SEPARATOR;
        let integrations = projection.part(Part::Integrations, &snapshot);
        assert_eq!(
            integrations["harnesses"][0]["config_path"],
            format!("~{sep}cursor.json")
        );
        let controls = projection.part(Part::Controls, &snapshot);
        assert_eq!(controls["diagnostics"]["folder"], format!("~{sep}logs"));
        let policy = projection.policy(&authority);
        let file = path.file_name().unwrap().to_string_lossy();
        assert_eq!(policy["layers"][0]["path"], format!("~{sep}{file}"));
        // What a person pastes stays exact.
        assert_eq!(
            integrations["harnesses"][0]["manual_setup"],
            snapshot.harnesses[0].manual_setup
        );
        assert_eq!(
            integrations["harnesses"][0]["connector_command"],
            snapshot.harnesses[0].connector_command
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_path_below_the_home_loses_the_home_and_nothing_else_does() {
    let home = Path::new("/home/ann");
    assert_eq!(display_path(home, "/home/ann"), "~");
    assert_eq!(
        display_path(home, "/home/ann/.ghostlight/state/x.json"),
        format!(
            "~{0}.ghostlight{0}state{0}x.json",
            std::path::MAIN_SEPARATOR
        )
    );
    assert_eq!(display_path(home, "/home/annie/x"), "/home/annie/x");
    assert_eq!(display_path(home, "/var/log/x"), "/var/log/x");
    assert_eq!(display_path(home, "/home"), "/home");
    assert_eq!(
        display_path(Path::new("/home/ann/"), "/home/ann/x"),
        display_path(home, "/home/ann/x")
    );
    assert_eq!(display_path(Path::new(""), "/home/ann/x"), "/home/ann/x");
}

#[cfg(windows)]
#[test]
fn on_windows_the_home_matches_without_regard_to_case_and_keeps_backslashes() {
    let home = Path::new("C:\\Users\\Ann");
    assert_eq!(
        display_path(home, "c:\\users\\ann\\.ghostlight\\state\\user-policy.json"),
        "~\\.ghostlight\\state\\user-policy.json"
    );
    assert_eq!(
        display_path(home, "C:/Users/Ann/.cursor/mcp.json"),
        "~\\.cursor\\mcp.json"
    );
    assert_eq!(
        display_path(home, "C:\\Users\\Annie\\x"),
        "C:\\Users\\Annie\\x"
    );
    assert_eq!(display_path(home, "D:\\data\\x"), "D:\\data\\x");
}

#[test]
fn the_rules_tag_ignores_formatting_key_order_and_version() {
    let (_, _, path) = populated();
    let mut authority = authority(&path);
    let tag = policy_etag(&authority);
    assert_eq!(tag.len(), 64);
    let set_document = |authority: &mut EffectiveAuthority, document: &str| {
        authority
            .layers
            .iter_mut()
            .find(|layer| layer.kind == LayerKind::User)
            .unwrap()
            .document = Some(document.to_owned());
    };
    let original = authority
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::User)
        .and_then(|layer| layer.document.clone())
        .unwrap();
    let mut reformatted: Value = serde_json::from_str(&original).unwrap();
    reformatted["version"] = json!("9");
    set_document(&mut authority, &reformatted.to_string());
    assert_eq!(policy_etag(&authority), tag);
    set_document(
        &mut authority,
        r#"{"schema":4,"name":"other","version":"1","grants":[]}"#,
    );
    assert_ne!(policy_etag(&authority), tag);
    authority.layers.clear();
    assert_eq!(policy_etag(&authority), NO_POLICY_ETAG);
    let _ = std::fs::remove_file(path);
}

/// The acts of a controls part, by name, with whether each is available.
fn available(controls: &Value) -> Vec<(String, bool)> {
    let mut found = Vec::new();
    for (row, acts) in [
        ("runtime", &controls["runtime"]["acts"]),
        ("viewing", &controls["viewing"]["acts"]),
        ("diagnostics", &controls["diagnostics"]["acts"]),
        ("notifications", &controls["notifications"]["acts"]),
    ] {
        for (name, act) in acts.as_object().unwrap() {
            found.push((format!("{row}.{name}"), act["available"].as_bool().unwrap()));
        }
    }
    found.sort();
    found
}

#[test]
fn the_window_may_do_every_act_of_the_controls_part_and_a_watcher_only_pauses() {
    let (snapshot, _, path) = populated();
    let window = projection(Audience::Control).part(Part::Controls, &snapshot);
    assert!(available(&window).iter().all(|(_, may)| *may), "{window}");
    assert_eq!(window["you"]["context"], "control");
    assert_eq!(window["you"]["word"], words::WORD_CONTROL);
    assert_eq!(window["you"]["sentence"], words::SENTENCE_CONTROL_WATCHED);

    let watcher = projection(Audience::Watch).part(Part::Controls, &snapshot);
    let allowed: Vec<_> = available(&watcher)
        .into_iter()
        .filter(|(_, may)| *may)
        .map(|(name, _)| name)
        .collect();
    assert_eq!(allowed, ["runtime.hold"]);
    assert_eq!(
        watcher["viewing"]["acts"]["set_watching"]["why_not"],
        words::CHANGE_IN_WINDOW
    );
    assert_eq!(watcher["you"]["context"], "watch");
    assert_eq!(watcher["you"]["sentence"], words::SENTENCE_WATCH);
    let _ = std::fs::remove_file(path);
}

#[test]
fn settings_say_what_they_are_told_in_the_service_words() {
    let (snapshot, _, path) = populated();
    let controls = projection(Audience::Control).part(Part::Controls, &snapshot);
    assert_eq!(controls["runtime"]["state"], "held");
    assert_eq!(controls["runtime"]["word"], words::CONTROLS_RUNTIME_HELD);
    assert_eq!(controls["runtime"]["note"], "Paused from the tray.");
    assert_eq!(controls["viewing"]["watch"], true);
    assert_eq!(controls["viewing"]["word"], words::CONTROLS_VIEWING_ON);
    assert_eq!(controls["viewing"]["url"], "http://127.0.0.1:41234/");
    assert_eq!(controls["diagnostics"]["on"], true);
    assert_eq!(controls["diagnostics"]["word"], "On (marker)");
    assert_eq!(
        controls["service"]["word"],
        "Ghostlight service is up; 1 MCP client and 1 browser connected."
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn watching_off_changes_the_word_and_the_sentence_the_window_is_told() {
    let (mut snapshot, _, path) = populated();
    snapshot.controls.viewing.watch = false;
    snapshot.controls.viewing.word = words::CONTROLS_VIEWING_OFF;
    let window = projection(Audience::Control).part(Part::Controls, &snapshot);
    assert_eq!(window["viewing"]["word"], words::CONTROLS_VIEWING_OFF);
    assert_eq!(window["you"]["sentence"], words::SENTENCE_CONTROL_PRIVATE);
    let _ = std::fs::remove_file(path);
}

#[test]
fn the_log_folder_is_shown_relative_to_the_users_home() {
    let (snapshot, _, path) = populated();
    let sep = std::path::MAIN_SEPARATOR;
    let controls = projection(Audience::Watch).part(Part::Controls, &snapshot);
    assert_eq!(
        controls["diagnostics"]["folder"],
        format!("~{sep}.ghostlight{sep}logs")
    );
    assert!(!controls.to_string().contains("/home/ann"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn an_event_that_carries_the_controls_is_projected_for_each_audience() {
    let (snapshot, _, path) = populated();
    let event = WorkbenchEvent {
        seq: 4,
        change: WorkbenchChange::ControlsChanged {
            controls: Box::new(snapshot.controls.clone()),
        },
    };
    let window = projection(Audience::Control).event(&event);
    assert_eq!(window["change"]["kind"], "controls_changed");
    assert_eq!(window["change"]["controls"]["you"]["context"], "control");
    assert_eq!(
        window["change"]["controls"]["runtime"]["acts"]["end_session"]["available"],
        true
    );
    let watcher = projection(Audience::Watch).event(&event);
    assert_eq!(watcher["change"]["controls"]["you"]["context"], "watch");
    assert_eq!(
        watcher["change"]["controls"]["runtime"]["acts"]["end_session"]["available"],
        false
    );
    let _ = std::fs::remove_file(path);
}
