//! Carry a schema-3 policy document to schema 4 where its meaning is kept.
//!
//! Schema 4 is schema 3 without observe mode, the channel switches, the user-policy switch and the
//! frame options. A document moves only when every part schema 4 drops says what schema 4 does
//! anyway: `enforce`, a channel set to `true`, the user switch (the user layer only tightens), and
//! a frame option at its default. Anything else would change what the policy does, so the
//! document stays as it is and each such part gets one sentence.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use thiserror::Error;

use super::manifest::{self, ManifestError, RETIRED_SCHEMA, SCHEMA};
use crate::install::record::{atomic_replace, atomic_replace_keeping_access};
use crate::language::migration::{self, Retired, RETIRED_SETTINGS};

/// File-name suffix of the copy kept beside a migrated file.
const BACKUP_SUFFIX: &str = ".schema3.bak";

/// The key schema 3 gave observe mode, on a policy and on each grant.
const MODE_KEY: &str = "mode";

/// The only `mode` value schema 4 keeps the meaning of.
const MODE_ENFORCE: &str = "enforce";

/// A document moved to schema 4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Migration {
    /// The schema-4 document, ready to write.
    pub document: String,
    /// One sentence per part removed.
    pub removed: Vec<String>,
}

/// What a migration found in a document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The document already speaks schema 4.
    Current,
    /// The document moved.
    Migrated(Migration),
}

/// Why a document did not move.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MigrateError {
    /// The text is not a policy this build can read at all.
    #[error("{0}")]
    Unreadable(String),
    /// Moving the document would change what it means. One sentence per part.
    #[error("{}", .0.join("\n"))]
    Refused(Vec<String>),
    /// The file or its backup could not be written.
    #[error("{0}")]
    Unwritable(String),
}

/// Move one document's text to schema 4, or say why it cannot move.
pub fn migrate(text: &str, origin: &str) -> Result<Outcome, MigrateError> {
    let mut value = manifest::read_value(text, origin).map_err(unreadable)?;
    match value.get("schema").and_then(Value::as_u64) {
        Some(SCHEMA) => {
            manifest::parse(text, origin).map_err(unreadable)?;
            return Ok(Outcome::Current);
        }
        Some(RETIRED_SCHEMA) => {}
        _ => {
            return Err(unreadable(ManifestError::UnsupportedSchema {
                origin: origin.into(),
                found: value
                    .get("schema")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "<missing>".into()),
            }))
        }
    }
    let mut plan = Plan::default();
    if let Some(document) = value.as_object_mut() {
        plan.document(document);
    }
    if !plan.refused.is_empty() {
        return Err(MigrateError::Refused(plan.refused));
    }
    let carried = manifest::parse(&value.to_string(), origin)
        .map_err(|error| MigrateError::Refused(vec![error.to_string()]))?;
    let mut document = serde_json::to_string_pretty(&carried)
        .map_err(|error| MigrateError::Unreadable(error.to_string()))?;
    document.push('\n');
    Ok(Outcome::Migrated(Migration {
        document,
        removed: plan.removed,
    }))
}

/// Move one policy file to schema 4 in place.
///
/// The original is copied to `<file>.schema3.bak` first, and the new document replaces the file in
/// one step, so a failure leaves the original where it was. The backup is private to the user; the
/// migrated file keeps the access the original had, because the file may be one other accounts
/// read. Returns the backup's path and what
/// moved, or `None` when the file already speaks schema 4.
pub fn migrate_file(path: &Path) -> Result<Option<(PathBuf, Migration)>, MigrateError> {
    let original = fs::read(path).map_err(|error| {
        MigrateError::Unreadable(format!("read policy {}: {error}", path.display()))
    })?;
    let text = String::from_utf8(original.clone())
        .map_err(|_| MigrateError::Unreadable(format!("{} is not UTF-8 text.", path.display())))?;
    let migration = match migrate(&text, &path.display().to_string())? {
        Outcome::Current => return Ok(None),
        Outcome::Migrated(migration) => migration,
    };
    let backup = backup_path(path);
    match fs::read(&backup) {
        Ok(existing) if existing == original => {}
        Ok(_) => {
            return Err(MigrateError::Unwritable(migration::backup_in_the_way(
                &backup,
            )))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            atomic_replace(&backup, &original).map_err(unwritable)?;
        }
        Err(error) => return Err(unwritable(error)),
    }
    atomic_replace_keeping_access(path, migration.document.as_bytes()).map_err(unwritable)?;
    Ok(Some((backup, migration)))
}

/// The backup path beside a policy file.
#[must_use]
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(BACKUP_SUFFIX);
    path.with_file_name(name)
}

fn unreadable(error: ManifestError) -> MigrateError {
    MigrateError::Unreadable(error.to_string())
}

fn unwritable(error: io::Error) -> MigrateError {
    MigrateError::Unwritable(error.to_string())
}

/// What the walk over one document decided.
#[derive(Default)]
struct Plan {
    removed: Vec<String>,
    refused: Vec<String>,
}

