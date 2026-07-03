//! The schema-3 manifest document: format types, parsing, and validation (ADR-0022 Decisions
//! 3/4/6). Domain-agnostic core: this module knows the manifest's SHAPE (grants, config
//! entries, identity, mode) but resolves no host pattern grammar itself -- both `hosts.allow`/
//! `hosts.deny` syntax and `content.security.sacred_domains` syntax are checked through the
//! SAME single injected `domain_pattern_valid` function pointer supplied by the composition
//! root (`browser::pattern::is_valid_pattern`), so this module never names `browser::` directly
//! (the a7 arch-test). The two call sites diverge on exactly one point, expressed inline rather
//! than via a second injected parameter: grant host patterns additionally accept the bare `*`
//! token (ADR-0022 Decision 4 rule 1, mirroring `browser::polarity::is_valid_host_rule`'s own
//! `pattern == "*" || is_valid_pattern(pattern)` shape without crossing the arch boundary to
//! call it directly); `content.security.sacred_domains` (via [`validate_config_entry`]) never
//! gets that carve-out, so `*` there is still rejected. Grant EVALUATION (matching a resolved
//! host against a grant's host rules, and the tool surface a grant permits) is
//! enforcement's/advertisement's job; this module validates SYNTAX only.
//!
//! Supersedes the schema-2 grant model (`docs/tasks/stage-2/00-shared-format.md` sections 4.3,
//! 6.1 `rw`, 8; ADR-0022): `domains`/`access`/`tools`/`exclude_tools` are gone, replaced by
//! `hosts` (allow/deny polarity, ADR-0022 Decision 4) and `allowed` (a capability set, ADR-0022
//! Decisions 1/3). A schema-2 (or any non-3) manifest fails here with a precise
//! unsupported-schema error, never silent compatibility parsing.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::governance::ports::{Capability, EffectiveMode};

/// The schema-3 manifest document (ADR-0022 Decision 6). `hash` is never authored; it is
/// computed by [`parse_manifest`] from the canonical bytes (shared format doc section 4.2) and
/// is the one field excluded from both serialization and deserialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be exactly `3`; any other value is rejected before shape validation runs.
    pub schema: u32,
    /// Required, non-empty.
    pub name: String,
    /// Required, non-empty. A free-form label, not a semver requirement.
    pub version: String,
    /// The manifest-level default enforcement mode; `None` defers to the resolved
    /// `governance.mode` config key (G15's precedence: per-grant > manifest > registry).
    pub mode: Option<EffectiveMode>,
    /// Informational identity block (shared format doc section 4.1); all fields optional and
    /// untyped strings, never validated against an enum -- the reconciled format keeps this
    /// informational.
    pub identity: Option<IdentityBlock>,
    /// Required (may be empty).
    pub grants: Vec<Grant>,
    /// Optional (defaults to empty).
    #[serde(default)]
    pub config: Vec<ConfigEntry>,
    /// SHA-256 content hash, 64 lowercase hex characters, computed by [`parse_manifest`] from
    /// the canonical bytes (shared format doc section 4.2). Never authored; an authored `hash`
    /// key is rejected as an unknown field by `deny_unknown_fields`, since this field is
    /// `#[serde(skip)]`.
    #[serde(skip)]
    pub hash: String,
}

/// The manifest's informational `identity` block (shared format doc section 4.1). Every field
/// is optional and, when present, type-checked but not otherwise validated (`resolved_by` is a
/// free string, not an enum). Distinct from
/// [`crate::governance::ports::Identity`] (the audit record's derived `{principal,
/// resolved_by}` pair): this is the full authored block a later task derives that pair from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct IdentityBlock {
    pub resolved_by: Option<String>,
    pub principal: Option<String>,
    pub groups: Option<Vec<String>>,
    pub resolved_at: Option<String>,
}

/// A grant's host-scoping rules (ADR-0022 Decision 4): `allow` grants coverage, `deny` carves
/// holes out of it. Both members default to empty; a grant without a `hosts` member at all is a
/// shape error (the field itself is required), but `{}` (both lists empty) is a valid grant
/// that covers nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HostRules {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

