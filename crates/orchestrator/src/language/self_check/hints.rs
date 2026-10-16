//! Every next step that names a tool asks only for fields that tool advertises (MSURF-2).
//!
//! A hint once told the model to call `browser_dialog` with `accept: true`, a field the dialog
//! schema never had, and a unit test pinned the wrong sentence. This guard reads every authored
//! next step the way a model would:
//!
//! - A word that looks like a tool name (`browser_...` or `policy_...`) must be a catalog tool.
//! - Within one clause (text between `.`, `;` and `,`), the word after `with` names a field
//!   (articles skipped). When the clause has already named a tool, that field must be one of the
//!   tool's top-level input fields, and when the field is a closed vocabulary such as `action`,
//!   the words after it joined by `or` must be values it accepts.
//! - Snake_case words after `with`, and words in backticks or quotes or written as `name:`, are
//!   field names too. In a clause that names no tool ("repeat the call with ..."), each must be a
//!   field some catalog tool advertises.
//!
//! - A handle value (a handle prefix, then eight lowercase base32 characters, such as
//!   `target_7k2nb5xe`) is never a field name. When it directly follows a field whose schema
//!   takes handles of one prefix, it must carry that prefix.
//!
//! Representative values cover every `Outcome` and `Refusal` variant. The matches in
//! `outcome_variant` and `refusal_variant` have no wildcard, so a new variant stops this module
//! compiling until it is named there. Add the name to the variant list beside the match and a
//! sample below; the coverage test compares the two.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::language::area::{LeftOut, LeftOutKey};
use crate::language::catalog;
use crate::language::composition::{
    CompositionProgress, EffectCounts, StepCause, StepCounts, StepIssue,
};
use crate::language::field_echo::HeldField;
use crate::language::field_problem::{ControlRef, FieldProblems, RejectedControl, RejectedField};
use crate::language::outcome::{
    ActionSubject, BlockedReason, BrowserRecoveryReason, BrowserUnavailableReason, CaptureKind,
    CheckFailure, DialogKind, EmptyObject, Outcome, Refusal, SavedTo, TargetNoun, TargetRole,
    WorkspaceReason,
};
use crate::workspace::StaleReason;

/// Words a hint may put between `with` and the field it names.
const ARTICLES: &[&str] = &["a", "an", "the"];

/// Prefixes that make a word a tool name.
const TOOL_PREFIXES: &[&str] = &["browser_", "policy_"];

/// The prefixes the catalog gives its handle fields (`handle()` in the catalog, pattern
/// `^<prefix>.+$`). A hint may name a handle value that starts with one of them.
const HANDLE_PREFIXES: &[&str] = &["target_", "tab_", "view_", "diag_", "image_", "invocation_"];

/// How many characters follow a handle prefix: eight, the length the workspace mints.
const HANDLE_SUFFIX_LEN: usize = 8;

/// Whether a character belongs to a handle's random part: lowercase RFC 4648 base32.
fn is_handle_suffix_char(character: char) -> bool {
    character.is_ascii_lowercase() || ('2'..='7').contains(&character)
}

/// The prefix of a word shaped like a handle value, or none.
fn handle_prefix(word: &str) -> Option<&'static str> {
    HANDLE_PREFIXES.iter().copied().find(|prefix| {
        word.strip_prefix(prefix).is_some_and(|suffix| {
            suffix.len() == HANDLE_SUFFIX_LEN && suffix.chars().all(is_handle_suffix_char)
        })
    })
}

/// The handle prefix a field's schema requires, read from its `^<prefix>` pattern.
fn required_handle_prefix(schema: &Value) -> Option<&'static str> {
    let pattern = schema.get("pattern")?.as_str()?.strip_prefix('^')?;
    HANDLE_PREFIXES
        .iter()
        .copied()
        .find(|prefix| pattern.starts_with(prefix))
}

/// One word of a hint, with whether the author marked it as a name.
struct Word {
    text: String,
    marked: bool,
}

fn words(clause: &str) -> Vec<Word> {
    clause
        .split_whitespace()
        .map(|raw| {
            let marked = raw.contains('`') || raw.contains('"') || raw.ends_with(':');
            let text = raw
                .trim_matches(|character: char| {
                    !(character.is_ascii_alphanumeric() || character == '_')
                })
                .to_owned();
            Word { text, marked }
        })
        .filter(|word| !word.text.is_empty())
        .collect()
}

fn is_tool_word(word: &str) -> bool {
    TOOL_PREFIXES.iter().any(|prefix| word.starts_with(prefix))
}

fn is_identifier(word: &Word) -> bool {
    word.marked
        || (word.text.contains('_')
            && !is_tool_word(&word.text)
            && handle_prefix(&word.text).is_none())
}

/// What the catalog advertises, read once.
struct Advertised {
    /// Each tool's top-level input fields and their schemas.
    tools: Vec<(String, serde_json::Map<String, Value>)>,
    /// Every field name any tool advertises, at any depth.
    anywhere: BTreeSet<String>,
}

