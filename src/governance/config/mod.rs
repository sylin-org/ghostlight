//! Governance configuration -- the typed registry of policy keys (ADR-0019).
//!
//! The full policy engine (manifest parsing, grants, per-call enforcement; SPEC sec 4-5) is
//! **staged**: engine correctness ships and stabilizes first, then the governance layer lands
//! on a proven engine, so the two layers are never debugged at the same time. The design
//! stance this module encodes:
//!
//! - The **engine is always truthful.** The extension and tools return raw page content; they
//!   make no access or redaction decisions (SPEC sec 9.5: the binary governs *structurally*,
//!   not by inspecting content semantically).
//! - Governed behavior is an **overlay** expressed as typed configuration keys. Each governed
//!   code path reads its setting from [`Config`] instead of hardcoding it.
//!
//! [`KEYS`] is the single static registry: names, descriptions, constraints, and one default
//! per preset ([`Preset::FullyOpen`], [`Preset::Safe`], [`Preset::Restricted`]). The built-in
//! Minimal defaults equal the Safe preset ("Safe is today's Minimal", ADR-0019).
//! [`Config::from_preset`] builds every field FROM the registry, never from duplicated
//! literals, so the registry stays the single source of truth. Layer resolution (file
//! loading, precedence, the resolved-value triple) is G02's job; this module only defines the
//! typed primitive G02 (and the CLI, JSON Schema, and native-messaging surfaces after it)
//! consume.
//!
//! This module is the domain-agnostic config CORE (RECONCILIATION.md section 1): it names no
//! browser type. One key ([`CONTENT_SECURITY_SACRED_DOMAINS`]) constrains its values to valid
//! domain patterns, but the actual pattern grammar is browser-domain and lives in the browser
//! plugin (kept out of this module by the a7 arch-test). [`KeyDef::parse_value`] takes the
//! validator as an injected function pointer rather than naming the browser plugin directly
//! (RECONCILIATION.md section 2, "known integration point").
//!
//! [`layers`] implements the ADR-0019 five-layer precedence model; [`load`] loads the two
//! configuration files of the shared format doc section 1 and produces the layer inputs.
//! [`Config::from_resolution`] builds the typed session `Config` from a [`layers::Resolution`].
//! [`reload`] holds the in-force snapshot behind an atomic swap and re-resolves it live on a
//! debounced file-watch, so config and org-policy changes take effect with no restart.
//! [`cli`] is the `browser-mcp config list/get/set` presentation surface over this registry.
//! [`schema`] generates the JSON Schema and markdown key reference (`config schema`/`docs`).
//! [`presets`] is `config preset` (G18, ADR-0019 decision 3): selecting a named bundle of
//! layer-4 defaults, shown as a plain-language diff before it writes anything.

pub mod cli;
pub mod layers;
pub mod load;
pub mod presets;
pub mod reload;
pub mod schema;

use crate::governance::ports::EffectiveMode;

/// A configuration preset: a named bundle of layer-4 defaults (shared format doc section 2).
/// The built-in Minimal defaults (layer 5) equal the Safe preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    FullyOpen,
    Safe,
    Restricted,
}

impl Preset {
    /// The wire/file name of this preset: "fully_open", "safe", or "restricted". This is the
    /// exact string written to and read from the user config file's `preset` field.
    pub fn as_str(&self) -> &'static str {
        match self {
            Preset::FullyOpen => "fully_open",
            Preset::Safe => "safe",
            Preset::Restricted => "restricted",
        }
    }

    /// The CLI-facing spelling: "fully-open", "safe", "restricted" (G18). Distinct from
    /// [`Preset::as_str`], the underscore form written to the user config file.
    pub fn cli_name(&self) -> &'static str {
        match self {
            Preset::FullyOpen => "fully-open",
            Preset::Safe => "safe",
            Preset::Restricted => "restricted",
        }
    }

    /// Parse a preset name as written in config files. Returns `None` for unknown names.
    pub fn from_name(name: &str) -> Option<Preset> {
        match name {
            "fully_open" => Some(Preset::FullyOpen),
            "safe" => Some(Preset::Safe),
            "restricted" => Some(Preset::Restricted),
            _ => None,
        }
    }
}

/// Every registered key's default value under `preset`, as a JSON map: layer 4's contribution
/// to [`layers::LayerInputs`] (shared format doc section 2, G18). Selecting a preset means
/// exactly this -- populate layer 4 with the preset's per-key defaults -- and nothing else: it
/// never writes a per-key value into any other layer.
pub fn preset_layer(preset: Preset) -> serde_json::Map<String, serde_json::Value> {
    KEYS.iter()
        .map(|def| {
            let value: ConfigValue = def.default_for(preset).into();
            (def.key.to_string(), value.to_json())
        })
        .collect()
}