/// One resolved-at-load-time grant (ADR-0022 Decision 6). Consumed unchanged by
/// [`crate::governance::ports::DecisionRequest`]: this IS the type a2 anticipated when it called
/// its own placeholder `Grant` "the manifest engine fleshes this out" -- there is exactly one
/// `Grant` type in the crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// Required, non-empty, unique within the manifest.
    pub id: String,
    /// Required. Host allow/deny polarity (ADR-0022 Decision 4); syntax validated at load,
    /// matching semantics are enforcement's (the browser plugin's ADR-0022 Decision 4
    /// evaluator, injected as a function pointer).
    pub hosts: HostRules,
    /// The capability set this grant permits (ADR-0022 Decisions 1/3). May be empty (a grant
    /// that scopes hosts but permits nothing beyond `requires: []` actions, which need no grant
    /// anyway); no duplicates.
    pub allowed: Vec<Capability>,
    pub description: Option<String>,
    /// Per-grant override of the manifest-level `mode`.
    pub mode: Option<EffectiveMode>,
}

/// One manifest `config` entry (shared format doc section 4.4): a registry key, a value, and
/// the layer it targets when the manifest is the org policy file (an entry from a
/// user-supplied manifest always lands in the user layer regardless of its declared level;
/// see `governance::manifest::source`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntry {
    /// Must name a key registered in the typed key registry (`governance::config::KEYS`).
    pub key: String,
    /// Must satisfy the key's declared type and constraint.
    pub value: serde_json::Value,
    pub level: Level,
}

/// A config entry's declared layer (shared format doc section 2, 4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Mandatory,
    Recommended,
}

/// Why a manifest failed to parse or validate. Every variant's `Display` names the source
/// label and enough detail to fix the manifest without reading Rust code.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// The text is not valid JSON at all.
    #[error("{source_label}: syntax error at line {line}, column {column}: {message}")]
    Syntax {
        source_label: String,
        line: usize,
        column: usize,
        message: String,
    },
    /// The `schema` field is missing, not an integer, or not `3`.
    #[error("{source_label}: unsupported schema version {found} (expected 3){adr_note}")]
    UnsupportedSchema {
        source_label: String,
        found: String,
        /// Appended verbatim after "(expected 3)"; empty except for the schema-2 case, which
        /// appends the ADR-0022 migration sentence.
        adr_note: String,
    },
    /// Valid JSON, wrong shape: an unknown field, a wrong type, or a missing required field.
    /// serde's own message already names the field and (when available) the position.
    #[error("{source_label}: {message}")]
    Shape {
        source_label: String,
        message: String,
    },
    /// Valid shape, invalid content. `path` is a dotted/indexed field path (e.g.
    /// `grants[1].hosts.allow[0]`); no line number is available at this validation stage.
    #[error("{source_label}: {path}: {reason}")]
    Field {
        source_label: String,
        path: String,
        reason: String,
    },
}

