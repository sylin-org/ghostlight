//! Golden guard for the generated config JSON Schema and markdown key reference (ADR-0020
//! commitment 1). Any registry change (a new key, an edited description, a changed
//! constraint) must fail HERE until `tests/golden/config-schema.json` and
//! `tests/golden/config-keys.md` are regenerated and reviewed deliberately -- that review is
//! what keeps the schema, the docs, and the code from drifting apart.

use browser_mcp::governance::config::schema::{render_config_schema, render_key_reference};
use browser_mcp::governance::config::KEYS;
use serde_json::Value;
use std::collections::HashSet;

/// Defense against a CRLF checkout only (the repo pins LF via `tests/golden/.gitattributes`);
/// the generator itself never emits `\r`.
#[test]
fn generated_schema_matches_the_golden_file() {
    let golden = include_str!("golden/config-schema.json").replace("\r\n", "\n");
    assert_eq!(render_config_schema(), golden);
}

#[test]
fn generated_key_reference_matches_the_golden_file() {
    let golden = include_str!("golden/config-keys.md").replace("\r\n", "\n");
    assert_eq!(render_key_reference(), golden);
}

#[test]
fn schema_covers_the_registry_exactly() {
    let schema: Value = serde_json::from_str(&render_config_schema()).expect("valid JSON");
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["config"]["additionalProperties"],
        false
    );

    let rendered_keys: HashSet<String> = schema["properties"]["config"]["properties"]
        .as_object()
        .expect("config properties object")
        .keys()
        .cloned()
        .collect();
    let registry_keys: HashSet<String> = KEYS.iter().map(|d| d.key.to_string()).collect();
    assert_eq!(
        rendered_keys, registry_keys,
        "no missing key, no stale property"
    );

    for def in KEYS {
        let description = schema["properties"]["config"]["properties"][def.key]["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{}: missing description", def.key));
        assert!(!description.is_empty(), "{}", def.key);
    }
}

#[test]
fn key_reference_covers_the_registry_exactly() {
    let rendered = render_key_reference();
    for def in KEYS {
        assert!(
            rendered.contains(&format!("## `{}`", def.key)),
            "missing section for {}",
            def.key
        );
    }
    assert_eq!(rendered.matches("\n## ").count(), KEYS.len());
}

#[test]
fn outputs_are_ascii_and_lf_only() {
    for rendered in [render_config_schema(), render_key_reference()] {
        assert!(!rendered.contains('\r'), "must contain no CR");
        assert!(rendered.is_ascii(), "must contain no byte above 0x7F");
    }
}
