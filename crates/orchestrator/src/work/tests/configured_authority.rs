//! Configured authority: `policy_explain` and its frame's audit line name one authority.

use super::*;
use crate::work::InvocationContext;
use std::time::Instant;

/// `policy_explain` reports the authority its frame enforces, even when the policy changes between
/// the frame's start and the explanation, so the explanation and the frame's audit line, which
/// records the snapshot's id, name one authority (ADR-0203 Decision 7.5).
#[test]
fn policy_explain_and_its_audit_line_name_one_authority() {
    let policy = TestPolicy::new();
    let (executor, _, _, workspace, _) = fixture_with_governance(policy.facade());
    let publication = executor.governance.published();
    let enforced = publication.authority().clone();
    assert_eq!(enforced.id, publication.snapshot().id());
    policy.set(json!(["read"]));
    let newer = executor.governance.published();
    assert_ne!(newer.authority().id, enforced.id, "the edit was published");
    assert_ne!(newer.authority().capabilities, enforced.capabilities);

    let cancellation = CancellationToken::default();
    let context = InvocationContext {
        settlement_pending: &std::sync::atomic::AtomicBool::new(false),
        settlement_requested: false,
        provenance: &crate::provenance::ConnectionEvidence::claimed("test", IntakeChannel::Mcp),
        invocation: &crate::work::InvocationId::mint(),
        workspace: &workspace,
        requested_browser: None,
        requested_tab: None,
        requested_target: None,
        repeat_tool: None,
        gesture: crate::language::page_refusal::PageGesture::Other,
        snapshot: publication.snapshot(),
        publication: &publication,
        requirements: crate::governance::CapabilitySet::EMPTY,
        deadline: Instant::now() + Duration::from_secs(1),
        deadline_adjustable: false,
        cancellation: &cancellation,
        findings: &crate::work::findings::FrameFindings::default(),
    };
    let explained = executor.explain_policy(&context);
    assert_eq!(explained.result.status, Status::Succeeded);
    assert_eq!(
        explained.result.facts["capabilities"],
        json!(enforced.capabilities)
    );
    assert_eq!(context.snapshot.id(), enforced.id);
}