/// Parse and validate manifest JSON text (ADR-0022 Decision 6) and compute its content hash
/// (shared format doc section 4.2). `source_label` names the origin (a file path or
/// `env://VAR`) for error messages. `domain_pattern_valid` is the browser plugin's real STRICT
/// host-syntax checker (`browser::pattern::is_valid_pattern`), injected so this core module
/// never names `browser::` directly; it is used verbatim for `content.security.sacred_domains`
/// config validation and wrapped with the bare-`*` carve-out (see [`is_valid_host_pattern`])
/// for grant `hosts.allow`/`hosts.deny` validation.
///
/// Pipeline, in this exact order so every failure class gets its most precise error: strip an
/// optional leading BOM; parse to a `Value` (a `Syntax` error carries serde's line/column);
/// check `schema == 3` BEFORE shape validation (so a schema-2 -- or any other -- manifest fails
/// with `UnsupportedSchema`, never a confusing unknown-field error); typed-deserialize
/// `Manifest` FROM THE STRING (not the `Value`, so serde's shape errors keep their
/// line/column); run semantic validation (field-path errors); compute the hash from the same
/// stripped bytes via [`super::identity::canonical_hash`] (the shared primitive g09 already
/// established).
pub fn parse_manifest(
    text: &str,
    source_label: &str,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<Manifest, ManifestError> {
    let stripped = text.strip_prefix('\u{feff}').unwrap_or(text);

    let value: serde_json::Value =
        serde_json::from_str(stripped).map_err(|e| ManifestError::Syntax {
            source_label: source_label.to_string(),
            line: e.line(),
            column: e.column(),
            message: e.to_string(),
        })?;

    let found_schema = value.get("schema").and_then(serde_json::Value::as_u64);
    if found_schema != Some(3) {
        let found = value
            .get("schema")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "<missing>".to_string());
        let adr_note = if found_schema == Some(2) {
            "; schema 2 is superseded by schema 3 (ADR-0022); update the manifest's grants to \
             hosts/allowed form."
                .to_string()
        } else {
            String::new()
        };
        return Err(ManifestError::UnsupportedSchema {
            source_label: source_label.to_string(),
            found,
            adr_note,
        });
    }

    let mut manifest: Manifest =
        serde_json::from_str(stripped).map_err(|e| ManifestError::Shape {
            source_label: source_label.to_string(),
            message: e.to_string(),
        })?;

    validate_semantics(&manifest, source_label, domain_pattern_valid)?;

    manifest.hash = super::identity::canonical_hash(stripped.as_bytes())
        .expect("already validated as a JSON object by the shape-validation step above");

    Ok(manifest)
}

/// A grant `hosts.allow`/`hosts.deny` pattern is valid iff it is the bare `*` token (ADR-0022
/// Decision 4 rule 1, legal ONLY here) or passes the injected `domain_pattern_valid` checker.
/// This mirrors `browser::polarity::is_valid_host_rule`'s own shape without this
/// domain-agnostic module naming `browser::` directly (the a7 arch-test): `*` never reaches
/// [`validate_config_entry`]'s `content.security.sacred_domains` check, which calls
/// `domain_pattern_valid` directly with no such carve-out.
fn is_valid_host_pattern(pattern: &str, domain_pattern_valid: fn(&str) -> bool) -> bool {
    pattern == "*" || domain_pattern_valid(pattern)
}

fn field_error(
    source_label: &str,
    path: impl Into<String>,
    reason: impl Into<String>,
) -> ManifestError {
    ManifestError::Field {
        source_label: source_label.to_string(),
        path: path.into(),
        reason: reason.into(),
    }
}

