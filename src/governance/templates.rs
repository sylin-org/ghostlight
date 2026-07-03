//! `ghostlight policy init --template <name>` (G18, ADR-0020 consequence: manifest templates
//! ride the same schema as `policy explain`/`policy simulate`). Writes one of three embedded
//! example manifests as an org admin's starting point. The embedded bytes and the committed
//! `examples/*.json` files are identical by construction (`include_str!`), so what ships is
//! exactly what is reviewed in the repository -- no re-serialization, no mutation on write.

/// The three valid template names, in the order printed in error messages.
pub const TEMPLATE_NAMES: [&str; 3] = [
    "enterprise-healthcare",
    "developer-unrestricted",
    "qa-staging",
];

const ENTERPRISE_HEALTHCARE: &str = include_str!("../../examples/enterprise-healthcare.json");
const DEVELOPER_UNRESTRICTED: &str = include_str!("../../examples/developer-unrestricted.json");
const QA_STAGING: &str = include_str!("../../examples/qa-staging.json");

/// Look up an embedded template's raw bytes by name. `None` for an unknown name.
pub fn template_bytes(name: &str) -> Option<&'static str> {
    match name {
        "enterprise-healthcare" => Some(ENTERPRISE_HEALTHCARE),
        "developer-unrestricted" => Some(DEVELOPER_UNRESTRICTED),
        "qa-staging" => Some(QA_STAGING),
        _ => None,
    }
}

/// Why [`run_init`] could not write the requested template.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error(
        "unknown template '{name}': valid templates are enterprise-healthcare, \
         developer-unrestricted, qa-staging"
    )]
    UnknownTemplate { name: String },
    #[error("{path} already exists; rerun with --force to overwrite it")]
    AlreadyExists { path: String },
    #[error("failed to write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// What [`run_init`] wrote, for [`render_orientation`].
#[derive(Debug)]
pub struct InitOutcome {
    pub path: std::path::PathBuf,
    pub template_name: String,
}

/// Write the named template's embedded bytes, exactly, to `out`. Refuses to overwrite an
/// existing file unless `force` is set; writes nothing on any error.
pub fn run_init(name: &str, out: &std::path::Path, force: bool) -> Result<InitOutcome, InitError> {
    let bytes = template_bytes(name).ok_or_else(|| InitError::UnknownTemplate {
        name: name.to_string(),
    })?;
    if out.exists() && !force {
        return Err(InitError::AlreadyExists {
            path: out.display().to_string(),
        });
    }
    std::fs::write(out, bytes).map_err(|e| InitError::Io {
        path: out.display().to_string(),
        source: e,
    })?;
    Ok(InitOutcome {
        path: out.to_path_buf(),
        template_name: name.to_string(),
    })
}

