//! Why a form fill cannot take its values, and what would succeed instead (ADR-0198 Decisions 8
//! and 10).
//!
//! The page collects every field problem before it edits anything and names each one with a word
//! from a closed vocabulary. This module owns that vocabulary, the detail the page sends, and the
//! one place that turns a rejected field into a sentence, a next step and a fact. The browser
//! layer decodes the detail by these types, so a word this build does not know makes the whole
//! detail undecodable and the refusal falls back to the generic browser refusal.
//!
//! A page can also change a field back after the fill set it. The same vocabulary names such a
//! field, with what it holds now, and this module speaks the partly applied fill.
//!
//! Page text (option text, group names, a reverted value) and the caller's given value only reach
//! a sentence when the governed snapshot preserves target names. Facts carry the options, because the fill was
//! authorized to read the document, but never the given value. A credential-class control never
//! lists options or echoes a value anywhere.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::outcome::{article, counted, one_line_bounded, ActionSubject, TargetRole};

/// The longest part of a given value a sentence quotes.
const GIVEN_MAX_CHARS: usize = 40;
/// The longest page text one option, group or limit contributes to a sentence.
const PAGE_TEXT_MAX_CHARS: usize = 60;
/// The most characters a listed set of options may take in one clause.
const LIST_MAX_CHARS: usize = 140;
/// The characters held back for the tail that counts clauses the summary had no room for.
const TAIL_RESERVE_CHARS: usize = 40;
/// Prefixes of the handles Ghostlight mints. Page text that looks like one is not repeated.
const HANDLE_PREFIXES: [&str; 6] = ["target_", "tab_", "view_", "diag_", "image_", "invocation_"];
/// How many characters follow a handle prefix.
const HANDLE_SUFFIX_CHARS: usize = 8;
/// What page text shaped like a handle becomes in a sentence.
const HANDLE_REPLACEMENT: &str = "[handle]";
/// The input type of a file input, which a fill hands to `browser_upload`.
const FILE_INPUT_TYPE: &str = "file";
/// The `reason` fact of a fill that was refused before any edit.
const REJECTED_REASON: &str = "field_value_rejected";
/// The `reason` fact of a fill the page partly undid.
const REVERTED_REASON: &str = "fields_reverted";
/// What a summary says a reverted field holds when it holds nothing.
const NO_VALUE: &str = "no value";
/// The most reverted fields a summary describes one by one.
const REVERTED_SHOWN_MAX: usize = 3;
/// The most next steps a result carries.
const NEXT_STEPS_MAX: usize = 2;

/// The closed vocabulary of reasons a page gives for a field that cannot take its value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldProblemKind {
    /// The select has no option the value names.
    OptionMissing,
    /// The value names more than one option.
    OptionAmbiguous,
    /// The value names an option the page disabled.
    OptionDisabled,
    /// A list was given to a control that takes one option.
    MultipleValues,
    /// A checkbox was given a word that is not true or false.
    NotBoolean,
    /// The value does not have the form the control takes.
    Format,
    /// The control is not filled with a fill: a file input, a button or a page widget.
    WrongTool,
    /// The page changed the value back after the fill set it.
    Reverted,
    /// The submit control does not belong to the form that holds the fields.
    SubmitNotOwned,
}

impl FieldProblemKind {
    /// The word the page and the facts use for this problem.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OptionMissing => "option_missing",
            Self::OptionAmbiguous => "option_ambiguous",
            Self::OptionDisabled => "option_disabled",
            Self::MultipleValues => "multiple_values",
            Self::NotBoolean => "not_boolean",
            Self::Format => "format",
            Self::WrongTool => "wrong_tool",
            Self::Reverted => "reverted",
            Self::SubmitNotOwned => "submit_not_owned",
        }
    }
}

/// One option a refusal offers, as the page lists it: bare text, text with the detail that tells
/// it apart or marks it disabled, or an option group.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OfferedOption {
    /// An option whose text alone names it.
    Text(String),
    /// A named option group and its options.
    Group {
        /// The group's label.
        group: String,
        /// The options inside the group.
        options: Vec<OfferedOption>,
    },
    /// An option listed with its value, its disabled mark, or both.
    Entry {
        /// The option's text.
        text: String,
        /// The option's value, listed when the text alone does not pick it out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        /// Whether the page disabled the option.
        #[serde(default, skip_serializing_if = "is_false")]
        disabled: bool,
    },
}

/// Whether a flag is unset, so a default flag stays out of the serialized option.
const fn is_false(value: &bool) -> bool {
    !*value
}

impl OfferedOption {
    /// Append the options a value could name, in order: groups are flattened and disabled
    /// options are left out. An entry that carries a value is shown with it.
    fn collect_enabled(&self, out: &mut Vec<String>) {
        match self {
            Self::Text(text) => out.push(page_text(text, PAGE_TEXT_MAX_CHARS)),
            Self::Group { options, .. } => {
                for option in options {
                    option.collect_enabled(out);
                }
            }
            Self::Entry {
                text,
                value,
                disabled,
            } => {
                if !disabled {
                    out.push(Self::shown(text, value.as_deref()));
                }
            }
        }
    }

    /// Add the options below this entry to a count, disabled ones included.
    fn count_into(&self, total: &mut usize) {
        match self {
            Self::Text(_) | Self::Entry { .. } => *total += 1,
            Self::Group { options, .. } => {
                for option in options {
                    option.count_into(total);
                }
            }
        }
    }

    fn shown(text: &str, value: Option<&str>) -> String {
        let text = page_text(text, PAGE_TEXT_MAX_CHARS);
        match value {
            Some(value) => format!("{text} (value {})", page_text(value, PAGE_TEXT_MAX_CHARS)),
            None => text,
        }
    }
}

/// One field the page could not fill, as the page reports it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldProblem {
    /// The field's position in the list the page was given.
    pub field: usize,
    /// What is wrong with it.
    #[serde(rename = "problem")]
    pub kind: FieldProblemKind,
    /// The options that would succeed, or the candidates a value matched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<OfferedOption>,
    /// The full option count when the page cut the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options_total: Option<usize>,
    /// The options of a long list whose text contains the refused value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub containing: Vec<String>,
    /// The form the control takes, such as `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// The constraints the control declares.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub limits: Map<String, Value>,
    /// Which entry of a given list the problem is about, when the value was a list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_index: Option<usize>,
    /// The name of the choice group a radio belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The input type of the control, when its role does not imply it.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    /// What a reverted control holds now, when the page can say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
}

