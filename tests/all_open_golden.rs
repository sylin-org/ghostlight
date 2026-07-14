// SPDX-License-Identifier: Apache-2.0 OR MIT
//! All-open golden guard for the A1 module reorg and the A3 governance facade. Neither the
//! regroup into governance/ browser/ transport/ (A1) nor the introduction of the `Governance`
//! facade at the dispatch chokepoint (A3) may change anything observable. Invariants, reached
//! through the NEW module locations:
//!   1. tools/list byte-stability -- the advertised tool surface is the same 13 tools in
//!      the same order, and `directory::descriptor` still resolves them.
//!   2. facade decide round-trip -- `Governance::all_open()` resolves every call to
//!      `Decision::Allow { grant_id: None }` without touching any decision port (audit is
//!      orthogonal to all-open, shared format doc section 4.5, so the facade still carries an
//!      audit sink).
//!
//! Process-boundary redaction coverage lives in the ADR-0056 Lightbox scenario library.

use ghostlight::browser::directory::{descriptor, requires};
use ghostlight::governance::dispatch::Governance;
use ghostlight::governance::ports::{
    AuditRecord, AuditSink, Capability, Decision, EffectiveMode, GoverningResource,
};
use ghostlight::transport::mcp::tools::advertised_tools_json;

/// The 25 tool names in advertised order (the 13 trained tools plus `narrate`, `wait_for`, `script`,
/// `form_fill`, `act_on`, `dialog`, `tab_control`, `file_upload` (ADR-0050 Decision 2), `browser_batch` (ADR-0050 Decision 3),
/// `upload_image` (ADR-0050 Decision 4), `gif_creator` (ADR-0050 Decision 5), and ADR-0022
/// Decision 7's sanctioned `explain` addition, positioned last), copied from the code-declared
/// registry (`browser::directory::REGISTRY`), in declared order.
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
    let v = advertised_tools_json();
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
        assert!(descriptor(name).is_some(), "{name} must be a known tool");
    }
    assert!(
        descriptor("bogus_tool").is_none(),
        "unknown tools stay unknown"
    );
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
    for name in GOLDEN_TOOL_NAMES {
        assert!(
            matches!(
                governance.decide(
                    name,
                    None,
                    &[],
                    GoverningResource::None,
                    EffectiveMode::Enforce
                ),
                Decision::Allow { grant_id: None }
            ),
            "{name} must be allowed in the all-open engine"
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
                "file_upload",
                None,
                &[],
                GoverningResource::None,
                EffectiveMode::Enforce
            ),
            Decision::Allow { grant_id: None }
        ),
        "file_upload must be allowed in the all-open engine"
    );
    assert_eq!(
        requires("file_upload", None),
        Some(&[Capability::Write][..]),
        "file_upload classifies as a Write capability"
    );
}