/// Render the orientation block printed after a successful `policy init` (Required behavior
/// section 3, exact text).
pub fn render_orientation(outcome: &InitOutcome) -> String {
    format!(
        "Wrote {} (template '{}').\n\
         \n\
         This file is a starting point. Edit the grants and config entries for your\n\
         organization, then deploy it with your management channel (GPO, Intune, Jamf)\n\
         to the org policy path for each platform:\n\
         \n\
         \x20 Windows  %ProgramData%\\ghostlight\\policy.json\n\
         \x20 macOS    /Library/Application Support/ghostlight/policy.json\n\
         \x20 Linux    /etc/ghostlight/policy.json\n\
         \n\
         For personal use, load it instead with:\n\
         \x20 ghostlight --manifest file:///absolute/path/to/policy.json\n\
         \n\
         Manifests are strict JSON (no comments). Grant \"description\" fields carry the\n\
         explanatory text.\n",
        outcome.path.display(),
        outcome.template_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only domain-pattern validator, mirroring the browser plugin's grammar exactly
    /// enough to exercise these three specific templates (they only ever declare an exact host
    /// or a single leading `*.` wildcard). The browser plugin owns the authoritative
    /// implementation and its exhaustive test list; this module cannot depend on it (the a7
    /// arch-test forbids a `governance -> browser` edge, even in test code, which is scanned as
    /// raw text).
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
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    }

    #[test]
    fn unknown_template_name_lists_the_three_valid_names() {
        assert!(template_bytes("bogus").is_none());
        let err = InitError::UnknownTemplate {
            name: "bogus".to_string(),
        };
        let msg = err.to_string();
        for name in TEMPLATE_NAMES {
            assert!(msg.contains(name), "{msg} missing {name}");
        }
    }

    #[test]
    fn every_template_name_resolves_to_non_empty_bytes() {
        for name in TEMPLATE_NAMES {
            let bytes = template_bytes(name).unwrap_or_else(|| panic!("{name} not found"));
            assert!(!bytes.is_empty(), "{name}");
        }
    }

    /// Required test 7: every embedded template validates through the real manifest
    /// loader/validator.
    #[test]
    fn every_embedded_template_validates_through_the_real_manifest_parser() {
        for name in TEMPLATE_NAMES {
            let bytes = template_bytes(name).unwrap();
            crate::governance::manifest::document::parse_manifest(
                bytes,
                name,
                test_domain_pattern_valid,
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    /// Required test 7's grant-order pin: qa-staging's first two grant ids, in order.
    #[test]
    fn qa_staging_grant_order_is_pinned() {
        let manifest = crate::governance::manifest::document::parse_manifest(
            QA_STAGING,
            "qa-staging",
            test_domain_pattern_valid,
        )
        .unwrap();
        let ids: Vec<&str> = manifest.grants.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(&ids[..2], &["staging", "production-readonly"]);
    }

    /// Each template's declared `name` field agrees with its lookup key and its `examples/`
    /// file stem (constraint 13).
    #[test]
    fn template_name_fields_agree_with_their_lookup_names() {
        for name in TEMPLATE_NAMES {
            let bytes = template_bytes(name).unwrap();
            let value: serde_json::Value = serde_json::from_str(bytes).unwrap();
            assert_eq!(value["name"], name);
        }
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ghostlight-g18-templates-{}-{}",
            std::process::id(),
            tag
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn run_init_writes_the_embedded_bytes_exactly() {
        let dir = temp_dir("write");
        let out = dir.join("policy.json");
        let outcome = run_init("qa-staging", &out, false).unwrap();
        assert_eq!(outcome.path, out);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), QA_STAGING);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_init_refuses_to_overwrite_without_force() {
        let dir = temp_dir("noforce");
        let out = dir.join("policy.json");
        std::fs::write(&out, "existing").unwrap();
        let err = run_init("qa-staging", &out, false).unwrap_err();
        assert!(matches!(err, InitError::AlreadyExists { .. }));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "existing");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_init_overwrites_with_force() {
        let dir = temp_dir("force");
        let out = dir.join("policy.json");
        std::fs::write(&out, "existing").unwrap();
        run_init("qa-staging", &out, true).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), QA_STAGING);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_init_unknown_template_writes_nothing() {
        let dir = temp_dir("bogus");
        let out = dir.join("policy.json");
        let err = run_init("bogus", &out, false).unwrap_err();
        assert!(matches!(err, InitError::UnknownTemplate { .. }));
        assert!(!out.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_orientation_matches_the_exact_required_text() {
        let outcome = InitOutcome {
            path: std::path::PathBuf::from("/tmp/policy.json"),
            template_name: "qa-staging".to_string(),
        };
        let expected = "Wrote /tmp/policy.json (template 'qa-staging').\n\
\n\
This file is a starting point. Edit the grants and config entries for your\n\
organization, then deploy it with your management channel (GPO, Intune, Jamf)\n\
to the org policy path for each platform:\n\
\n\
\x20 Windows  %ProgramData%\\ghostlight\\policy.json\n\
\x20 macOS    /Library/Application Support/ghostlight/policy.json\n\
\x20 Linux    /etc/ghostlight/policy.json\n\
\n\
For personal use, load it instead with:\n\
\x20 ghostlight --manifest file:///absolute/path/to/policy.json\n\
\n\
Manifests are strict JSON (no comments). Grant \"description\" fields carry the\n\
explanatory text.\n";
        assert_eq!(render_orientation(&outcome), expected);
    }
}