/// The detail of a page failure that refused a fill: every field it could not take.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldProblems {
    /// The problems, in field order.
    pub problems: Vec<FieldProblem>,
}

impl FieldProblems {
    /// Read the detail of a page failure. A shape this build does not know, an unknown problem
    /// word included, gives none, so the refusal stays the generic one.
    #[must_use]
    pub fn decode(detail: &Value) -> Option<Self> {
        serde_json::from_value::<Self>(detail.clone())
            .ok()
            .filter(|decoded| !decoded.problems.is_empty())
    }
}

/// What the service knows about one field of the call, beside what the page said about it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedControl {
    /// The model-facing handle of the field.
    pub target: String,
    /// The control's role as the page last reported it.
    pub role: TargetRole,
    /// The control's accessible name.
    pub name: String,
    /// The value the caller gave.
    pub given: String,
    /// Whether the page classified the control as a credential field.
    pub credential: bool,
    /// Whether an earlier field of the same call is a choice whose answer can change this one's
    /// options.
    pub follows_choice: bool,
}

/// A rejected field in the words the model reads. Whether names, option text and the given value
/// may reach a sentence is decided once, when it is built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedField {
    problem: FieldProblem,
    target: String,
    subject: ActionSubject,
    given: Option<String>,
    named: bool,
    credential: bool,
    follows_choice: bool,
}

impl RejectedField {
    /// Join the page's account of a field with what the service knows about it. `preserve_names`
    /// is the governed snapshot's `preserves_target_names()`.
    #[must_use]
    pub fn new(problem: FieldProblem, control: RejectedControl, preserve_names: bool) -> Self {
        let mut problem = problem;
        if control.credential {
            problem.options.clear();
            problem.options_total = None;
            problem.containing.clear();
            problem.now = None;
        }
        let subject = ActionSubject::with_role(control.role, &control.name, preserve_names);
        Self {
            problem,
            target: control.target,
            subject,
            given: (preserve_names && !control.credential).then_some(control.given),
            named: preserve_names,
            credential: control.credential,
            follows_choice: control.follows_choice,
        }
    }

    /// The fact that describes this field: its position, its handle, the problem word and what
    /// would succeed. It never holds the given value.
    #[must_use]
    pub fn fact(&self) -> Value {
        let mut fact = serde_json::to_value(&self.problem).unwrap_or(Value::Null);
        if let Value::Object(members) = &mut fact {
            members.insert("target".into(), json!(self.target));
        }
        fact
    }

    /// Whether the control is a radio button, which is named by its choice group.
    fn is_radio(&self) -> bool {
        self.subject.role() == TargetRole::Radio
    }

    /// The noun the clause uses for the control.
    fn noun(&self) -> &str {
        let input_type = self.problem.input_type.as_deref();
        match self.problem.kind {
            FieldProblemKind::WrongTool if input_type == Some(FILE_INPUT_TYPE) => "field",
            FieldProblemKind::WrongTool if self.subject.role() == TargetRole::Button => "control",
            FieldProblemKind::OptionMissing if self.is_radio() => "choice",
            _ => input_type
                .and_then(typed_noun)
                .unwrap_or_else(|| self.subject.role().noun()),
        }
    }

    /// The name the control goes by in a sentence, when the snapshot lets it be named.
    fn label(&self) -> Option<String> {
        if !self.named {
            return None;
        }
        if self.is_radio() {
            return self
                .problem
                .group
                .as_deref()
                .map(|group| page_text(group, PAGE_TEXT_MAX_CHARS))
                .filter(|group| !group.is_empty());
        }
        self.subject
            .label()
            .map(|label| page_text(label, PAGE_TEXT_MAX_CHARS))
    }

    /// The control as a sentence subject: `The "Role" dropdown` or `A dropdown`.
    fn subject_phrase(&self) -> String {
        match self.label() {
            Some(label) => format!("The \"{label}\" {}", self.noun()),
            None => format!("{} {}", capitalized(article(self.noun())), self.noun()),
        }
    }

    /// The control inside a sentence: `the "Role" dropdown` or `a dropdown`.
    fn inner_phrase(&self) -> String {
        match self.label() {
            Some(label) => format!("the \"{label}\" {}", self.noun()),
            None => format!("{} {}", article(self.noun()), self.noun()),
        }
    }

    /// The given value in quotes, bounded and on one line.
    fn quoted_given(&self) -> Option<String> {
        self.given
            .as_deref()
            .map(|given| format!("\"{}\"", page_text(given, GIVEN_MAX_CHARS)))
    }

    /// The enabled options the page listed, as sentence text.
    fn enabled_options(&self) -> Vec<String> {
        let mut out = Vec::new();
        for option in &self.problem.options {
            option.collect_enabled(&mut out);
        }
        out
    }

    /// How many options the select has: the page's count when it cut the list, otherwise the
    /// number listed.
    fn option_count(&self) -> usize {
        self.problem.options_total.unwrap_or_else(|| {
            let mut total = 0;
            for option in &self.problem.options {
                option.count_into(&mut total);
            }
            total
        })
    }

    /// What a reverted field holds, as sentence text: only when the snapshot lets values be named
    /// and the control is not a credential.
    fn reverted_value(&self) -> Option<String> {
        if !self.named || self.credential {
            return None;
        }
        let now = page_text(self.problem.now.as_deref()?, GIVEN_MAX_CHARS);
        Some(if now.is_empty() {
            NO_VALUE.to_owned()
        } else {
            format!("\"{now}\"")
        })
    }

    /// The control that went back and where, inside a sentence: `the "Role" dropdown went back
    /// to "Choose one..."`.
    fn reverted_phrase(&self) -> String {
        let control = self.inner_phrase();
        match self.reverted_value() {
            Some(value) => format!("{control} went back to {value}"),
            None => format!("{control} went back"),
        }
    }

