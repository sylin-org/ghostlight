//! The world messages of adapter protocol 4: what the service says to a page asset, and what the
//! asset says back.
//!
//! The extension passes a world message through unread, and the asset answers inside an envelope
//! the extension does not parse either. This module is the only place either side is spelled:
//! one typed constructor per message kind, the closed set of asset error codes, the envelope
//! decoder, and the typed values the translation reads.
//!
//! Every message carries two classes that the document scope enforces. Its scope class says which
//! documents may receive it: content kinds only reach documents the authority allowed, metadata
//! kinds reach any document in the admitted tree. Its effect class says what the call can have
//! changed, so a failure after an effect is never reported as clean.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

use super::wire::{CallAnswer, Point, Readiness};
use crate::browser::adapter::{
    BrowserOutcome, ObservedTarget, PhysicalActionSubject, PhysicalField, PhysicalFile,
};
use crate::language::field_problem::FieldProblems;
use EffectClass::{Effect, Prepare, Read};
use ScopeClass::{Content, Metadata};

/// A coordinate as the page reads it: a whole number stays a whole number, so a message built
/// from a point equals the same message written by hand.
fn coordinate(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < MAX_EXACT_INTEGER {
        json!(value as i64)
    } else {
        json!(value)
    }
}

/// The member of every message that names its kind.
const KIND_FIELD: &str = "kind";
/// The member that carries a kind's shared preparation budget in milliseconds.
const SETTLE_MS_FIELD: &str = "settle_ms";
/// The member that says whether a kind waits for the page to stop moving.
const VISUAL_SETTLE_FIELD: &str = "visual_settle";
/// The envelope member that says whether the asset succeeded.
const OK_FIELD: &str = "ok";
/// The envelope member that carries a successful value.
const VALUE_FIELD: &str = "value";
/// The envelope member that carries a failure.
const ERROR_FIELD: &str = "error";
/// The failure member that names its code.
const CODE_FIELD: &str = "code";
/// The failure member that carries its description.
const MESSAGE_FIELD: &str = "message";
/// The failure member that carries the optional detail a code does not say.
const DETAIL_FIELD: &str = "detail";
/// Longest description of a failure kept from the asset.
const MESSAGE_MAX_CHARS: usize = 500;
/// The description of an answer that is not an envelope.
const MALFORMED_ENVELOPE: &str = "the page asset answered without an envelope";
/// How long a filled value must keep its value before it counts as retained.
const FILL_STABLE_MS: u64 = 250;
/// How often a filled value is read back while it settles.
const FILL_POLL_MS: u64 = 25;
/// The largest magnitude below which every whole number is exact as a float.
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;

