//! The model view of a result: the canonical envelope with every field at its default left out.
//!
//! The service keeps the full envelope for audit, history and flow references. This module only
//! renders what the MCP edge hands to a client (ADR-0198 Decisions 21 and 22). It never adds a field and
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

/// The `facts` key that holds a result's coverage block.
const COVERAGE: &str = "coverage";

/// The coverage fields that signal a limitation. A count above zero or a true flag keeps the
/// whole block; `inspected_documents` and `targeted` describe scope, not a limit.
const COVERAGE_LIMITATIONS: [&str; 5] = [
    "excluded_documents",
    "page_excluded_documents",
    "unavailable_documents",
    "limited_by_size",
    "masked_regions",
];

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
    if let Some(facts) = fields.get_mut(FACTS).and_then(Value::as_object_mut) {
        if facts.get(COVERAGE).is_some_and(shows_no_limitation) {
            facts.remove(COVERAGE);
        }
    }
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

/// Whether `coverage` is a block with every limitation field clear. Any other shape stays.
fn shows_no_limitation(coverage: &Value) -> bool {
    coverage.as_object().is_some_and(|block| {
        COVERAGE_LIMITATIONS
            .iter()
            .all(|key| block.get(*key).is_none_or(is_clear))
    })
}

fn is_clear(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => !flag,
        Value::Number(count) => count.as_u64() == Some(0),
        _ => false,
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

    fn coverage(overrides: Value) -> Value {
        let mut block = json!({
            "targeted": false, "inspected_documents": 2, "excluded_documents": 0,
            "page_excluded_documents": 0, "unavailable_documents": 0,
            "limited_by_size": false, "masked_regions": 0,
        });
        for (key, value) in overrides.as_object().into_iter().flatten() {
            block[key] = value.clone();
        }
        block
    }

    #[test]
    fn an_unlimited_coverage_block_is_dropped() {
        for scope in [
            json!({}),
            json!({"targeted": true, "inspected_documents": 0}),
        ] {
            let view = project(&json!({"facts": {"tab": "t", "coverage": coverage(scope)}}));
            assert_eq!(view, json!({"facts": {"tab": "t"}}));
        }
    }

    #[test]
    fn each_limitation_keeps_the_full_block() {
        for limit in [
            json!({"excluded_documents": 1}),
            json!({"page_excluded_documents": 3}),
            json!({"unavailable_documents": 1}),
            json!({"limited_by_size": true}),
            json!({"masked_regions": 2}),
        ] {
            let envelope = json!({"facts": {"coverage": coverage(limit)}});
            assert_eq!(project(&envelope), envelope);
        }
    }

    #[test]
    fn coverage_recurses_into_flow_step_results() {
        let limited = coverage(json!({"excluded_documents": 1}));
        let envelope = json!({"facts": {"steps": [
            {"position": 1, "result": {"facts": {"coverage": coverage(json!({}))}}},
            {"position": 2, "result": {"facts": {"coverage": limited.clone()}}},
            {"position": 3, "omitted": true, "coverage": limited.clone()},
        ]}});
        assert_eq!(
            project(&envelope),
            json!({"facts": {"steps": [
                {"position": 1, "result": {"facts": {}}},
                {"position": 2, "result": {"facts": {"coverage": limited.clone()}}},
                {"position": 3, "omitted": true, "coverage": limited},
            ]}})
        );
    }

    #[test]
    fn a_coverage_value_that_is_not_a_block_stays() {
        let envelope = json!({"facts": {"coverage": "none"}});
        assert_eq!(project(&envelope), envelope);
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
