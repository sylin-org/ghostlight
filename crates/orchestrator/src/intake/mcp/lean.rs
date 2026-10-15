//! The model view of a result: the canonical envelope with every field at its default left out.
//!
//! The service keeps the full envelope for audit, history and flow references. This module only
//! renders what the MCP edge hands to a client (ADR-0198 Decisions 21 and 22). It never adds a field and
//! never changes a value; a reader treats an absent field as its default. `status`, `effect`,
//! `repeat_safe`, `summary`, `invocation` and `facts.tab` are not in any rule, so they always stay.
//! Inside `facts`, each entry of `items` and `matches`, the lists Ghostlight itself mints, drops an
//! empty `state` and a false `credential_class`, and a `facts.tree` node drops an empty `label` and
//! an empty `children`. Any other fact, such as the `value` a script returned, is page data and is
//! never read as an item.

use serde_json::{Map, Value};

/// The value a field has when it carries no news.
enum Default {
    /// A string value, such as `"saved"`.
    Text(&'static str),
    /// An empty JSON array.
    EmptyList,
    /// The boolean `false`.
    False,
}

impl Default {
    fn matches(&self, value: &Value) -> bool {
        match self {
            Self::Text(text) => value.as_str() == Some(text),
            Self::EmptyList => value.as_array().is_some_and(Vec::is_empty),
            Self::False => value.as_bool() == Some(false),
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
const COVERAGE_LIMITATIONS: [&str; 4] = [
    "excluded_documents",
    "page_excluded_documents",
    "unavailable_documents",
    "limited_by_size",
];

/// The `facts` keys that hold the addressable items Ghostlight mints: the controls of an inspect
/// and the matches of a find. Nothing else in `facts` is read as an item, because a script result
/// or a flow value can hold page data of any shape.
const ITEM_LISTS: [&str; 2] = ["items", "matches"];

/// Fields of an item omitted at their default. A present `value` of `""` is a fact and stays.
const ITEM_OMISSIONS: [Omission; 2] = [
    Omission {
        key: "state",
        default: Default::EmptyList,
    },
    Omission {
        key: "credential_class",
        default: Default::False,
    },
];

/// The `facts` key that holds a document tree.
const TREE: &str = "tree";

/// Fields of a document-tree node omitted at their default.
const NODE_OMISSIONS: [Omission; 2] = [
    Omission {
        key: "label",
        default: Default::Text(""),
    },
    Omission {
        key: "children",
        default: Default::EmptyList,
    },
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
        if let Some(tree) = facts.get_mut(TREE) {
            lean_tree(tree);
        }
        for key in ITEM_LISTS {
            let entries = facts.get_mut(key).and_then(Value::as_array_mut);
            for item in entries.into_iter().flatten() {
                if let Some(fields) = item.as_object_mut() {
                    omit(fields, &ITEM_OMISSIONS);
                }
            }
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

/// Drop the defaults from a document-tree node and, recursively, from its children.
fn lean_tree(node: &mut Value) {
    let Some(fields) = node.as_object_mut() else {
        return;
    };
    omit(fields, &NODE_OMISSIONS);
    if let Some(children) = fields.get_mut("children").and_then(Value::as_array_mut) {
        children.iter_mut().for_each(lean_tree);
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
            "limited_by_size": false,
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
    fn item_defaults_are_dropped_from_items_and_matches() {
        let item = json!({"credential_class": false, "name": "Email", "role": "textbox",
            "state": [], "target": "target_h4n2"});
        let view = project(&json!({"facts": {"items": [item.clone()], "matches": [item]}}));
        let lean = json!({"name": "Email", "role": "textbox", "target": "target_h4n2"});
        assert_eq!(
            view,
            json!({"facts": {"items": [lean.clone()], "matches": [lean]}})
        );
    }

    #[test]
    fn item_facts_stay() {
        let envelope = json!({"facts": {"items": [
            {"target": "t1", "credential_class": true, "state": ["disabled"], "value": ""},
            {"target": "t2", "value": "[hidden]"},
        ]}});
        assert_eq!(project(&envelope), envelope);
    }

    #[test]
    fn page_data_that_looks_like_an_item_stays() {
        let page_data = json!([{"href": "/a", "target": "_blank", "state": [],
            "credential_class": false}]);
        let envelope = json!({"facts": {"tab": "t", "value": page_data,
            "other": {"target": "x", "state": []}}});
        assert_eq!(project(&envelope), envelope);
        let flow = json!({"facts": {"steps": [{"position": 1, "result": envelope}]}});
        assert_eq!(project(&flow), flow);
    }

    #[test]
    fn tree_nodes_are_pruned_at_every_depth() {
        let view = project(
            &json!({"facts": {"tree": {"kind": "document", "label": "", "children": [
                {"kind": "heading", "label": "Title", "children": []},
                {"kind": "group", "label": "", "children": [
                    {"kind": "link", "label": "", "children": []}]},
            ]}}}),
        );
        assert_eq!(
            view,
            json!({"facts": {"tree": {"kind": "document", "children": [
                {"kind": "heading", "label": "Title"},
                {"kind": "group", "children": [{"kind": "link"}]},
            ]}}})
        );
    }

    #[test]
    fn item_and_tree_defaults_recurse_into_flow_steps() {
        let view = project(&json!({"facts": {"steps": [
            {"position": 1, "result": {"facts": {
                "items": [{"target": "t", "state": [], "credential_class": false}],
                "tree": {"kind": "document", "label": "", "children": []}}}},
        ]}}));
        assert_eq!(
            view,
            json!({"facts": {"steps": [{"position": 1, "result": {"facts": {
                "items": [{"target": "t"}], "tree": {"kind": "document"}}}}]}})
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
        let envelope = json!({"facts": {"items": [{"target": "t", "state": []}],
            "tree": {"kind": "document", "label": "", "children": []}}});
        let once = project(&envelope);
        assert_eq!(project(&once), once);
    }

    #[test]
    fn a_value_that_is_not_an_object_passes_through() {
        assert_eq!(project(&Value::Null), Value::Null);
        assert_eq!(project(&json!([1])), json!([1]));
    }
}
