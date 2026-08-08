// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral browser operations and canonical browser results.
//!
//! Surface profiles translate model-facing calls into these semantic identifiers before work
//! crosses the owner bridge. Browser mechanisms remain a separate, policy-free vocabulary below
//! the service operation pipeline.

use crate::workspace_id::WorkspaceId;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

macro_rules! stable_string_enum {
    (
        $(#[$enum_meta:meta])*
        pub enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// Every value in stable wire order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Return the stable wire spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            /// Parse one exact stable wire spelling.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown {} wire value: {value}",
                        stringify!($name)
                    ))
                })
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_string_enum! {
    /// Closed semantic browser-operation families shared by both bridge shores.
    ///
    /// The first twelve values are the native core. The browser-prefixed remainder are fixed
    /// capability pack families. One compatibility-only workflow family preserves the current
    /// client-side plan echo without pretending it is a browser mechanism. Availability is
    /// projected separately, so a reserved family does not claim that it is implemented.
    pub enum OperationId {
        /// Browser health, limits, and non-sensitive context.
        BrowserContext => "browser.context",
        /// Owned tab topology and selection.
        BrowserTabs => "browser.tabs",
        /// URL and history navigation.
        BrowserNavigate => "browser.navigate",
        /// Bounded structured page observation.
        BrowserSnapshot => "browser.snapshot",
        /// Bounded readable page text.
        BrowserRead => "browser.read",
        /// Targeted page search.
        BrowserFind => "browser.find",
        /// Visual page capture.
        BrowserScreenshot => "browser.screenshot",
        /// Target-bound semantic interaction.
        BrowserAct => "browser.act",
        /// Bounded multi-field form interaction.
        BrowserFill => "browser.fill",
        /// Delay, condition, and settlement observation.
        BrowserWait => "browser.wait",
        /// Canonical operation composition.
        BrowserFlow => "browser.flow",
        /// Blocking browser-dialog inspection and resolution.
        BrowserDialog => "browser.dialog",
        /// Screenshot-frame-bound precision input.
        BrowserInput => "browser.input",
        /// Browser-wide viewport control.
        BrowserViewport => "browser.viewport",
        /// Governed inbound files and captured artifacts.
        BrowserUpload => "browser.upload",
        /// Governed page-triggered download acquisition.
        BrowserDownload => "browser.download",
        /// Governed content export.
        BrowserExport => "browser.export",
        /// Bounded artifact lifecycle management.
        BrowserArtifacts => "browser.artifacts",
        /// Bounded console diagnostics.
        BrowserConsole => "browser.console",
        /// Bounded network diagnostics.
        BrowserNetwork => "browser.network",
        /// Explicit page-context execution.
        BrowserEvaluate => "browser.evaluate",
        /// In-memory browser recording.
        BrowserRecord => "browser.record",
        /// Model-authored human-visible narration and highlighting.
        BrowserPresent => "browser.present",
        /// Deliberate browser visibility control.
        BrowserVisibility => "browser.visibility",
        /// Multi-browser discovery and selection.
        BrowserInstances => "browser.instances",
        /// Compatibility-only client workflow state with no browser mechanism.
        WorkflowPlan => "workflow.plan"
    }
}

stable_string_enum! {
    /// Closed concrete semantic intents carried alongside an [`OperationId`].
    ///
    /// Intent ids deduplicate surface aliases while preserving distinctions needed by validation,
    /// capability classification, scheduling, result meaning, and audit. The service registry
    /// validates which intent belongs to which operation family.
    pub enum IntentId {
        /// Describe current browser context.
        ContextDescribe => "context.describe",
        /// List owned tabs.
        TabsList => "tabs.list",
        /// Create a new blank owned tab.
        TabsNew => "tabs.new",
        /// Select an owned tab.
        TabsFocus => "tabs.focus",
        /// Close an owned tab.
        TabsClose => "tabs.close",
        /// Navigate to an explicit URL.
        NavigateUrl => "navigate.url",
        /// Traverse backward in tab history.
        NavigateBack => "navigate.back",
        /// Traverse forward in tab history.
        NavigateForward => "navigate.forward",
        /// Reload the current document.
        NavigateReload => "navigate.reload",
        /// Capture a bounded structured page snapshot.
        SnapshotCapture => "snapshot.capture",
        /// Read bounded page text.
        ReadText => "read.text",
        /// Find ranked page targets.
        FindQuery => "find.query",
        /// Capture the current viewport.
        ScreenshotViewport => "screenshot.viewport",
        /// Capture a bounded page region.
        ScreenshotRegion => "screenshot.region",
        /// Click one semantic target.
        ActClick => "act.click",
        /// Double-click one semantic target.
        ActDoubleClick => "act.double_click",
        /// Right-click one semantic target.
        ActRightClick => "act.right_click",
        /// Triple-click one semantic target for legacy parity.
        ActTripleClick => "act.triple_click",
        /// Hover one semantic target.
        ActHover => "act.hover",
        /// Scroll one semantic target into view.
        ActScrollIntoView => "act.scroll_into_view",
        /// Focus one semantic target.
        ActFocus => "act.focus",
        /// Set the value of one semantic target.
        ActSetValue => "act.set_value",
        /// Press a key against one semantic target.
        ActPressKey => "act.press_key",
        /// Drag one semantic target to a destination.
        ActDrag => "act.drag",
        /// Set one compatibility field without target discovery.
        FillField => "fill.field",
        /// Fill a bounded set of fields without submitting.
        FillFields => "fill.fields",
        /// Fill a bounded set of fields and submit its form.
        FillFieldsAndSubmit => "fill.fields_and_submit",
        /// Wait for a fixed bounded duration.
        WaitDelay => "wait.delay",
        /// Wait for a bounded condition and optional settlement.
        WaitUntil => "wait.until",
        /// Execute a bounded canonical flow.
        FlowExecute => "flow.execute",
        /// Preflight a bounded canonical flow without requested effects.
        FlowPreflight => "flow.preflight",
        /// Inspect the current blocking dialog.
        DialogStatus => "dialog.status",
        /// Accept the current blocking dialog.
        DialogAccept => "dialog.accept",
        /// Dismiss the current blocking dialog.
        DialogDismiss => "dialog.dismiss",
        /// Respond with text to the current blocking dialog.
        DialogRespond => "dialog.respond",
        /// Click a screenshot-frame coordinate.
        InputPointerClick => "input.pointer.click",
        /// Right-click a screenshot-frame coordinate.
        InputPointerRightClick => "input.pointer.right_click",
        /// Double-click a screenshot-frame coordinate.
        InputPointerDoubleClick => "input.pointer.double_click",
        /// Triple-click a screenshot-frame coordinate for legacy parity.
        InputPointerTripleClick => "input.pointer.triple_click",
        /// Hover a screenshot-frame coordinate.
        InputPointerHover => "input.pointer.hover",
        /// Drag between screenshot-frame coordinates.
        InputPointerDrag => "input.pointer.drag",
        /// Type text into the currently focused target.
        InputTypeText => "input.text.type",
        /// Press a key against the currently focused target.
        InputPressKey => "input.key.press",
        /// Dispatch a bounded wheel delta.
        InputWheel => "input.scroll.wheel",
        /// Scroll to a coordinate offset for legacy parity.
        InputScrollToOffset => "input.scroll.to_offset",
        /// Resize the browser viewport.
        ViewportResizeWindow => "viewport.resize_window",
        /// Upload client-supplied file bytes.
        UploadClientFiles => "upload.client_files",
        /// Upload a previously captured browser artifact.
        UploadCapturedArtifact => "upload.captured_artifact",
        /// Read bounded console diagnostics.
        ConsoleRead => "console.read",
        /// Read and clear bounded console diagnostics.
        ConsoleReadAndClear => "console.read_and_clear",
        /// Read bounded network diagnostics.
        NetworkRead => "network.read",
        /// Read and clear bounded network diagnostics.
        NetworkReadAndClear => "network.read_and_clear",
        /// Evaluate explicit page-context JavaScript.
        EvaluateJavascript => "evaluate.javascript",
        /// Begin an in-memory browser recording.
        RecordStart => "record.start",
        /// Stop an in-memory browser recording.
        RecordStop => "record.stop",
        /// Inspect in-memory recording state.
        RecordStatus => "record.status",
        /// Clear in-memory recording state.
        RecordClear => "record.clear",
        /// Export an in-memory browser recording.
        RecordExport => "record.export",
        /// Narrate a human-visible presentation update.
        PresentNarrate => "present.narrate",
        /// Echo a compatibility client workflow-plan update.
        PlanUpdate => "plan.update"
    }
}

