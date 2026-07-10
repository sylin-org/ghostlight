// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Manifest SOURCE resolution (shared format doc sections 1.2-1.3, ADR-0018 step 3): where the
//! active manifest comes from, and the selection rule when both an org policy file and a
//! user-supplied source are present. Domain-agnostic core: the org policy path is reused from
//! [`crate::governance::config::load::org_policy_path`] (G02) rather than re-derived a second
//! time. Mid-session reload, file watching, and re-advertisement are NOT this module's job (the
//! manifest is loaded once at startup); those are later stage-2 tasks.

use std::path::{Path, PathBuf};

use super::document::{parse_manifest, Level, Manifest, ManifestError};

/// Where a resolved user-supplied source string points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserSource {
    /// `env://NAME`: the named environment variable's value IS the manifest JSON text.
    EnvVar(String),
    /// Everything else: a filesystem path (after stripping `file://` and its Windows
    /// drive-letter convenience, `/C:/...` -> `C:/...`).
    FilePath(PathBuf),
}

/// Why a source string could not be resolved to a [`UserSource`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    /// `managed://` in the USER source string (`--manifest` / `GHOSTLIGHT_MANIFEST`): rejected by
    /// design (ADR-0055). Managed governance carries a TRUST ANCHOR (the org's verifying key), so it
    /// is never user-activatable; it is provisioned only through the admin-only `managed.json`
    /// bootstrap (`managed.json`, located by [`crate::governance::paths::GovernancePaths`]).
    #[error("managed:// is not a user-supplied source; it is provisioned by the administrator via the managed.json bootstrap (ADR-0055)")]
    ManagedNotSupported,
    /// Any other `<scheme>://`.
    #[error("unsupported manifest source scheme '{0}://'")]
    UnsupportedScheme(String),
}

/// Parse the `--manifest` / `GHOSTLIGHT_MANIFEST` source-string grammar (shared format doc
/// section 1.3): `env://NAME`, `file://<path>` (with the Windows drive-letter convenience),
/// `managed://` (a precise unsupported error), any other `<scheme>://` (an error naming the
/// scheme), or a bare string (a filesystem path).
pub fn parse_source_string(s: &str) -> Result<UserSource, SourceError> {
    if let Some(rest) = s.strip_prefix("env://") {
        return Ok(UserSource::EnvVar(rest.to_string()));
    }
    if let Some(rest) = s.strip_prefix("file://") {
        return Ok(UserSource::FilePath(PathBuf::from(
            strip_windows_drive_slash(rest),
        )));
    }
    if s.starts_with("managed://") {
        return Err(SourceError::ManagedNotSupported);
    }
    if let Some(idx) = s.find("://") {
        return Err(SourceError::UnsupportedScheme(s[..idx].to_string()));
    }
    Ok(UserSource::FilePath(PathBuf::from(s)))
}

/// Strip a `file://` remainder's leading slash before a Windows drive letter (`/C:/...` ->
/// `C:/...`); everything else passes through unchanged.
fn strip_windows_drive_slash(rest: &str) -> &str {
    let bytes = rest.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        &rest[1..]
    } else {
        rest
    }
}

/// Where the active manifest came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestOrigin {
    OrgPolicyFile,
    UserFile,
    UserEnv,
    /// A signed policy bundle activated via the admin-provisioned managed:// bootstrap (ADR-0055).
    /// Org-authoritative like [`ManifestOrigin::OrgPolicyFile`]: its config entries take the org
    /// channel and it triggers the license stamp gate (both wired in ADR-0055 Phase 4). Loading and
    /// verification live in [`crate::governance::managed`].
    Managed,
}

/// The result of source selection and loading (shared format doc section 1.3).
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPolicy {
    /// The active manifest, or `None` for all-open.
    pub manifest: Option<Manifest>,
    /// Where the active manifest came from, when there is one.
    pub origin: Option<ManifestOrigin>,
    /// `true` when an org policy file displaced a user-supplied manifest's grants (shared
    /// format doc section 1.3 rule 1). The `user_manifest_ignored` session-event audit note
    /// (ADR-0025 Decision 5) is recorded by `transport::mcp::server`'s startup and
    /// policy-subscription logic, not here: this module stays domain-agnostic and audit-free.
    pub user_manifest_ignored: bool,
}

