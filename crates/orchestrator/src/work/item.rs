//! The bounded item the model sees for one observed control.
//!
//! The browser adapter already bounds what it sends; this module bounds it again, because the
//! service never trusts a size it did not check. An item always carries its handle, role, name,
//! state and credential flag. An item adds `context` and `nth` when its role and name repeat in
//! the answer, to tell the repeats apart (ADR-0198 rule 1); a credential-class control never
//! carries them. An item adds `area` when the page puts it in a landmark and `form` when a form
//! owns it (ADR-0198 decision 19); both are structure, so a credential-class control carries
//! them too. A control adds `value`, `options`, `type`, `limits`, `hint`,
//! `group` and `error` only when the page gave them (ADR-0198 decision 7). A credential-class
//! control carries its masked value and nothing else, whatever the adapter sent.

use serde_json::{json, Map, Value};

use crate::browser::adapter::ObservedTarget;
use crate::language::area::PageArea;

use super::bounded;

/// The longest role the item carries.
const ROLE_MAX: usize = 100;
/// The longest name the item carries.
const NAME_MAX: usize = 500;
/// The longest value, option text, option value or group label the item carries.
const VALUE_MAX: usize = 80;
/// The longest option text or option value.
const OPTION_TEXT_MAX: usize = 60;
/// The most options (not counting group headings) the item lists.
const OPTIONS_MAX: usize = 25;
/// The most chosen texts a multiple select reports.
const VALUE_LIST_MAX: usize = 25;
/// The longest input type word.
const TYPE_MAX: usize = 20;
/// The longest hint or error text.
const HINT_MAX: usize = 120;
/// The longest group name.
const GROUP_MAX: usize = 80;
/// The longest row or group label in `context`.
const CONTEXT_MAX: usize = 80;
/// The longest declared limit.
const LIMIT_MAX: usize = 120;
/// What a set credential-class value reads as.
const MASKED_VALUE: &str = "[hidden]";
/// The constraint attributes a control can declare, as strings.
const LIMIT_ATTRIBUTES: [&str; 7] = [
    "min",
    "max",
    "step",
    "minlength",
    "maxlength",
    "pattern",
    "accept",
];
/// The limit that is a flag rather than a string.
const MULTIPLE_LIMIT: &str = "multiple";

/// Build the model-facing item for one observed control under its handle.
pub(super) fn control_item(handle: &str, target: &ObservedTarget) -> Value {
    let mut item = Map::new();
    item.insert("target".into(), json!(handle));
    item.insert("role".into(), json!(bounded(&target.role, ROLE_MAX)));
    item.insert("name".into(), json!(bounded(&target.name, NAME_MAX)));
    if !target.credential_class {
        let context = target.context.as_deref().map(one_line);
        insert_text(&mut item, "context", context.as_deref(), CONTEXT_MAX);
        if let Some(nth) = target.nth {
            item.insert("nth".into(), json!(nth));
        }
    }
    if let Some(area) = target.area.as_deref().and_then(PageArea::classify) {
        item.insert("area".into(), json!(area.word()));
    }
    if let Some(form) = target.form {
        item.insert("form".into(), json!(form));
    }
    item.insert("state".into(), json!(target.state));
    item.insert("credential_class".into(), json!(target.credential_class));
    if target.credential_class {
        if let Some(value) = masked(target.value.as_ref()) {
            item.insert("value".into(), json!(value));
        }
        return Value::Object(item);
    }
    if let Some(value) = target.value.as_ref().and_then(bounded_value) {
        let is_text = value.is_string();
        item.insert("value".into(), value);
        if let Some(chars) = target.value_chars.filter(|_| is_text) {
            item.insert("value_chars".into(), json!(chars));
        }
    }
    if let Some(options) = target.options.as_ref().and_then(bounded_options) {
        item.insert("options".into(), options);
        if let Some(total) = target.options_total {
            item.insert("options_total".into(), json!(total));
        }
    }
    insert_text(&mut item, "type", target.input_type.as_deref(), TYPE_MAX);
    if let Some(limits) = target.limits.as_ref().and_then(bounded_limits) {
        item.insert("limits".into(), limits);
    }
    insert_text(&mut item, "hint", target.hint.as_deref(), HINT_MAX);
    insert_text(&mut item, "group", target.group.as_deref(), GROUP_MAX);
    insert_text(&mut item, "error", target.error.as_deref(), HINT_MAX);
    Value::Object(item)
}

/// The item a stale-target refusal lists for one candidate control (ADR-0198 Decision 28): its
/// handle and role always, its place among repeats when it has one, and its name and row context
/// only when governance preserves target names. A credential-class control carries neither
/// context nor place. The page's verdict on whether the candidate is the same control never
/// appears here.
pub(super) fn candidate_item(handle: &str, target: &ObservedTarget, preserve_names: bool) -> Value {
    let mut item = Map::new();
    item.insert("target".into(), json!(handle));
    item.insert("role".into(), json!(bounded(&target.role, ROLE_MAX)));
    if preserve_names {
        insert_text(&mut item, "name", Some(&target.name), NAME_MAX);
        if !target.credential_class {
            let context = target.context.as_deref().map(one_line);
            insert_text(&mut item, "context", context.as_deref(), CONTEXT_MAX);
        }
    }
    if let Some(nth) = target.nth.filter(|_| !target.credential_class) {
        item.insert("nth".into(), json!(nth));
    }
    Value::Object(item)
}

