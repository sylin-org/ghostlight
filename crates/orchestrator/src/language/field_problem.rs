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

use super::handle::HandleKind;
use super::outcome::{ActionSubject, TargetRole};
use super::phrase::{article, counted, one_line_bounded, NO_VALUE};

/// The longest part of a given value a sentence quotes.
const GIVEN_MAX_CHARS: usize = 40;
/// The longest page text one option, group or limit contributes to a sentence.
const PAGE_TEXT_MAX_CHARS: usize = 60;
/// The most characters a listed set of options may take in one clause.
const LIST_MAX_CHARS: usize = 140;
/// The characters held back for the tail that counts clauses the summary had no room for.
const TAIL_RESERVE_CHARS: usize = 40;
/// What page text shaped like a handle becomes in a sentence.
const HANDLE_REPLACEMENT: &str = "[handle]";
/// The input type of a range, whose refusal names the step it snaps to.
const RANGE_INPUT_TYPE: &str = "range";
/// The `reason` fact of a fill that was refused before any edit.
pub(super) const REJECTED_REASON: &str = "field_value_rejected";
/// The `reason` fact of a fill the page partly undid.
pub(super) const REVERTED_REASON: &str = "fields_reverted";
/// The most reverted fields a summary describes one by one.
const REVERTED_SHOWN_MAX: usize = 3;
/// The most next steps a result carries.
const NEXT_STEPS_MAX: usize = 2;
/// The most handles one shared step names before it counts the rest.
const HANDLES_LISTED_MAX: usize = 4;

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