impl Advertised {
    fn read() -> Self {
        let mut anywhere = BTreeSet::new();
        let tools = catalog()
            .into_iter()
            .map(|tool| {
                collect_fields(&tool.input_schema, &mut anywhere);
                let fields = tool.input_schema["properties"]
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                (tool.name, fields)
            })
            .collect();
        Self { tools, anywhere }
    }

    fn fields(&self, tool: &str) -> Option<&serde_json::Map<String, Value>> {
        self.tools
            .iter()
            .find(|(name, _)| name == tool)
            .map(|(_, fields)| fields)
    }
}

fn collect_fields(schema: &Value, found: &mut BTreeSet<String>) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, field) in properties {
            found.insert(name.clone());
            collect_fields(field, found);
        }
    }
    if let Some(items) = schema.get("items") {
        collect_fields(items, found);
    }
    for keyword in ["oneOf", "anyOf"] {
        for branch in schema
            .get(keyword)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            collect_fields(branch, found);
        }
    }
}

/// The values a closed field accepts, or none when the field is open.
fn closed_values(field: &Value) -> Option<Vec<String>> {
    if let Some(value) = field.get("const").and_then(Value::as_str) {
        return Some(vec![value.to_owned()]);
    }
    field.get("enum").and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

/// Every way one hint asks for something the catalog does not offer.
fn hint_violations(hint: &str, advertised: &Advertised) -> Vec<String> {
    let mut found = Vec::new();
    for clause in hint.split(['.', ';', ',']) {
        let words = words(clause);
        let mut tools: Vec<&str> = Vec::new();
        let mut after_with = false;
        let mut index = 0;
        while index < words.len() {
            let word = &words[index];
            index += 1;
            if is_tool_word(&word.text) {
                if advertised.fields(&word.text).is_none() {
                    found.push(format!("`{}` is not a catalog tool", word.text));
                }
                tools.push(&word.text);
                continue;
            }
            if let Some(prefix) = handle_prefix(&word.text) {
                let named = index
                    .checked_sub(2)
                    .and_then(|before| words.get(before))
                    .map(|field| field.text.as_str());
                for tool in &tools {
                    let required = named
                        .and_then(|name| advertised.fields(tool)?.get(name))
                        .and_then(required_handle_prefix);
                    if let Some(required) = required.filter(|required| *required != prefix) {
                        found.push(format!(
                            "`{tool}` field `{}` does not take a {prefix} handle (it takes {required})",
                            named.unwrap_or_default()
                        ));
                    }
                }
                continue;
            }
            let field = if word.text == "with" {
                after_with = true;
                while words
                    .get(index)
                    .is_some_and(|next| ARTICLES.contains(&next.text.as_str()))
                {
                    index += 1;
                }
                match words.get(index) {
                    Some(next)
                        if !is_tool_word(&next.text)
                            && handle_prefix(&next.text).is_none()
                            && (!tools.is_empty() || is_identifier(next)) =>
                    {
                        index += 1;
                        next
                    }
                    _ => continue,
                }
            } else if (after_with && is_identifier(word)) || word.marked {
                word
            } else {
                continue;
            };
            if tools.is_empty() {
                if !advertised.anywhere.contains(&field.text) {
                    found.push(format!("no catalog tool has a field `{}`", field.text));
                }
                continue;
            }
            let mut values_end = index;
            for tool in &tools {
                let Some(schema) = advertised
                    .fields(tool)
                    .and_then(|fields| fields.get(&field.text))
                else {
                    found.push(format!("`{tool}` has no field `{}`", field.text));
                    continue;
                };
                let Some(accepted) = closed_values(schema) else {
                    continue;
                };
                // A closed field names its values next, joined by "or".
                let mut position = index;
                loop {
                    match words.get(position) {
                        Some(value) if accepted.contains(&value.text) => {}
                        Some(value) => found.push(format!(
                            "`{tool}` field `{}` does not accept `{}`",
                            field.text, value.text
                        )),
                        None => {
                            found.push(format!("`{tool}` field `{}` names no value", field.text))
                        }
                    }
                    if words
                        .get(position + 1)
                        .is_some_and(|next| next.text == "or")
                    {
                        position += 2;
                    } else {
                        break;
                    }
                }
                values_end = values_end.max(position + 1);
            }
            index = values_end;
        }
    }
    found
}

/// The variant an outcome is, by name. No wildcard: a new variant fails to compile here until it
/// is named, and the coverage test then fails until `outcome_samples` holds a value of it.
fn outcome_variant(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::TabsListed { .. } => "TabsListed",
        Outcome::PolicyExplained { .. } => "PolicyExplained",
        Outcome::SelectorUnresolved { .. } => "SelectorUnresolved",
        Outcome::CompositionRan(_) => "CompositionRan",
        Outcome::CompositionProgress { .. } => "CompositionProgress",
        Outcome::CompositionUnrecorded => "CompositionUnrecorded",
        Outcome::StepNotStarted => "StepNotStarted",
        Outcome::DocumentInspected { .. } => "DocumentInspected",
        Outcome::TabActivated { .. } => "TabActivated",
        Outcome::PageOpened { .. } => "PageOpened",
        Outcome::PageReused { .. } => "PageReused",
        Outcome::PageRecovered { .. } => "PageRecovered",
        Outcome::TabAlreadyClosed => "TabAlreadyClosed",
        Outcome::PageNavigated { .. } => "PageNavigated",
        Outcome::HistoryTraversed { .. } => "HistoryTraversed",
        Outcome::PageReloaded { .. } => "PageReloaded",
        Outcome::TabClosed => "TabClosed",
        Outcome::TextRead { .. } => "TextRead",
        Outcome::TargetsListed { .. } => "TargetsListed",
        Outcome::Captured { .. } => "Captured",
        Outcome::TargetClicked { .. } => "TargetClicked",
        Outcome::PointClicked { .. } => "PointClicked",
        Outcome::PageScrolled { .. } => "PageScrolled",
        Outcome::TargetRevealed { .. } => "TargetRevealed",
        Outcome::ZoomSet { .. } => "ZoomSet",
        Outcome::WindowResized { .. } => "WindowResized",
        Outcome::Hovered { .. } => "Hovered",
        Outcome::FormFilled { .. } => "FormFilled",
        Outcome::TextTyped { .. } => "TextTyped",
        Outcome::KeyboardSent { .. } => "KeyboardSent",
        Outcome::Dragged { .. } => "Dragged",
        Outcome::FilesUploaded { .. } => "FilesUploaded",
        Outcome::ScriptEvaluated { .. } => "ScriptEvaluated",
        Outcome::Waited { .. } => "Waited",
        Outcome::DialogHandled { .. } => "DialogHandled",
        Outcome::DialogObserved { .. } => "DialogObserved",
        Outcome::DiagnosticsRead { .. } => "DiagnosticsRead",
        Outcome::RecordingStarted { .. } => "RecordingStarted",
        Outcome::RecordingObserved { .. } => "RecordingObserved",
        Outcome::RecordingStopped { .. } => "RecordingStopped",
        Outcome::RecordingBoundary { .. } => "RecordingBoundary",
        Outcome::RecordingSaved { .. } => "RecordingSaved",
        Outcome::RecordingDiscarded => "RecordingDiscarded",
        Outcome::DialogOpened { .. } => "DialogOpened",
    }
}

