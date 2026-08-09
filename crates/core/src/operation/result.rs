// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Operation-owned reduction from private browser evidence to closed Ghostlight results.
//!
//! The policy-free adapter returns a private mechanism envelope containing `content`, optional
//! `structuredContent`, and optional `isError`. This module consumes it exactly once inside the
//! admitted operation. Protocol wrappers and unknown fields are rejected rather than retained in
//! the service result.

use super::registry::SuccessDisposition;
use crate::tool::outcome::{
    ExecutionDisposition, NativeTabFact, OperationCompletion, OperationExecution,
    OperationTopology, ResolvedTargets,
};
use ghostlight_transport::operation::{
    BrowserConnectionStatus, BrowserResult, BrowserResultStatus, CanonicalCursor, CaptureScope,
    DialogKind, DialogResolution, FilledFieldResult, FlowResultData, GovernanceModeStatus,
    Operation, OperationEffect, OperationKind, OperationResult, PageProvenance, PolicySourceStatus,
    ResultPart, SkippedFieldResult, StatusAuthority, StatusLimits, TargetAction, TargetFact,
    MAX_PAGE_ORIGIN_BYTES,
};
use ghostlight_transport::workspace_id::WorkspaceId;
use serde_json::{Map, Value};

/// A current internal success value cannot be represented by the canonical result vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResultConversionError {
    /// The current success boundary requires an object result.
    #[error("successful browser result must be an object")]
    RootNotObject,
    /// A top-level field is not part of the private mechanism success contract.
    #[error("successful browser result contains unsupported top-level field: {field}")]
    UnsupportedTopLevelField {
        /// Unsupported field name. Its value is never retained or rendered.
        field: String,
    },
    /// The optional content field was not an array.
    #[error("successful browser result content must be an array")]
    ContentNotArray,
    /// The optional error marker was not a boolean.
    #[error("successful browser result isError must be a boolean")]
    ErrorMarkerNotBoolean,
    /// The temporary grouped execution identity has no canonical surface operation.
    #[error("successful execution result has no canonical operation identity")]
    UnmappedOperation,
    /// Successful browser evidence omitted a fact required by the operation result.
    #[error("successful {operation} result omitted required fact: {fact}")]
    MissingResultFact {
        /// Exact canonical operation.
        operation: OperationKind,
        /// Stable name of the missing fact.
        fact: &'static str,
    },
    /// A sequence exceeded the fixed aggregate image count or byte budget.
    #[error("canonical sequence media exceeds the aggregate result budget")]
    SequenceMediaLimit,
    /// One content item was not an object.
    #[error("successful browser result content block {index} must be an object")]
    ContentBlockNotObject {
        /// Zero-based content-block index.
        index: usize,
    },
    /// One content item had no string type discriminator.
    #[error("successful browser result content block {index} must have a string type")]
    ContentBlockTypeMissing {
        /// Zero-based content-block index.
        index: usize,
    },
    /// The current canonical result vocabulary does not support this block type.
    #[error("successful browser result content block {index} has unsupported type: {block_type}")]
    UnsupportedContentBlock {
        /// Zero-based content-block index.
        index: usize,
        /// Unsupported type discriminator. No block payload is retained.
        block_type: String,
    },
    /// A text block did not have the exact supported shape.
    #[error("successful browser result text block {index} must contain only type and string text")]
    InvalidTextBlock {
        /// Zero-based content-block index.
        index: usize,
    },
    /// An image block did not have one exact supported base64 shape.
    #[error(
        "successful browser result image block {index} must contain base64 data and a media type"
    )]
    InvalidImageBlock {
        /// Zero-based content-block index.
        index: usize,
    },
    /// Both reserved legacy provenance locations were populated, so placement is ambiguous.
    #[error("successful browser result contains conflicting provenance markers")]
    ConflictingProvenanceMarkers,
    /// A reserved legacy provenance marker did not have the exact service-authored shape.
    #[error("successful browser result has malformed provenance at {location}: {reason}")]
    MalformedProvenanceMarker {
        /// Reserved legacy marker location.
        location: &'static str,
        /// Stable validation reason without retaining marker payload.
        reason: &'static str,
    },
}

/// Adapter evidence parsed at the operation boundary before typed result construction.
pub struct CanonicalizedMechanism {
    /// Canonical terminal envelope without its operation-owned result.
    pub result: BrowserResult,
    /// Private structured mechanism evidence consumed by exactly one operation reducer.
    pub evidence: Value,
}