/// A statically-declared default value for a registry key (one per preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValue {
    Bool(bool),
    Uint(u64),
    Enum(&'static str),
    Str(&'static str),
    StrList(&'static [&'static str]),
}

/// An owned, validated configuration value at runtime. Produced by [`KeyDef::parse_value`] and
/// by converting a static [`KeyValue`] default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    Bool(bool),
    Uint(u64),
    Enum(String),
    Str(String),
    StrList(Vec<String>),
}

impl From<KeyValue> for ConfigValue {
    fn from(v: KeyValue) -> Self {
        match v {
            KeyValue::Bool(b) => ConfigValue::Bool(b),
            KeyValue::Uint(u) => ConfigValue::Uint(u),
            KeyValue::Enum(s) => ConfigValue::Enum(s.to_string()),
            KeyValue::Str(s) => ConfigValue::Str(s.to_string()),
            KeyValue::StrList(list) => {
                ConfigValue::StrList(list.iter().map(|s| s.to_string()).collect())
            }
        }
    }
}

impl ConfigValue {
    /// Render as the JSON shape the registered key's type uses (shared format doc 3.2). Later
    /// tasks (`config list`, `get_config`) render resolved values through this.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            ConfigValue::Bool(b) => serde_json::Value::Bool(*b),
            ConfigValue::Uint(u) => serde_json::Value::Number((*u).into()),
            ConfigValue::Enum(s) | ConfigValue::Str(s) => serde_json::Value::String(s.clone()),
            ConfigValue::StrList(list) => serde_json::Value::Array(
                list.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        }
    }
}

/// The value type of a registry key, in the shared format doc's type vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    Bool,
    Uint,
    Enum,
    Str,
    StrList,
}

impl KeyType {
    /// The wire name: "bool", "uint", "enum", "string", or "string_list" (shared format doc
    /// section 9.2 vocabulary).
    pub fn name(&self) -> &'static str {
        match self {
            KeyType::Bool => "bool",
            KeyType::Uint => "uint",
            KeyType::Enum => "enum",
            KeyType::Str => "string",
            KeyType::StrList => "string_list",
        }
    }
}

/// Extra validation attached to a key beyond its base type (shared format doc 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyConstraint {
    /// Base-type check only.
    None,
    /// Uint keys: inclusive bounds. Every Uint key MUST declare this.
    UintRange { min: u64, max: u64 },
    /// Enum keys: the closed set of legal variants. Every Enum key MUST declare this.
    EnumVariants(&'static [&'static str]),
    /// Str keys: the empty string, or an absolute filesystem path
    /// (`std::path::Path::is_absolute`).
    EmptyOrAbsolutePath,
    /// StrList keys: each element must be a valid domain pattern (shared format doc 5.1),
    /// checked through the `domain_pattern_valid` callback injected into
    /// [`KeyDef::parse_value`]. The concrete grammar lives in the browser plugin, kept out of
    /// this core module per the a7 arch-test boundary (RECONCILIATION.md section 2).
    DomainPatternList,
}

/// A value failed validation against a [`KeyDef`]. Display strings are user-facing: they
/// appear verbatim in CLI errors and the native-messaging `invalid_value` message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValueError {
    #[error("expected a boolean")]
    ExpectedBool,
    #[error("expected an integer between {min} and {max}")]
    ExpectedUint { min: u64, max: u64 },
    #[error("expected one of: {}", .variants.join(", "))]
    ExpectedVariant { variants: &'static [&'static str] },
    #[error("expected a string")]
    ExpectedString,
    #[error("expected an empty string or an absolute path")]
    ExpectedAbsolutePath,
    #[error("expected an array of strings")]
    ExpectedStringList,
    #[error("duplicate list entry: {0}")]
    DuplicateEntry(String),
    #[error("invalid domain pattern: {0}")]
    InvalidDomainPattern(String),
}

/// A governance configuration key: a stable dotted name, a human description, its validation
/// constraint, and one default per preset. The static [`KEYS`] table is the single source of
/// truth for the whole configurable surface (ADR-0019); the CLI, extension UI, JSON Schema,
/// and docs are all generated from it.
#[derive(Debug, Clone, Copy)]
pub struct KeyDef {
    /// Stable dotted identifier, e.g. `content.security.secrets.redact`.
    pub key: &'static str,
    /// What the key governs. Surfaced verbatim by config UIs.
    pub description: &'static str,
    /// Validation beyond the base type.
    pub constraint: KeyConstraint,
    /// Default under the "fully_open" preset.
    pub default_fully_open: KeyValue,
    /// Default under the "safe" preset. The built-in Minimal defaults equal these.
    pub default_safe: KeyValue,
    /// Default under the "restricted" preset.
    pub default_restricted: KeyValue,
}

impl KeyDef {
    /// The default value for `preset`.
    pub fn default_for(&self, preset: Preset) -> KeyValue {
        match preset {
            Preset::FullyOpen => self.default_fully_open,
            Preset::Safe => self.default_safe,
            Preset::Restricted => self.default_restricted,
        }
    }

