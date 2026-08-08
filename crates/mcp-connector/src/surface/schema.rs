// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Closed JSON-Schema subset used to validate edge-owned tool surfaces.
//!
//! Ghostlight keeps model-facing declarations at the protocol edge. This validator implements
//! every assertion keyword present in the frozen `ghostlight-legacy/v1` input schemas and rejects
//! unknown future keywords rather than silently weakening the advertised contract.

use serde_json::{Map, Value};

const SUPPORTED_KEYWORDS: &[&str] = &[
    "additionalProperties",
    "allOf",
    "const",
    "default",
    "description",
    "else",
    "enum",
    "if",
    "items",
    "maximum",
    "maxItems",
    "maxLength",
    "minimum",
    "minItems",
    "minLength",
    "minProperties",
    "not",
    "properties",
    "required",
    "then",
    "type",
];

/// One fail-closed schema or instance validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidationError {
    kind: ErrorKind,
    path: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorKind {
    Instance,
    Schema,
}

impl ValidationError {
    fn instance(path: &str, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Instance,
            path: path.to_owned(),
            message: message.into(),
        }
    }

    fn schema(path: &str, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Schema,
            path: path.to_owned(),
            message: message.into(),
        }
    }

    fn is_instance(&self) -> bool {
        self.kind == ErrorKind::Instance
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ErrorKind::Instance => write!(formatter, "{}: {}", self.path, self.message),
            ErrorKind::Schema => write!(
                formatter,
                "frozen input schema at {} is invalid: {}",
                self.path, self.message
            ),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate one JSON value against the closed schema subset used by edge profiles.
pub(super) fn validate(schema: &Value, instance: &Value) -> Result<(), ValidationError> {
    validate_at(schema, instance, "$")
}

fn validate_at(schema: &Value, instance: &Value, path: &str) -> Result<(), ValidationError> {
    let schema = schema
        .as_object()
        .ok_or_else(|| ValidationError::schema(path, "schema must be an object"))?;
    reject_unknown_keywords(schema, path)?;

    if let Some(type_spec) = schema.get("type") {
        validate_type(type_spec, instance, path)?;
    }
    if let Some(expected) = schema.get("const") {
        if instance != expected {
            return Err(ValidationError::instance(
                path,
                format!("must equal {expected}"),
            ));
        }
    }
    if let Some(allowed) = schema.get("enum") {
        let allowed = allowed
            .as_array()
            .ok_or_else(|| ValidationError::schema(path, "enum must be an array"))?;
        if !allowed.contains(instance) {
            let choices = allowed
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(ValidationError::instance(
                path,
                format!("must be one of {choices}"),
            ));
        }
    }

    if let Some(object) = instance.as_object() {
        validate_object(schema, object, path)?;
    }
    if let Some(array) = instance.as_array() {
        validate_array(schema, array, path)?;
    }
    if let Some(text) = instance.as_str() {
        validate_string(schema, text, path)?;
    }
    if let Some(number) = instance.as_f64() {
        validate_number(schema, number, path)?;
    }

    if let Some(all_of) = schema.get("allOf") {
        let all_of = all_of
            .as_array()
            .ok_or_else(|| ValidationError::schema(path, "allOf must be an array"))?;
        for member in all_of {
            validate_at(member, instance, path)?;
        }
    }

    if let Some(condition) = schema.get("if") {
        let condition_matches = match validate_at(condition, instance, path) {
            Ok(()) => true,
            Err(error) if error.is_instance() => false,
            Err(error) => return Err(error),
        };
        let branch = if condition_matches {
            schema.get("then")
        } else {
            schema.get("else")
        };
        if let Some(branch) = branch {
            validate_at(branch, instance, path)?;
        }
    }

    if let Some(negated) = schema.get("not") {
        match validate_at(negated, instance, path) {
            Ok(()) => {
                return Err(ValidationError::instance(
                    path,
                    "must not match the forbidden schema",
                ));
            }
            Err(error) if error.is_instance() => {}
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

fn reject_unknown_keywords(schema: &Map<String, Value>, path: &str) -> Result<(), ValidationError> {
    if let Some(keyword) = schema
        .keys()
        .find(|keyword| !SUPPORTED_KEYWORDS.contains(&keyword.as_str()))
    {
        return Err(ValidationError::schema(
            path,
            format!("unsupported keyword '{keyword}'"),
        ));
    }
    Ok(())
}

fn validate_type(type_spec: &Value, instance: &Value, path: &str) -> Result<(), ValidationError> {
    let names = match type_spec {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names) => {
            let parsed = names
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| ValidationError::schema(path, "type array must contain strings"))?;
            if parsed.is_empty() {
                return Err(ValidationError::schema(
                    path,
                    "type array must not be empty",
                ));
            }
            parsed
        }
        _ => {
            return Err(ValidationError::schema(
                path,
                "type must be a string or string array",
            ))
        }
    };
    for name in &names {
        if !matches!(
            *name,
            "array" | "boolean" | "integer" | "null" | "number" | "object" | "string"
        ) {
            return Err(ValidationError::schema(
                path,
                format!("unsupported type '{name}'"),
            ));
        }
    }
    if names.iter().any(|name| type_matches(instance, name)) {
        return Ok(());
    }
    Err(ValidationError::instance(
        path,
        format!("must have type {}", names.join(" or ")),
    ))
}

fn type_matches(instance: &Value, expected: &str) -> bool {
    match expected {
        "array" => instance.is_array(),
        "boolean" => instance.is_boolean(),
        "integer" => {
            instance.as_i64().is_some()
                || instance.as_u64().is_some()
                || instance
                    .as_f64()
                    .is_some_and(|number| number.fract() == 0.0)
        }
        "null" => instance.is_null(),
        "number" => instance.is_number(),
        "object" => instance.is_object(),
        "string" => instance.is_string(),
        _ => false,
    }
}

fn validate_object(
    schema: &Map<String, Value>,
    object: &Map<String, Value>,
    path: &str,
) -> Result<(), ValidationError> {
    let properties = match schema.get("properties") {
        Some(value) => Some(
            value
                .as_object()
                .ok_or_else(|| ValidationError::schema(path, "properties must be an object"))?,
        ),
        None => None,
    };
    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| ValidationError::schema(path, "required must be an array"))?;
        for field in required {
            let field = field
                .as_str()
                .ok_or_else(|| ValidationError::schema(path, "required entries must be strings"))?;
            if !object.contains_key(field) {
                return Err(ValidationError::instance(
                    path,
                    format!("missing required field '{field}'"),
                ));
            }
        }
    }
    if let Some(minimum) = schema.get("minProperties") {
        let minimum = schema_usize(minimum, path, "minProperties")?;
        if object.len() < minimum {
            return Err(ValidationError::instance(
                path,
                format!("must contain at least {minimum} properties"),
            ));
        }
    }

    for (field, value) in object {
        let child_path = field_path(path, field);
        if let Some(property_schema) = properties.and_then(|properties| properties.get(field)) {
            validate_at(property_schema, value, &child_path)?;
            continue;
        }
        match schema.get("additionalProperties") {
            None | Some(Value::Bool(true)) => {}
            Some(Value::Bool(false)) => {
                return Err(ValidationError::instance(
                    &child_path,
                    format!("unexpected field '{field}'"),
                ));
            }
            Some(additional_schema @ Value::Object(_)) => {
                validate_at(additional_schema, value, &child_path)?;
            }
            Some(_) => {
                return Err(ValidationError::schema(
                    path,
                    "additionalProperties must be a boolean or schema object",
                ));
            }
        }
    }
    Ok(())
}