/// Convert one current internal successful result into the canonical result vocabulary.
///
/// Accepted top-level fields are `content`, `structuredContent`, and `isError`. Text blocks and
/// base64 image blocks become typed [`ResultPart`] values. `structuredContent` becomes canonical
/// structured data. The registry-derived [`SuccessDisposition`] supplies canonical status,
/// effect, and retry semantics. The temporary `isError` marker is validated and removed, but it
/// cannot independently weaken or strengthen that disposition. Unknown fields and unsupported
/// content shapes return [`ResultConversionError`] instead of crossing the bridge through a
/// fallback payload.
pub fn canonicalize_success(
    operation: OperationKind,
    disposition: SuccessDisposition,
    workspace: Option<WorkspaceId>,
    value: Value,
) -> Result<CanonicalizedMechanism, ResultConversionError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(ResultConversionError::RootNotObject)?;

    if let Some(field) = object
        .keys()
        .find(|field| !matches!(field.as_str(), "content" | "structuredContent" | "isError"))
        .cloned()
    {
        return Err(ResultConversionError::UnsupportedTopLevelField { field });
    }

    match object.remove("isError") {
        None | Some(Value::Bool(_)) => {}
        Some(_) => return Err(ResultConversionError::ErrorMarkerNotBoolean),
    }
    let parts = parse_content(object.remove("content"))?;
    let mut data = object.remove("structuredContent").unwrap_or(Value::Null);
    let provenance = take_mechanism_provenance(&mut data, &parts)?;

    let mut result = BrowserResult::new(operation, disposition.status, disposition.effect);
    if let Some(repeat) = disposition.retry {
        result.repeat = repeat;
    }
    result.workspace = workspace;
    result.parts = parts;
    result.provenance = provenance;
    Ok(CanonicalizedMechanism {
        result,
        evidence: data,
    })
}

/// Convert one internal success and project its adapter-shaped data into the canonical result
/// payload owned by the admitted operation.
pub fn canonicalize_operation_success(
    operation: &Operation,
    disposition: SuccessDisposition,
    workspace: Option<WorkspaceId>,
    value: Value,
) -> Result<CanonicalizedMechanism, ResultConversionError> {
    let mut canonical = canonicalize_success(operation.kind(), disposition, workspace, value)?;
    canonical.result.operation = operation.kind();
    Ok(canonical)
}

/// Consume private adapter evidence into one operation-owned typed completion.
///
/// This is the action pipeline's result reducer. After it returns, adapter JSON is gone. The
/// completion chokepoint receives only the typed result and typed browser-topology facts needed
/// to bind opaque workspace handles.
pub fn build_operation_completion(
    operation: &Operation,
    workspace: Option<WorkspaceId>,
    execution: OperationExecution,
) -> Result<OperationCompletion, ResultConversionError> {
    let disposition = match execution.disposition {
        ExecutionDisposition::DescriptorDefault => {
            crate::operation::registry::descriptor(operation.kind()).success_disposition(&execution)
        }
        ExecutionDisposition::Override(disposition) => disposition,
    };
    let operation_tab = execution.operation_tab;
    let readiness = execution.navigation.readiness.clone();
    let final_navigation_url = execution.navigation.final_url.clone();
    let (target, from_target, to_target) = match &execution.targets {
        ResolvedTargets::None => (None, None, None),
        ResolvedTargets::One(target) => (Some(target.clone()), None, None),
        ResolvedTargets::Drag { from, to } => (None, Some(from.clone()), Some(to.clone())),
    };
    let canonical =
        canonicalize_operation_success(operation, disposition, workspace, execution.into_value())?;
    let evidence = canonical.evidence;
    let mut result = canonical.result;
    result.readiness = readiness;
    result
        .parts
        .extend(sequence_media_parts(operation, &evidence)?);
    if matches!(
        result.status,
        BrowserResultStatus::Ok | BrowserResultStatus::Partial | BrowserResultStatus::NotMet
    ) {
        result.result = Some(reduce_operation_result(
            operation,
            &result,
            &evidence,
            target.as_ref(),
            from_target.as_ref(),
            to_target.as_ref(),
        )?);
    }
    Ok(OperationCompletion {
        result,
        topology: operation_topology(&evidence, operation_tab, final_navigation_url),
    })
}