/// Every name `outcome_variant` can return.
const OUTCOME_VARIANTS: &[&str] = &[
    "TabsListed",
    "PolicyExplained",
    "SelectorUnresolved",
    "CompositionRan",
    "CompositionProgress",
    "CompositionUnrecorded",
    "StepNotStarted",
    "DocumentInspected",
    "TabActivated",
    "PageOpened",
    "PageReused",
    "PageRecovered",
    "TabAlreadyClosed",
    "PageNavigated",
    "HistoryTraversed",
    "PageReloaded",
    "TabClosed",
    "TextRead",
    "TargetsListed",
    "Captured",
    "TargetClicked",
    "PointClicked",
    "PageScrolled",
    "TargetRevealed",
    "ZoomSet",
    "WindowResized",
    "Hovered",
    "FormFilled",
    "TextTyped",
    "KeyboardSent",
    "Dragged",
    "FilesUploaded",
    "ScriptEvaluated",
    "Waited",
    "DialogHandled",
    "DialogObserved",
    "DiagnosticsRead",
    "RecordingStarted",
    "RecordingObserved",
    "RecordingStopped",
    "RecordingBoundary",
    "RecordingSaved",
    "RecordingDiscarded",
    "DialogOpened",
];

/// The variant a refusal is, by name. No wildcard, for the same reason as `outcome_variant`.
fn refusal_variant(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::Capacity => "Capacity",
        Refusal::InvalidRequest => "InvalidRequest",
        Refusal::CancelledBeforeStart => "CancelledBeforeStart",
        Refusal::DeadlineBeforeStart => "DeadlineBeforeStart",
        Refusal::AuthorityBlocked { .. } => "AuthorityBlocked",
        Refusal::LocalInterlock => "LocalInterlock",
        Refusal::CredentialAuthorization { .. } => "CredentialAuthorization",
        Refusal::IncompatibleReceipt => "IncompatibleReceipt",
        Refusal::BrowserAdapterOutdated => "BrowserAdapterOutdated",
        Refusal::DeadlineExpired { .. } => "DeadlineExpired",
        Refusal::BrowserPrimitive { .. } => "BrowserPrimitive",
        Refusal::FieldValuesRejected { .. } => "FieldValuesRejected",
        Refusal::FieldsReverted { .. } => "FieldsReverted",
        Refusal::DocumentUnavailable => "DocumentUnavailable",
        Refusal::CancelledBeforeDispatch => "CancelledBeforeDispatch",
        Refusal::BrowserUnavailable { .. } => "BrowserUnavailable",
        Refusal::BrowserContract => "BrowserContract",
        Refusal::BrowserAmbiguous => "BrowserAmbiguous",
        Refusal::BrowserUnknown => "BrowserUnknown",
        Refusal::BrowserPinned => "BrowserPinned",
        Refusal::BrowserStartupManual { .. } => "BrowserStartupManual",
        Refusal::BrowserRecoveryFailed { .. } => "BrowserRecoveryFailed",
        Refusal::ConnectionLost => "ConnectionLost",
        Refusal::CancelledAfterDispatch => "CancelledAfterDispatch",
        Refusal::EffectUnknown => "EffectUnknown",
        Refusal::LandingDeniedUnknown => "LandingDeniedUnknown",
        Refusal::WorkspaceUnusable { .. } => "WorkspaceUnusable",
        Refusal::FilesUnreadable => "FilesUnreadable",
        Refusal::CaptureTooLarge => "CaptureTooLarge",
        Refusal::NoDialogVisible => "NoDialogVisible",
        Refusal::DialogOpen => "DialogOpen",
        Refusal::RecordingUnavailable => "RecordingUnavailable",
        Refusal::RecordingHeld => "RecordingHeld",
        Refusal::RecordingExportFailed => "RecordingExportFailed",
        Refusal::ActionUnconfirmed { .. } => "ActionUnconfirmed",
    }
}