/// One closed operation-family and concrete-intent pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationKey {
    /// Canonical operation family.
    pub id: OperationId,
    /// Concrete semantic intent.
    pub intent: IntentId,
}

impl OperationKey {
    /// Construct an operation key without applying registry-owned family validation.
    pub const fn new(id: OperationId, intent: IntentId) -> Self {
        Self { id, intent }
    }
}

/// One protocol-neutral call accepted by the service operation registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserOperation {
    /// Canonical operation family.
    pub id: OperationId,
    /// Concrete semantic intent.
    pub intent: IntentId,
    /// Canonical arguments validated by the service registry before admission.
    pub arguments: Value,
}

impl BrowserOperation {
    /// Construct one canonical browser operation.
    pub const fn new(id: OperationId, intent: IntentId, arguments: Value) -> Self {
        Self {
            id,
            intent,
            arguments,
        }
    }

    /// Return this operation's closed semantic key.
    pub const fn key(&self) -> OperationKey {
        OperationKey::new(self.id, self.intent)
    }
}

/// Maximum UTF-8 byte length of one invocation-presentation field.
pub const MAX_INVOCATION_PRESENTATION_FIELD_BYTES: usize = 128;

/// Validation failure for bounded invocation-presentation metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvocationPresentationError {
    /// Profile versions are positive integers.
    #[error("profile version must be positive")]
    InvalidProfileVersion,
    /// One named field was empty, too long, or contained a control character.
    #[error("{field} must be non-empty, control-free, and at most 128 UTF-8 bytes")]
    InvalidField {
        /// Invalid presentation field.
        field: &'static str,
    },
}

/// Bounded edge-authored facts retained only for corrective copy and audit presentation.
///
/// These fields are never an operation lookup key, routing handle, policy input, or authority
/// claim. The canonical [`BrowserOperation`] alone drives service behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvocationPresentation {
    profile_id: String,
    profile_version: u32,
    external_tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    external_action: Option<String>,
}

impl InvocationPresentation {
    /// Validate and construct bounded invocation-presentation metadata.
    pub fn new(
        profile_id: impl Into<String>,
        profile_version: u32,
        external_tool: impl Into<String>,
        external_action: Option<String>,
    ) -> std::result::Result<Self, InvocationPresentationError> {
        if profile_version == 0 {
            return Err(InvocationPresentationError::InvalidProfileVersion);
        }

        let profile_id = profile_id.into();
        validate_presentation_field("profile_id", &profile_id)?;
        let external_tool = external_tool.into();
        validate_presentation_field("external_tool", &external_tool)?;
        if let Some(action) = external_action.as_deref() {
            validate_presentation_field("external_action", action)?;
        }

        Ok(Self {
            profile_id,
            profile_version,
            external_tool,
            external_action,
        })
    }

    /// Return the selected surface profile id.
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// Return the positive selected surface profile version.
    pub const fn profile_version(&self) -> u32 {
        self.profile_version
    }

    /// Return the external tool name used for this invocation.
    pub fn external_tool(&self) -> &str {
        &self.external_tool
    }

    /// Return the optional external action spelling used for this invocation.
    pub fn external_action(&self) -> Option<&str> {
        self.external_action.as_deref()
    }
}

impl<'de> Deserialize<'de> for InvocationPresentation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WirePresentation {
            profile_id: String,
            profile_version: u32,
            external_tool: String,
            #[serde(default)]
            external_action: Option<String>,
        }

        let value = WirePresentation::deserialize(deserializer)?;
        Self::new(
            value.profile_id,
            value.profile_version,
            value.external_tool,
            value.external_action,
        )
        .map_err(serde::de::Error::custom)
    }
}

