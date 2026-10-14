//! The model view of a result: the canonical envelope with every field at its default left out.
//!
//! The service keeps the full envelope for audit, history and flow references. This module only
//! renders what the MCP edge hands to a client (ADR-0198 Decision 21). It never adds a field and
//! never changes a value; a reader treats an absent field as its default. `status`, `effect`,
//! `repeat_safe`, `summary`, `invocation` and `facts.tab` are not in any rule, so they always stay.

use serde_json::{Map, Value};

/// The value a field has when it carries no news.
enum Default {
    /// A string value, such as `"saved"`.
    Text(&'static str),
    /// An empty JSON array.
    EmptyList,
}

impl Default {
    fn matches(&self, value: &Value) -> bool {
        match self {
            Self::Text(text) => value.as_str() == Some(text),
            Self::EmptyList => value.as_array().is_some_and(Vec::is_empty),
        }
    }
}

/// One field and the value at which the model view omits it.
struct Omission {
    key: &'static str,
    default: Default,
}

/// Fields of a result envelope, and of each flow step result, omitted at their default.
const ENVELOPE_OMISSIONS: [Omission; 3] = [
    Omission {
        key: "history_storage",
        default: Default::Text("saved"),
    },
    Omission {
        key: "readiness",
        default: Default::Text("not_applicable"),
    },
    Omission {
        key: "next_steps",
        default: Default::EmptyList,
    },
];

/// Fields of a flow step row omitted at their default.
const ROW_OMISSIONS: [Omission; 1] = [Omission {
    key: "history_storage",
    default: Default::Text("saved"),
}];

/// The envelope key that holds a flow's or composition's step rows, inside `facts`.
const FACTS: &str = "facts";
const STEPS: &str = "steps";
/// The key inside a step row that holds that step's own envelope.
const STEP_RESULT: &str = "result";

/// Render the model view of `envelope`: the same envelope with every field at its default omitted.
///
/// The projection is idempotent and leaves a value that is not an object untouched.
pub(super) fn project(envelope: &Value) -> Value {
    let mut view = envelope.clone();
    lean_envelope(&mut view);
    view
}

fn lean_envelope(envelope: &mut Value) {
    let Some(fields) = envelope.as_object_mut() else {
        return;
    };
    omit(fields, &ENVELOPE_OMISSIONS);
    let rows = fields
        .get_mut(FACTS)
        .and_then(|facts| facts.get_mut(STEPS))
        .and_then(Value::as_array_mut);
    for row in rows.into_iter().flatten() {
        if let Some(row_fields) = row.as_object_mut() {
            omit(row_fields, &ROW_OMISSIONS);
        }
        if let Some(result) = row.get_mut(STEP_RESULT) {
            lean_envelope(result);
        }
    }
}

fn omit(fields: &mut Map<String, Value>, rules: &[Omission]) {
    for rule in rules {
        if fields
            .get(rule.key)
            .is_some_and(|value| rule.default.matches(value))
        {
            fields.remove(rule.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::project;

    #[test]
    fn each_default_is_dropped() {
        let view = project(&json!({
            "status": "succeeded", "history_storage": "saved",
            "readiness": "not_applicable", "next_steps": [],
        }));
        assert_eq!(view, json!({"status": "succeeded"}));
    }

    #[test]
    fn a_value_off_its_default_stays() {
        let envelope = json!({
            "history_storage": "unconfirmed", "readiness": "complete",
            "next_steps": ["Inspect the current page."],
        });
        assert_eq!(project(&envelope), envelope);
    }

    #[test]
    fn the_always_kept_keys_stay_at_any_value() {
        let envelope = json!({
            "status": "saved", "effect": "not_applicable", "repeat_safe": false,
            "summary": "", "invocation": "", "facts": {"tab": "[]"},
        });
        assert_eq!(project(&envelope), envelope);
    }

    #[test]
    fn a_default_of_the_wrong_type_stays() {
        let envelope = json!({"history_storage": ["saved"], "next_steps": "", "readiness": null});
        assert_eq!(project(&envelope), envelope);
    }

    #[test]
    fn flow_rows_and_their_results_recurse() {
        let child = json!({
            "status": "succeeded", "history_storage": "saved", "readiness": "not_applicable",
            "next_steps": [], "facts": {"tab": "tab_a"},
        });
        let envelope = json!({
            "history_storage": "saved",
            "facts": {"steps": [
                {"position": 1, "history_storage": "saved", "result": child},
                {"position": 2, "history_storage": "unconfirmed",
                 "result": {"history_storage": "unconfirmed", "next_steps": []}},
                {"position": 3, "status": "not_run"},
            ]},
        });
        assert_eq!(
            project(&envelope),
            json!({
                "facts": {"steps": [
                    {"position": 1,
                     "result": {"status": "succeeded", "facts": {"tab": "tab_a"}}},
                    {"position": 2, "history_storage": "unconfirmed",
                     "result": {"history_storage": "unconfirmed"}},
                    {"position": 3, "status": "not_run"},
                ]},
            })
        );
    }

    #[test]
    fn projection_is_idempotent() {
        let envelope = json!({
            "history_storage": "saved", "readiness": "not_applicable", "next_steps": [],
            "facts": {"steps": [{"history_storage": "saved",
                "result": {"history_storage": "saved", "next_steps": []}}]},
        });
        let once = project(&envelope);
        assert_eq!(project(&once), once);
    }

    #[test]
    fn a_value_that_is_not_an_object_passes_through() {
        assert_eq!(project(&Value::Null), Value::Null);
        assert_eq!(project(&json!([1])), json!([1]));
    }
}
