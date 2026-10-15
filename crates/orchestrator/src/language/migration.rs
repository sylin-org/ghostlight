//! The sentences `ghostlight policy migrate` and the automatic user-policy upgrade say about the
//! settings schema 4 no longer has.

use std::path::Path;

use serde_json::Value;

/// A part of a schema-3 document that schema 4 does not carry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Retired {
    /// A `mode` on the policy or on one of its grants.
    Mode,
    /// A channel switch in `config`, with the key it used.
    Channel(&'static str),
    /// The switch that stopped a person authoring their own policy.
    UserPolicySwitch,
    /// A frame option in `config`, with the key it used and the value schema 4 fixes it at.
    Frame(&'static str, &'static str),
}

impl Retired {
    /// The registered `config` key, when this part lived in `config`.
    #[must_use]
    pub const fn config_key(self) -> Option<&'static str> {
        match self {
            Self::Mode => None,
            Self::Channel(key) | Self::Frame(key, _) => Some(key),
            Self::UserPolicySwitch => Some("policy.user.enabled"),
        }
    }
}

/// Every retired `config` key, in the order the migration checks them.
pub const RETIRED_SETTINGS: [Retired; 5] = [
    Retired::Channel("channels.mcp.enabled"),
    Retired::Channel("channels.cli.enabled"),
    Retired::UserPolicySwitch,
    Retired::Frame("content.frames.handling", "permitted_content"),
    Retired::Frame("content.frames.notice", "when_affected"),
];

/// The sentence for a part the migration removed because its meaning is kept.
#[must_use]
pub fn removed(retired: Retired, path: &str) -> String {
    match retired {
        Retired::Mode => format!("Removed {path}: enforce is the only behavior in schema 4."),
        Retired::Channel(key) => {
            format!("Removed {key}: schema 4 has no channel switch, and true admitted the channel.")
        }
        Retired::UserPolicySwitch => "Removed policy.user.enabled: schema 4 has no switch for the user layer, and that layer can only tighten policy.".into(),
        Retired::Frame(key, fixed) => {
            format!("Removed {key}: schema 4 fixes it at {fixed}, the value this policy had.")
        }
    }
}

/// The sentence for a part the migration refuses to remove because its meaning would change.
#[must_use]
pub fn refused(retired: Retired, path: &str, value: &Value) -> String {
    match retired {
        Retired::Mode if value.as_str() == Some("observe") => format!(
            "{path} is {value}: schema 4 has no observe mode, so work this policy only reports would now be refused."
        ),
        Retired::Mode => format!("{path} is {value}: schema 3 accepted only enforce or observe."),
        Retired::Channel(key) => format!(
            "{key} is {value}: schema 4 has no channel switch, so that channel would now be admitted."
        ),
        Retired::UserPolicySwitch => format!(
            "policy.user.enabled is {value}: schema 4 has no switch for the user layer."
        ),
        Retired::Frame(key, fixed) => format!(
            "{key} is {value}: schema 4 fixes it at {fixed}, so what Ghostlight reads from a page would change."
        ),
    }
}

/// What a person reads when the file is already in the current schema.
#[must_use]
pub fn already_current(path: &Path) -> String {
    format!("{} is already schema 4. Nothing changed.", path.display())
}

/// What a person reads after a file moved to schema 4.
#[must_use]
pub fn migrated(path: &Path, backup: &Path, removed: &[String]) -> String {
    let mut lines = vec![format!(
        "Moved {} to schema 4. The original is kept as {}.",
        path.display(),
        backup.display()
    )];
    lines.extend(removed.iter().cloned());
    lines.join("\n")
}

/// What a person reads when the file cannot move without changing what it means.
#[must_use]
pub fn kept(path: &Path, reasons: &[String]) -> String {
    let mut lines = vec![format!(
        "{} stays in schema 3. Moving it would change what it means:",
        path.display()
    )];
    lines.extend(reasons.iter().map(|reason| format!("  {reason}")));
    lines.join("\n")
}

/// What the service records when it moved the policy file Ghostlight owns.
#[must_use]
pub fn upgraded_user_policy(path: &Path, backup: &Path) -> String {
    format!(
        "Ghostlight moved {} to policy schema 4. The original is kept as {}.",
        path.display(),
        backup.display()
    )
}

/// What the person is told when a backup of the same name already holds something else.
#[must_use]
pub fn backup_in_the_way(backup: &Path) -> String {
    format!(
        "{} already exists and differs from this file. Move it aside and run the command again.",
        backup.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sentence_is_plain_ascii() {
        let path = Path::new("policy.json");
        let backup = Path::new("policy.json.schema3.bak");
        let mut sentences = vec![
            already_current(path),
            migrated(path, backup, &["Removed mode.".into()]),
            kept(path, &["mode is observe.".into()]),
            upgraded_user_policy(path, backup),
            backup_in_the_way(backup),
        ];
        for retired in RETIRED_SETTINGS.into_iter().chain([Retired::Mode]) {
            sentences.push(removed(retired, "mode"));
            sentences.push(refused(retired, "mode", &Value::String("observe".into())));
        }
        for sentence in sentences {
            assert!(sentence.is_ascii(), "{sentence}");
        }
    }

    #[test]
    fn the_retired_settings_name_their_registered_keys() {
        let keys: Vec<_> = RETIRED_SETTINGS
            .iter()
            .filter_map(|retired| retired.config_key())
            .collect();
        assert_eq!(
            keys,
            [
                "channels.mcp.enabled",
                "channels.cli.enabled",
                "policy.user.enabled",
                "content.frames.handling",
                "content.frames.notice"
            ]
        );
    }
}