    /// The sentence for this field.
    fn clause(&self) -> String {
        if self.credential && self.problem.kind != FieldProblemKind::Reverted {
            return format!("{} is credential-class.", self.subject_phrase());
        }
        match self.problem.kind {
            FieldProblemKind::OptionMissing => self.missing_clause(),
            FieldProblemKind::OptionAmbiguous => self.ambiguous_clause(),
            FieldProblemKind::OptionDisabled => match self.quoted_given() {
                Some(given) => {
                    format!("The option {given} of {} is disabled.", self.inner_phrase())
                }
                None => format!(
                    "The matching option of {} is disabled.",
                    self.inner_phrase()
                ),
            },
            FieldProblemKind::MultipleValues => {
                format!("{} takes one value, not a list.", self.subject_phrase())
            }
            FieldProblemKind::NotBoolean => match self.quoted_given() {
                Some(given) => format!(
                    "{} takes true or false, not {given}.",
                    self.subject_phrase()
                ),
                None => format!("{} takes true or false.", self.subject_phrase()),
            },
            FieldProblemKind::Format => self.format_clause(),
            FieldProblemKind::WrongTool => self.wrong_tool_clause(),
            FieldProblemKind::Reverted => {
                format!("{}.", capitalized(&self.reverted_phrase()))
            }
            FieldProblemKind::SubmitNotOwned => {
                "The submit control does not belong to the form that holds these fields.".into()
            }
        }
    }

    fn missing_clause(&self) -> String {
        let Some(given) = self.quoted_given() else {
            return format!("{} has no matching option.", self.subject_phrase());
        };
        let offered = self.enabled_options();
        let mut clause;
        if self.list_is_cut() {
            clause = format!(
                "{} has no option {given} among its {}.",
                self.subject_phrase(),
                counted(self.option_count(), "option", "options")
            );
            let similar = fitting(&self.problem.containing);
            if !similar.is_empty() {
                clause.push_str(&format!(
                    " Options containing {given}: {}.",
                    similar.join(", ")
                ));
            }
        } else {
            clause = format!("{} has no option {given}.", self.subject_phrase());
            if self.option_count() == 0 {
                clause.push_str(" It offers no options.");
            } else if offered.is_empty() {
                clause.push_str(" Every option is disabled.");
            } else {
                clause.push_str(&format!(" It offers: {}.", offered_phrase(&offered)));
            }
        }
        if self.follows_choice {
            clause.push_str(" Its options can depend on earlier fields.");
        }
        clause
    }

    /// Whether the page cut the list, so the facts do not hold every option. A list the page sent
    /// whole is never cut, even when its names are too wide for one clause.
    fn list_is_cut(&self) -> bool {
        self.problem.options_total.is_some()
    }

    fn ambiguous_clause(&self) -> String {
        let Some(given) = self.quoted_given() else {
            return format!("{} matches more than one option.", self.subject_phrase());
        };
        let mut candidates = Vec::new();
        for option in &self.problem.options {
            match option {
                OfferedOption::Entry { text, value, .. } => {
                    candidates.push(OfferedOption::shown(text, value.as_deref()));
                }
                OfferedOption::Text(text) => candidates.push(page_text(text, PAGE_TEXT_MAX_CHARS)),
                OfferedOption::Group { .. } => {}
            }
        }
        let mut count = 0;
        for option in &self.problem.options {
            option.count_into(&mut count);
        }
        format!(
            "{given} matches {count} options of {}: {}.",
            self.inner_phrase(),
            fitting(&candidates).join(", ")
        )
    }

    fn format_clause(&self) -> String {
        let mut clause = match (self.problem.expected.as_deref(), self.quoted_given()) {
            (Some(expected), Some(given)) => format!(
                "{} takes {}, not {given}.",
                self.subject_phrase(),
                page_text(expected, PAGE_TEXT_MAX_CHARS)
            ),
            (Some(expected), None) => format!(
                "{} takes {}.",
                self.subject_phrase(),
                page_text(expected, PAGE_TEXT_MAX_CHARS)
            ),
            (None, Some(given)) => {
                format!("{} does not accept {given}.", self.subject_phrase())
            }
            (None, None) => format!("{} does not accept that value.", self.subject_phrase()),
        };
        let limit = |key: &str| {
            self.problem
                .limits
                .get(key)
                .and_then(Value::as_str)
                .map(|text| page_text(text, PAGE_TEXT_MAX_CHARS))
                .filter(|text| !text.is_empty())
        };
        match (limit("min"), limit("max")) {
            (Some(min), Some(max)) => clause.push_str(&format!(" Allowed: {min} to {max}.")),
            (Some(min), None) => clause.push_str(&format!(" Allowed: from {min}.")),
            (None, Some(max)) => clause.push_str(&format!(" Allowed: up to {max}.")),
            (None, None) => {}
        }
        clause
    }

    fn wrong_tool_clause(&self) -> String {
        let phrase = self.subject_phrase();
        if self.problem.input_type.as_deref() == Some(FILE_INPUT_TYPE) {
            format!("{phrase} is a file input.")
        } else if self.subject.role() == TargetRole::Button {
            format!("{phrase} is a button, not a field.")
        } else {
            format!("{phrase} is a page widget, not a form field.")
        }
    }

    /// The step that fixes this field.
    fn next_step(&self) -> String {
        let target = &self.target;
        if self.credential && self.problem.kind != FieldProblemKind::Reverted {
            return format!(
                "Ask the user for the value, then repeat browser_fill_form with {target} set to it."
            );
        }
        match self.problem.kind {
            FieldProblemKind::OptionMissing if self.follows_choice => {
                format!("Fill the earlier fields first, then repeat for {target}.")
            }
            FieldProblemKind::OptionMissing if self.is_radio() => {
                format!("Repeat with one of the offered labels for {target}.")
            }
            FieldProblemKind::OptionMissing
                if self.enabled_options().is_empty() || self.list_is_cut() =>
            {
                format!("Read {target} with browser_read to list all its options.")
            }
            FieldProblemKind::OptionMissing => {
                format!("Repeat browser_fill_form with {target} set to one of the offered options.")
            }
            FieldProblemKind::OptionAmbiguous if self.is_radio() => {
                "Find the radio you mean with browser_find, then fill that radio's own handle with true."
                    .into()
            }
            FieldProblemKind::OptionAmbiguous => {
                format!("Repeat with the option's value for {target}.")
            }
            FieldProblemKind::OptionDisabled => {
                "Choose an enabled option, or inspect the form for the field that enables it."
                    .into()
            }
            FieldProblemKind::MultipleValues => {
                format!("Repeat browser_fill_form with {target} set to a single value.")
            }
            FieldProblemKind::NotBoolean => format!("Repeat with true or false for {target}."),
            FieldProblemKind::Format => {
                format!("Repeat browser_fill_form with {target} set to a corrected value.")
            }
            FieldProblemKind::WrongTool => {
                if self.problem.input_type.as_deref() == Some(FILE_INPUT_TYPE) {
                    format!("Attach the file with browser_upload on {target}.")
                } else if self.subject.role() == TargetRole::Button {
                    format!("Click it with browser_click on {target}.")
                } else {
                    format!(
                        "Open it with browser_click on {target}, then browser_click the option by name."
                    )
                }
            }
            FieldProblemKind::Reverted => {
                format!("Read {target} with browser_read to see its current value.")
            }
            FieldProblemKind::SubmitNotOwned => {
                "Repeat without submit_target, then click the submit control with browser_click."
                    .into()
            }
        }
    }
}