/// The closed vocabulary of world message kinds.
pub mod kind {
    /// Read page text.
    pub const READ_TEXT: &str = "read_text";
    /// List controls or structure.
    pub const INSPECT: &str = "inspect";
    /// Read the structure tree.
    pub const INSPECT_TREE: &str = "inspect_tree";
    /// Find targets by visible text.
    pub const FIND: &str = "find";
    /// Find targets by role and name.
    pub const QUERY_SEMANTIC: &str = "query_semantic";
    /// Describe targets by locator.
    pub const DESCRIBE: &str = "describe";
    /// Describe the focused control.
    pub const DESCRIBE_FOCUSED: &str = "describe_focused";
    /// Check that the focused control takes text.
    pub const VERIFY_FOCUSED_TEXT: &str = "verify_focused_text";
    /// Measure a target.
    pub const BOX: &str = "box";
    /// Validate a fill and wait for a stable form.
    pub const PREPARE_FILL: &str = "prepare_fill";
    /// Wait until filled values are retained.
    pub const VERIFY_FILL_RETAINED: &str = "verify_fill_retained";
    /// Wait for a condition.
    pub const OBSERVE: &str = "observe";
    /// Scroll a target into view and report where to click.
    pub const ACTIVATE: &str = "activate";
    /// Scroll a target into view and report where to hover.
    pub const HOVER: &str = "hover";
    /// Focus a target.
    pub const FOCUS: &str = "focus";
    /// Scroll a page point into view.
    pub const SCROLL_POINT: &str = "scroll_point";
    /// Map a page point to the viewport.
    pub const VIEWPORT_POINT: &str = "viewport_point";
    /// Report the scroll offset.
    pub const SCROLL_OFFSET: &str = "scroll_offset";
    /// Report what lies under a point.
    pub const HIT_POINT: &str = "hit_point";
    /// Measure the content box of the owner element.
    pub const OWNER_BOX: &str = "owner_box";
    /// Check whether the owner element lies under a point.
    pub const OWNER_PROBE: &str = "owner_probe";
    /// Report whether the page holds focus.
    pub const FOCUS_PROBE: &str = "focus_probe";
    /// Require that the page holds focus.
    pub const REQUIRE_FOCUSED: &str = "require_focused";
    /// Fill one control that takes no keystrokes.
    pub const FILL_LOCAL: &str = "fill_local";
    /// Focus a text control for browser typing.
    pub const FOCUS_TEXT_FIELD: &str = "focus_text_field";
    /// Click the submit control of a validated fill.
    pub const SUBMIT_FILL: &str = "submit_fill";
    /// Type into a control.
    pub const TYPE_TEXT: &str = "type_text";
    /// Clear the focused control.
    pub const CLEAR_FOCUSED: &str = "clear_focused";
    /// Scroll a target or the page.
    pub const SCROLL: &str = "scroll";
    /// Attach files to a file input.
    pub const UPLOAD_FILES: &str = "upload_files";
    /// Drop files at a point.
    pub const DROP_FILES: &str = "drop_files";
    /// Attach the call's attachment to a file input.
    pub const ATTACH_FILE: &str = "attach_file";
}

/// Which documents may receive a message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeClass {
    /// Reads or changes page content: only documents the authority allowed.
    Content,
    /// Reads geometry or focus only: any document in the admitted tree.
    Metadata,
}

/// What a message can have changed in the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectClass {
    /// Changes nothing.
    Read,
    /// Positions the page for an action without being the action.
    Prepare,
    /// Is the action or part of it.
    Effect,
}

/// One message for a page asset, built only by the constructors in this module.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    kind: &'static str,
    payload: Value,
    scope: ScopeClass,
    effect: EffectClass,
    settles: bool,
}

impl Message {
    fn new(kind: &'static str, scope: ScopeClass, effect: EffectClass, fields: Value) -> Self {
        let mut payload = match fields {
            Value::Object(members) => members,
            _ => Map::new(),
        };
        payload.retain(|_, member| !member.is_null());
        payload.insert(KIND_FIELD.to_owned(), Value::String(kind.to_owned()));
        Self {
            kind,
            payload: Value::Object(payload),
            scope,
            effect,
            settles: false,
        }
    }

    fn settling(mut self) -> Self {
        self.settles = true;
        self
    }

    /// The kind this message names.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    /// The message as sent: the kind and its members.
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    /// The message as the `call` primitive carries it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.payload.clone()
    }

    /// Which documents may receive this message.
    #[must_use]
    pub const fn scope_class(&self) -> ScopeClass {
        self.scope
    }

    /// What this message can have changed.
    #[must_use]
    pub const fn effect_class(&self) -> EffectClass {
        self.effect
    }

    /// Whether this kind receives the scope's remaining preparation budget.
    #[must_use]
    pub const fn takes_settle(&self) -> bool {
        self.settles
    }

    /// Give a settling kind the preparation budget left in milliseconds and the agent's intent.
    /// `requested` is false only when the agent turned settling off; a budget that is spent
    /// (zero) with `requested` true still tells the page the agent wants settling. Any other kind
    /// is returned unchanged.
    #[must_use]
    pub fn with_settle(mut self, settle_ms: u64, requested: bool) -> Self {
        if self.settles {
            if let Value::Object(members) = &mut self.payload {
                members.insert(SETTLE_MS_FIELD.to_owned(), json!(settle_ms));
                members.insert(VISUAL_SETTLE_FIELD.to_owned(), json!(requested));
            }
        }
        self
    }
}

