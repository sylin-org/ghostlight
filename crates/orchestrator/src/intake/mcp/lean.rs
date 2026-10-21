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

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::language::coverage::{Coverage, COVERAGE_FACT};

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
        if facts.get(COVERAGE_FACT).is_some_and(shows_no_limitation) {
            facts.remove(COVERAGE_FACT);
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

/// Whether `coverage` is a coverage block that withholds nothing. Any other shape stays.
fn shows_no_limitation(coverage: &Value) -> bool {
    Coverage::deserialize(coverage).is_ok_and(|block| !block.withholds())
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
    use crate::language::coverage::Coverage;

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

    /// ACCEPTANCE C21 and DB3: the model view keeps a coverage block exactly when it withholds
    /// something, at the top level and in each flow step result, so an absent block truthfully
    /// means nothing was withheld. Whether a block withholds is the coverage type's own answer,
    /// so a limitation the type learns reaches the model without a second list here (1b693758);
    /// a coverage value that is not a block, and a block on a step row rather than in its
    /// result, are left as they are.
    #[test]
    fn coverage_reaches_the_model_exactly_when_it_withholds() {
        for scope in [
            json!({}),
            json!({"targeted": true, "inspected_documents": 0}),
        ] {
            let view = project(&json!({"facts": {"tab": "t", "coverage": coverage(scope)}}));
            assert_eq!(view, json!({"facts": {"tab": "t"}}));
        }
        for limit in [
            json!({"excluded_documents": 1}),
            json!({"page_excluded_documents": 3}),
            json!({"unavailable_documents": 1}),
            json!({"limited_by_size": true}),
        ] {
            let envelope = json!({"facts": {"coverage": coverage(limit)}});
            assert_eq!(project(&envelope), envelope);
        }

        let shapes = [
            Coverage::default(),
            Coverage {
                targeted: true,
                inspected_documents: 3,
                ..Coverage::default()
            },
            Coverage {
                excluded_documents: 1,
                ..Coverage::default()
            },
            Coverage {
                page_excluded_documents: 2,
                ..Coverage::default()
            },
            Coverage {
                unavailable_documents: 1,
                ..Coverage::default()
            },
            Coverage {
                limited_by_size: true,
                ..Coverage::default()
            },
        ];
        for coverage in shapes {
            let envelope = json!({"facts": {"tab": "t", "coverage": coverage}});
            let kept = project(&envelope)["facts"].get("coverage").is_some();
            assert_eq!(kept, coverage.withholds(), "{coverage:?}");
        }

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

        let envelope = json!({"facts": {"coverage": "none"}});
        assert_eq!(project(&envelope), envelope);
    }

    /// Defect a199794e (the model view read any object with a string `target` under `facts` as
    /// an item, so a script result lost its `state`), and ACCEPTANCE C21: page data such as a
    /// script's `facts.value` is never rewritten, at the top level or inside a flow step result;
    /// only the items Ghostlight mints lose their defaults.
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
}