    /// The base value type, derived from the "safe" default's variant. A registry-integrity
    /// unit test guarantees all three presets share one variant, so deriving from
    /// `default_safe` alone is sound.
    pub fn key_type(&self) -> KeyType {
        match self.default_safe {
            KeyValue::Bool(_) => KeyType::Bool,
            KeyValue::Uint(_) => KeyType::Uint,
            KeyValue::Enum(_) => KeyType::Enum,
            KeyValue::Str(_) => KeyType::Str,
            KeyValue::StrList(_) => KeyType::StrList,
        }
    }

    /// Validate a JSON value against this key's type and constraint, returning the owned typed
    /// value on success. This is the single validation path for every write surface (config
    /// files, CLI, native-messaging settings; those land in later tasks).
    ///
    /// `domain_pattern_valid` is consulted only when `constraint` is
    /// [`KeyConstraint::DomainPatternList`]; every other key ignores it. Callers outside the
    /// governance core pass the browser plugin's real pattern-syntax checker; this module
    /// cannot name it directly (the a7 arch-test forbids a `governance -> browser` edge).
    pub fn parse_value(
        &self,
        value: &serde_json::Value,
        domain_pattern_valid: fn(&str) -> bool,
    ) -> Result<ConfigValue, ConfigValueError> {
        match self.key_type() {
            KeyType::Bool => value
                .as_bool()
                .map(ConfigValue::Bool)
                .ok_or(ConfigValueError::ExpectedBool),
            KeyType::Uint => {
                let (min, max) = match self.constraint {
                    KeyConstraint::UintRange { min, max } => (min, max),
                    _ => (0, u64::MAX),
                };
                match value.as_u64() {
                    Some(v) if v >= min && v <= max => Ok(ConfigValue::Uint(v)),
                    _ => Err(ConfigValueError::ExpectedUint { min, max }),
                }
            }
            KeyType::Enum => {
                let variants = match self.constraint {
                    KeyConstraint::EnumVariants(variants) => variants,
                    _ => &[],
                };
                match value.as_str() {
                    Some(s) if variants.contains(&s) => Ok(ConfigValue::Enum(s.to_string())),
                    _ => Err(ConfigValueError::ExpectedVariant { variants }),
                }
            }
            KeyType::Str => {
                let s = value.as_str().ok_or(ConfigValueError::ExpectedString)?;
                if matches!(self.constraint, KeyConstraint::EmptyOrAbsolutePath)
                    && !s.is_empty()
                    && !std::path::Path::new(s).is_absolute()
                {
                    return Err(ConfigValueError::ExpectedAbsolutePath);
                }
                Ok(ConfigValue::Str(s.to_string()))
            }
            KeyType::StrList => {
                let arr = value
                    .as_array()
                    .ok_or(ConfigValueError::ExpectedStringList)?;
                let mut items = Vec::with_capacity(arr.len());
                let mut seen = std::collections::HashSet::new();
                for item in arr {
                    let s = item.as_str().ok_or(ConfigValueError::ExpectedStringList)?;
                    if !seen.insert(s.to_string()) {
                        return Err(ConfigValueError::DuplicateEntry(s.to_string()));
                    }
                    if matches!(self.constraint, KeyConstraint::DomainPatternList)
                        && !domain_pattern_valid(s)
                    {
                        return Err(ConfigValueError::InvalidDomainPattern(s.to_string()));
                    }
                    items.push(s.to_string());
                }
                Ok(ConfigValue::StrList(items))
            }
        }
    }
}

/// Look up a key definition by its dotted name. `None` for unregistered names.
pub fn key_def(key: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|k| k.key == key)
}

/// `engine.connection.first_call_wait_ms` -- upper bound on the first-call wait for the
/// extension handshake (ADR-0017).
pub const ENGINE_CONNECTION_FIRST_CALL_WAIT_MS: &str = "engine.connection.first_call_wait_ms";

/// `content.security.secrets.redact` -- when true, values of fields the page itself marks
/// secret (input `type=password`/`hidden`, or a sensitive `autocomplete` token) are replaced
/// with `[value redacted]` in `read_page` output before it leaves the binary. The engine still
/// returns the raw value (marked); this key only governs whether the overlay redacts it.
pub const CONTENT_SECURITY_SECRETS_REDACT: &str = "content.security.secrets.redact";

/// `content.security.sacred_domains` -- user-authored never-touch domain patterns (ADR-0018
/// step 2). Always enforced regardless of `governance.mode` or manifest presence, at the
/// dispatch chokepoint (`browser::sacred`, `transport::mcp::server`). Values are validated
/// against the section 5.1 pattern grammar (`browser::pattern::is_valid_pattern`) at config
/// load; matching semantics live in `browser::pattern`/`browser::sacred`.
pub const CONTENT_SECURITY_SACRED_DOMAINS: &str = "content.security.sacred_domains";