impl Plan {
    fn document(&mut self, document: &mut Map<String, Value>) {
        document.insert("schema".into(), Value::from(SCHEMA));
        self.mode(document, MODE_KEY);
        if let Some(Value::Array(grants)) = document.get_mut("grants") {
            for (index, grant) in grants.iter_mut().enumerate() {
                if let Value::Object(grant) = grant {
                    self.mode(grant, &format!("grants[{index}].{MODE_KEY}"));
                }
            }
        }
        let emptied = match document.get_mut("config") {
            Some(Value::Array(entries)) if !entries.is_empty() => {
                entries.retain(|entry| !self.setting(entry));
                entries.is_empty()
            }
            _ => false,
        };
        if emptied {
            document.remove("config");
        }
    }

    /// Drop a `mode` that says enforce; refuse any other.
    fn mode(&mut self, owner: &mut Map<String, Value>, path: &str) {
        let Some(value) = owner.get(MODE_KEY) else {
            return;
        };
        if value.as_str() == Some(MODE_ENFORCE) {
            self.removed.push(migration::removed(Retired::Mode, path));
            owner.remove(MODE_KEY);
        } else {
            self.refused
                .push(migration::refused(Retired::Mode, path, value));
        }
    }

    /// Whether one `config` entry is removed. A refused entry stays, so the result is never used.
    fn setting(&mut self, entry: &Value) -> bool {
        let Some(key) = entry.get("key").and_then(Value::as_str) else {
            return false;
        };
        let Some(retired) = RETIRED_SETTINGS
            .into_iter()
            .find(|retired| retired.config_key() == Some(key))
        else {
            return false;
        };
        let value = entry.get("value").unwrap_or(&Value::Null);
        let keeps_meaning = match retired {
            Retired::Channel(_) => value == &Value::Bool(true),
            Retired::UserPolicySwitch => true,
            Retired::Frame(_, fixed) => value.as_str() == Some(fixed),
            Retired::Mode => false,
        };
        if keeps_meaning {
            self.removed.push(migration::removed(retired, key));
        } else {
            self.refused.push(migration::refused(retired, key, value));
        }
        keeps_meaning
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn temporary(name: &str) -> Scratch {
        Scratch::file(&format!("migrate-{name}.json"))
    }

    fn moved(text: &str) -> Migration {
        match migrate(text, "policy.json").expect("the document moves") {
            Outcome::Migrated(migration) => migration,
            Outcome::Current => panic!("a schema-3 document is never current"),
        }
    }

    fn refused(text: &str) -> Vec<String> {
        match migrate(text, "policy.json").expect_err("the document stays") {
            MigrateError::Refused(reasons) => reasons,
            other => panic!("expected a refusal, got {other}"),
        }
    }

    const GRANTS: &str =
        r#""grants":[{"id":"g","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#;

    #[test]
    fn every_shape_that_keeps_its_meaning_moves() {
        let text = format!(
            r#"{{"schema":3,"name":"mine","version":"2","mode":"enforce",{GRANTS},"config":[
                {{"key":"channels.mcp.enabled","value":true,"level":"mandatory"}},
                {{"key":"channels.cli.enabled","value":true,"level":"recommended"}},
                {{"key":"policy.user.enabled","value":false,"level":"mandatory"}},
                {{"key":"content.frames.handling","value":"permitted_content","level":"mandatory"}},
                {{"key":"content.frames.notice","value":"when_affected","level":"mandatory"}},
                {{"key":"browser.startup","value":"manual","level":"mandatory"}}]}}"#
        );
        let migration = moved(&text);
        assert_eq!(migration.removed.len(), 6);
        let carried = manifest::parse(&migration.document, "moved").unwrap();
        assert_eq!(carried.schema, 4);
        assert_eq!(carried.name, "mine");
        assert_eq!(carried.version, "2");
        assert_eq!(carried.grants.len(), 1);
        assert_eq!(
            carried.browser_startup(),
            Some(manifest::BrowserStartup::Manual)
        );
        assert_eq!(carried.config.len(), 1);
        assert!(migration.document.ends_with("}\n"));
    }

    #[test]
    fn a_grant_that_enforces_and_an_emptied_config_leave_a_plain_document() {
        let migration = moved(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"g","mode":"enforce","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"channels.cli.enabled","value":true,"level":"mandatory"}]}"#,
        );
        assert!(!migration.document.contains("config"));
        assert!(!migration.document.contains("mode"));
        assert!(migration
            .removed
            .iter()
            .any(|line| line.contains("grants[0].mode")));
        let carried = manifest::parse(&migration.document, "moved").unwrap();
        assert!(carried.config.is_empty());
    }

    #[test]
    fn the_organization_and_identity_blocks_survive() {
        let migration = moved(
            r#"{"schema":3,"name":"org","version":"1","identity":{"principal":"Ada"},"organization":{"name":"Example","contacts":[{"kind":"email","value":"a@example.com"}]},"grants":[]}"#,
        );
        let carried = manifest::parse(&migration.document, "moved").unwrap();
        assert_eq!(carried.identity.unwrap().principal.as_deref(), Some("Ada"));
        let organization = carried.organization.unwrap();
        assert_eq!(organization.name, "Example");
        assert_eq!(organization.contacts.len(), 1);
        assert!(!migration.document.contains("null"));
    }

    #[test]
    fn every_shape_that_would_change_meaning_is_refused_with_one_sentence_each() {
        let cases = [
            (r#""mode":"observe","grants":[]"#, "observe mode"),
            (
                r#""grants":[{"id":"g","mode":"observe","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
                "grants[0].mode",
            ),
            (
                r#""grants":[],"config":[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
                "channels.cli.enabled is false",
            ),
            (
                r#""grants":[],"config":[{"key":"channels.mcp.enabled","value":"no","level":"mandatory"}]"#,
                "channels.mcp.enabled",
            ),
            (
                r#""grants":[],"config":[{"key":"content.frames.handling","value":"complete_page","level":"mandatory"}]"#,
                "content.frames.handling",
            ),
            (
                r#""grants":[],"config":[{"key":"content.frames.notice","value":"on_demand","level":"mandatory"}]"#,
                "content.frames.notice",
            ),
        ];
        for (body, expected) in cases {
            let text = format!(r#"{{"schema":3,"name":"mine","version":"1",{body}}}"#);
            let reasons = refused(&text);
            assert_eq!(reasons.len(), 1, "{body}: {reasons:?}");
            assert!(reasons[0].contains(expected), "{body}: {reasons:?}");
        }
        let both = refused(
            r#"{"schema":3,"name":"mine","version":"1","mode":"observe","grants":[],"config":[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]}"#,
        );
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn content_schema_four_would_refuse_is_refused_not_carried() {
        let reasons = refused(
            r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"g","hosts":{"allow":["not a host"]},"allowed":["read"]}]}"#,
        );
        assert_eq!(reasons.len(), 1);
        assert!(
            reasons[0].contains("grants[0].hosts.allow[0]"),
            "{reasons:?}"
        );
        let unknown = refused(
            r#"{"schema":3,"name":"mine","version":"1","grants":[],"config":[{"key":"browser.unheard","value":true,"level":"mandatory"}]}"#,
        );
        assert!(
            unknown[0].contains("not a registered policy setting"),
            "{unknown:?}"
        );
    }

    #[test]
    fn current_and_unreadable_documents_are_told_apart() {
        assert_eq!(
            migrate(r#"{"schema":4,"name":"n","version":"1","grants":[]}"#, "p").unwrap(),
            Outcome::Current
        );
        for text in [
            r#"{"schema":2,"name":"n","version":"1","grants":[]}"#,
            r#"{"name":"n"}"#,
            "{half",
        ] {
            assert!(matches!(
                migrate(text, "p").unwrap_err(),
                MigrateError::Unreadable(_)
            ));
        }
        assert!(matches!(
            migrate(r#"{"schema":4,"name":"","version":"1","grants":[]}"#, "p").unwrap_err(),
            MigrateError::Unreadable(_)
        ));
    }

    #[test]
    fn a_file_moves_with_a_backup_and_a_second_run_changes_nothing() {
        let path = temporary("moves");
        let original =
            format!(r#"{{"schema":3,"name":"mine","version":"1","mode":"enforce",{GRANTS}}}"#);
        fs::write(&path, &original).unwrap();
        let (backup, migration) = migrate_file(&path).unwrap().expect("the file moves");
        assert_eq!(backup, backup_path(&path));
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(fs::read_to_string(&path).unwrap(), migration.document);
        assert!(manifest::parse(&migration.document, "moved").is_ok());
        assert_eq!(migrate_file(&path).unwrap(), None);
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn a_migrated_file_keeps_its_access_and_its_backup_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = temporary("access");
        let original = format!(r#"{{"schema":3,"name":"mine","version":"1",{GRANTS}}}"#);
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let (backup, _) = migrate_file(&path).unwrap().expect("the file moves");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o644);
        assert_eq!(mode(&backup), 0o600);
    }

    #[test]
    fn a_refused_file_is_left_untouched_with_no_backup() {
        let path = temporary("refused");
        let original = r#"{"schema":3,"name":"mine","version":"1","mode":"observe","grants":[]}"#;
        fs::write(&path, original).unwrap();
        assert!(matches!(
            migrate_file(&path).unwrap_err(),
            MigrateError::Refused(_)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert!(!backup_path(&path).exists());
    }

    #[test]
    fn a_backup_holding_something_else_stops_the_move_before_anything_changes() {
        let path = temporary("in-the-way");
        let original = format!(r#"{{"schema":3,"name":"mine","version":"1",{GRANTS}}}"#);
        fs::write(&path, &original).unwrap();
        let backup = backup_path(&path);
        fs::write(&backup, "an earlier copy").unwrap();
        assert!(matches!(
            migrate_file(&path).unwrap_err(),
            MigrateError::Unwritable(_)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(fs::read_to_string(&backup).unwrap(), "an earlier copy");
    }
}