fn validate_presentation_field(
    field: &'static str,
    value: &str,
) -> std::result::Result<(), InvocationPresentationError> {
    if value.is_empty()
        || value.len() > MAX_INVOCATION_PRESENTATION_FIELD_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(InvocationPresentationError::InvalidField { field });
    }
    Ok(())
}

/// Maximum UTF-8 byte length accepted for an opaque tab handle.
pub const MAX_TAB_HANDLE_BYTES: usize = 256;

/// Opaque, service-issued proof that a tab belongs to a workspace.
///
/// A tab handle is verification-only. It is never authority without the corresponding
/// [`WorkspaceId`]. Debug and display output are redacted so native identity cannot leak into
/// logs by accident.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TabHandle(String);

impl TabHandle {
    /// Parse a non-empty, bounded, control-free opaque tab handle.
    pub fn parse(value: &str) -> Option<Self> {
        if value.is_empty()
            || value.len() > MAX_TAB_HANDLE_BYTES
            || value.chars().any(char::is_control)
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    /// Return the raw handle for serialization or exact service-side verification.
    ///
    /// Never use this value as workspace authority or write it to a log, error, or metric label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for TabHandle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TabHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).ok_or_else(|| serde::de::Error::custom("invalid opaque tab handle"))
    }
}

impl std::fmt::Display for TabHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<redacted-tab-handle>")
    }
}

impl std::fmt::Debug for TabHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TabHandle(<redacted>)")
    }
}

stable_string_enum! {
    /// Version marker for the canonical browser-result envelope.
    pub enum BrowserResultSchema {
        /// Initial canonical browser-result vocabulary.
        V1 => "ghostlight.browser.result/1"
    }
}

stable_string_enum! {
    /// Canonical terminal status of one browser operation.
    pub enum BrowserResultStatus {
        /// The browser mechanism completed as defined.
        Ok => "ok",
        /// An acknowledged effect completed but a requested follow-up fact did not.
        Partial => "partial",
        /// A requested observation was not met.
        NotMet => "not_met",
        /// Policy, target validity, or another precondition blocked completion.
        Blocked => "blocked",
        /// Human-control hold prevented dispatch.
        Held => "held",
        /// The workspace denial circuit requires user attention.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation retired the work.
        Cancelled => "cancelled",
        /// Admission failed before browser dispatch.
        NotDispatched => "not_dispatched",
        /// Dispatch may have occurred without conclusive terminal proof.
        OutcomeUnknown => "outcome_unknown",
        /// A declared operation was temporarily unavailable.
        Unavailable => "unavailable"
    }
}

stable_string_enum! {
    /// Proven physical-effect disposition for one operation.
    pub enum OperationEffect {
        /// The requested effect was proven not to have been dispatched.
        None => "none",
        /// Dispatch was accepted but no terminal acknowledgement was received.
        Dispatched => "dispatched",
        /// The defined browser acknowledgement was received.
        Committed => "committed",
        /// Available evidence cannot determine whether the effect ran.
        Unknown => "unknown"
    }
}

stable_string_enum! {
    /// Corrective retry guidance derived from the proven effect disposition.
    pub enum RetryDisposition {
        /// Repeating the operation cannot duplicate a dispatched effect.
        Safe => "safe",
        /// Repeating the operation may duplicate an effect.
        Unsafe => "unsafe",
        /// Refresh state before deciding whether to retry.
        AfterStateChange => "after_state_change"
    }
}

stable_string_enum! {
    /// Aggregate state of a requested readiness observation.
    pub enum ReadinessStatus {
        /// Every requested readiness axis completed.
        Ready => "ready",
        /// The readiness deadline expired.
        TimedOut => "timed_out",
        /// The current document could not provide readiness evidence.
        Unavailable => "unavailable",
        /// No readiness observation was requested.
        NotRequested => "not_requested"
    }
}

stable_string_enum! {
    /// State of the settlement readiness axis.
    pub enum SettlementStatus {
        /// The document met the settlement predicate.
        Settled => "settled",
        /// The document did not settle before the deadline.
        NotSettled => "not_settled",
        /// The document could not provide settlement evidence.
        Unavailable => "unavailable"
    }
}

/// Condition-axis readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessCondition {
    /// Whether the operation requested a condition observation.
    pub requested: bool,
    /// Whether the requested condition was met.
    pub met: bool,
}

/// Settlement-axis readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessSettlement {
    /// Whether the operation requested settlement observation.
    pub requested: bool,
    /// Proven settlement disposition.
    pub status: SettlementStatus,
}

/// Canonical readiness evidence, separate from operation success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    /// Aggregate readiness disposition.
    pub status: ReadinessStatus,
    /// Condition-axis evidence, omitted when that axis was not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<ReadinessCondition>,
    /// Settlement-axis evidence, omitted when that axis was not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<ReadinessSettlement>,
    /// Time spent on requested readiness observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

/// Presentation-only corrective guidance attached to a canonical result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryHint {
    /// Human-readable corrective action.
    pub message: String,
    /// Optional canonical operation that can refresh relevant state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_operation: Option<OperationKey>,
}

/// One concise, protocol-neutral model-facing result part.
///
/// These are product result parts, not MCP content blocks or vendor result wrappers. GIF output
/// uses the image variant with its actual media type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultPart {
    /// Concise canonical text.
    Text {
        /// Text returned to the selected edge renderer.
        text: String,
    },
    /// Base64-encoded image or image-media output.
    Image {
        /// Base64-encoded bytes without a data-URL prefix.
        data: String,
        /// Validated media type, such as `image/jpeg` or `image/gif`.
        mime_type: String,
    },
}

/// Maximum byte length accepted for one canonical image media type.
pub const MAX_RESULT_IMAGE_MIME_TYPE_BYTES: usize = 128;

/// Validation failure for one canonical result part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResultPartError {
    /// Image data was not non-empty canonical RFC 4648 standard base64.
    #[error("result image data must be non-empty canonical standard base64")]
    InvalidImageData,
    /// The media type was not one bounded, concrete, canonical image type.
    #[error("result image media type must be a bounded concrete lowercase image type")]
    InvalidImageMimeType,
}

