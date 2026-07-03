//! `browser-mcp config preset <name>` (G18, ADR-0019 decision 3: "presets are UX, not
//! machinery"). Selecting a preset writes ONLY the `preset` field of the user config file --
//! never a per-key value -- so it populates layer 4 (shared format doc section 2) and nothing
//! else: the user's own explicit edits (layer 2) and any org policy (layer 1/3) always keep
//! their precedence over it, unchanged. Before writing, the command shows a plain-language diff
//! of the CURRENT effective state against the CANDIDATE state under the new preset, computed by
//! resolving the same on-disk org/user layers twice through [`super::load::layer_inputs`] --
//! once with the current preset name, once with the new one -- so the diff can never disagree
//! with what `config list`/`config get` would show after the write.

use super::layers::{self, Source};
use super::{load, Preset};

/// One row of the `config preset` diff (Required behavior section 1c).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffRow {
    /// The effective value changes and the key is neither locked nor user-owned.
    Changed {
        key: &'static str,
        before: String,
        after: String,
    },
    /// The key is org-locked; the new preset's default would differ but never applies.
    Locked {
        key: &'static str,
        effective: String,
    },
    /// The key has an explicit user-layer value; the new preset's default would differ but
    /// never applies.
    Kept {
        key: &'static str,
        effective: String,
    },
}

/// Render a resolved JSON value the way the diff table does: compact JSON (`true`, `"file"`,
/// `[]`, `5000`).
fn render_value(v: &serde_json::Value) -> String {
    serde_json::to_string(v).expect("resolved value serializes")
}

/// Compute the diff rows between the CURRENT effective state and the CANDIDATE state under
/// `new_preset`, in registry order (Required behavior section 1c). Pure: takes both
/// already-resolved states, so it is testable without touching the filesystem.
///
/// Because only layer 4 differs between `current` and `candidate` (both are resolved from the
/// same org/user layers), a key resolved from org-mandatory, user, or org-recommended has the
/// identical value and source in both -- it can only ever produce a `Locked`/`Kept` notice, never
/// a `Changed` row. A key resolved from preset or builtin is exactly where a real value change
/// can happen.
pub fn diff_rows(
    current: &layers::Resolution,
    candidate: &layers::Resolution,
    new_preset: Preset,
) -> Vec<DiffRow> {
    let new_defaults = super::preset_layer(new_preset);
    let mut rows = Vec::new();
    for def in super::KEYS {
        let key = def.key;
        let cur = current.get(key).expect("registered key resolves");
        let cand = candidate.get(key).expect("registered key resolves");
        let new_default = new_defaults
            .get(key)
            .expect("preset_layer covers every key");

        if cur.value != cand.value {
            rows.push(DiffRow::Changed {
                key,
                before: render_value(&cur.value),
                after: render_value(&cand.value),
            });
        } else if cur.locked && new_default != &cur.value {
            rows.push(DiffRow::Locked {
                key,
                effective: render_value(&cur.value),
            });
        } else if cur.source == Source::User && new_default != &cur.value {
            rows.push(DiffRow::Kept {
                key,
                effective: render_value(&cur.value),
            });
        }
    }
    rows
}

/// Render the fallback before/after table (Required behavior section 1c). This is the only
/// renderer today: the G16 plain-language renderer, if it lands, would replace it for this
/// call site.
// G16 integration point: replace this table with the plain-language diff renderer when it lands.
pub fn render_diff(
    current_preset_name: Option<&str>,
    new_preset: Preset,
    user_config_path: &str,
    rows: &[DiffRow],
) -> String {
    let current_label = current_preset_name
        .and_then(Preset::from_name)
        .map(|p| p.cli_name().to_string())
        .unwrap_or_else(|| "(none)".to_string());
    let mut out = format!(
        "Preset change: {current_label} -> {}\nUser config file: {user_config_path}\n",
        new_preset.cli_name()
    );
    if rows.is_empty() {
        out.push_str("  no effective values change.\n");
    } else {
        for row in rows {
            match row {
                DiffRow::Changed { key, before, after } => {
                    out.push_str(&format!("  {key}: {before} -> {after}\n"));
                }
                DiffRow::Locked { key, effective } => {
                    out.push_str(&format!(
                        "  {key}: {effective} (managed by your organization; preset does not affect this key)\n"
                    ));
                }
                DiffRow::Kept { key, effective } => {
                    out.push_str(&format!(
                        "  {key}: {effective} (kept: your explicit setting overrides the preset)\n"
                    ));
                }
            }
        }
    }
    out
}

