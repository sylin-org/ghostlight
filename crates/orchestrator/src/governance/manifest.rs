//! Strict schema-4 policy documents and their canonical identity.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{Capability, CapabilitySet};
use crate::intake::session::IntakeChannel;

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

/// Informational identity resolved before a manifest was authored.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityBlock {
    /// How the principal was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// Human-plain principal label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    /// Optional group labels used to explain the resolved policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
    /// Informational resolution time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement: Option<String>,
    /// An HTTPS page the organization publishes about this policy.
    ///
    /// Presented as text. The workbench opens destinations from a closed vocabulary and never an
    /// authored address, so carrying one here can never turn into a reachable link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Channels a governed person may use to ask about this policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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

/// One registered schema-4 policy setting: the closed vocabulary of `config[].key`.
///
/// Validation, assembly and every reader name a setting through this type, so a key is spelled
/// once, in [`Self::key`]. A document whose key is not one of these is refused.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PolicySetting {
    /// `audit.availability`: whether work continues while history cannot be saved.
    AuditAvailability,
    /// `browser.tabs.allow_close`: `false` removes model-driven tab close.
    TabsAllowClose,
    /// `browser.screenshots.enabled`: `false` refuses complete screenshots, including visible
    /// embedded documents.
    ScreenshotsEnabled,
    /// `browser.startup`: whether Ghostlight may start a browser when none is connected.
    BrowserStartup,
    /// `channels.mcp.enabled`: `false` refuses every request on the `mcp` channel.
    McpChannelEnabled,
    /// `channels.cli.enabled`: `false` refuses every request on the `cli` channel.
    CliChannelEnabled,
    /// `privacy.preserve_target_names`: `false` keeps page-authored target names out of results and
    /// audit.
    PreserveTargetNames,
    /// `content.security.sacred_domains`: the destinations a layer marks never-touch.
    NeverTouch,
}

impl PolicySetting {
    /// Every registered setting.
    pub const ALL: [Self; 8] = [
        Self::AuditAvailability,
        Self::TabsAllowClose,
        Self::ScreenshotsEnabled,
        Self::BrowserStartup,
        Self::McpChannelEnabled,
        Self::CliChannelEnabled,
        Self::PreserveTargetNames,
        Self::NeverTouch,
    ];

    /// The key a schema-4 document authors for this setting.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::AuditAvailability => "audit.availability",
            Self::TabsAllowClose => "browser.tabs.allow_close",
            Self::ScreenshotsEnabled => "browser.screenshots.enabled",
            Self::BrowserStartup => "browser.startup",
            Self::McpChannelEnabled => "channels.mcp.enabled",
            Self::CliChannelEnabled => "channels.cli.enabled",
            Self::PreserveTargetNames => "privacy.preserve_target_names",
            Self::NeverTouch => "content.security.sacred_domains",
        }
    }

    /// The registered setting an authored key names, or `None` for a key this schema lacks.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|setting| setting.key() == key)
    }

    /// The switch that turns `channel` on or off (ADR-0202 Decision 22a). Unset means on.
    #[must_use]
    pub const fn channel_enabled(channel: IntakeChannel) -> Self {
        match channel {
            IntakeChannel::Mcp => Self::McpChannelEnabled,
            IntakeChannel::Cli => Self::CliChannelEnabled,
        }
    }

    /// Refuse a value this setting does not accept, naming `path` in `source`.
    fn check(self, value: &Value, source: &str, path: &str) -> Result<(), ManifestError> {
        match self {
            Self::AuditAvailability => {
                if value
                    .as_str()
                    .and_then(super::audit_requirement::AuditMode::parse)
                    .is_none()
                {
                    return field(source, path, "must be keep_working or require_audit");
                }
            }
            Self::TabsAllowClose
            | Self::ScreenshotsEnabled
            | Self::McpChannelEnabled
            | Self::CliChannelEnabled
            | Self::PreserveTargetNames => {
                if !value.is_boolean() {
                    return field(source, path, "must be a boolean");
                }
            }
            Self::NeverTouch => {
                let Some(items) = value.as_array() else {
                    return field(source, path, "must be an array of host patterns");
                };
                let patterns: Option<Vec<String>> = items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned))
                    .collect();
                let Some(patterns) = patterns else {
                    return field(source, path, "must contain only strings");
                };
                if patterns.iter().any(|pattern| pattern == "*") {
                    return field(source, path, "must not contain the universal pattern");
                }
                validate_patterns(&patterns, source, path)?;
            }
            Self::BrowserStartup => {
                let Some(value) = value.as_str() else {
                    return field(source, path, "must be a string");
                };
                if BrowserStartup::from_str(value).is_none() {
                    return field(source, path, "must be on_demand or manual");
                }
            }
        }
        Ok(())
    }
}

