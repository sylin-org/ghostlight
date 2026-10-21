//! Decoding and validation: one catalog call's JSON becomes a typed `Operation`, or the refusal
//! that says what the call got wrong, before any work starts.

use super::*;

const RETIRED_RESTRICTIONS: &[&str] = &["restrict_hosts", "restrict_capabilities"];
/// The refusal of region work that names no view.
const NEEDS_VIEW: &str =
    "region work needs a current view handle: take a screenshot, then pass its view with these coordinates";
/// The refusal of a window resize that leaves out a dimension.
const RESIZE_NEEDS_SIZE: &str =
    "action resize needs width and height: the outer window size in pixels";

/// Tools that left the catalog, each with the refusal that tells a caller holding an old
/// catalog what to do instead. A retired name never decodes and never dispatches.
const RETIRED_TOOLS: &[(&str, &str)] = &[
    (
        "browser_sequence",
        "browser_sequence was removed; use browser_flow with ordinary tool steps. Refresh the tool catalog before retrying.",
    ),
    (
        "browser_workspace",
        "browser_workspace was removed; each session works only in its own tabs. Call browser_tabs with action list to see them. Refresh the tool catalog before retrying.",
    ),
];

/// Decode and validate one catalog invocation.
pub fn decode(name: &str, input: Value) -> Result<Operation, LanguageError> {
    if let Some((_, refusal)) = RETIRED_TOOLS.iter().find(|(retired, _)| *retired == name) {
        return Err(LanguageError::Invalid((*refusal).into()));
    }
    match name {
        "browser_tabs" => decode_tabs(input),
        "browser_navigate" => decode_navigate(input),
        "browser_history" => decode_history(input),
        "browser_window" => decode_window(input),
        "browser_read" => Ok(Operation::ReadPage(parse(
            input,
            &[
                "visual_settle",
                "tab",
                "target",
                "mode",
                "max_chars",
                "timeout_ms",
            ],
            |value: &ReadPage| {
                validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
                validate_timeout(value.timeout_ms)?;
                validate_optional_handle(value.target.as_deref(), HandleKind::Target)?;
                if value.mode.is_some() && value.target.is_some() {
                    return Err(LanguageError::Invalid(
                        "mode cannot be combined with target".into(),
                    ));
                }
                validate_within(value.max_chars, READ_CHARS, "max_chars")?;
                Ok(())
            },
        )?)),
        "browser_inspect" => Ok(Operation::InspectPage(parse(
            input,
            &[
                "visual_settle",
                "tab",
                "scope",
                "root",
                "max_depth",
                "max_items",
                "timeout_ms",
            ],
            |value: &InspectPage| {
                validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
                validate_timeout(value.timeout_ms)?;
                if value.scope == InspectScope::Document {
                    validate_optional_handle(value.root.as_deref(), HandleKind::Target)?;
                    if let Some(depth) = value.max_depth {
                        validate_within(depth, DOCUMENT_DEPTH, "max_depth")?;
                    }
                } else {
                    if value.root.is_some() || value.max_depth.is_some() {
                        return Err(LanguageError::Invalid(
                            "root and max_depth apply only to document scope".into(),
                        ));
                    }
                }
                validate_within(value.max_items, INSPECT_ITEMS, "max_items")?;
                Ok(())
            },
        )?)),
        "browser_find" => Ok(Operation::Find(parse(
            input,
            &[
                "visual_settle",
                "text",
                "tab",
                "scope",
                "max_results",
                "timeout_ms",
            ],
            |value: &Find| {
                validate_text(&value.text, SEARCH_TEXT_CHARS, "text")?;
                validate_timeout(value.timeout_ms)?;
                validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
                validate_within(value.max_results, FIND_RESULTS, "max_results")?;
                Ok(())
            },
        )?)),
        "browser_screenshot" => Ok(Operation::TakeScreenshot(parse(
            input,
            &[
                "tab",
                "target",
                "full_page",
                "view",
                "x",
                "y",
                "width",
                "height",
                "visual_settle",
                "timeout_ms",
            ],
            validate_screenshot,
        )?)),
        "browser_click" => Ok(Operation::Click(parse(
            input,
            &[
                "visual_settle",
                "target",
                "selector",
                "view",
                "x",
                "y",
                "tab",
                "button",
                "click_count",
                "modifiers",
                "expect",
                "timeout_ms",
            ],
            validate_click,
        )?)),
        "browser_scroll" => {
            let mut scroll: ScrollPage = parse(
                input,
                &[
                    "visual_settle",
                    "tab",
                    "target",
                    "direction",
                    "amount",
                    "view",
                    "x",
                    "y",
                    "ticks",
                    "timeout_ms",
                ],
                validate_scroll,
            )?;
            if scroll.wheel() {
                scroll.direction.get_or_insert(SCROLL_DIRECTION_DEFAULT);
            }
            Ok(Operation::ScrollPage(scroll))
        }
        "browser_hover" => Ok(Operation::Hover(parse(
            input,
            &[
                "visual_settle",
                "target",
                "view",
                "x",
                "y",
                "tab",
                "timeout_ms",
            ],
            validate_hover,
        )?)),
        "browser_fill_form" => Ok(Operation::FillForm(parse(
            input,
            &[
                "visual_settle",
                "fields",
                "tab",
                "submit_target",
                "user_authorized_credentials",
                "expect",
                "timeout_ms",
            ],
            validate_fill,
        )?)),
        "browser_type_text" => Ok(Operation::TypeText(parse(
            input,
            &[
                "visual_settle",
                "target",
                "focused",
                "selector",
                "text",
                "tab",
                "clear_first",
                "user_authorized_credentials",
                "expect",
                "timeout_ms",
            ],
            validate_type_text,
        )?)),
        "browser_press_key" => Ok(Operation::PressKey(parse(
            input,
            &[
                "visual_settle",
                "key",
                "strokes",
                "repeat",
                "tab",
                "target",
                "modifiers",
                "expect",
                "timeout_ms",
            ],
            validate_press_key,
        )?)),
        "browser_drag" => Ok(Operation::Drag(parse(
            input,
            &[
                "visual_settle",
                "source_target",
                "destination_target",
                "view",
                "start_x",
                "start_y",
                "end_x",
                "end_y",
                "tab",
                "timeout_ms",
            ],
            validate_drag,
        )?)),
        "browser_upload" => Ok(Operation::UploadFiles(parse(
            input,
            &[
                "visual_settle",
                "target",
                "selector",
                "paths",
                "files",
                "source_image",
                "view",
                "x",
                "y",
                "tab",
                "timeout_ms",
            ],
            validate_upload,
        )?)),
        "browser_execute" => Ok(Operation::RunScript(parse(
            input,
            &[
                "visual_settle",
                "script",
                "tab",
                "max_result_chars",
                "timeout_ms",
            ],
            |value: &RunScript| {
                validate_text(&value.script, SCRIPT_MAX_CHARS, "script")?;
                validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
                validate_within(
                    value.max_result_chars,
                    SCRIPT_RESULT_CHARS,
                    "max_result_chars",
                )?;
                validate_timeout(value.timeout_ms)?;
                Ok(())
            },
        )?)),
        "browser_wait" => Ok(Operation::Wait(parse(
            input,
            &[
                "condition",
                "tab",
                "value",
                "target",
                "selector",
                "visual_settle",
                "timeout_ms",
            ],
            validate_wait,
        )?)),
        "browser_flow" => decode_flow(input),
        "browser_dialog" => decode_dialog(input),
        "browser_record" => decode_record(input),
        "browser_diagnose" => decode_diagnose(input),
        "policy_explain" => decode_explain_policy(input),
        other => Err(LanguageError::UnknownTool(other.into())),
    }
}