impl ResultPart {
    /// Construct one validated canonical image part.
    pub fn image(
        data: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Result<Self, ResultPartError> {
        let data = data.into();
        if !is_canonical_standard_base64(&data) {
            return Err(ResultPartError::InvalidImageData);
        }

        let mime_type = normalize_image_mime_type(&mime_type.into())
            .ok_or(ResultPartError::InvalidImageMimeType)?;
        Ok(Self::Image { data, mime_type })
    }

    /// Validate that this part satisfies the canonical transport invariant.
    pub fn validate(&self) -> Result<(), ResultPartError> {
        let Self::Image { data, mime_type } = self else {
            return Ok(());
        };
        if !is_canonical_standard_base64(data) {
            return Err(ResultPartError::InvalidImageData);
        }
        if normalize_image_mime_type(mime_type).as_deref() != Some(mime_type.as_str()) {
            return Err(ResultPartError::InvalidImageMimeType);
        }
        Ok(())
    }
}

impl Serialize for ResultPart {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        match self {
            Self::Text { text } => {
                let mut state = serializer.serialize_struct("ResultPart", 2)?;
                state.serialize_field("type", "text")?;
                state.serialize_field("text", text)?;
                state.end()
            }
            Self::Image { data, mime_type } => {
                let mut state = serializer.serialize_struct("ResultPart", 3)?;
                state.serialize_field("type", "image")?;
                state.serialize_field("data", data)?;
                state.serialize_field("mime_type", mime_type)?;
                state.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ResultPart {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum WireResultPart {
            Text { text: String },
            Image { data: String, mime_type: String },
        }

        match WireResultPart::deserialize(deserializer)? {
            WireResultPart::Text { text } => Ok(Self::Text { text }),
            WireResultPart::Image { data, mime_type } => {
                Self::image(data, mime_type).map_err(serde::de::Error::custom)
            }
        }
    }
}

fn is_canonical_standard_base64(data: &str) -> bool {
    if data.is_empty() {
        return false;
    }

    let bytes = data.as_bytes();
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return false;
    }
    let symbol_count = bytes.len() - padding;
    if symbol_count == 0
        || bytes[..symbol_count]
            .iter()
            .any(|byte| base64_sextet(*byte).is_none())
    {
        return false;
    }

    let remainder = symbol_count % 4;
    let valid_padding = match padding {
        0 => remainder != 1,
        1 => bytes.len().is_multiple_of(4) && remainder == 3,
        2 => bytes.len().is_multiple_of(4) && remainder == 2,
        _ => false,
    };
    if !valid_padding {
        return false;
    }

    let tail = base64_sextet(bytes[symbol_count - 1]).expect("symbols were validated above");
    match remainder {
        2 => tail & 0x0f == 0,
        3 => tail & 0x03 == 0,
        _ => true,
    }
}

fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn normalize_image_mime_type(mime_type: &str) -> Option<String> {
    if mime_type.is_empty()
        || mime_type.len() > MAX_RESULT_IMAGE_MIME_TYPE_BYTES
        || !mime_type.is_ascii()
    {
        return None;
    }
    let (top_level, subtype) = mime_type.split_once('/')?;
    if !top_level.eq_ignore_ascii_case("image") || subtype.is_empty() {
        return None;
    }

    let mut bytes = subtype.bytes();
    if !bytes.next()?.is_ascii_alphanumeric()
        || bytes.any(|byte| {
            !byte.is_ascii_alphanumeric()
                && !matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'&' | b'-' | b'^' | b'_' | b'.' | b'+'
                )
        })
    {
        return None;
    }
    Some(mime_type.to_ascii_lowercase())
}

/// Tab facts returned by the canonical service result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultTab {
    /// Opaque verification-only tab handle.
    pub id: TabHandle,
    /// Best available page-derived URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Best available page-derived title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Maximum UTF-8 byte length accepted for a page origin carried as provenance.
pub const MAX_PAGE_ORIGIN_BYTES: usize = 240;

/// Validation failure for page-derived result provenance.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PageProvenanceError {
    /// A provenance marker is only useful when it names at least one page-derived field.
    #[error("page provenance must name at least one untrusted field")]
    Empty,
    /// The JSON pointer names a service-authored or unknown result field.
    #[error("untrusted field is outside the page-derived result scope: {pointer}")]
    OutOfScope {
        /// Rejected JSON pointer.
        pointer: String,
    },
    /// A frame origin was empty, unbounded, or contained a control character.
    #[error("frame origin must be non-empty, control-free, and at most 240 UTF-8 bytes")]
    InvalidFrameOrigin,
}

/// Scoped provenance for page-derived fields in one canonical result.
///
/// Only page payload under `data`, text/image bytes under `parts`, and page-derived tab URL/title
/// may be named. Service-authored schema, operation, status, effect, retry, recovery, workspace,
/// and handle facts remain trusted by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageProvenance {
    untrusted_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    top_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_nonce: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame_origin: Option<String>,
}

impl PageProvenance {
    /// Validate and construct scoped page-derived provenance.
    pub fn new(
        untrusted_fields: Vec<String>,
        top_origin: Option<String>,
        session_nonce: Option<String>,
        frame_origin: Option<String>,
    ) -> std::result::Result<Self, PageProvenanceError> {
        if untrusted_fields.is_empty() {
            return Err(PageProvenanceError::Empty);
        }
        if let Some(pointer) = untrusted_fields
            .iter()
            .find(|pointer| !is_page_derived_pointer(pointer))
        {
            return Err(PageProvenanceError::OutOfScope {
                pointer: pointer.clone(),
            });
        }
        if frame_origin
            .as_deref()
            .is_some_and(|origin| !is_valid_page_origin(origin))
        {
            return Err(PageProvenanceError::InvalidFrameOrigin);
        }
        Ok(Self {
            untrusted_fields,
            top_origin,
            session_nonce,
            frame_origin,
        })
    }

    /// Return the scoped JSON pointers identifying page-derived result fields.
    pub fn untrusted_fields(&self) -> &[String] {
        &self.untrusted_fields
    }

    /// Return the best available top-level page origin.
    pub fn top_origin(&self) -> Option<&str> {
        self.top_origin.as_deref()
    }

    /// Return the service-authored nonce that delimits one page-content session.
    pub fn session_nonce(&self) -> Option<&str> {
        self.session_nonce.as_deref()
    }

    /// Return the best available origin for the frame that supplied the page payload.
    pub fn frame_origin(&self) -> Option<&str> {
        self.frame_origin.as_deref()
    }
}