fn validate_array(
    schema: &Map<String, Value>,
    array: &[Value],
    path: &str,
) -> Result<(), ValidationError> {
    if let Some(minimum) = schema.get("minItems") {
        let minimum = schema_usize(minimum, path, "minItems")?;
        if array.len() < minimum {
            return Err(ValidationError::instance(
                path,
                format!("must contain at least {minimum} items"),
            ));
        }
    }
    if let Some(maximum) = schema.get("maxItems") {
        let maximum = schema_usize(maximum, path, "maxItems")?;
        if array.len() > maximum {
            return Err(ValidationError::instance(
                path,
                format!("must contain at most {maximum} items"),
            ));
        }
    }
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in array.iter().enumerate() {
            validate_at(item_schema, item, &format!("{path}/{index}"))?;
        }
    }
    Ok(())
}

fn validate_string(
    schema: &Map<String, Value>,
    text: &str,
    path: &str,
) -> Result<(), ValidationError> {
    let length = text.chars().count();
    if let Some(minimum) = schema.get("minLength") {
        let minimum = schema_usize(minimum, path, "minLength")?;
        if length < minimum {
            return Err(ValidationError::instance(
                path,
                format!("must contain at least {minimum} characters"),
            ));
        }
    }
    if let Some(maximum) = schema.get("maxLength") {
        let maximum = schema_usize(maximum, path, "maxLength")?;
        if length > maximum {
            return Err(ValidationError::instance(
                path,
                format!("must contain at most {maximum} characters"),
            ));
        }
    }
    Ok(())
}

