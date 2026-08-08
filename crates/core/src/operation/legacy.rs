// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Bounded R1 compatibility decoder for in-process legacy callers.
//!
//! Protocol edges own long-term surface translation. This duplicate exists only while local
//! orchestrators and focused core fixtures still invoke frozen Ghostlight tool names. It must
//! remain equivalent to the edge decoder and is deleted when those callers become canonical.

use crate::ToolError;
use ghostlight_transport::operation::{BrowserOperation, IntentId, OperationId};
use serde_json::{json, Map, Value};

/// Normalize one frozen Ghostlight legacy call into a canonical browser operation.
pub fn decode_call(name: &str, arguments: &Value) -> Result<BrowserOperation, ToolError> {
    decode_call_inner(name, arguments, false)
}

fn decode_call_inner(
    name: &str,
    arguments: &Value,
    allow_deferred_references: bool,
) -> Result<BrowserOperation, ToolError> {
    let schema = crate::tool::validation::ToolSchema::for_tool(name)
        .ok_or_else(|| ToolError::invalid_request(format!("Unknown tool: {name}")))?;
    let validation_arguments = if allow_deferred_references {
        deferred_validation_view(&schema.input_schema, arguments)
    } else {
        arguments.clone()
    };
    crate::tool::validation::validate_arguments(&schema, &validation_arguments)?;
    let args = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request(format!("{name} arguments must be an object")))?;
    match name {
        "tabs_context_mcp" => simple(
            OperationId::BrowserTabs,
            IntentId::TabsList,
            rename(args, &[("createIfEmpty", "create_if_empty")]),
        ),
        "tabs_create_mcp" => simple(OperationId::BrowserTabs, IntentId::TabsNew, args),
        "navigate" => decode_navigate(args),
        "computer" => decode_computer(args),
        "find" => simple(
            OperationId::BrowserFind,
            IntentId::FindQuery,
            with_tab(args),
        ),
        "form_input" => decode_form_input(args),
        "get_page_text" => simple(OperationId::BrowserRead, IntentId::ReadText, with_tab(args)),
        "javascript_tool" => decode_javascript(args),
        "read_console_messages" => decode_console(args),
        "read_network_requests" => decode_network(args),
        "read_page" => simple(
            OperationId::BrowserSnapshot,
            IntentId::SnapshotCapture,
            rename(with_tab(args), &[("ref_id", "scope_ref")]),
        ),
        "resize_window" => simple(
            OperationId::BrowserViewport,
            IntentId::ViewportResizeWindow,
            with_tab(args),
        ),
        "update_plan" => simple(OperationId::WorkflowPlan, IntentId::PlanUpdate, args),
        "narrate" => simple(
            OperationId::BrowserPresent,
            IntentId::PresentNarrate,
            with_tab(args),
        ),
        "wait_for" => decode_wait_for(args),
        "script" => decode_script(args),
        "form_fill" => decode_form_fill(args),
        "act_on" => decode_act_on(args),
        "dialog" => decode_dialog(args),
        "tab_control" => decode_tab_control(args),
        "file_upload" => decode_file_upload(args),
        "browser_batch" => decode_browser_batch(args),
        "upload_image" => decode_upload_image(args),
        "gif_creator" => decode_gif(args),
        "explain" => simple(OperationId::BrowserContext, IntentId::ContextDescribe, args),
        other => Err(ToolError::invalid_request(format!("Unknown tool: {other}"))),
    }
}

fn simple(
    id: OperationId,
    intent: IntentId,
    arguments: Map<String, Value>,
) -> Result<BrowserOperation, ToolError> {
    Ok(BrowserOperation::new(id, intent, Value::Object(arguments)))
}

fn with_tab(args: Map<String, Value>) -> Map<String, Value> {
    rename(args, &[("tabId", "tab")])
}

fn rename(
    mut args: Map<String, Value>,
    fields: &[(&'static str, &'static str)],
) -> Map<String, Value> {
    for (old, new) in fields {
        if let Some(value) = args.remove(*old) {
            args.insert((*new).to_owned(), value);
        }
    }
    args
}

fn move_field(
    source: &mut Map<String, Value>,
    target: &mut Map<String, Value>,
    source_name: &str,
    target_name: &str,
) {
    if let Some(value) = source.remove(source_name) {
        target.insert(target_name.to_owned(), value);
    }
}

fn take_tab(args: &mut Map<String, Value>) -> Map<String, Value> {
    let mut canonical = Map::new();
    move_field(args, &mut canonical, "tabId", "tab");
    canonical
}