impl<'de> Deserialize<'de> for PageProvenance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireProvenance {
            untrusted_fields: Vec<String>,
            #[serde(default)]
            top_origin: Option<String>,
            #[serde(default)]
            session_nonce: Option<String>,
            #[serde(default)]
            frame_origin: Option<String>,
        }

        let value = WireProvenance::deserialize(deserializer)?;
        Self::new(
            value.untrusted_fields,
            value.top_origin,
            value.session_nonce,
            value.frame_origin,
        )
        .map_err(serde::de::Error::custom)
    }
}

fn is_valid_page_origin(origin: &str) -> bool {
    !origin.is_empty()
        && origin.len() <= MAX_PAGE_ORIGIN_BYTES
        && !origin.chars().any(char::is_control)
}

fn is_page_derived_pointer(pointer: &str) -> bool {
    if pointer == "/data" || pointer.starts_with("/data/") {
        return true;
    }
    if matches!(pointer, "/tab/url" | "/tab/title") {
        return true;
    }

    let Some(part_pointer) = pointer.strip_prefix("/parts/") else {
        return false;
    };
    let Some((index, field)) = part_pointer.split_once('/') else {
        return false;
    };
    !index.is_empty()
        && index.bytes().all(|byte| byte.is_ascii_digit())
        && matches!(field, "text" | "data")
}

/// Versioned, protocol-neutral terminal result produced by the operation pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserResult {
    /// Canonical result schema marker.
    pub schema: BrowserResultSchema,
    /// Canonical operation family.
    pub operation: OperationId,
    /// Concrete semantic intent.
    pub intent: IntentId,
    /// Canonical terminal status.
    pub status: BrowserResultStatus,
    /// Proven physical-effect disposition.
    pub effect: OperationEffect,
    /// Corrective retry guidance, omitted when no retry guidance applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryDisposition>,
    /// Presentation-only recovery guidance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryHint>,
    /// Readiness evidence, distinct from operation success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Readiness>,
    /// Workspace used or created by the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceId>,
    /// Bounded tab facts relevant to the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<ResultTab>,
    /// Concise protocol-neutral text and image output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ResultPart>,
    /// Structured canonical result data.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
    /// Scoped page-derived provenance, omitted when the result has no page payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<PageProvenance>,
}

/// A canonical browser result carries an internally inconsistent terminal disposition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BrowserResultValidationError {
    /// Terminal results cannot retain the in-flight dispatched effect.
    #[error("a terminal browser result cannot have effect dispatched")]
    TerminalDispatched,
    /// Outcome-unknown status has one exact effect and retry contract.
    #[error("outcome_unknown requires effect unknown and retry unsafe")]
    InvalidOutcomeUnknown,
    /// Unknown effect cannot accompany another terminal status.
    #[error("effect unknown requires outcome_unknown status")]
    UnknownEffectWithTerminalStatus,
    /// Cancellation retry guidance must match its proven effect.
    #[error("cancelled requires no/safe retry with no effect and unsafe retry otherwise")]
    InvalidCancellation,
    /// A proven pre-dispatch terminal cannot claim a physical effect.
    #[error("held, attention_required, and not_dispatched require effect none")]
    PreDispatchStatusWithEffect,
}

impl BrowserResult {
    /// Construct an empty version-one canonical result envelope.
    pub fn new(
        operation: OperationId,
        intent: IntentId,
        status: BrowserResultStatus,
        effect: OperationEffect,
    ) -> Self {
        Self {
            schema: BrowserResultSchema::V1,
            operation,
            intent,
            status,
            effect,
            retry: None,
            recovery: None,
            readiness: None,
            workspace: None,
            tab: None,
            parts: Vec::new(),
            data: Value::Null,
            provenance: None,
        }
    }

    /// Validate the closed status/effect/retry relationship before edge rendering.
    pub fn validate_semantics(&self) -> Result<(), BrowserResultValidationError> {
        if self.effect == OperationEffect::Dispatched {
            return Err(BrowserResultValidationError::TerminalDispatched);
        }
        if self.status == BrowserResultStatus::OutcomeUnknown {
            if self.effect != OperationEffect::Unknown
                || self.retry != Some(RetryDisposition::Unsafe)
            {
                return Err(BrowserResultValidationError::InvalidOutcomeUnknown);
            }
            return Ok(());
        }
        if self.status == BrowserResultStatus::Cancelled {
            let valid = match self.effect {
                OperationEffect::None => {
                    matches!(self.retry, None | Some(RetryDisposition::Safe))
                }
                OperationEffect::Committed | OperationEffect::Unknown => {
                    self.retry == Some(RetryDisposition::Unsafe)
                }
                OperationEffect::Dispatched => false,
            };
            return valid
                .then_some(())
                .ok_or(BrowserResultValidationError::InvalidCancellation);
        }
        if self.effect == OperationEffect::Unknown {
            return Err(BrowserResultValidationError::UnknownEffectWithTerminalStatus);
        }
        if matches!(
            self.status,
            BrowserResultStatus::Held
                | BrowserResultStatus::AttentionRequired
                | BrowserResultStatus::NotDispatched
        ) && self.effect != OperationEffect::None
        {
            return Err(BrowserResultValidationError::PreDispatchStatusWithEffect);
        }
        Ok(())
    }
}

stable_string_enum! {
    /// Canonical execution state of one step inside a browser flow.
    pub enum FlowStepStatus {
        /// The step completed as defined.
        Ok => "ok",
        /// The step committed an effect but did not complete every requested observation.
        Partial => "partial",
        /// The step's requested observation was not met.
        NotMet => "not_met",
        /// A semantic precondition blocked the step.
        Blocked => "blocked",
        /// A policy decision denied the step before dispatch.
        Denied => "denied",
        /// Human control held the step before dispatch.
        Held => "held",
        /// The workspace denial circuit requires user attention.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation retired the step.
        Cancelled => "cancelled",
        /// Admission failed before browser dispatch.
        NotDispatched => "not_dispatched",
        /// Dispatch may have occurred without conclusive terminal proof.
        OutcomeUnknown => "outcome_unknown",
        /// The operation was temporarily unavailable or returned an invalid result.
        Unavailable => "unavailable",
        /// Flow control prevented this declared step from running.
        NotRun => "not_run",
        /// Preflight proved that the step would be admitted without dispatching it.
        WouldAllow => "would_allow",
        /// Preflight proved that policy would deny the step without dispatching it.
        WouldDeny => "would_deny"
    }
}