/// Read page text, from one target or the whole document. `mode` is `article` or `visible`.
#[must_use]
pub fn read_text(locator: Option<&str>, mode: Option<&str>, max_chars: usize) -> Message {
    Message::new(
        kind::READ_TEXT,
        Content,
        Read,
        json!({"locator": locator, "mode": mode, "max_chars": max_chars}),
    )
    .settling()
}

/// List controls or structure of one inspection kind.
#[must_use]
pub fn inspect(inspect_kind: &str, max_items: usize) -> Message {
    Message::new(
        kind::INSPECT,
        Content,
        Read,
        json!({"inspect_kind": inspect_kind, "max_items": max_items}),
    )
    .settling()
}

/// Read the structure tree below a target, or below the document body.
#[must_use]
pub fn inspect_tree(locator: Option<&str>, max_depth: usize, max_nodes: usize) -> Message {
    Message::new(
        kind::INSPECT_TREE,
        Content,
        Read,
        json!({"locator": locator, "max_depth": max_depth, "max_nodes": max_nodes}),
    )
    .settling()
}

/// Find targets by visible text.
#[must_use]
pub fn find(text: &str, find_kind: &str, max_results: usize) -> Message {
    Message::new(
        kind::FIND,
        Content,
        Read,
        json!({"text": text, "find_kind": find_kind, "max_results": max_results}),
    )
    .settling()
}

/// Find targets by accessible name and optional role.
#[must_use]
pub fn query_semantic(name: &str, role: Option<&str>, exact: bool, form_scope: bool) -> Message {
    Message::new(
        kind::QUERY_SEMANTIC,
        Content,
        Read,
        json!({"name": name, "role": role, "exact": exact, "form_scope": form_scope}),
    )
    .settling()
}

/// Describe targets by their local locators.
#[must_use]
pub fn describe(locators: &[String]) -> Message {
    Message::new(kind::DESCRIBE, Content, Read, json!({"locators": locators}))
}

/// Describe the focused control.
#[must_use]
pub fn describe_focused() -> Message {
    Message::new(kind::DESCRIBE_FOCUSED, Content, Read, json!({}))
}

/// Check that the focused control takes text.
#[must_use]
pub fn verify_focused_text(allow_credentials: bool) -> Message {
    Message::new(
        kind::VERIFY_FOCUSED_TEXT,
        Content,
        Read,
        json!({"allow_credentials": allow_credentials}),
    )
}

/// Measure a target without moving the page.
#[must_use]
pub fn box_(locator: &str) -> Message {
    Message::new(kind::BOX, Content, Read, json!({"locator": locator}))
}

/// Validate every field and the submit control, and wait for the form to be stable, before any
/// field is edited. The wait draws on the request's shared settle budget.
#[must_use]
pub fn prepare_fill(
    fields: &[PhysicalField],
    allow_credentials: bool,
    submit_locator: Option<&str>,
) -> Message {
    Message::new(
        kind::PREPARE_FILL,
        Content,
        Read,
        json!({
            "fields": fields,
            "allow_credentials": allow_credentials,
            "submit_locator": submit_locator,
        }),
    )
    .settling()
}

/// Wait until every filled value has kept its value for the stable interval, or fail with
/// `not_retained` at `limit_ms`.
#[must_use]
pub fn verify_fill_retained(
    fields: &[PhysicalField],
    allow_credentials: bool,
    limit_ms: u64,
) -> Message {
    Message::new(
        kind::VERIFY_FILL_RETAINED,
        Content,
        Read,
        json!({
            "fields": fields,
            "allow_credentials": allow_credentials,
            "stable_ms": FILL_STABLE_MS,
            "poll_ms": FILL_POLL_MS,
            "limit_ms": limit_ms,
        }),
    )
}