/// Resolve the CURRENT effective state (as declared on disk today) and the CANDIDATE state
/// under `new_preset`, from one policy load (ADR-0023 Decision 1) plus one read of the on-disk
/// user layer. Returns the current declared preset name too (for [`render_diff`]'s header).
fn resolve_current_and_candidate(
    domain_pattern_valid: fn(&str) -> bool,
    new_preset: Preset,
) -> crate::Result<(layers::Resolution, layers::Resolution, Option<String>)> {
    let user_manifest_source = std::env::var("BROWSER_MCP_MANIFEST").ok();
    let loaded_policy = crate::governance::manifest::source::load_policy(
        user_manifest_source.as_deref(),
        domain_pattern_valid,
    )
    .map_err(|e| crate::Error::Config(e.to_string()))?;
    let loaded = load::read_layers(domain_pattern_valid, &loaded_policy)?;
    let current_preset_name = loaded.user.preset.clone();
    let current = load::layer_inputs(
        loaded.org.clone(),
        loaded.user.values.clone(),
        current_preset_name.as_deref(),
    );
    let candidate = load::layer_inputs(loaded.org, loaded.user.values, Some(new_preset.as_str()));
    Ok((
        layers::resolve(&current),
        layers::resolve(&candidate),
        current_preset_name,
    ))
}

/// Write ONLY the `preset` field of the user config file at `path` (Required behavior section
/// 1, "rules for the write"): read-modify-write, re-reading the file at apply time, preserving
/// every other member (including the `config` map) exactly. A missing file is created (with
/// its parent directory, via [`crate::install::native_host::write_file_atomic`]) as
/// `{ "preset": "<name>" }`. A present-but-invalid-JSON or non-object-root file is refused,
/// left byte-for-byte untouched.
fn write_preset_at(path: &std::path::Path, preset: Preset) -> crate::Result<()> {
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
    obj.insert(
        "preset".to_string(),
        serde_json::Value::String(preset.as_str().to_string()),
    );

    let serialized = serde_json::to_string_pretty(&root).expect("value serializes") + "\n";
    crate::install::native_host::write_file_atomic(path, &serialized)
        .map_err(|e| crate::Error::Config(format!("cannot update {}: {e}", path.display())))
}

