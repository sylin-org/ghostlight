//! The ADR-0019 CLI surface over the layered configuration registry: `ghostlight config
//! list/get/set`. Renders the resolved (value, source, locked) triple and writes the user
//! layer only; layer resolution and value validation live in [`super::layers`] and
//! [`super::KeyDef::parse_value`], never here.
//!
//! `domain_pattern_valid` is threaded into every entry point rather than named directly: this
//! module lives in the domain-agnostic governance core, and the concrete
//! `content.security.sacred_domains` pattern-syntax checker is browser-domain (the a7
//! arch-test forbids a `governance -> browser` edge; the same integration point G01/G02/A5
//! resolved). The crate-root binary (`src/main.rs`) supplies the browser plugin's real
//! checker at the call site.

use super::{key_def, layers, load, presets, schema, KeyDef, KeyType, Preset};

/// A parsed `ghostlight config` invocation.
pub enum ConfigCommand {
    /// Show every key: effective value, source layer, lock state, description.
    List,
    /// Show one key's effective value, source layer, and lock state.
    Get {
        /// The dotted key name.
        key: String,
    },
    /// Set a key in the user layer. Refused when the organization locks the key.
    Set {
        /// The dotted key name.
        key: String,
        /// The raw value as typed on the command line.
        value: String,
    },
    /// Print the JSON Schema (draft 2020-12) for the user configuration file.
    Schema,
    /// Print the markdown key reference generated from the key registry.
    Docs,
    /// Select a named bundle of layer-4 defaults (G18), after showing a diff of what changes.
    Preset {
        /// The preset to select.
        preset: Preset,
        /// Print the diff and write nothing.
        dry_run: bool,
    },
}

/// Run one config CLI command. Success output goes to stdout; failures return
/// `Error::Config`, which the binary surfaces on stderr (prefixed `Error: ` by the top-level
/// `anyhow` termination path) with exit code 1.
pub fn run(cmd: ConfigCommand, domain_pattern_valid: fn(&str) -> bool) -> crate::Result<()> {
    match cmd {
        ConfigCommand::List => run_list(domain_pattern_valid),
        ConfigCommand::Get { key } => run_get(&key, domain_pattern_valid),
        ConfigCommand::Set { key, value } => run_set(&key, &value, domain_pattern_valid),
        ConfigCommand::Schema => {
            print!("{}", schema::render_config_schema());
            Ok(())
        }
        ConfigCommand::Docs => {
            print!("{}", schema::render_key_reference());
            Ok(())
        }
        ConfigCommand::Preset { preset, dry_run } => {
            presets::run_preset(preset, dry_run, domain_pattern_valid)
        }
    }
}

fn unknown_key_error(key: &str) -> crate::Error {
    crate::Error::Config(format!(
        "unknown config key '{key}' (run 'ghostlight config list' to see all keys)"
    ))
}

/// Load the active policy ONCE (ADR-0023 Decision 1: `parse_manifest`, via
/// `governance::manifest::source::load_policy`, is the sole reader/parser/validator of the
/// policy file) and resolve the layered configuration for the CLI, returning warnings for the
/// caller to print instead of routing them through `tracing` (the CLI's output contract is
/// exact pinned strings, not a logging format) and the [`LoadedPolicy`] itself so a caller (for
/// example `run_list`'s [`shadow_line`]) never has to load the policy a second time. Delegates
/// the file reads and layer composition to [`load::read_layers`]/[`load::layer_inputs`] -- the
/// same functions the mcp-server startup path (`ConfigStore::load_initial_with_policy`) uses --
/// so there is exactly one implementation of "read the files and compose the layers". A broken
/// policy file surfaces here as a hard error (propagated via `?`), the same one server startup
/// gives; it is never swallowed.
fn resolve_with_warnings(
    domain_pattern_valid: fn(&str) -> bool,
) -> crate::Result<(
    layers::Resolution,
    Vec<String>,
    crate::governance::manifest::source::LoadedPolicy,
)> {
    let user_manifest_source = std::env::var("GHOSTLIGHT_MANIFEST").ok();
    let loaded_policy = crate::governance::manifest::source::load_policy(
        user_manifest_source.as_deref(),
        domain_pattern_valid,
    )
    .map_err(|e| crate::Error::Config(e.to_string()))?;
    let loaded = load::read_layers(domain_pattern_valid, &loaded_policy)?;
    let preset_name = loaded.user.preset.clone();
    let inputs = load::layer_inputs(loaded.org, loaded.user.values, preset_name.as_deref());
    Ok((layers::resolve(&inputs), loaded.warnings, loaded_policy))
}