/// Wait for a condition. With `until_satisfied`, a condition still unmet at the timeout fails
/// with `unsatisfied`, so a race between documents can tell a loser from a winner.
#[must_use]
pub fn observe(
    condition: &str,
    value: Option<&str>,
    locator: Option<&str>,
    timeout_ms: u64,
    until_satisfied: bool,
) -> Message {
    Message::new(
        kind::OBSERVE,
        Content,
        Read,
        json!({
            "condition": condition,
            "value": value,
            "locator": locator,
            "timeout_ms": timeout_ms,
            "until_satisfied": until_satisfied,
        }),
    )
}

/// Scroll a target into view and report its rectangle and subject, ready for a click.
#[must_use]
pub fn activate(locator: &str) -> Message {
    Message::new(
        kind::ACTIVATE,
        Content,
        Prepare,
        json!({"locator": locator}),
    )
}

/// Scroll a target into view and report its rectangle and subject, ready for a hover.
#[must_use]
pub fn hover(locator: &str) -> Message {
    Message::new(kind::HOVER, Content, Prepare, json!({"locator": locator}))
}

/// Scroll a target into view and focus it.
#[must_use]
pub fn focus(locator: &str) -> Message {
    Message::new(kind::FOCUS, Content, Prepare, json!({"locator": locator}))
}

/// Scroll a page point into view and report it in viewport coordinates.
#[must_use]
pub fn scroll_point(x: f64, y: f64) -> Message {
    Message::new(
        kind::SCROLL_POINT,
        Content,
        Prepare,
        json!({"x": coordinate(x), "y": coordinate(y)}),
    )
}

/// Map a page point to the viewport, failing with `outside_viewport` when it is not visible.
#[must_use]
pub fn viewport_point(x: f64, y: f64) -> Message {
    Message::new(
        kind::VIEWPORT_POINT,
        Metadata,
        Read,
        json!({"x": coordinate(x), "y": coordinate(y)}),
    )
}

/// Report the document's scroll offset.
#[must_use]
pub fn scroll_offset() -> Message {
    Message::new(kind::SCROLL_OFFSET, Metadata, Read, json!({}))
}

/// Report what lies under a point of the viewport, or of the page when `page` is set.
#[must_use]
pub fn hit_point(x: f64, y: f64, page: bool) -> Message {
    Message::new(
        kind::HIT_POINT,
        Metadata,
        Read,
        json!({"x": coordinate(x), "y": coordinate(y), "page": page}),
    )
}

/// Measure the content box of the owner element. The call names the child frame it owns.
#[must_use]
pub fn owner_box() -> Message {
    Message::new(kind::OWNER_BOX, Metadata, Read, json!({}))
}

/// Say whether the owner element lies under a point, and where its content box starts.
#[must_use]
pub fn owner_probe(x: f64, y: f64) -> Message {
    Message::new(
        kind::OWNER_PROBE,
        Metadata,
        Read,
        json!({"x": coordinate(x), "y": coordinate(y)}),
    )
}

/// Say whether this document holds the page's focus.
#[must_use]
pub fn focus_probe() -> Message {
    Message::new(kind::FOCUS_PROBE, Metadata, Read, json!({}))
}

/// Fail with `focus_lost` unless this document holds the page's focus on a non-frame element.
#[must_use]
pub fn require_focused() -> Message {
    Message::new(kind::REQUIRE_FOCUSED, Metadata, Read, json!({}))
}

/// Fill one control that takes no keystrokes.
#[must_use]
pub fn fill_local(field: &PhysicalField, allow_credentials: bool) -> Message {
    Message::new(
        kind::FILL_LOCAL,
        Content,
        Effect,
        json!({"field": field, "allow_credentials": allow_credentials}),
    )
}

/// Focus a text control so the browser can type its value.
#[must_use]
pub fn focus_text_field(field: &PhysicalField, allow_credentials: bool) -> Message {
    Message::new(
        kind::FOCUS_TEXT_FIELD,
        Content,
        Effect,
        json!({"field": field, "allow_credentials": allow_credentials}),
    )
    .settling()
}

