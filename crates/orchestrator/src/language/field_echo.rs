//! What a fill says the page now holds (ADR-0198 Decision 11).
//!
//! A fill reads back the fields worth reading: choice and typed controls, and text the page
//! reformatted, such as a phone number a mask rewrote. This module owns how those readings join
//! the sentence of a completed fill and its facts. A sentence carries a name and a value only
//! when the governed snapshot preserves target names, bounded and on one line like every page
//! string. The facts always carry the reading, because the fill was authorized to read the
//! document. Audit never holds either: the audit projection of a fill is the sentence without
//! its readings.
//!
//! A choice the page matched only after normalizing the given value says so (ADR-0198 Decision 8),
//! in the facts as `match` and in the sentence as the value that was given.
//!
//! A radio picked through a sibling's label is not the radio the handle names, and that radio now
//! reads `unchecked`. Its fact keeps that reading and adds `picked` and `group`, and its sentence
//! speaks of the group (`Plan = Pro`) or, with no group name, of the pick (`Pro is checked`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::field_problem::page_text;
use super::outcome::{counted, ActionSubject, TargetRole};

/// The longest page value a sentence quotes.
const NOW_MAX_CHARS: usize = 40;
/// The longest page name a sentence quotes.
const NAME_MAX_CHARS: usize = 60;
/// The most characters the readings of one fill may take in its sentence.
const READINGS_MAX_CHARS: usize = 260;
/// What a sentence says a field holds when it holds nothing.
const NO_VALUE: &str = "no value";

/// The fact key that says how a given value matched a choice.
const MATCH_KEY: &str = "match";
/// The fact key that names the sibling radio a fill picked in the addressed radio's place.
const PICKED_KEY: &str = "picked";
/// The fact key that names the choice group of a radio picked through a sibling.
const GROUP_KEY: &str = "group";

/// How a given value matched a choice when it was not the option's exact text, value or label.
/// An exact match is the unremarkable case and carries no mark.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceMatch {
    /// The value matched after Unicode composition, trimming, white space collapse and case
    /// folding, and only one option did.
    Normalized,
}

impl ChoiceMatch {
    /// The word the facts use for this match.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normalized => "normalized",
        }
    }
}

/// The sibling radio a fill picked in place of the one addressed.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Picked {
    label: String,
    group: Option<String>,
}

/// One filled field and what the page holds in it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldField {
    target: String,
    subject: ActionSubject,
    now: String,
    tolerated: bool,
    named: bool,
    matched: Option<(ChoiceMatch, String)>,
    picked: Option<Picked>,
}

impl HeldField {
    /// Join the page's reading of a field with what the service knows about it. `preserve_names`
    /// is the governed snapshot's `preserves_target_names()`. `tolerated` says the page holds the
    /// given value in another format.
    #[must_use]
    pub fn new(
        target: &str,
        role: TargetRole,
        name: &str,
        now: &str,
        tolerated: bool,
        preserve_names: bool,
    ) -> Self {
        Self {
            target: target.to_owned(),
            subject: ActionSubject::with_role(role, name, preserve_names),
            now: now.to_owned(),
            tolerated,
            named: preserve_names,
            matched: None,
            picked: None,
        }
    }

    /// Say the fill picked the sibling radio labelled `label`, in the group `group` when it has a
    /// name, rather than the radio this field addressed. The field's own reading stays as the
    /// page gave it.
    #[must_use]
    pub fn picked(mut self, label: &str, group: Option<&str>) -> Self {
        self.picked = Some(Picked {
            label: label.to_owned(),
            group: group.map(str::to_owned),
        });
        self
    }

    /// Say the page matched this choice by `how`, and which value the caller gave. The given
    /// value reaches a sentence only with the name, and never the facts.
    #[must_use]
    pub fn matched(mut self, how: ChoiceMatch, given: &str) -> Self {
        self.matched = Some((how, given.to_owned()));
        self
    }

    /// The fact that describes this field: its handle, what it holds, and how a choice matched
    /// when it did not match exactly.
    #[must_use]
    pub fn fact(&self) -> Value {
        let mut fact = json!({"target": self.target, "now": self.now});
        if let Value::Object(members) = &mut fact {
            if let Some((how, _)) = &self.matched {
                members.insert(MATCH_KEY.into(), json!(how.as_str()));
            }
            if let Some(picked) = &self.picked {
                members.insert(PICKED_KEY.into(), json!(picked.label));
                if let Some(group) = &picked.group {
                    members.insert(GROUP_KEY.into(), json!(group));
                }
            }
        }
        fact
    }

    /// The field's name as a sentence may say it, when the snapshot lets it be named.
    fn name(&self) -> Option<String> {
        self.subject
            .label()
            .map(|label| page_text(label, NAME_MAX_CHARS))
            .filter(|label| !label.is_empty())
    }