/// Every name `refusal_variant` can return.
const REFUSAL_VARIANTS: &[&str] = &[
    "Capacity",
    "InvalidRequest",
    "CancelledBeforeStart",
    "DeadlineBeforeStart",
    "AuthorityBlocked",
    "LocalInterlock",
    "CredentialAuthorization",
    "IncompatibleReceipt",
    "BrowserAdapterOutdated",
    "DeadlineExpired",
    "BrowserPrimitive",
    "FieldValuesRejected",
    "FieldsReverted",
    "DocumentUnavailable",
    "CancelledBeforeDispatch",
    "BrowserUnavailable",
    "BrowserContract",
    "BrowserAmbiguous",
    "BrowserUnknown",
    "BrowserPinned",
    "BrowserStartupManual",
    "BrowserRecoveryFailed",
    "ConnectionLost",
    "CancelledAfterDispatch",
    "EffectUnknown",
    "LandingDeniedUnknown",
    "WorkspaceUnusable",
    "FilesUnreadable",
    "CaptureTooLarge",
    "NoDialogVisible",
    "DialogOpen",
    "RecordingUnavailable",
    "RecordingHeld",
    "RecordingExportFailed",
    "ActionUnconfirmed",
];

/// Every policy reason. The match below has no wildcard, so a new reason fails to compile there.
fn blocked_reasons() -> Vec<BlockedReason> {
    let all = vec![
        BlockedReason::AuditUnavailable,
        BlockedReason::Host,
        BlockedReason::ProtectedHost,
        BlockedReason::OwnWorkbench,
        BlockedReason::Capability,
        BlockedReason::TabClose,
        BlockedReason::Screenshot,
        BlockedReason::InvalidAuthority,
        BlockedReason::Hold,
        BlockedReason::SessionEnded,
        BlockedReason::Unspecified,
    ];
    for reason in &all {
        match reason {
            BlockedReason::AuditUnavailable
            | BlockedReason::Host
            | BlockedReason::ProtectedHost
            | BlockedReason::OwnWorkbench
            | BlockedReason::Capability
            | BlockedReason::TabClose
            | BlockedReason::Screenshot
            | BlockedReason::InvalidAuthority
            | BlockedReason::Hold
            | BlockedReason::SessionEnded
            | BlockedReason::Unspecified => {}
        }
    }
    all
}

/// Every browser recovery reason, with the same compile-time tripwire.
fn recovery_reasons() -> Vec<BrowserRecoveryReason> {
    let all = vec![
        BrowserRecoveryReason::BrowserAbsent,
        BrowserRecoveryReason::LaunchFailed,
        BrowserRecoveryReason::SandboxedPackage,
        BrowserRecoveryReason::ExtensionAbsent,
        BrowserRecoveryReason::NativeHostUnavailable,
        BrowserRecoveryReason::OwnedElsewhere,
        BrowserRecoveryReason::HandshakeTimeout,
    ];
    for reason in &all {
        match reason {
            BrowserRecoveryReason::BrowserAbsent
            | BrowserRecoveryReason::LaunchFailed
            | BrowserRecoveryReason::SandboxedPackage
            | BrowserRecoveryReason::ExtensionAbsent
            | BrowserRecoveryReason::NativeHostUnavailable
            | BrowserRecoveryReason::OwnedElsewhere
            | BrowserRecoveryReason::HandshakeTimeout => {}
        }
    }
    all
}