fn is_deferred_reference(value: &Value) -> bool {
    value.as_str().is_some_and(is_deferred_reference_string)
}

fn is_deferred_reference_string(value: &str) -> bool {
    let Some(body) = value.strip_prefix('$') else {
        return false;
    };
    if body.starts_with('$') {
        return false;
    }
    let rest = if let Some(rest) = body.strip_prefix("prev") {
        rest
    } else {
        let digit_count = body.bytes().take_while(u8::is_ascii_digit).count();
        if digit_count == 0 || body.as_bytes()[0] == b'0' {
            return false;
        }
        &body[digit_count..]
    };
    rest.is_empty()
        || rest
            .strip_prefix('.')
            .is_some_and(|path| !path.is_empty() && path.split('.').all(|part| !part.is_empty()))
}

fn deferred_validation_view(schema: &Value, instance: &Value) -> Value {
    if is_deferred_reference(instance) {
        return validation_placeholder(schema);
    }
    match instance {
        Value::Object(object) => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let additional = schema.get("additionalProperties");
            Value::Object(
                object
                    .iter()
                    .map(|(field, value)| {
                        let field_schema = properties
                            .and_then(|properties| properties.get(field))
                            .or_else(|| additional.filter(|value| value.is_object()))
                            .unwrap_or(&Value::Null);
                        (field.clone(), deferred_validation_view(field_schema, value))
                    })
                    .collect(),
            )
        }
        Value::Array(items) => {
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                items
                    .iter()
                    .map(|item| deferred_validation_view(item_schema, item))
                    .collect(),
            )
        }
        _ => instance.clone(),
    }
}

fn validation_placeholder(schema: &Value) -> Value {
    if let Some(value) = schema.get("const") {
        return value.clone();
    }
    if let Some(value) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    {
        return value.clone();
    }
    if let Some(value) = schema.get("default") {
        return value.clone();
    }
    let type_name = match schema.get("type") {
        Some(Value::String(name)) => Some(name.as_str()),
        Some(Value::Array(names)) => names.first().and_then(Value::as_str),
        _ => None,
    };
    match type_name {
        Some("string") => {
            let length = schema.get("minLength").and_then(Value::as_u64).unwrap_or(1) as usize;
            Value::String("x".repeat(length))
        }
        Some("number") | Some("integer") => {
            schema.get("minimum").cloned().unwrap_or_else(|| json!(0))
        }
        Some("boolean") => Value::Bool(false),
        Some("array") => {
            let count = schema.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize;
            let item_schema = schema.get("items").unwrap_or(&Value::Null);
            Value::Array(
                (0..count)
                    .map(|_| validation_placeholder(item_schema))
                    .collect(),
            )
        }
        Some("object") => {
            let properties = schema.get("properties").and_then(Value::as_object);
            let mut object = Map::new();
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for field in required.iter().filter_map(Value::as_str) {
                    let field_schema = properties
                        .and_then(|properties| properties.get(field))
                        .unwrap_or(&Value::Null);
                    object.insert(field.to_owned(), validation_placeholder(field_schema));
                }
            }
            let minimum = schema
                .get("minProperties")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            if object.len() < minimum {
                let additional = schema
                    .get("additionalProperties")
                    .filter(|value| value.is_object())
                    .unwrap_or(&Value::Null);
                object.insert("deferred".into(), validation_placeholder(additional));
            }
            Value::Object(object)
        }
        Some("null") | None => Value::Null,
        Some(_) => Value::Null,
    }
}

fn non_empty_string_or_reference(value: &Value) -> bool {
    is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.trim().is_empty())
}

fn valid_point_or_reference(value: &Value) -> bool {
    is_deferred_reference(value)
        || value.as_array().is_some_and(|point| {
            point.len() == 2
                && point
                    .iter()
                    .all(|coordinate| coordinate.is_number() || is_deferred_reference(coordinate))
        })
}

fn reject_deferred_discriminant(
    tool: &'static str,
    field: &'static str,
    value: Option<&Value>,
) -> Result<(), ToolError> {
    if value.is_some_and(is_deferred_reference) {
        return Err(ToolError::invalid_request(format!(
            "{tool}.{field} cannot be a deferred reference because it selects a canonical intent"
        )));
    }
    Ok(())
}

fn declares_argument(tool: &str, field: &str) -> bool {
    crate::tool::validation::ToolSchema::for_tool(tool)
        .and_then(|schema| schema.input_schema.get("properties").cloned())
        .and_then(|properties| properties.as_object().cloned())
        .is_some_and(|properties| properties.contains_key(field))
}

