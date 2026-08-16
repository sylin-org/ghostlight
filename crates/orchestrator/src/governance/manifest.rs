//! Strict schema-3 policy documents and their canonical identity.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{Capability, CapabilitySet};

/// Whether Ghostlight may start a browser when admitted work finds none connected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserStartup {
    /// Ghostlight may request one bounded recovery attempt.
    OnDemand,
    /// Ghostlight diagnoses the missing browser and leaves launching to the person.
    Manual,
}

impl BrowserStartup {
    /// Resolve the platform default without consulting policy.
    #[must_use]
    pub const fn default_for_platform(windows: bool) -> Self {
        if windows {
            Self::OnDemand
        } else {
            Self::Manual
        }
    }

    /// Decode the setting's closed string vocabulary.
    #[must_use]
    pub const fn from_str(value: &str) -> Option<Self> {
        match value.as_bytes() {
            b"on_demand" => Some(Self::OnDemand),
            b"manual" => Some(Self::Manual),
            _ => None,
        }
    }

    /// Stable policy and presentation vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OnDemand => "on_demand",
            Self::Manual => "manual",
        }
    }
}

/// Whether an ordinary policy denial blocks work or is reported while work continues.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyMode {
    /// Denials are authoritative.
    #[default]
    Enforce,
    /// Ordinary denials are observed; permanent protected-resource denials still enforce.
    Observe,
}

impl PolicyMode {
    /// Compose two tiers without allowing a lower tier to weaken enforcement.
    #[must_use]
    pub const fn strictest(self, other: Self) -> Self {
        if matches!(self, Self::Enforce) || matches!(other, Self::Enforce) {
            Self::Enforce
        } else {
            Self::Observe
        }
    }

    /// Stable policy and audit vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Observe => "observe",
        }
    }
}

/// Informational identity resolved before a manifest was authored.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityBlock {
    /// How the principal was resolved.
    #[serde(default)]
    pub resolved_by: Option<String>,
    /// Human-plain principal label.
    #[serde(default)]
    pub principal: Option<String>,
    /// Optional group labels used to explain the resolved policy.
    #[serde(default)]
    pub groups: Option<Vec<String>>,
    /// Informational resolution time.
    #[serde(default)]
    pub resolved_at: Option<String>,
}

/// Who authored this policy, addressed to the person it governs.
///
/// Informational only. Nothing here grants, denies, or participates in a decision; it exists so a
/// governed person can see who is restricting them and where to ask about it, which every mature
/// managed-device surface names in a sentence rather than leaving anonymous (ADR-0122 Decision 3).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationBlock {
    /// Display name of the authoring organization.
    pub name: String,
    /// The organization's own explanation of why this policy exists.
    #[serde(default)]
    pub statement: Option<String>,
    /// An HTTPS page the organization publishes about this policy.
    ///
    /// Presented as text. The workbench opens destinations from a closed vocabulary and never an
    /// authored address, so carrying one here can never turn into a reachable link.
    #[serde(default)]
    pub url: Option<String>,
    /// Channels a governed person may use to ask about this policy.
    #[serde(default)]
    pub contacts: Vec<OrganizationContact>,
}

/// One organization contact channel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationContact {
    /// Channel kind, such as email or url.
    pub kind: String,
    /// Channel address.
    pub value: String,
    /// Optional organization-authored display label.
    #[serde(default)]
    pub label: Option<String>,
}

/// Host allow/deny polarity for one grant.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostRules {
    /// Host patterns this grant covers.
    #[serde(default)]
    pub allow: Vec<String>,
    /// Holes carved only out of this grant.
    #[serde(default)]
    pub deny: Vec<String>,
}

/// One ordered host-scoped RAWX grant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// Stable unique attribution id.
    pub id: String,
    /// Host polarity for this grant.
    pub hosts: HostRules,
    /// Independent capability facts this grant permits.
    pub allowed: Vec<Capability>,
    /// Optional human-plain purpose.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional override of the manifest mode.
    #[serde(default)]
    pub mode: Option<PolicyMode>,
}