/// Every browser availability reason, with the same compile-time tripwire.
fn unavailable_reasons() -> Vec<BrowserUnavailableReason> {
    let all = vec![
        BrowserUnavailableReason::NotAnswering,
        BrowserUnavailableReason::NotConnected,
        BrowserUnavailableReason::WrongProfile,
        BrowserUnavailableReason::Busy,
    ];
    for reason in &all {
        match reason {
            BrowserUnavailableReason::NotAnswering
            | BrowserUnavailableReason::NotConnected
            | BrowserUnavailableReason::WrongProfile
            | BrowserUnavailableReason::Busy => {}
        }
    }
    all
}

/// Every workspace reason, with the same compile-time tripwire.
fn workspace_reasons() -> Vec<WorkspaceReason> {
    let mut all = vec![
        WorkspaceReason::NoTab,
        WorkspaceReason::AmbiguousTab,
        WorkspaceReason::TabUnavailable,
        WorkspaceReason::StaleView,
        WorkspaceReason::TabHeld,
        WorkspaceReason::WorkspaceBusy,
        WorkspaceReason::OwnershipMismatch {
            owner: Some("Claude Code".into()),
        },
        WorkspaceReason::OwnershipMismatch { owner: None },
        WorkspaceReason::WorkspaceClosed,
    ];
    all.extend(
        StaleReason::ALL
            .into_iter()
            .map(|reason| WorkspaceReason::StaleTarget { reason }),
    );
    for reason in &all {
        match reason {
            WorkspaceReason::NoTab
            | WorkspaceReason::AmbiguousTab
            | WorkspaceReason::TabUnavailable
            | WorkspaceReason::StaleTarget { .. }
            | WorkspaceReason::StaleView
            | WorkspaceReason::TabHeld
            | WorkspaceReason::WorkspaceBusy
            | WorkspaceReason::OwnershipMismatch { .. }
            | WorkspaceReason::WorkspaceClosed => {}
        }
    }
    all
}

/// Every reason a requested check can fall short, with the same compile-time tripwire.
fn check_failures() -> Vec<CheckFailure> {
    let mut all = vec![
        CheckFailure::Unmet,
        CheckFailure::Cancelled,
        CheckFailure::DeadlineExpired,
        CheckFailure::Disconnected,
        CheckFailure::BrowserUnavailable,
        CheckFailure::DocumentUnavailable,
        CheckFailure::BrowserRefused,
    ];
    all.extend(blocked_reasons().into_iter().map(CheckFailure::Blocked));
    for failure in &all {
        match failure {
            CheckFailure::Unmet
            | CheckFailure::Blocked(_)
            | CheckFailure::Cancelled
            | CheckFailure::DeadlineExpired
            | CheckFailure::Disconnected
            | CheckFailure::BrowserUnavailable
            | CheckFailure::DocumentUnavailable
            | CheckFailure::BrowserRefused => {}
        }
    }
    all
}

/// Composition accounts that reach every branch of their next steps: each step cause, plus
/// unknown and applied effects.
fn composition_samples() -> Vec<CompositionProgress> {
    let causes = [
        StepCause::AuditUnavailable,
        StepCause::InvalidArguments,
        StepCause::MissingReference,
        StepCause::PolicyBlocked,
        StepCause::Paused,
        StepCause::SessionEnded,
        StepCause::AttentionRequired,
        StepCause::Failed,
        StepCause::ConnectionLost,
        StepCause::Deadline,
        StepCause::Cancelled,
        StepCause::Unconfirmed,
    ];
    for cause in &causes {
        match cause {
            StepCause::AuditUnavailable
            | StepCause::InvalidArguments
            | StepCause::MissingReference
            | StepCause::PolicyBlocked
            | StepCause::Paused
            | StepCause::SessionEnded
            | StepCause::AttentionRequired
            | StepCause::Failed
            | StepCause::ConnectionLost
            | StepCause::Deadline
            | StepCause::Cancelled
            | StepCause::Unconfirmed => {}
        }
    }
    let unfinished = |cause: Option<StepCause>, effects: EffectCounts| CompositionProgress {
        counts: StepCounts {
            total: 3,
            succeeded: 1,
            failed: 2,
            ..StepCounts::default()
        },
        effects,
        stopped: true,
        issue: cause.map(|cause| StepIssue { step: 2, cause }),
    };
    let mut samples = vec![
        CompositionProgress {
            counts: StepCounts {
                total: 2,
                succeeded: 2,
                ..StepCounts::default()
            },
            ..CompositionProgress::default()
        },
        unfinished(None, EffectCounts::default()),
    ];
    for cause in causes {
        for effects in [
            EffectCounts::default(),
            EffectCounts {
                applied: 1,
                ..EffectCounts::default()
            },
            EffectCounts {
                unknown: 1,
                ..EffectCounts::default()
            },
        ] {
            samples.push(unfinished(Some(cause), effects));
        }
    }
    samples
}