/// Click the submit control after the fields were filled. The click runs after the answer, so a
/// dialog it opens cannot block the call.
#[must_use]
pub fn submit_fill(
    fields: &[PhysicalField],
    submit_locator: &str,
    allow_credentials: bool,
) -> Message {
    Message::new(
        kind::SUBMIT_FILL,
        Content,
        Effect,
        json!({
            "fields": fields,
            "submit_locator": submit_locator,
            "allow_credentials": allow_credentials,
        }),
    )
}

/// Type into a control, clearing it first when asked.
#[must_use]
pub fn type_text(locator: &str, text: &str, clear_first: bool, allow_credentials: bool) -> Message {
    Message::new(
        kind::TYPE_TEXT,
        Content,
        Effect,
        json!({
            "locator": locator,
            "text": text,
            "clear_first": clear_first,
            "allow_credentials": allow_credentials,
        }),
    )
}

/// Clear the focused control.
#[must_use]
pub fn clear_focused(allow_credentials: bool) -> Message {
    Message::new(
        kind::CLEAR_FOCUSED,
        Content,
        Effect,
        json!({"allow_credentials": allow_credentials}),
    )
}

/// Scroll a target into view, or the page in a direction by an amount.
#[must_use]
pub fn scroll(locator: Option<&str>, direction: Option<&str>, amount: Option<&str>) -> Message {
    Message::new(
        kind::SCROLL,
        Content,
        Effect,
        json!({"locator": locator, "direction": direction, "amount": amount}),
    )
}

/// Attach files to a file input.
#[must_use]
pub fn upload_files(locator: &str, files: &[PhysicalFile]) -> Message {
    Message::new(
        kind::UPLOAD_FILES,
        Content,
        Effect,
        json!({"locator": locator, "files": files}),
    )
}

/// Drop files at a viewport point.
#[must_use]
pub fn drop_files(x: f64, y: f64, files: &[PhysicalFile]) -> Message {
    Message::new(
        kind::DROP_FILES,
        Content,
        Effect,
        json!({"x": coordinate(x), "y": coordinate(y), "files": files}),
    )
}

/// Attach the call's attachment to a file input under a file name.
#[must_use]
pub fn attach_file(locator: &str, file_name: &str) -> Message {
    Message::new(
        kind::ATTACH_FILE,
        Content,
        Effect,
        json!({"locator": locator, "file_name": file_name}),
    )
}

// ---- failures ----

/// Closed code of a failure a page asset reports. A code this service does not know decodes as
/// [`AssetErrorCode::PageException`], the code that assumes the page may have changed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetErrorCode {
    /// The locator names no live element of this world.
    StaleLocator,
    /// The target is not visible.
    NotVisible,
    /// The target is disabled.
    Disabled,
    /// The target is read-only.
    ReadOnly,
    /// The target does not take text.
    NotEditable,
    /// The target cannot be filled.
    NotFillable,
    /// The select has no such option.
    OptionMissing,
    /// A credential-class target needs the user's authorization.
    CredentialAuthorizationRequired,
    /// The submit control lies outside the form.
    SubmitOutsideForm,
    /// The target is not a file input.
    NotFileInput,
    /// No control holds the focus.
    NoFocusedControl,
    /// The target lost the focus.
    FocusLost,
    /// The point lies outside the viewport.
    OutsideViewport,
    /// Nothing lies under the point.
    NoSubject,
    /// The asset does not know the message kind.
    UnknownKind,
    /// The message failed validation.
    InvalidRequest,
    /// The asset is not installed in this world. The extension's call shim reports it.
    RuntimeMissing,
    /// A cancel token interrupted the call.
    Cancelled,
    /// A condition stayed unmet until its timeout.
    Unsatisfied,
    /// A filled value did not stay.
    NotRetained,
    /// A file payload disagrees with its declared size or encoding.
    PayloadMismatch,
    /// The page threw, or the code is unknown.
    #[serde(other)]
    PageException,
}