impl Grant {
    /// Return the grant's independent allowed set.
    #[must_use]
    pub fn allowed_set(&self) -> CapabilitySet {
        self.allowed.iter().copied().collect()
    }
}

/// Which monotonic policy layer an authored setting targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingLevel {
    /// A ceiling that no lower source can relax.
    Mandatory,
    /// A default that may still only tighten a higher ceiling in 1.0.
    Recommended,
}

impl SettingLevel {
    /// Stable policy and explanation vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mandatory => "mandatory",
            Self::Recommended => "recommended",
        }
    }
}

/// One narrowly registered schema-3 setting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntry {
    /// Registered key.
    pub key: String,
    /// Typed JSON value validated for the key.
    pub value: Value,
    /// Authored policy level.
    pub level: SettingLevel,
}

/// One strict schema-3 policy manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be exactly 3.
    pub schema: u32,
    /// Stable human-plain policy name.
    pub name: String,
    /// Authored policy version label.
    pub version: String,
    /// Manifest default mode.
    #[serde(default)]
    pub mode: Option<PolicyMode>,
    /// Informational resolved identity.
    #[serde(default)]
    pub identity: Option<IdentityBlock>,
    /// Informational authoring organization.
    #[serde(default)]
    pub organization: Option<OrganizationBlock>,
    /// Ordered grants. May be empty.
    pub grants: Vec<Grant>,
    /// Narrow monotonic product settings.
    #[serde(default)]
    pub config: Vec<ConfigEntry>,
    /// Canonical SHA-256 identity, never authored.
    #[serde(skip)]
    pub hash: String,
}

impl Manifest {
    /// Resolve one boolean setting if this manifest authors it.
    #[must_use]
    pub fn boolean_setting(&self, key: &str) -> Option<bool> {
        self.config
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.value.as_bool())
    }

    /// Resolve one string setting if this manifest authors it.
    #[must_use]
    pub fn string_setting(&self, key: &str) -> Option<&str> {
        self.config
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.value.as_str())
    }

    /// Resolve the browser startup setting if this manifest authors it.
    #[must_use]
    pub fn browser_startup(&self) -> Option<BrowserStartup> {
        self.string_setting("browser.startup")
            .and_then(BrowserStartup::from_str)
    }

    /// Resolve one string-array setting if this manifest authors it.
    #[must_use]
    pub fn string_array_setting(&self, key: &str) -> Option<Vec<String>> {
        self.config
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| entry.value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
    }
}

/// Why a schema-3 manifest could not become authority.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ManifestError {
    /// The text is not JSON.
    #[error("{origin}: JSON syntax at line {line}, column {column}: {message}")]
    Syntax {
        /// Human-plain source label.
        origin: String,
        /// One-based line.
        line: usize,
        /// One-based column.
        column: usize,
        /// Parser detail.
        message: String,
    },
    /// The schema marker is absent or unsupported.
    #[error("{origin}: unsupported policy schema {found}; expected 3")]
    UnsupportedSchema {
        /// Human-plain source label.
        origin: String,
        /// Authored JSON value or `<missing>`.
        found: String,
    },
    /// The JSON shape does not match the strict document.
    #[error("{origin}: {message}")]
    Shape {
        /// Human-plain source label.
        origin: String,
        /// Serde shape detail.
        message: String,
    },
    /// A typed field is semantically invalid.
    #[error("{origin}: {path}: {reason}")]
    Field {
        /// Human-plain source label.
        origin: String,
        /// Dotted/indexed field location.
        path: String,
        /// Corrective reason.
        reason: String,
    },
}

/// Upper bound on structural nesting any real schema-3 document needs. The deepest legitimate
/// shape is manifest -> grants -> grant -> hosts -> allow, five levels; this leaves generous room
/// for growth while still rejecting a document nested deep enough to threaten the parser below.
const MAX_NESTING_DEPTH: usize = 64;