fn take_action(tool: &'static str, args: &mut Map<String, Value>) -> Result<String, ToolError> {
    args.remove("action")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| ToolError::invalid_request(format!("{tool} requires a string action")))
}

fn unknown_action(tool: &'static str, action: &str) -> ToolError {
    ToolError::invalid_request(format!("Unknown {tool} action: {action}"))
}

fn decode_navigate(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let intent = match args.get("url").and_then(Value::as_str) {
        Some("back") => {
            args.remove("url");
            IntentId::NavigateBack
        }
        Some("forward") => {
            args.remove("url");
            IntentId::NavigateForward
        }
        _ => IntentId::NavigateUrl,
    };
    simple(OperationId::BrowserNavigate, intent, with_tab(args))
}

fn decode_computer(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("computer", &mut args)?;
    let coordinate = args.remove("coordinate");
    let reference = args.remove("ref");
    let mut canonical = take_tab(&mut args);
    match action.as_str() {
        "left_click" | "right_click" | "double_click" | "triple_click" | "hover" => {
            let coordinate_intent = match action.as_str() {
                "left_click" => IntentId::InputPointerClick,
                "right_click" => IntentId::InputPointerRightClick,
                "double_click" => IntentId::InputPointerDoubleClick,
                "triple_click" => IntentId::InputPointerTripleClick,
                "hover" => IntentId::InputPointerHover,
                _ => unreachable!(),
            };
            let reference_intent = match action.as_str() {
                "left_click" => IntentId::ActClick,
                "right_click" => IntentId::ActRightClick,
                "double_click" => IntentId::ActDoubleClick,
                "triple_click" => IntentId::ActTripleClick,
                "hover" => IntentId::ActHover,
                _ => unreachable!(),
            };
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
                simple(OperationId::BrowserInput, coordinate_intent, canonical)
            } else if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
                simple(OperationId::BrowserAct, reference_intent, canonical)
            } else {
                Err(ToolError::invalid_request(format!(
                    "computer {action} requires coordinate or a non-empty ref"
                )))
            }
        }
        "screenshot" => simple(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            canonical,
        ),
        "zoom" => {
            let region = args
                .remove("region")
                .ok_or_else(|| ToolError::invalid_request("computer zoom requires region"))?;
            canonical.insert("region".into(), region);
            simple(
                OperationId::BrowserScreenshot,
                IntentId::ScreenshotRegion,
                canonical,
            )
        }
        "wait" => {
            let seconds = match args.remove("duration") {
                None => json!(1),
                Some(value) if value.as_f64() == Some(0.0) => json!(1),
                Some(value) => value,
            };
            canonical.insert("seconds".into(), seconds);
            simple(OperationId::BrowserWait, IntentId::WaitDelay, canonical)
        }
        "type" => {
            let text = args
                .remove("text")
                .filter(non_empty_string_or_reference)
                .ok_or_else(|| {
                    ToolError::invalid_request("computer type requires non-empty text")
                })?;
            canonical.insert("text".into(), text);
            simple(
                OperationId::BrowserInput,
                IntentId::InputTypeText,
                canonical,
            )
        }
        "key" => {
            let key = args
                .remove("text")
                .filter(non_empty_string_or_reference)
                .ok_or_else(|| {
                    ToolError::invalid_request("computer key requires non-empty text")
                })?;
            canonical.insert("key".into(), key);
            canonical.insert(
                "repeat".into(),
                args.remove("repeat").unwrap_or_else(|| json!(1)),
            );
            simple(
                OperationId::BrowserInput,
                IntentId::InputPressKey,
                canonical,
            )
        }
        "scroll" => {
            if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
            } else if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
            }
            canonical.insert(
                "direction".into(),
                args.remove("scroll_direction")
                    .unwrap_or_else(|| json!("down")),
            );
            canonical.insert(
                "amount".into(),
                args.remove("scroll_amount").unwrap_or_else(|| json!(3)),
            );
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            simple(OperationId::BrowserInput, IntentId::InputWheel, canonical)
        }
        "scroll_to" => {
            if let Some(reference) = reference.filter(non_empty_string_or_reference) {
                canonical.insert("target".into(), json!({"ref": reference}));
                simple(
                    OperationId::BrowserAct,
                    IntentId::ActScrollIntoView,
                    canonical,
                )
            } else if let Some(point) = coordinate {
                canonical.insert("point".into(), point);
                simple(
                    OperationId::BrowserInput,
                    IntentId::InputScrollToOffset,
                    canonical,
                )
            } else {
                Err(ToolError::invalid_request(
                    "computer scroll_to requires ref or coordinate",
                ))
            }
        }
        "left_click_drag" => {
            let from = args.remove("start_coordinate").ok_or_else(|| {
                ToolError::invalid_request("computer left_click_drag requires start_coordinate")
            })?;
            let to = coordinate.ok_or_else(|| {
                ToolError::invalid_request("computer left_click_drag requires coordinate")
            })?;
            canonical.insert("from".into(), from);
            canonical.insert("to".into(), to);
            move_field(&mut args, &mut canonical, "modifiers", "modifiers");
            simple(
                OperationId::BrowserInput,
                IntentId::InputPointerDrag,
                canonical,
            )
        }
        other => Err(unknown_action("computer", other)),
    }
}