fn outcome_samples() -> Vec<Outcome> {
    let host = || Some("example.com".to_owned());
    let subject = || ActionSubject::unnamed(TargetRole::Button);
    let mut samples = vec![
        Outcome::TabsListed { count: 0 },
        Outcome::TabsListed { count: 2 },
        Outcome::PolicyExplained {
            capabilities: 4,
            layers: 0,
        },
        Outcome::SelectorUnresolved { matched: 0 },
        Outcome::SelectorUnresolved { matched: 2 },
        Outcome::CompositionProgress {
            completed: 1,
            total: 3,
        },
        Outcome::CompositionUnrecorded,
        Outcome::StepNotStarted,
        Outcome::DocumentInspected {
            nodes: 0,
            truncated: false,
            differences: None,
        },
        Outcome::DocumentInspected {
            nodes: 40,
            truncated: true,
            differences: Some(2),
        },
        Outcome::DocumentInspected {
            nodes: 40,
            truncated: false,
            differences: None,
        },
        Outcome::TabActivated { host: host() },
        Outcome::PageOpened { host: host() },
        Outcome::PageReused { host: host() },
        Outcome::PageRecovered { host: None },
        Outcome::TabAlreadyClosed,
        Outcome::PageNavigated { host: host() },
        Outcome::HistoryTraversed {
            direction: "back".into(),
            host: host(),
        },
        Outcome::PageReloaded { host: host() },
        Outcome::TabClosed,
        Outcome::TextRead {
            words: 0,
            host: host(),
        },
        Outcome::TextRead {
            words: 120,
            host: host(),
        },
        Outcome::Captured {
            scope: CaptureKind::Viewport,
            width: 800,
            height: 600,
        },
        Outcome::TargetClicked {
            host: host(),
            subject: subject(),
        },
        Outcome::PointClicked {
            host: host(),
            x: 1,
            y: 2,
            subject: None,
        },
        Outcome::PageScrolled {
            host: host(),
            direction: "down".into(),
        },
        Outcome::TargetRevealed {
            host: host(),
            subject: subject(),
        },
        Outcome::ZoomSet {
            percent: 150,
            host: host(),
        },
        Outcome::WindowResized {
            width: 1280,
            height: 800,
        },
        Outcome::Hovered {
            host: host(),
            subject: Some(subject()),
        },
        Outcome::FormFilled {
            fields: 2,
            submitted: false,
            host: host(),
            held: vec![HeldField::new(
                "target_7k2nb5xe",
                TargetRole::Combobox,
                "Role",
                "Administrator",
                false,
                true,
            )],
        },
        Outcome::TextTyped {
            host: host(),
            subject: subject(),
            characters: 5,
        },
        Outcome::KeyboardSent {
            host: host(),
            key: Some("Enter".into()),
            subject: None,
        },
        Outcome::Dragged {
            host: host(),
            source: None,
            destination: None,
        },
        Outcome::FilesUploaded {
            count: 1,
            host: host(),
            subject: None,
        },
        Outcome::ScriptEvaluated {
            host: host(),
            empty_object: EmptyObject::Absent,
        },
        Outcome::ScriptEvaluated {
            host: host(),
            empty_object: EmptyObject::Awaited,
        },
        Outcome::ScriptEvaluated {
            host: host(),
            empty_object: EmptyObject::NotAwaited,
        },
        Outcome::DialogHandled { accepted: true },
        Outcome::DialogObserved {
            present: true,
            message: None,
        },
        Outcome::DialogObserved {
            present: false,
            message: None,
        },
        Outcome::DiagnosticsRead {
            count: 0,
            capture_started: true,
            problems_only: true,
            host: host(),
        },
        Outcome::DiagnosticsRead {
            count: 3,
            capture_started: false,
            problems_only: false,
            host: host(),
        },
        Outcome::RecordingStarted { host: host() },
        Outcome::RecordingObserved {
            frames: 10,
            duration_ms: 2_000,
        },
        Outcome::RecordingStopped { duration_ms: 2_000 },
        Outcome::RecordingBoundary { duration_ms: 2_000 },
        Outcome::RecordingSaved {
            duration_ms: 2_000,
            delivery: SavedTo::Client,
        },
        Outcome::RecordingDiscarded,
    ];
    for dialog in [
        DialogKind::Alert,
        DialogKind::Confirm,
        DialogKind::Prompt,
        DialogKind::BeforeUnload,
        DialogKind::Other,
    ] {
        samples.push(
            Outcome::TargetClicked {
                host: host(),
                subject: ActionSubject::unnamed(TargetRole::Button),
            }
            .with_open_dialog(dialog),
        );
    }
    for noun in [TargetNoun::Match, TargetNoun::Control, TargetNoun::Item] {
        for count in [0, 3] {
            samples.push(Outcome::TargetsListed {
                noun,
                count,
                host: host(),
                left_out: LeftOut::default(),
            });
        }
        for key in [LeftOutKey::Content, LeftOutKey::Aside, LeftOutKey::Chrome] {
            let mut left_out = LeftOut::default();
            left_out.add(key, 96);
            samples.push(Outcome::TargetsListed {
                noun,
                count: 80,
                host: host(),
                left_out,
            });
        }
    }
    for satisfied in [false, true] {
        samples.push(Outcome::Waited {
            condition: "text_present".into(),
            elapsed_ms: 1_200,
            satisfied,
            host: host(),
        });
    }
    samples.extend(
        composition_samples()
            .into_iter()
            .map(Outcome::CompositionRan),
    );
    samples
}

