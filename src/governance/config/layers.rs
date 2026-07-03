//! The ADR-0019 layer model (shared format doc section 2): resolve a single typed value per
//! key from up to five precedence layers. Precedence, highest to lowest: org-mandatory, user,
//! org-recommended, preset default, built-in Minimal. Layer 5 (the registry defaults) always
//! defines every key, so resolution never fails. Pure: no filesystem, no environment, no
//! tracing; designed as a pure function of [`LayerInputs`] so a later re-resolve (hot-reload,
//! task A5) can re-run it on a fresh snapshot.

use super::{ConfigValue, KeyDef, Preset, KEYS};

/// Which layer a resolved value came from (shared format section 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    OrgMandatory,
    User,
    OrgRecommended,
    Preset,
    Builtin,
}

impl Source {
    /// The shared-format 2.1 wire name: exactly "org_mandatory", "user", "org_recommended",
    /// "preset", or "builtin". Consumed by `config list`, the extension options page, and
    /// audit tooling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::OrgMandatory => "org_mandatory",
            Source::User => "user",
            Source::OrgRecommended => "org_recommended",
            Source::Preset => "preset",
            Source::Builtin => "builtin",
        }
    }
}

/// The resolved triple for one key (shared format section 2.1).
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The effective value, already validated against the key's type and constraints.
    pub value: serde_json::Value,
    /// The layer that defined it.
    pub source: Source,
    /// True if and only if `source` is `Source::OrgMandatory`.
    pub locked: bool,
}

/// Per-layer candidate values keyed by dotted key name. Entries are validated by the loaders
/// before they get here; [`resolve`] only picks, it does not re-validate.
#[derive(Debug, Clone, Default)]
pub struct LayerInputs {
    pub org_mandatory: serde_json::Map<String, serde_json::Value>,
    pub user: serde_json::Map<String, serde_json::Value>,
    pub org_recommended: serde_json::Map<String, serde_json::Value>,
    /// Layer 4. Composed by [`super::load::layer_inputs`] (G18) from a declared preset name's
    /// per-key defaults ([`super::preset_layer`]); empty when no preset is declared.
    pub preset: serde_json::Map<String, serde_json::Value>,
}

/// The full resolution: one triple per registered key, in [`KEYS`] registry order.
#[derive(Debug, Clone)]
pub struct Resolution {
    entries: Vec<(&'static str, Resolved)>,
}

impl Resolution {
    /// The resolved triple for `key`, or `None` if `key` is not registered.
    pub fn get(&self, key: &str) -> Option<&Resolved> {
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, r)| r)
    }

    /// Every resolved key, in registry order (the order `config list` renders).
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Resolved)> {
        self.entries.iter().map(|(k, r)| (*k, r))
    }
}

/// Resolve every registered key against the five layers. Infallible: the built-in layer (the
/// registry defaults, equal to the Safe preset) always defines every key.
pub fn resolve(layers: &LayerInputs) -> Resolution {
    let entries = KEYS
        .iter()
        .map(|def| (def.key, resolve_one(def, layers)))
        .collect();
    Resolution { entries }
}

fn resolve_one(def: &KeyDef, layers: &LayerInputs) -> Resolved {
    if let Some(v) = layers.org_mandatory.get(def.key) {
        return Resolved {
            value: v.clone(),
            source: Source::OrgMandatory,
            locked: true,
        };
    }
    if let Some(v) = layers.user.get(def.key) {
        return Resolved {
            value: v.clone(),
            source: Source::User,
            locked: false,
        };
    }
    if let Some(v) = layers.org_recommended.get(def.key) {
        return Resolved {
            value: v.clone(),
            source: Source::OrgRecommended,
            locked: false,
        };
    }
    if let Some(v) = layers.preset.get(def.key) {
        return Resolved {
            value: v.clone(),
            source: Source::Preset,
            locked: false,
        };
    }
    // Layer 5: the built-in Minimal default (equals the Safe preset, ADR-0019).
    let default: ConfigValue = def.default_for(Preset::Safe).into();
    Resolved {
        value: default.to_json(),
        source: Source::Builtin,
        locked: false,
    }
}