/// `audit.enabled` -- record one audit line per tool call (the flight recorder, ADR-0018 step
/// 1).
pub const AUDIT_ENABLED: &str = "audit.enabled";

/// `audit.destination` -- where audit records are written (`file` or `stderr`; `syslog`,
/// `http`, and `none` are deferred beyond stage 2).
pub const AUDIT_DESTINATION: &str = "audit.destination";

/// `audit.file.path` -- audit file path; empty means the platform default location.
pub const AUDIT_FILE_PATH: &str = "audit.file.path";

/// `governance.mode` -- the manifest-level default enforcement mode when the active manifest
/// does not set its own. Precedence for the effective mode of a decision: per-grant `mode` >
/// manifest `mode` > this resolved value.
pub const GOVERNANCE_MODE: &str = "governance.mode";

/// The static registry of every governance key: the single source of truth for names,
/// descriptions, constraints, and per-preset defaults (shared format doc section 3.4). The
/// `restricted` preset equals `safe` for every stage-2 key by design; it is registered now so
/// the preset name is stable.
pub const KEYS: &[KeyDef] = &[
    KeyDef {
        key: ENGINE_CONNECTION_FIRST_CALL_WAIT_MS,
        description: "Upper bound on the first-call wait for the extension handshake.",
        constraint: KeyConstraint::UintRange {
            min: 0,
            max: 60000,
        },
        default_fully_open: KeyValue::Uint(5000),
        default_safe: KeyValue::Uint(5000),
        default_restricted: KeyValue::Uint(5000),
    },
    KeyDef {
        key: CONTENT_SECURITY_SECRETS_REDACT,
        description: "Redact values of secret fields (password/OTP/payment) in read_page output.",
        constraint: KeyConstraint::None,
        default_fully_open: KeyValue::Bool(false),
        default_safe: KeyValue::Bool(true),
        default_restricted: KeyValue::Bool(true),
    },
    KeyDef {
        key: CONTENT_SECURITY_SACRED_DOMAINS,
        description: "Domains the agent must never touch: any tool call on a tab showing one of these domains, and any navigation targeting one, is denied. Always enforced.",
        constraint: KeyConstraint::DomainPatternList,
        default_fully_open: KeyValue::StrList(&[]),
        default_safe: KeyValue::StrList(&[]),
        default_restricted: KeyValue::StrList(&[]),
    },
    KeyDef {
        key: AUDIT_ENABLED,
        description: "Record one audit line per tool call (the flight recorder).",
        constraint: KeyConstraint::None,
        default_fully_open: KeyValue::Bool(false),
        default_safe: KeyValue::Bool(true),
        default_restricted: KeyValue::Bool(true),
    },
    KeyDef {
        key: AUDIT_DESTINATION,
        description: "Where audit records are written.",
        constraint: KeyConstraint::EnumVariants(&["file", "stderr"]),
        default_fully_open: KeyValue::Enum("file"),
        default_safe: KeyValue::Enum("file"),
        default_restricted: KeyValue::Enum("file"),
    },
    KeyDef {
        key: AUDIT_FILE_PATH,
        description: "Audit file path; empty means the platform default location.",
        constraint: KeyConstraint::EmptyOrAbsolutePath,
        default_fully_open: KeyValue::Str(""),
        default_safe: KeyValue::Str(""),
        default_restricted: KeyValue::Str(""),
    },
    KeyDef {
        key: GOVERNANCE_MODE,
        description: "Default enforcement mode when the active manifest does not set one: observe records shadow denials, enforce blocks.",
        constraint: KeyConstraint::EnumVariants(&["observe", "enforce"]),
        default_fully_open: KeyValue::Enum("observe"),
        default_safe: KeyValue::Enum("enforce"),
        default_restricted: KeyValue::Enum("enforce"),
    },
];

/// Extract a registered key's preset default as `bool`. Panics on a registry/type mismatch:
/// unreachable for a well-formed registry, and every preset is exercised by
/// `every_preset_default_parses_against_its_own_key` plus the `Config` construction tests, so
/// drift is caught by `cargo test`, never at runtime in the field.
fn preset_bool(key: &str, preset: Preset) -> bool {
    match key_def(key).expect("registered key").default_for(preset) {
        KeyValue::Bool(b) => b,
        other => panic!("key {key} default is not Bool: {other:?}"),
    }
}

/// Extract a registered key's preset default as `u64`. See [`preset_bool`] for the panic
/// rationale.
fn preset_uint(key: &str, preset: Preset) -> u64 {
    match key_def(key).expect("registered key").default_for(preset) {
        KeyValue::Uint(u) => u,
        other => panic!("key {key} default is not Uint: {other:?}"),
    }
}