fn decode_form_input(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let reference = args
        .remove("ref")
        .filter(non_empty_string_or_reference)
        .ok_or_else(|| ToolError::invalid_request("form_input requires a non-empty ref"))?;
    let mut canonical = with_tab(args);
    canonical.insert("target".into(), json!({"ref": reference}));
    simple(OperationId::BrowserFill, IntentId::FillField, canonical)
}

fn decode_javascript(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("javascript_tool", &mut args)?;
    if action != "javascript_exec" {
        return Err(unknown_action("javascript_tool", &action));
    }
    simple(
        OperationId::BrowserEvaluate,
        IntentId::EvaluateJavascript,
        rename(with_tab(args), &[("text", "script")]),
    )
}

fn decode_console(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    reject_deferred_discriminant("read_console_messages", "clear", args.get("clear"))?;
    let clear = args
        .remove("clear")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    simple(
        OperationId::BrowserConsole,
        if clear {
            IntentId::ConsoleReadAndClear
        } else {
            IntentId::ConsoleRead
        },
        rename(with_tab(args), &[("onlyErrors", "only_errors")]),
    )
}

fn decode_network(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    reject_deferred_discriminant("read_network_requests", "clear", args.get("clear"))?;
    let clear = args
        .remove("clear")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    simple(
        OperationId::BrowserNetwork,
        if clear {
            IntentId::NetworkReadAndClear
        } else {
            IntentId::NetworkRead
        },
        rename(with_tab(args), &[("urlPattern", "url_pattern")]),
    )
}

fn decode_wait_for(args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let selector = args.get("selector").filter(|value| {
        is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.is_empty())
    });
    let text = args.get("text").filter(|value| {
        is_deferred_reference(value) || value.as_str().is_some_and(|value| !value.is_empty())
    });
    if selector.is_some() && text.is_some() {
        return Err(ToolError::invalid_request(
            "wait_for accepts at most one non-empty selector or text",
        ));
    }
    if args.get("state").and_then(Value::as_str) == Some("settled")
        && (selector.is_some() || text.is_some())
    {
        return Err(ToolError::invalid_request(
            "wait_for state settled cannot be combined with selector or text",
        ));
    }
    let timeout = args.get("timeout_ms").and_then(Value::as_f64);
    let minimum = args.get("min_ms").and_then(Value::as_f64);
    if timeout.is_some_and(|value| value > 30_000.0) {
        return Err(ToolError::invalid_request(
            "wait_for timeout_ms must not exceed 30000",
        ));
    }
    if minimum
        .zip(timeout)
        .is_some_and(|(minimum, timeout)| minimum > timeout)
    {
        return Err(ToolError::invalid_request(
            "wait_for min_ms must not exceed timeout_ms",
        ));
    }
    simple(
        OperationId::BrowserWait,
        IntentId::WaitUntil,
        with_tab(args),
    )
}

fn decode_script(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    reject_deferred_discriminant("script", "dry_run", args.get("dry_run"))?;
    let preflight = args
        .remove("dry_run")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let steps = args
        .remove("steps")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| ToolError::invalid_request("script requires a steps array"))?;
    let mut canonical_steps = Vec::with_capacity(steps.len());
    let mut retained_tab = args.get("tabId").cloned();
    for step in steps {
        let object = step
            .as_object()
            .ok_or_else(|| ToolError::invalid_request("script step must be an object"))?;
        let tool = object
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::invalid_request("script step requires tool"))?;
        if matches!(tool, "script" | "browser_batch") {
            return Err(ToolError::invalid_request(
                "flows cannot contain another flow",
            ));
        }
        let mut step_args = object.get("args").cloned().unwrap_or_else(|| json!({}));
        if declares_argument(tool, "tabId") {
            if let (Some(step_args), Some(tab)) = (step_args.as_object_mut(), retained_tab.as_ref())
            {
                step_args.entry("tabId").or_insert_with(|| tab.clone());
            }
        }
        let operation = decode_call_inner(tool, &step_args, true)?;
        if let Some(tab) = operation.arguments.get("tab") {
            retained_tab = Some(tab.clone());
        }
        canonical_steps
            .push(serde_json::to_value(operation).expect("canonical operation serializes"));
    }
    let mut canonical = rename(with_tab(args), &[("onError", "on_error")]);
    canonical.insert("steps".into(), Value::Array(canonical_steps));
    simple(
        OperationId::BrowserFlow,
        if preflight {
            IntentId::FlowPreflight
        } else {
            IntentId::FlowExecute
        },
        canonical,
    )
}

