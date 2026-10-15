//! The parts of the workbench state a screen may ask for, and what each audience sees of them.
//!
//! A snapshot is cut into closed parts: status, connections, activity, integrations and
//! diagnostics. Policy has its own route. Each part is projected for an [`Audience`] when the
//! response is written, so a page never redacts and never holds data its context may not see
//! (ADR-0199, amendment of 2026-10-15). The projection drops what nobody reads, shows home-relative
//! paths to everyone, and turns the person's own lists of sensitive sites into counts for a watcher.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    BrowserInstanceSummary, DiagnosticItem, HistoryItem, OperationSummary, ReadinessSummary,
    ServiceSummary, SessionSummary, WorkbenchSnapshot,
};
use crate::governance::effective::{EffectiveAuthority, LayerKind, PolicyChip};
use crate::governance::ManagedPolicyPassport;
use crate::install::{HarnessActionResult, HarnessSummary};
use crate::language::audit_health::AuditHealth;
use crate::language::coverage::HumanCoverage;
use crate::language::workbench as words;

/// The environment variable that names the user's home directory.
#[cfg(windows)]
pub const USER_HOME_VARIABLE: &str = "USERPROFILE";

/// The environment variable that names the user's home directory.
#[cfg(not(windows))]
pub const USER_HOME_VARIABLE: &str = "HOME";

/// The setting key of the person's list of sites agents must never touch.
pub const SACRED_DOMAINS_KEY: &str = "content.security.sacred_domains";

/// The etag of a policy that has no user layer.
pub const NO_POLICY_ETAG: &str = "none";

/// Who a projection is made for. Closed contexts never receive data, so they have no audience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Audience {
    /// The Ghostlight window.
    Control,
    /// Any other screen on this computer while watching is on.
    Watch,
}

/// One closed slice of the snapshot a screen can ask for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    /// Readiness, service facts, saving health and the policy chip.
    Status,
    /// Admitted sessions and connected browsers.
    Connections,
    /// Live operations, history and document coverage.
    Activity,
    /// The supported clients and their registrations.
    Integrations,
    /// Checkup cards and process diagnostics.
    Diagnostics,
}

/// A part token the service does not know.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("unknown part token")]
pub struct UnknownPart;

impl Part {
    /// Every part, in the order a snapshot lists them.
    pub const ALL: [Self; 5] = [
        Self::Status,
        Self::Connections,
        Self::Activity,
        Self::Integrations,
        Self::Diagnostics,
    ];

    /// The snake_case token that names this part on the wire.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Connections => "connections",
            Self::Activity => "activity",
            Self::Integrations => "integrations",
            Self::Diagnostics => "diagnostics",
        }
    }

    /// The part a token names.
    pub fn from_token(token: &str) -> Result<Self, UnknownPart> {
        Self::ALL
            .into_iter()
            .find(|part| part.token() == token)
            .ok_or(UnknownPart)
    }
}

#[derive(Serialize)]
struct StatusPart<'a> {
    service: &'a ServiceSummary,
    readiness: &'a ReadinessSummary,
    audit_health: &'a AuditHealth,
    audit_notice: &'a str,
    configuration: StatusConfiguration<'a>,
}

#[derive(Serialize)]
struct StatusConfiguration<'a> {
    policy: &'a PolicyChip,
    managed_policy: &'a ManagedPolicyPassport,
}

#[derive(Serialize)]
struct ConnectionsPart<'a> {
    sessions: &'a [SessionSummary],
    browsers: &'a [BrowserInstanceSummary],
}

#[derive(Serialize)]
struct ActivityPart<'a> {
    operations: &'a [OperationSummary],
    history: &'a [HistoryItem],
    document_coverage: &'a [HumanCoverage],
}

#[derive(Serialize)]
struct IntegrationsPart<'a> {
    harnesses: &'a [HarnessSummary],
}

#[derive(Serialize)]
struct DiagnosticsPart<'a> {
    diagnostics: &'a [DiagnosticItem],
    process_diagnostics: &'a crate::diagnostics::DiagnosticsReport,
}

/// What one audience sees, made for one user home.
#[derive(Clone, Debug)]
pub struct Projection {
    audience: Audience,
    user_home: Option<PathBuf>,
}

impl Projection {
    /// A projection for `audience` that shows paths relative to the user's home directory.
    #[must_use]
    pub fn new(audience: Audience) -> Self {
        Self::with_home(audience, user_home())
    }