fn decode_tabs(input: Value) -> Result<Operation, LanguageError> {
    let value: TabsRequest = parse(
        input,
        &["action", "tab", "timeout_ms"],
        |value: &TabsRequest| {
            validate_choice(&value.action, &["list", "focus", "close"], "action")?;
            validate_timeout(value.timeout_ms)?;
            match value.action.as_str() {
                "list" if value.tab.is_some() => {
                    return Err(LanguageError::Invalid(
                        "tab is not valid when action is list".into(),
                    ))
                }
                "focus" | "close" => validate_handle(
                    value.tab.as_deref().ok_or_else(|| {
                        LanguageError::Invalid(
                        "action focus and action close need a tab: the handle of the tab to act on"
                            .into(),
                    )
                    })?,
                    HandleKind::Tab,
                )?,
                _ => {}
            }
            Ok(())
        },
    )?;
    Ok(match value.action.as_str() {
        "list" => Operation::ListTabs(ListTabs {
            timeout_ms: value.timeout_ms,
        }),
        "focus" => Operation::ActivateTab(ActivateTab {
            tab: value.tab.expect("validated tab"),
            timeout_ms: value.timeout_ms,
        }),
        "close" => Operation::CloseTab(CloseTab {
            tab: value.tab.expect("validated tab"),
            timeout_ms: value.timeout_ms,
        }),
        _ => unreachable!("validated action"),
    })
}

fn decode_navigate(input: Value) -> Result<Operation, LanguageError> {
    let value: NavigateRequest = parse(
        input,
        &[
            "url",
            "tab",
            "browser",
            "new_tab",
            "reuse",
            "beforeunload",
            "timeout_ms",
        ],
        |value: &NavigateRequest| {
            validate_url(&value.url)?;
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            validate_optional_handle(value.browser.as_deref(), HandleKind::Browser)?;
            if let Some(reuse) = value.reuse.as_deref() {
                validate_choice(reuse, &["domain", "never"], "reuse")?;
            }
            if value.new_tab && value.tab.is_some() {
                return Err(LanguageError::Invalid(
                    "tab and new_tab cannot be combined".into(),
                ));
            }
            if value.beforeunload.is_some() {
                if value.new_tab || value.browser.is_some() {
                    return Err(LanguageError::Invalid(
                        "beforeunload applies only to same-tab navigation".into(),
                    ));
                }
                validate_choice(
                    value.beforeunload.as_deref().expect("validated presence"),
                    &["discard"],
                    "beforeunload",
                )?;
            }
            if value.browser.is_some() && !value.new_tab {
                return Err(LanguageError::Invalid(
                    "browser is only valid when new_tab is true".into(),
                ));
            }
            validate_timeout(value.timeout_ms)?;
            Ok(())
        },
    )?;
    Ok(if value.new_tab {
        // A fresh tab is the entire point of new_tab, so reuse never applies there. A client
        // that fills in the advertised default sends reuse "domain" beside it, which is
        // therefore ignored rather than refused.
        Operation::OpenPage(OpenPage {
            url: value.url,
            browser: value.browser,
            timeout_ms: value.timeout_ms,
            reuse: ReusePolicy::Never,
        })
    } else {
        Operation::NavigatePage(NavigatePage {
            beforeunload_discard: value.beforeunload.as_deref() == Some("discard"),
            url: value.url,
            tab: value.tab,
            reuse: match value.reuse.as_deref() {
                Some("never") => ReusePolicy::Never,
                _ => ReusePolicy::Domain,
            },
            timeout_ms: value.timeout_ms,
        })
    })
}

/// The history action that reloads the page instead of moving through its history.
pub(super) const HISTORY_RELOAD: &str = "reload";

