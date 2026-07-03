//! Deterministic plain-language policy rendering (ADR-0020 commitment 2, g16; grant rendering
//! updated for ADR-0022 Decisions 3/4/8).
//!
//! `policy explain` is a trust surface: an administrator reads exactly what a policy manifest
//! does, in sentences generated from the same parsed structures the engine enforces, before
//! shipping it. The template set in this module is fixed -- no paraphrasing, no reordering, no
//! "improving" a sentence -- and pinned byte-for-byte by golden tests, because a rendering bug
//! that misstates policy is a serious defect, not a cosmetic one. [`explain_manifest`] and
//! [`explain_user_config`] are exported as public library functions so a future import-preview
//! surface reuses the EXACT same sentences an administrator already reviewed: the sentence an
//! admin sees and the sentence a user sees can never drift apart.
//!
//! Pure: no I/O, no clock, no randomness, no platform lookups, no `HashMap` iteration order
//! anywhere in the output path (manifest and config types preserve author order via
//! `preserve_order`; the key registry is an ordered const slice). Identical input yields
//! byte-identical output on every platform. [`explain_file`] is the one impure entry point (it
//! reads a path); `domain_pattern_valid` is an injected function pointer, the same "known
//! integration point" shape used everywhere else in `governance/`, so this domain-agnostic core
//! module never names `browser::`/`transport::` directly (the a7 arch-test).

use std::path::Path;

use crate::governance::config::layers::validate_value;
use crate::governance::config::{key_def, Config, Preset};
use crate::governance::manifest::document::{
    parse_manifest, ConfigEntry, Grant, IdentityBlock, Level, Manifest, ManifestError,
};
use crate::governance::ports::{Capability, EffectiveMode};

/// Why [`explain_file`] could not produce a rendering. A manifest that parses but fails
/// validation surfaces as [`ExplainError::Manifest`] -- explain never renders a best-effort
/// explanation of an invalid manifest; a half-explained policy is a misstated policy.
#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    #[error("failed to read '{path}': {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("'{path}' is not valid UTF-8")]
    NotUtf8 { path: String },
    #[error("'{path}': invalid JSON: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

/// A parsed user configuration file (shared format doc section 1.1), the input
/// [`explain_user_config`] renders. Same shape as
/// [`crate::governance::config::load::UserConfig`] (preset name, validated flat value map in
/// file order); a distinct type here because explain's own warning wording (section 4.5's
/// exact sentences) differs from that loader's log-oriented warning strings, so explain
/// resolves its own warnings alongside this rather than reusing that loader's warning output
/// verbatim (see [`explain_file`]'s doc comment).
#[derive(Debug, Clone, Default)]
pub struct UserConfigFile {
    pub preset: Option<String>,
    pub values: serde_json::Map<String, serde_json::Value>,
}

/// Join rendered blocks with exactly one blank line and end with exactly one trailing `\n`
/// (Required behavior section 2). Always `\n`, never `\r\n`.
fn join_blocks(blocks: &[String]) -> String {
    let mut out = blocks.join("\n\n");
    out.push('\n');
    out
}

// --- Manifest rendering ---

/// Render a parsed manifest as the fixed-template plain-language explanation (Required
/// behavior sections 2-3). `hash` is the 64-lowercase-hex content hash (shared format 4.2) of
/// the source bytes -- pass `&manifest.hash` (already computed by [`parse_manifest`]; this
/// function never recomputes it).
pub fn explain_manifest(manifest: &Manifest, hash: &str) -> String {
    let (mode, mode_suffix) = resolve_manifest_mode(manifest);

    let mut blocks = vec![
        header_block(manifest, hash),
        identity_block(&manifest.identity),
        mode_block(mode, mode_suffix),
        grants_block(&manifest.grants, mode),
        settings_block(&manifest.config),
        denial_block(mode),
    ];

    let warnings = collect_manifest_warnings(&manifest.grants);
    if !warnings.is_empty() {
        blocks.push(warnings_block(&warnings));
    }

    join_blocks(&blocks)
}

