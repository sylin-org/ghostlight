//! Generates the user-config-file JSON Schema and the markdown key reference from the typed
//! key registry (ADR-0020 commitment 1). The registry ([`super::KEYS`]) is the single source
//! of truth; both outputs are pinned by golden tests (`tests/config_schema_golden.rs`) so any
//! registry drift (a new key, an edited description, a changed constraint) fails the build
//! until the goldens are regenerated deliberately. [`key_value_schema`] is public so the
//! manifest schema generator (task G12) can reuse the same per-key type mapping for manifest
//! `config` entries without duplicating it.
//!
//! This module is pure: no I/O, no CLI parsing, no file paths.

use serde_json::json;

use super::{KeyConstraint, KeyDef, KeyType, KeyValue, Preset, KEYS};

/// JSON Schema (draft 2020-12) fragment validating one key's VALUE, derived from the key's
/// registered type, constraint, description, and built-in Minimal default. Member insertion
/// order: `description`, `type`, constraint fields, `default`.
pub fn key_value_schema(def: &KeyDef) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "description".to_string(),
        json!(def.description.to_string()),
    );
    match def.key_type() {
        KeyType::Bool => {
            map.insert("type".to_string(), json!("boolean"));
        }
        KeyType::Uint => {
            map.insert("type".to_string(), json!("integer"));
            if let KeyConstraint::UintRange { min, max } = def.constraint {
                map.insert("minimum".to_string(), json!(min));
                map.insert("maximum".to_string(), json!(max));
            }
        }
        KeyType::Enum => {
            map.insert("type".to_string(), json!("string"));
            if let KeyConstraint::EnumVariants(variants) = def.constraint {
                map.insert("enum".to_string(), json!(variants));
            }
        }
        KeyType::Str => {
            map.insert("type".to_string(), json!("string"));
        }
        KeyType::StrList => {
            map.insert("type".to_string(), json!("array"));
            map.insert("items".to_string(), json!({"type": "string"}));
            map.insert("uniqueItems".to_string(), json!(true));
        }
    }
    let default: super::ConfigValue = def.default_for(Preset::Safe).into();
    map.insert("default".to_string(), default.to_json());
    serde_json::Value::Object(map)
}