impl AssetErrorCode {
    /// Whether a failure with this code proves the call changed nothing. An effect kind raises
    /// these only before its first mutation. `cancelled`, `not_retained` and `page_exception` can
    /// follow a mutation.
    #[must_use]
    pub const fn is_pre_effect(self) -> bool {
        !matches!(
            self,
            Self::Cancelled | Self::NotRetained | Self::PageException
        )
    }
}

/// A failure a page asset reported.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct AssetError {
    /// The closed failure code.
    pub code: AssetErrorCode,
    /// The asset's bounded description, which may reach the model.
    pub message: String,
    /// The fields a refused fill could not take, when the asset reported them and this build
    /// knows every word it used. An unknown word leaves the failure as its code and message.
    pub detail: Option<FieldProblems>,
}

impl AssetError {
    fn malformed() -> Self {
        Self {
            code: AssetErrorCode::PageException,
            message: MALFORMED_ENVELOPE.to_owned(),
            detail: None,
        }
    }
}

/// Decode one envelope: `{ok:true,value}` or `{ok:false,error:{code,message,detail?}}`.
///
/// # Errors
///
/// Returns the asset's failure. Anything that is not an envelope is a `page_exception`, the code
/// that assumes the page may have changed.
pub fn decode_envelope(envelope: &Value) -> Result<Value, AssetError> {
    match envelope.get(OK_FIELD).and_then(Value::as_bool) {
        Some(true) => Ok(envelope.get(VALUE_FIELD).cloned().unwrap_or(Value::Null)),
        Some(false) => {
            let error = envelope.get(ERROR_FIELD);
            let code = error
                .and_then(|error| error.get(CODE_FIELD))
                .and_then(Value::as_str)
                .map_or(AssetErrorCode::PageException, |code| {
                    serde_json::from_value(Value::String(code.to_owned()))
                        .unwrap_or(AssetErrorCode::PageException)
                });
            let message = error
                .and_then(|error| error.get(MESSAGE_FIELD))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .chars()
                .take(MESSAGE_MAX_CHARS)
                .collect();
            let detail = error
                .and_then(|error| error.get(DETAIL_FIELD))
                .and_then(FieldProblems::decode);
            Err(AssetError {
                code,
                message,
                detail,
            })
        }
        None => Err(AssetError::malformed()),
    }
}

/// Decode the envelope of one call answer. Only an answer the asset gave has one: a call that
/// failed in the extension or never ran answers `None`.
#[must_use]
pub fn decode_answer(answer: &CallAnswer) -> Option<Result<Value, AssetError>> {
    match answer {
        CallAnswer::Returned { envelope } => Some(decode_envelope(envelope)),
        CallAnswer::Failed { .. } | CallAnswer::Skipped | CallAnswer::Unknown => None,
    }
}

// ---- typed values ----

/// A rectangle in a document's own viewport CSS pixels, as `getBoundingClientRect` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub left: f64,
    /// Top edge.
    pub top: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// The middle of the rectangle.
    #[must_use]
    pub fn center(&self) -> Point {
        Point {
            x: self.left + self.width / 2.0,
            y: self.top + self.height / 2.0,
        }
    }
}

/// The content box of an owner element: where a child document's viewport starts inside its
/// parent's, and its size.
pub type OwnerBox = Rect;

/// A target brought into view: where it is and what it is.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Prepared {
    /// The target's rectangle in its document's viewport.
    pub rectangle: Rect,
    /// The role and name the action will use.
    pub subject: PhysicalActionSubject,
}

/// Bounded page text.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Text {
    /// The text.
    pub text: String,
    /// Whether the text was cut at its bound.
    #[serde(default)]
    pub truncated: bool,
    /// The document's title.
    #[serde(default)]
    pub title: String,
    /// The document's URL.
    #[serde(default)]
    pub url: String,
}

/// The answer to an article read: text, and whether an article was found at all.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Article {
    /// The article text, or empty when none was found.
    pub text: String,
    /// Whether the text was cut at its bound.
    #[serde(default)]
    pub truncated: bool,
    /// Whether the document holds an article.
    pub article_found: bool,
    /// The document's title.
    #[serde(default)]
    pub title: String,
    /// The document's URL.
    #[serde(default)]
    pub url: String,
}