/// One narrowly registered setting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigEntry {
    /// The authored key, one of [`PolicySetting::key`] once the document is validated.
    pub key: String,
    /// Typed JSON value validated for the key.
    pub value: Value,
    /// Authored policy level.
    pub level: SettingLevel,
}

/// One strict schema-4 policy manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Must be exactly [`SCHEMA`].
    pub schema: u32,
    /// Stable human-plain policy name.
    pub name: String,
    /// Authored policy version label.
    pub version: String,
    /// Informational resolved identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentityBlock>,
    /// Informational authoring organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<OrganizationBlock>,
    /// Ordered grants. May be empty.
    pub grants: Vec<Grant>,
    /// Narrow monotonic product settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<ConfigEntry>,
    /// Canonical SHA-256 identity, never authored.
    #[serde(skip)]
    pub hash: String,
}

impl Manifest {
    /// The authored value of `setting`, if this manifest authors it.
    fn setting(&self, setting: PolicySetting) -> Option<&Value> {
        self.config
            .iter()
            .find(|entry| entry.key == setting.key())
            .map(|entry| &entry.value)
    }

    /// Resolve one boolean setting if this manifest authors it.
    #[must_use]
    pub fn boolean_setting(&self, setting: PolicySetting) -> Option<bool> {
        self.setting(setting).and_then(Value::as_bool)
    }

    /// Resolve one string setting if this manifest authors it.
    #[must_use]
    pub fn string_setting(&self, setting: PolicySetting) -> Option<&str> {
        self.setting(setting).and_then(Value::as_str)
    }

    /// Resolve the browser startup setting if this manifest authors it.
    #[must_use]
    pub fn browser_startup(&self) -> Option<BrowserStartup> {
        self.string_setting(PolicySetting::BrowserStartup)
            .and_then(BrowserStartup::from_str)
    }

    /// Resolve one string-array setting if this manifest authors it.
    #[must_use]
    pub fn string_array_setting(&self, setting: PolicySetting) -> Option<Vec<String>> {
        self.setting(setting)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
    }
}

/// Why a manifest could not become authority.
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
    /// The document is schema 3, which `ghostlight policy migrate` can carry forward.
    #[error("{origin}: unsupported policy schema 3; run ghostlight policy migrate <file>")]
    RetiredSchema {
        /// Human-plain source label.
        origin: String,
    },
    /// The schema marker is absent or unsupported.
    #[error("{origin}: unsupported policy schema {found}; expected 4")]
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

/// The one policy schema this build reads.
pub const SCHEMA: u64 = 4;

/// The policy schema `ghostlight policy migrate` reads and no other path accepts.
pub const RETIRED_SCHEMA: u64 = 3;

/// Upper bound on structural nesting any real document needs. The deepest legitimate
/// shape is manifest -> grants -> grant -> hosts -> allow, five levels; this leaves generous room
/// for growth while still rejecting a document nested deep enough to threaten the parser below.
const MAX_NESTING_DEPTH: usize = 64;

/// Reject a document nested deeper than any real manifest could need, before it ever reaches a
/// recursive-descent parser with no depth limit of its own.
///
/// `serde_json::from_str` recurses once per nesting level with no cap, and a deeply nested
/// document -- comfortably under any byte-size limit callers enforce -- overflows the call stack
/// before parsing finishes. That aborts the whole process: it is not a panic, so `catch_unwind`
/// cannot intercept it, and the service that parses policy runs every agent's work in the same
/// process. A linear scan of raw bracket depth, skipping string contents, is cheap and catches this first.
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