/// Reject a document nested deeper than any real manifest could need, before it ever reaches a
/// recursive-descent parser with no depth limit of its own.
///
/// `serde_json::from_str` recurses once per nesting level with no cap, and a deeply nested
/// document -- comfortably under any byte-size limit callers enforce -- overflows the call stack
/// before parsing finishes. That aborts the whole process: it is not a panic, so `catch_unwind`
/// cannot intercept it, and on this product's desktop build the orchestrator's real authority
/// shares a process with the Tauri shell that can hand this function untrusted WebView input. A
/// linear scan of raw bracket depth, skipping string contents, is cheap and catches this first.
fn reject_excessive_nesting(text: &str, source: &str) -> Result<(), ManifestError> {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_NESTING_DEPTH {
                    return Err(ManifestError::Shape {
                        origin: source.into(),
                        message: format!("nested more than {MAX_NESTING_DEPTH} levels deep"),
                    });
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Parse, validate, and canonically identify one policy manifest.
pub fn parse(text: &str, source: &str) -> Result<Manifest, ManifestError> {
    let stripped = text.strip_prefix('\u{feff}').unwrap_or(text);
    reject_excessive_nesting(stripped, source)?;
    let value: Value = serde_json::from_str(stripped).map_err(|error| ManifestError::Syntax {
        origin: source.into(),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })?;
    if value.get("schema").and_then(Value::as_u64) != Some(3) {
        return Err(ManifestError::UnsupportedSchema {
            origin: source.into(),
            found: value
                .get("schema")
                .map(Value::to_string)
                .unwrap_or_else(|| "<missing>".into()),
        });
    }
    let mut manifest: Manifest =
        serde_json::from_str(stripped).map_err(|error| ManifestError::Shape {
            origin: source.into(),
            message: error.to_string(),
        })?;
    validate(&manifest, source)?;
    manifest.hash = canonical_hash(&value);
    Ok(manifest)
}

fn validate(manifest: &Manifest, source: &str) -> Result<(), ManifestError> {
    bounded_nonempty(&manifest.name, 100, source, "name")?;
    bounded_nonempty(&manifest.version, 80, source, "version")?;
    if manifest.grants.len() > 256 {
        return field(source, "grants", "must contain at most 256 entries");
    }
    let mut ids = HashSet::new();
    for (index, grant) in manifest.grants.iter().enumerate() {
        validate_grant(grant, index, source, &mut ids)?;
    }
    if manifest.config.len() > 32 {
        return field(source, "config", "must contain at most 32 entries");
    }
    let mut keys = HashSet::new();
    for (index, entry) in manifest.config.iter().enumerate() {
        if !keys.insert(entry.key.as_str()) {
            return field(source, &format!("config[{index}].key"), "must be unique");
        }
        validate_config(entry, index, source)?;
    }
    if let Some(identity) = &manifest.identity {
        for (path, value) in [
            ("identity.resolved_by", identity.resolved_by.as_deref()),
            ("identity.principal", identity.principal.as_deref()),
            ("identity.resolved_at", identity.resolved_at.as_deref()),
        ] {
            if let Some(value) = value {
                bounded_nonempty(value, 300, source, path)?;
            }
        }
        if let Some(groups) = &identity.groups {
            if groups.len() > 64 {
                return field(source, "identity.groups", "must contain at most 64 entries");
            }
            for (index, group) in groups.iter().enumerate() {
                bounded_nonempty(group, 200, source, &format!("identity.groups[{index}]"))?;
            }
        }
    }
    if let Some(organization) = &manifest.organization {
        validate_organization(organization, source)?;
    }
    Ok(())
}

/// Bounds mirror the signed presentation block so the same organization facts survive either
/// delivery path unchanged.
fn validate_organization(
    organization: &OrganizationBlock,
    source: &str,
) -> Result<(), ManifestError> {
    bounded_nonempty(&organization.name, 100, source, "organization.name")?;
    if let Some(statement) = &organization.statement {
        bounded_nonempty(statement, 500, source, "organization.statement")?;
    }
    if let Some(url) = &organization.url {
        bounded_nonempty(url, 300, source, "organization.url")?;
        if !url.starts_with("https://") {
            return field(source, "organization.url", "must be an https address");
        }
    }
    if organization.contacts.len() > 8 {
        return field(
            source,
            "organization.contacts",
            "must contain at most 8 entries",
        );
    }
    for (index, contact) in organization.contacts.iter().enumerate() {
        let prefix = format!("organization.contacts[{index}]");
        bounded_nonempty(&contact.kind, 32, source, &format!("{prefix}.kind"))?;
        bounded_nonempty(&contact.value, 240, source, &format!("{prefix}.value"))?;
        if let Some(label) = &contact.label {
            bounded_nonempty(label, 80, source, &format!("{prefix}.label"))?;
        }
    }
    Ok(())
}

fn validate_grant(
    grant: &Grant,
    index: usize,
    source: &str,
    ids: &mut HashSet<String>,
) -> Result<(), ManifestError> {
    let prefix = format!("grants[{index}]");
    bounded_nonempty(&grant.id, 80, source, &format!("{prefix}.id"))?;
    if !grant
        .id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return field(
            source,
            &format!("{prefix}.id"),
            "must use only ASCII letters, digits, dash, underscore, or dot",
        );
    }
    if !ids.insert(grant.id.clone()) {
        return field(source, &format!("{prefix}.id"), "must be unique");
    }
    if let Some(description) = &grant.description {
        bounded_nonempty(description, 500, source, &format!("{prefix}.description"))?;
    }
    validate_patterns(&grant.hosts.allow, source, &format!("{prefix}.hosts.allow"))?;
    validate_patterns(&grant.hosts.deny, source, &format!("{prefix}.hosts.deny"))?;
    let mut capabilities = HashSet::new();
    for (capability_index, capability) in grant.allowed.iter().enumerate() {
        if !capabilities.insert(*capability) {
            return field(
                source,
                &format!("{prefix}.allowed[{capability_index}]"),
                "must be unique",
            );
        }
    }
    Ok(())
}

fn validate_patterns(patterns: &[String], source: &str, path: &str) -> Result<(), ManifestError> {
    if patterns.len() > 256 {
        return field(source, path, "must contain at most 256 entries");
    }
    let mut seen = HashSet::new();
    for (index, pattern) in patterns.iter().enumerate() {
        if !valid_host_pattern(pattern) {
            return field(
                source,
                &format!("{path}[{index}]"),
                "must be *, an exact hostname, or a *.suffix wildcard",
            );
        }
        if !seen.insert(pattern.to_ascii_lowercase()) {
            return field(source, &format!("{path}[{index}]"), "must be unique");
        }
    }
    Ok(())
}

fn validate_config(entry: &ConfigEntry, index: usize, source: &str) -> Result<(), ManifestError> {
    let path = format!("config[{index}].value");
    match entry.key.as_str() {
        "browser.tabs.allow_close"
        | "privacy.preserve_target_names"
        | "channels.mcp.enabled"
        | "channels.cli.enabled"
        | "policy.user.enabled" => {
            if !entry.value.is_boolean() {
                return field(source, &path, "must be a boolean");
            }
        }
        "content.security.sacred_domains" => {
            let Some(items) = entry.value.as_array() else {
                return field(source, &path, "must be an array of host patterns");
            };
            let patterns: Option<Vec<String>> = items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect();
            let Some(patterns) = patterns else {
                return field(source, &path, "must contain only strings");
            };
            if patterns.iter().any(|pattern| pattern == "*") {
                return field(source, &path, "must not contain the universal pattern");
            }
            validate_patterns(&patterns, source, &path)?;
        }
        "browser.startup" => {
            let Some(value) = entry.value.as_str() else {
                return field(source, &path, "must be a string");
            };
            if BrowserStartup::from_str(value).is_none() {
                return field(source, &path, "must be on_demand or manual");
            }
        }
        _ => {
            return field(
                source,
                &format!("config[{index}].key"),
                "is not a registered 1.0 policy setting",
            )
        }
    }
    Ok(())
}

fn bounded_nonempty(
    value: &str,
    maximum: usize,
    source: &str,
    path: &str,
) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        return field(source, path, "must not be empty");
    }
    if value.chars().count() > maximum {
        return field(
            source,
            path,
            &format!("must be at most {maximum} characters"),
        );
    }
    Ok(())
}