impl Article {
    /// The article as the text outcome of a tab.
    #[must_use]
    pub fn into_text(self, tab_id: u64) -> BrowserOutcome {
        BrowserOutcome::Text {
            tab_id,
            text: self.text,
            truncated: self.truncated,
            title: self.title,
            url: self.url,
        }
    }
}

/// Targets one document observed.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Targets {
    /// The targets, each with a document-local locator.
    pub targets: Vec<ObservedTarget>,
    /// Whether more targets existed than were returned.
    #[serde(default)]
    pub truncated: bool,
}

/// One node of a structure tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    /// `control`, `heading` or `container`.
    pub kind: String,
    /// The node's bounded label.
    pub label: String,
    /// The node's children.
    #[serde(default)]
    pub children: Vec<TreeNode>,
}

/// A structure tree one document built.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TreeValue {
    /// The root.
    pub tree: TreeNode,
    /// How many nodes the tree holds.
    pub nodes: usize,
    /// Whether the node budget cut the tree.
    #[serde(default)]
    pub truncated: bool,
}

/// How one field of a fill takes its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// The page sets the value itself: selects, checkboxes, radios and other controls that take
    /// no keystrokes.
    Local,
    /// The browser types the value into the focused control as trusted input.
    BrowserText,
}

/// The answer to preparing a fill: how each field, in order, takes its value.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct FillPlan {
    /// One kind per field.
    pub field_kinds: Vec<FieldKind>,
}

/// The answer to a submit.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub struct Submitted {
    /// Whether the submit click was scheduled.
    pub submitted: bool,
}

/// Where a scroll left a document, and the target it revealed.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Scrolled {
    /// The document's horizontal scroll offset.
    pub x: f64,
    /// The document's vertical scroll offset.
    pub y: f64,
    /// The role and name of the target brought into view, when the scroll named one.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

/// A document's scroll offset.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct ScrollOffset {
    /// The horizontal offset.
    pub x: f64,
    /// The vertical offset.
    pub y: f64,
}

/// The answer to typing into a control.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Typed {
    /// The role and name of the control that took the text.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

/// The answer to focusing a control, or to requiring that one holds the focus.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Focused {
    /// The role and name of the focused control, when the call named one.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

/// The answer to attaching files to a control.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Uploaded {
    /// How many files the control now holds.
    pub uploaded_count: usize,
    /// How many bytes those files hold.
    pub uploaded_bytes: u64,
    /// The role and name of the control that took the files.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

/// A page point placed in the top viewport.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ViewportPoint {
    /// The point's horizontal position in the top document's viewport.
    pub x: f64,
    /// The point's vertical position in the top document's viewport.
    pub y: f64,
    /// The role and name of the element the top document finds under the point.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

impl ViewportPoint {
    /// Where input must go to reach the point.
    #[must_use]
    pub const fn at(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

/// What lies under a point.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Hit {
    /// The point's horizontal position in this document's viewport.
    pub x: f64,
    /// The point's vertical position in this document's viewport.
    pub y: f64,
    /// Whether an embedded document lies under the point, so the caller must descend.
    pub embed: bool,
    /// The role and name of the element, absent for an embed.
    #[serde(default)]
    pub subject: Option<PhysicalActionSubject>,
}

/// The answer to an owner probe.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct OwnerProbe {
    /// Whether the owner element is the deepest element under the point.
    pub hit: bool,
    /// Where the owner's content box starts, horizontally.
    pub left: f64,
    /// Where the owner's content box starts, vertically.
    pub top: f64,
}

/// The answer to a focus probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub struct FocusProbe {
    /// Whether this document holds the page's focus.
    pub focused: bool,
}

/// The answer to an observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
pub struct Observed {
    /// Whether the condition held.
    pub satisfied: bool,
    /// How long the asset waited.
    pub elapsed_ms: u64,
    /// How far the document had loaded when the wait ended.
    pub readiness: Readiness,
}

#[cfg(test)]
mod tests;