fn header_block(manifest: &Manifest, hash: &str) -> String {
    format!(
        "Policy '{}', version {}.\nContent hash: {hash}.",
        manifest.name, manifest.version
    )
}

fn identity_block(identity: &Option<IdentityBlock>) -> String {
    let Some(id) = identity else {
        return "No identity block: this policy does not name a principal.".to_string();
    };
    let principal = id.principal.as_deref().unwrap_or("(not specified)");
    let resolved_by = id.resolved_by.as_deref().unwrap_or("(not specified)");
    let mut line = format!("Prepared for '{principal}', resolved by {resolved_by}.");
    if let Some(groups) = &id.groups {
        if !groups.is_empty() {
            line.push_str(&format!(" Groups: {}.", groups.join(", ")));
        }
    }
    line
}

/// Resolve the manifest-level effective mode and its explanatory suffix (Required behavior
/// section 3, mirroring shared format 3.4 for a file explained in isolation): (a) the
/// manifest's own `mode` field; else (b)/(c) a `governance.mode` config entry at
/// mandatory/recommended level; else (d) the registry's built-in default (Safe preset, the
/// same "byte-identical to stage 1" baseline used everywhere else in this codebase).
fn resolve_manifest_mode(manifest: &Manifest) -> (EffectiveMode, &'static str) {
    if let Some(mode) = manifest.mode {
        return (mode, "");
    }
    if let Some(mode) = manifest_mode_config_entry(manifest, Level::Mandatory) {
        return (mode, " This mode is locked by the policy.");
    }
    if let Some(mode) = manifest_mode_config_entry(manifest, Level::Recommended) {
        return (mode, " This mode is a default the user may change.");
    }
    let built_in = Config::minimal().governance_mode();
    (
        built_in,
        " This policy sets no mode; the built-in default applies.",
    )
}

fn manifest_mode_config_entry(manifest: &Manifest, level: Level) -> Option<EffectiveMode> {
    manifest
        .config
        .iter()
        .find(|e| e.key == "governance.mode" && e.level == level)
        .map(|e| EffectiveMode::from_config_str(e.value.as_str().unwrap_or("enforce")))
}

fn mode_block(mode: EffectiveMode, suffix: &str) -> String {
    let base = match mode {
        EffectiveMode::Enforce => {
            "Mode: enforce. Calls the grants below do not permit are blocked."
        }
        EffectiveMode::Observe => {
            "Mode: observe (shadow). Nothing is blocked by this policy: would-deny events are \
             recorded to the audit log and the calls proceed. Observation is not protection."
        }
    };
    format!("{base}{suffix}")
}

fn grants_block(grants: &[Grant], mode: EffectiveMode) -> String {
    if grants.is_empty() {
        return match mode {
            EffectiveMode::Enforce => {
                "Where agents may read and write: nowhere. This policy grants no domains.\n\
                 Every tool call on every page is denied."
                    .to_string()
            }
            EffectiveMode::Observe => {
                "Where agents may read and write: nowhere. This policy grants no domains.\n\
                 Every tool call would be denied; in this mode those denials are recorded, not \
                 blocked."
                    .to_string()
            }
        };
    }

    let mut lines = vec![
        "Where agents may read and write, in match order (the first matching domain wins):"
            .to_string(),
        "(a pattern like 'example.com' matches only that exact host; '*.example.com' matches \
         its subdomains and never example.com itself)"
            .to_string(),
    ];
    for (i, grant) in grants.iter().enumerate() {
        lines.push(format!("  {}. {}", i + 1, grant_line(grant)));
    }
    lines.push(match mode {
        EffectiveMode::Enforce => "Any domain not matched above is denied.".to_string(),
        EffectiveMode::Observe => {
            "Any domain not matched above would be denied; in this mode that denial is \
             recorded, not blocked."
                .to_string()
        }
    });
    lines.join("\n")
}