/// Extract a registered key's preset default as an owned `String` (Enum or Str). See
/// [`preset_bool`] for the panic rationale.
fn preset_string_like(key: &str, preset: Preset) -> String {
    match key_def(key).expect("registered key").default_for(preset) {
        KeyValue::Enum(s) | KeyValue::Str(s) => s.to_string(),
        other => panic!("key {key} default is not Enum or Str: {other:?}"),
    }
}

/// Extract a registered key's preset default as an owned `Vec<String>`. See [`preset_bool`]
/// for the panic rationale.
fn preset_str_list(key: &str, preset: Preset) -> Vec<String> {
    match key_def(key).expect("registered key").default_for(preset) {
        KeyValue::StrList(list) => list.iter().map(|s| s.to_string()).collect(),
        other => panic!("key {key} default is not StrList: {other:?}"),
    }
}

/// Extract a resolved key's JSON value as `bool`, falling back to the Safe preset default on
/// an unreachable-by-construction shape mismatch. See [`Config::from_resolution`] for the
/// fallback rationale.
fn resolved_bool(resolution: &layers::Resolution, key: &str) -> bool {
    let resolved = resolution.get(key).expect("registered key");
    match resolved.value.as_bool() {
        Some(b) => b,
        None => {
            debug_assert!(
                false,
                "resolved value for {key} is not a bool: {:?}",
                resolved.value
            );
            preset_bool(key, Preset::Safe)
        }
    }
}

/// Extract a resolved key's JSON value as `u64`. See [`resolved_bool`] for the fallback
/// rationale.
fn resolved_uint(resolution: &layers::Resolution, key: &str) -> u64 {
    let resolved = resolution.get(key).expect("registered key");
    match resolved.value.as_u64() {
        Some(u) => u,
        None => {
            debug_assert!(
                false,
                "resolved value for {key} is not a uint: {:?}",
                resolved.value
            );
            preset_uint(key, Preset::Safe)
        }
    }
}

/// Extract a resolved key's JSON value as an owned `String` (Enum or Str). See
/// [`resolved_bool`] for the fallback rationale.
fn resolved_string_like(resolution: &layers::Resolution, key: &str) -> String {
    let resolved = resolution.get(key).expect("registered key");
    match resolved.value.as_str() {
        Some(s) => s.to_string(),
        None => {
            debug_assert!(
                false,
                "resolved value for {key} is not a string: {:?}",
                resolved.value
            );
            preset_string_like(key, Preset::Safe)
        }
    }
}

/// Extract a resolved key's JSON value as an owned `Vec<String>`. See [`resolved_bool`] for
/// the fallback rationale.
fn resolved_str_list(resolution: &layers::Resolution, key: &str) -> Vec<String> {
    let resolved = resolution.get(key).expect("registered key");
    match resolved.value.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        None => {
            debug_assert!(
                false,
                "resolved value for {key} is not an array: {:?}",
                resolved.value
            );
            preset_str_list(key, Preset::Safe)
        }
    }
}

/// The governance configuration currently in force, with values typed for direct use by
/// governed code paths. `Config` is owned (not `Copy`): it holds `String`/`Vec<String>`
/// fields so a later re-resolve (hot-reload, A5) can swap in a fresh snapshot without a
/// lifetime constraint back to the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    first_call_wait_ms: u64,
    secrets_redact: bool,
    sacred_domains: Vec<String>,
    audit_enabled: bool,
    audit_destination: String,
    audit_file_path: String,
    governance_mode: EffectiveMode,
}

impl Config {
    /// Build every field from the registry's preset defaults (never from duplicated
    /// literals), so [`KEYS`] stays the single source of truth.
    pub fn from_preset(preset: Preset) -> Self {
        Self {
            first_call_wait_ms: preset_uint(ENGINE_CONNECTION_FIRST_CALL_WAIT_MS, preset),
            secrets_redact: preset_bool(CONTENT_SECURITY_SECRETS_REDACT, preset),
            sacred_domains: preset_str_list(CONTENT_SECURITY_SACRED_DOMAINS, preset),
            audit_enabled: preset_bool(AUDIT_ENABLED, preset),
            audit_destination: preset_string_like(AUDIT_DESTINATION, preset),
            audit_file_path: preset_string_like(AUDIT_FILE_PATH, preset),
            governance_mode: EffectiveMode::from_config_str(&preset_string_like(
                GOVERNANCE_MODE,
                preset,
            )),
        }
    }

    /// The built-in **"Minimal"** preset: safe-by-default. Equals [`Preset::Safe`] ("Safe is
    /// today's Minimal", ADR-0019).
    pub fn minimal() -> Self {
        Self::from_preset(Preset::Safe)
    }