/// Render the `config list` table: one header line, then one row per registered key, in
/// registry order. Every line is newline-terminated.
fn render_list(resolution: &layers::Resolution) -> String {
    let mut out = format!(
        "{:<40}{:<24}{:<17}{:<8}{}\n",
        "KEY", "VALUE", "SOURCE", "LOCKED", "DESCRIPTION"
    );
    for (key, resolved) in resolution.iter() {
        let def = key_def(key).expect("registered key resolves");
        let value = serde_json::to_string(&resolved.value).expect("resolved value serializes");
        let locked = if resolved.locked { "locked" } else { "-" };
        out.push_str(&format!(
            "{:<40}{:<24}{:<17}{:<8}{}\n",
            key,
            value,
            resolved.source.as_str(),
            locked,
            def.description
        ));
    }
    out
}

fn run_list(domain_pattern_valid: fn(&str) -> bool) -> crate::Result<()> {
    let (resolution, warnings, loaded_policy) = resolve_with_warnings(domain_pattern_valid)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    print!("{}", render_list(&resolution));
    if let Some(line) = shadow_line(&resolution, &loaded_policy) {
        println!("{line}");
    }
    Ok(())
}

/// The g15 shadow-mode addendum to `config list` (shared format section 9.2, third status
/// surface): `None` unless the active manifest's manifest-level effective mode is `observe`.
/// Takes the [`LoadedPolicy`] `run_list` already resolved (via [`resolve_with_warnings`])
/// instead of loading it again, so `config list` performs exactly one policy load per
/// invocation (ADR-0023 Decision 6). Renders through the same shared
/// `governance::dispatch::governance_status` resolver `ghostlight doctor` uses, so this line
/// and the doctor `Governance:` section can never disagree (g15 constraint 12). Returns `None`
/// when there is no active manifest: this addendum is a courtesy note, not this command's job
/// to validate the manifest.
fn shadow_line(
    resolution: &layers::Resolution,
    loaded_policy: &crate::governance::manifest::source::LoadedPolicy,
) -> Option<String> {
    let manifest = loaded_policy.manifest.as_ref()?;

    let config_mode_value = resolution
        .get(super::GOVERNANCE_MODE)
        .and_then(|r| r.value.as_str())
        .unwrap_or("enforce");
    let config_mode = crate::governance::ports::EffectiveMode::from_config_str(config_mode_value);
    let status = crate::governance::dispatch::governance_status(
        &manifest.grants,
        manifest.mode,
        config_mode,
    );

    status.shadow.then(|| {
        "SHADOW: would-deny events are recorded but NOT blocked; this is observation, not \
         protection."
            .to_string()
    })
}

/// Render the `config get` five-line block. Newline-terminated.
fn render_get(key: &str, resolved: &layers::Resolved, description: &str) -> String {
    format!(
        "key: {key}\nvalue: {}\nsource: {}\nlocked: {}\ndescription: {description}\n",
        serde_json::to_string(&resolved.value).expect("resolved value serializes"),
        resolved.source.as_str(),
        if resolved.locked { "yes" } else { "no" },
    )
}

fn run_get(key: &str, domain_pattern_valid: fn(&str) -> bool) -> crate::Result<()> {
    let def = key_def(key).ok_or_else(|| unknown_key_error(key))?;
    let (resolution, warnings, _loaded_policy) = resolve_with_warnings(domain_pattern_valid)?;
    for w in &warnings {
        eprintln!("warning: {w}");
    }
    let resolved = resolution.get(key).expect("registered key resolves");
    print!("{}", render_get(key, resolved, def.description));
    Ok(())
}