/// The text on one line: every run of whitespace, line breaks included, becomes a single space.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Add a bounded text key when the text is present and not empty.
fn insert_text(item: &mut Map<String, Value>, key: &str, text: Option<&str>, maximum: usize) {
    if let Some(text) = text.filter(|text| !text.is_empty()) {
        item.insert(key.into(), json!(bounded(text, maximum)));
    }
}

/// The value a credential-class control may show: the marker when set, nothing when empty.
/// Absent when the adapter sent no value, which is every control that holds none.
fn masked(value: Option<&Value>) -> Option<&'static str> {
    let set = match value? {
        Value::String(text) => !text.is_empty(),
        Value::Array(texts) => !texts.is_empty(),
        _ => return None,
    };
    Some(if set { MASKED_VALUE } else { "" })
}

/// A string value cut to the cap, or an array of strings cut to the list cap.
fn bounded_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => Some(json!(bounded(text, VALUE_MAX))),
        Value::Array(texts) => Some(json!(texts
            .iter()
            .filter_map(Value::as_str)
            .take(VALUE_LIST_MAX)
            .map(|text| bounded(text, VALUE_MAX))
            .collect::<Vec<_>>())),
        _ => None,
    }
}

/// Rebuild a select's option list: bare texts, `{text,value,disabled}` entries and
/// `{group,options}` groups only, the option total capped, every string cut.
fn bounded_options(options: &Value) -> Option<Value> {
    let mut remaining = OPTIONS_MAX;
    Some(Value::Array(option_entries(
        options.as_array()?,
        &mut remaining,
    )))
}

fn option_entries(entries: &[Value], remaining: &mut usize) -> Vec<Value> {
    let mut kept = Vec::new();
    for entry in entries {
        if *remaining == 0 {
            break;
        }
        if let Some(group) = entry.get("group").and_then(Value::as_str) {
            let inner = entry
                .get("options")
                .and_then(Value::as_array)
                .map(|inner| option_entries(inner, remaining))
                .unwrap_or_default();
            if !inner.is_empty() {
                kept.push(json!({"group": bounded(group, OPTION_TEXT_MAX), "options": inner}));
            }
            continue;
        }
        if let Some(option) = option_entry(entry) {
            *remaining -= 1;
            kept.push(option);
        }
    }
    kept
}

fn option_entry(entry: &Value) -> Option<Value> {
    if let Some(text) = entry.as_str() {
        return Some(json!(bounded(text, OPTION_TEXT_MAX)));
    }
    let text = entry.get("text")?.as_str()?;
    let mut option = Map::new();
    option.insert("text".into(), json!(bounded(text, OPTION_TEXT_MAX)));
    if let Some(value) = entry.get("value").and_then(Value::as_str) {
        option.insert("value".into(), json!(bounded(value, OPTION_TEXT_MAX)));
    }
    if entry.get("disabled") == Some(&Value::Bool(true)) {
        option.insert("disabled".into(), Value::Bool(true));
    }
    Some(Value::Object(option))
}

/// Keep the declared constraint attributes, cut, and the multiple flag.
fn bounded_limits(limits: &Value) -> Option<Value> {
    let declared = limits.as_object()?;
    let mut kept = Map::new();
    for attribute in LIMIT_ATTRIBUTES {
        if let Some(text) = declared.get(attribute).and_then(Value::as_str) {
            kept.insert(attribute.into(), json!(bounded(text, LIMIT_MAX)));
        }
    }
    if declared.get(MULTIPLE_LIMIT) == Some(&Value::Bool(true)) {
        kept.insert(MULTIPLE_LIMIT.into(), Value::Bool(true));
    }
    (!kept.is_empty()).then_some(Value::Object(kept))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> ObservedTarget {
        ObservedTarget {
            locator: "l1".into(),
            role: "combobox".into(),
            name: "Role".into(),
            ..ObservedTarget::default()
        }
    }

    #[test]
    fn a_credential_control_never_carries_context_or_nth() {
        let mut secret = target();
        secret.credential_class = true;
        secret.context = Some("Account".into());
        secret.nth = Some(1);
        let item = control_item("target_1", &secret);
        assert!(item.get("context").is_none() && item.get("nth").is_none());
    }

    #[test]
    fn a_credential_control_keeps_its_structure_and_nothing_else() {
        let mut secret = target();
        secret.credential_class = true;
        secret.area = Some("dialog".into());
        secret.form = Some(1);
        secret.hint = Some("seven characters".into());
        let item = control_item("target_1", &secret);
        assert_eq!(item["area"], json!("dialog"));
        assert_eq!(item["form"], json!(1));
        assert!(item.get("hint").is_none());
    }

    #[test]
    fn a_credential_class_control_shows_only_the_marker_whatever_the_adapter_sent() {
        let mut secret = target();
        secret.credential_class = true;
        secret.value = Some(json!("hunter2"));
        secret.value_chars = Some(7);
        secret.options = Some(json!(["Alpha"]));
        secret.hint = Some("your secret".into());
        secret.error = Some("is too short".into());
        secret.limits = Some(json!({"maxlength":"20"}));
        secret.input_type = Some("email".into());
        secret.group = Some("Account".into());

        let item = control_item("target_1", &secret);

        let mut keys: Vec<_> = item.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "credential_class",
                "name",
                "role",
                "state",
                "target",
                "value"
            ]
        );
        assert_eq!(item["value"], json!("[hidden]"));

        secret.value = Some(json!(""));
        assert_eq!(control_item("target_1", &secret)["value"], json!(""));
        secret.value = None;
        assert!(control_item("target_1", &secret).get("value").is_none());
    }
}