fn refusal_samples() -> Vec<Refusal> {
    let mut samples = vec![
        Refusal::Capacity,
        Refusal::InvalidRequest,
        Refusal::CancelledBeforeStart,
        Refusal::DeadlineBeforeStart,
        Refusal::LocalInterlock,
        Refusal::CredentialAuthorization {
            control: ControlRef::new(
                TargetRole::Textbox,
                "Password",
                Some("target_7k2nb5xe"),
                true,
            ),
        },
        Refusal::IncompatibleReceipt,
        Refusal::BrowserAdapterOutdated,
        Refusal::BrowserPrimitive {
            detail: "no such frame".into(),
        },
        Refusal::DocumentUnavailable,
        Refusal::CancelledBeforeDispatch,
        Refusal::BrowserContract,
        Refusal::BrowserAmbiguous,
        Refusal::BrowserUnknown,
        Refusal::BrowserPinned,
        Refusal::BrowserStartupManual {
            browsers: vec!["Chrome".into(), "Edge".into()],
        },
        Refusal::ConnectionLost,
        Refusal::CancelledAfterDispatch,
        Refusal::EffectUnknown,
        Refusal::LandingDeniedUnknown,
        Refusal::FilesUnreadable,
        Refusal::CaptureTooLarge,
        Refusal::NoDialogVisible,
        Refusal::DialogOpen,
        Refusal::RecordingUnavailable,
        Refusal::RecordingHeld,
        Refusal::RecordingExportFailed,
    ];
    samples.extend(rejected_fill_samples());
    samples.extend(reverted_fill_samples());
    for before_dispatch in [false, true] {
        for timeout_adjustable in [false, true] {
            samples.push(Refusal::DeadlineExpired {
                before_dispatch,
                timeout_adjustable,
            });
        }
    }
    for reason in blocked_reasons() {
        samples.push(Refusal::AuthorityBlocked {
            reason,
            host: Some("example.com".into()),
            origin: (reason == BlockedReason::OwnWorkbench)
                .then(|| "http://example.com:8080".into()),
        });
    }
    for reason in recovery_reasons() {
        samples.push(Refusal::BrowserRecoveryFailed { reason });
    }
    for reason in unavailable_reasons() {
        samples.push(Refusal::BrowserUnavailable { reason });
    }
    for reason in workspace_reasons() {
        samples.push(Refusal::WorkspaceUnusable { reason });
    }
    for cause in check_failures() {
        samples.push(Refusal::ActionUnconfirmed {
            action: Box::new(Outcome::TargetClicked {
                host: Some("example.com".into()),
                subject: ActionSubject::unnamed(TargetRole::Link),
            }),
            cause,
        });
    }
    samples
}

/// A fill the page partly undid, with a value the page can quote and with one it cannot.
fn reverted_fill_samples() -> Vec<Refusal> {
    use serde_json::json;
    [(json!({"now": "Choose one..."}), false), (json!({}), true)]
        .into_iter()
        .map(|(mut problem, credential)| {
            problem["field"] = json!(0);
            problem["problem"] = json!("reverted");
            let decoded = FieldProblems::decode(&json!({"problems": [problem]}))
                .expect("a sample problem decodes")
                .problems
                .remove(0);
            let control = RejectedControl {
                target: "target_7k2nb5xe".into(),
                role: TargetRole::Combobox,
                name: "Role".into(),
                given: "Engineer".into(),
                credential,
                follows_choice: false,
            };
            Refusal::FieldsReverted {
                set: 2,
                reverted: vec![RejectedField::new(decoded, control, true)],
            }
        })
        .collect()
}