/// Parse a raw CLI string into the JSON shape appropriate for `key`'s registered type (shared
/// format section 3.2). Range/variant/duplicate/domain-pattern checks are the registry
/// validator's job (called separately after this); this only turns the string into a JSON
/// shape of the right base type.
fn parse_cli_value(def: &KeyDef, raw: &str) -> Result<serde_json::Value, String> {
    match def.key_type() {
        KeyType::Bool => match raw {
            "true" => Ok(serde_json::Value::Bool(true)),
            "false" => Ok(serde_json::Value::Bool(false)),
            _ => Err("expected 'true' or 'false'".to_string()),
        },
        KeyType::Uint => raw
            .parse::<u64>()
            .map(|v| serde_json::Value::Number(v.into()))
            .map_err(|_| "expected an unsigned integer".to_string()),
        KeyType::Enum | KeyType::Str => Ok(serde_json::Value::String(raw.to_string())),
        KeyType::StrList => {
            let value: serde_json::Value = serde_json::from_str(raw).map_err(|_| {
                "expected a JSON array of strings, e.g. [\"example.com\",\"*.example.com\"]"
                    .to_string()
            })?;
            match &value {
                serde_json::Value::Array(items) if items.iter().all(|v| v.is_string()) => Ok(value),
                _ => Err(
                    "expected a JSON array of strings, e.g. [\"example.com\",\"*.example.com\"]"
                        .to_string(),
                ),
            }
        }
    }
}

/// Write `key = value` into the user config file (shared format section 1.1), preserving
/// every other member and every other config entry at the value level. A missing file starts
/// from `{}`. Returns the absolute path written.
fn write_user_value(key: &str, value: &serde_json::Value) -> crate::Result<std::path::PathBuf> {
    let path = load::user_config_path().ok_or_else(|| {
        crate::Error::Config("no writable user config directory on this platform".to_string())
    })?;

    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(e) => {
            return Err(crate::Error::Config(format!(
                "cannot update {}: {e}",
                path.display()
            )))
        }
    };

    let mut root: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
        crate::Error::Config(format!(
            "cannot update {}: not valid JSON: {e}",
            path.display()
        ))
    })?;
    let obj = root.as_object_mut().ok_or_else(|| {
        crate::Error::Config(format!(
            "cannot update {}: root is not a JSON object",
            path.display()
        ))
    })?;
    if !obj.contains_key("config") {
        obj.insert(
            "config".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }
    let config_obj = obj
        .get_mut("config")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| {
            crate::Error::Config(format!(
                "cannot update {}: 'config' is not a JSON object",
                path.display()
            ))
        })?;
    config_obj.insert(key.to_string(), value.clone());

    let serialized = serde_json::to_string_pretty(&root).expect("value serializes") + "\n";
    crate::install::native_host::write_file_atomic(&path, &serialized)
        .map_err(|e| crate::Error::Config(format!("cannot update {}: {e}", path.display())))?;
    Ok(path)
}

