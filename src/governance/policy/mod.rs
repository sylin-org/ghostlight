//! Governance configuration -- the central registry of policy keys.
//!
//! The full policy engine (manifest parsing, grants, per-call enforcement; SPEC sec 4-5) is
//! **staged**: engine correctness ships and stabilizes first, then the governance layer lands on a
//! proven engine, so the two layers are never debugged at the same time. The design stance this
//! module encodes:
//!
//! - The **engine is always truthful.** The extension and tools return raw page content; they make
//!   no access or redaction decisions (SPEC sec 9.5: the binary governs *structurally*, not by
//!   inspecting content semantically).
//! - Governed behavior is an **overlay** expressed as typed configuration keys. Each governed code
//!   path reads its setting from [`Config`] instead of hardcoding it.
//!
//! Today there is no manifest, so [`Config`] holds the built-in **"Minimal"** default preset
//! (safe-by-default). When the policy engine lands it will resolve these values from the manifest
//! and thread a [`Config`] through dispatch; the key names and semantics defined here are the
//! stable contract. This is the "module to store all keys."

/// A governance configuration key: a stable dotted name, a human description, and its value under
/// the built-in "Minimal" preset. Add an entry here (and a field on [`Config`]) for each new
/// governed behavior. Keeping every key in one static table makes the surface introspectable and
/// gives future config UIs / docs a single source of truth.
#[derive(Debug, Clone, Copy)]
pub struct KeyDef {
    /// Stable dotted identifier, e.g. `content.security.secrets.redact`.
    pub key: &'static str,
    /// What the key governs.
    pub description: &'static str,
    /// Value under the "Minimal" default preset.
    pub minimal_default: bool,
}

/// `content.security.secrets.redact` -- when true, values of fields the page itself marks secret
/// (input `type=password`/`hidden`, or a sensitive `autocomplete` token) are replaced with
/// `[value redacted]` in `read_page` output before it leaves the binary. The engine still returns
/// the raw value (marked); this key only governs whether the overlay redacts it. Default: `true`.
pub const CONTENT_SECURITY_SECRETS_REDACT: &str = "content.security.secrets.redact";

/// The static registry of every governance key: the single source of truth for names, descriptions,
/// and "Minimal"-preset defaults.
pub const KEYS: &[KeyDef] = &[KeyDef {
    key: CONTENT_SECURITY_SECRETS_REDACT,
    description: "Redact values of secret fields (password/OTP/payment) in read_page output.",
    minimal_default: true,
}];

/// The governance configuration currently in force, with values typed for direct use by governed
/// code paths. Built from the "Minimal" default preset today (no manifest engine yet).
#[derive(Debug, Clone, Copy)]
pub struct Config {
    secrets_redact: bool,
}

impl Config {
    /// The built-in **"Minimal"** preset: safe-by-default. The only preset until the policy engine
    /// and manifest-driven presets land.
    pub fn minimal() -> Self {
        Self {
            secrets_redact: true,
        }
    }

    /// Whether secret field values must be redacted from `read_page` output
    /// (`content.security.secrets.redact`).
    pub fn secrets_redact(&self) -> bool {
        self.secrets_redact
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

    /// The typed `Config::minimal()` must agree with the registry's declared Minimal defaults, so
    /// the introspectable table and the values actually in force can never silently diverge.
    #[test]
    fn minimal_config_matches_the_registry_defaults() {
        let redact_default = KEYS
            .iter()
            .find(|k| k.key == CONTENT_SECURITY_SECRETS_REDACT)
            .expect("the secrets.redact key must be registered")
            .minimal_default;
        assert_eq!(Config::minimal().secrets_redact(), redact_default);
    }

    #[test]
    fn every_key_name_is_dotted_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for k in KEYS {
            assert!(k.key.contains('.'), "{} should be a dotted key", k.key);
            assert!(seen.insert(k.key), "duplicate config key: {}", k.key);
            assert!(!k.description.is_empty(), "{} needs a description", k.key);
        }
    }
}