/// Reduce private mechanism evidence into the closed result owned by this operation.
fn reduce_operation_result(
    operation: &Operation,
    result: &BrowserResult,
    evidence: &Value,
    target: Option<&Value>,
    from_target: Option<&Value>,
    to_target: Option<&Value>,
) -> Result<OperationResult, ResultConversionError> {
    reduce_operation_payload(operation, result, evidence, target, from_target, to_target)
}

const MAX_SEQUENCE_MEDIA_BYTES: usize = 16 * 1024 * 1024;

fn operation_topology(
    evidence: &Value,
    affected_tab: Option<i64>,
    final_navigation_url: Option<String>,
) -> OperationTopology {
    let candidates = [
        "",
        "/page",
        "/interactionReceipt/page",
        "/interactionReceipt/observedAfter",
    ]
    .into_iter()
    .filter_map(|pointer| {
        let value = if pointer.is_empty() {
            evidence
        } else {
            evidence.pointer(pointer)?
        };
        native_tab_fact(value)
    })
    .collect();
    let inventory = ["/tabs", "/tabDelta/opened"]
        .into_iter()
        .filter_map(|pointer| evidence.pointer(pointer).and_then(Value::as_array))
        .flatten()
        .filter_map(native_tab_fact)
        .take(ghostlight_transport::operation::MAX_RESULT_TABS)
        .collect();
    let mut closed_tabs = evidence
        .pointer("/tabDelta/closed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .collect::<Vec<_>>();
    if evidence
        .pointer("/interactionReceipt/observedAfter/tabClosed")
        .and_then(Value::as_bool)
        == Some(true)
    {
        if let Some(tab) = affected_tab {
            if !closed_tabs.contains(&tab) {
                closed_tabs.push(tab);
            }
        }
    }
    OperationTopology {
        affected_tab,
        candidates,
        inventory,
        closed_tabs,
        final_navigation_url,
    }
}

fn native_tab_fact(value: &Value) -> Option<NativeTabFact> {
    let object = value.as_object()?;
    Some(NativeTabFact {
        tab_id: object.get("tabId")?.as_i64()?,
        url: bounded_fact(
            object.get("url"),
            ghostlight_transport::operation::MAX_RESULT_TAB_URL_BYTES,
        ),
        title: bounded_fact(
            object.get("title"),
            ghostlight_transport::operation::MAX_RESULT_TAB_TITLE_BYTES,
        ),
        redacted: object
            .get("redacted")
            .and_then(Value::as_str)
            .and_then(ghostlight_transport::operation::TabFactRedaction::parse),
    })
}

fn bounded_fact(value: Option<&Value>, max_bytes: usize) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
}

fn sequence_media_parts(
    operation: &Operation,
    evidence: &Value,
) -> Result<Vec<ResultPart>, ResultConversionError> {
    if !matches!(operation, Operation::BrowserRunSequence(_)) {
        return Ok(Vec::new());
    }
    let flow = serde_json::from_value::<FlowResultData>(evidence.clone())
        .map_err(|_| missing(operation, "steps"))?;
    let media = flow
        .steps
        .into_iter()
        .flat_map(|step| step.result.parts)
        .filter(|part| matches!(part, ResultPart::Image { .. }))
        .collect::<Vec<_>>();
    let bytes = media
        .iter()
        .map(|part| match part {
            ResultPart::Image { data, mime_type } => data.len() + mime_type.len(),
            ResultPart::Text { .. } => 0,
        })
        .sum::<usize>();
    if media.len() > ghostlight_transport::operation::MAX_OPERATION_SEQUENCE_MEDIA_PARTS
        || bytes > MAX_SEQUENCE_MEDIA_BYTES
    {
        return Err(ResultConversionError::SequenceMediaLimit);
    }
    Ok(media)
}