    /// A projection for `audience` over an explicit user home. Without one, paths stay as they are.
    #[must_use]
    pub const fn with_home(audience: Audience, user_home: Option<PathBuf>) -> Self {
        Self {
            audience,
            user_home,
        }
    }

    /// Whether this projection is made for a watcher.
    const fn watching(&self) -> bool {
        matches!(self.audience, Audience::Watch)
    }

    fn path(&self, path: &str) -> String {
        match &self.user_home {
            Some(home) => display_path(home, path),
            None => path.to_owned(),
        }
    }

    fn rewrite_path(&self, value: &mut Value) {
        if let Some(path) = value.as_str() {
            *value = Value::String(self.path(path));
        }
    }

    /// One part of `snapshot`, as this audience sees it.
    #[must_use]
    pub fn part(&self, part: Part, snapshot: &WorkbenchSnapshot) -> Value {
        let mut value = match part {
            Part::Status => to_value(&StatusPart {
                service: &snapshot.service,
                readiness: &snapshot.readiness,
                audit_health: &snapshot.audit_health,
                audit_notice: &snapshot.audit_notice,
                configuration: StatusConfiguration {
                    policy: &snapshot.configuration.policy,
                    managed_policy: &snapshot.configuration.managed_policy,
                },
            }),
            Part::Connections => to_value(&ConnectionsPart {
                sessions: &snapshot.sessions,
                browsers: &snapshot.browsers,
            }),
            Part::Activity => to_value(&ActivityPart {
                operations: &snapshot.operations,
                history: &snapshot.history,
                document_coverage: &snapshot.document_coverage,
            }),
            Part::Integrations => to_value(&IntegrationsPart {
                harnesses: &snapshot.harnesses,
            }),
            Part::Diagnostics => to_value(&DiagnosticsPart {
                diagnostics: &snapshot.diagnostics,
                process_diagnostics: &snapshot.process_diagnostics,
            }),
        };
        match part {
            Part::Status => {}
            Part::Connections => each(&mut value, "sessions", |session| {
                remove(session, &["leased", "held_tab_count"]);
            }),
            Part::Activity => each(&mut value, "document_coverage", |coverage| {
                self.count_excluded_hosts(coverage);
            }),
            Part::Integrations => each(&mut value, "harnesses", |harness| {
                self.project_harness(harness);
            }),
            Part::Diagnostics => {
                if let Some(directory) = value.pointer_mut("/process_diagnostics/directory") {
                    self.rewrite_path(directory);
                }
            }
        }
        value
    }

    /// The excluded sites of one document, which only the window may name.
    fn count_excluded_hosts(&self, coverage: &mut Value) {
        let count = coverage
            .get("excluded_hosts")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if self.watching() {
            remove(coverage, &["excluded_hosts"]);
        }
        if let Some(object) = coverage.as_object_mut() {
            object.insert("excluded_host_count".into(), json!(count));
        }
    }

    fn project_harness(&self, harness: &mut Value) {
        remove(harness, &["private_view"]);
        if let Some(path) = harness.get_mut("config_path") {
            self.rewrite_path(path);
        }
    }

    /// The process-diagnostics state, with its folder shown relative to the user's home.
    #[must_use]
    pub fn diagnostics_report(&self, report: &crate::diagnostics::DiagnosticsReport) -> Value {
        let mut value = to_value(report);
        if let Some(directory) = value.get_mut("directory") {
            self.rewrite_path(directory);
        }
        value
    }

    /// The supported clients, as this audience sees them.
    #[must_use]
    pub fn harnesses(&self, summaries: &[HarnessSummary]) -> Value {
        let mut value = to_value(&summaries);
        if let Some(items) = value.as_array_mut() {
            for item in items {
                self.project_harness(item);
            }
        }
        value
    }

    /// The result of one client act, with its summary projected like any other.
    #[must_use]
    pub fn harness_result(&self, result: &HarnessActionResult) -> Value {
        let mut value = to_value(result);
        if let Some(summary) = value.get_mut("summary") {
            self.project_harness(summary);
        }
        value
    }