/// `browser-mcp config preset <name> [--dry-run]` (Required behavior section 1). Prints the
/// diff, then -- unless `dry_run` -- writes the preset and confirms.
pub fn run_preset(
    preset: Preset,
    dry_run: bool,
    domain_pattern_valid: fn(&str) -> bool,
) -> crate::Result<()> {
    let path = load::user_config_path().ok_or_else(|| {
        crate::Error::Config("no writable user config directory on this platform".to_string())
    })?;

    let (current, candidate, current_name) =
        resolve_current_and_candidate(domain_pattern_valid, preset)?;
    let rows = diff_rows(&current, &candidate, preset);
    print!(
        "{}",
        render_diff(
            current_name.as_deref(),
            preset,
            &path.display().to_string(),
            &rows
        )
    );

    if dry_run {
        println!("Dry run: nothing written.");
        return Ok(());
    }

    write_preset_at(&path, preset)?;
    println!("Preset '{}' saved.", preset.cli_name());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolve(inputs: &layers::LayerInputs) -> layers::Resolution {
        layers::resolve(inputs)
    }

    /// Required test 5 (g18 doc, Tests item 5): an org-mandatory `audit.enabled: true` and a
    /// user-layer `content.security.secrets.redact: true`, switching from `safe` to
    /// `fully_open`, yields exactly: a changed row for `governance.mode`
    /// (`"enforce" -> "observe"`), a locked notice for `audit.enabled`, and a kept notice for
    /// `content.security.secrets.redact`. Every other key produces no row.
    #[test]
    fn diff_rows_matches_the_required_locked_kept_changed_scenario() {
        let org = super::super::load::OrgConfig {
            mandatory: serde_json::Map::from_iter([(
                super::super::AUDIT_ENABLED.to_string(),
                json!(true),
            )]),
            recommended: serde_json::Map::new(),
        };
        let user_values = serde_json::Map::from_iter([(
            super::super::CONTENT_SECURITY_SECRETS_REDACT.to_string(),
            json!(true),
        )]);

        let current = resolve(&load::layer_inputs(
            org.clone(),
            user_values.clone(),
            Some("safe"),
        ));
        let candidate = resolve(&load::layer_inputs(org, user_values, Some("fully_open")));

        let rows = diff_rows(&current, &candidate, Preset::FullyOpen);
        assert_eq!(
            rows,
            vec![
                DiffRow::Kept {
                    key: super::super::CONTENT_SECURITY_SECRETS_REDACT,
                    effective: "true".to_string(),
                },
                DiffRow::Locked {
                    key: super::super::AUDIT_ENABLED,
                    effective: "true".to_string(),
                },
                DiffRow::Changed {
                    key: super::super::GOVERNANCE_MODE,
                    before: "\"enforce\"".to_string(),
                    after: "\"observe\"".to_string(),
                },
            ]
        );
    }

    /// Required test 6: from pristine defaults (no user file, no org file), selecting `safe`
    /// yields no rows at all, since layer 5 (builtin) already equals the Safe preset.
    #[test]
    fn diff_rows_is_empty_switching_to_safe_from_pristine_defaults() {
        let current = resolve(&layers::LayerInputs::default());
        let candidate = resolve(&load::layer_inputs(
            super::super::load::OrgConfig::default(),
            serde_json::Map::new(),
            Some("safe"),
        ));
        let rows = diff_rows(&current, &candidate, Preset::Safe);
        assert!(rows.is_empty(), "{rows:?}");
    }

    #[test]
    fn render_diff_header_uses_none_when_no_current_preset_is_declared() {
        let out = render_diff(None, Preset::Safe, "/x/config.json", &[]);
        assert!(out.starts_with("Preset change: (none) -> safe\n"));
        assert!(out.contains("User config file: /x/config.json\n"));
        assert!(out.ends_with("  no effective values change.\n"));
    }

    #[test]
    fn render_diff_header_uses_the_current_presets_cli_name() {
        let out = render_diff(Some("restricted"), Preset::FullyOpen, "/x/config.json", &[]);
        assert!(out.starts_with("Preset change: restricted -> fully-open\n"));
    }

    #[test]
    fn render_diff_renders_every_row_kind_exactly() {
        let rows = vec![
            DiffRow::Changed {
                key: "a.b",
                before: "1".to_string(),
                after: "2".to_string(),
            },
            DiffRow::Locked {
                key: "c.d",
                effective: "true".to_string(),
            },
            DiffRow::Kept {
                key: "e.f",
                effective: "\"x\"".to_string(),
            },
        ];
        let out = render_diff(None, Preset::Safe, "/p", &rows);
        assert!(out.contains("  a.b: 1 -> 2\n"));
        assert!(out.contains(
            "  c.d: true (managed by your organization; preset does not affect this key)\n"
        ));
        assert!(out.contains("  e.f: \"x\" (kept: your explicit setting overrides the preset)\n"));
    }

    fn with_temp_file<F: FnOnce(&std::path::Path)>(name: &str, initial: Option<&str>, f: F) {
        let dir = std::env::temp_dir().join(format!(
            "browser-mcp-g18-presets-{}-{}",
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

    #[test]
    fn write_preset_at_creates_a_missing_file_with_parent_directories() {
        with_temp_file("missing", None, |path| {
            assert!(!path.exists());
            write_preset_at(path, Preset::Restricted).unwrap();
            assert!(path.exists());
            let root: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            // Stored value is always the underscore form, regardless of the CLI spelling.
            assert_eq!(root, json!({ "preset": "restricted" }));
        });
    }

    #[test]
    fn write_preset_at_preserves_sibling_content_and_replaces_only_preset() {
        with_temp_file(
            "preserve",
            Some(
                r#"{"preset":"safe","config":{"content.security.secrets.redact":false},"future_member":{"x":1}}"#,
            ),
            |path| {
                write_preset_at(path, Preset::FullyOpen).unwrap();
                let root: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                assert_eq!(root["preset"], "fully_open");
                assert_eq!(root["future_member"], json!({"x": 1}));
                assert_eq!(root["config"]["content.security.secrets.redact"], false);
            },
        );
    }

    #[test]
    fn write_preset_at_refuses_invalid_json_and_leaves_it_untouched() {
        with_temp_file("badjson", Some("not json"), |path| {
            let before = std::fs::read_to_string(path).unwrap();
            let err = write_preset_at(path, Preset::Safe).unwrap_err();
            assert!(err.to_string().contains("not valid JSON"), "{err}");
            let after = std::fs::read_to_string(path).unwrap();
            assert_eq!(before, after, "file must be left byte-for-byte untouched");
        });
    }

    #[test]
    fn write_preset_at_refuses_a_non_object_root_and_leaves_it_untouched() {
        with_temp_file("nonobject", Some("[]"), |path| {
            let before = std::fs::read_to_string(path).unwrap();
            let err = write_preset_at(path, Preset::Safe).unwrap_err();
            assert!(
                err.to_string().contains("root is not a JSON object"),
                "{err}"
            );
            let after = std::fs::read_to_string(path).unwrap();
            assert_eq!(before, after);
        });
    }

    #[test]
    fn preset_stored_value_is_always_the_underscore_form() {
        for (preset, expected) in [
            (Preset::FullyOpen, "fully_open"),
            (Preset::Safe, "safe"),
            (Preset::Restricted, "restricted"),
        ] {
            with_temp_file(&format!("underscore-{expected}"), None, |path| {
                write_preset_at(path, preset).unwrap();
                let root: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
                assert_eq!(root["preset"], expected);
            });
        }
    }
}