fn field<T>(source: &str, path: &str, reason: &str) -> Result<T, ManifestError> {
    Err(ManifestError::Field {
        origin: source.into(),
        path: path.into(),
        reason: reason.into(),
    })
}

/// Whether one authored grant host pattern has valid bounded syntax.
#[must_use]
pub fn valid_host_pattern(pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let host = pattern.strip_prefix("*.").unwrap_or(pattern);
    !host.is_empty()
        && host.len() <= 253
        && host.is_ascii()
        && !host.contains(['/', ':', '*'])
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn canonical_hash(value: &Value) -> String {
    let mut canonical = String::new();
    write_canonical(value, &mut canonical);
    let mut hash = String::with_capacity(64);
    for byte in Sha256::digest(canonical.as_bytes()) {
        write!(&mut hash, "{byte:02x}").expect("writing to a string cannot fail");
    }
    hash
}

fn write_canonical(value: &Value, output: &mut String) {
    match value {
        Value::Object(object) => {
            output.push('{');
            let mut keys: Vec<_> = object.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).expect("JSON object key serializes"));
                output.push(':');
                write_canonical(&object[key], output);
            }
            output.push('}');
        }
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical(item, output);
            }
            output.push(']');
        }
        _ => output.push_str(&serde_json::to_string(value).expect("JSON value serializes")),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, valid_host_pattern, BrowserStartup, PolicyMode};

    #[test]
    fn a_deeply_nested_document_is_rejected_before_it_reaches_the_recursive_parser() {
        // Depth far beyond anything a real manifest needs, and far beyond what this crate's own
        // test stack could survive parsing recursively if the guard were absent -- the guard must
        // reject it on a linear scan, never by calling into serde_json's Value parser at all.
        let bomb = format!(
            r#"{{"schema":3,"name":"x","version":"1","grants":[],"config":[{{"key":"content.security.sacred_domains","value":{}[]{},"level":"mandatory"}}]}}"#,
            "[".repeat(5_000),
            "]".repeat(5_000)
        );
        let error = parse(&bomb, "bomb").unwrap_err();
        assert!(error.to_string().contains("nested more than"), "{error}");

        // A string containing bracket-shaped text must not itself be counted as nesting. 80
        // brackets exceeds MAX_NESTING_DEPTH if the scanner mistakenly counted them, while
        // staying under the unrelated 100-character bound `name` already enforces.
        let deceptive = format!(
            r#"{{"schema":3,"name":"{}","version":"1","grants":[]}}"#,
            "[".repeat(80)
        );
        assert!(parse(&deceptive, "deceptive").is_ok());
    }

    #[test]
    fn schema_three_is_strict_and_has_a_format_independent_hash() {
        let first = parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[]}"#,
            "first",
        )
        .unwrap();
        let second = parse(
            "{\n  \"grants\": [], \"version\": \"1\", \"name\": \"test\", \"schema\": 3\n}",
            "second",
        )
        .unwrap();
        assert_eq!(first.hash, second.hash);
        assert!(parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"surprise":true}"#,
            "strict"
        )
        .is_err());
        assert!(parse(r#"{"version":1,"allow_capabilities":["read"]}"#, "old").is_err());
    }

    #[test]
    fn grants_reject_duplicate_identity_capabilities_and_patterns() {
        for text in [
            r#"{"schema":3,"name":"test","version":"1","grants":[{"id":"same","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"same","hosts":{"allow":["other.com"]},"allowed":["read"]}]}"#,
            r#"{"schema":3,"name":"test","version":"1","grants":[{"id":"g","hosts":{"allow":["example.com"]},"allowed":["read","read"]}]}"#,
            r#"{"schema":3,"name":"test","version":"1","grants":[{"id":"g","hosts":{"allow":["example.com","EXAMPLE.com"]},"allowed":["read"]}]}"#,
        ] {
            assert!(parse(text, "duplicate").is_err());
        }
    }

    #[test]
    fn host_patterns_are_exact_suffix_or_universal() {
        for valid in ["*", "example.com", "*.example.com", "a-b.example"] {
            assert!(valid_host_pattern(valid), "{valid}");
        }
        for invalid in [
            "",
            "https://example.com",
            "example.com/path",
            "foo.*.example.com",
            "*.example.com:443",
            ".example.com",
        ] {
            assert!(!valid_host_pattern(invalid), "{invalid}");
        }
    }

    #[test]
    fn organization_identity_is_optional_bounded_and_additive() {
        let anonymous = parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[]}"#,
            "plain",
        )
        .unwrap();
        assert!(anonymous.organization.is_none());

        let named = parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"organization":{"name":"Example Organization","statement":"Keeps browser work inside approved sites.","url":"https://example.com/policy","contacts":[{"kind":"email","value":"security@example.com","label":"Security team"}]}}"#,
            "named",
        )
        .unwrap();
        let organization = named.organization.expect("organization block parses");
        assert_eq!(organization.name, "Example Organization");
        assert_eq!(organization.contacts.len(), 1);
        assert_eq!(organization.contacts[0].value, "security@example.com");

        for rejected in [
            r#"{"schema":3,"name":"test","version":"1","grants":[],"organization":{"name":""}}"#,
            r#"{"schema":3,"name":"test","version":"1","grants":[],"organization":{"name":"Example","url":"http://example.com"}}"#,
            r#"{"schema":3,"name":"test","version":"1","grants":[],"organization":{"name":"Example","surprise":true}}"#,
            r#"{"schema":3,"name":"test","version":"1","grants":[],"organization":{}}"#,
        ] {
            assert!(parse(rejected, "rejected").is_err(), "{rejected}");
        }
    }

    #[test]
    fn the_user_layer_switch_is_a_registered_boolean_setting() {
        assert!(parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"config":[{"key":"policy.user.enabled","value":false,"level":"mandatory"}]}"#,
            "registered"
        )
        .is_ok());
        assert!(parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"config":[{"key":"policy.user.enabled","value":"no","level":"mandatory"}]}"#,
            "typed"
        )
        .is_err());
    }

    #[test]
    fn browser_startup_accepts_only_the_two_closed_values() {
        for value in ["on_demand", "manual"] {
            let document = format!(
                r#"{{"schema":3,"name":"test","version":"1","grants":[],"config":[{{"key":"browser.startup","value":"{value}","level":"mandatory"}}]}}"#
            );
            let policy = parse(&document, "registered").expect("closed value parses");
            assert_eq!(policy.string_setting("browser.startup"), Some(value));
            assert_eq!(policy.browser_startup(), BrowserStartup::from_str(value));
        }
    }

    #[test]
    fn browser_startup_refuses_an_unknown_value() {
        assert!(parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"config":[{"key":"browser.startup","value":"automatic","level":"mandatory"}]}"#,
            "unknown"
        )
        .is_err());
    }

    #[test]
    fn browser_startup_refuses_a_non_string_value() {
        assert!(parse(
            r#"{"schema":3,"name":"test","version":"1","grants":[],"config":[{"key":"browser.startup","value":true,"level":"mandatory"}]}"#,
            "typed"
        )
        .is_err());
    }

    #[test]
    fn browser_startup_defaults_per_platform() {
        assert_eq!(
            BrowserStartup::default_for_platform(true),
            BrowserStartup::OnDemand
        );
        assert_eq!(
            BrowserStartup::default_for_platform(false),
            BrowserStartup::Manual
        );
    }

    #[test]
    fn enforce_is_the_strict_mode() {
        assert_eq!(
            PolicyMode::Observe.strictest(PolicyMode::Enforce),
            PolicyMode::Enforce
        );
        assert_eq!(
            PolicyMode::Observe.strictest(PolicyMode::Observe),
            PolicyMode::Observe
        );
    }
}
