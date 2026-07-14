// SPDX-License-Identifier: Apache-2.0 OR MIT
//! inputSchema validation at the `tools/call` entry point (ADR-0031 Decision 4: hard-fail with
//! corrective errors). The binary previously performed ZERO schema validation -- it extracted
//! `name` and `arguments` loosely and forwarded the raw arguments blob to the extension, where a
//! missing `tabId` silently became `None` and surfaced as an extension error with no corrective
//! content. This module rejects schema violations BEFORE dispatch with a corrective
//! [`ToolError::invalid_request(...)`](crate::ToolError::invalid_request)`.next_step(...)`, in the
//! same shape the "Unknown tool" path already uses (`pipeline.rs`).
//!
//! The error contract (ADR-0031 Decision 4): "what went wrong" + "what to try next" WHEN the
//! fixture can produce a suggestion honestly. The suggestion text is GENERATED from the fixture,
//! never hand-authored per tool:
//!   - field name + expected type come from `inputSchema`;
//!   - enum alternatives and scalar bounds come from `inputSchema.properties.<field>`;
//!   - the example shape comes from the tool's `example.call` field;
//!   - the "get a tabId first" hint is one hard-coded conditional (the single piece of logic not
//!     derived from the fixture; justified because a per-field `suggestion` annotation is
//!     over-engineering for one field).
//!
//! Cases that do NOT attach a suggestion (per the ADR): runtime/state failures, governance denials
//! (already self-correcting via `Denied (D-xxxxxxxx):` + `explain`), and internal errors. Those do
//! not flow through this module at all -- this module only handles the four schema-violation
//! classes that the fixture knows enough about to correct.

use crate::ToolError;
use serde_json::Value;

/// The schema and example for one tool, extracted from the fixture once. All validation paths
/// draw their corrective suggestion text from these fields, so the validator stores no strings of
/// its own and cannot drift.
#[derive(Debug, Clone)]
pub struct ToolSchema {
    /// The tool's `inputSchema` object (`{ type, properties, required, additionalProperties }`).
    pub input_schema: Value,
    /// The tool's `example.call` object, rendered as the corrective suggestion's shape. `None`
    /// for tools that carry no example (today: only `explain`, which is argument-less).
    pub example_call: Option<Value>,
}

impl ToolSchema {
    /// Look up one tool's schema + example from the code-declared registry by tool name.
    /// Returns `None` for an unknown tool name (the pipeline's pre-existing registry lookup
    /// already rejects unknown names before this module runs, so this is defense in depth).
    pub fn for_tool(name: &str) -> Option<Self> {
        let desc = crate::browser::directory::descriptor(name)?;
        let input_schema = (desc.input_schema)();
        let example_call = desc
            .example
            .and_then(|ex| serde_json::from_str(ex.call).ok());
        Some(Self {
            input_schema,
            example_call,
        })
    }
}