fn decode_history(input: Value) -> Result<Operation, LanguageError> {
    let value: HistoryRequest = parse(
        input,
        &["action", "tab", "bypass_cache", "timeout_ms"],
        |value: &HistoryRequest| {
            validate_choice(
                &value.action,
                &[
                    HistoryDirection::Back.as_str(),
                    HistoryDirection::Forward.as_str(),
                    HISTORY_RELOAD,
                ],
                "action",
            )?;
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            // false is the advertised default, which a client may send with any action.
            if value.action != HISTORY_RELOAD && value.bypass_cache {
                return Err(LanguageError::Invalid(
                    "bypass_cache is only valid when action is reload".into(),
                ));
            }
            validate_timeout(value.timeout_ms)?;
            Ok(())
        },
    )?;
    Ok(if value.action == HISTORY_RELOAD {
        Operation::ReloadPage(ReloadPage {
            tab: value.tab,
            bypass_cache: value.bypass_cache,
            timeout_ms: value.timeout_ms,
        })
    } else {
        Operation::NavigateHistory(NavigateHistory {
            direction: HistoryDirection::named(&value.action).map_err(LanguageError::Invalid)?,
            tab: value.tab,
            timeout_ms: value.timeout_ms,
        })
    })
}

fn decode_window(input: Value) -> Result<Operation, LanguageError> {
    let value: WindowRequest = parse(
        input,
        &["action", "tab", "percent", "width", "height", "timeout_ms"],
        |value: &WindowRequest| {
            validate_choice(&value.action, &["zoom", "resize"], "action")?;
            validate_timeout(value.timeout_ms)?;
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            match value.action.as_str() {
                "zoom" => {
                    let percent = value.percent.ok_or_else(|| {
                        LanguageError::Invalid(format!(
                            "action zoom needs percent: the zoom level as a whole number, {} to {}",
                            ZOOM_PERCENT.min, ZOOM_PERCENT.max
                        ))
                    })?;
                    validate_within(usize::from(percent), ZOOM_PERCENT, "percent")?;
                    if value.width.is_some() || value.height.is_some() {
                        return Err(LanguageError::Invalid(
                            "width and height are only valid when action is resize".into(),
                        ));
                    }
                }
                "resize" => {
                    let width = value
                        .width
                        .ok_or_else(|| LanguageError::Invalid(RESIZE_NEEDS_SIZE.into()))?;
                    let height = value
                        .height
                        .ok_or_else(|| LanguageError::Invalid(RESIZE_NEEDS_SIZE.into()))?;
                    validate_within(width as usize, WINDOW_WIDTH, "width")?;
                    validate_within(height as usize, WINDOW_HEIGHT, "height")?;
                    if value.percent.is_some() {
                        return Err(LanguageError::Invalid(
                            "percent is only valid when action is zoom".into(),
                        ));
                    }
                }
                _ => unreachable!("validated action"),
            }
            Ok(())
        },
    )?;
    Ok(if value.action == "zoom" {
        Operation::SetZoom(SetZoom {
            percent: value.percent.expect("validated percent"),
            tab: value.tab,
            timeout_ms: value.timeout_ms,
        })
    } else {
        Operation::ResizeWindow(ResizeWindow {
            width: value.width.expect("validated width"),
            height: value.height.expect("validated height"),
            tab: value.tab,
            timeout_ms: value.timeout_ms,
        })
    })
}

fn decode_dialog(input: Value) -> Result<Operation, LanguageError> {
    let text_present = has_field(&input, "text");
    let value: HandleDialog = parse(
        input,
        &["action", "tab", "text", "timeout_ms"],
        |value: &HandleDialog| {
            validate_timeout(value.timeout_ms)?;
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            match value.action {
                DialogAction::Respond => validate_text_allow_empty(
                    value.text.as_deref().ok_or_else(|| {
                        LanguageError::Invalid(
                            "action respond needs text: the prompt response, which may be empty"
                                .into(),
                        )
                    })?,
                    DIALOG_TEXT_CHARS,
                    "text",
                )?,
                _ if text_present => {
                    return Err(LanguageError::Invalid(
                        "text is only valid when action is respond".into(),
                    ))
                }
                _ => {}
            }
            Ok(())
        },
    )?;
    Ok(Operation::HandleDialog(value))
}

fn decode_record(input: Value) -> Result<Operation, LanguageError> {
    let value: Record = parse(
        input,
        &["action", "tab", "target", "download", "timeout_ms"],
        |value: &Record| {
            validate_timeout(value.timeout_ms)?;
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            validate_optional_handle(value.target.as_deref(), HandleKind::Target)?;
            match value.action {
                RecordAction::Start if value.target.is_some() => {
                    return Err(LanguageError::Invalid(
                        "start accepts tab but not target".into(),
                    ))
                }
                RecordAction::Start => {}
                RecordAction::Save if value.tab.is_some() => {
                    return Err(LanguageError::Invalid(
                        "tab is only valid when action is start".into(),
                    ))
                }
                // One replay goes to one place. Asking for two is a mistake worth naming rather
                // than a preference to resolve silently.
                RecordAction::Save if value.target.is_some() && value.download => {
                    return Err(LanguageError::Invalid(
                        "save accepts target or download, not both".into(),
                    ))
                }
                RecordAction::Save => {}
                RecordAction::Status | RecordAction::Stop | RecordAction::Discard
                    if value.tab.is_some() || value.target.is_some() || value.download =>
                {
                    return Err(LanguageError::Invalid(
                        "tab is only valid for start; target and download are only valid for save"
                            .into(),
                    ))
                }
                RecordAction::Status | RecordAction::Stop | RecordAction::Discard => {}
            }
            Ok(())
        },
    )?;
    Ok(Operation::Record(value))
}

fn decode_diagnose(input: Value) -> Result<Operation, LanguageError> {
    let value: Diagnose = parse(
        input,
        &[
            "tab",
            "source",
            "detail",
            "match",
            "after",
            "limit",
            "timeout_ms",
        ],
        |value: &Diagnose| {
            validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
            validate_timeout(value.timeout_ms)?;
            if let Some(pattern) = &value.r#match {
                validate_text(pattern, SEARCH_TEXT_CHARS, "match")?;
            }
            validate_optional_handle(value.after.as_deref(), HandleKind::Diagnostics)?;
            validate_within(value.limit, DIAGNOSTIC_LIMIT, "limit")?;
            Ok(())
        },
    )?;
    Ok(Operation::Diagnose(value))
}

