// SPDX-License-Identifier: Apache-2.0 OR MIT
//! All-open golden guard for the A1 module reorg and the A3 governance facade. Neither the
//! regroup into governance/ browser/ transport/ (A1) nor the introduction of the `Governance`
//! facade at the dispatch chokepoint (A3) may change anything observable. Invariants:
//!   1. the edge-owned legacy surface stays byte stable;
//!   2. facade decide round-trip -- `Governance::all_open()` resolves every call to
//!      `Decision::Allow { grant_id: None }` without touching any decision port (audit is
//!      orthogonal to all-open, shared format doc section 4.5, so the facade still carries an
//!      audit sink).
//!
//! Process-boundary redaction coverage lives in the ADR-0056 Lightbox scenario library.

use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::{
    AuditRecord, AuditSink, Capability, Decision, EffectiveMode, GoverningResource,
};
use ghostlight_transport::operation::{IntentId, OperationId, OperationKey};
use serde_json::Value;

const EDGE_LEGACY_SURFACE: &str =
    include_str!("../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1.json");

/// The 25 tool names in advertised order (the 13 trained tools plus `narrate`, `wait_for`, `script`,
/// `form_fill`, `act_on`, `dialog`, `tab_control`, `file_upload` (ADR-0050 Decision 2), `browser_batch` (ADR-0050 Decision 3),
/// `upload_image` (ADR-0050 Decision 4), `gif_creator` (ADR-0050 Decision 5), and ADR-0022
/// Decision 7's sanctioned `explain` addition, positioned last), copied from the code-declared
/// edge-owned frozen profile, in declared order.
const GOLDEN_TOOL_NAMES: [&str; 25] = [
    "tabs_context_mcp",
    "tabs_create_mcp",
    "navigate",
    "computer",
    "find",
    "form_input",
    "get_page_text",
    "javascript_tool",
    "read_console_messages",
    "read_network_requests",
    "read_page",
    "resize_window",
    "update_plan",
    "narrate",
    "wait_for",
    "script",
    "form_fill",
    "act_on",
    "dialog",
    "tab_control",
    "file_upload",
    "browser_batch",
    "upload_image",
    "gif_creator",
    "explain",
];

#[test]
fn tools_list_is_byte_stable_through_the_move() {
    let v: Value = serde_json::from_str(EDGE_LEGACY_SURFACE)
        .expect("edge-owned ghostlight-legacy/v1 profile must parse");
    let tools = v["tools"].as_array().expect("tools array");
    assert_eq!(
        tools.len(),
        GOLDEN_TOOL_NAMES.len(),
        "all 25 tools advertised (13 trained plus narrate, wait_for, script, form_fill, act_on, dialog, tab_control, file_upload, browser_batch, upload_image, gif_creator, and explain)"
    );
    for (i, name) in GOLDEN_TOOL_NAMES.iter().enumerate() {
        assert_eq!(
            tools[i]["name"], *name,
            "tool #{i} name and order preserved"
        );
    }
}

/// A sink that drops every record; enough to construct an all-open facade for this test
/// without pulling in the real file/stderr recorders.
struct NullAuditSink;
impl AuditSink for NullAuditSink {
    fn record(&self, _record: &AuditRecord) {}
    fn record_session_event(&self, _record: &ghostlight::governance::ports::SessionEventRecord) {}
    fn record_attention_event(
        &self,
        _record: &ghostlight::governance::ports::AttentionEventRecord,
    ) {
    }
}

#[test]
fn facade_decide_is_all_open_after_the_move() {
    let governance = Governance::all_open(std::sync::Arc::new(NullAuditSink));
    for descriptor in ghostlight::operation::registry::descriptors() {
        assert!(
            matches!(
                governance.decide(
                    descriptor.key.id.as_str(),
                    Some(descriptor.key.intent.as_str()),
                    descriptor.requires,
                    GoverningResource::None,
                    EffectiveMode::Enforce
                ),
                Decision::Allow { grant_id: None }
            ),
            "{} / {} must be allowed in the all-open engine",
            descriptor.key.id,
            descriptor.key.intent
        );
    }
}

/// ADR-0050 Decision 2: `file_upload` is a new additive tool. It is allowed under the all-open
/// engine (no manifest = no denials) and classifies as a Write capability (bytes leave the user's
/// control into a web destination; the `ref` was located by a separately-governed read).
#[test]
fn file_upload_is_all_open_allowed_and_classifies_write() {
    let governance = Governance::all_open(std::sync::Arc::new(NullAuditSink));
    assert!(
        matches!(
            governance.decide(
                OperationId::BrowserUpload.as_str(),
                Some(IntentId::UploadClientFiles.as_str()),
                &[],
                GoverningResource::None,
                EffectiveMode::Enforce
            ),
            Decision::Allow { grant_id: None }
        ),
        "file_upload must be allowed in the all-open engine"
    );
    assert_eq!(
        ghostlight::operation::registry::descriptor(OperationKey::new(
            OperationId::BrowserUpload,
            IntentId::UploadClientFiles,
        ))
        .map(|descriptor| descriptor.requires),
        Some(&[Capability::Write][..]),
        "file_upload classifies as a Write capability"
    );
}