    /// Build the typed session `Config` from a layer resolution ([`layers::resolve`]).
    /// Resolved values are already validated by the loaders, so conversion cannot fail; an
    /// impossible mismatch (a resolved JSON shape that does not match its key's declared type)
    /// falls back to the registry's Safe default. That fallback is unreachable by construction
    /// for a well-formed registry and a resolver that only ever inserts pre-validated values,
    /// and `debug_assert!` makes a violation loud in tests/debug builds rather than silently
    /// substituting the wrong value.
    pub fn from_resolution(resolution: &layers::Resolution) -> Self {
        Self {
            first_call_wait_ms: resolved_uint(resolution, ENGINE_CONNECTION_FIRST_CALL_WAIT_MS),
            secrets_redact: resolved_bool(resolution, CONTENT_SECURITY_SECRETS_REDACT),
            sacred_domains: resolved_str_list(resolution, CONTENT_SECURITY_SACRED_DOMAINS),
            audit_enabled: resolved_bool(resolution, AUDIT_ENABLED),
            audit_destination: resolved_string_like(resolution, AUDIT_DESTINATION),
            audit_file_path: resolved_string_like(resolution, AUDIT_FILE_PATH),
            governance_mode: EffectiveMode::from_config_str(&resolved_string_like(
                resolution,
                GOVERNANCE_MODE,
            )),
        }
    }

    /// Upper bound on the first-call wait for the extension handshake
    /// (`engine.connection.first_call_wait_ms`).
    pub fn first_call_wait_ms(&self) -> u64 {
        self.first_call_wait_ms
    }

    /// Whether secret field values must be redacted from `read_page` output
    /// (`content.security.secrets.redact`).
    pub fn secrets_redact(&self) -> bool {
        self.secrets_redact
    }

    /// User-authored never-touch domain patterns (`content.security.sacred_domains`).
    pub fn sacred_domains(&self) -> &[String] {
        &self.sacred_domains
    }

    /// Whether the audit flight recorder is enabled (`audit.enabled`).
    pub fn audit_enabled(&self) -> bool {
        self.audit_enabled
    }

    /// Where audit records are written (`audit.destination`).
    pub fn audit_destination(&self) -> &str {
        &self.audit_destination
    }

    /// Audit file path; empty means the platform default location (`audit.file.path`).
    pub fn audit_file_path(&self) -> &str {
        &self.audit_file_path
    }

    /// The default enforcement mode (`governance.mode`), parsed once at resolution time.
    pub fn governance_mode(&self) -> EffectiveMode {
        self.governance_mode
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::minimal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Test-only domain-pattern validator that mirrors the browser plugin's grammar. The
    /// browser plugin's pattern module owns the authoritative implementation and its
    /// exhaustive test list; this exists only so `parse_value`'s `DomainPatternList` wiring
    /// can be exercised in this module without depending on the browser plugin (the a7
    /// arch-test forbids that edge).
    fn test_domain_pattern_valid(pattern: &str) -> bool {
        if pattern.is_empty() || !pattern.is_ascii() {
            return false;
        }
        let host = match pattern.strip_prefix("*.") {
            Some(rest) if !rest.is_empty() && !rest.contains('*') => rest,
            Some(_) => return false,
            None if pattern.contains('*') => return false,
            None => pattern,
        };
        if host.starts_with('.') || host.ends_with('.') {
            return false;
        }
        host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
    }

    /// Stand-in for tests that must never actually invoke the domain-pattern validator. Every
    /// `content.security.sacred_domains` preset default is an empty list, so `parse_value`
    /// never reaches per-element validation on the values exercised below; a panicking stub
    /// makes that guarantee explicit rather than silently trusting it.
    fn unused_pattern_valid(_: &str) -> bool {
        panic!("domain pattern validator must not be called for this input")
    }

    #[test]
    fn every_key_name_is_dotted_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for k in KEYS {
            assert!(k.key.contains('.'), "{} should be a dotted key", k.key);
            assert!(seen.insert(k.key), "duplicate config key: {}", k.key);
            assert!(!k.description.is_empty(), "{} needs a description", k.key);
            for segment in k.key.split('.') {
                assert!(!segment.is_empty(), "{}: empty segment", k.key);
                assert!(
                    segment
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "{}: segment '{}' must match [a-z0-9_]+",
                    k.key,
                    segment
                );
            }
        }
    }

    #[test]
    fn every_key_defaults_share_one_type() {
        for k in KEYS {
            let d1 = std::mem::discriminant(&k.default_fully_open);
            let d2 = std::mem::discriminant(&k.default_safe);
            let d3 = std::mem::discriminant(&k.default_restricted);
            assert_eq!(d1, d2, "{}: fully_open/safe type mismatch", k.key);
            assert_eq!(d2, d3, "{}: safe/restricted type mismatch", k.key);
        }
    }