fn decode_explain_policy(input: Value) -> Result<Operation, LanguageError> {
    let value: ExplainPolicy = parse(input, &["timeout_ms"], |value: &ExplainPolicy| {
        validate_timeout(value.timeout_ms)
    })?;
    Ok(Operation::ExplainPolicy(value))
}

fn has_field(input: &Value, field: &str) -> bool {
    input
        .as_object()
        .is_some_and(|object| object.contains_key(field))
}
fn parse<T: DeserializeOwned>(
    input: Value,
    fields: &[&str],
    validate: impl FnOnce(&T) -> Result<(), LanguageError>,
) -> Result<T, LanguageError> {
    ensure_fields(&input, fields)?;
    let value: T =
        serde_json::from_value(input).map_err(|error| LanguageError::Invalid(error.to_string()))?;
    validate(&value)?;
    Ok(value)
}

fn ensure_fields(input: &Value, fields: &[&str]) -> Result<(), LanguageError> {
    let object = input
        .as_object()
        .ok_or_else(|| LanguageError::Invalid("input must be an object".into()))?;
    reject_retired_arguments(input)?;
    for key in object.keys() {
        if !fields.contains(&key.as_str()) {
            return Err(LanguageError::Invalid(format!(
                "unknown field `{key}`: check this tool's advertised fields"
            )));
        }
    }
    Ok(())
}

fn validate_selector(selector: &SemanticSelector) -> Result<(), LanguageError> {
    validate_text(&selector.name, SEARCH_TEXT_CHARS, "name")?;
    if let Some(role) = &selector.role {
        validate_choice(role, SEMANTIC_ROLES, "role")?;
    }
    Ok(())
}

/// Composite tools a flow step may never name.
const FLOW_FORBIDDEN_TOOLS: &[&str] = &["browser_flow", "browser_sequence"];
const FLOW_REF_POINTER_LIMIT: usize = 512;
const FLOW_REF_DEPTH_LIMIT: usize = 32;

fn reject_retired_arguments(input: &Value) -> Result<(), LanguageError> {
    for field in RETIRED_RESTRICTIONS {
        if has_field(input, field) {
            return Err(LanguageError::Invalid(format!(
                "`{field}` was removed. Browser authority comes from configured policy and human controls. Refresh the tool catalog and review the intended work before retrying."
            )));
        }
    }
    Ok(())
}

fn decode_flow(mut input: Value) -> Result<Operation, LanguageError> {
    if has_field(&input, "dry_run") {
        return Err(LanguageError::Invalid("browser_flow.dry_run was removed; a flow executes its steps. Refresh the tool catalog before retrying.".into()));
    }
    if let Some(steps) = input.get_mut("steps").and_then(Value::as_array_mut) {
        for (index, step) in steps.iter_mut().enumerate() {
            if let Some(object) = step.as_object_mut() {
                object
                    .entry("id")
                    .or_insert_with(|| Value::String(format!("step_{}", index + 1)));
            }
        }
    }
    let value = parse(
        input,
        &["steps", "on_error", "tab", "timeout_ms"],
        validate_flow,
    )?;
    Ok(Operation::RunFlow(value))
}

/// The arguments whose target or view handle belongs to one tab, which the handle names.
const TAB_NAMING_HANDLES: &[&str] = &[
    "target",
    "root",
    "view",
    "submit_target",
    "source_target",
    "destination_target",
];

/// Whether every tab-scoped part of a step's arguments is a handle that names its own tab: a
/// target or view handle, or a fill field's target. A selector names no tab, so a step with one,
/// such as a fill whose other fields or submit_target are handles, still works in the flow's tab.
fn names_its_tab(input: &Value) -> bool {
    let named = |value: &Value, field: &str| {
        value
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|handle| !handle.is_empty())
    };
    let selects = |value: &Value| {
        value
            .get("selector")
            .is_some_and(|selector| !selector.is_null())
    };
    let fields = input
        .get("fields")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if selects(input) || fields.iter().any(selects) {
        return false;
    }
    TAB_NAMING_HANDLES.iter().any(|field| named(input, field))
        || fields.iter().any(|field| named(field, "target"))
}

/// Decode an ordinary flow child, inheriting the batch tab for tab-scoped work unless every
/// tab-scoped part of it is a target or view handle, which already says which tab it is in.
pub(crate) fn decode_flow_step(
    tool: &str,
    mut input: Value,
    tab: Option<&str>,
) -> Result<Operation, LanguageError> {
    let uses_tab = !names_its_tab(&input)
        && match tool {
            "policy_explain" => false,
            "browser_tabs" => input
                .get("action")
                .and_then(Value::as_str)
                .is_some_and(|action| matches!(action, "focus" | "close")),
            "browser_record" => input.get("action").and_then(Value::as_str) == Some("start"),
            "browser_navigate" => input.get("new_tab") != Some(&Value::Bool(true)),
            _ => true,
        };
    if uses_tab {
        if let (Some(tab), Some(object)) = (tab, input.as_object_mut()) {
            object
                .entry("tab")
                .or_insert_with(|| Value::String(tab.into()));
        }
    }
    decode(tool, input)
}