/// Why loading the selected source(s) failed. A source that is SELECTED but cannot be read,
/// parsed, or validated is always fatal (shared format doc section 1.3): absence is normal
/// (all-open); presence of a broken one is never silently ignored.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("manifest source: {0}")]
    Source(#[from] SourceError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("failed to read manifest file '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("environment variable '{0}' is not set or is empty")]
    EmptyEnvVar(String),
}

/// Pure: given only whether an org-policy manifest and a user-supplied manifest are each
/// PRESENT, decide which wins (shared format doc section 1.3) and whether the user manifest's
/// grants are ignored. Org always wins when present. Testable without touching real files or
/// the environment.
fn select(org_present: bool, user_present: bool) -> (bool, bool) {
    // (org_wins, user_ignored)
    match (org_present, user_present) {
        (true, true) => (true, true),
        (true, false) => (true, false),
        (false, _) => (false, false),
    }
}

/// Read and parse the org policy file at `path`, if any. `Ok(None)` means the file does not
/// exist (normal: no org policy). `Err` means it exists but could not be read or is invalid --
/// always fatal (an org policy that fails open is worse than a startup crash).
fn load_org_manifest_at(
    path: &Path,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<Option<Manifest>, LoadError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let label = path.display().to_string();
            Ok(Some(parse_manifest(&text, &label, domain_pattern_valid)?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(LoadError::Io {
            path: path.display().to_string(),
            source: e,
        }),
    }
}

/// Resolve a user-supplied source string to its manifest text plus a source label, then parse
/// it. `env://NAME` reads the named environment variable (missing or empty is a load error
/// naming the variable); everything else is read as a file.
fn load_user_manifest(
    source_string: &str,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<(Manifest, ManifestOrigin), LoadError> {
    match parse_source_string(source_string)? {
        UserSource::EnvVar(name) => {
            let text = std::env::var(&name).ok().filter(|v| !v.is_empty());
            let Some(text) = text else {
                return Err(LoadError::EmptyEnvVar(name));
            };
            let label = format!("env://{name}");
            let manifest = parse_manifest(&text, &label, domain_pattern_valid)?;
            Ok((manifest, ManifestOrigin::UserEnv))
        }
        UserSource::FilePath(path) => {
            let text = std::fs::read_to_string(&path).map_err(|e| LoadError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            let label = path.display().to_string();
            let manifest = parse_manifest(&text, &label, domain_pattern_valid)?;
            Ok((manifest, ManifestOrigin::UserFile))
        }
    }
}

/// Resolve the active manifest source and load it (shared format doc sections 1.2-1.3). This
/// is the impure orchestration: it reads the real org policy file
/// ([`crate::governance::config::load::org_policy_path`]) and, if given, the user-supplied
/// source, then applies the pure [`select`] rule. A user-supplied manifest is ALWAYS parsed
/// and validated when given, even when the org file displaces its grants (rule 1: its errors
/// are still fatal, and its config entries still apply at the user layer via
/// [`manifest_config_as_user_layer`]).
pub fn load_policy(
    user_source_string: Option<&str>,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<LoadedPolicy, LoadError> {
    let org = load_org_manifest_at(
        &crate::governance::config::load::org_policy_path(),
        domain_pattern_valid,
    )?;
    let user = user_source_string
        .map(|s| load_user_manifest(s, domain_pattern_valid))
        .transpose()?;
    Ok(combine(org, user))
}

/// Pure combination of the already-loaded org/user manifests into the final [`LoadedPolicy`],
/// per the [`select`] rule. Factored out so [`load_policy`]'s only impure work is the two
/// reads above.
fn combine(org: Option<Manifest>, user: Option<(Manifest, ManifestOrigin)>) -> LoadedPolicy {
    let (org_wins, user_ignored) = select(org.is_some(), user.is_some());
    if user_ignored {
        if let Some((manifest, _)) = &user {
            tracing::warn!(
                name = %manifest.name,
                "org policy file present: user-supplied manifest's grants are ignored; its \
                 config entries still apply at the user layer"
            );
        }
    }

    if org_wins {
        LoadedPolicy {
            manifest: org,
            origin: Some(ManifestOrigin::OrgPolicyFile),
            user_manifest_ignored: user_ignored,
        }
    } else if let Some((manifest, origin)) = user {
        LoadedPolicy {
            manifest: Some(manifest),
            origin: Some(origin),
            user_manifest_ignored: false,
        }
    } else {
        LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        }
    }
}

/// The active manifest's config entries, downgraded to a single flat user-layer map (shared
/// format doc section 1.3 rule 2): a user-supplied manifest's entries ALWAYS land in the user
/// layer regardless of their declared `level`; an entry declaring `level: mandatory` is
/// downgraded with a `tracing::warn!` naming the key. An org-sourced manifest (or no manifest
/// at all) yields an empty map: an org-sourced manifest's `config` entries take the ORG
/// channel instead (`governance::config::load::org_config_from_entries`, ADR-0023 Decision 2),
/// not because a second parser reads the file -- `parse_manifest` is the ONLY reader/parser of
/// the policy file (ADR-0023 Decision 1), so feeding these entries again here would be a
/// redundant second path for the SAME already-parsed entries, not a new one.
pub fn manifest_config_as_user_layer(
    loaded: &LoadedPolicy,
) -> serde_json::Map<String, serde_json::Value> {
    let is_user_sourced = matches!(
        loaded.origin,
        Some(ManifestOrigin::UserFile) | Some(ManifestOrigin::UserEnv)
    );
    let mut map = serde_json::Map::new();
    if !is_user_sourced {
        return map;
    }
    let Some(manifest) = &loaded.manifest else {
        return map;
    };
    for entry in &manifest.config {
        if entry.level == Level::Mandatory {
            tracing::warn!(
                key = %entry.key,
                "user-supplied manifest declared 'mandatory' for a user-layer entry; \
                 downgraded to user level"
            );
        }
        map.insert(entry.key.clone(), entry.value.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_valid_pattern(_: &str) -> bool {
        true
    }

    #[test]
    fn env_scheme_extracts_the_variable_name() {
        assert_eq!(
            parse_source_string("env://MY_VAR").unwrap(),
            UserSource::EnvVar("MY_VAR".to_string())
        );
    }

    #[test]
    fn file_scheme_yields_the_path() {
        assert_eq!(
            parse_source_string("file:///etc/x.json").unwrap(),
            UserSource::FilePath(PathBuf::from("/etc/x.json"))
        );
    }

    #[test]
    fn file_scheme_strips_the_windows_drive_slash() {
        assert_eq!(
            parse_source_string("file:///C:/x.json").unwrap(),
            UserSource::FilePath(PathBuf::from("C:/x.json"))
        );
    }

    #[test]
    fn bare_path_passes_through() {
        assert_eq!(
            parse_source_string("/etc/x.json").unwrap(),
            UserSource::FilePath(PathBuf::from("/etc/x.json"))
        );
    }

    #[test]
    fn managed_scheme_is_a_precise_error() {
        assert!(matches!(
            parse_source_string("managed://foo"),
            Err(SourceError::ManagedNotSupported)
        ));
    }

    #[test]
    fn unknown_scheme_names_the_scheme() {
        match parse_source_string("weird://x") {
            Err(SourceError::UnsupportedScheme(scheme)) => assert_eq!(scheme, "weird"),
            other => panic!("expected UnsupportedScheme, got {other:?}"),
        }
    }

    #[test]
    fn selection_org_and_user_present_org_wins_and_user_is_ignored() {
        assert_eq!(select(true, true), (true, true));
    }

    #[test]
    fn selection_org_only_present_org_wins_not_ignored() {
        assert_eq!(select(true, false), (true, false));
    }

    #[test]
    fn selection_user_only_present_user_wins() {
        assert_eq!(select(false, true), (false, false));
    }

    #[test]
    fn selection_neither_present_is_all_open() {
        assert_eq!(select(false, false), (false, false));
    }

    fn minimal_manifest(name: &str) -> String {
        format!(r#"{{"schema":3,"name":"{name}","version":"1","grants":[]}}"#)
    }

    #[test]
    fn combine_prefers_org_and_marks_user_ignored() {
        let org =
            Some(parse_manifest(&minimal_manifest("org"), "org", always_valid_pattern).unwrap());
        let user = Some((
            parse_manifest(&minimal_manifest("user"), "user", always_valid_pattern).unwrap(),
            ManifestOrigin::UserFile,
        ));
        let loaded = combine(org, user);
        assert_eq!(loaded.manifest.unwrap().name, "org");
        assert_eq!(loaded.origin, Some(ManifestOrigin::OrgPolicyFile));
        assert!(loaded.user_manifest_ignored);
    }

    #[test]
    fn combine_uses_user_when_org_absent() {
        let user = Some((
            parse_manifest(&minimal_manifest("user"), "user", always_valid_pattern).unwrap(),
            ManifestOrigin::UserEnv,
        ));
        let loaded = combine(None, user);
        assert_eq!(loaded.manifest.unwrap().name, "user");
        assert_eq!(loaded.origin, Some(ManifestOrigin::UserEnv));
        assert!(!loaded.user_manifest_ignored);
    }

    #[test]
    fn combine_both_absent_is_all_open() {
        let loaded = combine(None, None);
        assert_eq!(loaded.manifest, None);
        assert_eq!(loaded.origin, None);
        assert!(!loaded.user_manifest_ignored);
    }

    #[test]
    fn load_org_manifest_at_absent_file_is_ok_none() {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-manifest-source-test-{}-absent.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        assert!(matches!(
            load_org_manifest_at(&path, always_valid_pattern),
            Ok(None)
        ));
    }

    #[test]
    fn load_org_manifest_at_reads_and_parses_a_real_file() {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-manifest-source-test-{}-present.json",
            std::process::id()
        ));
        std::fs::write(&path, minimal_manifest("org-file")).unwrap();
        let result = load_org_manifest_at(&path, always_valid_pattern);
        std::fs::remove_file(&path).ok();
        assert_eq!(result.unwrap().unwrap().name, "org-file");
    }

    #[test]
    fn load_org_manifest_at_invalid_file_is_fatal() {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-manifest-source-test-{}-invalid.json",
            std::process::id()
        ));
        std::fs::write(&path, "not json").unwrap();
        let result = load_org_manifest_at(&path, always_valid_pattern);
        std::fs::remove_file(&path).ok();
        assert!(matches!(result, Err(LoadError::Manifest(_))));
    }

    #[test]
    fn manifest_config_as_user_layer_downgrades_mandatory_and_ignores_org_origin() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[],
            "config":[{"key":"audit.enabled","value":true,"level":"mandatory"}]}"#;
        let manifest = parse_manifest(json, "test", always_valid_pattern).unwrap();

        let user_loaded = LoadedPolicy {
            manifest: Some(manifest.clone()),
            origin: Some(ManifestOrigin::UserFile),
            user_manifest_ignored: false,
        };
        let map = manifest_config_as_user_layer(&user_loaded);
        assert_eq!(map.get("audit.enabled"), Some(&serde_json::json!(true)));

        let org_loaded = LoadedPolicy {
            manifest: Some(manifest),
            origin: Some(ManifestOrigin::OrgPolicyFile),
            user_manifest_ignored: false,
        };
        assert!(manifest_config_as_user_layer(&org_loaded).is_empty());
    }

    #[test]
    fn manifest_config_as_user_layer_empty_when_no_manifest() {
        let loaded = LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        };
        assert!(manifest_config_as_user_layer(&loaded).is_empty());
    }
}