/// The complete JSON Schema (draft 2020-12) document for the user configuration file (shared
/// format doc section 1.1).
pub fn config_file_schema() -> serde_json::Value {
    let mut config_properties = serde_json::Map::new();
    for def in KEYS {
        config_properties.insert(def.key.to_string(), key_value_schema(def));
    }

    let mut config_field = serde_json::Map::new();
    config_field.insert(
        "description".to_string(),
        json!("Flat map of dotted key name to value. Each entry sets the user layer for that key."),
    );
    config_field.insert("type".to_string(), json!("object"));
    config_field.insert("additionalProperties".to_string(), json!(false));
    config_field.insert(
        "properties".to_string(),
        serde_json::Value::Object(config_properties),
    );

    let mut preset_field = serde_json::Map::new();
    preset_field.insert(
        "description".to_string(),
        json!(
            "Preset supplying the preset-default layer. When absent, the built-in Minimal \
             defaults (equal to safe) apply."
        ),
    );
    preset_field.insert("type".to_string(), json!("string"));
    preset_field.insert(
        "enum".to_string(),
        json!(["fully_open", "safe", "restricted"]),
    );

    let mut properties = serde_json::Map::new();
    properties.insert(
        "preset".to_string(),
        serde_json::Value::Object(preset_field),
    );
    properties.insert(
        "config".to_string(),
        serde_json::Value::Object(config_field),
    );

    let mut root = serde_json::Map::new();
    root.insert(
        "$schema".to_string(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert(
        "title".to_string(),
        json!("ghostlight user configuration file"),
    );
    root.insert(
        "description".to_string(),
        json!(
            "User-level configuration for ghostlight. Both fields are optional; an absent \
             file means no user layer."
        ),
    );
    root.insert("type".to_string(), json!("object"));
    root.insert("additionalProperties".to_string(), json!(false));
    root.insert(
        "properties".to_string(),
        serde_json::Value::Object(properties),
    );

    serde_json::Value::Object(root)
}

/// `config_file_schema()` pretty-printed (serde_json 2-space style) plus exactly one trailing
/// LF. This exact string is what `ghostlight config schema` prints and what the golden test
/// pins.
pub fn render_config_schema() -> String {
    let mut s = serde_json::to_string_pretty(&config_file_schema()).expect("schema serializes");
    s.push('\n');
    s
}

/// The type word used in the markdown key reference, in the shared format doc section 3.2
/// vocabulary (note "string list" has a space here, unlike the JSON Schema/wire type name
/// "string_list").
fn type_word(t: KeyType) -> &'static str {
    match t {
        KeyType::Bool => "bool",
        KeyType::Uint => "uint",
        KeyType::Enum => "enum",
        KeyType::Str => "string",
        KeyType::StrList => "string list",
    }
}

/// The human-readable constraint phrase for one key, keyed on its base type first so a
/// mismatched constraint (unreachable for a well-formed registry) still renders a sane
/// fallback rather than panicking a doc generator.
fn constraints_phrase(def: &KeyDef) -> String {
    match def.key_type() {
        KeyType::Bool => "none".to_string(),
        KeyType::Uint => match def.constraint {
            KeyConstraint::UintRange { min, max } => format!("integer between {min} and {max}"),
            _ => "none".to_string(),
        },
        KeyType::Enum => match def.constraint {
            KeyConstraint::EnumVariants(variants) => format!("one of: {}", variants.join(", ")),
            _ => "none".to_string(),
        },
        // EmptyOrAbsolutePath is a real constraint on Str keys (e.g. audit.file.path) that
        // postdates this task's own phrase table; rendering "none" for it would be untruthful
        // documentation, so it gets its own phrase rather than falling through silently.
        KeyType::Str => match def.constraint {
            KeyConstraint::EmptyOrAbsolutePath => "empty string, or an absolute path".to_string(),
            _ => "none".to_string(),
        },
        KeyType::StrList => match def.constraint {
            KeyConstraint::DomainPatternList => {
                "unique string elements; each a valid domain pattern".to_string()
            }
            _ => "unique string elements".to_string(),
        },
    }
}

/// Render one preset default as compact JSON (booleans bare, numbers bare, strings quoted,
/// lists as `[]` / `["a","b"]`).
fn compact_json(value: KeyValue) -> String {
    let cv: super::ConfigValue = value.into();
    serde_json::to_string(&cv.to_json()).expect("value serializes")
}

/// The markdown key reference generated from the registry, LF line endings, exactly one
/// trailing LF. This exact string is what `ghostlight config docs` prints and what the golden
/// test pins.
pub fn render_key_reference() -> String {
    let header = "# Configuration key reference\n\n\
Generated from the typed key registry in src/policy/mod.rs by `ghostlight config docs`.\n\
Do not edit by hand; change the registry and regenerate.\n\n\
Layer resolution: org-mandatory > user > org-recommended > preset default > built-in\n\
Minimal. The built-in Minimal defaults equal the `safe` preset.";

    let mut blocks = vec![header.to_string()];
    for def in KEYS {
        let block = format!(
            "## `{}`\n\n{}\n\n- Type: {}\n- Constraints: {}\n- Default (fully_open): {}\n\
             - Default (safe, = built-in Minimal): {}\n- Default (restricted): {}",
            def.key,
            def.description,
            type_word(def.key_type()),
            constraints_phrase(def),
            compact_json(def.default_for(Preset::FullyOpen)),
            compact_json(def.default_for(Preset::Safe)),
            compact_json(def.default_for(Preset::Restricted)),
        );
        blocks.push(block);
    }
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_value_schema_has_description_type_and_default() {
        for def in KEYS {
            let schema = key_value_schema(def);
            let obj = schema.as_object().unwrap();
            let description = obj["description"].as_str().unwrap();
            assert!(!description.is_empty(), "{}", def.key);
            let ty = obj["type"].as_str().unwrap();
            assert!(
                ["boolean", "integer", "string", "array"].contains(&ty),
                "{}: unexpected type {ty}",
                def.key
            );
            assert!(obj.contains_key("default"), "{}", def.key);
        }
    }

    #[test]
    fn uint_keys_carry_bounds_and_enum_keys_carry_variants() {
        for def in KEYS {
            let schema = key_value_schema(def);
            let obj = schema.as_object().unwrap();
            if obj["type"] == "integer" {
                assert!(obj["minimum"].is_number(), "{}", def.key);
                assert!(obj["maximum"].is_number(), "{}", def.key);
            }
            if let Some(variants) = obj.get("enum") {
                let arr = variants.as_array().unwrap();
                assert!(!arr.is_empty(), "{}", def.key);
                assert!(arr.iter().all(|v| v.is_string()), "{}", def.key);
            }
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(render_config_schema(), render_config_schema());
        assert_eq!(render_key_reference(), render_key_reference());
    }
}