    #[test]
    fn every_typed_key_declares_its_constraint() {
        for k in KEYS {
            match k.key_type() {
                KeyType::Uint => match k.constraint {
                    KeyConstraint::UintRange { min, max } => {
                        assert!(min <= max, "{}: min > max", k.key)
                    }
                    _ => panic!("{}: Uint key must declare UintRange", k.key),
                },
                KeyType::Enum => match k.constraint {
                    KeyConstraint::EnumVariants(variants) => {
                        assert!(
                            variants.len() >= 2,
                            "{}: needs at least two variants",
                            k.key
                        );
                        for preset in [Preset::FullyOpen, Preset::Safe, Preset::Restricted] {
                            match k.default_for(preset) {
                                KeyValue::Enum(v) => assert!(
                                    variants.contains(&v),
                                    "{}: default {} not a declared variant",
                                    k.key,
                                    v
                                ),
                                _ => panic!("{}: default is not Enum", k.key),
                            }
                        }
                    }
                    _ => panic!("{}: Enum key must declare EnumVariants", k.key),
                },
                _ => {}
            }
        }
    }

    #[test]
    fn every_preset_default_parses_against_its_own_key() {
        for k in KEYS {
            for preset in [Preset::FullyOpen, Preset::Safe, Preset::Restricted] {
                let value: ConfigValue = k.default_for(preset).into();
                let json = value.to_json();
                k.parse_value(&json, unused_pattern_valid)
                    .unwrap_or_else(|e| panic!("{} ({:?}): {e}", k.key, preset));
            }
        }
    }

    #[test]
    fn minimal_config_matches_the_registry_defaults() {
        let cfg = Config::minimal();
        assert_eq!(
            cfg.first_call_wait_ms(),
            preset_uint(ENGINE_CONNECTION_FIRST_CALL_WAIT_MS, Preset::Safe)
        );
        assert_eq!(
            cfg.secrets_redact(),
            preset_bool(CONTENT_SECURITY_SECRETS_REDACT, Preset::Safe)
        );
        assert_eq!(
            cfg.sacred_domains(),
            preset_str_list(CONTENT_SECURITY_SACRED_DOMAINS, Preset::Safe).as_slice()
        );
        assert_eq!(
            cfg.audit_enabled(),
            preset_bool(AUDIT_ENABLED, Preset::Safe)
        );
        assert_eq!(
            cfg.audit_destination(),
            preset_string_like(AUDIT_DESTINATION, Preset::Safe)
        );
        assert_eq!(
            cfg.audit_file_path(),
            preset_string_like(AUDIT_FILE_PATH, Preset::Safe)
        );
        assert_eq!(
            cfg.governance_mode(),
            EffectiveMode::from_config_str(&preset_string_like(GOVERNANCE_MODE, Preset::Safe))
        );
    }

    /// t03 (ADR-0024 Decision 3, typed `governance_mode`): minimal and preset configs yield
    /// `EffectiveMode` values directly, with no `from_config_str` round-trip at the call site.
    #[test]
    fn governance_mode_is_typed() {
        assert_eq!(Config::minimal().governance_mode(), EffectiveMode::Enforce);
        assert_eq!(
            Config::from_preset(Preset::FullyOpen).governance_mode(),
            EffectiveMode::Observe
        );
    }

    #[test]
    fn restricted_preset_equals_safe_for_stage_2() {
        assert_eq!(
            Config::from_preset(Preset::Restricted),
            Config::from_preset(Preset::Safe)
        );
    }

    #[test]
    fn fully_open_preset_opens_the_governed_defaults() {
        let cfg = Config::from_preset(Preset::FullyOpen);
        assert!(!cfg.secrets_redact());
        assert!(!cfg.audit_enabled());
        assert_eq!(cfg.governance_mode(), EffectiveMode::Observe);
        assert_eq!(cfg.first_call_wait_ms(), 5000);
    }

    #[test]
    fn preset_names_round_trip() {
        for p in [Preset::FullyOpen, Preset::Safe, Preset::Restricted] {
            assert_eq!(Preset::from_name(p.as_str()), Some(p));
        }
        assert_eq!(Preset::from_name("Safe"), None);
        assert_eq!(Preset::from_name("full_open"), None);
        assert_eq!(Preset::from_name(""), None);
    }

    #[test]
    fn bool_key_parse_value() {
        let k = key_def(CONTENT_SECURITY_SECRETS_REDACT).unwrap();
        assert_eq!(
            k.parse_value(&json!(true), unused_pattern_valid),
            Ok(ConfigValue::Bool(true))
        );
        assert_eq!(
            k.parse_value(&json!(false), unused_pattern_valid),
            Ok(ConfigValue::Bool(false))
        );
        for bad in [json!("true"), json!(1), json!(null), json!({})] {
            assert_eq!(
                k.parse_value(&bad, unused_pattern_valid),
                Err(ConfigValueError::ExpectedBool)
            );
        }
    }