    /// The value as a sentence may say it.
    fn value(&self) -> String {
        let value = page_text(&self.now, NOW_MAX_CHARS);
        if value.is_empty() {
            NO_VALUE.to_owned()
        } else {
            value
        }
    }

    /// How a radio picked through a sibling reads: `Plan = Pro` in a named group, `Pro is checked`
    /// without one. Page names reach a sentence only when the snapshot preserves them.
    fn pick(&self, picked: &Picked) -> Option<String> {
        if !self.named {
            return None;
        }
        let label = page_text(&picked.label, NOW_MAX_CHARS);
        let group = picked
            .group
            .as_deref()
            .map(|group| page_text(group, NAME_MAX_CHARS))
            .filter(|group| !group.is_empty());
        Some(match group {
            Some(group) => format!("{group} = {label}"),
            None => format!("{label} is checked"),
        })
    }

    /// How a choice or typed control reads: `Role = Administrator`, and after a normalized match
    /// `Role = Administrator (matched "administrator")`.
    fn assignment(&self) -> Option<String> {
        if self.tolerated {
            return None;
        }
        let mut reading = match &self.picked {
            Some(picked) => self.pick(picked)?,
            None => format!("{} = {}", self.name()?, self.value()),
        };
        if let Some((_, given)) = &self.matched {
            reading.push_str(&format!(
                " (matched \"{}\")",
                page_text(given, NOW_MAX_CHARS)
            ));
        }
        Some(reading)
    }

    /// How a reformatted text field reads: `Phone now reads "(555) 123-4567"`.
    fn reading(&self) -> Option<String> {
        if !self.tolerated {
            return None;
        }
        Some(format!("{} now reads \"{}\"", self.name()?, self.value()))
    }
}

/// The part of a completed fill's sentence that says what the page holds: the choice and typed
/// readings after a colon, and a sentence for each reformatted text field. Empty when nothing is
/// named. Readings the room does not hold are counted, and the facts carry all of them.
#[must_use]
pub fn readings(held: &[HeldField]) -> Readings {
    let mut room = READINGS_MAX_CHARS;
    let mut assignments = Vec::new();
    let mut sentences = Vec::new();
    let mut left_out = 0;
    for field in held {
        let (text, into) = match (field.assignment(), field.reading()) {
            (Some(text), _) => (text, &mut assignments),
            (None, Some(text)) => (text, &mut sentences),
            (None, None) => continue,
        };
        let cost = text.chars().count() + 2;
        if cost > room {
            left_out += 1;
            continue;
        }
        room -= cost;
        into.push(text);
    }
    if left_out > 0 {
        assignments.push(format!(
            "{} more in the facts",
            counted(left_out, "reading", "readings")
        ));
    }
    Readings {
        assignments: assignments.join(", "),
        sentences,
    }
}

/// The readings of a fill, ready to join its sentence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Readings {
    assignments: String,
    sentences: Vec<String>,
}

impl Readings {
    /// The readings after the colon that follows the fill's first clause, or none.
    #[must_use]
    pub fn after_clause(&self) -> Option<&str> {
        (!self.assignments.is_empty()).then_some(self.assignments.as_str())
    }

    /// The sentences that follow the fill's first sentence, each ending in a full stop.
    #[must_use]
    pub fn trailing(&self) -> String {
        self.sentences
            .iter()
            .map(|sentence| format!(" {sentence}."))
            .collect()
    }
}