/// Validate `arguments` against the tool's `inputSchema`. Returns `Ok(())` for a well-formed
/// call, or `Err(ToolError::InvalidRequest { .. })` with a corrective `next_step` for the
/// schema-violation classes the ADR names.
///
/// The four checks, in the order they run (the first failure wins, so the model gets the most
/// actionable correction first):
///   1. unknown property (under `additionalProperties: false`) -- caught before required/type so
///      a typo'd field name is named explicitly rather than masked as a missing required field;
///   2. missing required field;
///   3. wrong type on a present field;
///   4. enum, numeric-bound, and string-length constraints on a present field.
///
/// NOTE on the one enum exception: an `action` field (e.g. `computer.action`) is deliberately NOT
/// enforced here. An unknown action is already handled fail-closed by the governance
/// layer (the directory classifies it as a miss and returns a stable `Denied (D-xxxxxxxx):`
/// audit-grade denial), which is a MORE informative outcome than a generic validation error --
/// it is a governed decision with a denial id, not a structural rejection. Enforcing enums here
/// would shadow that well-designed path. Other enum fields and scalar constraints have no
/// governed alternative, so they are enforced here.
pub fn validate_arguments(schema: &ToolSchema, arguments: &Value) -> Result<(), ToolError> {
    let obj = match arguments.as_object() {
        Some(o) => o,
        None => {
            // The schema's type is "object"; a non-object (null, array, primitive) is a wrong-shape
            // call. `arguments` defaults to Value::Null upstream when absent, so this is also the
            // "no arguments at all on a tool with required fields" path.
            return Err(missing_required_hint(schema, "the arguments object"));
        }
    };

    let properties = schema
        .input_schema
        .get("properties")
        .and_then(Value::as_object);
    let additional = schema
        .input_schema
        .get("additionalProperties")
        .and_then(Value::as_bool);

    // Check 1: unknown property (additionalProperties: false). Names a typo'd field explicitly.
    if additional == Some(false) {
        if let Some(props) = properties {
            for key in obj.keys() {
                if !props.contains_key(key) {
                    let msg = format!("unexpected field '{key}' (not in this tool's schema)");
                    return Err(
                        ToolError::invalid_request(msg).next_step(unknown_field_hint(schema, key))
                    );
                }
            }
        }
    }

    let required: Vec<String> = schema
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    // Check 2: missing required field. The corrective hint names the field, its type, and (for
    // tabId specifically) where to get one.
    for field in &required {
        if !obj.contains_key(field) {
            let type_hint = properties
                .and_then(|p| p.get(field))
                .and_then(|f| f.get("type"))
                .map(type_descriptor)
                .unwrap_or_else(|| "a value".to_string());
            let msg = format!("missing required field '{field}' ({type_hint})");
            return Err(
                ToolError::invalid_request(msg).next_step(missing_field_hint(schema, field))
            );
        }
    }

    // Check 3: wrong type on each PRESENT field. The trained schemas use simple JSON-Schema types
    // and (for form_input.value) a type union; we accept the value if it matches ANY of them.
    if let Some(props) = properties {
        for (field, value) in obj {
            let Some(spec) = props.get(field) else {
                continue; // already caught by the additionalProperties check above
            };
            if let Some(type_value) = spec.get("type") {
                if !type_matches(value, type_value) {
                    let expected = type_descriptor(type_value);
                    let actual = json_type_name(value);
                    let msg = format!("field '{field}' must be {expected}, got {actual}");
                    return Err(ToolError::invalid_request(msg)
                        .next_step(example_shape_hint(schema, field)));
                }
            }
            if field != "action" {
                if let Some(allowed) = spec.get("enum").and_then(Value::as_array) {
                    if !allowed.contains(value) {
                        let choices = allowed
                            .iter()
                            .map(Value::to_string)
                            .collect::<Vec<_>>()
                            .join(", ");
                        let msg = format!("field '{field}' must be one of {choices}");
                        return Err(ToolError::invalid_request(msg)
                            .next_step(example_shape_hint(schema, field)));
                    }
                }
            }
            if let Some(number) = value.as_f64() {
                if let Some(minimum) = spec.get("minimum").and_then(Value::as_f64) {
                    if number < minimum {
                        let msg = format!("field '{field}' must be at least {minimum}");
                        return Err(ToolError::invalid_request(msg)
                            .next_step(example_shape_hint(schema, field)));
                    }
                }
                if let Some(maximum) = spec.get("maximum").and_then(Value::as_f64) {
                    if number > maximum {
                        let msg = format!("field '{field}' must be at most {maximum}");
                        return Err(ToolError::invalid_request(msg)
                            .next_step(example_shape_hint(schema, field)));
                    }
                }
            }
            if let Some(text) = value.as_str() {
                let length = text.chars().count() as u64;
                if let Some(minimum) = spec.get("minLength").and_then(Value::as_u64) {
                    if length < minimum {
                        let msg =
                            format!("field '{field}' must contain at least {minimum} character(s)");
                        return Err(ToolError::invalid_request(msg)
                            .next_step(example_shape_hint(schema, field)));
                    }
                }
                if let Some(maximum) = spec.get("maxLength").and_then(Value::as_u64) {
                    if length > maximum {
                        let msg =
                            format!("field '{field}' must contain at most {maximum} character(s)");
                        return Err(ToolError::invalid_request(msg)
                            .next_step(example_shape_hint(schema, field)));
                    }
                }
            }
        }
    }

    Ok(())
}

// --- Corrective-suggestion generators (the single piece of "logic" that is not pure fixture
//     lookup is the tabId-specific hint below; everything else is fixture passthrough). ---

/// The corrective hint for a missing required field. The example shape is always shown; for
/// `tabId` specifically, the load-bearing "get one from tabs_context_mcp first" hint is appended
/// (the one hard-coded conditional -- the field every tool needs, but only one such field exists).
fn missing_field_hint(schema: &ToolSchema, field: &str) -> String {
    let base = example_shape_hint(schema, field);
    if field == "tabId" {
        format!(
            "{base}; get one from tabs_context_mcp (createIfEmpty: true) or tabs_create_mcp first"
        )
    } else {
        base
    }
}

