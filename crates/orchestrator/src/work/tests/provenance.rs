//! Original connection evidence survives queue refusal and loss of the workspace itself.

use super::*;
use crate::provenance::{ConnectionEvidence, PeerObservation};
use crate::work::PreparedInvocation;
use ghostlight_bridge::service::SessionMarker;

#[test]
fn queued_refusals_keep_original_evidence_after_another_peer_and_workspace_release() {
    let (executor, browser, store, _, audit) = fixture();
    let first = Arc::new(ConnectionEvidence::with_peer(
        "PRIVATE_FIRST_CLAIM",
        IntakeChannel::Mcp,
        PeerObservation::Observed {
            executable: "first.exe".into(),
        },
    ));
    let marker = SessionMarker::Declared {
        key: "shared".into(),
    };
    let workspace = store.connect(first.clone(), Some(marker.clone()));
    let prepared: Vec<_> = ["policy_explain", "policy_explain", "browser_invalid"]
        .into_iter()
        .map(|tool| PreparedInvocation::new(tool, json!({}), None, Some(first.clone())))
        .collect();
    let second = Arc::new(ConnectionEvidence::with_peer(
        "PRIVATE_SECOND_CLAIM",
        IntakeChannel::Cli,
        PeerObservation::Observed {
            executable: "second.exe".into(),
        },
    ));
    assert_eq!(store.connect(second, Some(marker)), workspace);
    store.release(&workspace);

    for (index, prepared) in prepared.iter().enumerate() {
        let cancelled = CancellationToken::default();
        if index == 0 {
            cancelled.cancel();
        }
        let result = executor.execute_prepared(&workspace, prepared, &cancelled, index == 1);
        assert_eq!(result.effect, Effect::None);
        assert!(!serde_json::to_string(&result).unwrap().contains("PRIVATE_"));
    }
    assert!(browser.calls().is_empty());
    let records = audit.0.lock().unwrap();
    assert_eq!(records.len(), 3);
    for record in records.iter() {
        assert_eq!(record.provenance.as_ref(), Some(first.attribution()));
        assert_eq!(record.channel, Some(IntakeChannel::Mcp));
        assert_eq!(record.peer_image.as_deref(), Some("first.exe"));
    }
    assert!(!serde_json::to_string(&*records)
        .unwrap()
        .contains("PRIVATE_"));
}