/// The fixed capability -> agent-facing phrase table (ADR-0022 Decision 1), rendered in this
/// exact order regardless of the manifest's authored order in `allowed`. The `action` phrase
/// carries the ADR's mandated warning inline: `action` is not a weaker `write`; it can cause
/// one.
const CAPABILITY_PHRASES: &[(Capability, &str)] = &[
    (Capability::Read, "read pages"),
    (
        Capability::Action,
        "operate page controls (clicks and typing; this can trigger writes)",
    ),
    (Capability::Write, "submit forms and structured writes"),
    (Capability::Execute, "run arbitrary JavaScript"),
];

/// Render `hosts.allow` (ADR-0022 Decision 4): the single pattern `*` renders as `every site`;
/// an empty list renders as `no sites`; otherwise the patterns, comma-joined, verbatim.
fn render_hosts(allow: &[String]) -> String {
    if allow == ["*"] {
        "every site".to_string()
    } else if allow.is_empty() {
        "no sites".to_string()
    } else {
        allow.join(", ")
    }
}

/// The `Allowed on {hosts}: {phrases}.` sentence (ADR-0022 Decision 8 point 1): an empty
/// `allowed` renders the explicit "nothing granted" wording rather than an empty phrase list.
fn allowed_sentence(grant: &Grant) -> String {
    let hosts = render_hosts(&grant.hosts.allow);
    if grant.allowed.is_empty() {
        return format!("Allowed on {hosts}: nothing (no capabilities granted).");
    }
    let phrases: Vec<&str> = CAPABILITY_PHRASES
        .iter()
        .filter(|(cap, _)| grant.allowed.contains(cap))
        .map(|(_, phrase)| *phrase)
        .collect();
    format!("Allowed on {hosts}: {}.", phrases.join(", "))
}

fn grant_line(grant: &Grant) -> String {
    let mut sentences = vec![allowed_sentence(grant)];

    if !grant.hosts.deny.is_empty() {
        sentences.push(format!("Excluded: {}.", grant.hosts.deny.join(", ")));
    }

    match grant.mode {
        Some(EffectiveMode::Enforce) => sentences.push(
            "This grant always enforces: its denials block even when the policy mode is \
             observe."
                .to_string(),
        ),
        Some(EffectiveMode::Observe) => sentences.push(
            "This grant is always observe-only: its denials are recorded, never blocked."
                .to_string(),
        ),
        None => {}
    }

    if let Some(description) = &grant.description {
        sentences.push(format!("Purpose: {description}."));
    }

    sentences.join(" ")
}