fn validate_flow(value: &RunFlow) -> Result<(), LanguageError> {
    validate_range(
        value.steps.len(),
        1,
        composition::COMPOSITION_STEP_LIMIT,
        "steps",
    )?;
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_timeout(value.timeout_ms)?;
    let mut seen: Vec<&str> = Vec::with_capacity(value.steps.len());
    for (index, step) in value.steps.iter().enumerate() {
        validate_text(&step.id, FLOW_STEP_ID_CHARS, "id")?;
        if seen.contains(&step.id.as_str()) {
            return Err(LanguageError::Invalid(format!(
                "step id `{}` is not unique",
                step.id
            )));
        }
        seen.push(&step.id);
        if FLOW_FORBIDDEN_TOOLS.contains(&step.tool.as_str()) {
            return Err(LanguageError::Invalid(
                "a flow step may not name a composite tool".into(),
            ));
        }
        if !crate::language::capability_map::DIRECTORY
            .iter()
            .any(|entry| entry.tool == step.tool)
        {
            return Err(LanguageError::Invalid(format!(
                "step tool `{}` is not an advertised Ghostlight tool",
                step.tool
            )));
        }
        if !step.arguments.is_object() {
            return Err(LanguageError::Invalid(
                "step arguments must be an object".into(),
            ));
        }
        reject_retired_arguments(&step.arguments)?;
        validate_flow_references(&step.arguments, &seen[..seen.len() - 1], index)?;
    }
    Ok(())
}

fn validate_flow_references(
    input: &Value,
    earlier: &[&str],
    index: usize,
) -> Result<(), LanguageError> {
    match input {
        Value::Object(object) => {
            if let Some(reference) = object.get("flow_ref") {
                if object.len() != 1 {
                    return Err(LanguageError::Invalid(
                        "a flow reference object must contain only `flow_ref`".into(),
                    ));
                }
                let parsed: ResultReference =
                    serde_json::from_value(reference.clone()).map_err(|_| {
                        LanguageError::Invalid(
                            "flow_ref requires `step` and `pointer` strings".into(),
                        )
                    })?;
                if !earlier.contains(&parsed.step.as_str()) {
                    return Err(LanguageError::Invalid(format!(
                        "step {index} references `{}`, which is missing or later",
                        parsed.step
                    )));
                }
                let pointer = parsed.pointer.as_bytes();
                if pointer.is_empty()
                    || pointer[0] != b'/'
                    || parsed.pointer.len() > FLOW_REF_POINTER_LIMIT
                    || parsed.pointer.matches('/').count() > FLOW_REF_DEPTH_LIMIT
                {
                    return Err(LanguageError::Invalid(
                        "flow_ref pointer must be a bounded JSON Pointer starting with `/`".into(),
                    ));
                }
                return Ok(());
            }
            for nested in object.values() {
                validate_flow_references(nested, earlier, index)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                validate_flow_references(item, earlier, index)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_expect(expect: &Option<Postcondition>) -> Result<(), LanguageError> {
    let Some(expectation) = expect else {
        return Ok(());
    };
    let condition = expectation.condition;
    match condition {
        PageCondition::LoadReady => {
            if expectation.value.is_some() {
                return Err(LanguageError::Invalid("load_ready accepts no value".into()));
            }
            Ok(())
        }
        PageCondition::UrlContains | PageCondition::TextPresent | PageCondition::TextAbsent => {
            let value = expectation.value.as_deref().ok_or_else(|| {
                LanguageError::Invalid(format!("{} requires value", condition.as_str()))
            })?;
            validate_text(value, MAX_POSTCONDITION_VALUE_CHARS, "value")
        }
        PageCondition::TargetPresent
        | PageCondition::TargetAbsent
        | PageCondition::VisualSettle => Err(LanguageError::Invalid(wait::EXPECT_REFUSAL.into())),
    }
}

fn validate_click(value: &Click) -> Result<(), LanguageError> {
    if let Some(selector) = &value.selector {
        if value.target.is_some() || value.view.is_some() || value.x.is_some() || value.y.is_some()
        {
            return Err(LanguageError::Invalid(
                "selector cannot be combined with target, view, or coordinates".into(),
            ));
        }
        validate_selector(selector)?;
    } else {
        validate_location(
            value.target.as_deref(),
            value.view.as_deref(),
            value.x,
            value.y,
        )?;
    }
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_within(usize::from(value.click_count), CLICK_COUNT, "click_count")?;
    validate_modifiers(&value.modifiers)?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_screenshot(value: &TakeScreenshot) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    let region_requested = value.view.is_some()
        || value.x.is_some()
        || value.y.is_some()
        || value.width.is_some()
        || value.height.is_some();
    if region_requested {
        if value.target.is_some() || value.full_page {
            return Err(LanguageError::Invalid(
                "view region cannot be combined with target or full_page".into(),
            ));
        }
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid(NEEDS_VIEW.into()))?,
            HandleKind::View,
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("region capture needs x and y: the top-left corner of the rectangle in view coordinates".into()))?,
                name,
            )?;
        }
        for (name, extent) in [("width", value.width), ("height", value.height)] {
            validate_extent(
                extent.ok_or_else(|| {
                    LanguageError::Invalid(
                        "region capture needs width and height: the rectangle size in CSS pixels"
                            .into(),
                    )
                })?,
                name,
            )?;
        }
    } else {
        validate_optional_handle(value.target.as_deref(), HandleKind::Target)?;
        if value.target.is_some() && value.full_page {
            return Err(LanguageError::Invalid(
                "target and full_page cannot be combined".into(),
            ));
        }
    }
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_hover(value: &Hover) -> Result<(), LanguageError> {
    validate_location(
        value.target.as_deref(),
        value.view.as_deref(),
        value.x,
        value.y,
    )?;
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_location(
    target: Option<&str>,
    view: Option<&str>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), LanguageError> {
    match (target, view, x, y) {
        (Some(target), None, None, None) => validate_handle(target, HandleKind::Target),
        (None, Some(view), Some(x), Some(y)) => {
            validate_handle(view, HandleKind::View)?;
            validate_coordinate(x, "x")?;
            validate_coordinate(y, "y")
        }
        _ => Err(LanguageError::Invalid(
            "provide exactly target, or view with x and y".into(),
        )),
    }
}

/// The direction a scroll takes when the call gives none: the catalog advertises it, the decoder
/// applies it to a wheel scroll and the executor to a page scroll.
pub(crate) const SCROLL_DIRECTION_DEFAULT: ScrollDirection = ScrollDirection::Down;

/// The amount a page scroll takes when the call gives none: the catalog advertises it and the
/// executor applies it.
pub(crate) const SCROLL_AMOUNT_DEFAULT: ScrollAmount = ScrollAmount::Medium;

/// The directions a wheel scroll at a view point takes.
pub(super) const WHEEL_DIRECTIONS: &[&str] =
    &[ScrollDirection::Up.as_str(), ScrollDirection::Down.as_str()];

impl ScrollPage {
    /// Whether the call scrolls the wheel at a view point rather than the page or a target.
    fn wheel(&self) -> bool {
        self.view.is_some() || self.x.is_some() || self.y.is_some() || self.ticks.is_some()
    }
}

fn validate_scroll(value: &ScrollPage) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    let wheel = value.wheel();
    if let Some(target) = &value.target {
        if wheel {
            return Err(LanguageError::Invalid(
                "target cannot be combined with a view point".into(),
            ));
        }
        validate_handle(target, HandleKind::Target)?;
        // A client that fills in the advertised defaults sends them beside a target; only another
        // direction or amount asks for the page scroll a target excludes.
        let page_scroll = value
            .direction
            .is_some_and(|direction| direction != SCROLL_DIRECTION_DEFAULT)
            || value
                .amount
                .is_some_and(|amount| amount != SCROLL_AMOUNT_DEFAULT);
        if page_scroll {
            return Err(LanguageError::Invalid(
                "target cannot be combined with direction or amount".into(),
            ));
        }
    } else if wheel {
        if let Some(direction) = value.direction {
            validate_choice(direction.as_str(), WHEEL_DIRECTIONS, "direction")?;
        }
        // Ticks set how far a wheel scrolls. The advertised amount default is what a
        // defaults-filling client sends beside them; any other amount asks for a page scroll.
        if let Some(amount) = value.amount {
            if amount != SCROLL_AMOUNT_DEFAULT {
                return Err(LanguageError::Invalid(
                    "amount cannot be combined with a view point: ticks set how far a wheel scrolls"
                        .into(),
                ));
            }
        }
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid(NEEDS_VIEW.into()))?,
            HandleKind::View,
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("coordinate wheel scrolling needs x and y: the point to scroll at, in view coordinates".into()))?,
                name,
            )?;
        }
        validate_within(
            usize::from(value.ticks.ok_or_else(|| {
                LanguageError::Invalid(format!(
                    "coordinate wheel scrolling needs ticks: how far to scroll, {} to {}",
                    WHEEL_TICKS.min, WHEEL_TICKS.max
                ))
            })?),
            WHEEL_TICKS,
            "ticks",
        )?;
    }
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_fill(value: &FillForm) -> Result<(), LanguageError> {
    validate_within(value.fields.len(), FILL_FIELDS, "fields")?;
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_optional_handle(value.submit_target.as_deref(), HandleKind::Target)?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    for field in &value.fields {
        let branches = usize::from(field.target.is_some()) + usize::from(field.selector.is_some());
        if branches != 1 {
            return Err(LanguageError::Invalid(
                "each field provides exactly one of target or selector".into(),
            ));
        }
        if let Some(target) = &field.target {
            validate_handle(target, HandleKind::Target)?;
        }
        if let Some(selector) = &field.selector {
            validate_selector(selector)?;
        }
        match &field.value {
            FormFieldValue::Text(text) => {
                validate_text_allow_empty(text, FIELD_VALUE_CHARS, "field value")?
            }
            FormFieldValue::Choices(choices) => {
                validate_range(choices.len(), 0, FIELD_CHOICES, "field values")?;
                for choice in choices {
                    validate_text_allow_empty(choice, FIELD_VALUE_CHARS, "field value")?;
                }
            }
            FormFieldValue::Flag(_) | FormFieldValue::Number(_) => {}
        }
    }
    Ok(())
}

