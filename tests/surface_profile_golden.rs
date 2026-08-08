// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Frozen compatibility oracle for ADR-0101's `ghostlight-legacy/v1` profile.
//!
//! The production registry remains the declaration source. These fixtures are deliberate
//! migration oracles: an intentional guidance or schema change must update them separately and
//! visibly, while an architectural extraction must leave them unchanged.

use ghostlight::tool::tools::{advertised_tools_json, agent_guide_text};
use serde_json::Value;
use std::fmt::Write;

const LEGACY_SURFACE: &str = include_str!("golden/surfaces/ghostlight-legacy-v1.json");
const LEGACY_AGENT_GUIDE: &str =
    include_str!("golden/surfaces/ghostlight-legacy-v1-agent-guide.txt");

fn checkout_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn ascii_json(value: &Value) -> String {
    let rendered = serde_json::to_string_pretty(value).expect("advertised tools must serialize");
    let mut escaped = String::with_capacity(rendered.len());
    for character in rendered.chars() {
        let scalar = u32::from(character);
        if character.is_ascii() {
            escaped.push(character);
        } else if scalar <= 0xffff {
            write!(escaped, "\\u{scalar:04x}").expect("writing to a String cannot fail");
        } else {
            let supplementary = scalar - 0x10000;
            let high = 0xd800 + (supplementary >> 10);
            let low = 0xdc00 + (supplementary & 0x3ff);
            write!(escaped, "\\u{high:04x}\\u{low:04x}").expect("writing to a String cannot fail");
        }
    }
    escaped
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

    let actual = advertised_tools_json();
    let rendered = format!("{}\n", ascii_json(&actual));
    assert_eq!(
        rendered, fixture,
        "serialized ghostlight-legacy/v1 surface changed"
    );

    let expected: Value =
        serde_json::from_str(&fixture).expect("legacy surface fixture must be valid JSON");
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

    let rendered = format!("{}\n", agent_guide_text());
    assert_eq!(
        rendered, fixture,
        "ghostlight-legacy/v1 agent guide changed"
    );
}