/// Read one document as JSON, without yet asking what schema it speaks.
pub(super) fn read_value(text: &str, source: &str) -> Result<Value, ManifestError> {
    let stripped = text.strip_prefix('\u{feff}').unwrap_or(text);
    reject_excessive_nesting(stripped, source)?;
    serde_json::from_str(stripped).map_err(|error| ManifestError::Syntax {
        origin: source.into(),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

/// Parse, validate, and canonically identify one policy manifest.
pub fn parse(text: &str, source: &str) -> Result<Manifest, ManifestError> {
    let value = read_value(text, source)?;
    match value.get("schema").and_then(Value::as_u64) {
        Some(SCHEMA) => {}
        Some(RETIRED_SCHEMA) => {
            return Err(ManifestError::RetiredSchema {
                origin: source.into(),
            })
        }
        _ => {
            return Err(ManifestError::UnsupportedSchema {
                origin: source.into(),
                found: value
                    .get("schema")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "<missing>".into()),
            })
        }
    }
    let mut manifest: Manifest =
        serde_json::from_value(value.clone()).map_err(|error| ManifestError::Shape {
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
        if super::host_pattern::HostPattern::parse(pattern).is_none() {
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
    let Some(setting) = PolicySetting::from_key(&entry.key) else {
        return field(
            source,
            &format!("config[{index}].key"),
            "is not a registered policy setting",
        );
    };
    setting.check(&entry.value, source, &format!("config[{index}].value"))
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
    use serde_json::{json, Value};

    use super::{parse, BrowserStartup, ManifestError, PolicySetting, MAX_NESTING_DEPTH};
    use crate::language::migration::RETIRED_SETTINGS;

    /// 6c138338: a document nested past `MAX_NESTING_DEPTH` is refused by a linear scan before
    /// serde recurses, so a small hostile policy cannot overflow the stack and abort the service.
    /// Brackets inside a string do not count.
    #[test]
    fn a_deeply_nested_document_is_rejected_before_it_reaches_the_recursive_parser() {
        // Depth far beyond anything a real manifest needs, and far beyond what this crate's own
        // test stack could survive parsing recursively if the guard were absent -- the guard must
        // reject it on a linear scan, never by calling into serde_json's Value parser at all.
        let bomb = format!(
            r#"{{"schema":4,"name":"x","version":"1","grants":[],"config":[{{"key":"content.security.sacred_domains","value":{}[]{},"level":"mandatory"}}]}}"#,
            "[".repeat(5_000),
            "]".repeat(5_000)
        );
        assert!(
            matches!(
                parse(&bomb, "bomb").unwrap_err(),
                ManifestError::Shape { message, .. }
                    if message == format!("nested more than {MAX_NESTING_DEPTH} levels deep")
            ),
            "the nesting guard, not the parser, refuses it"
        );

        // A string holding more brackets than MAX_NESTING_DEPTH is not nesting, and stays under
        // the unrelated 100-character bound `name` already enforces.
        let deceptive = format!(
            r#"{{"schema":4,"name":"{}","version":"1","grants":[]}}"#,
            "[".repeat(MAX_NESTING_DEPTH + 16)
        );
        assert!(parse(&deceptive, "deceptive").is_ok());
    }

    /// ACCEPTANCE Governance 4 and the governance guide: the decoder reads schema 4 alone. Schema
    /// 3 is refused with the migrate sentence the guide quotes, any other or missing marker as
    /// unsupported, and a schema-4 document carrying observe mode, the user switch, a frame option
    /// or any unknown field is refused.
    #[test]
    fn only_schema_four_without_its_retired_parts_is_read() {
        let document = |schema: &str, rest: &str| {
            format!(r#"{{{schema}"name":"test","version":"1","grants":[]{rest}}}"#)
        };
        assert!(parse(&document(r#""schema":4,"#, ""), "current").is_ok());

        let retired = parse(&document(r#""schema":3,"#, ""), "old.json").unwrap_err();
        assert!(matches!(retired, ManifestError::RetiredSchema { .. }));
        assert_eq!(
            retired.to_string(),
            "old.json: unsupported policy schema 3; run ghostlight policy migrate <file>"
        );
        for marker in [r#""schema":2,"#, r#""schema":5,"#, r#""schema":"4","#, ""] {
            assert!(
                matches!(
                    parse(&document(marker, ""), "other").unwrap_err(),
                    ManifestError::UnsupportedSchema { .. }
                ),
                "{marker}"
            );
        }

        for retired in RETIRED_SETTINGS {
            let key = retired.config_key().expect("a retired setting has a key");
            let text = document(
                r#""schema":4,"#,
                &format!(r#","config":[{{"key":"{key}","value":false,"level":"mandatory"}}]"#),
            );
            assert!(
                matches!(
                    parse(&text, "retired").unwrap_err(),
                    ManifestError::Field { path, .. } if path == "config[0].key"
                ),
                "{key}"
            );
        }
        for text in [
            r#"{"schema":4,"name":"test","version":"1","mode":"enforce","grants":[]}"#,
            r#"{"schema":4,"name":"test","version":"1","grants":[{"id":"g","mode":"observe","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
            r#"{"schema":4,"name":"test","version":"1","grants":[],"surprise":true}"#,
        ] {
            assert!(
                matches!(
                    parse(text, "retired").unwrap_err(),
                    ManifestError::Shape { .. }
                ),
                "{text}"
            );
        }
    }

    /// One value `setting` accepts, for documents that exercise every registered key.
    fn accepted(setting: PolicySetting) -> Value {
        match setting {
            PolicySetting::AuditAvailability => json!("require_audit"),
            PolicySetting::TabsAllowClose
            | PolicySetting::ScreenshotsEnabled
            | PolicySetting::McpChannelEnabled
            | PolicySetting::CliChannelEnabled
            | PolicySetting::PreserveTargetNames => json!(false),
            PolicySetting::BrowserStartup => json!("manual"),
            PolicySetting::NeverTouch => json!(["vault.example", "*.bank.example"]),
        }
    }

    /// A schema-4 document authoring `setting` as `value`.
    fn authoring(setting: PolicySetting, value: Value) -> String {
        json!({"schema": 4, "name": "one", "version": "1", "grants": [],
            "config": [{"key": setting.key(), "value": value, "level": "mandatory"}]})
        .to_string()
    }

    /// ADR-0203 P7 and the governance guide: the setting keys administrators' documents use. Each
    /// registered setting is read under its documented key and written back unchanged, as the
    /// migration and the managed path re-serialize a manifest. A value of the wrong type is
    /// refused for every setting, and each closed string setting reads its own values and refuses
    /// any other.
    #[test]
    fn every_registered_setting_parses_and_round_trips() {
        // The schema-4 keys, spelled out: a renamed variant must not change a document.
        assert_eq!(
            PolicySetting::ALL.map(PolicySetting::key),
            [
                "audit.availability",
                "browser.tabs.allow_close",
                "browser.screenshots.enabled",
                "browser.startup",
                "channels.mcp.enabled",
                "channels.cli.enabled",
                "privacy.preserve_target_names",
                "content.security.sacred_domains",
            ]
        );
        let config: Vec<Value> = PolicySetting::ALL
            .into_iter()
            .map(|setting| {
                json!({"key": setting.key(), "value": accepted(setting), "level": "mandatory"})
            })
            .collect();
        let document =
            json!({"schema": 4, "name": "every", "version": "1", "grants": [], "config": config});
        let policy = parse(&document.to_string(), "every").expect("every setting is registered");
        for (setting, entry) in PolicySetting::ALL.into_iter().zip(&policy.config) {
            assert_eq!(entry.key, setting.key());
            assert_eq!(entry.value, accepted(setting));
        }
        assert_eq!(
            policy.string_setting(PolicySetting::AuditAvailability),
            Some("require_audit")
        );
        assert_eq!(
            policy.boolean_setting(PolicySetting::TabsAllowClose),
            Some(false)
        );
        assert_eq!(policy.browser_startup(), Some(BrowserStartup::Manual));
        assert_eq!(
            policy.string_array_setting(PolicySetting::NeverTouch),
            Some(vec![
                "vault.example".to_owned(),
                "*.bank.example".to_owned()
            ])
        );

        let written = serde_json::to_string(&policy).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&written).unwrap(),
            document,
            "the document is written back unchanged"
        );
        assert_eq!(parse(&written, "written").unwrap(), policy);

        for setting in PolicySetting::ALL {
            assert!(
                parse(&authoring(setting, json!({})), "wrong").is_err(),
                "{setting:?} refuses a value of the wrong type"
            );
        }
        for (setting, values, unknown) in [
            (
                PolicySetting::BrowserStartup,
                ["on_demand", "manual"],
                "automatic",
            ),
            (
                PolicySetting::AuditAvailability,
                ["keep_working", "require_audit"],
                "silent",
            ),
        ] {
            for value in values {
                let policy = parse(&authoring(setting, json!(value)), "closed").unwrap();
                assert_eq!(policy.string_setting(setting), Some(value), "{setting:?}");
            }
            assert!(
                parse(&authoring(setting, json!(unknown)), "unknown").is_err(),
                "{setting:?} refuses {unknown}"
            );
        }
        assert_eq!(
            parse(
                &authoring(PolicySetting::BrowserStartup, json!("on_demand")),
                "startup"
            )
            .unwrap()
            .browser_startup(),
            Some(BrowserStartup::OnDemand)
        );
    }
}
