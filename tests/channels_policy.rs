// SPDX-License-Identifier: Apache-2.0 OR MIT
//! H8 (`docs/tasks/hub/H8-web-api-loopback-policy.md`, ADR-0030 Decision 5/9): the
//! `channels.webapi.from` decision is produced by `PolicyDecisionPoint::decide` (the PDP), never
//! by any transport-layer check. Drives the pure decision directly, no listener involved.

use ghostlight::governance::channels::ChannelsPdp;
use ghostlight::governance::ports::{
    Decision, DecisionRequest, EffectiveMode, GoverningResource, PolicyDecisionPoint,
};

fn request(channel_source: &str) -> DecisionRequest {
    DecisionRequest {
        grants: Vec::new(),
        tool: String::new(),
        action: None,
        requires: Vec::new(),
        resource: GoverningResource::None,
        manifest_mode: None,
        config_mode: EffectiveMode::Enforce,
        manifest_hash: String::new(),
        channel_source: Some(channel_source.to_string()),
    }
}

#[test]
fn webapi_from_is_decided_in_the_pdp_on_the_subject() {
    let pdp = ChannelsPdp::new(vec!["localhost".to_string()]);

    // A member of the allowlist is allowed.
    assert_eq!(
        pdp.decide(&request("localhost")),
        Decision::Allow { grant_id: None }
    );

    // A source that is NOT a member is denied, by the pure PDP `decide`, PINNED rule label and
    // denial_id shape (docs/tasks/hub/PINS.md SS7).
    match pdp.decide(&request("203.0.113.7")) {
        Decision::Deny(denial) => {
            assert_eq!(denial.rule, "channel/webapi_from");
            assert!(
                denial.denial_id.starts_with("D-"),
                "denial_id: {}",
                denial.denial_id
            );
            assert_eq!(
                denial.denial_id.len(),
                10,
                "\"D-\" plus 8 lowercase hex: {}",
                denial.denial_id
            );
            assert!(
                denial.denial_id[2..]
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "denial_id: {}",
                denial.denial_id
            );
        }
        other => panic!("expected Decision::Deny, got {other:?}"),
    }
}