fn validate_type_text(value: &TypeText) -> Result<(), LanguageError> {
    let branches = usize::from(!value.target.is_empty())
        + usize::from(value.focused)
        + usize::from(value.selector.is_some());
    if branches != 1 {
        return Err(LanguageError::Invalid(
            "provide exactly one of target, focused, or selector".into(),
        ));
    }
    if !value.target.is_empty() {
        validate_handle(&value.target, HandleKind::Target)?;
    }
    if let Some(selector) = &value.selector {
        validate_selector(selector)?;
    }
    if value.text.is_empty() && !value.clear_first {
        return Err(LanguageError::Invalid(
            "text cannot be empty unless clear_first is true".into(),
        ));
    }
    validate_text_allow_empty(&value.text, FIELD_VALUE_CHARS, "text")?;
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_drag(value: &Drag) -> Result<(), LanguageError> {
    let targets = value.source_target.is_some() || value.destination_target.is_some();
    let points = value.view.is_some()
        || value.start_x.is_some()
        || value.start_y.is_some()
        || value.end_x.is_some()
        || value.end_y.is_some();
    if targets == points {
        return Err(LanguageError::Invalid(
            "provide exactly target endpoints or view coordinates".into(),
        ));
    }
    if targets {
        validate_handle(
            value
                .source_target
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("drag by targets needs source_target and destination_target: current handles from browser_find or browser_inspect".into()))?,
            HandleKind::Target,
        )?;
        validate_handle(
            value
                .destination_target
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid("drag by targets needs destination_target as well as source_target: where to drop".into()))?,
            HandleKind::Target,
        )?;
    } else {
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid(NEEDS_VIEW.into()))?,
            HandleKind::View,
        )?;
        for (name, coordinate) in [
            ("start_x", value.start_x),
            ("start_y", value.start_y),
            ("end_x", value.end_x),
            ("end_y", value.end_y),
        ] {
            validate_coordinate(
                coordinate.ok_or_else(|| LanguageError::Invalid("drag by coordinates needs all four of start_x, start_y, end_x, and end_y: the two view points to drag between".into()))?,
                name,
            )?;
        }
    }
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