/// One ordered canonical step result inside a browser flow.
///
/// The nested [`BrowserResult`] carries canonical operation identity, effect and retry facts,
/// result parts, structured data, and typed provenance. `status` adds flow-control distinctions
/// such as `denied`, `not_run`, and preflight verdicts without introducing a surface tool name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStepResult {
    /// One-based position in the declared flow.
    pub step: u32,
    /// Flow-control disposition for this step.
    pub status: FlowStepStatus,
    /// Canonical result facts for the declared operation.
    pub result: BrowserResult,
}

stable_string_enum! {
    /// Why canonical flow iteration stopped.
    pub enum FlowTerminationReason {
        /// Every declared step was considered, including continue-on-error execution.
        Completed => "completed",
        /// A non-policy step failure stopped iteration.
        Failed => "failed",
        /// A policy denial stopped iteration.
        Denied => "denied",
        /// Human control held iteration.
        Held => "held",
        /// The workspace denial circuit stopped iteration.
        AttentionRequired => "attention_required",
        /// Cooperative cancellation stopped iteration.
        Cancelled => "cancelled",
        /// The bounded flow wall-clock budget stopped iteration.
        BudgetExhausted => "budget_exhausted"
    }
}

/// Typed terminal reason for one canonical flow execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowTermination {
    /// Stable reason independent of human summary text.
    pub reason: FlowTerminationReason,
    /// One-based step at or after which iteration stopped, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
}