/// The tool a control that a fill cannot use is left to, as the page found it in the element
/// itself. The control's role is no guide: an image input reads as a text field and a button
/// with the role `switch` reads as a checkbox.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WrongToolRoute {
    /// A file input, which `browser_upload` attaches a file to.
    File,
    /// A button, or a submit, reset or image input, which `browser_click` presses.
    Button,
    /// A page widget with a combobox or listbox role, which `browser_click` opens.
    Widget,
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
    /// The constraints the control declares, or for a range the bounds and step it applies.
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
    /// The tool a wrong-tool control is left to. The page always names one for that problem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<WrongToolRoute>,
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
    /// word included, or a wrong-tool problem that names no route, gives none, so the refusal
    /// stays the generic one.
    #[must_use]
    pub fn decode(detail: &Value) -> Option<Self> {
        serde_json::from_value::<Self>(detail.clone())
            .ok()
            .filter(|decoded| !decoded.problems.is_empty())
            .filter(|decoded| {
                decoded.problems.iter().all(|problem| {
                    problem.kind != FieldProblemKind::WrongTool || problem.route.is_some()
                })
            })
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

    /// The tool a wrong-tool control is left to. The decoder refuses a wrong-tool problem without
    /// a route, so the widget reading here is never a guess about a page's answer.
    fn route(&self) -> WrongToolRoute {
        self.problem.route.unwrap_or(WrongToolRoute::Widget)
    }

    /// The noun the clause uses for the control.
    fn noun(&self) -> &str {
        let input_type = self.problem.input_type.as_deref();
        match self.problem.kind {
            FieldProblemKind::WrongTool => match self.route() {
                WrongToolRoute::File => "field",
                WrongToolRoute::Button => "control",
                WrongToolRoute::Widget => self.subject.role().noun(),
            },
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
        let bounds = match (limit("min"), limit("max")) {
            (Some(min), Some(max)) => Some(format!("{min} to {max}")),
            (Some(min), None) => Some(format!("from {min}")),
            (None, Some(max)) => Some(format!("up to {max}")),
            (None, None) => None,
        };
        // Only a range moves a value to its step. Any other typed input keeps a value off its
        // step, so naming the step there would refuse what the page accepts.
        let step = (self.problem.input_type.as_deref() == Some(RANGE_INPUT_TYPE))
            .then(|| limit("step"))
            .flatten();
        match (bounds, step) {
            (Some(bounds), Some(step)) => {
                clause.push_str(&format!(" Allowed: {bounds}, in steps of {step}."));
            }
            (Some(bounds), None) => clause.push_str(&format!(" Allowed: {bounds}.")),
            (None, Some(step)) => clause.push_str(&format!(" Allowed: steps of {step}.")),
            (None, None) => {}
        }
        clause
    }

    fn wrong_tool_clause(&self) -> String {
        let phrase = self.subject_phrase();
        match self.route() {
            WrongToolRoute::File => format!("{phrase} is a file input."),
            WrongToolRoute::Button => format!("{phrase} is a button, not a field."),
            WrongToolRoute::Widget => format!("{phrase} is a page widget, not a form field."),
        }
    }

    /// Whether repeating the fill with a different value fixes this field, so its step can be
    /// shared with the other fields that need the same.
    fn needs_corrected_value(&self) -> bool {
        !self.credential
            && matches!(
                self.problem.kind,
                FieldProblemKind::Format
                    | FieldProblemKind::NotBoolean
                    | FieldProblemKind::MultipleValues
            )
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
            FieldProblemKind::WrongTool => match self.route() {
                WrongToolRoute::File => format!("Attach the file with browser_upload on {target}."),
                WrongToolRoute::Button => format!("Click it with browser_click on {target}."),
                WrongToolRoute::Widget => format!(
                    "Open it with browser_click on {target}, then browser_click the option by name."
                ),
            },
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

/// The steps for a refused fill, at most two. Fields whose value only needs correcting share one
/// step that names every handle, placed where the first of them stands; every other field
/// brings its own step.
#[must_use]
pub fn rejected_next_steps(fields: &[RejectedField]) -> Vec<String> {
    let mut steps = Vec::new();
    let mut correctable: Vec<&RejectedField> = Vec::new();
    let mut shared_at = 0;
    for field in fields {
        if field.needs_corrected_value() {
            if correctable.is_empty() {
                shared_at = steps.len();
                steps.push(String::new());
            }
            correctable.push(field);
        } else {
            steps.push(field.next_step());
        }
    }
    match correctable.as_slice() {
        [] => {}
        [only] => steps[shared_at] = only.next_step(),
        several => {
            let handles: Vec<&str> = several.iter().map(|field| field.target.as_str()).collect();
            steps[shared_at] = format!(
                "Repeat browser_fill_form with corrected values for {}.",
                handle_list(&handles)
            );
        }
    }
    steps.truncate(NEXT_STEPS_MAX);
    steps
}

/// Handles as a sentence list: `a`, `a and b`, `a, b and c`, and past the room the leading
/// ones and how many more.
fn handle_list(handles: &[&str]) -> String {
    match handles {
        [] => String::new(),
        [only] => (*only).to_owned(),
        _ if handles.len() > HANDLES_LISTED_MAX => format!(
            "{} and {} more",
            handles[..HANDLES_LISTED_MAX].join(", "),
            handles.len() - HANDLES_LISTED_MAX
        ),
        [leading @ .., last] => format!("{} and {last}", leading.join(", ")),
    }
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

    /// The same control by its role alone, with no page name and no handle, as durable history
    /// keeps it.
    #[must_use]
    pub const fn unnamed(&self) -> Self {
        Self {
            subject: ActionSubject::unnamed(self.subject.role()),
            handle: None,
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
    HandleKind::IN_PAGE_TEXT
        .into_iter()
        .any(|kind| kind.minted(word))
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
    use crate::language::field_echo::{self, HeldField, NOW_MAX_CHARS};
    use crate::language::outcome::{Outcome, Refusal, SUMMARY_MAX_CHARS};

    const HANDLE: &str = "target_7k2nb5xe";
    const OTHER_HANDLE: &str = "target_q3m5c2ra";
    /// How a bounded page string ends when it was cut.
    const ELLIPSIS: &str = "...";

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

    fn at(target: &str, mut control: RejectedControl) -> RejectedControl {
        control.target = target.into();
        control
    }

    fn reverted(problem: Value, role: TargetRole, name: &str, preserve: bool) -> RejectedField {
        let mut problem = problem;
        problem["field"] = json!(0);
        problem["problem"] = json!("reverted");
        field(problem, role, name, "given", preserve)
    }

    /// A given value quoted in a sentence after it was cut to `max_chars`.
    fn quoted_cut(character: char, max_chars: usize) -> String {
        let kept = character.to_string().repeat(max_chars - ELLIPSIS.len());
        format!("\"{kept}{ELLIPSIS}\"")
    }

    /// One row of the clause table: the page's problem, the control's role, name and given
    /// value, whether an earlier choice of the call can change its options, its clause and its
    /// one next step.
    type ClauseRow = (
        Value,
        TargetRole,
        &'static str,
        &'static str,
        bool,
        String,
        String,
    );

    /// LANGUAGE.md's fill-refusal table: each problem word, on each control, reads its own clause
    /// after `Nothing was changed.`, and its one next step names the handle and the tool that
    /// fixes it. Rows reproduce a9b1529a (the route the page names, not the role the control reads
    /// as, picks the sentence; a range names the step it snaps to, a number field never does) and
    /// fc52b0ef (an ambiguous radio sends the model to find the radio it means). Every word a
    /// refused fill can carry has a row.
    #[test]
    fn each_refused_field_speaks_the_clause_and_step_language_md_names() {
        let offered =
            format!("Repeat browser_fill_form with {HANDLE} set to one of the offered options.");
        let corrected = format!("Repeat browser_fill_form with {HANDLE} set to a corrected value.");
        let click = format!("Click it with browser_click on {HANDLE}.");
        let widget = format!(
            "Open it with browser_click on {HANDLE}, then browser_click the option by name."
        );
        let wrong_tool = |route: &str| json!({"field": 0, "problem": "wrong_tool", "route": route});
        let range = |input_type: &str| {
            json!({"field": 0, "problem": "format", "type": input_type, "expected": "a number",
                "limits": {"min": "0", "max": "100", "step": "10"}})
        };
        let button = |name: &str| format!("The \"{name}\" control is a button, not a field.");
        let rows: Vec<ClauseRow> = vec![
            (
                missing_role(json!(["Developer", "Administrator", "Researcher", "Other"])),
                TargetRole::Combobox, "Role", "Engineer", false,
                "The \"Role\" dropdown has no option \"Engineer\". It offers: Developer, Administrator, Researcher, Other.".into(),
                offered.clone(),
            ),
            (
                missing_role(json!(["Free", {"text": "Pro", "disabled": true}, {"group": "Staff", "options": ["Team"]}])),
                TargetRole::Combobox, "Plan", "Gold", false,
                "The \"Plan\" dropdown has no option \"Gold\". It offers: Free, Team.".into(),
                offered.clone(),
            ),
            (
                missing_role(json!(["Lyon", "Nice"])),
                TargetRole::Combobox, "City", "Paris", true,
                "The \"City\" dropdown has no option \"Paris\". It offers: Lyon, Nice. Its options can depend on earlier fields.".into(),
                format!("Fill the earlier fields first, then repeat for {HANDLE}."),
            ),
            (
                json!({"field": 0, "problem": "option_missing", "options": ["Standard", "Express"], "group": "Shipping"}),
                TargetRole::Radio, "Standard", "Overnight", false,
                "The \"Shipping\" choice has no option \"Overnight\". It offers: Standard, Express.".into(),
                format!("Repeat with one of the offered labels for {HANDLE}."),
            ),
            (
                json!({"field": 0, "problem": "option_ambiguous", "options": [
                    {"text": "Other", "value": "other-staff"}, {"text": "Other", "value": "other-guest"}]}),
                TargetRole::Combobox, "Kind", "other", false,
                "\"other\" matches 2 options of the \"Kind\" dropdown: Other (value other-staff), Other (value other-guest).".into(),
                format!("Repeat with the option's value for {HANDLE}."),
            ),
            // fc52b0ef: two radios with one label.
            (
                json!({"field": 0, "problem": "option_ambiguous", "options": ["Yes", "Yes"], "group": "Subscribe"}),
                TargetRole::Radio, "No", "yes", false,
                "\"yes\" matches 2 options of the \"Subscribe\" radio button: Yes, Yes.".into(),
                "Find the radio you mean with browser_find, then fill that radio's own handle with true.".into(),
            ),
            (
                json!({"field": 0, "problem": "option_disabled"}),
                TargetRole::Combobox, "Plan", "Pro", false,
                "The option \"Pro\" of the \"Plan\" dropdown is disabled.".into(),
                "Choose an enabled option, or inspect the form for the field that enables it.".into(),
            ),
            (
                json!({"field": 0, "problem": "multiple_values"}),
                TargetRole::Combobox, "Role", "a, b", false,
                "The \"Role\" dropdown takes one value, not a list.".into(),
                format!("Repeat browser_fill_form with {HANDLE} set to a single value."),
            ),
            (
                json!({"field": 0, "problem": "not_boolean"}),
                TargetRole::Checkbox, "Agree", "maybe", false,
                "The \"Agree\" checkbox takes true or false, not \"maybe\".".into(),
                format!("Repeat with true or false for {HANDLE}."),
            ),
            (
                json!({"field": 0, "problem": "format", "expected": "YYYY-MM-DD", "type": "date",
                    "limits": {"min": "2026-10-01", "max": "2026-12-31"}}),
                TargetRole::Textbox, "Start", "next week", false,
                "The \"Start\" date field takes YYYY-MM-DD, not \"next week\". Allowed: 2026-10-01 to 2026-12-31.".into(),
                corrected.clone(),
            ),
            (
                json!({"field": 0, "problem": "format", "limits": {"min": "5"}}),
                TargetRole::NumberField, "Count", "x", false,
                "The \"Count\" number field does not accept \"x\". Allowed: from 5.".into(),
                corrected.clone(),
            ),
            // a9b1529a: a range names the step it snaps to.
            (
                range("range"),
                TargetRole::Slider, "Level", "15", false,
                "The \"Level\" slider takes a number, not \"15\". Allowed: 0 to 100, in steps of 10.".into(),
                corrected.clone(),
            ),
            // a9b1529a: a number field keeps a value off its step, so it never names one.
            (
                range("number"),
                TargetRole::NumberField, "Count", "many", false,
                "The \"Count\" number field takes a number, not \"many\". Allowed: 0 to 100.".into(),
                corrected.clone(),
            ),
            (
                wrong_tool("file"),
                TargetRole::Textbox, "Resume", "cv.pdf", false,
                "The \"Resume\" field is a file input.".into(),
                format!("Attach the file with browser_upload on {HANDLE}."),
            ),
            // a9b1529a: the button route decides the sentence for an image input, a switch, a tab
            // and a menu item alike.
            (wrong_tool("button"), TargetRole::Button, "Save", "x", false, button("Save"), click.clone()),
            (wrong_tool("button"), TargetRole::Textbox, "Go", "x", false, button("Go"), click.clone()),
            (wrong_tool("button"), TargetRole::Checkbox, "Dark mode", "x", false, button("Dark mode"), click.clone()),
            (wrong_tool("button"), TargetRole::Tab, "Overview", "x", false, button("Overview"), click.clone()),
            (wrong_tool("button"), TargetRole::MenuItem, "Settings", "x", false, button("Settings"), click),
            (
                wrong_tool("widget"),
                TargetRole::Combobox, "Country", "x", false,
                "The \"Country\" dropdown is a page widget, not a form field.".into(),
                widget.clone(),
            ),
            (
                wrong_tool("widget"),
                TargetRole::ListBox, "Colors", "x", false,
                "The \"Colors\" list box is a page widget, not a form field.".into(),
                widget,
            ),
            (
                json!({"field": 0, "problem": "submit_not_owned"}),
                TargetRole::Button, "Go", "x", false,
                "The submit control does not belong to the form that holds these fields.".into(),
                "Repeat without submit_target, then click the submit control with browser_click.".into(),
            ),
        ];

        let mut covered = Vec::new();
        for (problem, role, name, given, follows_choice, clause, step) in rows {
            let decoded = decode(problem.clone());
            covered.push(decoded.kind);
            let mut asked = control(role, name, given);
            asked.follows_choice = follows_choice;
            let refusal = Refusal::FieldValuesRejected {
                fields: vec![RejectedField::new(decoded, asked, true)],
                total: 1,
            };
            assert_eq!(
                refusal.summary(),
                format!("Nothing was changed. {clause}"),
                "{problem}"
            );
            assert_eq!(refusal.next_steps(), vec![step], "{problem}");
        }

        let every = [
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
        for kind in every {
            // No wildcard: a new problem word stops this compiling until it is listed above.
            let refuses = match kind {
                FieldProblemKind::Reverted => false,
                FieldProblemKind::OptionMissing
                | FieldProblemKind::OptionAmbiguous
                | FieldProblemKind::OptionDisabled
                | FieldProblemKind::MultipleValues
                | FieldProblemKind::NotBoolean
                | FieldProblemKind::Format
                | FieldProblemKind::WrongTool
                | FieldProblemKind::SubmitNotOwned => true,
            };
            assert_eq!(
                covered.contains(&kind),
                refuses,
                "{kind:?} has no clause row"
            );
        }
    }

    /// Reproduces 5bcb28f3: a list is cut only when the page says so (`options_total`), so a list
    /// the transport dropped reads `among its N options` with the browser_read step and never
    /// `offers no options`, and an empty or all-disabled list sends the model to read the
    /// control; a whole list too wide for one clause is named as far as it fits, keeps the repeat
    /// step and leaves every option in the facts.
    #[test]
    fn the_read_step_follows_an_option_list_the_summary_cannot_offer_whole() {
        let read = format!("Read {HANDLE} with browser_read to list all its options.");
        for (problem, given, summary) in [
            (
                json!({"field": 0, "problem": "option_missing", "options": ["Alpha", "Beta"],
                    "options_total": 195, "containing": ["Region East", "Region West"]}),
                "region",
                "The \"Region\" dropdown has no option \"region\" among its 195 options. Options containing \"region\": Region East, Region West.",
            ),
            (
                json!({"field": 0, "problem": "option_missing", "options": ["Alpha"], "options_total": 195}),
                "zzz",
                "The \"Region\" dropdown has no option \"zzz\" among its 195 options.",
            ),
            (
                json!({"field": 0, "problem": "option_missing", "options_total": 4}),
                "Engineer",
                "The \"Region\" dropdown has no option \"Engineer\" among its 4 options.",
            ),
            (
                missing_role(json!([])),
                "Gold",
                "The \"Region\" dropdown has no option \"Gold\". It offers no options.",
            ),
            (
                missing_role(json!([{"text": "Pro", "disabled": true}])),
                "Gold",
                "The \"Region\" dropdown has no option \"Gold\". Every option is disabled.",
            ),
        ] {
            let refusal = one(problem.clone(), TargetRole::Combobox, "Region", given, true);
            assert_eq!(refusal.summary(), format!("Nothing was changed. {summary}"), "{problem}");
            assert_eq!(refusal.next_steps(), vec![read.clone()], "{problem}");
        }

        let options: Vec<String> = (0..25)
            .map(|index| format!("{}{index}", "w".repeat(50)))
            .collect();
        let wide = field(
            missing_role(json!(options)),
            TargetRole::Combobox,
            "Wide",
            "x",
            true,
        );
        let facts = rejected_facts(std::slice::from_ref(&wide));
        let refusal = Refusal::FieldValuesRejected {
            fields: vec![wide],
            total: 1,
        };
        let summary = refusal.summary();
        assert!(
            summary.contains("has no option \"x\". It offers: "),
            "{summary}"
        );
        assert!(summary.contains(" more, all in the facts."), "{summary}");
        assert!(!summary.contains("among its"), "{summary}");
        assert!(summary.chars().count() <= SUMMARY_MAX_CHARS, "{summary}");
        assert_eq!(
            refusal.next_steps(),
            vec![format!(
                "Repeat browser_fill_form with {HANDLE} set to one of the offered options."
            )]
        );
        assert_eq!(
            facts["problems"][0]["options"].as_array().map(Vec::len),
            Some(options.len())
        );
    }

    /// Contract with the page asset, which sends the detail: a detail this build cannot read
    /// decodes to nothing, so the refusal stays the browser's own and never guesses a route. That
    /// covers an unknown problem word, no problems, a detail that is not an object, and a
    /// wrong_tool problem with no route or an unknown one (a9b1529a).
    #[test]
    fn a_detail_this_build_cannot_read_does_not_decode() {
        for detail in [
            json!({"problems": [{"field": 0, "problem": "invented"}]}),
            json!({"problems": []}),
            json!("text"),
            json!({"problems": [{"field": 0, "problem": "wrong_tool"}]}),
            json!({"problems": [{"field": 0, "problem": "wrong_tool", "route": "teleport"}]}),
            json!({"problems": [{"field": 0, "problem": "wrong_tool", "type": "file"}]}),
        ] {
            assert!(FieldProblems::decode(&detail).is_none(), "{detail}");
        }
    }

    /// LANGUAGE.md's bounds on a fill's summary: a refusal names `N of M fields`, stays within
    /// SUMMARY_MAX_CHARS and counts the clauses it has no room for; a fill the page partly undid
    /// names the first REVERTED_SHOWN_MAX fields that went back and counts the rest; and a
    /// completed fill names the readings it has room for and counts the rest, while its facts keep
    /// every reading.
    #[test]
    fn a_fill_summary_names_what_it_has_room_for_and_counts_the_rest() {
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
        let summary = Refusal::FieldValuesRejected { fields, total: 12 }.summary();
        assert!(summary.chars().count() <= SUMMARY_MAX_CHARS, "{summary}");
        assert!(
            summary.starts_with("Nothing was changed: 12 of 12 fields cannot take their values."),
            "{summary}"
        );
        assert!(summary.ends_with(" more are in the facts."), "{summary}");

        let went_back = |count: usize, set: usize| Refusal::FieldsReverted {
            set,
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
        let one_back = Refusal::FieldsReverted {
            set: 2,
            reverted: vec![reverted(
                json!({"now": "Choose one..."}),
                TargetRole::Combobox,
                "Role",
                true,
            )],
        };
        assert_eq!(
            one_back.summary(),
            "Set 2 of 3 fields; the \"Role\" dropdown went back to \"Choose one...\" after the page reacted."
        );
        assert_eq!(
            went_back(2, 1).summary(),
            "Set 1 of 3 fields; the \"Field 0\" dropdown went back to \"Old 0\" and the \"Field 1\" dropdown went back to \"Old 1\" after the page reacted."
        );
        let many = 5;
        let five = went_back(many, 1).summary();
        assert!(
            five.starts_with("Set 1 of 6 fields; the \"Field 0\" dropdown"),
            "{five}"
        );
        assert!(
            five.ends_with(&format!(
                "and {} more fields went back too after the page reacted.",
                many - REVERTED_SHOWN_MAX
            )),
            "{five}"
        );

        let held: Vec<HeldField> = (0..12)
            .map(|index| {
                HeldField::new(
                    HANDLE,
                    TargetRole::Combobox,
                    &format!("Field number {index}"),
                    &"v".repeat(30),
                    false,
                    true,
                )
            })
            .collect();
        let readings = field_echo::readings(&held);
        let said = readings.after_clause().expect("named readings are said");
        assert!(said.ends_with("more in the facts"), "{said}");
        let facts = field_echo::facts(&held);
        let filled = Outcome::FormFilled {
            fields: held.len(),
            submitted: false,
            host: Some("example.com".into()),
            held,
            rebound: false,
        };
        assert!(filled.summary().chars().count() <= SUMMARY_MAX_CHARS);
        assert_eq!(
            facts.and_then(|facts| facts.as_array().map(Vec::len)),
            Some(12)
        );
    }

    /// LANGUAGE.md's example and step rules: one call with a bad date, a bad checkbox word and a
    /// file input names every field, edits nothing and gives two steps; fields that need only a
    /// corrected value share one step that names their handles, past HANDLES_LISTED_MAX counting
    /// the rest; a credential field keeps its own step; a reverted field is read back; and a
    /// result carries at most NEXT_STEPS_MAX steps.
    #[test]
    fn fields_that_only_need_a_corrected_value_share_one_step_and_a_result_has_at_most_two() {
        let example = Refusal::FieldValuesRejected {
            fields: vec![
                RejectedField::new(
                    decode(
                        json!({"field": 0, "problem": "format", "type": "date", "expected": "YYYY-MM-DD"}),
                    ),
                    at(
                        "target_14",
                        control(TargetRole::Textbox, "Start", "10/14/2026"),
                    ),
                    true,
                ),
                RejectedField::new(
                    decode(json!({"field": 1, "problem": "not_boolean"})),
                    at(
                        "target_15",
                        control(TargetRole::Checkbox, "Remember me", "maybe"),
                    ),
                    true,
                ),
                RejectedField::new(
                    decode(json!({"field": 2, "problem": "wrong_tool", "route": "file"})),
                    at(
                        "target_16",
                        control(TargetRole::Textbox, "Resume", "cv.pdf"),
                    ),
                    true,
                ),
            ],
            total: 4,
        };
        assert_eq!(
            example.summary(),
            "Nothing was changed: 3 of 4 fields cannot take their values. The \"Start\" date field takes YYYY-MM-DD, not \"10/14/2026\". The \"Remember me\" checkbox takes true or false, not \"maybe\". The \"Resume\" field is a file input."
        );
        assert_eq!(
            example.next_steps(),
            vec![
                "Repeat browser_fill_form with corrected values for target_14 and target_15."
                    .to_owned(),
                "Attach the file with browser_upload on target_16.".to_owned(),
            ]
        );

        let correctable = |count: usize| -> (Vec<String>, Vec<String>) {
            let handles: Vec<String> = (0..count).map(|index| format!("target_{index}")).collect();
            let fields: Vec<RejectedField> = handles
                .iter()
                .enumerate()
                .map(|(index, handle)| {
                    RejectedField::new(
                        decode(
                            json!({"field": index, "problem": "format", "expected": "a number"}),
                        ),
                        at(handle, control(TargetRole::NumberField, "Qty", "x")),
                        true,
                    )
                })
                .collect();
            (handles, rejected_next_steps(&fields))
        };
        let (handles, steps) = correctable(3);
        assert_eq!(
            steps,
            vec![format!(
                "Repeat browser_fill_form with corrected values for {}, {} and {}.",
                handles[0], handles[1], handles[2]
            )]
        );
        let many = HANDLES_LISTED_MAX + 2;
        let (handles, steps) = correctable(many);
        assert_eq!(
            steps,
            vec![format!(
                "Repeat browser_fill_form with corrected values for {} and {} more.",
                handles[..HANDLES_LISTED_MAX].join(", "),
                many - HANDLES_LISTED_MAX
            )]
        );

        let mut secret = at("target_9", control(TargetRole::Textbox, "Password", "x"));
        secret.credential = true;
        let beside_a_credential = rejected_next_steps(&[
            RejectedField::new(
                decode(json!({"field": 0, "problem": "format", "expected": "a number"})),
                at("target_1", control(TargetRole::NumberField, "Qty", "x")),
                true,
            ),
            RejectedField::new(
                decode(json!({"field": 1, "problem": "format"})),
                secret,
                true,
            ),
            RejectedField::new(
                decode(json!({"field": 2, "problem": "format", "expected": "a number"})),
                at("target_3", control(TargetRole::NumberField, "Count", "y")),
                true,
            ),
        ]);
        assert_eq!(
            beside_a_credential,
            vec![
                "Repeat browser_fill_form with corrected values for target_1 and target_3."
                    .to_owned(),
                "Ask the user for the value, then repeat browser_fill_form with target_9 set to it."
                    .to_owned(),
            ]
        );

        let other = at(
            OTHER_HANDLE,
            control(TargetRole::Checkbox, "Agree", "maybe"),
        );
        let mixed = Refusal::FieldValuesRejected {
            fields: vec![
                field(
                    missing_role(json!(["A", "B"])),
                    TargetRole::Combobox,
                    "Role",
                    "x",
                    true,
                ),
                RejectedField::new(
                    decode(json!({"field": 3, "problem": "not_boolean"})),
                    other,
                    true,
                ),
                field(
                    json!({"field": 4, "problem": "multiple_values"}),
                    TargetRole::Combobox,
                    "Team",
                    "x",
                    true,
                ),
            ],
            total: 5,
        };
        let steps = mixed.next_steps();
        assert_eq!(steps.len(), NEXT_STEPS_MAX, "{steps:?}");
        assert!(steps[1].contains(OTHER_HANDLE), "{steps:?}");

        let read_back = format!("Read {HANDLE} with browser_read to see its current value.");
        let went_back = |count: usize| Refusal::FieldsReverted {
            set: 1,
            reverted: (0..count)
                .map(|index| {
                    reverted(
                        json!({"now": "Old"}),
                        TargetRole::Combobox,
                        &format!("Field {index}"),
                        true,
                    )
                })
                .collect(),
        };
        assert_eq!(went_back(1).next_steps(), vec![read_back]);
        assert_eq!(went_back(5).next_steps().len(), NEXT_STEPS_MAX);
    }

    /// LANGUAGE.md and ACCEPTANCE Browser journeys 4: a credential-class control never lists
    /// options or echoes a value, in its sentence, its steps or its facts, whether the fill was
    /// refused or the page changed the control back.
    #[test]
    fn a_credential_class_control_lists_nothing_and_echoes_nothing() {
        let mut secret = control(TargetRole::Combobox, "Token kind", "PRIVATE_VALUE");
        secret.credential = true;
        let refused = RejectedField::new(
            decode(missing_role(json!(["PRIVATE_OPTION"]))),
            secret,
            true,
        );
        let fact = refused.fact();
        let refusal = Refusal::FieldValuesRejected {
            fields: vec![refused],
            total: 1,
        };
        assert_eq!(
            refusal.summary(),
            "Nothing was changed. The \"Token kind\" dropdown is credential-class."
        );
        let everything = format!("{} {:?} {fact}", refusal.summary(), refusal.next_steps());
        assert!(!everything.contains("PRIVATE"), "{everything}");
        assert_eq!(fact.get("options"), None);

        let mut password = control(TargetRole::Textbox, "Password", "PRIVATE_GIVEN");
        password.credential = true;
        let changed_back = RejectedField::new(
            decode(json!({"field": 0, "problem": "reverted", "now": "PRIVATE_NOW"})),
            password,
            true,
        );
        let fact = changed_back.fact().to_string();
        assert!(!fact.contains("PRIVATE"), "{fact}");
        let refusal = Refusal::FieldsReverted {
            set: 1,
            reverted: vec![changed_back],
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

    /// ACCEPTANCE Governance 17 for fill and credential refusals: with preserve_target_names off,
    /// no page name, option text, group or given value reaches a summary or a step, for every
    /// problem word and role, a revert, and a credential typing refusal.
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
            json!({"field": 0, "problem": "wrong_tool", "route": "file", "group": "PRIVATE_GROUP"}),
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
                let text = format!("{} {:?}", refusal.summary(), refusal.next_steps());
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

        let went_back = Refusal::FieldsReverted {
            set: 0,
            reverted: vec![reverted(
                json!({"now": "PRIVATE_VALUE"}),
                TargetRole::Combobox,
                "PRIVATE_NAME",
                false,
            )],
        };
        assert_eq!(
            went_back.summary(),
            "Set 0 of 1 field; a dropdown went back after the page reacted."
        );

        let typing = Refusal::CredentialAuthorization {
            control: ControlRef::new(TargetRole::Textbox, "PRIVATE_NAME", None, false),
        };
        let text = format!("{} {:?}", typing.summary(), typing.next_steps());
        assert!(!text.contains("PRIVATE"), "{text}");
    }

    /// LANGUAGE.md's page-text rule, and reproduces L-D4 (eff79366): page text and a given value
    /// stand in a sentence on one line, bounded, with single quotes, and a word shaped like a real
    /// Ghostlight id, a minted invocation id and a diag_ cursor included, becomes
    /// HANDLE_REPLACEMENT, while near misses stay as they are. A refused fill quotes the given value
    /// within GIVEN_MAX_CHARS, a revert quotes what the field holds within the same bound and says
    /// NO_VALUE for nothing, and a completed fill quotes what a field holds within NOW_MAX_CHARS.
    #[test]
    fn page_text_reaches_a_sentence_as_one_bounded_line_that_cannot_pose_as_a_handle() {
        let nasty = format!("Line\none \"quoted\" {HANDLE} {}", "x".repeat(100));
        let refusal = one(
            missing_role(json!([nasty.clone()])),
            TargetRole::Combobox,
            &nasty,
            &"g".repeat(100),
            true,
        );
        let summary = refusal.summary();
        assert!(!summary.contains('\n'), "{summary}");
        assert!(!summary.contains(HANDLE), "{summary}");
        assert!(summary.contains(HANDLE_REPLACEMENT), "{summary}");
        assert!(summary.contains("'quoted'"), "{summary}");
        assert!(
            summary.contains(&quoted_cut('g', GIVEN_MAX_CHARS)),
            "{summary}"
        );

        let invocation = crate::work::InvocationId::mint();
        for minted in [
            HANDLE,
            "tab_ab2c3d4e",
            "view_zzzz7777",
            "image_abcdefgh",
            invocation.as_str(),
            "diag_1k_0a1b2c3d",
            "diag_0_ffffffff",
        ] {
            assert_eq!(
                page_text(&format!("see ({minted})."), PAGE_TEXT_MAX_CHARS),
                format!("see {HANDLE_REPLACEMENT}"),
                "{minted}"
            );
        }
        for near in [
            "target_7k2nb5x",
            "tab_ABCDEFGH",
            "invocation_0123456789abcdef0123456789abcde",
            "invocation_0123456789abcdef0123456789abcdeg",
            "diag__0a1b2c3d",
            "diag_1k_0a1b2c3",
            "diag_1k_0A1B2C3D",
            "diag_1k",
        ] {
            assert_eq!(page_text(near, PAGE_TEXT_MAX_CHARS), near, "{near}");
        }

        let went_back = |now: &str| Refusal::FieldsReverted {
            set: 0,
            reverted: vec![reverted(
                json!({ "now": now }),
                TargetRole::Textbox,
                "Note",
                true,
            )],
        };
        let long = went_back(&"y".repeat(100)).summary();
        assert!(long.contains(&quoted_cut('y', GIVEN_MAX_CHARS)), "{long}");
        let empty = went_back("").summary();
        assert!(
            empty.contains(&format!("went back to {NO_VALUE}")),
            "{empty}"
        );

        let held = [
            HeldField::new(
                HANDLE,
                TargetRole::Combobox,
                "Note",
                &format!("a\n\"b\" {}", "x".repeat(80)),
                false,
                true,
            ),
            HeldField::new(HANDLE, TargetRole::Combobox, "Empty", "", false, true),
        ];
        let readings = field_echo::readings(&held);
        let said = readings.after_clause().expect("named readings are said");
        assert!(!said.contains('\n'), "{said}");
        let (note, empty) = said.split_once(", ").expect("two readings");
        let value = note.strip_prefix("Note = ").expect("the note is named");
        assert!(value.starts_with("a 'b' "), "{value}");
        assert!(value.ends_with(ELLIPSIS), "{value}");
        assert_eq!(value.chars().count(), NOW_MAX_CHARS, "{value}");
        assert_eq!(empty, format!("Empty = {NO_VALUE}"));
    }
}