    #[test]
    fn uint_key_parse_value() {
        let k = key_def(ENGINE_CONNECTION_FIRST_CALL_WAIT_MS).unwrap();
        for ok in [json!(0), json!(5000), json!(60000)] {
            assert!(k.parse_value(&ok, unused_pattern_valid).is_ok());
        }
        let exp_err = ConfigValueError::ExpectedUint { min: 0, max: 60000 };
        assert_eq!(
            k.parse_value(&json!(60001), unused_pattern_valid),
            Err(exp_err.clone())
        );
        assert_eq!(
            k.parse_value(&json!(-1), unused_pattern_valid),
            Err(exp_err.clone())
        );
        assert_eq!(
            k.parse_value(&json!(1.5), unused_pattern_valid),
            Err(exp_err.clone())
        );
        assert_eq!(
            k.parse_value(&json!("5000"), unused_pattern_valid),
            Err(exp_err.clone())
        );
        let exponent: serde_json::Value = serde_json::from_str("5e3").unwrap();
        assert_eq!(
            k.parse_value(&exponent, unused_pattern_valid),
            Err(exp_err.clone())
        );
        assert_eq!(
            exp_err.to_string(),
            "expected an integer between 0 and 60000"
        );
    }

    #[test]
    fn enum_key_parse_value() {
        let k = key_def(AUDIT_DESTINATION).unwrap();
        assert_eq!(
            k.parse_value(&json!("file"), unused_pattern_valid),
            Ok(ConfigValue::Enum("file".into()))
        );
        assert_eq!(
            k.parse_value(&json!("stderr"), unused_pattern_valid),
            Ok(ConfigValue::Enum("stderr".into()))
        );
        let err = ConfigValueError::ExpectedVariant {
            variants: &["file", "stderr"],
        };
        assert_eq!(
            k.parse_value(&json!("syslog"), unused_pattern_valid),
            Err(err.clone())
        );
        assert_eq!(
            k.parse_value(&json!("File"), unused_pattern_valid),
            Err(err.clone())
        );
        assert_eq!(
            k.parse_value(&json!(1), unused_pattern_valid),
            Err(err.clone())
        );
        assert_eq!(err.to_string(), "expected one of: file, stderr");
    }

    #[test]
    fn str_key_parse_value() {
        let k = key_def(AUDIT_FILE_PATH).unwrap();
        assert_eq!(
            k.parse_value(&json!(""), unused_pattern_valid),
            Ok(ConfigValue::Str("".into()))
        );
        let abs = if cfg!(windows) {
            "C:\\logs\\audit.jsonl"
        } else {
            "/var/log/audit.jsonl"
        };
        assert!(k.parse_value(&json!(abs), unused_pattern_valid).is_ok());
        assert_eq!(
            k.parse_value(&json!("logs/audit.jsonl"), unused_pattern_valid),
            Err(ConfigValueError::ExpectedAbsolutePath)
        );
        assert_eq!(
            k.parse_value(&json!(42), unused_pattern_valid),
            Err(ConfigValueError::ExpectedString)
        );
    }

    #[test]
    fn str_list_key_parse_value() {
        let k = key_def(CONTENT_SECURITY_SACRED_DOMAINS).unwrap();
        assert_eq!(
            k.parse_value(&json!([]), test_domain_pattern_valid),
            Ok(ConfigValue::StrList(vec![]))
        );
        assert_eq!(
            k.parse_value(
                &json!(["example.com", "*.example.com"]),
                test_domain_pattern_valid
            ),
            Ok(ConfigValue::StrList(vec![
                "example.com".into(),
                "*.example.com".into()
            ]))
        );
        assert_eq!(
            k.parse_value(
                &json!(["example.com", "example.com"]),
                test_domain_pattern_valid
            ),
            Err(ConfigValueError::DuplicateEntry("example.com".into()))
        );
        assert_eq!(
            k.parse_value(&json!(["example.com", 3]), test_domain_pattern_valid),
            Err(ConfigValueError::ExpectedStringList)
        );
        assert_eq!(
            k.parse_value(&json!("example.com"), test_domain_pattern_valid),
            Err(ConfigValueError::ExpectedStringList)
        );
        assert_eq!(
            k.parse_value(&json!(["EXAMPLE.com"]), test_domain_pattern_valid),
            Err(ConfigValueError::InvalidDomainPattern("EXAMPLE.com".into()))
        );
        assert_eq!(
            k.parse_value(&json!(["evil*.com"]), test_domain_pattern_valid),
            Err(ConfigValueError::InvalidDomainPattern("evil*.com".into()))
        );
    }

    #[test]
    fn null_and_object_are_invalid_for_every_key() {
        for k in KEYS {
            assert!(
                k.parse_value(&json!(null), unused_pattern_valid).is_err(),
                "{}",
                k.key
            );
            assert!(
                k.parse_value(&json!({}), unused_pattern_valid).is_err(),
                "{}",
                k.key
            );
        }
    }
}