const UPLOAD_AGGREGATE_BYTES: u64 = 5_000_000;

/// How many bytes standard base64 decodes to, counted exactly: the digits without their `=`
/// padding give three bytes per four, and a final group of two or three digits one or two more.
#[must_use]
pub fn base64_decoded_len(data: &str) -> u64 {
    let digits = data.trim_end_matches('=').len();
    let partial = match digits % 4 {
        2 => 1,
        3 => 2,
        _ => 0,
    };
    (digits / 4 * 3 + partial) as u64
}

fn validate_upload(value: &UploadFiles) -> Result<(), LanguageError> {
    let sources = usize::from(!value.paths.is_empty())
        + usize::from(!value.files.is_empty())
        + usize::from(value.source_image.is_some());
    if sources != 1 {
        return Err(LanguageError::Invalid(
            "provide exactly one of paths, files, or source_image".into(),
        ));
    }
    let dropping = value.view.is_some() || value.x.is_some() || value.y.is_some();
    if !value.paths.is_empty() || !value.files.is_empty() {
        if dropping {
            return Err(LanguageError::Invalid(
                "only a source_image may be dropped at a view point".into(),
            ));
        }
        let branches = usize::from(value.target.is_some()) + usize::from(value.selector.is_some());
        if branches != 1 {
            return Err(LanguageError::Invalid(
                "provide exactly one of target or selector".into(),
            ));
        }
    }
    if let Some(selector) = &value.selector {
        validate_selector(selector)?;
    }
    if dropping {
        if value.source_image.is_none() {
            return Err(LanguageError::Invalid(
                "a drop requires source_image".into(),
            ));
        }
        if value.target.is_some() || value.selector.is_some() {
            return Err(LanguageError::Invalid(
                "a drop provides a view point, not target or selector".into(),
            ));
        }
        validate_handle(
            value
                .view
                .as_deref()
                .ok_or_else(|| LanguageError::Invalid(NEEDS_VIEW.into()))?,
            HandleKind::View,
        )?;
        for (name, coordinate) in [("x", value.x), ("y", value.y)] {
            validate_coordinate(
                coordinate.ok_or_else(|| {
                    LanguageError::Invalid(
                        "dropping an image needs x and y: the view point to drop at".into(),
                    )
                })?,
                name,
            )?;
        }
    } else if let Some(image) = &value.source_image {
        validate_handle(image, HandleKind::Image)?;
        if value.target.is_none() && value.selector.is_none() {
            return Err(LanguageError::Invalid(
                "attaching an image requires target or selector".into(),
            ));
        }
    }
    if !value.paths.is_empty() {
        validate_within(value.paths.len(), UPLOAD_PATHS, "paths")?;
        if has_duplicates(&value.paths) {
            return Err(LanguageError::Invalid("paths must be unique".into()));
        }
        for path in &value.paths {
            validate_text(path, UPLOAD_PATH_CHARS, "path")?;
            if !Path::new(path).is_absolute() {
                return Err(LanguageError::Invalid("paths must be absolute".into()));
            }
        }
    } else {
        validate_range(value.files.len(), 0, INLINE_FILE_LIMIT, "files")?;
        let mut aggregate = 0u64;
        for file in &value.files {
            validate_text(&file.name, FILE_NAME_CHARS, "name")?;
            validate_text(&file.media_type, MEDIA_TYPE_CHARS, "media_type")?;
            let data = file.data_base64.as_bytes();
            if data.is_empty() || data.len() % 4 != 0 {
                return Err(LanguageError::Invalid(
                    "data_base64 must be non-empty valid base64".into(),
                ));
            }
            if !data.iter().all(|byte| {
                byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/' || *byte == b'='
            }) {
                return Err(LanguageError::Invalid(
                    "data_base64 must be standard base64".into(),
                ));
            }
            aggregate = aggregate.saturating_add(base64_decoded_len(&file.data_base64));
        }
        if aggregate > UPLOAD_AGGREGATE_BYTES {
            return Err(LanguageError::Invalid(
                "inline files exceed the 5,000,000-byte upload ceiling".into(),
            ));
        }
    }
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_coordinate(value: f64, field: &str) -> Result<(), LanguageError> {
    if value.is_finite() && (0.0..=f64::from(COORDINATE_MAX)).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be a finite non-negative coordinate"
        )))
    }
}

fn validate_extent(value: f64, field: &str) -> Result<(), LanguageError> {
    if value.is_finite() && (0.0..=f64::from(COORDINATE_MAX)).contains(&value) && value > 0.0 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be a finite positive extent"
        )))
    }
}

fn validate_press_key(value: &PressKey) -> Result<(), LanguageError> {
    if value.strokes.is_empty() {
        validate_key(&value.key)?;
        if value.repeat != 1 {
            return Err(LanguageError::Invalid(
                "repeat applies to a stroke sequence".into(),
            ));
        }
    } else {
        if !value.key.is_empty() {
            return Err(LanguageError::Invalid(
                "provide exactly key or strokes".into(),
            ));
        }
        validate_within(value.strokes.len(), KEY_STROKES, "strokes")?;
        for stroke in &value.strokes {
            validate_key(stroke)?;
        }
        validate_within(usize::from(value.repeat), KEY_REPEAT, "repeat")?;
    }
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_optional_handle(value.target.as_deref(), HandleKind::Target)?;
    validate_modifiers(&value.modifiers)?;
    validate_expect(&value.expect)?;
    validate_timeout(value.timeout_ms)?;
    Ok(())
}