/// The summary of a fill that was refused before any edit: nothing changed, how many of the
/// call's fields cannot take their values, and one clause for each. Clauses the room does not
/// hold are counted, and the facts carry all of them.
#[must_use]
pub fn rejected_summary(fields: &[RejectedField], total: usize, max_chars: usize) -> String {
    let mut summary = if fields.len() == 1 {
        "Nothing was changed.".to_owned()
    } else {
        format!(
            "Nothing was changed: {} of {total} fields cannot take their values.",
            fields.len()
        )
    };
    let mut shown = 0;
    for (index, field) in fields.iter().enumerate() {
        let clause = field.clause();
        let last = index + 1 == fields.len();
        let room = if last {
            max_chars
        } else {
            max_chars.saturating_sub(TAIL_RESERVE_CHARS)
        };
        if summary.chars().count() + 1 + clause.chars().count() > room {
            break;
        }
        summary.push(' ');
        summary.push_str(&clause);
        shown += 1;
    }
    if shown < fields.len() {
        summary.push_str(&format!(" {} more are in the facts.", fields.len() - shown));
    }
    summary
}

/// The summary of a fill the page partly undid: how many fields held their values and which ones
/// went back after the page reacted. Fields past the first few are counted, and the facts carry
/// all of them.
#[must_use]
pub fn reverted_summary(set: usize, fields: &[RejectedField], max_chars: usize) -> String {
    let total = set + fields.len();
    let head = format!("Set {set} of {}; ", counted(total, "field", "fields"));
    let tail = " after the page reacted.";
    let mut phrases: Vec<String> = fields
        .iter()
        .take(REVERTED_SHOWN_MAX)
        .map(RejectedField::reverted_phrase)
        .collect();
    while phrases.len() > 1
        && head.chars().count()
            + tail.chars().count()
            + phrases
                .iter()
                .map(|phrase| phrase.chars().count() + 5)
                .sum::<usize>()
            > max_chars
    {
        phrases.pop();
    }
    let hidden = fields.len() - phrases.len();
    if hidden > 0 {
        phrases.push(format!(
            "{} went back too",
            counted(hidden, "more field", "more fields")
        ));
    }
    let last = phrases.pop().unwrap_or_default();
    let list = if phrases.is_empty() {
        last
    } else {
        format!("{} and {last}", phrases.join(", "))
    };
    format!("{head}{list}{tail}")
}

/// The facts of a fill the page partly undid: the reason, how many fields held, and one entry for
/// each field that went back.
#[must_use]
pub fn reverted_facts(set: usize, fields: &[RejectedField]) -> Value {
    json!({
        "reason": REVERTED_REASON,
        "set": set,
        "reverted": fields.iter().map(RejectedField::fact).collect::<Vec<_>>(),
    })
}

/// The steps for a refused fill: those of the first fields, at most two.
#[must_use]
pub fn rejected_next_steps(fields: &[RejectedField]) -> Vec<String> {
    fields
        .iter()
        .take(NEXT_STEPS_MAX)
        .map(RejectedField::next_step)
        .collect()
}

/// The facts of a refused fill: the reason and one entry for each field.
#[must_use]
pub fn rejected_facts(fields: &[RejectedField]) -> Value {
    json!({
        "reason": REJECTED_REASON,
        "problems": fields.iter().map(RejectedField::fact).collect::<Vec<_>>(),
    })
}

/// A control a refusal names so the person or the model can find it: its role, its governed
/// name and, when there is one, its handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlRef {
    subject: ActionSubject,
    handle: Option<String>,
}

impl ControlRef {
    /// Name a control by its role and the name the page reported. The name is kept only when
    /// the governed snapshot preserves target names.
    #[must_use]
    pub fn new(role: TargetRole, name: &str, handle: Option<&str>, preserve_names: bool) -> Self {
        Self {
            subject: ActionSubject::with_role(role, name, preserve_names),
            handle: handle.map(str::to_owned),
        }
    }

    /// The control as a clause subject: `The "Password" text field (target_x)` or `A text field`.
    #[must_use]
    pub fn phrase(&self) -> String {
        let noun = self.subject.role().noun();
        let mut phrase = match self.subject.label() {
            Some(label) => format!("The \"{}\" {noun}", page_text(label, PAGE_TEXT_MAX_CHARS)),
            None => format!("{} {noun}", capitalized(article(noun))),
        };
        if let Some(handle) = &self.handle {
            phrase.push_str(&format!(" ({handle})"));
        }
        phrase
    }
}

/// The input types that read better than their role: `date field` rather than `text field`.
fn typed_noun(input_type: &str) -> Option<&'static str> {
    Some(match input_type {
        "date" => "date field",
        "time" => "time field",
        "datetime-local" => "date and time field",
        "month" => "month field",
        "week" => "week field",
        "number" => "number field",
        "email" => "email field",
        "url" => "URL field",
        "tel" => "phone number field",
        "color" => "color field",
        _ => return None,
    })
}