fn settings_block(config: &[ConfigEntry]) -> String {
    if config.is_empty() {
        return "This policy locks no settings and sets no defaults; users keep control of \
                every setting."
            .to_string();
    }

    let mandatory: Vec<&ConfigEntry> = config
        .iter()
        .filter(|e| e.level == Level::Mandatory)
        .collect();
    let recommended: Vec<&ConfigEntry> = config
        .iter()
        .filter(|e| e.level == Level::Recommended)
        .collect();

    let mut lines = Vec::new();
    if !mandatory.is_empty() {
        lines.push("Settings locked by the organization (users cannot change these):".to_string());
        for entry in &mandatory {
            lines.push(settings_entry_line(entry));
        }
    }
    if !recommended.is_empty() {
        lines.push("Org-recommended defaults (users may change these):".to_string());
        for entry in &recommended {
            lines.push(settings_entry_line(entry));
        }
    }
    lines.push("All other settings keep their user, preset, or built-in values.".to_string());
    if !mandatory.is_empty() {
        lines.push(
            "If a user loads this file themselves instead of the organization installing it \
             as the org policy file, the locked entries above become user-level defaults and \
             nothing is locked."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn settings_entry_line(entry: &ConfigEntry) -> String {
    let value = serde_json::to_string(&entry.value).expect("a config entry value serializes");
    let desc = key_def(&entry.key)
        .expect("a validated manifest's config keys are registered")
        .description;
    format!("  - {} = {value} ({desc})", entry.key)
}

fn denial_block(mode: EffectiveMode) -> String {
    match mode {
        EffectiveMode::Enforce => {
            "On a denial the agent receives a plain-text message with a stable denial id, in \
             the form 'Denied (D-xxxxxxxx): ...'. Hand that id to the policy administrator: it \
             identifies the exact rule and policy version that produced the denial."
                .to_string()
        }
        EffectiveMode::Observe => {
            "On a would-deny the agent sees the ordinary tool result and no denial text. The \
             denial id appears only in the audit record, as decision 'shadow_deny'."
                .to_string()
        }
    }
}

fn warnings_block(warnings: &[String]) -> String {
    let mut lines = vec!["Warnings:".to_string()];
    for w in warnings {
        lines.push(format!("  - {w}"));
    }
    lines.join("\n")
}

/// Warning collection order is deterministic (Required behavior section 3, block 7): iterate
/// grants in manifest order; for each grant emit first the acting-without-read lint (ADR-0022
/// Decision 3), then one non-ASCII-host-pattern lint per offending pattern across `hosts.allow`
/// then `hosts.deny`, in that order. The non-ASCII lint is unreachable via [`explain_file`]'s
/// own pipeline today (a manifest that parsed successfully already passed
/// `host_pattern_valid`, which rejects non-ASCII patterns outright) but is exercised directly
/// by this module's own tests, since [`explain_manifest`] makes no assumption about how its
/// caller validated its input.
fn collect_manifest_warnings(grants: &[Grant]) -> Vec<String> {
    let mut warnings = Vec::new();
    for grant in grants {
        let acts_without_reading = grant.allowed.iter().any(|c| {
            matches!(
                c,
                Capability::Action | Capability::Write | Capability::Execute
            )
        }) && !grant.allowed.contains(&Capability::Read);
        if acts_without_reading {
            warnings.push(format!(
                "grant '{}': allowed includes acting capabilities without 'read'; agents can \
                 act on pages they cannot see.",
                grant.id
            ));
        }
        for pattern in grant.hosts.allow.iter().chain(grant.hosts.deny.iter()) {
            if !pattern.is_ascii() {
                warnings.push(format!(
                    "grant '{}': domain pattern '{pattern}' contains non-ASCII characters; \
                     author IDN domains in punycode (A-label) form.",
                    grant.id
                ));
            }
        }
    }
    warnings
}

// --- User configuration file rendering ---

/// Render a parsed user configuration file (Required behavior section 4). `warnings` are the
/// load warnings (unknown key, invalid value, unknown preset) in file order.
pub fn explain_user_config(file: &UserConfigFile, warnings: &[String]) -> String {
    let mut blocks = vec![
        "User configuration file (not a policy manifest).".to_string(),
        match &file.preset {
            Some(p) => format!("Preset: {p}."),
            None => "Preset: none (the built-in defaults apply).".to_string(),
        },
        user_settings_block(&file.values),
        "Nothing here is locked: only an org policy file can lock settings.".to_string(),
    ];
    if !warnings.is_empty() {
        blocks.push(warnings_block(warnings));
    }
    join_blocks(&blocks)
}

fn user_settings_block(values: &serde_json::Map<String, serde_json::Value>) -> String {
    if values.is_empty() {
        return "User settings: none.".to_string();
    }
    let mut lines = vec!["User settings:".to_string()];
    for (key, value) in values {
        let value_str = serde_json::to_string(value).expect("a config value serializes");
        let desc = key_def(key)
            .expect("a validated user-config key is registered")
            .description;
        lines.push(format!("  - {key} = {value_str} ({desc})"));
    }
    lines.join("\n")
}

/// Structural parse of a user config file's `preset`/`config` members, producing explain's own
/// exact warning wording (Required behavior section 4.5). Reuses the real validation
/// primitives ([`Preset::from_name`], [`key_def`], [`validate_value`]) rather than
/// re-implementing constraint checking; only the JSON navigation and the warning SENTENCES are
/// local to this module, since [`crate::governance::config::load::parse_user_config`]'s own
/// warning strings are formatted for its own (log-oriented) callers and do not match the exact
/// sentences this task's templates require.
fn parse_user_config_file(
    root: &serde_json::Value,
    domain_pattern_valid: fn(&str) -> bool,
) -> (UserConfigFile, Vec<String>) {
    let mut warnings = Vec::new();
    let obj = root.as_object();

    let mut preset = None;
    if let Some(p) = obj.and_then(|o| o.get("preset")).and_then(|v| v.as_str()) {
        if Preset::from_name(p).is_some() {
            preset = Some(p.to_string());
        } else {
            warnings.push(format!(
                "unknown preset '{p}'; the built-in defaults apply."
            ));
        }
    }

    let mut values = serde_json::Map::new();
    if let Some(cfg) = obj
        .and_then(|o| o.get("config"))
        .and_then(|v| v.as_object())
    {
        for (key, value) in cfg {
            match key_def(key) {
                None => warnings.push(format!("unknown key '{key}' is ignored.")),
                Some(def) => match validate_value(def, value, domain_pattern_valid) {
                    Ok(()) => {
                        values.insert(key.clone(), value.clone());
                    }
                    Err(reason) => {
                        warnings.push(format!("invalid value for '{key}' is ignored ({reason})."));
                    }
                },
            }
        }
    }

    (UserConfigFile { preset, values }, warnings)
}

// --- File loading and kind detection ---

/// Load a file, detect its kind, and render it (Required behavior section 1). Detection: strip
/// a UTF-8 BOM if present, parse as JSON; a top-level object containing a `"schema"` member is
/// a manifest (parsed and validated by [`parse_manifest`]; validation failures are errors,
/// never a best-effort rendering); anything else is treated as a user configuration file
/// (shared format 1.1; unknown keys and invalid values become warnings, not errors).
///
/// Reads exactly the one named file: no live org policy file, no live user config file, no
/// environment variable, no platform path. `domain_pattern_valid` is the browser plugin's real
/// host-pattern checker, injected by the caller (the composition root) so this
/// governance-core module never names `browser::`/`transport::` directly (the a7 arch-test).
pub fn explain_file(
    path: &Path,
    domain_pattern_valid: fn(&str) -> bool,
) -> Result<String, ExplainError> {
    let path_str = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| ExplainError::Io {
        path: path_str.clone(),
        source: e,
    })?;
    let stripped = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(bytes.as_slice());
    let text = std::str::from_utf8(stripped).map_err(|_| ExplainError::NotUtf8 {
        path: path_str.clone(),
    })?;
    let root: serde_json::Value = serde_json::from_str(text).map_err(|e| ExplainError::Json {
        path: path_str.clone(),
        source: e,
    })?;

    let is_manifest = root
        .as_object()
        .map(|o| o.contains_key("schema"))
        .unwrap_or(false);

    if is_manifest {
        let manifest = parse_manifest(text, &path_str, domain_pattern_valid)?;
        let hash = manifest.hash.clone();
        Ok(explain_manifest(&manifest, &hash))
    } else {
        let (file, warnings) = parse_user_config_file(&root, domain_pattern_valid);
        Ok(explain_user_config(&file, &warnings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::document::HostRules;

    fn always_valid_pattern(_: &str) -> bool {
        true
    }

    fn sample_grant(id: &str, allowed: &[Capability]) -> Grant {
        Grant {
            id: id.to_string(),
            hosts: HostRules {
                allow: vec!["example.com".to_string()],
                deny: Vec::new(),
            },
            allowed: allowed.to_vec(),
            description: None,
            mode: None,
        }
    }

    fn sample_manifest(mode: Option<EffectiveMode>, grants: Vec<Grant>) -> Manifest {
        Manifest {
            schema: 3,
            name: "a".to_string(),
            version: "1".to_string(),
            mode,
            identity: None,
            grants,
            config: Vec::new(),
            hash: "0".repeat(64),
        }
    }

    #[test]
    fn allowed_sentences_are_exact_per_capability() {
        assert_eq!(
            grant_line(&sample_grant("g", &[Capability::Read])),
            "Allowed on example.com: read pages."
        );
        assert_eq!(
            grant_line(&sample_grant("g", &[Capability::Action])),
            "Allowed on example.com: operate page controls (clicks and typing; this can \
             trigger writes)."
        );
        assert_eq!(
            grant_line(&sample_grant("g", &[Capability::Write])),
            "Allowed on example.com: submit forms and structured writes."
        );
        assert_eq!(
            grant_line(&sample_grant("g", &[Capability::Execute])),
            "Allowed on example.com: run arbitrary JavaScript."
        );
        assert_eq!(
            grant_line(&sample_grant(
                "g",
                &[
                    Capability::Read,
                    Capability::Action,
                    Capability::Write,
                    Capability::Execute
                ]
            )),
            "Allowed on example.com: read pages, operate page controls (clicks and typing; \
             this can trigger writes), submit forms and structured writes, run arbitrary \
             JavaScript."
        );
    }

    #[test]
    fn empty_allowed_renders_the_nothing_granted_wording() {
        assert_eq!(
            grant_line(&sample_grant("g", &[])),
            "Allowed on example.com: nothing (no capabilities granted)."
        );
    }

    #[test]
    fn hosts_render_star_as_every_site_and_empty_as_no_sites() {
        let mut star = sample_grant("g", &[Capability::Read]);
        star.hosts.allow = vec!["*".to_string()];
        assert_eq!(grant_line(&star), "Allowed on every site: read pages.");

        let mut empty = sample_grant("g", &[Capability::Read]);
        empty.hosts.allow = vec![];
        assert_eq!(grant_line(&empty), "Allowed on no sites: read pages.");
    }

    #[test]
    fn deny_list_renders_the_excluded_sentence() {
        let mut g = sample_grant("g", &[Capability::Read]);
        g.hosts.allow = vec!["*".to_string()];
        g.hosts.deny = vec!["evil.com".to_string(), "bad.example.com".to_string()];
        assert_eq!(
            grant_line(&g),
            "Allowed on every site: read pages. Excluded: evil.com, bad.example.com."
        );
    }

    #[test]
    fn acting_without_read_lint_fires_for_action_write_or_execute_without_read() {
        for caps in [
            vec![Capability::Action],
            vec![Capability::Write],
            vec![Capability::Execute],
            vec![Capability::Action, Capability::Write],
        ] {
            let g = sample_grant("w", &caps);
            let warnings = collect_manifest_warnings(std::slice::from_ref(&g));
            assert_eq!(
                warnings,
                vec![
                    "grant 'w': allowed includes acting capabilities without 'read'; agents \
                     can act on pages they cannot see."
                        .to_string()
                ],
                "{caps:?}"
            );
        }

        for caps in [
            vec![Capability::Read],
            vec![Capability::Read, Capability::Write],
            vec![],
        ] {
            let g = sample_grant("g", &caps);
            assert!(
                collect_manifest_warnings(&[g]).is_empty(),
                "{caps:?} must not warn"
            );
        }
    }

    #[test]
    fn non_ascii_pattern_lint_is_exact() {
        let mut g = sample_grant("g", &[Capability::Read]);
        g.hosts.allow = vec!["b\u{fc}cher.de".to_string()];
        let warnings = collect_manifest_warnings(&[g]);
        assert_eq!(
            warnings,
            vec![
                "grant 'g': domain pattern 'b\u{fc}cher.de' contains non-ASCII characters; \
                 author IDN domains in punycode (A-label) form."
                    .to_string()
            ]
        );
    }

    #[test]
    fn per_grant_mode_sentences_are_exact() {
        let mut enforce_grant = sample_grant("g", &[Capability::Read]);
        enforce_grant.mode = Some(EffectiveMode::Enforce);
        assert!(grant_line(&enforce_grant).ends_with(
            "This grant always enforces: its denials block even when the policy mode is observe."
        ));

        let mut observe_grant = sample_grant("g", &[Capability::Read]);
        observe_grant.mode = Some(EffectiveMode::Observe);
        assert!(grant_line(&observe_grant).ends_with(
            "This grant is always observe-only: its denials are recorded, never blocked."
        ));

        let no_mode_grant = sample_grant("g", &[Capability::Read]);
        let line = grant_line(&no_mode_grant);
        assert!(!line.contains("This grant"));
    }

    #[test]
    fn mode_line_and_suffixes() {
        let with_mode = sample_manifest(Some(EffectiveMode::Enforce), vec![]);
        assert_eq!(
            mode_block(
                resolve_manifest_mode(&with_mode).0,
                resolve_manifest_mode(&with_mode).1
            ),
            "Mode: enforce. Calls the grants below do not permit are blocked."
        );

        let mut mandatory = sample_manifest(None, vec![]);
        mandatory.config.push(ConfigEntry {
            key: "governance.mode".to_string(),
            value: serde_json::json!("observe"),
            level: Level::Mandatory,
        });
        let (mode, suffix) = resolve_manifest_mode(&mandatory);
        assert_eq!(mode, EffectiveMode::Observe);
        assert_eq!(suffix, " This mode is locked by the policy.");

        let mut recommended = sample_manifest(None, vec![]);
        recommended.config.push(ConfigEntry {
            key: "governance.mode".to_string(),
            value: serde_json::json!("enforce"),
            level: Level::Recommended,
        });
        let (_, suffix) = resolve_manifest_mode(&recommended);
        assert_eq!(suffix, " This mode is a default the user may change.");

        let none = sample_manifest(None, vec![]);
        let (mode, suffix) = resolve_manifest_mode(&none);
        assert_eq!(mode, EffectiveMode::Enforce);
        assert_eq!(
            suffix,
            " This policy sets no mode; the built-in default applies."
        );

        let observe_line = mode_block(EffectiveMode::Observe, "");
        assert!(observe_line.ends_with("Observation is not protection."));
    }

    #[test]
    fn empty_grants_renderings_are_exact() {
        assert_eq!(
            grants_block(&[], EffectiveMode::Enforce),
            "Where agents may read and write: nowhere. This policy grants no domains.\n\
             Every tool call on every page is denied."
        );
        assert_eq!(
            grants_block(&[], EffectiveMode::Observe),
            "Where agents may read and write: nowhere. This policy grants no domains.\n\
             Every tool call would be denied; in this mode those denials are recorded, not \
             blocked."
        );
    }

    #[test]
    fn no_identity_line_is_exact() {
        assert_eq!(
            identity_block(&None),
            "No identity block: this policy does not name a principal."
        );
    }

    #[test]
    fn settings_block_cases() {
        assert_eq!(
            settings_block(&[]),
            "This policy locks no settings and sets no defaults; users keep control of every \
             setting."
        );

        let mandatory_only = vec![ConfigEntry {
            key: "audit.enabled".to_string(),
            value: serde_json::json!(true),
            level: Level::Mandatory,
        }];
        let out = settings_block(&mandatory_only);
        assert!(out.starts_with("Settings locked by the organization (users cannot change these):"));
        assert!(out.contains("All other settings keep their user, preset, or built-in values."));
        assert!(out.contains("the locked entries above become user-level defaults"));

        let recommended_only = vec![ConfigEntry {
            key: "content.security.secrets.redact".to_string(),
            value: serde_json::json!(true),
            level: Level::Recommended,
        }];
        let out = settings_block(&recommended_only);
        assert!(out.starts_with("Org-recommended defaults (users may change these):"));
        assert!(!out.contains("the locked entries above become user-level defaults"));

        let both = vec![
            ConfigEntry {
                key: "audit.enabled".to_string(),
                value: serde_json::json!(true),
                level: Level::Mandatory,
            },
            ConfigEntry {
                key: "content.security.secrets.redact".to_string(),
                value: serde_json::json!(true),
                level: Level::Recommended,
            },
        ];
        let out = settings_block(&both);
        assert!(out.contains("Settings locked by the organization"));
        assert!(out.contains("Org-recommended defaults"));
    }

    #[test]
    fn denial_block_is_exact() {
        assert_eq!(
            denial_block(EffectiveMode::Enforce),
            "On a denial the agent receives a plain-text message with a stable denial id, in \
             the form 'Denied (D-xxxxxxxx): ...'. Hand that id to the policy administrator: it \
             identifies the exact rule and policy version that produced the denial."
        );
        assert_eq!(
            denial_block(EffectiveMode::Observe),
            "On a would-deny the agent sees the ordinary tool result and no denial text. The \
             denial id appears only in the audit record, as decision 'shadow_deny'."
        );
    }

    #[test]
    fn determinism_and_line_endings() {
        let json = r#"{"schema":3,"name":"a","version":"1","grants":[
            {"id":"g1","hosts":{"allow":["example.com"]},"allowed":["read"]}
        ]}"#;
        let m1 = parse_manifest(json, "t", always_valid_pattern).unwrap();
        let m2 = parse_manifest(json, "t", always_valid_pattern).unwrap();
        let out1 = explain_manifest(&m1, &m1.hash);
        let out2 = explain_manifest(&m2, &m2.hash);
        assert_eq!(out1, out2);
        assert!(out1.ends_with('\n'));
        assert!(
            !out1[..out1.len() - 1].ends_with('\n'),
            "exactly one trailing newline"
        );
        assert!(!out1.contains('\r'));
    }

    #[test]
    fn user_config_file_renders_settings_and_warnings() {
        let root: serde_json::Value = serde_json::from_str(
            r#"{"preset":"safe","config":{"audit.enabled":true,"bogus.key":1,"engine.connection.first_call_wait_ms":"nope"}}"#,
        )
        .unwrap();
        let (file, warnings) = parse_user_config_file(&root, always_valid_pattern);
        assert_eq!(file.preset.as_deref(), Some("safe"));
        assert_eq!(
            file.values.get("audit.enabled"),
            Some(&serde_json::json!(true))
        );
        assert!(warnings
            .iter()
            .any(|w| w == "unknown key 'bogus.key' is ignored."));
        assert!(warnings.iter().any(|w| w
            .starts_with("invalid value for 'engine.connection.first_call_wait_ms' is ignored (")));

        let rendered = explain_user_config(&file, &warnings);
        assert!(rendered
            .starts_with("User configuration file (not a policy manifest).\n\nPreset: safe."));
        assert!(
            rendered.contains("Nothing here is locked: only an org policy file can lock settings.")
        );
        assert!(rendered.contains("Warnings:"));
    }

    #[test]
    fn unknown_preset_warns_and_renders_as_none() {
        let root: serde_json::Value = serde_json::from_str(r#"{"preset":"nope"}"#).unwrap();
        let (file, warnings) = parse_user_config_file(&root, always_valid_pattern);
        assert_eq!(file.preset, None);
        assert_eq!(
            warnings,
            vec!["unknown preset 'nope'; the built-in defaults apply.".to_string()]
        );
        let rendered = explain_user_config(&file, &warnings);
        assert!(rendered.contains("Preset: none (the built-in defaults apply)."));
    }

    #[test]
    fn empty_user_config_renders_the_none_line() {
        let file = UserConfigFile::default();
        let rendered = explain_user_config(&file, &[]);
        assert!(rendered.contains("User settings: none."));
    }
}
