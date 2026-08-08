// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Frozen compatibility oracle for ADR-0101's `ghostlight-legacy/v1` profile.
//!
//! The MCP edge owns the production declarations. These separate fixtures remain deliberate
//! migration oracles: an intentional guidance or schema change must update them separately and
//! visibly, while an architectural extraction must leave them unchanged.

use serde_json::Value;

const LEGACY_SURFACE: &str = include_str!("golden/surfaces/ghostlight-legacy-v1.json");
const LEGACY_AGENT_GUIDE: &str =
    include_str!("golden/surfaces/ghostlight-legacy-v1-agent-guide.txt");
const EDGE_LEGACY_SURFACE: &str =
    include_str!("../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1.json");
const EDGE_LEGACY_AGENT_GUIDE: &str =
    include_str!("../crates/mcp-connector/src/surface/data/ghostlight-legacy-v1-agent-guide.txt");

fn checkout_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn ghostlight_legacy_v1_surface_matches_the_frozen_oracle() {
    let fixture = checkout_lf(LEGACY_SURFACE);
    assert!(fixture.is_ascii(), "legacy surface fixture must be ASCII");
    assert!(
        !fixture.contains('\r'),
        "legacy surface fixture must use LF"
    );
    assert!(
        fixture.ends_with('\n'),
        "legacy surface fixture must end with LF"
    );

    let rendered = checkout_lf(EDGE_LEGACY_SURFACE);
    assert_eq!(
        rendered, fixture,
        "edge-owned ghostlight-legacy/v1 surface changed"
    );

    let expected: Value =
        serde_json::from_str(&fixture).expect("legacy surface fixture must be valid JSON");
    let actual: Value =
        serde_json::from_str(&rendered).expect("edge legacy surface must be valid JSON");
    assert_eq!(
        actual, expected,
        "ghostlight-legacy/v1 surface changed structurally"
    );
    assert_eq!(
        expected["tools"]
            .as_array()
            .expect("legacy surface must contain a tools array")
            .len(),
        25,
        "ghostlight-legacy/v1 must freeze all 25 declarations"
    );
}

#[test]
fn ghostlight_legacy_v1_agent_guide_matches_the_frozen_oracle() {
    let fixture = checkout_lf(LEGACY_AGENT_GUIDE);
    assert!(
        fixture.is_ascii(),
        "legacy agent guide fixture must be ASCII"
    );
    assert!(
        !fixture.contains('\r'),
        "legacy agent guide fixture must use LF"
    );
    assert!(
        fixture.ends_with('\n'),
        "legacy agent guide fixture must end with LF"
    );

    let rendered = checkout_lf(EDGE_LEGACY_AGENT_GUIDE);
    assert_eq!(
        rendered, fixture,
        "ghostlight-legacy/v1 agent guide changed"
    );
}