/// Semantic validation of an already-shape-checked manifest, including the config array's
/// duplicate-key rule (ADR-0023 Decision 3): a `config` array carrying the same `key` twice is
/// a field error naming the duplicate key at `config[{i}].key`, for every manifest origin (org
/// or user-sourced alike -- this is a deliberate tightening over the pre-ADR-0023 org-only
/// check).
fn validate_semantics(
    manifest: &Manifest,
    source_label: &str,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<(), ManifestError> {
    if manifest.name.is_empty() {
        return Err(field_error(source_label, "name", "must not be empty"));
    }
    if manifest.version.is_empty() {
        return Err(field_error(source_label, "version", "must not be empty"));
    }

    let mut seen_ids = HashSet::new();
    for (i, grant) in manifest.grants.iter().enumerate() {
        validate_grant(grant, i, source_label, domain_pattern_valid, &mut seen_ids)?;
    }

    let mut seen_config_keys = HashSet::new();
    for (i, entry) in manifest.config.iter().enumerate() {
        validate_config_entry(entry, i, source_label, domain_pattern_valid)?;
        if !seen_config_keys.insert(entry.key.clone()) {
            return Err(field_error(
                source_label,
                format!("config[{i}].key"),
                format!("duplicate config key '{}'", entry.key),
            ));
        }
    }

    Ok(())
}

fn validate_grant(
    grant: &Grant,
    index: usize,
    source_label: &str,
    domain_pattern_valid: fn(&str) -> bool,
    seen_ids: &mut HashSet<String>,
) -> Result<(), ManifestError> {
    let prefix = format!("grants[{index}]");

    if grant.id.is_empty() {
        return Err(field_error(
            source_label,
            format!("{prefix}.id"),
            "must not be empty",
        ));
    }
    if !seen_ids.insert(grant.id.clone()) {
        return Err(field_error(
            source_label,
            format!("{prefix}.id"),
            format!("duplicate grant id '{}'", grant.id),
        ));
    }

    for (j, pattern) in grant.hosts.allow.iter().enumerate() {
        if !is_valid_host_pattern(pattern, domain_pattern_valid) {
            return Err(field_error(
                source_label,
                format!("{prefix}.hosts.allow[{j}]"),
                format!("invalid host pattern '{pattern}'"),
            ));
        }
    }
    for (j, pattern) in grant.hosts.deny.iter().enumerate() {
        if !is_valid_host_pattern(pattern, domain_pattern_valid) {
            return Err(field_error(
                source_label,
                format!("{prefix}.hosts.deny[{j}]"),
                format!("invalid host pattern '{pattern}'"),
            ));
        }
    }

    let mut seen_caps = HashSet::new();
    for (j, cap) in grant.allowed.iter().enumerate() {
        if !seen_caps.insert(*cap) {
            return Err(field_error(
                source_label,
                format!("{prefix}.allowed[{j}]"),
                format!("duplicate capability '{}'", cap.as_str()),
            ));
        }
    }

    Ok(())
}

fn validate_config_entry(
    entry: &ConfigEntry,
    index: usize,
    source_label: &str,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<(), ManifestError> {
    let prefix = format!("config[{index}]");
    let Some(def) = crate::governance::config::key_def(&entry.key) else {
        return Err(field_error(
            source_label,
            format!("{prefix}.key"),
            format!("unknown config key '{}'", entry.key),
        ));
    };
    crate::governance::config::layers::validate_value(def, &entry.value, domain_pattern_valid)
        .map_err(|reason| field_error(source_label, format!("{prefix}.value"), reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_valid_host(_: &str) -> bool {
        true
    }

    /// A test-local mirror of the section-5.1 STRICT domain-pattern grammar (never
    /// `browser::pattern::is_valid_pattern` itself -- the a7 arch-test forbids that edge even in
    /// test code). Deliberately does NOT accept bare `*` -- that carve-out is
    /// [`is_valid_host_pattern`]'s job, applied only to grant `hosts` fields, never to this
    /// checker directly (mirroring the real `domain_pattern_valid` = `is_valid_pattern` wiring).
    /// The authoritative implementation and its own exhaustive bypass-class tests live in
    /// `browser::pattern`.
    fn is_valid_pattern(p: &str) -> bool {
        if p.is_empty() || !p.is_ascii() {
            return false;
        }
        if p.contains('/')
            || p.contains(':')
            || p.contains('@')
            || p.chars().any(char::is_whitespace)
        {
            return false;
        }
        let host = match p.strip_prefix("*.") {
            Some(rest) if !rest.is_empty() && !rest.contains('*') => rest,
            Some(_) => return false,
            None if p.contains('*') => return false,
            None => p,
        };
        if host.starts_with('.') || host.ends_with('.') {
            return false;
        }
        host.split('.').all(|label| !label.is_empty())
    }

    fn minimal_json() -> String {
        r#"{"schema":3,"name":"a","version":"1","grants":[]}"#.to_string()
    }

    #[test]
    fn minimal_manifest_parses_with_expected_defaults() {
        let m = parse_manifest(&minimal_json(), "test", always_valid_host).unwrap();
        assert_eq!(m.schema, 3);
        assert_eq!(m.name, "a");
        assert_eq!(m.version, "1");
        assert_eq!(m.mode, None);
        assert_eq!(m.identity, None);
        assert!(m.grants.is_empty());
        assert!(m.config.is_empty());
        assert_eq!(m.hash.len(), 64);
        assert!(m
            .hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn schema_3_shape_is_accepted() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[
            {"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"]}
        ]}"#;
        let m = parse_manifest(json, "test", is_valid_pattern).unwrap();
        assert_eq!(m.grants[0].hosts.allow, vec!["example.com".to_string()]);
        assert_eq!(m.grants[0].allowed, vec![Capability::Read]);
    }

    #[test]
    fn missing_schema_is_unsupported() {
        let json = r#"{"name":"a","version":"1","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::UnsupportedSchema { .. }));
        assert!(err.to_string().contains("expected 3"));
    }

    #[test]
    fn non_integer_schema_is_unsupported() {
        let json = r#"{"schema":"3","name":"a","version":"1","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::UnsupportedSchema { .. }));
    }

    /// Schema-2 precise error, including the ADR-0022 pointer (Required behavior section 1).
    #[test]
    fn schema_2_is_unsupported_with_the_adr_pointer() {
        let json = r#"{"schema":2,"name":"a","version":"1","grants":[
            {"id":"g1","domains":["example.com"],"access":"read"}
        ]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        match err {
            ManifestError::UnsupportedSchema { found, .. } => assert_eq!(found, "2"),
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
        let msg = err_to_string(json);
        assert!(msg.contains("expected 3"), "{msg}");
        assert!(msg.contains("ADR-0022"), "{msg}");
        assert!(
            msg.contains("hosts/allowed"),
            "{msg}: must name the new grant shape"
        );
    }

    fn err_to_string(json: &str) -> String {
        parse_manifest(json, "test", always_valid_host)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn schema_1_is_unsupported_with_no_adr_note() {
        let json = r#"{"schema":1,"name":"a","version":"1"}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        match err {
            ManifestError::UnsupportedSchema {
                found, adr_note, ..
            } => {
                assert_eq!(found, "1");
                assert!(adr_note.is_empty(), "{adr_note}");
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn missing_name_is_a_shape_error() {
        let json = r#"{"schema":3,"version":"1","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn empty_name_is_a_field_error() {
        let json = r#"{"schema":3,"name":"","version":"1","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Field { ref path, .. } if path == "name"));
    }

    #[test]
    fn missing_version_is_a_shape_error() {
        let json = r#"{"schema":3,"name":"a","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn unknown_top_level_field_is_rejected() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],"defaults":{}}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
        assert!(err.to_string().contains("defaults"));
    }

    #[test]
    fn authored_hash_field_is_rejected() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],"hash":"deadbeef"}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
        assert!(err.to_string().contains("hash"));
    }

    #[test]
    fn invalid_mode_enum_value_is_a_shape_error() {
        let json = r#"{"schema":3,"name":"a","version":"1","mode":"audit","grants":[]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn mode_observe_and_enforce_parse_at_manifest_and_grant_level() {
        let json = r#"{"schema":3,"name":"a","version":"1","mode":"observe","grants":[
            {"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"mode":"enforce"}
        ]}"#;
        let m = parse_manifest(json, "test", always_valid_host).unwrap();
        assert_eq!(
            m.mode,
            Some(crate::governance::ports::EffectiveMode::Observe)
        );
        assert_eq!(
            m.grants[0].mode,
            Some(crate::governance::ports::EffectiveMode::Enforce)
        );
    }

    #[test]
    fn missing_grants_is_a_shape_error() {
        let json = r#"{"schema":3,"name":"a","version":"1"}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn grants_not_an_array_is_a_shape_error() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":{}}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    fn grant_json(body: &str) -> String {
        format!(r#"{{"schema":3,"name":"a","version":"1","grants":[{body}]}}"#)
    }

    #[test]
    fn grant_missing_id_is_a_shape_error() {
        let json = grant_json(r#"{"hosts":{"allow":["example.com"]},"allowed":["read"]}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn duplicate_grant_ids_are_rejected() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[
                {"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"]},
                {"id":"g1","hosts":{"allow":["other.com"]},"allowed":["read"]}
            ]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Field { .. }));
        assert!(err.to_string().contains("duplicate grant id 'g1'"));
    }

    /// `hosts` is required on every grant (a grant without a `hosts` member is a shape error);
    /// its two members default to empty.
    #[test]
    fn grant_missing_hosts_is_a_shape_error() {
        let json = grant_json(r#"{"id":"g1","allowed":["read"]}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    /// Empty `hosts` and `allowed: []` are both VALID (they express "nothing").
    #[test]
    fn empty_hosts_and_empty_allowed_are_valid() {
        let json = grant_json(r#"{"id":"g1","hosts":{},"allowed":[]}"#);
        let m = parse_manifest(&json, "test", always_valid_host).unwrap();
        assert!(m.grants[0].hosts.allow.is_empty());
        assert!(m.grants[0].hosts.deny.is_empty());
        assert!(m.grants[0].allowed.is_empty());
    }

    #[test]
    fn invalid_host_patterns_are_each_rejected_with_the_pattern_named() {
        let cases = [
            "https://example.com",
            "example.com:8443",
            "example.com/path",
            "user@example.com",
            "ex*mple.com",
            "*.",
            "foo.*.com",
            ".example.com",
            "example.com.",
            "example..com",
            "",
        ];
        for pattern in cases {
            let json = grant_json(&format!(
                r#"{{"id":"g1","hosts":{{"allow":[{}]}},"allowed":["read"]}}"#,
                serde_json::to_string(pattern).unwrap()
            ));
            let err = parse_manifest(&json, "test", is_valid_pattern).unwrap_err();
            match err {
                ManifestError::Field { path, reason, .. } => {
                    assert_eq!(path, "grants[0].hosts.allow[0]");
                    assert!(
                        reason.contains(pattern),
                        "reason: {reason}, pattern: {pattern}"
                    );
                }
                other => panic!("pattern {pattern:?}: expected a Field error, got {other:?}"),
            }
        }
    }

    /// Bare `*` is legal ONLY in schema-3 `hosts` lists (ADR-0022 Decision 4 rule 1); this
    /// module applies that carve-out itself, inline, on top of the STRICT injected
    /// `domain_pattern_valid` (`is_valid_pattern` here, `browser::pattern::is_valid_pattern` in
    /// production -- neither accepts `*` directly). The real checker's own tests
    /// (`browser::polarity::is_valid_host_rule_accepts_star_and_delegates_the_rest`) pin the
    /// equivalent behavior for the standalone function; [`config_entry_star_pattern_is_rejected`]
    /// below pins that `content.security.sacred_domains` still rejects `*` through this SAME
    /// `domain_pattern_valid` used with no carve-out.
    #[test]
    fn bare_star_is_accepted_in_hosts_allow_and_deny() {
        let json =
            grant_json(r#"{"id":"g1","hosts":{"allow":["*"],"deny":["*"]},"allowed":["read"]}"#);
        let m = parse_manifest(&json, "test", is_valid_pattern).unwrap();
        assert_eq!(m.grants[0].hosts.allow, vec!["*".to_string()]);
        assert_eq!(m.grants[0].hosts.deny, vec!["*".to_string()]);
    }

    #[test]
    fn deny_pattern_invalid_is_also_rejected() {
        let json = grant_json(
            r#"{"id":"g1","hosts":{"allow":["example.com"],"deny":["bad*pattern"]},"allowed":["read"]}"#,
        );
        let err = parse_manifest(&json, "test", is_valid_pattern).unwrap_err();
        match err {
            ManifestError::Field { path, .. } => assert_eq!(path, "grants[0].hosts.deny[0]"),
            other => panic!("expected a Field error, got {other:?}"),
        }
    }

    #[test]
    fn invalid_capability_name_is_rejected() {
        let json =
            grant_json(r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["mutate"]}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn duplicate_capability_is_rejected() {
        let json = grant_json(
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read","read"]}"#,
        );
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Field { .. }));
        assert!(err.to_string().contains("duplicate capability 'read'"));
    }

    #[test]
    fn unknown_field_inside_hosts_is_rejected() {
        let json = grant_json(
            r#"{"id":"g1","hosts":{"allow":["example.com"],"unexpected":true},"allowed":["read"]}"#,
        );
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    /// The removed schema-2 fields (`domains`, `access`, `tools`, `exclude_tools`) are rejected
    /// as unknown fields by `deny_unknown_fields`.
    #[test]
    fn schema_2_grant_fields_are_unknown_fields() {
        for body in [
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"domains":["example.com"]}"#,
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"access":"read"}"#,
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"tools":["navigate"]}"#,
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"exclude_tools":["navigate"]}"#,
        ] {
            let json = grant_json(body);
            let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
            assert!(matches!(err, ManifestError::Shape { .. }), "{body}");
        }
    }

    #[test]
    fn grant_mode_shadow_is_a_shape_error() {
        let json = grant_json(
            r#"{"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"],"mode":"shadow"}"#,
        );
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    fn config_json(entry: &str) -> String {
        format!(r#"{{"schema":3,"name":"a","version":"1","grants":[],"config":[{entry}]}}"#)
    }

    #[test]
    fn config_entry_unregistered_key_is_rejected() {
        let json = config_json(r#"{"key":"no.such.key","value":true,"level":"mandatory"}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Field { .. }));
        assert!(err.to_string().contains("no.such.key"));
    }

    #[test]
    fn config_entry_wrong_value_type_is_rejected() {
        let json = config_json(r#"{"key":"audit.enabled","value":"yes","level":"mandatory"}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Field { .. }));
    }

    /// The bare `*` carve-out (ADR-0022 Decision 4 rule 1) applies ONLY to grant `hosts`
    /// fields, via [`is_valid_host_pattern`]; `content.security.sacred_domains` config
    /// validation calls the injected `domain_pattern_valid` directly, with no such carve-out,
    /// so `*` is still rejected there even though it is legal in a grant's `hosts.allow`.
    #[test]
    fn config_entry_star_pattern_is_rejected_even_though_hosts_accept_it() {
        let json = config_json(
            r#"{"key":"content.security.sacred_domains","value":["*"],"level":"mandatory"}"#,
        );
        let err = parse_manifest(&json, "test", is_valid_pattern).unwrap_err();
        assert!(matches!(err, ManifestError::Field { .. }), "{err:?}");
    }

    #[test]
    fn config_entry_missing_level_is_a_shape_error() {
        let json = config_json(r#"{"key":"audit.enabled","value":true}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn config_entry_level_optional_is_a_shape_error() {
        let json = config_json(r#"{"key":"audit.enabled","value":true,"level":"optional"}"#);
        let err = parse_manifest(&json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn identity_groups_as_a_string_is_a_shape_error() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],
            "identity":{"groups":"not-an-array"}}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn unknown_field_inside_identity_is_rejected() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],
            "identity":{"resolved_by":"x","unexpected":"y"}}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Shape { .. }));
    }

    #[test]
    fn syntax_error_carries_line_and_column() {
        let err = parse_manifest("{not json", "test", always_valid_host).unwrap_err();
        assert!(matches!(err, ManifestError::Syntax { .. }));
    }

    /// ADR-0023 Decision 3: a `config` array with the same key twice is a field error naming
    /// the duplicate key at the second occurrence's `config[1].key` path.
    #[test]
    fn duplicate_config_key_is_a_field_error() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],"config":[
            {"key":"audit.enabled","value":true,"level":"mandatory"},
            {"key":"audit.enabled","value":false,"level":"recommended"}
        ]}"#;
        let err = parse_manifest(json, "test", always_valid_host).unwrap_err();
        match err {
            ManifestError::Field { path, reason, .. } => {
                assert_eq!(path, "config[1].key");
                assert_eq!(reason, "duplicate config key 'audit.enabled'");
            }
            other => panic!("expected a Field error, got {other:?}"),
        }
    }
}