fn capitalized(word: &str) -> String {
    let mut characters = word.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// Page text as it may stand in a sentence: one line, bounded, quotes replaced, and anything
/// shaped like a handle replaced by a marker so page text cannot pose as a reference.
pub(super) fn page_text(value: &str, max_chars: usize) -> String {
    one_line_bounded(value, max_chars)
        .unwrap_or_default()
        .split(' ')
        .map(|word| {
            if is_handle_shaped(word) {
                HANDLE_REPLACEMENT
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_handle_shaped(word: &str) -> bool {
    let word = word
        .trim_matches(|character: char| !(character.is_ascii_alphanumeric() || character == '_'));
    HANDLE_PREFIXES.iter().any(|prefix| {
        word.strip_prefix(prefix).is_some_and(|suffix| {
            suffix.len() == HANDLE_SUFFIX_CHARS
                && suffix
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c))
        })
    })
}

/// The options a clause offers: all of them when they fit the room, otherwise the leading ones
/// and how many more the facts hold.
fn offered_phrase(offered: &[String]) -> String {
    let kept = fitting(offered);
    let rest = offered.len() - kept.len();
    if rest == 0 {
        kept.join(", ")
    } else {
        format!("{} and {rest} more, all in the facts", kept.join(", "))
    }
}

/// The leading items of a list whose joined text stays inside one clause's room. Page text is
/// made safe for a sentence on the way.
fn fitting(items: &[String]) -> Vec<String> {
    let mut used = 0;
    let mut kept = Vec::new();
    for item in items {
        let item = page_text(item, PAGE_TEXT_MAX_CHARS);
        used += item.chars().count() + 2;
        if used > LIST_MAX_CHARS && !kept.is_empty() {
            break;
        }
        kept.push(item);
    }
    kept
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::language::outcome::Refusal;

    const HANDLE: &str = "target_7k2nb5xe";
    const OTHER_HANDLE: &str = "target_q3m5c2ra";

    fn decode(problem: Value) -> FieldProblem {
        FieldProblems::decode(&json!({ "problems": [problem] }))
            .expect("the problem decodes")
            .problems
            .remove(0)
    }

    fn control(role: TargetRole, name: &str, given: &str) -> RejectedControl {
        RejectedControl {
            target: HANDLE.into(),
            role,
            name: name.into(),
            given: given.into(),
            credential: false,
            follows_choice: false,
        }
    }

    fn field(
        problem: Value,
        role: TargetRole,
        name: &str,
        given: &str,
        preserve: bool,
    ) -> RejectedField {
        RejectedField::new(decode(problem), control(role, name, given), preserve)
    }

    fn one(problem: Value, role: TargetRole, name: &str, given: &str, preserve: bool) -> Refusal {
        Refusal::FieldValuesRejected {
            fields: vec![field(problem, role, name, given, preserve)],
            total: 1,
        }
    }

    fn missing_role(options: Value) -> Value {
        json!({"field": 0, "problem": "option_missing", "options": options})
    }

    #[test]
    fn a_short_list_is_offered_in_the_summary_and_the_step_names_the_handle() {
        let refusal = one(
            missing_role(json!(["Developer", "Administrator", "Researcher", "Other"])),
            TargetRole::Combobox,
            "Role",
            "Engineer",
            true,
        );
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Role\" dropdown has no option \"Engineer\". It offers: Developer, Administrator, Researcher, Other."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Repeat browser_fill_form with {HANDLE} set to one of the offered options."
            )]
        );
    }

    #[test]
    fn a_long_list_gives_its_count_and_the_options_that_contain_the_value() {
        let problem = json!({
            "field": 0, "problem": "option_missing", "options": ["Alpha", "Beta"],
            "options_total": 195, "containing": ["Region East", "Region West"]
        });
        let refusal = one(problem, TargetRole::Combobox, "Region", "region", true);
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Region\" dropdown has no option \"region\" among its 195 options. Options containing \"region\": Region East, Region West."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Read {HANDLE} with browser_read to list all its options."
            )]
        );
        let none = one(
            json!({"field": 0, "problem": "option_missing", "options": ["Alpha"], "options_total": 195}),
            TargetRole::Combobox,
            "Region",
            "zzz",
            true,
        );
        assert_eq!(
            none.summary(),
            "Nothing was changed. The \"Region\" dropdown has no option \"zzz\" among its 195 options."
        );
    }

    #[test]
    fn a_whole_list_too_wide_for_one_clause_names_the_rest_and_keeps_the_repeat_step() {
        let options: Vec<String> = (0..25)
            .map(|index| format!("{}{index}", "w".repeat(50)))
            .collect();
        let refusal = one(
            missing_role(json!(options)),
            TargetRole::Combobox,
            "Wide",
            "x",
            true,
        );
        let summary = refusal.summary();
        assert!(
            summary.contains("has no option \"x\". It offers: "),
            "{summary}"
        );
        assert!(summary.contains(" more, all in the facts."), "{summary}");
        assert!(!summary.contains("among its"), "{summary}");
        assert!(summary.chars().count() <= 500);
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Repeat browser_fill_form with {HANDLE} set to one of the offered options."
            )]
        );
        let Refusal::FieldValuesRejected { fields, .. } = &refusal else {
            unreachable!()
        };
        assert_eq!(
            rejected_facts(fields)["problems"][0]["options"]
                .as_array()
                .map(Vec::len),
            Some(25)
        );
    }

    #[test]
    fn options_the_transport_dropped_are_never_spoken_as_an_empty_select() {
        let dropped = one(
            json!({"field": 0, "problem": "option_missing", "options_total": 4}),
            TargetRole::Combobox,
            "Role",
            "Engineer",
            true,
        );
        assert_eq!(
            dropped.summary(),
            "Nothing was changed. The \"Role\" dropdown has no option \"Engineer\" among its 4 options."
        );
        assert!(!dropped.summary().contains("offers no options"));
        assert_eq!(
            dropped.next_steps(),
            vec![format!(
                "Read {HANDLE} with browser_read to list all its options."
            )]
        );
    }

    #[test]
    fn a_choice_earlier_in_the_call_says_the_options_can_depend_on_it() {
        let mut dependent = control(TargetRole::Combobox, "City", "Paris");
        dependent.follows_choice = true;
        let refusal = Refusal::FieldValuesRejected {
            fields: vec![RejectedField::new(
                decode(missing_role(json!(["Lyon", "Nice"]))),
                dependent,
                true,
            )],
            total: 2,
        };
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"City\" dropdown has no option \"Paris\". It offers: Lyon, Nice. Its options can depend on earlier fields."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Fill the earlier fields first, then repeat for {HANDLE}."
            )]
        );
    }

    #[test]
    fn disabled_options_are_not_offered() {
        let options = json!([
            "Free",
            {"text": "Pro", "disabled": true},
            {"group": "Staff", "options": ["Team"]}
        ]);
        let refusal = one(
            missing_role(options),
            TargetRole::Combobox,
            "Plan",
            "Gold",
            true,
        );
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Plan\" dropdown has no option \"Gold\". It offers: Free, Team."
        );
        let all_disabled = one(
            missing_role(json!([{"text": "Pro", "disabled": true}])),
            TargetRole::Combobox,
            "Plan",
            "Gold",
            true,
        );
        assert!(all_disabled
            .summary()
            .ends_with("Every option is disabled."));
        let empty = one(
            missing_role(json!([])),
            TargetRole::Combobox,
            "Plan",
            "Gold",
            true,
        );
        assert!(empty.summary().ends_with("It offers no options."));
        assert!(empty.next_steps()[0].starts_with("Read "));
    }

    #[test]
    fn an_ambiguous_value_lists_the_candidates_with_their_values() {
        let problem = json!({"field": 0, "problem": "option_ambiguous", "options": [
            {"text": "Other", "value": "other-staff"}, {"text": "Other", "value": "other-guest"}]});
        let refusal = one(problem, TargetRole::Combobox, "Kind", "other", true);
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. \"other\" matches 2 options of the \"Kind\" dropdown: Other (value other-staff), Other (value other-guest)."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!("Repeat with the option's value for {HANDLE}.")]
        );
    }

    #[test]
    fn a_disabled_option_is_named_with_the_dropdown() {
        let refusal = one(
            json!({"field": 0, "problem": "option_disabled"}),
            TargetRole::Combobox,
            "Plan",
            "Pro",
            true,
        );
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The option \"Pro\" of the \"Plan\" dropdown is disabled."
        );
        assert_eq!(
            refusal.next_steps(),
            vec!["Choose an enabled option, or inspect the form for the field that enables it."]
        );
    }

    #[test]
    fn a_list_for_a_single_choice_and_a_word_for_a_checkbox() {
        let many = one(
            json!({"field": 0, "problem": "multiple_values"}),
            TargetRole::Combobox,
            "Role",
            "a, b",
            true,
        );
        assert_eq!(
            many.summary(),
            "Nothing was changed. The \"Role\" dropdown takes one value, not a list."
        );
        let flag = one(
            json!({"field": 0, "problem": "not_boolean"}),
            TargetRole::Checkbox,
            "Agree",
            "maybe",
            true,
        );
        assert_eq!(
            flag.summary(),
            "Nothing was changed. The \"Agree\" checkbox takes true or false, not \"maybe\"."
        );
        assert_eq!(
            flag.next_steps(),
            vec![format!("Repeat with true or false for {HANDLE}.")]
        );
    }

    #[test]
    fn a_radio_is_named_by_its_group_and_offers_its_labels() {
        let problem = json!({"field": 0, "problem": "option_missing",
            "options": ["Standard", "Express"], "group": "Shipping"});
        let refusal = one(problem, TargetRole::Radio, "Standard", "Overnight", true);
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Shipping\" choice has no option \"Overnight\". It offers: Standard, Express."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Repeat with one of the offered labels for {HANDLE}."
            )]
        );
    }

    #[test]
    fn two_radios_with_one_label_send_the_model_to_the_radio_it_means() {
        let problem = json!({"field": 0, "problem": "option_ambiguous",
            "options": ["Yes", "Yes"], "group": "Subscribe"});
        let refusal = one(problem, TargetRole::Radio, "No", "yes", true);
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. \"yes\" matches 2 options of the \"Subscribe\" radio button: Yes, Yes."
        );
        assert_eq!(
            refusal.next_steps(),
            vec!["Find the radio you mean with browser_find, then fill that radio's own handle with true.".to_owned()]
        );
    }

    #[test]
    fn a_format_problem_names_the_form_and_the_allowed_range() {
        let problem = json!({"field": 0, "problem": "format", "expected": "YYYY-MM-DD", "type": "date",
            "limits": {"min": "2026-10-01", "max": "2026-12-31"}});
        let refusal = one(problem, TargetRole::Textbox, "Start", "next week", true);
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Start\" date field takes YYYY-MM-DD, not \"next week\". Allowed: 2026-10-01 to 2026-12-31."
        );
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Repeat browser_fill_form with {HANDLE} set to a corrected value."
            )]
        );
        let open = one(
            json!({"field": 0, "problem": "format", "limits": {"min": "5"}}),
            TargetRole::NumberField,
            "Count",
            "x",
            true,
        );
        assert_eq!(
            open.summary(),
            "Nothing was changed. The \"Count\" number field does not accept \"x\". Allowed: from 5."
        );
    }

    #[test]
    fn a_control_a_fill_cannot_use_points_at_the_tool_that_can() {
        let file = one(
            json!({"field": 0, "problem": "wrong_tool", "type": "file"}),
            TargetRole::Textbox,
            "Resume",
            "cv.pdf",
            true,
        );
        assert_eq!(
            file.summary(),
            "Nothing was changed. The \"Resume\" field is a file input."
        );
        assert_eq!(
            file.next_steps(),
            vec![format!("Attach the file with browser_upload on {HANDLE}.")]
        );
        let button = one(
            json!({"field": 0, "problem": "wrong_tool"}),
            TargetRole::Button,
            "Save",
            "x",
            true,
        );
        assert_eq!(
            button.summary(),
            "Nothing was changed. The \"Save\" control is a button, not a field."
        );
        assert_eq!(
            button.next_steps(),
            vec![format!("Click it with browser_click on {HANDLE}.")]
        );
        let widget = one(
            json!({"field": 0, "problem": "wrong_tool"}),
            TargetRole::Combobox,
            "Country",
            "x",
            true,
        );
        assert_eq!(
            widget.summary(),
            "Nothing was changed. The \"Country\" dropdown is a page widget, not a form field."
        );
        assert_eq!(
            widget.next_steps(),
            vec![format!(
                "Open it with browser_click on {HANDLE}, then browser_click the option by name."
            )]
        );
    }

    #[test]
    fn a_foreign_submit_control_has_its_sentence() {
        let submit = one(
            json!({"field": 0, "problem": "submit_not_owned"}),
            TargetRole::Button,
            "Go",
            "x",
            true,
        );
        assert_eq!(
            submit.summary(),
            "Nothing was changed. The submit control does not belong to the form that holds these fields."
        );
    }

    fn reverted(problem: Value, role: TargetRole, name: &str, preserve: bool) -> RejectedField {
        let mut problem = problem;
        problem["field"] = json!(0);
        problem["problem"] = json!("reverted");
        field(problem, role, name, "given", preserve)
    }

    #[test]
    fn a_reverted_field_is_named_with_what_it_holds_now() {
        let refusal = Refusal::FieldsReverted {
            set: 2,
            reverted: vec![reverted(
                json!({"now": "Choose one..."}),
                TargetRole::Combobox,
                "Role",
                true,
            )],
        };
        assert_eq!(
            refusal.summary(),
            "Set 2 of 3 fields; the \"Role\" dropdown went back to \"Choose one...\" after the page reacted."
        );
        assert_eq!(
            refusal.next_steps(),
            [format!(
                "Read {HANDLE} with browser_read to see its current value."
            )]
        );
        let Refusal::FieldsReverted { set, reverted } = &refusal else {
            panic!("a reverted fill");
        };
        assert_eq!(
            reverted_facts(*set, reverted),
            json!({"reason": "fields_reverted", "set": 2, "reverted": [
                {"field": 0, "problem": "reverted", "now": "Choose one...", "target": HANDLE}
            ]})
        );
    }

    #[test]
    fn several_reverted_fields_read_as_one_list_and_the_rest_are_counted() {
        let many = |count: usize| Refusal::FieldsReverted {
            set: 1,
            reverted: (0..count)
                .map(|index| {
                    reverted(
                        json!({"now": format!("Old {index}")}),
                        TargetRole::Combobox,
                        &format!("Field {index}"),
                        true,
                    )
                })
                .collect(),
        };
        assert_eq!(
            many(2).summary(),
            "Set 1 of 3 fields; the \"Field 0\" dropdown went back to \"Old 0\" and the \"Field 1\" dropdown went back to \"Old 1\" after the page reacted."
        );
        let five = many(5).summary();
        assert!(five.starts_with("Set 1 of 6 fields; the \"Field 0\" dropdown"));
        assert!(
            five.ends_with("and 2 more fields went back too after the page reacted."),
            "{five}"
        );
        assert_eq!(many(5).next_steps().len(), 2);
    }

    #[test]
    fn a_reverted_field_names_nothing_when_names_are_not_preserved() {
        let refusal = Refusal::FieldsReverted {
            set: 0,
            reverted: vec![reverted(
                json!({"now": "PRIVATE_VALUE"}),
                TargetRole::Combobox,
                "PRIVATE_NAME",
                false,
            )],
        };
        let summary = refusal.summary();
        assert_eq!(
            summary,
            "Set 0 of 1 field; a dropdown went back after the page reacted."
        );
        assert!(!refusal.audit().summary().contains("PRIVATE"));
    }

    #[test]
    fn a_reverted_credential_never_carries_what_it_holds() {
        let mut secret = control(TargetRole::Textbox, "Password", "PRIVATE_GIVEN");
        secret.credential = true;
        let field = RejectedField::new(
            decode(json!({"field": 0, "problem": "reverted", "now": "PRIVATE_NOW"})),
            secret,
            true,
        );
        let fact = field.fact().to_string();
        assert!(!fact.contains("PRIVATE"), "{fact}");
        let refusal = Refusal::FieldsReverted {
            set: 1,
            reverted: vec![field],
        };
        assert_eq!(
            refusal.summary(),
            "Set 1 of 2 fields; the \"Password\" text field went back after the page reacted."
        );
        assert_eq!(
            refusal.next_steps(),
            [format!(
                "Read {HANDLE} with browser_read to see its current value."
            )]
        );
    }

    #[test]
    fn a_reverted_value_is_bounded_and_empty_reads_as_no_value() {
        let long = "y".repeat(100);
        let bounded = Refusal::FieldsReverted {
            set: 0,
            reverted: vec![reverted(
                json!({ "now": long }),
                TargetRole::Textbox,
                "Note",
                true,
            )],
        };
        assert!(bounded
            .summary()
            .contains(&format!("\"{}...\"", "y".repeat(37))));
        let empty = Refusal::FieldsReverted {
            set: 0,
            reverted: vec![reverted(
                json!({"now": ""}),
                TargetRole::Textbox,
                "Note",
                true,
            )],
        };
        assert!(empty.summary().contains("went back to no value"));
    }

    #[test]
    fn a_credential_class_control_lists_nothing_and_echoes_nothing() {
        let mut secret = control(TargetRole::Combobox, "Token kind", "PRIVATE_VALUE");
        secret.credential = true;
        let field = RejectedField::new(
            decode(missing_role(json!(["PRIVATE_OPTION"]))),
            secret,
            true,
        );
        let fact = field.fact();
        let refusal = Refusal::FieldValuesRejected {
            fields: vec![field],
            total: 1,
        };
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Token kind\" dropdown is credential-class."
        );
        let everything = format!("{} {:?} {fact}", refusal.summary(), refusal.next_steps());
        assert!(!everything.contains("PRIVATE"), "{everything}");
        assert_eq!(fact.get("options"), None);
    }

    #[test]
    fn without_name_governance_no_page_text_or_given_value_reaches_a_sentence() {
        let cases = [
            missing_role(json!(["PRIVATE_OPTION"])),
            json!({"field": 0, "problem": "option_missing", "options": ["PRIVATE_OPTION"],
                "options_total": 99, "containing": ["PRIVATE_OPTION"]}),
            json!({"field": 0, "problem": "option_ambiguous",
                "options": [{"text": "PRIVATE_OPTION", "value": "PRIVATE_OPTION"}]}),
            json!({"field": 0, "problem": "option_disabled"}),
            json!({"field": 0, "problem": "multiple_values"}),
            json!({"field": 0, "problem": "not_boolean"}),
            json!({"field": 0, "problem": "format", "expected": "YYYY-MM-DD", "type": "date"}),
            json!({"field": 0, "problem": "wrong_tool", "type": "file", "group": "PRIVATE_GROUP"}),
            json!({"field": 0, "problem": "reverted"}),
        ];
        for problem in cases {
            for role in [TargetRole::Combobox, TargetRole::Radio, TargetRole::Textbox] {
                let refusal = one(
                    problem.clone(),
                    role,
                    "PRIVATE_NAME",
                    "PRIVATE_VALUE",
                    false,
                );
                let steps = refusal.next_steps();
                let text = format!("{} {steps:?}", refusal.summary());
                assert!(!text.contains("PRIVATE"), "{problem}: {text}");
            }
        }
        let refusal = one(
            missing_role(json!(["PRIVATE_OPTION"])),
            TargetRole::Combobox,
            "PRIVATE_NAME",
            "PRIVATE_VALUE",
            false,
        );
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. A dropdown has no matching option."
        );
    }

    #[test]
    fn facts_hold_the_options_and_the_handle_but_never_the_given_value() {
        let refusal = one(
            missing_role(json!(["Developer", "Other"])),
            TargetRole::Combobox,
            "Role",
            "PRIVATE_VALUE",
            false,
        );
        let Refusal::FieldValuesRejected { fields, .. } = &refusal else {
            unreachable!()
        };
        let facts = rejected_facts(fields);
        assert_eq!(
            facts,
            json!({"reason": "field_value_rejected", "problems": [
                {"field": 0, "target": HANDLE, "problem": "option_missing",
                 "options": ["Developer", "Other"]}
            ]})
        );
        assert!(!facts.to_string().contains("PRIVATE_VALUE"));
    }

    #[test]
    fn several_problems_are_counted_and_each_gets_a_clause_and_at_most_two_steps() {
        let first = field(
            missing_role(json!(["A", "B"])),
            TargetRole::Combobox,
            "Role",
            "x",
            true,
        );
        let mut other = control(TargetRole::Checkbox, "Agree", "maybe");
        other.target = OTHER_HANDLE.into();
        let second = RejectedField::new(
            decode(json!({"field": 3, "problem": "not_boolean"})),
            other,
            true,
        );
        let third = field(
            json!({"field": 4, "problem": "multiple_values"}),
            TargetRole::Combobox,
            "Team",
            "x",
            true,
        );
        let refusal = Refusal::FieldValuesRejected {
            fields: vec![first, second, third],
            total: 5,
        };
        assert_eq!(
            refusal.summary(),
            "Nothing was changed: 3 of 5 fields cannot take their values. The \"Role\" dropdown has no option \"x\". It offers: A, B. The \"Agree\" checkbox takes true or false, not \"maybe\". The \"Team\" dropdown takes one value, not a list."
        );
        let steps = refusal.next_steps();
        assert_eq!(steps.len(), 2);
        assert!(steps[1].contains(OTHER_HANDLE));
    }

    #[test]
    fn the_summary_fits_the_cap_and_counts_what_it_left_out() {
        let fields: Vec<RejectedField> = (0..12)
            .map(|index| {
                field(
                    json!({"field": index, "problem": "option_missing",
                        "options": ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta"]}),
                    TargetRole::Combobox,
                    &format!("A fairly long control name number {index}"),
                    "a value of some length",
                    true,
                )
            })
            .collect();
        let refusal = Refusal::FieldValuesRejected { fields, total: 12 };
        let summary = refusal.summary();
        assert!(
            summary.chars().count() <= crate::language::outcome::SUMMARY_MAX_CHARS,
            "{}",
            summary.chars().count()
        );
        assert!(
            summary.starts_with("Nothing was changed: 12 of 12 fields cannot take their values.")
        );
        assert!(summary.ends_with(" more are in the facts."), "{summary}");
        assert!(summary.is_ascii());
    }

    #[test]
    fn page_text_in_a_sentence_is_one_bounded_line_without_quotes_or_handles() {
        let nasty = format!("Line\none \"quoted\" {HANDLE} {}", "x".repeat(100));
        let refusal = one(
            missing_role(json!([nasty.clone()])),
            TargetRole::Combobox,
            &nasty,
            &"g".repeat(100),
            true,
        );
        let summary = refusal.summary();
        assert!(!summary.contains('\n'));
        assert!(!summary.contains(HANDLE));
        assert!(summary.contains("[handle]"));
        assert!(summary.contains("'quoted'"));
        assert!(
            summary.contains(&format!("\"{}...\"", "g".repeat(37))),
            "{summary}"
        );
    }

    #[test]
    fn the_audit_holds_a_count_and_nothing_the_page_or_the_caller_said() {
        let refusal = one(
            missing_role(json!(["PRIVATE_OPTION"])),
            TargetRole::Combobox,
            "PRIVATE_NAME",
            "PRIVATE_VALUE",
            true,
        );
        let audit = refusal.audit();
        assert_eq!(
            audit.summary(),
            "Refused a form fill before any edit: 1 field could not take its value."
        );
        assert_eq!(
            serde_json::to_value(audit.refusal()).unwrap(),
            json!({"reason": "field_values_rejected"})
        );
        let two = Refusal::FieldValuesRejected {
            fields: vec![
                field(
                    missing_role(json!([])),
                    TargetRole::Combobox,
                    "A",
                    "x",
                    true,
                ),
                field(
                    missing_role(json!([])),
                    TargetRole::Combobox,
                    "B",
                    "x",
                    true,
                ),
            ],
            total: 4,
        };
        assert_eq!(
            two.audit().summary(),
            "Refused a form fill before any edit: 2 fields could not take their values."
        );
    }

    #[test]
    fn a_credential_refusal_names_the_control_and_the_handle() {
        let named = Refusal::CredentialAuthorization {
            control: ControlRef::new(TargetRole::Textbox, "Password", Some(HANDLE), true),
        };
        assert_eq!(
            named.summary(),
            format!(
                "Nothing was typed. The \"Password\" text field ({HANDLE}) is credential-class."
            )
        );
        assert_eq!(
            named.next_steps(),
            vec!["If the user explicitly authorized it, repeat with user_authorized_credentials true; otherwise ask the user to type it."]
        );
        let unnamed = Refusal::CredentialAuthorization {
            control: ControlRef::new(TargetRole::Textbox, "Password", None, false),
        };
        assert_eq!(
            unnamed.summary(),
            "Nothing was typed. A text field is credential-class."
        );
        assert!(!named.audit().summary().contains("Password"));
    }

    #[test]
    fn every_problem_word_round_trips_and_has_a_sentence() {
        let kinds = [
            FieldProblemKind::OptionMissing,
            FieldProblemKind::OptionAmbiguous,
            FieldProblemKind::OptionDisabled,
            FieldProblemKind::MultipleValues,
            FieldProblemKind::NotBoolean,
            FieldProblemKind::Format,
            FieldProblemKind::WrongTool,
            FieldProblemKind::Reverted,
            FieldProblemKind::SubmitNotOwned,
        ];
        for kind in kinds {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(kind.as_str()));
            let refusal = one(
                json!({"field": 0, "problem": kind.as_str()}),
                TargetRole::Combobox,
                "Role",
                "x",
                true,
            );
            assert!(refusal.summary().is_ascii());
            assert!(refusal.summary().len() > "Nothing was changed.".len());
            assert!(!refusal.next_steps().is_empty());
        }
    }

    #[test]
    fn a_detail_with_an_unknown_word_or_no_problems_does_not_decode() {
        assert!(
            FieldProblems::decode(&json!({"problems": [{"field": 0, "problem": "invented"}]}))
                .is_none()
        );
        assert!(FieldProblems::decode(&json!({"problems": []})).is_none());
        assert!(FieldProblems::decode(&json!("text")).is_none());
    }
}