/// Render the tool's `example.call` as a shape suggestion for the named field. If the example is
/// present and contains the field, the suggestion quotes just that field's value; otherwise it
/// quotes the whole example object so the model has a known-good shape to copy.
fn example_shape_hint(schema: &ToolSchema, field: &str) -> String {
    match &schema.example_call {
        Some(ex) if ex.get(field).is_some() => {
            format!("pass '{field}' (example value: {})", ex[field])
        }
        Some(ex) => {
            format!("example call shape: {ex}")
        }
        None => format!("pass a value for '{field}' matching the advertised inputSchema"),
    }
}

/// The corrective hint for an unknown field under additionalProperties: false.
fn unknown_field_hint(schema: &ToolSchema, _field: &str) -> String {
    match &schema.example_call {
        Some(ex) => format!("use only fields in this tool's schema; example call shape: {ex}"),
        None => "use only fields in this tool's schema (see tools/list)".to_string(),
    }
}

/// Fallback hint when the arguments object itself is missing/wrong shape.
fn missing_required_hint(schema: &ToolSchema, what: &str) -> ToolError {
    ToolError::invalid_request(format!("missing {what}")).next_step(example_shape_hint(schema, ""))
}

// --- JSON-Schema type helpers ---

/// True iff `value` matches the (possibly union) JSON-Schema `type`. The trained schemas use only
/// the simple primitives plus one union (`form_input.value` is `["string","boolean","number"]`);
/// we accept the value if it matches ANY listed type.
fn type_matches(value: &Value, type_value: &Value) -> bool {
    match type_value {
        Value::String(t) => single_type_matches(value, t),
        Value::Array(types) => types
            .iter()
            .filter_map(|t| t.as_str())
            .any(|t| single_type_matches(value, t)),
        _ => true, // an unknown type spec is not something this validator enforces
    }
}

fn single_type_matches(value: &Value, type_name: &str) -> bool {
    match type_name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "null" => value.is_null(),
        _ => true, // unknown type names are not enforced
    }
}

/// A human-readable name for a JSON-Schema type spec (e.g. `"number"`, or `"string or boolean"`
/// for a union). Used in the corrective message so the model sees the expected shape in words.
fn type_descriptor(type_value: &Value) -> String {
    match type_value {
        Value::String(t) => article(t),
        Value::Array(types) => {
            let parts: Vec<&str> = types.iter().filter_map(|t| t.as_str()).collect();
            match parts.len() {
                0 => "a value".to_string(),
                1 => article(parts[0]),
                _ => {
                    let (last, rest) = parts.split_last().unwrap();
                    format!("{} or {last}", rest.join(", "))
                }
            }
        }
        _ => "a value".to_string(),
    }
}