/// Validate a candidate value against a key's declared type and constraints (shared format
/// section 3.2), delegating to [`KeyDef::parse_value`]. `domain_pattern_valid` is forwarded
/// unchanged; it is consulted only for the `content.security.sacred_domains` key's
/// `DomainPatternList` constraint (the concrete grammar lives in the browser plugin, kept out
/// of this core module by the a7 arch-test).
pub fn validate_value(
    def: &KeyDef,
    value: &serde_json::Value,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<(), String> {
    def.parse_value(value, domain_pattern_valid)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn always_valid(_: &str) -> bool {
        true
    }

    #[test]
    fn builtin_layer_defines_every_key_when_inputs_are_empty() {
        let resolution = resolve(&LayerInputs::default());
        for def in KEYS {
            let resolved = resolution.get(def.key).expect("every key resolves");
            assert_eq!(resolved.source, Source::Builtin, "{}", def.key);
            assert!(!resolved.locked, "{}", def.key);
            let expected: ConfigValue = def.default_for(Preset::Safe).into();
            assert_eq!(resolved.value, expected.to_json(), "{}", def.key);
        }
    }

    #[test]
    fn precedence_walks_org_mandatory_user_org_recommended_preset_builtin() {
        let key = super::super::CONTENT_SECURITY_SECRETS_REDACT;
        let mut inputs = LayerInputs {
            org_mandatory: serde_json::Map::from_iter([(key.to_string(), json!("org_mandatory"))]),
            user: serde_json::Map::from_iter([(key.to_string(), json!("user"))]),
            org_recommended: serde_json::Map::from_iter([(
                key.to_string(),
                json!("org_recommended"),
            )]),
            preset: serde_json::Map::from_iter([(key.to_string(), json!("preset"))]),
        };

        let r = resolve(&inputs);
        assert_eq!(r.get(key).unwrap().source, Source::OrgMandatory);

        inputs.org_mandatory.clear();
        let r = resolve(&inputs);
        assert_eq!(r.get(key).unwrap().source, Source::User);

        inputs.user.clear();
        let r = resolve(&inputs);
        assert_eq!(r.get(key).unwrap().source, Source::OrgRecommended);

        inputs.org_recommended.clear();
        let r = resolve(&inputs);
        assert_eq!(r.get(key).unwrap().source, Source::Preset);

        inputs.preset.clear();
        let r = resolve(&inputs);
        assert_eq!(r.get(key).unwrap().source, Source::Builtin);
    }

    #[test]
    fn locked_is_true_iff_source_is_org_mandatory() {
        let key = super::super::AUDIT_ENABLED;
        for (inputs, expect_locked) in [
            (
                LayerInputs {
                    org_mandatory: serde_json::Map::from_iter([(key.to_string(), json!(true))]),
                    ..Default::default()
                },
                true,
            ),
            (
                LayerInputs {
                    user: serde_json::Map::from_iter([(key.to_string(), json!(true))]),
                    ..Default::default()
                },
                false,
            ),
            (
                LayerInputs {
                    org_recommended: serde_json::Map::from_iter([(key.to_string(), json!(true))]),
                    ..Default::default()
                },
                false,
            ),
            (
                LayerInputs {
                    preset: serde_json::Map::from_iter([(key.to_string(), json!(true))]),
                    ..Default::default()
                },
                false,
            ),
            (LayerInputs::default(), false),
        ] {
            let r = resolve(&inputs);
            assert_eq!(r.get(key).unwrap().locked, expect_locked);
        }
    }

    #[test]
    fn source_strings_match_the_shared_format_enum() {
        assert_eq!(Source::OrgMandatory.as_str(), "org_mandatory");
        assert_eq!(Source::User.as_str(), "user");
        assert_eq!(Source::OrgRecommended.as_str(), "org_recommended");
        assert_eq!(Source::Preset.as_str(), "preset");
        assert_eq!(Source::Builtin.as_str(), "builtin");
    }

    #[test]
    fn resolution_iterates_in_registry_order() {
        let resolution = resolve(&LayerInputs::default());
        let keys: Vec<&str> = resolution.iter().map(|(k, _)| k).collect();
        let expected: Vec<&str> = KEYS.iter().map(|d| d.key).collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn validate_value_enforces_section_3_2() {
        let bool_def =
            super::super::key_def(super::super::CONTENT_SECURITY_SECRETS_REDACT).unwrap();
        assert!(validate_value(bool_def, &json!(true), always_valid).is_ok());
        assert!(validate_value(bool_def, &json!("yes"), always_valid).is_err());

        let uint_def =
            super::super::key_def(super::super::ENGINE_CONNECTION_FIRST_CALL_WAIT_MS).unwrap();
        assert!(validate_value(uint_def, &json!(5000), always_valid).is_ok());
        assert!(validate_value(uint_def, &json!(-1), always_valid).is_err());
        assert!(validate_value(uint_def, &json!(5.5), always_valid).is_err());
        let exponent: serde_json::Value = serde_json::from_str("1e3").unwrap();
        assert!(validate_value(uint_def, &exponent, always_valid).is_err());
        assert!(validate_value(uint_def, &json!(999999), always_valid).is_err());

        let enum_def = super::super::key_def(super::super::AUDIT_DESTINATION).unwrap();
        assert!(validate_value(enum_def, &json!("file"), always_valid).is_ok());
        assert!(validate_value(enum_def, &json!("File"), always_valid).is_err());
        assert!(validate_value(enum_def, &json!("carrier-pigeon"), always_valid).is_err());

        let str_def = super::super::key_def(super::super::AUDIT_FILE_PATH).unwrap();
        assert!(validate_value(str_def, &json!(""), always_valid).is_ok());
        assert!(validate_value(str_def, &json!(42), always_valid).is_err());

        let list_def =
            super::super::key_def(super::super::CONTENT_SECURITY_SACRED_DOMAINS).unwrap();
        assert!(validate_value(list_def, &json!(["a.com", 3]), always_valid).is_err());
        assert!(
            validate_value(list_def, &json!(["a.com", "a.com"]), always_valid).is_err(),
            "duplicate elements are rejected"
        );

        for def in [bool_def, uint_def, enum_def, str_def, list_def] {
            assert!(
                validate_value(def, &json!(null), always_valid).is_err(),
                "{}",
                def.key
            );
            assert!(
                validate_value(def, &json!({}), always_valid).is_err(),
                "{}",
                def.key
            );
        }
    }
}