/// Canonical structured result of one browser flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowResultData {
    /// Ordered result entry for every declared step, including steps that did not run.
    pub steps: Vec<FlowStepResult>,
    /// Concise aggregate summary of completed and stopped work.
    pub summary: String,
    /// Total wall-clock execution duration.
    pub duration_ms: u64,
    /// Typed aggregate stop reason used for truthful status derivation.
    pub termination: FlowTermination,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn operation_ids_are_closed_unique_dotted_and_serde_stable() {
        assert_eq!(OperationId::ALL.len(), 26);
        let values: BTreeSet<_> = OperationId::ALL.iter().map(|id| id.as_str()).collect();
        assert_eq!(values.len(), OperationId::ALL.len());

        for id in OperationId::ALL {
            let wire = id.as_str();
            assert!(wire.is_ascii());
            assert!(wire.starts_with("browser.") || wire == "workflow.plan");
            assert_eq!(OperationId::parse(wire), Some(*id));
            let json = serde_json::to_string(id).expect("serialize operation id");
            assert_eq!(
                serde_json::from_str::<OperationId>(&json).expect("deserialize operation id"),
                *id
            );
        }

        assert_eq!(OperationId::parse("browser.click"), None);
        assert!(serde_json::from_str::<OperationId>("\"computer\"").is_err());
    }

    #[test]
    fn intent_ids_are_closed_unique_dotted_and_serde_stable() {
        assert_eq!(IntentId::ALL.len(), 60);
        let values: BTreeSet<_> = IntentId::ALL.iter().map(|id| id.as_str()).collect();
        assert_eq!(values.len(), IntentId::ALL.len());

        for intent in IntentId::ALL {
            let wire = intent.as_str();
            assert!(wire.is_ascii());
            assert!(wire.contains('.'));
            assert_eq!(IntentId::parse(wire), Some(*intent));
            let json = serde_json::to_string(intent).expect("serialize intent id");
            assert_eq!(
                serde_json::from_str::<IntentId>(&json).expect("deserialize intent id"),
                *intent
            );
        }

        assert_eq!(IntentId::parse("left_click"), None);
    }

    #[test]
    fn browser_operation_has_only_semantic_identity_and_arguments() {
        let operation = BrowserOperation::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            serde_json::json!({"target": {"ref": "r_1"}}),
        );
        assert_eq!(
            serde_json::to_value(operation).expect("serialize operation"),
            serde_json::json!({
                "id": "browser.act",
                "intent": "act.click",
                "arguments": {"target": {"ref": "r_1"}}
            })
        );
    }

    #[test]
    fn invocation_presentation_is_bounded_and_validated_on_decode() {
        let presentation = InvocationPresentation::new(
            "ghostlight-legacy",
            1,
            "computer",
            Some("left_click".into()),
        )
        .expect("valid presentation");
        assert_eq!(presentation.profile_id(), "ghostlight-legacy");
        assert_eq!(presentation.profile_version(), 1);
        assert_eq!(presentation.external_tool(), "computer");
        assert_eq!(presentation.external_action(), Some("left_click"));

        let json = serde_json::to_string(&presentation).expect("serialize presentation");
        assert_eq!(
            serde_json::from_str::<InvocationPresentation>(&json)
                .expect("deserialize presentation"),
            presentation
        );
        assert!(InvocationPresentation::new("native", 0, "tool", None).is_err());
        assert!(InvocationPresentation::new("native", 1, "", None).is_err());
        assert!(InvocationPresentation::new("native\n", 1, "tool", None).is_err());
        assert!(InvocationPresentation::new("x".repeat(129), 1, "tool", None).is_err());
        assert!(
            serde_json::from_value::<InvocationPresentation>(serde_json::json!({
                "profileId": "native",
                "profileVersion": 1,
                "externalTool": "bad\nname"
            }))
            .is_err()
        );
    }

    #[test]
    fn tab_handle_is_bounded_opaque_and_redacted() {
        let handle = TabHandle::parse("t_generation_7").expect("valid handle");
        assert_eq!(handle.as_str(), "t_generation_7");
        assert_eq!(handle.to_string(), "<redacted-tab-handle>");
        assert_eq!(format!("{handle:?}"), "TabHandle(<redacted>)");
        let json = serde_json::to_string(&handle).expect("serialize handle");
        assert_eq!(
            serde_json::from_str::<TabHandle>(&json).expect("deserialize handle"),
            handle
        );
        assert!(TabHandle::parse("").is_none());
        assert!(TabHandle::parse("bad\nhandle").is_none());
        assert!(TabHandle::parse(&"x".repeat(MAX_TAB_HANDLE_BYTES + 1)).is_none());
    }

    #[test]
    fn image_parts_enforce_canonical_base64_and_media_types() {
        for data in [
            "AA==", "AA", "AAA=", "AAA", "AAAA", "Zg==", "Zg", "Zm8=", "Zm8", "R0lGODlh",
        ] {
            assert!(ResultPart::image(data, "image/png").is_ok(), "{data}");
        }
        for data in [
            "",
            "A",
            "=AAA",
            "AA=A",
            "AA=",
            "AA===",
            "AAA==",
            "AAAA=",
            "AA A=",
            "AA\nA",
            "AA-_",
            "data:image/png;base64,AA==",
            "Zh==",
            "Zh",
            "Zm9=",
            "Zm9",
        ] {
            assert_eq!(
                ResultPart::image(data, "image/png"),
                Err(ResultPartError::InvalidImageData),
                "{data}"
            );
        }

        let uppercase = ResultPart::image("AA==", "IMAGE/PNG").expect("case normalizes");
        assert_eq!(
            uppercase,
            ResultPart::Image {
                data: "AA==".into(),
                mime_type: "image/png".into(),
            }
        );
        for mime_type in [
            "image/jpeg",
            "image/gif",
            "image/svg+xml",
            "image/vnd.microsoft.icon",
        ] {
            assert!(ResultPart::image("AA==", mime_type).is_ok(), "{mime_type}");
        }
        for mime_type in [
            "",
            "image/",
            "/png",
            "text/plain",
            "image/*",
            "image/png; charset=binary",
            "image//png",
            " image/png",
            "image/png ",
            "image/p\nng",
            "image/pong\u{00e9}",
            "image/+png",
        ] {
            assert_eq!(
                ResultPart::image("AA==", mime_type),
                Err(ResultPartError::InvalidImageMimeType),
                "{mime_type:?}"
            );
        }
        let exact_bound = format!(
            "image/{}",
            "a".repeat(MAX_RESULT_IMAGE_MIME_TYPE_BYTES - "image/".len())
        );
        assert!(ResultPart::image("AA==", exact_bound).is_ok());
        let over_bound = format!(
            "image/{}",
            "a".repeat(MAX_RESULT_IMAGE_MIME_TYPE_BYTES - "image/".len() + 1)
        );
        assert_eq!(
            ResultPart::image("AA==", over_bound),
            Err(ResultPartError::InvalidImageMimeType)
        );
    }

    #[test]
    fn image_part_wire_shape_validates_in_both_directions() {
        let part = ResultPart::image("AA==", "image/png").expect("valid image");
        assert_eq!(
            serde_json::to_string(&part).expect("serialize image"),
            r#"{"type":"image","data":"AA==","mime_type":"image/png"}"#
        );
        assert_eq!(
            serde_json::from_value::<ResultPart>(serde_json::json!({
                "type": "image",
                "data": "AA==",
                "mime_type": "IMAGE/PNG"
            }))
            .expect("deserialize and normalize image"),
            part
        );

        let invalid_in_process = ResultPart::Image {
            data: "AA=".into(),
            mime_type: "image/png".into(),
        };
        assert!(serde_json::to_value(&invalid_in_process).is_err());
        assert!(serde_json::from_value::<ResultPart>(serde_json::json!({
            "type": "image",
            "data": "AA=",
            "mime_type": "image/png"
        }))
        .is_err());

        let mut result = BrowserResult::new(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        result.parts = vec![part];
        let mut value = serde_json::to_value(result).expect("serialize browser result");
        value["parts"][0]["data"] = serde_json::json!("AAAA=");
        assert!(serde_json::from_value::<BrowserResult>(value).is_err());
    }

    #[test]
    fn browser_result_is_canonical_and_protocol_neutral() {
        let workspace = WorkspaceId::mint();
        let tab = TabHandle::parse("t_generation_7").expect("valid handle");
        let provenance = PageProvenance::new(
            vec![
                "/tab/url".into(),
                "/tab/title".into(),
                "/parts/0/text".into(),
                "/parts/1/data".into(),
                "/data/interaction_receipt/target".into(),
            ],
            Some("https://example.com".into()),
            Some("session-7".into()),
            Some("https://frame.example".into()),
        )
        .expect("scoped provenance");

        let mut result = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Ok,
            OperationEffect::Committed,
        );
        result.readiness = Some(Readiness {
            status: ReadinessStatus::Ready,
            condition: Some(ReadinessCondition {
                requested: true,
                met: true,
            }),
            settlement: Some(ReadinessSettlement {
                requested: true,
                status: SettlementStatus::Settled,
            }),
            elapsed_ms: Some(1850),
        });
        result.workspace = Some(workspace.clone());
        result.tab = Some(ResultTab {
            id: tab,
            url: Some("https://example.com".into()),
            title: Some("Example".into()),
        });
        result.parts = vec![
            ResultPart::Text {
                text: "clicked".into(),
            },
            ResultPart::Image {
                data: "aW1hZ2U=".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        result.data = serde_json::json!({
            "interaction_receipt": {"target": "Save", "assurance": "ref"}
        });
        result.provenance = Some(provenance);

        let value = serde_json::to_value(&result).expect("serialize canonical result");
        assert_eq!(value["schema"], "ghostlight.browser.result/1");
        assert_eq!(value["operation"], "browser.act");
        assert_eq!(value["intent"], "act.click");
        assert_eq!(value["workspace"], workspace.as_str());
        assert_eq!(value["parts"][0]["type"], "text");
        assert_eq!(value["parts"][1]["type"], "image");
        assert_eq!(value["parts"][1]["mime_type"], "image/jpeg");
        assert_eq!(value["provenance"]["frame_origin"], "https://frame.example");
        assert_eq!(
            serde_json::from_value::<BrowserResult>(value.clone())
                .expect("deserialize canonical result"),
            result
        );

        let rendered = value.to_string();
        assert!(!rendered.contains("\"content\""));
        assert!(!rendered.contains("jsonrpc"));
        assert!(!rendered.contains("structuredContent"));
    }

    #[test]
    fn flow_result_round_trips_without_nested_surface_identity() {
        let mut completed = BrowserResult::new(
            OperationId::BrowserScreenshot,
            IntentId::ScreenshotViewport,
            BrowserResultStatus::Ok,
            OperationEffect::None,
        );
        completed.parts = vec![
            ResultPart::Text {
                text: "captured".into(),
            },
            ResultPart::Image {
                data: "AAAA".into(),
                mime_type: "image/jpeg".into(),
            },
        ];
        completed.data = serde_json::json!({"image_id": "img_1"});
        completed.provenance = Some(
            PageProvenance::new(
                vec!["/parts/0/text".into(), "/parts/1/data".into()],
                Some("https://example.com".into()),
                Some("session-1".into()),
                None,
            )
            .expect("valid nested provenance"),
        );

        let not_run = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::NotDispatched,
            OperationEffect::None,
        );
        let flow = FlowResultData {
            steps: vec![
                FlowStepResult {
                    step: 1,
                    status: FlowStepStatus::Ok,
                    result: completed,
                },
                FlowStepResult {
                    step: 2,
                    status: FlowStepStatus::NotRun,
                    result: not_run,
                },
            ],
            summary: "1/2 steps completed".into(),
            duration_ms: 7,
            termination: FlowTermination {
                reason: FlowTerminationReason::Failed,
                step: Some(1),
            },
        };

        let value = serde_json::to_value(&flow).expect("serialize flow result");
        assert_eq!(
            value["steps"][0]["result"]["operation"],
            "browser.screenshot"
        );
        assert_eq!(value["steps"][1]["status"], "not_run");
        let rendered = value.to_string();
        assert!(!rendered.contains("\"tool\""));
        assert!(!rendered.contains("\"name\""));
        assert_eq!(
            serde_json::from_value::<FlowResultData>(value).expect("deserialize flow result"),
            flow
        );
    }

    #[test]
    fn browser_result_terminal_dispositions_validate_as_one_closed_contract() {
        let key = (OperationId::BrowserAct, IntentId::ActClick);
        for (status, effect, retry, expected) in [
            (
                BrowserResultStatus::Ok,
                OperationEffect::Unknown,
                None,
                BrowserResultValidationError::UnknownEffectWithTerminalStatus,
            ),
            (
                BrowserResultStatus::Partial,
                OperationEffect::Dispatched,
                None,
                BrowserResultValidationError::TerminalDispatched,
            ),
            (
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                None,
                BrowserResultValidationError::InvalidOutcomeUnknown,
            ),
            (
                BrowserResultStatus::Held,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe),
                BrowserResultValidationError::PreDispatchStatusWithEffect,
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
                None,
                BrowserResultValidationError::InvalidCancellation,
            ),
        ] {
            let mut result = BrowserResult::new(key.0, key.1, status, effect);
            result.retry = retry;
            assert_eq!(result.validate_semantics(), Err(expected));
        }

        for (status, effect, retry) in [
            (
                BrowserResultStatus::OutcomeUnknown,
                OperationEffect::Unknown,
                Some(RetryDisposition::Unsafe),
            ),
            (
                BrowserResultStatus::Blocked,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe),
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Committed,
                Some(RetryDisposition::Unsafe),
            ),
            (
                BrowserResultStatus::Cancelled,
                OperationEffect::Unknown,
                Some(RetryDisposition::Unsafe),
            ),
        ] {
            let mut result = BrowserResult::new(key.0, key.1, status, effect);
            result.retry = retry;
            assert_eq!(result.validate_semantics(), Ok(()));
        }
    }

    #[test]
    fn blocked_result_can_carry_state_refresh_recovery() {
        let mut result = BrowserResult::new(
            OperationId::BrowserAct,
            IntentId::ActClick,
            BrowserResultStatus::Blocked,
            OperationEffect::None,
        );
        result.retry = Some(RetryDisposition::AfterStateChange);
        result.recovery = Some(RecoveryHint {
            message: "refresh target refs".into(),
            next_operation: Some(OperationKey::new(
                OperationId::BrowserSnapshot,
                IntentId::SnapshotCapture,
            )),
        });

        let value = serde_json::to_value(result).expect("serialize blocked result");
        assert_eq!(value["status"], "blocked");
        assert_eq!(value["effect"], "none");
        assert_eq!(value["retry"], "after_state_change");
        assert_eq!(
            value["recovery"]["next_operation"],
            serde_json::json!({
                "id": "browser.snapshot",
                "intent": "snapshot.capture"
            })
        );
    }

    #[test]
    fn provenance_cannot_mark_service_facts_or_handles_untrusted() {
        for pointer in [
            "/schema",
            "/operation",
            "/intent",
            "/status",
            "/effect",
            "/retry",
            "/recovery",
            "/workspace",
            "/tab/id",
            "/parts/0/type",
            "/parts/0/mime_type",
        ] {
            assert!(PageProvenance::new(vec![pointer.into()], None, None, None).is_err());
        }

        assert!(serde_json::from_value::<PageProvenance>(serde_json::json!({
            "untrusted_fields": ["/status"]
        }))
        .is_err());
    }

    #[test]
    fn provenance_frame_origin_is_bounded_and_validated_on_decode() {
        let provenance = PageProvenance::new(
            vec!["/data".into()],
            Some("https://example.com".into()),
            Some("00112233445566778899aabbccddeeff".into()),
            Some("https://frame.example".into()),
        )
        .expect("bounded frame origin");
        let wire = serde_json::to_value(&provenance).expect("serialize provenance");
        assert_eq!(wire["frame_origin"], "https://frame.example");
        assert_eq!(
            serde_json::from_value::<PageProvenance>(wire).expect("deserialize provenance"),
            provenance
        );

        for frame_origin in [
            String::new(),
            "https://frame.example\nforged".into(),
            "x".repeat(MAX_PAGE_ORIGIN_BYTES + 1),
        ] {
            assert_eq!(
                PageProvenance::new(vec!["/data".into()], None, None, Some(frame_origin)),
                Err(PageProvenanceError::InvalidFrameOrigin)
            );
        }

        assert!(serde_json::from_value::<PageProvenance>(serde_json::json!({
            "untrusted_fields": ["/data"],
            "frame_origin": "bad\norigin"
        }))
        .is_err());
    }
}