fn validate_number(
    schema: &Map<String, Value>,
    number: f64,
    path: &str,
) -> Result<(), ValidationError> {
    if let Some(minimum) = schema.get("minimum") {
        let minimum = schema_number(minimum, path, "minimum")?;
        if number < minimum {
            return Err(ValidationError::instance(
                path,
                format!("must be at least {minimum}"),
            ));
        }
    }
    if let Some(maximum) = schema.get("maximum") {
        let maximum = schema_number(maximum, path, "maximum")?;
        if number > maximum {
            return Err(ValidationError::instance(
                path,
                format!("must be at most {maximum}"),
            ));
        }
    }
    Ok(())
}

fn schema_usize(value: &Value, path: &str, keyword: &str) -> Result<usize, ValidationError> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            ValidationError::schema(path, format!("{keyword} must be a non-negative integer"))
        })
}

fn schema_number(value: &Value, path: &str, keyword: &str) -> Result<f64, ValidationError> {
    value
        .as_f64()
        .ok_or_else(|| ValidationError::schema(path, format!("{keyword} must be a number")))
}

fn field_path(parent: &str, field: &str) -> String {
    let escaped = field.replace('~', "~0").replace('/', "~1");
    format!("{parent}/{escaped}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recursively_enforces_nested_shapes_and_additional_property_schemas() {
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array", "minItems": 1, "maxItems": 2,
                    "items": {
                        "type": "object",
                        "properties": {"state": {"type": "string", "enum": ["ready"]}},
                        "required": ["state"],
                        "additionalProperties": false
                    }
                },
                "fields": {
                    "type": "object", "minProperties": 1,
                    "additionalProperties": {"type": ["string", "number"]}
                }
            },
            "required": ["items", "fields"],
            "additionalProperties": false
        });
        assert!(validate(
            &schema,
            &json!({"items":[{"state":"ready"}],"fields":{"name":"Ada","age":37}})
        )
        .is_ok());
        for invalid in [
            json!({"items":[],"fields":{"name":"Ada"}}),
            json!({"items":[{"state":"later"}],"fields":{"name":"Ada"}}),
            json!({"items":[{"state":"ready","extra":true}],"fields":{"name":"Ada"}}),
            json!({"items":[{"state":"ready"}],"fields":{"name":true}}),
        ] {
            assert!(validate(&schema, &invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn evaluates_the_frozen_conditional_shape() {
        let schema = json!({
            "type": "object",
            "properties": {"action":{"type":"string"},"text":{"type":"string"}},
            "required": ["action"],
            "allOf": [{
                "if": {"properties":{"action":{"const":"respond"}},"required":["action"]},
                "then": {"required":["text"]},
                "else": {"not":{"required":["text"]}}
            }],
            "additionalProperties": false
        });
        assert!(validate(&schema, &json!({"action":"respond","text":"Ada"})).is_ok());
        assert!(validate(&schema, &json!({"action":"status"})).is_ok());
        assert!(validate(&schema, &json!({"action":"respond"})).is_err());
        assert!(validate(&schema, &json!({"action":"status","text":"ignored"})).is_err());
    }

    #[test]
    fn unknown_schema_keywords_fail_closed() {
        let error = validate(&json!({"type":"string","pattern":".*"}), &json!("value"))
            .expect_err("unsupported keyword must fail");
        assert!(error.to_string().contains("unsupported keyword 'pattern'"));
    }

    #[test]
    fn enforces_scalar_bounds_and_integer_types() {
        for (schema, invalid) in [
            (json!({"type":"number","minimum":1,"maximum":2}), json!(0)),
            (json!({"type":"number","minimum":1,"maximum":2}), json!(3)),
            (
                json!({"type":"string","minLength":2,"maxLength":3}),
                json!("x"),
            ),
            (
                json!({"type":"string","minLength":2,"maxLength":3}),
                json!("long"),
            ),
            (json!({"type":"integer"}), json!(1.5)),
        ] {
            assert!(validate(&schema, &invalid).is_err(), "accepted {invalid}");
        }
        assert!(validate(
            &json!({"type":"number","minimum":1,"maximum":2}),
            &json!(1.5)
        )
        .is_ok());
        assert!(validate(
            &json!({"type":"string","minLength":2,"maxLength":3}),
            &json!("ok")
        )
        .is_ok());
        assert!(validate(&json!({"type":"integer"}), &json!(1.0)).is_ok());
    }
}