fn decode_browser_batch(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let actions = args
        .remove("actions")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| ToolError::invalid_request("browser_batch requires an actions array"))?;
    let mut steps = Vec::with_capacity(actions.len());
    for action in actions {
        let object = action
            .as_object()
            .ok_or_else(|| ToolError::invalid_request("browser_batch action must be an object"))?;
        let tool = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::invalid_request("browser_batch action requires name"))?;
        if matches!(tool, "script" | "browser_batch") {
            return Err(ToolError::invalid_request(
                "flows cannot contain another flow",
            ));
        }
        let input = object.get("input").cloned().unwrap_or_else(|| json!({}));
        steps.push(
            serde_json::to_value(decode_call(tool, &input)?)
                .expect("canonical operation serializes"),
        );
    }
    let mut canonical = rename(with_tab(args), &[("onError", "on_error")]);
    canonical.insert("steps".into(), Value::Array(steps));
    canonical.entry("on_error").or_insert_with(|| json!("stop"));
    simple(OperationId::BrowserFlow, IntentId::FlowExecute, canonical)
}

fn decode_form_fill(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    reject_deferred_discriminant("form_fill", "submit", args.get("submit"))?;
    let submit = args
        .remove("submit")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let fields = match args
        .remove("fields")
        .ok_or_else(|| ToolError::invalid_request("form_fill requires fields"))?
    {
        Value::Object(fields) => Value::Array(
            fields
                .into_iter()
                .map(|(query, value)| {
                    if query.trim().is_empty() {
                        return Err(ToolError::invalid_request(
                            "form_fill field queries must not be empty",
                        ));
                    }
                    Ok(json!({"target": {"query": query}, "value": value}))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        reference if is_deferred_reference(&reference) => reference,
        _ => {
            return Err(ToolError::invalid_request(
                "form_fill fields must be an object or deferred reference",
            ))
        }
    };
    let mut canonical = with_tab(args);
    canonical.insert("fields".into(), fields);
    simple(
        OperationId::BrowserFill,
        if submit {
            IntentId::FillFieldsAndSubmit
        } else {
            IntentId::FillFields
        },
        canonical,
    )
}

fn decode_act_on(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("act_on", &mut args)?;
    let intent = match action.as_str() {
        "left_click" => IntentId::ActClick,
        "right_click" => IntentId::ActRightClick,
        "double_click" => IntentId::ActDoubleClick,
        "hover" => IntentId::ActHover,
        "scroll_to" => IntentId::ActScrollIntoView,
        "set_value" => IntentId::ActSetValue,
        other => return Err(unknown_action("act_on", other)),
    };
    let target = args
        .get("target")
        .ok_or_else(|| ToolError::invalid_request("act_on requires target"))?;
    if !is_deferred_reference(target) {
        let target = target
            .as_object()
            .ok_or_else(|| ToolError::invalid_request("act_on target must be an object"))?;
        if target
            .keys()
            .any(|field| !matches!(field.as_str(), "ref" | "query" | "name" | "role"))
        {
            return Err(ToolError::invalid_request(
                "act_on target contains an unsupported field",
            ));
        }
        let modes = ["ref", "query", "name"]
            .iter()
            .filter(|field| {
                target
                    .get(**field)
                    .is_some_and(non_empty_string_or_reference)
            })
            .count();
        if modes != 1 {
            return Err(ToolError::invalid_request(
                "act_on target requires exactly one non-empty ref, query, or name",
            ));
        }
        if target.contains_key("role") && !target.contains_key("name") {
            return Err(ToolError::invalid_request(
                "act_on target.role is valid only with target.name",
            ));
        }
        if target
            .get("role")
            .is_some_and(|role| !non_empty_string_or_reference(role))
        {
            return Err(ToolError::invalid_request(
                "act_on target.role must be a non-empty string or deferred reference",
            ));
        }
    }
    let has_value = args.contains_key("value");
    if action == "set_value" && !has_value {
        return Err(ToolError::invalid_request(
            "act_on set_value requires value",
        ));
    }
    if action != "set_value" && has_value {
        return Err(ToolError::invalid_request(
            "act_on value is valid only for set_value",
        ));
    }
    if let Some(expect) = args.get("expect") {
        if !is_deferred_reference(expect) {
            let expect = expect
                .as_object()
                .ok_or_else(|| ToolError::invalid_request("act_on expect must be an object"))?;
            let modes = ["selector", "text"]
                .iter()
                .filter(|field| {
                    expect
                        .get(**field)
                        .is_some_and(non_empty_string_or_reference)
                })
                .count();
            if modes != 1 {
                return Err(ToolError::invalid_request(
                    "act_on expect requires exactly one non-empty selector or text",
                ));
            }
            if expect.keys().any(|field| {
                !matches!(field.as_str(), "selector" | "text" | "state" | "timeout_ms")
            }) {
                return Err(ToolError::invalid_request(
                    "act_on expect contains an unsupported field",
                ));
            }
            if expect.get("state").is_some_and(|state| {
                !is_deferred_reference(state)
                    && !matches!(state.as_str(), Some("visible" | "present" | "gone"))
            }) {
                return Err(ToolError::invalid_request(
                    "act_on expect.state must be visible, present, gone, or deferred",
                ));
            }
            if expect.get("timeout_ms").is_some_and(|timeout| {
                !is_deferred_reference(timeout)
                    && !timeout
                        .as_f64()
                        .is_some_and(|timeout| (0.0..=30_000.0).contains(&timeout))
            }) {
                return Err(ToolError::invalid_request(
                    "act_on expect.timeout_ms must be from 0 through 30000 or deferred",
                ));
            }
        }
    }
    simple(OperationId::BrowserAct, intent, with_tab(args))
}

fn decode_dialog(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("dialog", &mut args)?;
    let intent = match action.as_str() {
        "status" => IntentId::DialogStatus,
        "accept" => IntentId::DialogAccept,
        "dismiss" => IntentId::DialogDismiss,
        "respond" => IntentId::DialogRespond,
        other => return Err(unknown_action("dialog", other)),
    };
    simple(OperationId::BrowserDialog, intent, with_tab(args))
}

fn decode_tab_control(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("tab_control", &mut args)?;
    let (id, intent) = match action.as_str() {
        "focus" => (OperationId::BrowserTabs, IntentId::TabsFocus),
        "reload" => (OperationId::BrowserNavigate, IntentId::NavigateReload),
        "close" => (OperationId::BrowserTabs, IntentId::TabsClose),
        other => return Err(unknown_action("tab_control", other)),
    };
    simple(id, intent, with_tab(args))
}

fn decode_file_upload(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let reference = args
        .remove("ref")
        .ok_or_else(|| ToolError::invalid_request("file_upload requires ref"))?;
    let mut canonical = with_tab(args);
    canonical.insert("target".into(), json!({"ref": reference}));
    if let Some(Value::Array(files)) = canonical.get_mut("files") {
        for file in files {
            if let Some(object) = file.as_object_mut() {
                if let Some(mime) = object.remove("mimeType") {
                    object.insert("mime_type".into(), mime);
                }
            }
        }
    }
    simple(
        OperationId::BrowserUpload,
        IntentId::UploadClientFiles,
        canonical,
    )
}

fn decode_upload_image(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let artifact = args
        .remove("imageId")
        .ok_or_else(|| ToolError::invalid_request("upload_image requires imageId"))?;
    let reference = args.remove("ref");
    let point = args.remove("coordinate");
    if usize::from(reference.is_some()) + usize::from(point.is_some()) != 1 {
        return Err(ToolError::invalid_request(
            "upload_image requires exactly one ref or coordinate",
        ));
    }
    if point
        .as_ref()
        .is_some_and(|point| !valid_point_or_reference(point))
    {
        return Err(ToolError::invalid_request(
            "upload_image coordinate must contain exactly two numbers or deferred references",
        ));
    }
    let mut canonical = with_tab(args);
    canonical.insert("artifact".into(), artifact);
    if let Some(reference) = reference {
        canonical.insert("target".into(), json!({"ref": reference}));
    }
    if let Some(point) = point {
        canonical.insert("point".into(), point);
    }
    simple(
        OperationId::BrowserUpload,
        IntentId::UploadCapturedArtifact,
        canonical,
    )
}

fn decode_gif(mut args: Map<String, Value>) -> Result<BrowserOperation, ToolError> {
    let action = take_action("gif_creator", &mut args)?;
    let intent = match action.as_str() {
        "start_recording" => IntentId::RecordStart,
        "stop_recording" => IntentId::RecordStop,
        "status" => IntentId::RecordStatus,
        "clear" => IntentId::RecordClear,
        "export" => IntentId::RecordExport,
        other => return Err(unknown_action("gif_creator", other)),
    };
    let mut canonical = take_tab(&mut args);
    if intent == IntentId::RecordExport {
        let reference = args.remove("ref");
        let point = args.remove("coordinate");
        let download = args.remove("download");
        let download_target = download
            .as_ref()
            .is_some_and(|value| value.as_bool() == Some(true) || is_deferred_reference(value));
        let delivery_targets = usize::from(reference.is_some())
            + usize::from(point.is_some())
            + usize::from(download_target);
        if delivery_targets != 1 {
            return Err(ToolError::invalid_request(
                "gif_creator export requires exactly one ref, coordinate, or download:true",
            ));
        }
        if point
            .as_ref()
            .is_some_and(|point| !valid_point_or_reference(point))
        {
            return Err(ToolError::invalid_request(
                "gif_creator coordinate must contain exactly two numbers or deferred references",
            ));
        }
        if let Some(reference) = reference {
            canonical.insert("target".into(), json!({"ref": reference}));
        }
        if let Some(point) = point {
            canonical.insert("point".into(), point);
        }
        if download_target {
            canonical.insert("download".into(), download.expect("download target exists"));
        }
        move_field(&mut args, &mut canonical, "filename", "filename");
        move_field(&mut args, &mut canonical, "options", "options");
    }
    simple(OperationId::BrowserRecord, intent, canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostlight_transport::operation::OperationKey;

    #[test]
    fn discriminants_are_removed_and_coordinate_wins_over_ref() {
        let operation = decode_call(
            "computer",
            &json!({"action":"left_click","tabId":7,"coordinate":[10,20],"ref":"ignored"}),
        )
        .expect("decode coordinate click");
        assert_eq!(
            operation.key(),
            OperationKey::new(OperationId::BrowserInput, IntentId::InputPointerClick)
        );
        assert_eq!(operation.arguments, json!({"tab":7,"point":[10,20]}));
        assert!(operation.arguments.get("action").is_none());
    }

    #[test]
    fn flows_contain_canonical_operations_only() {
        let operation = decode_call(
            "script",
            &json!({"steps":[{"tool":"find","args":{"tabId":7,"query":"Save"}}]}),
        )
        .expect("decode flow");
        let rendered = operation.arguments.to_string();
        assert!(rendered.contains("browser.find"));
        assert!(!rendered.contains("\"tool\""));
    }

    #[test]
    fn semantic_sanitization_matches_the_legacy_edge_contract() {
        let cases = [
            (
                json!({
                    "action":"screenshot", "tabId":7, "coordinate":[1,2], "duration":2,
                    "modifiers":"ctrl", "ref":"ref_1", "region":[0,0,10,10], "repeat":4,
                    "scroll_direction":"up", "scroll_amount":2, "start_coordinate":[3,4],
                    "text":"ignored"
                }),
                OperationKey::new(OperationId::BrowserScreenshot, IntentId::ScreenshotViewport),
                json!({"tab":7}),
            ),
            (
                json!({
                    "action":"type", "tabId":7, "text":"hello", "coordinate":[1,2],
                    "ref":"ref_1", "repeat":4, "modifiers":"ctrl"
                }),
                OperationKey::new(OperationId::BrowserInput, IntentId::InputTypeText),
                json!({"tab":7,"text":"hello"}),
            ),
            (
                json!({
                    "action":"key", "tabId":7, "text":"Enter", "repeat":4,
                    "coordinate":[1,2], "ref":"ref_1", "modifiers":"ctrl"
                }),
                OperationKey::new(OperationId::BrowserInput, IntentId::InputPressKey),
                json!({"tab":7,"key":"Enter","repeat":4}),
            ),
            (
                json!({
                    "action":"scroll", "tabId":7, "coordinate":[1,2], "ref":"ignored",
                    "scroll_direction":"left", "scroll_amount":2, "modifiers":"shift"
                }),
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                json!({"tab":7,"point":[1,2],"direction":"left","amount":2,"modifiers":"shift"}),
            ),
            (
                json!({"action":"scroll","tabId":7,"ref":"ref_1"}),
                OperationKey::new(OperationId::BrowserInput, IntentId::InputWheel),
                json!({"tab":7,"target":{"ref":"ref_1"},"direction":"down","amount":3}),
            ),
            (
                json!({"action":"scroll_to","tabId":7,"ref":"ref_1","coordinate":[1,2]}),
                OperationKey::new(OperationId::BrowserAct, IntentId::ActScrollIntoView),
                json!({"tab":7,"target":{"ref":"ref_1"}}),
            ),
        ];
        for (arguments, key, expected) in cases {
            let operation = decode_call("computer", &arguments).expect("valid computer action");
            assert_eq!(operation.key(), key);
            assert_eq!(operation.arguments, expected);
        }

        for duration in [None, Some(0)] {
            let mut arguments = json!({"action":"wait","tabId":7});
            if let Some(duration) = duration {
                arguments["duration"] = json!(duration);
            }
            let operation = decode_call("computer", &arguments).expect("wait default");
            assert_eq!(operation.arguments, json!({"tab":7,"seconds":1}));
        }

        for action in ["start_recording", "stop_recording", "status", "clear"] {
            let operation = decode_call(
                "gif_creator",
                &json!({
                    "action":action, "tabId":7, "coordinate":[1,2], "ref":"ref_1",
                    "download":true, "filename":"ignored.gif", "options":{"speed":2}
                }),
            )
            .expect("non-export action");
            assert_eq!(operation.arguments, json!({"tab":7}));
        }
        let exported = decode_call(
            "gif_creator",
            &json!({
                "action":"export", "tabId":7, "ref":"ref_1",
                "filename":"capture.gif", "options":{"speed":2}
            }),
        )
        .expect("ref export");
        assert_eq!(
            exported.arguments,
            json!({
                "tab":7, "target":{"ref":"ref_1"}, "filename":"capture.gif",
                "options":{"speed":2}
            })
        );
    }

    #[test]
    fn core_bypass_rejects_the_same_or_stricter_semantic_ambiguities() {
        for (tool, arguments) in [
            ("computer", json!({"action":"left_click","tabId":7})),
            ("computer", json!({"action":"zoom","tabId":7})),
            (
                "computer",
                json!({"action":"left_click_drag","tabId":7,"coordinate":[1,2]}),
            ),
            ("form_input", json!({"tabId":7,"ref":"","value":"x"})),
            ("form_fill", json!({"tabId":7,"fields":{"":"x"}})),
            (
                "act_on",
                json!({"tabId":7,"action":"left_click","target":{"ref":"ref_1"},"value":"x"}),
            ),
            (
                "wait_for",
                json!({"tabId":7,"selector":"#save","text":"Saved"}),
            ),
            (
                "upload_image",
                json!({"tabId":7,"imageId":"img_1","ref":"ref_1","coordinate":[1,2]}),
            ),
            (
                "gif_creator",
                json!({"action":"export","tabId":7,"download":false}),
            ),
        ] {
            assert!(
                decode_call(tool, &arguments).is_err(),
                "{tool} semantic ambiguity must fail"
            );
        }
    }

    #[test]
    fn core_flow_preserves_strict_references_and_validates_concrete_siblings() {
        let flow = decode_call(
            "script",
            &json!({
                "steps":[
                    {"tool":"get_page_text","args":{"tabId":"$prev.tabId","max_chars":"$1.limit"}},
                    {"tool":"form_fill","args":{"tabId":7,"fields":"$prev.fields"}}
                ]
            }),
        )
        .expect("typed references remain deferred");
        assert_eq!(
            flow.arguments.pointer("/steps/0/arguments/tab"),
            Some(&json!("$prev.tabId"))
        );
        assert_eq!(
            flow.arguments.pointer("/steps/1/arguments/fields"),
            Some(&json!("$prev.fields"))
        );

        assert!(decode_call(
            "script",
            &json!({"steps":[{"tool":"get_page_text","args":{"tabId":"$prev.tabId","max_chars":true}}]})
        )
        .is_err());
        assert!(decode_call(
            "script",
            &json!({"steps":[{"tool":"get_page_text","args":{"tabId":"$0.tabId"}}]})
        )
        .is_err());
        // ADR-0101 deliberately tightens composition: unknown inner tools fail closed.
        assert!(decode_call(
            "script",
            &json!({"steps":[{"tool":"future_tool","args":{}}]})
        )
        .is_err());
    }
}