fn reduce_operation_payload(
    operation: &Operation,
    result: &BrowserResult,
    evidence: &Value,
    target: Option<&Value>,
    from_target: Option<&Value>,
    to_target: Option<&Value>,
) -> Result<OperationResult, ResultConversionError> {
    let observed = |field: &str| {
        evidence
            .pointer(&format!("/interactionReceipt/observedAfter/{field}"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let committed = result.effect == OperationEffect::Committed
        && matches!(
            result.status,
            BrowserResultStatus::Ok | BrowserResultStatus::Partial | BrowserResultStatus::NotMet
        );

    if !matches!(
        result.status,
        BrowserResultStatus::Ok | BrowserResultStatus::Partial | BrowserResultStatus::NotMet
    ) {
        return Err(missing(operation, "successful terminal result"));
    }
    if matches!(operation, Operation::BrowserRunSequence(_)) {
        return serde_json::from_value::<FlowResultData>(evidence.clone())
            .map(OperationResult::BrowserRunSequence)
            .map_err(|_| missing(operation, "steps"));
    }

    let operation_result = match operation {
        Operation::BrowserGetStatus(_) => {
            let operations = crate::operation::registry::descriptors()
                .iter()
                .map(|descriptor| descriptor.operation)
                .collect::<Vec<_>>();
            OperationResult::BrowserGetStatus {
                browser: if evidence
                    .get("browserConnected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    BrowserConnectionStatus::Connected
                } else {
                    BrowserConnectionStatus::Disconnected
                },
                authority: StatusAuthority {
                    policy_source: evidence
                        .pointer("/authority/policySource")
                        .and_then(Value::as_str)
                        .and_then(PolicySourceStatus::parse)
                        .unwrap_or(PolicySourceStatus::None),
                    mode: evidence
                        .pointer("/authority/mode")
                        .and_then(Value::as_str)
                        .and_then(GovernanceModeStatus::parse)
                        .unwrap_or(GovernanceModeStatus::Open),
                },
                operations,
                packs: Vec::new(),
                limits: StatusLimits {
                    max_sequence_steps: 10,
                    max_tabs: 64,
                    max_read_chars: 50_000,
                },
            }
        }
        Operation::BrowserOpenTab(arguments) => {
            let created = evidence
                .get("created")
                .and_then(Value::as_bool)
                .ok_or_else(|| missing(operation, "created"))?;
            OperationResult::BrowserOpenTab {
                created,
                navigated: arguments
                    .url
                    .as_ref()
                    .and_then(|_| evidence.get("navigated").and_then(Value::as_bool)),
            }
        }
        Operation::BrowserListTabs(_) => OperationResult::BrowserListTabs {
            count: u32::try_from(
                evidence
                    .get("tabs")
                    .and_then(Value::as_array)
                    .map_or(result.tabs.len(), Vec::len),
            )
            .unwrap_or(u32::MAX),
        },
        Operation::BrowserFocusTab(_) => OperationResult::BrowserFocusTab {
            focused: observed("tabFocused"),
        },
        Operation::BrowserCloseTab(_) => OperationResult::BrowserCloseTab {
            closed: observed("tabClosed"),
        },
        Operation::BrowserNavigate(_) => OperationResult::BrowserNavigate {
            landed: committed && result.readiness.is_some(),
        },
        Operation::BrowserGoBack(_) => OperationResult::BrowserGoBack {
            moved: committed && result.status != BrowserResultStatus::NotMet,
        },
        Operation::BrowserGoForward(_) => OperationResult::BrowserGoForward {
            moved: committed && result.status != BrowserResultStatus::NotMet,
        },
        Operation::BrowserReloadPage(_) => OperationResult::BrowserReloadPage {
            reloaded: observed("tabReloaded") || committed,
        },
        Operation::BrowserInspectPage(_) => {
            let values = evidence
                .get("targets")
                .or_else(|| evidence.get("results"))
                .and_then(Value::as_array)
                .ok_or_else(|| missing(operation, "targets"))?;
            let targets = values
                .iter()
                .take(100)
                .filter_map(target_fact)
                .collect::<Vec<_>>();
            OperationResult::BrowserInspectPage {
                targets,
                more: evidence
                    .get("more")
                    .and_then(Value::as_bool)
                    .unwrap_or(values.len() > 100),
                cursor: evidence
                    .get("cursor")
                    .and_then(Value::as_str)
                    .and_then(CanonicalCursor::parse),
            }
        }
        Operation::BrowserReadPage(arguments) => {
            let text = first_text_part(result).ok_or_else(|| missing(operation, "text"))?;
            let bounded = text
                .chars()
                .take(arguments.max_chars as usize)
                .collect::<String>();
            OperationResult::BrowserReadPage {
                text: bounded,
                more: text.chars().count() > arguments.max_chars as usize,
                cursor: evidence
                    .get("cursor")
                    .and_then(Value::as_str)
                    .and_then(CanonicalCursor::parse),
            }
        }
        Operation::BrowserTakeScreenshot(_) => {
            let capture = evidence
                .get("capture")
                .and_then(Value::as_object)
                .ok_or_else(|| missing(operation, "capture"))?;
            let width = capture
                .get("width")
                .and_then(Value::as_u64)
                .ok_or_else(|| missing(operation, "capture.width"))?;
            let height = capture
                .get("height")
                .and_then(Value::as_u64)
                .ok_or_else(|| missing(operation, "capture.height"))?;
            let scope = capture
                .get("scope")
                .and_then(Value::as_str)
                .ok_or_else(|| missing(operation, "capture.scope"))?;
            let scope =
                CaptureScope::parse(scope).ok_or_else(|| missing(operation, "capture.scope"))?;
            OperationResult::BrowserTakeScreenshot {
                frame: format!("f_{}", uuid::Uuid::new_v4().simple()),
                width: u32::try_from(width).map_err(|_| missing(operation, "capture.width"))?,
                height: u32::try_from(height).map_err(|_| missing(operation, "capture.height"))?,
                scope,
                target: match scope {
                    CaptureScope::Viewport => None,
                    CaptureScope::Target => Some(canonical_target(operation, target, "target")?),
                },
            }
        }
        Operation::BrowserClick(_) => OperationResult::BrowserClick {
            target: canonical_target(operation, target, "target")?,
            clicked: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserHover(_) => OperationResult::BrowserHover {
            target: canonical_target(operation, target, "target")?,
            hovered: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserScrollToTarget(_) => OperationResult::BrowserScrollToTarget {
            target: canonical_target(operation, target, "target")?,
            visible: committed,
            moved: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserScrollPage(arguments) => OperationResult::BrowserScrollPage {
            direction: arguments.direction,
            amount: arguments.amount,
            moved: evidence
                .pointer("/scroll/moved")
                .and_then(Value::as_bool)
                .unwrap_or(committed),
            page_changed: page_changed(evidence),
        },
        Operation::BrowserPressKey(arguments) => OperationResult::BrowserPressKey {
            key: arguments.key,
            target: canonical_target(operation, target, "target")?,
            pressed: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserPressEscape(_) => OperationResult::BrowserPressEscape {
            pressed: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserDrag(_) => OperationResult::BrowserDrag {
            from: canonical_target(operation, from_target, "from")?,
            to: canonical_target(operation, to_target, "to")?,
            dragged: committed,
            page_changed: page_changed(evidence),
        },
        Operation::BrowserFillForm(arguments) => {
            let filled = evidence
                .get("filled")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|field| {
                    let name = field
                        .get("label")
                        .or_else(|| field.get("field"))
                        .and_then(Value::as_str)?;
                    Some(FilledFieldResult {
                        field: name.to_owned(),
                    })
                })
                .collect::<Vec<_>>();
            let skipped = evidence
                .get("skipped")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|field| {
                    let name = field
                        .get("label")
                        .or_else(|| field.get("field"))
                        .and_then(Value::as_str)?;
                    let code = field
                        .get("kind")
                        .or_else(|| field.get("reason"))
                        .and_then(Value::as_str)
                        .map(problem_token)
                        .unwrap_or_else(|| "not_filled".to_owned());
                    Some(SkippedFieldResult {
                        field: name.to_owned(),
                        code,
                    })
                })
                .collect::<Vec<_>>();
            OperationResult::BrowserFillForm {
                filled,
                skipped,
                submitted: evidence
                    .get("submitted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                submit_target: arguments.submit_target.as_ref().and_then(|_| {
                    evidence
                        .get("submit_ref")
                        .and_then(Value::as_str)
                        .map(ref_only_target)
                }),
            }
        }
        Operation::BrowserWaitFor(arguments) => OperationResult::BrowserWaitFor {
            condition: arguments.condition.clone(),
            state: arguments.state,
            met: result.status != BrowserResultStatus::NotMet
                && evidence
                    .get("found")
                    .or_else(|| evidence.get("met"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            elapsed_ms: u32::try_from(
                evidence
                    .get("elapsed_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(arguments.timeout_ms as u64)
                    .min(30_000),
            )
            .expect("elapsed time is clamped to u32"),
        },
        Operation::BrowserRunSequence(_) => unreachable!("sequence results return above"),
        Operation::BrowserGetDialog(_) => {
            let open = evidence
                .get("open")
                .and_then(Value::as_bool)
                .ok_or_else(|| missing(operation, "open"))?;
            OperationResult::BrowserGetDialog {
                open,
                kind: open.then(|| dialog_kind(evidence.get("type").and_then(Value::as_str))),
                message: open.then(|| {
                    evidence
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .chars()
                        .take(2000)
                        .collect()
                }),
                actions: if open {
                    vec![
                        DialogResolution::Accept,
                        DialogResolution::Dismiss,
                        DialogResolution::Respond,
                    ]
                } else {
                    Vec::new()
                },
            }
        }
        Operation::BrowserHandleDialog(arguments) => OperationResult::BrowserHandleDialog {
            action: arguments.action,
            resolved: evidence
                .get("resolved")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
    };
    Ok(operation_result)
}

fn missing(operation: &Operation, fact: &'static str) -> ResultConversionError {
    ResultConversionError::MissingResultFact {
        operation: operation.kind(),
        fact,
    }
}

fn first_text_part(result: &BrowserResult) -> Option<&str> {
    result.parts.iter().find_map(|part| match part {
        ResultPart::Text { text } => Some(text.as_str()),
        ResultPart::Image { .. } => None,
    })
}

fn canonical_target(
    operation: &Operation,
    value: Option<&Value>,
    field: &'static str,
) -> Result<TargetFact, ResultConversionError> {
    value
        .and_then(target_fact)
        .ok_or_else(|| missing(operation, field))
}

fn target_fact(value: &Value) -> Option<TargetFact> {
    let object = value.as_object()?;
    let reference = object.get("ref")?.as_str()?;
    let mut projected = Vec::new();
    if let Some(actions) = object
        .get("mechanicalActions")
        .or_else(|| object.get("actions"))
        .and_then(Value::as_array)
    {
        for action in actions.iter().filter_map(Value::as_str) {
            let action = match action {
                "left_click" | "right_click" | "double_click" | "triple_click" | "click" => {
                    TargetAction::Click
                }
                "hover" => TargetAction::Hover,
                "scroll_to" => TargetAction::ScrollTo,
                "set_value" | "fill" => TargetAction::Fill,
                "drag" => TargetAction::Drag,
                "press_key" => TargetAction::PressKey,
                _ => continue,
            };
            if !projected.contains(&action) {
                projected.push(action);
            }
        }
    }
    Some(TargetFact {
        r#ref: canonical_ref(reference),
        role: object
            .get("role")
            .and_then(Value::as_str)
            .map(|value| value.chars().take(64).collect()),
        name: object
            .get("name")
            .and_then(Value::as_str)
            .map(|value| value.chars().take(500).collect()),
        visible: object.get("visible").and_then(Value::as_bool),
        enabled: object.get("enabled").and_then(Value::as_bool),
        actions: projected,
    })
}

fn canonical_ref(reference: &str) -> String {
    reference
        .strip_prefix("ref_")
        .map_or_else(|| reference.to_owned(), |suffix| format!("r_{suffix}"))
}

fn ref_only_target(reference: &str) -> TargetFact {
    TargetFact {
        r#ref: canonical_ref(reference),
        role: None,
        name: None,
        visible: None,
        enabled: None,
        actions: Vec::new(),
    }
}

fn page_changed(evidence: &Value) -> bool {
    let observed = evidence
        .pointer("/interactionReceipt/observedAfter")
        .and_then(Value::as_object);
    observed.is_some_and(|observed| {
        observed
            .get("mutations")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
            || [
                "renderAdvanced",
                "urlChanged",
                "titleChanged",
                "alertOrStatus",
            ]
            .iter()
            .any(|field| observed.contains_key(*field))
            || observed
                .get("changedElements")
                .and_then(Value::as_array)
                .is_some_and(|value| !value.is_empty())
    })
}

fn problem_token(value: &str) -> String {
    let mut token = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while token.contains("__") {
        token = token.replace("__", "_");
    }
    token.trim_matches('_').chars().take(64).collect::<String>()
}

fn dialog_kind(value: Option<&str>) -> DialogKind {
    match value {
        Some("alert") => DialogKind::Alert,
        Some("confirm") => DialogKind::Confirm,
        Some("prompt") => DialogKind::Prompt,
        Some("beforeunload") => DialogKind::BeforeUnload,
        _ => DialogKind::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MechanismProvenancePlacement {
    Root,
    InteractionReceipt,
}

impl MechanismProvenancePlacement {
    const fn location(self) -> &'static str {
        match self {
            Self::Root => "structuredContent.provenance",
            Self::InteractionReceipt => "structuredContent.interactionReceipt.provenance",
        }
    }

    const fn data_pointer(self) -> &'static str {
        match self {
            Self::Root | Self::InteractionReceipt => "/result",
        }
    }
}

fn take_mechanism_provenance(
    data: &mut Value,
    parts: &[ResultPart],
) -> Result<Option<PageProvenance>, ResultConversionError> {
    let Some(root) = data.as_object() else {
        return Ok(None);
    };
    let root_marker = root.contains_key("provenance");
    let receipt_marker = root
        .get("interactionReceipt")
        .and_then(Value::as_object)
        .is_some_and(|receipt| receipt.contains_key("provenance"));
    if root_marker && receipt_marker {
        return Err(ResultConversionError::ConflictingProvenanceMarkers);
    }
    if root_marker && root.contains_key("interactionReceipt") {
        return Err(ResultConversionError::MalformedProvenanceMarker {
            location: MechanismProvenancePlacement::Root.location(),
            reason: "root marker cannot accompany interactionReceipt",
        });
    }
    let placement = match (root_marker, receipt_marker) {
        (false, false) => return Ok(None),
        (true, true) => unreachable!("conflicting markers were rejected above"),
        (true, false) => MechanismProvenancePlacement::Root,
        (false, true) => MechanismProvenancePlacement::InteractionReceipt,
    };

    let marker = match placement {
        MechanismProvenancePlacement::Root => data
            .as_object_mut()
            .and_then(|root| root.remove("provenance")),
        MechanismProvenancePlacement::InteractionReceipt => data
            .get_mut("interactionReceipt")
            .and_then(Value::as_object_mut)
            .and_then(|receipt| receipt.remove("provenance")),
    }
    .expect("the selected legacy provenance marker was observed above");

    let (top_origin, session_nonce, frame_origin) =
        parse_legacy_provenance_marker(marker, placement.location())?;
    let mut untrusted_fields = vec![placement.data_pointer().to_owned()];
    for (index, part) in parts.iter().enumerate() {
        let field = match part {
            ResultPart::Text { .. } => "text",
            ResultPart::Image { .. } => "data",
        };
        untrusted_fields.push(format!("/parts/{index}/{field}"));
    }

    PageProvenance::new(
        untrusted_fields,
        Some(top_origin),
        Some(session_nonce),
        frame_origin,
    )
    .map(Some)
    .map_err(|_| ResultConversionError::MalformedProvenanceMarker {
        location: placement.location(),
        reason: "frameOrigin is empty, contains a control character, or exceeds 240 UTF-8 bytes",
    })
}

fn parse_legacy_provenance_marker(
    marker: Value,
    location: &'static str,
) -> Result<(String, String, Option<String>), ResultConversionError> {
    let Value::Object(marker) = marker else {
        return malformed_provenance(location, "marker must be an object");
    };
    if marker.keys().any(|field| {
        !matches!(
            field.as_str(),
            "pageSourced" | "untrusted" | "topOrigin" | "frameOrigin" | "sessionNonce"
        )
    }) {
        return malformed_provenance(location, "marker contains an unsupported field");
    }
    if marker.get("pageSourced") != Some(&Value::Bool(true)) {
        return malformed_provenance(location, "pageSourced must be true");
    }
    if marker.get("untrusted") != Some(&Value::Bool(true)) {
        return malformed_provenance(location, "untrusted must be true");
    }
    let top_origin = marker
        .get("topOrigin")
        .and_then(Value::as_str)
        .filter(|origin| is_valid_origin(origin))
        .ok_or(ResultConversionError::MalformedProvenanceMarker {
            location,
            reason: "topOrigin must be non-empty, control-free, and at most 240 UTF-8 bytes",
        })?
        .to_owned();
    let session_nonce = marker
        .get("sessionNonce")
        .and_then(Value::as_str)
        .filter(|nonce| is_valid_session_nonce(nonce))
        .ok_or(ResultConversionError::MalformedProvenanceMarker {
            location,
            reason: "sessionNonce must be bounded lowercase even-length hexadecimal with at least 96 bits",
        })?
        .to_owned();
    let frame_origin = match marker.get("frameOrigin") {
        None => None,
        Some(Value::String(origin)) => Some(origin.clone()),
        Some(_) => return malformed_provenance(location, "frameOrigin must be a string"),
    };
    Ok((top_origin, session_nonce, frame_origin))
}

fn malformed_provenance<T>(
    location: &'static str,
    reason: &'static str,
) -> Result<T, ResultConversionError> {
    Err(ResultConversionError::MalformedProvenanceMarker { location, reason })
}

fn is_valid_origin(origin: &str) -> bool {
    !origin.is_empty()
        && origin.len() <= MAX_PAGE_ORIGIN_BYTES
        && !origin.chars().any(char::is_control)
}

fn is_valid_session_nonce(nonce: &str) -> bool {
    const MIN_NONCE_BYTES: usize = 12;
    const MAX_NONCE_BYTES: usize = 64;

    nonce.len() >= MIN_NONCE_BYTES * 2
        && nonce.len() <= MAX_NONCE_BYTES * 2
        && nonce.len().is_multiple_of(2)
        && nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_content(value: Option<Value>) -> Result<Vec<ResultPart>, ResultConversionError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(blocks) = value else {
        return Err(ResultConversionError::ContentNotArray);
    };

    blocks
        .into_iter()
        .enumerate()
        .map(|(index, block)| parse_content_block(index, block))
        .collect()
}

fn parse_content_block(index: usize, block: Value) -> Result<ResultPart, ResultConversionError> {
    let Value::Object(block) = block else {
        return Err(ResultConversionError::ContentBlockNotObject { index });
    };
    let block_type = block
        .get("type")
        .and_then(Value::as_str)
        .ok_or(ResultConversionError::ContentBlockTypeMissing { index })?;

    match block_type {
        "text" => parse_text_block(index, block),
        "image" => parse_image_block(index, block),
        other => Err(ResultConversionError::UnsupportedContentBlock {
            index,
            block_type: other.to_owned(),
        }),
    }
}

fn parse_text_block(
    index: usize,
    block: Map<String, Value>,
) -> Result<ResultPart, ResultConversionError> {
    if block.len() != 2 {
        return Err(ResultConversionError::InvalidTextBlock { index });
    }
    let text = block
        .get("text")
        .and_then(Value::as_str)
        .ok_or(ResultConversionError::InvalidTextBlock { index })?;
    Ok(ResultPart::Text {
        text: text.to_owned(),
    })
}

fn parse_image_block(
    index: usize,
    block: Map<String, Value>,
) -> Result<ResultPart, ResultConversionError> {
    let parsed = if block.contains_key("source") {
        parse_source_image(&block)
    } else {
        parse_direct_image(&block)
    };
    let Some((data, mime_type)) = parsed else {
        return Err(ResultConversionError::InvalidImageBlock { index });
    };
    ResultPart::image(data, mime_type)
        .map_err(|_| ResultConversionError::InvalidImageBlock { index })
}

fn parse_direct_image(block: &Map<String, Value>) -> Option<(&str, &str)> {
    if block.len() != 3 {
        return None;
    }
    Some((
        block.get("data")?.as_str()?,
        block.get("mimeType")?.as_str()?,
    ))
}

fn parse_source_image(block: &Map<String, Value>) -> Option<(&str, &str)> {
    if !matches!(block.len(), 2 | 3) {
        return None;
    }
    if block
        .keys()
        .any(|field| !matches!(field.as_str(), "type" | "source" | "mimeType"))
    {
        return None;
    }
    let source = block.get("source")?.as_object()?;
    if source.get("type")?.as_str()? != "base64" {
        return None;
    }
    let data = source.get("data")?.as_str()?;

    let outer_mime = block.get("mimeType").and_then(Value::as_str);
    let source_snake_mime = source.get("media_type").and_then(Value::as_str);
    let source_camel_mime = source.get("mimeType").and_then(Value::as_str);
    let mime_count = usize::from(outer_mime.is_some())
        + usize::from(source_snake_mime.is_some())
        + usize::from(source_camel_mime.is_some());
    if mime_count != 1 {
        return None;
    }
    let mime_type = outer_mime.or(source_snake_mime).or(source_camel_mime)?;

    let expected_source_len = if source_snake_mime.is_some() || source_camel_mime.is_some() {
        3
    } else {
        2
    };
    if source.len() != expected_source_len {
        return None;
    }
    Some((data, mime_type))
}
