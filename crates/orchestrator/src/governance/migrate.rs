//! Carry a schema-3 policy document to schema 4 where its meaning is kept.
//!
//! Schema 4 is schema 3 without observe mode, the user-policy switch and the frame options. Its
//! channel switches carry over as they are (ADR-0202 Decision 22a). A document moves only when
//! every part schema 4 drops says what schema 4 does anyway: `enforce`, the user switch (the user
//! layer only tightens), and a frame option at its default. Anything else would change what the
//! policy does, so the document stays as it is and each such part gets one sentence.

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

/// Who may read a policy file once it has moved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    /// Ghostlight owns the file, so the moved file is readable only by the user.
    OwnerOnly,
    /// Someone else named the file, and other accounts may read it, so the moved file keeps the
    /// access the original had.
    Kept,
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
/// migrated file gets `access`. Returns the backup's path and what moved, or `None` when the file
/// already speaks schema 4.
pub fn migrate_file(
    path: &Path,
    access: Access,
) -> Result<Option<(PathBuf, Migration)>, MigrateError> {
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
    let document = migration.document.as_bytes();
    match access {
        Access::OwnerOnly => atomic_replace(path, document),
        Access::Kept => atomic_replace_keeping_access(path, document),
    }
    .map_err(unwritable)?;
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
    use serde_json::json;

    use super::*;
    use crate::install::record::assert_owner_only;
    use crate::scratch::Scratch;

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

    /// A schema-3 document with `body` after its name and version.
    fn schema_3(body: &str) -> String {
        format!(r#"{{"schema":3,"name":"mine","version":"2",{body}}}"#)
    }

    const GRANTS: &str =
        r#""grants":[{"id":"g","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#;

    /// The value schema 4 fixes a retired setting at, or any value for the user switch, whose
    /// meaning schema 4 keeps whatever it said.
    fn kept_meaning(retired: Retired) -> serde_json::Value {
        match retired {
            Retired::Frame(_, fixed) => json!(fixed),
            Retired::UserPolicySwitch | Retired::Mode => json!(false),
        }
    }

    /// ACCEPTANCE Governance 4 and ADR-0202 Decision 22a: a schema-3 document moves to schema 4
    /// only when every part schema 4 drops already said what schema 4 does (enforce, the user
    /// switch, a frame option at its fixed value), carrying its grants, channel switches,
    /// settings, identity and organization. Otherwise it stays, with one sentence for each part
    /// that would change meaning or that schema 4 would refuse.
    #[test]
    fn a_schema_3_document_moves_only_where_its_meaning_is_kept() {
        let mcp = manifest::PolicySetting::McpChannelEnabled;
        let cli = manifest::PolicySetting::CliChannelEnabled;
        let startup = manifest::PolicySetting::BrowserStartup;
        let mut config = vec![
            json!({"key": mcp.key(), "value": false, "level": "mandatory"}),
            json!({"key": cli.key(), "value": true, "level": "recommended"}),
        ];
        for retired in RETIRED_SETTINGS {
            config.push(json!({
                "key": retired.config_key(),
                "value": kept_meaning(retired),
                "level": "mandatory"
            }));
        }
        config.push(json!({"key": startup.key(), "value": "manual", "level": "mandatory"}));
        let text = schema_3(&format!(
            r#""mode":"{MODE_ENFORCE}",{GRANTS},"identity":{{"principal":"Ada"}},"organization":{{"name":"Example","contacts":[{{"kind":"email","value":"a@example.com"}}]}},"config":{}"#,
            serde_json::Value::from(config)
        ));
        let migration = moved(&text);
        let carried = manifest::parse(&migration.document, "moved").unwrap();
        let grants = manifest::parse(
            &format!(r#"{{"schema":4,"name":"g","version":"1",{GRANTS}}}"#),
            "grants",
        )
        .unwrap()
        .grants;
        assert_eq!(carried.schema, u32::try_from(SCHEMA).unwrap());
        assert_eq!(carried.name, "mine");
        assert_eq!(carried.version, "2");
        assert_eq!(carried.grants, grants);
        assert_eq!(carried.boolean_setting(mcp), Some(false));
        assert_eq!(carried.boolean_setting(cli), Some(true));
        assert_eq!(carried.config[1].level, manifest::SettingLevel::Recommended);
        assert_eq!(
            carried.browser_startup(),
            Some(manifest::BrowserStartup::Manual)
        );
        let keys: Vec<_> = carried
            .config
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert_eq!(keys, [mcp.key(), cli.key(), startup.key()]);
        assert_eq!(carried.identity.unwrap().principal.as_deref(), Some("Ada"));
        let organization = carried.organization.unwrap();
        assert_eq!(organization.name, "Example");
        assert_eq!(organization.contacts[0].value, "a@example.com");
        let mut expected = vec![migration::removed(Retired::Mode, MODE_KEY)];
        expected.extend(RETIRED_SETTINGS.map(|retired| {
            migration::removed(retired, retired.config_key().expect("a config key"))
        }));
        assert_eq!(migration.removed, expected);
        assert!(migration.document.ends_with('\n'));

        let grant_mode = moved(&schema_3(&format!(
            r#""grants":[{{"id":"g","{MODE_KEY}":"{MODE_ENFORCE}","hosts":{{"allow":["*"]}},"allowed":["read"]}}]"#
        )));
        let grant_path = format!("grants[0].{MODE_KEY}");
        assert_eq!(
            grant_mode.removed,
            [migration::removed(Retired::Mode, &grant_path)]
        );
        assert!(manifest::parse(&grant_mode.document, "moved").is_ok());

        let [_, handling, notice] = RETIRED_SETTINGS;
        let setting = |retired: Retired, value: &str| {
            format!(
                r#"{{"key":"{}","value":"{value}","level":"mandatory"}}"#,
                retired.config_key().expect("a config key")
            )
        };
        let observe = json!("observe");
        for (body, expected) in [
            (
                format!(r#""{MODE_KEY}":"observe","grants":[]"#),
                vec![migration::refused(Retired::Mode, MODE_KEY, &observe)],
            ),
            (
                format!(
                    r#""grants":[{{"id":"g","{MODE_KEY}":"observe","hosts":{{"allow":["*"]}},"allowed":["read"]}}]"#
                ),
                vec![migration::refused(Retired::Mode, &grant_path, &observe)],
            ),
            (
                format!(
                    r#""grants":[],"config":[{}]"#,
                    setting(handling, "complete_page")
                ),
                vec![migration::refused(
                    handling,
                    handling.config_key().unwrap(),
                    &json!("complete_page"),
                )],
            ),
            (
                format!(r#""grants":[],"config":[{}]"#, setting(notice, "on_demand")),
                vec![migration::refused(
                    notice,
                    notice.config_key().unwrap(),
                    &json!("on_demand"),
                )],
            ),
            (
                format!(
                    r#""{MODE_KEY}":"observe","grants":[],"config":[{}]"#,
                    setting(notice, "on_demand")
                ),
                vec![
                    migration::refused(Retired::Mode, MODE_KEY, &observe),
                    migration::refused(notice, notice.config_key().unwrap(), &json!("on_demand")),
                ],
            ),
        ] {
            assert_eq!(refused(&schema_3(&body)), expected, "{body}");
        }

        // Content schema 4 itself would refuse is refused, never carried, naming where it is.
        for (body, path) in [
            (
                r#""grants":[{"id":"g","hosts":{"allow":["not a host"]},"allowed":["read"]}]"#,
                "grants[0].hosts.allow[0]",
            ),
            (
                r#""grants":[],"config":[{"key":"browser.unheard","value":true,"level":"mandatory"}]"#,
                "config[0].key",
            ),
            (
                r#""grants":[],"config":[{"key":"channels.mcp.enabled","value":"no","level":"mandatory"}]"#,
                "config[0].value",
            ),
        ] {
            let reasons = refused(&schema_3(body));
            assert_eq!(reasons.len(), 1, "{body}: {reasons:?}");
            assert!(reasons[0].contains(path), "{body}: {reasons:?}");
        }
    }

    /// Whether an error is the refusal a row expects.
    type Refusal = fn(&MigrateError) -> bool;

    /// ACCEPTANCE Governance 4 and fafb0cb5: policy migrate moves a named schema-3 file in place.
    /// The original is kept beside it as <file>.schema3.bak, readable only by the user, the moved
    /// file keeps the access it had, and a second run finds it current and changes nothing. A file
    /// it cannot move stays byte for byte, with no backup written and an existing one untouched:
    /// one whose meaning would change, one this build cannot read (never reported as current), or
    /// one whose backup path already holds something else.
    #[test]
    fn policy_migrate_changes_a_file_only_by_one_whole_move_with_a_private_backup() {
        const EARLIER_COPY: &str = "an earlier copy";
        let path = Scratch::file("migrate-moves.json");
        let original = schema_3(&format!(r#""mode":"{MODE_ENFORCE}",{GRANTS}"#));
        fs::write(&path, &original).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }
        let (backup, migration) = migrate_file(&path, Access::Kept)
            .unwrap()
            .expect("the file moves");
        assert_eq!(
            backup,
            PathBuf::from(format!("{}{BACKUP_SUFFIX}", path.display()))
        );
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(fs::read_to_string(&path).unwrap(), migration.document);
        assert!(manifest::parse(&migration.document, "moved").is_ok());
        assert_owner_only(&backup);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o644,
                "the moved file keeps the access it had"
            );
        }
        assert_eq!(migrate_file(&path, Access::Kept).unwrap(), None);
        assert_eq!(fs::read_to_string(&path).unwrap(), migration.document);
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);

        let refused_document = schema_3(r#""mode":"observe","grants":[]"#);
        let movable = schema_3(GRANTS);
        let rows: [(&str, &str, Option<&str>, Refusal); 6] = [
            ("meaning would change", &refused_document, None, |error| {
                matches!(error, MigrateError::Refused(_))
            }),
            ("not JSON", "{half", None, |error| {
                matches!(error, MigrateError::Unreadable(_))
            }),
            (
                "an invalid schema-4 document",
                r#"{"schema":4,"name":"","version":"1","grants":[]}"#,
                None,
                |error| matches!(error, MigrateError::Unreadable(_)),
            ),
            (
                "schema 2",
                r#"{"schema":2,"name":"n","version":"1","grants":[]}"#,
                None,
                |error| matches!(error, MigrateError::Unreadable(_)),
            ),
            ("no schema", r#"{"name":"n"}"#, None, |error| {
                matches!(error, MigrateError::Unreadable(_))
            }),
            (
                "a backup in the way",
                &movable,
                Some(EARLIER_COPY),
                |error| matches!(error, MigrateError::Unwritable(_)),
            ),
        ];
        for (row, text, earlier, refusal) in rows {
            let path = Scratch::file("migrate-stays.json");
            fs::write(&path, text).unwrap();
            let backup = backup_path(&path);
            if let Some(earlier) = earlier {
                fs::write(&backup, earlier).unwrap();
            }
            let error = migrate_file(&path, Access::Kept).unwrap_err();
            assert!(refusal(&error), "{row}: {error}");
            assert_eq!(fs::read_to_string(&path).unwrap(), text, "{row}");
            match earlier {
                Some(earlier) => {
                    assert_eq!(fs::read_to_string(&backup).unwrap(), earlier, "{row}");
                }
                None => assert!(!backup.exists(), "{row}: a backup was written"),
            }
        }
    }
}