/// One refused fill for each way a field can be refused, so every step it can author is read.
fn rejected_fill_samples() -> Vec<Refusal> {
    use serde_json::json;
    let options = json!(["Developer", "Administrator", "Other"]);
    let cases = [
        (
            json!({"problem": "option_missing", "options": options}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "option_missing", "options": options}),
            TargetRole::Combobox,
            false,
            true,
        ),
        (
            json!({"problem": "option_missing", "options": options}),
            TargetRole::Combobox,
            true,
            false,
        ),
        (
            json!({"problem": "option_missing", "options": options, "options_total": 195, "containing": ["Eng"]}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "option_missing", "options": ["Standard", "Express"], "group": "Shipping"}),
            TargetRole::Radio,
            false,
            false,
        ),
        (
            json!({"problem": "option_ambiguous", "options": [{"text": "Other", "value": "a"}, {"text": "Other", "value": "b"}]}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "option_ambiguous", "options": ["Yes", "Yes"], "group": "Subscribe"}),
            TargetRole::Radio,
            false,
            false,
        ),
        (
            json!({"problem": "option_disabled"}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "multiple_values"}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "not_boolean"}),
            TargetRole::Checkbox,
            false,
            false,
        ),
        (
            json!({"problem": "format", "expected": "YYYY-MM-DD", "type": "date", "limits": {"min": "2026-10-01", "max": "2026-12-31"}}),
            TargetRole::Textbox,
            false,
            false,
        ),
        (
            json!({"problem": "wrong_tool", "route": "file"}),
            TargetRole::Textbox,
            false,
            false,
        ),
        (
            json!({"problem": "wrong_tool", "route": "button"}),
            TargetRole::Button,
            false,
            false,
        ),
        (
            json!({"problem": "wrong_tool", "route": "widget"}),
            TargetRole::Combobox,
            false,
            false,
        ),
        (
            json!({"problem": "submit_not_owned"}),
            TargetRole::Button,
            false,
            false,
        ),
    ];
    cases
        .into_iter()
        .map(|(mut problem, role, credential, follows_choice)| {
            problem["field"] = json!(0);
            let decoded = FieldProblems::decode(&json!({"problems": [problem]}))
                .expect("a sample problem decodes")
                .problems
                .remove(0);
            let control = RejectedControl {
                target: "target_7k2nb5xe".into(),
                role,
                name: "Role".into(),
                given: "Engineer".into(),
                credential,
                follows_choice,
            };
            Refusal::FieldValuesRejected {
                fields: vec![RejectedField::new(decoded, control, true)],
                total: 1,
            }
        })
        .collect()
}

/// Every distinct next step Ghostlight can author.
fn every_next_step() -> BTreeSet<String> {
    outcome_samples()
        .iter()
        .flat_map(Outcome::next_steps)
        .chain(refusal_samples().iter().flat_map(Refusal::next_steps))
        .collect()
}

#[test]
fn every_outcome_and_refusal_variant_is_sampled() {
    let sampled: BTreeSet<&str> = outcome_samples().iter().map(outcome_variant).collect();
    let expected: BTreeSet<&str> = OUTCOME_VARIANTS.iter().copied().collect();
    assert_eq!(
        expected.len(),
        OUTCOME_VARIANTS.len(),
        "duplicate outcome name"
    );
    assert_eq!(sampled, expected);

    let sampled: BTreeSet<&str> = refusal_samples().iter().map(refusal_variant).collect();
    let expected: BTreeSet<&str> = REFUSAL_VARIANTS.iter().copied().collect();
    assert_eq!(
        expected.len(),
        REFUSAL_VARIANTS.len(),
        "duplicate refusal name"
    );
    assert_eq!(sampled, expected);
}

#[test]
fn every_next_step_names_real_tools_and_their_fields() {
    let advertised = Advertised::read();
    let steps = every_next_step();
    assert!(
        steps
            .iter()
            .any(|step| step.contains("browser_dialog with action")),
        "the samples no longer reach the dialog hint"
    );
    for step in &steps {
        assert!(step.is_ascii(), "{step}");
        let found = hint_violations(step, &advertised);
        assert!(found.is_empty(), "{step:?}: {found:?}");
    }
}

/// Negative control: hints that ask for what the catalog does not offer are caught.
#[test]
fn the_hint_checker_catches_calls_the_decoder_would_reject() {
    let advertised = Advertised::read();
    for (hint, expected) in [
        (
            "Call browser_dialog with accept: true or accept: false before continuing.",
            "`browser_dialog` has no field `accept`",
        ),
        (
            "Call browser_tabs with action open to see the tabs.",
            "does not accept `open`",
        ),
        (
            "Call browser_dialog with action accept or approve.",
            "does not accept `approve`",
        ),
        (
            "Call browser_teleport to go anywhere.",
            "`browser_teleport` is not a catalog tool",
        ),
        (
            "Repeat the call with a longer timeout_millis.",
            "no catalog tool has a field `timeout_millis`",
        ),
        (
            "Repeat browser_click with tab target_7k2nb5xe.",
            "`browser_click` field `tab` does not take a target_ handle",
        ),
        (
            "Repeat browser_click with targt target_7k2nb5xe.",
            "`browser_click` has no field `targt`",
        ),
        (
            "Call browser_read with `max_result_chars` set higher.",
            "`browser_read` has no field `max_result_chars`",
        ),
    ] {
        let found = hint_violations(hint, &advertised);
        assert!(
            found.iter().any(|violation| violation.contains(expected)),
            "{hint:?} should report {expected:?}; got {found:?}"
        );
    }
    for hint in [
        "Call browser_tabs with action list to see the connected browsers.",
        "Call browser_dialog with action accept or dismiss (or respond with text for a prompt) before continuing.",
        "Repeat with a longer timeout_ms when the page genuinely needs more time.",
        "Check the page with browser_read or browser_inspect before repeating.",
        "Repeat browser_click with target target_7k2nb5xe.",
        "Repeat browser_fill_form with target_7k2nb5xe set to one of the offered options.",
        "Call browser_dialog with action dismiss on tab tab_m3k5q2xa.",
    ] {
        assert_eq!(hint_violations(hint, &advertised), Vec::<String>::new(), "{hint}");
    }
}