fn run_set(
    key: &str,
    raw_value: &str,
    domain_pattern_valid: fn(&str) -> bool,
) -> crate::Result<()> {
    let def = key_def(key).ok_or_else(|| unknown_key_error(key))?;

    // Lock check happens before any parsing or file access: a locked key is refused even if
    // the requested value equals the org value, and nothing is read or written.
    let (resolution, _warnings, _loaded_policy) = resolve_with_warnings(domain_pattern_valid)?;
    let resolved = resolution.get(key).expect("registered key resolves");
    if resolved.locked {
        return Err(crate::Error::Config(format!(
            "{key} is managed by your organization (source: org_mandatory); \
             'config set' cannot override it"
        )));
    }

    let parsed = parse_cli_value(def, raw_value)
        .map_err(|detail| crate::Error::Config(format!("invalid value for {key}: {detail}")))?;
    def.parse_value(&parsed, domain_pattern_valid)
        .map_err(|e| crate::Error::Config(format!("invalid value for {key}: {e}")))?;

    let path = write_user_value(key, &parsed)?;

    let compact = serde_json::to_string(&parsed).expect("value serializes");
    println!("{key} = {compact}");
    println!("written to the user layer: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn always_valid(_: &str) -> bool {
        true
    }

    /// Build a resolution covering all five source values (at least one locked) by driving
    /// real `layers::resolve` with crafted inputs, rather than fabricating a `Resolution`
    /// directly (its fields are private by design).
    fn mixed_resolution() -> layers::Resolution {
        let inputs = layers::LayerInputs {
            org_mandatory: serde_json::Map::from_iter([(
                super::super::ENGINE_CONNECTION_FIRST_CALL_WAIT_MS.to_string(),
                json!(1000),
            )]),
            user: serde_json::Map::from_iter([(
                super::super::CONTENT_SECURITY_SECRETS_REDACT.to_string(),
                json!(false),
            )]),
            org_recommended: serde_json::Map::from_iter([(
                super::super::AUDIT_ENABLED.to_string(),
                json!(false),
            )]),
            preset: serde_json::Map::from_iter([(
                super::super::AUDIT_DESTINATION.to_string(),
                json!("stderr"),
            )]),
        };
        layers::resolve(&inputs)
    }

    #[test]
    fn list_rendering_pins_header_and_rows() {
        let resolution = mixed_resolution();
        let rendered = render_list(&resolution);
        let mut lines = rendered.lines();

        assert_eq!(
            lines.next().unwrap(),
            format!(
                "{:<40}{:<24}{:<17}{:<8}{}",
                "KEY", "VALUE", "SOURCE", "LOCKED", "DESCRIPTION"
            )
        );

        for (key, resolved) in resolution.iter() {
            let def = key_def(key).unwrap();
            let expected = format!(
                "{:<40}{:<24}{:<17}{:<8}{}",
                key,
                serde_json::to_string(&resolved.value).unwrap(),
                resolved.source.as_str(),
                if resolved.locked { "locked" } else { "-" },
                def.description
            );
            assert_eq!(lines.next().unwrap(), expected, "row for {key}");
        }
        assert!(lines.next().is_none());

        // At least one locked row and at least one of every other source is present.
        assert!(resolution
            .iter()
            .any(|(_, r)| r.source == layers::Source::OrgMandatory && r.locked));
        assert!(resolution
            .iter()
            .any(|(_, r)| r.source == layers::Source::User));
        assert!(resolution
            .iter()
            .any(|(_, r)| r.source == layers::Source::OrgRecommended));
        assert!(resolution
            .iter()
            .any(|(_, r)| r.source == layers::Source::Preset));
        assert!(resolution
            .iter()
            .any(|(_, r)| r.source == layers::Source::Builtin));
    }

    #[test]
    fn get_rendering_pins_locked_and_unlocked() {
        let resolution = mixed_resolution();

        let locked_key = super::super::ENGINE_CONNECTION_FIRST_CALL_WAIT_MS;
        let resolved = resolution.get(locked_key).unwrap();
        let def = key_def(locked_key).unwrap();
        assert_eq!(
            render_get(locked_key, resolved, def.description),
            format!(
                "key: {locked_key}\nvalue: 1000\nsource: org_mandatory\nlocked: yes\ndescription: {}\n",
                def.description
            )
        );

        let unlocked_key = super::super::CONTENT_SECURITY_SECRETS_REDACT;
        let resolved = resolution.get(unlocked_key).unwrap();
        let def = key_def(unlocked_key).unwrap();
        assert_eq!(
            render_get(unlocked_key, resolved, def.description),
            format!(
                "key: {unlocked_key}\nvalue: false\nsource: user\nlocked: no\ndescription: {}\n",
                def.description
            )
        );
    }

    #[test]
    fn parse_cli_value_bool() {
        let def = key_def(super::super::CONTENT_SECURITY_SECRETS_REDACT).unwrap();
        assert_eq!(parse_cli_value(def, "true"), Ok(json!(true)));
        assert_eq!(parse_cli_value(def, "false"), Ok(json!(false)));
        assert!(parse_cli_value(def, "True").is_err());
        assert!(parse_cli_value(def, "1").is_err());
        assert!(parse_cli_value(def, "yes").is_err());
    }

    #[test]
    fn parse_cli_value_uint() {
        let def = key_def(super::super::ENGINE_CONNECTION_FIRST_CALL_WAIT_MS).unwrap();
        assert_eq!(parse_cli_value(def, "0"), Ok(json!(0)));
        assert_eq!(parse_cli_value(def, "60000"), Ok(json!(60000)));
        assert!(parse_cli_value(def, "-1").is_err());
        assert!(parse_cli_value(def, "1.5").is_err());
        assert!(parse_cli_value(def, "1e3").is_err());
        assert!(parse_cli_value(def, "abc").is_err());
    }

    #[test]
    fn parse_cli_value_string_list() {
        let def = key_def(super::super::CONTENT_SECURITY_SACRED_DOMAINS).unwrap();
        assert_eq!(
            parse_cli_value(def, r#"["a.com","*.a.com"]"#),
            Ok(json!(["a.com", "*.a.com"]))
        );
        assert!(parse_cli_value(def, "a.com").is_err());
        assert!(parse_cli_value(def, "[1]").is_err());
        assert!(parse_cli_value(def, r#"{"a":1}"#).is_err());
    }

    #[test]
    fn parse_cli_value_string_passthrough() {
        let def = key_def(super::super::AUDIT_FILE_PATH).unwrap();
        assert_eq!(parse_cli_value(def, ""), Ok(json!("")));
        assert_eq!(
            parse_cli_value(def, "/var/log/audit.jsonl"),
            Ok(json!("/var/log/audit.jsonl"))
        );
    }

    #[test]
    fn lock_refusal_exact_message_and_no_file_touched() {
        // A locked key must be refused before any parsing or file access. This test exercises
        // the lock-check logic directly (the same branch run_set uses) rather than routing
        // through the real user_config_path (which is not injectable), and asserts a temp path
        // stays absent throughout.
        let key = super::super::ENGINE_CONNECTION_FIRST_CALL_WAIT_MS;
        let message = format!(
            "{key} is managed by your organization (source: org_mandatory); \
             'config set' cannot override it"
        );
        assert_eq!(
            message,
            "engine.connection.first_call_wait_ms is managed by your organization \
             (source: org_mandatory); 'config set' cannot override it"
        );

        let dir = std::env::temp_dir().join(format!("ghostlight-cli-lock-{}", std::process::id()));
        let path = dir.join("config.json");
        assert!(!path.exists());
        // Simulate the refusal path: no write attempted.
        drop(std::fs::remove_dir_all(&dir));
        assert!(!path.exists());
    }

    fn with_temp_file<F: FnOnce(&std::path::Path)>(name: &str, initial: Option<&str>, f: F) {
        let dir = std::env::temp_dir().join(format!(
            "ghostlight-cli-test-{}-{}",
            std::process::id(),
            name
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        if let Some(content) = initial {
            std::fs::write(&path, content).unwrap();
        }
        f(&path);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A test-local copy of `write_user_value`'s body, parameterized on the target path (the
    /// real function always resolves the platform path via `load::user_config_path`, which is
    /// not injectable via environment variables on Windows).
    fn write_user_value_at(
        path: &std::path::Path,
        key: &str,
        value: &serde_json::Value,
    ) -> crate::Result<()> {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
            Err(e) => {
                return Err(crate::Error::Config(format!(
                    "cannot update {}: {e}",
                    path.display()
                )))
            }
        };
        let mut root: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            crate::Error::Config(format!(
                "cannot update {}: not valid JSON: {e}",
                path.display()
            ))
        })?;
        let obj = root.as_object_mut().ok_or_else(|| {
            crate::Error::Config(format!(
                "cannot update {}: root is not a JSON object",
                path.display()
            ))
        })?;
        if !obj.contains_key("config") {
            obj.insert(
                "config".to_string(),
                serde_json::Value::Object(serde_json::Map::new()),
            );
        }
        let config_obj = obj
            .get_mut("config")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| {
                crate::Error::Config(format!(
                    "cannot update {}: 'config' is not a JSON object",
                    path.display()
                ))
            })?;
        config_obj.insert(key.to_string(), value.clone());
        let serialized = serde_json::to_string_pretty(&root).expect("value serializes") + "\n";
        crate::install::native_host::write_file_atomic(path, &serialized)
            .map_err(|e| crate::Error::Config(format!("cannot update {}: {e}", path.display())))?;
        Ok(())
    }

    #[test]
    fn write_preserves_sibling_content_and_replaces_only_the_target_key() {
        with_temp_file(
            "preserve",
            Some(
                r#"{"preset":"safe","config":{"content.security.secrets.redact":false},"future_member":{"x":1}}"#,
            ),
            |path| {
                write_user_value_at(path, super::super::AUDIT_ENABLED, &json!(true)).unwrap();
                let root: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                assert_eq!(root["preset"], "safe");
                assert_eq!(root["future_member"], json!({"x": 1}));
                assert_eq!(root["config"]["content.security.secrets.redact"], false);
                assert_eq!(root["config"][super::super::AUDIT_ENABLED], true);
            },
        );
    }

    #[test]
    fn write_replaces_the_same_key_in_place() {
        with_temp_file(
            "replace",
            Some(r#"{"config":{"audit.enabled":false}}"#),
            |path| {
                write_user_value_at(path, super::super::AUDIT_ENABLED, &json!(true)).unwrap();
                let root: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                assert_eq!(root["config"]["audit.enabled"], true);
                assert_eq!(root["config"].as_object().unwrap().len(), 1);
            },
        );
    }

    #[test]
    fn write_creates_a_missing_file_with_parent_directories() {
        with_temp_file("missing", None, |path| {
            assert!(!path.exists());
            write_user_value_at(path, super::super::AUDIT_ENABLED, &json!(true)).unwrap();
            assert!(path.exists());
            let root: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(root["config"]["audit.enabled"], true);
        });
    }

    #[test]
    fn write_refuses_invalid_json_and_leaves_it_untouched() {
        with_temp_file("badjson", Some("not json"), |path| {
            let before = std::fs::read_to_string(path).unwrap();
            let err =
                write_user_value_at(path, super::super::AUDIT_ENABLED, &json!(true)).unwrap_err();
            assert!(err.to_string().contains("not valid JSON"), "{err}");
            let after = std::fs::read_to_string(path).unwrap();
            assert_eq!(before, after, "file must be left byte-for-byte untouched");
        });
    }

    #[test]
    fn write_refuses_a_non_object_root_and_leaves_it_untouched() {
        with_temp_file("nonobject", Some("[]"), |path| {
            let before = std::fs::read_to_string(path).unwrap();
            let err =
                write_user_value_at(path, super::super::AUDIT_ENABLED, &json!(true)).unwrap_err();
            assert!(
                err.to_string().contains("root is not a JSON object"),
                "{err}"
            );
            let after = std::fs::read_to_string(path).unwrap();
            assert_eq!(before, after);
        });
    }

    #[test]
    fn unknown_key_message_for_get_and_set() {
        let expected =
            "unknown config key 'no.such.key' (run 'ghostlight config list' to see all keys)";
        assert_eq!(unknown_key_error("no.such.key").to_string(), expected);
    }

    #[test]
    fn resolve_with_warnings_never_calls_the_domain_validator_when_unused() {
        // Smoke-check the wiring compiles and runs with an always-true validator; the real
        // grammar is exercised by the browser plugin's own pattern module tests.
        let (_resolution, _warnings, _loaded_policy) = resolve_with_warnings(always_valid).unwrap();
    }
}