/// The facts of a completed fill: one entry for each field the page reported, or none.
#[must_use]
pub fn facts(held: &[HeldField]) -> Option<Value> {
    (!held.is_empty()).then(|| Value::Array(held.iter().map(HeldField::fact).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::outcome::Outcome;

    const HANDLE: &str = "target_7k2nb5xe";

    fn field(
        role: TargetRole,
        name: &str,
        now: &str,
        tolerated: bool,
        preserve: bool,
    ) -> HeldField {
        HeldField::new(HANDLE, role, name, now, tolerated, preserve)
    }

    #[test]
    fn a_choice_reads_as_an_assignment_and_a_reformatted_text_as_a_sentence() {
        let held = [
            field(TargetRole::Combobox, "Role", "Administrator", false, true),
            field(TargetRole::Textbox, "Phone", "(555) 123-4567", true, true),
        ];
        let readings = readings(&held);
        assert_eq!(readings.after_clause(), Some("Role = Administrator"));
        assert_eq!(readings.trailing(), " Phone now reads \"(555) 123-4567\".");
    }

    #[test]
    fn a_radio_picked_through_a_sibling_speaks_of_the_group_not_the_addressed_radio() {
        let held = [field(TargetRole::Radio, "Free", "unchecked", false, true)
            .matched(ChoiceMatch::Normalized, "pro")
            .picked("Pro", Some("Plan"))];
        assert_eq!(
            readings(&held).after_clause(),
            Some("Plan = Pro (matched \"pro\")")
        );
        assert_eq!(
            facts(&held),
            Some(json!([{
                "target": HANDLE,
                "now": "unchecked",
                "match": "normalized",
                "picked": "Pro",
                "group": "Plan"
            }]))
        );
        let ungrouped =
            [field(TargetRole::Radio, "Free", "unchecked", false, true).picked("Pro", None)];
        assert_eq!(readings(&ungrouped).after_clause(), Some("Pro is checked"));
        assert_eq!(
            facts(&ungrouped),
            Some(json!([{"target": HANDLE, "now": "unchecked", "picked": "Pro"}]))
        );
        let unnamed =
            [field(TargetRole::Radio, "Free", "unchecked", false, false)
                .picked("Pro", Some("Plan"))];
        assert_eq!(readings(&unnamed).after_clause(), None);
        assert_eq!(
            facts(&unnamed),
            Some(json!([{
                "target": HANDLE,
                "now": "unchecked",
                "picked": "Pro",
                "group": "Plan"
            }]))
        );
    }

    #[test]
    fn without_names_nothing_is_said_but_the_facts_still_hold_the_reading() {
        let held = [field(
            TargetRole::Combobox,
            "Role",
            "Administrator",
            false,
            false,
        )];
        let readings = readings(&held);
        assert_eq!(readings.after_clause(), None);
        assert_eq!(readings.trailing(), "");
        assert_eq!(
            facts(&held),
            Some(json!([{"target": HANDLE, "now": "Administrator"}]))
        );
    }

    #[test]
    fn a_value_is_bounded_on_one_line_and_never_poses_as_a_handle() {
        let long = "x".repeat(80);
        let held = [
            field(
                TargetRole::Combobox,
                "Note",
                &format!("a\n\"b\" {long}"),
                false,
                true,
            ),
            field(TargetRole::Combobox, "Ref", "target_abcdefgh", false, true),
            field(TargetRole::Combobox, "Empty", "", false, true),
        ];
        let said = readings(&held).assignments;
        assert!(said.contains("Note = a 'b' xxx"));
        assert!(said.contains("..."));
        assert!(said.contains("Ref = [handle]"));
        assert!(said.contains("Empty = no value"));
        assert!(!said.contains('\n'));
    }

    #[test]
    fn readings_the_room_does_not_hold_are_counted() {
        let held: Vec<HeldField> = (0..12)
            .map(|index| {
                field(
                    TargetRole::Combobox,
                    &format!("Field number {index}"),
                    &"v".repeat(30),
                    false,
                    true,
                )
            })
            .collect();
        let said = readings(&held).assignments;
        assert!(said.chars().count() < 340);
        assert!(said.ends_with("more in the facts"), "{said}");
        assert_eq!(
            facts(&held).and_then(|facts| facts.as_array().map(Vec::len)),
            Some(12)
        );
    }

    fn filled(preserve: bool, submitted: bool) -> Outcome {
        Outcome::FormFilled {
            fields: 3,
            submitted,
            host: Some("example.com".into()),
            held: vec![
                field(
                    TargetRole::Combobox,
                    "Role",
                    "Administrator",
                    false,
                    preserve,
                ),
                field(
                    TargetRole::Combobox,
                    "Shipping method",
                    "Express",
                    false,
                    preserve,
                ),
                field(
                    TargetRole::Textbox,
                    "Phone",
                    "(555) 123-4567",
                    true,
                    preserve,
                ),
            ],
        }
    }

    #[test]
    fn a_completed_fill_says_what_the_page_holds_only_when_names_are_preserved() {
        assert_eq!(
            filled(true, false).summary(),
            "Filled 3 fields on example.com: Role = Administrator, Shipping method = Express. Phone now reads \"(555) 123-4567\"."
        );
        assert_eq!(
            filled(true, true).summary(),
            "Filled 3 fields on example.com and submitted the form: Role = Administrator, Shipping method = Express. Phone now reads \"(555) 123-4567\"."
        );
        assert_eq!(
            filled(false, false).summary(),
            "Filled 3 fields on example.com."
        );
    }

    #[test]
    fn history_keeps_the_fill_without_what_the_page_held() {
        let audit = filled(true, false).audit();
        assert_eq!(audit.summary(), "Filled 3 fields on example.com.");
        assert_eq!(filled(true, false).observed().count, Some(3));
    }

    #[test]
    fn a_fill_that_reported_nothing_has_no_facts() {
        assert_eq!(facts(&[]), None);
        assert_eq!(readings(&[]).after_clause(), None);
    }
}