fn validate_wait(value: &Wait) -> Result<(), LanguageError> {
    validate_optional_handle(value.tab.as_deref(), HandleKind::Tab)?;
    validate_timeout(value.timeout_ms)?;
    if value.condition == WaitCondition::SelectorPresent {
        let selector = value
            .selector
            .as_ref()
            .ok_or_else(|| LanguageError::Invalid("selector_present requires selector".into()))?;
        if selector.name.trim().is_empty() {
            return Err(LanguageError::Invalid(
                "selector_present requires a non-empty name".into(),
            ));
        }
        if value.value.is_some() || value.target.is_some() {
            return Err(LanguageError::Invalid(
                "selector_present accepts neither value nor target".into(),
            ));
        }
        return Ok(());
    }
    if value.selector.is_some() {
        return Err(LanguageError::Invalid(
            "only selector_present accepts selector".into(),
        ));
    }
    validate_condition(
        value.condition,
        value.value.as_deref(),
        value.target.as_deref(),
    )
}

fn validate_condition(
    condition: WaitCondition,
    value: Option<&str>,
    target: Option<&str>,
) -> Result<(), LanguageError> {
    let name = condition.as_str();
    match condition {
        WaitCondition::Duration => {
            let raw =
                value.ok_or_else(|| LanguageError::Invalid("duration requires value".into()))?;
            if target.is_some() {
                return Err(LanguageError::Invalid(
                    "duration does not accept target".into(),
                ));
            }
            let milliseconds = raw.parse::<u64>().map_err(|_| {
                LanguageError::Invalid("duration requires whole milliseconds".into())
            })?;
            validate_within(
                usize::try_from(milliseconds).unwrap_or(usize::MAX),
                DURATION_MS,
                "duration",
            )
        }
        WaitCondition::LoadReady if value.is_some() || target.is_some() => Err(
            LanguageError::Invalid("load_ready accepts neither value nor target".into()),
        ),
        WaitCondition::LoadReady | WaitCondition::SelectorPresent => Ok(()),
        WaitCondition::VisualSettle => {
            if value.is_some() {
                return Err(refuses_value(name));
            }
            if let Some(target) = target {
                validate_handle(target, HandleKind::Target)?;
            }
            Ok(())
        }
        WaitCondition::UrlContains | WaitCondition::TextPresent | WaitCondition::TextAbsent => {
            let value =
                value.ok_or_else(|| LanguageError::Invalid(format!("{name} requires value")))?;
            validate_text(value, MAX_POSTCONDITION_VALUE_CHARS, "value")?;
            if target.is_some() {
                Err(LanguageError::Invalid(format!(
                    "{name} does not accept target"
                )))
            } else {
                Ok(())
            }
        }
        WaitCondition::TargetPresent | WaitCondition::TargetAbsent => {
            let target =
                target.ok_or_else(|| LanguageError::Invalid(format!("{name} requires target")))?;
            validate_handle(target, HandleKind::Target)?;
            if value.is_some() {
                Err(refuses_value(name))
            } else {
                Ok(())
            }
        }
    }
}

/// The refusal of a wait condition given a value it does not take.
fn refuses_value(condition: &str) -> LanguageError {
    LanguageError::Invalid(format!("{condition} does not accept value"))
}

fn validate_url(value: &str) -> Result<(), LanguageError> {
    if value.len() > URL_MAX {
        return Err(LanguageError::Invalid(format!(
            "url is limited to {URL_MAX} bytes; got {}",
            value.len()
        )));
    }
    let parsed = Url::parse(value).map_err(|_| {
        LanguageError::Invalid(
            "url must be absolute: include the scheme, as in https://example.com".into(),
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(LanguageError::Invalid(
            "url must use http or https and include a host".into(),
        ));
    }
    Ok(())
}

fn validate_timeout(value: u64) -> Result<(), LanguageError> {
    if (MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}; got {value}"
        )))
    }
}

fn validate_within(value: usize, range: Range, field: &str) -> Result<(), LanguageError> {
    validate_range(value, range.min, range.max, field)
}

fn validate_range(
    value: usize,
    minimum: usize,
    maximum: usize,
    field: &str,
) -> Result<(), LanguageError> {
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be between {minimum} and {maximum}; got {value}"
        )))
    }
}

fn validate_choice(value: &str, choices: &[&str], field: &str) -> Result<(), LanguageError> {
    if choices.contains(&value) {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} must be one of {}: got `{value}`",
            choices.join(", ")
        )))
    }
}

fn validate_handle(value: &str, kind: HandleKind) -> Result<(), LanguageError> {
    let prefix = kind.prefix();
    if value.starts_with(prefix) && value.len() > prefix.len() && value.len() <= HANDLE_CHARS {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "handles start with {prefix}; pass one exactly as a previous result issued it"
        )))
    }
}

fn validate_optional_handle(value: Option<&str>, kind: HandleKind) -> Result<(), LanguageError> {
    value.map_or(Ok(()), |handle| validate_handle(handle, kind))
}

fn validate_text(value: &str, maximum: usize, field: &str) -> Result<(), LanguageError> {
    if value.trim().is_empty() {
        return Err(LanguageError::Invalid(format!("{field} cannot be empty")));
    }
    validate_text_allow_empty(value, maximum, field)
}

fn validate_text_allow_empty(
    value: &str,
    maximum: usize,
    field: &str,
) -> Result<(), LanguageError> {
    let length = value.chars().count();
    if length <= maximum {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "{field} is limited to {maximum} characters; got {length}"
        )))
    }
}

fn validate_modifiers(values: &[String]) -> Result<(), LanguageError> {
    let mut seen = Vec::new();
    for value in values {
        validate_choice(value, &["Alt", "Control", "Meta", "Shift"], "modifiers")?;
        if seen.contains(value) {
            return Err(LanguageError::Invalid("modifiers must be unique".into()));
        }
        seen.push(value.clone());
    }
    Ok(())
}

fn validate_key(value: &str) -> Result<(), LanguageError> {
    if NAMED_KEYS.contains(&value) || value.chars().count() == 1 {
        Ok(())
    } else {
        Err(LanguageError::Invalid(format!(
            "key must be one character or a supported named key: {}",
            NAMED_KEYS.join(", ")
        )))
    }
}

fn has_duplicates(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[..index].contains(value))
}