    /// The compiled policy, as this audience sees it. Control also gets the etag of the user
    /// layer, which the policy write routes check; a watcher never writes and is not given a
    /// value it could test guesses about the withheld never-touch list against.
    #[must_use]
    pub fn policy(&self, authority: &EffectiveAuthority) -> Value {
        let mut value = to_value(authority);
        let mut never_touch: Vec<String> = Vec::new();
        each(&mut value, "layers", |layer| {
            if let Some(path) = layer.get_mut("path") {
                self.rewrite_path(path);
            }
            if self.watching() {
                layer["document"] = Value::Null;
            }
            each(layer, "settings", |setting| {
                if setting["key"] != SACRED_DOMAINS_KEY {
                    return;
                }
                let hosts = setting["value"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
                    .unwrap_or_default();
                if self.watching() {
                    setting["value"] = Value::String(hosts.len().to_string());
                    setting["count"] = json!(hosts.len());
                    never_touch.extend(hosts);
                }
            });
        });
        if let Some(path) = value.pointer_mut("/user_layer/path") {
            self.rewrite_path(path);
        }
        if self.watching() {
            Self::count_never_touch_ceilings(&mut value, &never_touch);
        }
        if !self.watching() {
            value["etag"] = Value::String(policy_etag(authority));
        }
        value
    }

    /// Replace the ceiling lines that name the person's never-touch sites with one count.
    fn count_never_touch_ceilings(policy: &mut Value, hosts: &[String]) {
        let Some(lines) = policy.get_mut("ceilings").and_then(Value::as_array_mut) else {
            return;
        };
        let before = lines.len();
        lines.retain(|line| {
            let text = line.as_str().unwrap_or_default();
            !hosts.iter().any(|host| {
                text.strip_prefix(host.as_str())
                    .is_some_and(|rest| rest.starts_with(','))
            })
        });
        if lines.len() < before {
            lines.push(Value::String(words::never_touch_count(hosts.len())));
        }
    }
}

fn to_value<T: Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).expect("a workbench part is plain data")
}

/// Run `edit` on every element of the array under `key`, when there is one.
fn each(value: &mut Value, key: &str, mut edit: impl FnMut(&mut Value)) {
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        items.iter_mut().for_each(&mut edit);
    }
}

fn remove(value: &mut Value, keys: &[&str]) {
    if let Some(object) = value.as_object_mut() {
        for key in keys {
            object.remove(*key);
        }
    }
}

/// The user's home directory, when the environment names one.
#[must_use]
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os(USER_HOME_VARIABLE)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// A path as `~` plus what lies below the user's home, with the platform's separator.
///
/// The prefix match ignores case on Windows, where either separator matches. A path outside the
/// home stays as it is.
#[must_use]
pub fn display_path(user_home: &Path, path: &str) -> String {
    let home = user_home.to_string_lossy();
    let home = home.trim_end_matches(is_separator);
    if home.is_empty() || path.len() < home.len() || !path.is_char_boundary(home.len()) {
        return path.to_owned();
    }
    let (prefix, rest) = path.split_at(home.len());
    if !same_prefix(prefix, home) || !(rest.is_empty() || rest.starts_with(is_separator)) {
        return path.to_owned();
    }
    let rest: String = rest
        .chars()
        .map(|character| {
            if is_separator(character) {
                std::path::MAIN_SEPARATOR
            } else {
                character
            }
        })
        .collect();
    format!("~{rest}")
}

fn same_prefix(left: &str, right: &str) -> bool {
    left.chars().count() == right.chars().count()
        && left.chars().zip(right.chars()).all(|(a, b)| {
            if is_separator(a) || is_separator(b) {
                is_separator(a) && is_separator(b)
            } else if cfg!(windows) {
                a.eq_ignore_ascii_case(&b)
            } else {
                a == b
            }
        })
}

fn is_separator(character: char) -> bool {
    character == '/' || (cfg!(windows) && character == '\\')
}

/// The tag a write must quote to change the person's own rules.
///
/// It is a hash of the user layer's document, parsed, without its top-level `version` and with
/// sorted keys, so a reformatted or re-versioned file keeps its tag. A machine with no user layer
/// has the tag [`NO_POLICY_ETAG`].
#[must_use]
pub fn policy_etag(authority: &EffectiveAuthority) -> String {
    let document = authority
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::User)
        .and_then(|layer| layer.document.as_deref());
    let Some(document) = document else {
        return NO_POLICY_ETAG.to_owned();
    };
    let text = match serde_json::from_str::<Value>(document) {
        Ok(mut parsed) => {
            if let Some(object) = parsed.as_object_mut() {
                object.remove("version");
            }
            let mut text = String::new();
            write_sorted(&parsed, &mut text);
            text
        }
        Err(_) => document.to_owned(),
    };
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_sorted(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_sorted(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_sorted(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

#[cfg(test)]
mod tests;