fn article(type_name: &str) -> String {
    let first = type_name.chars().next().unwrap_or('a');
    let article = if "aeiou".contains(first) { "an" } else { "a" };
    format!("{article} {type_name}")
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative schema + example for the test cases (mirrors `navigate`'s shape).
    fn fixture() -> ToolSchema {
        ToolSchema {
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "tabId": { "type": "number" },
                    "force": { "type": "boolean" }
                },
                "required": ["url", "tabId"],
                "additionalProperties": false
            }),
            example_call: Some(serde_json::json!({ "tabId": 0, "url": "https://example.com" })),
        }
    }

    #[test]
    fn a_well_formed_call_validates() {
        let schema = fixture();
        let args = serde_json::json!({ "tabId": 5, "url": "https://example.com" });
        assert!(validate_arguments(&schema, &args).is_ok());
    }

    #[test]
    fn a_missing_required_field_carries_the_tabid_specific_hint() {
        let schema = fixture();
        let args = serde_json::json!({ "url": "https://example.com" }); // no tabId
        let err = validate_arguments(&schema, &args).unwrap_err();
        let Display { message, next_step } = display(&err);
        assert!(
            message.contains("missing required field 'tabId'"),
            "message: {message}"
        );
        assert!(
            next_step.contains("tabs_context_mcp"),
            "the tabId-specific hint names where to get one: {next_step}"
        );
        assert!(
            next_step.contains("example"),
            "the hint shows the example shape: {next_step}"
        );
    }

    #[test]
    fn a_wrong_type_carries_the_expected_type_and_example() {
        let schema = fixture();
        let args = serde_json::json!({ "tabId": "five", "url": "https://example.com" });
        let err = validate_arguments(&schema, &args).unwrap_err();
        let Display { message, next_step } = display(&err);
        assert!(
            message.contains("'tabId' must be a number"),
            "message: {message}"
        );
        assert!(
            message.contains("got a string"),
            "message names the actual type: {message}"
        );
        assert!(
            next_step.contains("example value") || next_step.contains("example call"),
            "next_step shows the example: {next_step}"
        );
    }

    #[test]
    fn an_unknown_action_value_is_not_enforced_here_governance_handles_it() {
        // Action enum values are deliberately NOT checked by this validator --
        // governance's directory classifies an unknown action as a miss and returns a stable
        // Denied (D-xxxxxxxx) denial, which is more informative than a structural rejection. So an
        // unknown action value must pass this validator and reach governance.
        let schema = ToolSchema {
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["read", "write"] }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
            example_call: Some(serde_json::json!({ "action": "read" })),
        };
        let args = serde_json::json!({ "action": "unknown" });
        assert!(
            validate_arguments(&schema, &args).is_ok(),
            "action enum values are not enforced here; an unknown value passes to governance"
        );
    }

    #[test]
    fn non_action_enums_and_scalar_bounds_are_enforced() {
        let schema = ToolSchema::for_tool("narrate").expect("narrate schema");
        for args in [
            serde_json::json!({ "tabId": 1, "text": "", "position": "bottom" }),
            serde_json::json!({ "tabId": 1, "text": "ok", "position": "sideways" }),
            serde_json::json!({ "tabId": 1, "text": "ok", "duration_ms": 999 }),
            serde_json::json!({ "tabId": 1, "text": "ok", "duration_ms": 30001 }),
        ] {
            assert!(
                validate_arguments(&schema, &args).is_err(),
                "accepted {args}"
            );
        }
        assert!(validate_arguments(
            &schema,
            &serde_json::json!({
                "tabId": 1,
                "text": "ok",
                "position": "auto",
                "duration_ms": 5000
            })
        )
        .is_ok());
    }

    #[test]
    fn an_unexpected_field_under_additional_properties_false_is_named() {
        let schema = fixture();
        let args = serde_json::json!({ "tabId": 5, "url": "https://example.com", "evil": true });
        let err = validate_arguments(&schema, &args).unwrap_err();
        let Display { message, next_step } = display(&err);
        assert!(
            message.contains("unexpected field 'evil'"),
            "the unexpected field is named: {message}"
        );
        assert!(
            next_step.contains("example call"),
            "the hint shows the example shape: {next_step}"
        );
    }

    #[test]
    fn a_non_object_arguments_object_is_caught() {
        let schema = fixture();
        let args = serde_json::json!("not an object");
        let err = validate_arguments(&schema, &args).unwrap_err();
        let Display { message, .. } = display(&err);
        assert!(
            message.contains("missing the arguments object"),
            "a non-object arguments is caught: {message}"
        );
    }

    #[test]
    fn a_form_input_value_union_accepts_string_boolean_or_number() {
        // form_input.value is ["string","boolean","number"]; each must validate.
        let schema = ToolSchema {
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "ref": { "type": "string" },
                    "value": { "type": ["string", "boolean", "number"] },
                    "tabId": { "type": "number" }
                },
                "required": ["ref", "value", "tabId"],
                "additionalProperties": false
            }),
            example_call: Some(serde_json::json!({ "ref": "ref_1", "value": "hello", "tabId": 0 })),
        };
        assert!(validate_arguments(
            &schema,
            &serde_json::json!({ "ref": "ref_1", "value": "x", "tabId": 0 })
        )
        .is_ok());
        assert!(validate_arguments(
            &schema,
            &serde_json::json!({ "ref": "ref_1", "value": true, "tabId": 0 })
        )
        .is_ok());
        assert!(validate_arguments(
            &schema,
            &serde_json::json!({ "ref": "ref_1", "value": 7, "tabId": 0 })
        )
        .is_ok());
        // An object is none of the union types -> rejected.
        let err = validate_arguments(
            &schema,
            &serde_json::json!({ "ref": "ref_1", "value": {}, "tabId": 0 }),
        )
        .unwrap_err();
        assert!(display(&err).message.contains("'value' must be"));
    }

    /// Test-only extraction of the two display fields, mirroring `ToolError`'s `Display` impl.
    struct Display {
        message: String,
        next_step: String,
    }

    fn display(err: &ToolError) -> Display {
        let s = err.to_string();
        // The InvalidRequest Display is "[hop: invalid-request] {message}. Next step: {next_step}."
        let s = s.strip_prefix("[hop: invalid-request] ").unwrap_or(&s);
        let (message, next_step) = s.split_once(". Next step: ").unwrap_or((s, ""));
        Display {
            message: message.to_string(),
            next_step: next_step.trim_end_matches('.').to_string(),
        }
    }
}
