// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Policy-free physical browser mechanisms below canonical product operations.
//!
//! [`OperationKey`] remains the service's validation, governance, scheduling, and audit identity.
//! A [`MechanismId`] names only a concrete browser-side capability. Surface names and legacy
//! extension aliases never enter this module. The outbound browser adapter may serialize a
//! [`MechanismRequest`] to a covered older extension wire, but that compatibility spelling is not
//! a mechanism identity.

use crate::ToolError;
use ghostlight_transport::operation::{IntentId, OperationId, OperationKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Map, Value};

macro_rules! stable_mechanism_ids {
    ($(#[$enum_meta:meta])* $($variant:ident => $wire:literal,)+) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum MechanismId {
            $($variant,)+
        }

        impl MechanismId {
            /// Every mechanism id in stable wire order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Return the stable physical-mechanism spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }

            /// Parse one exact stable physical-mechanism spelling.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for MechanismId {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for MechanismId {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown physical browser mechanism: {value}"
                    ))
                })
            }
        }

        impl std::fmt::Display for MechanismId {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_mechanism_ids! {
    /// Closed request/reply vocabulary implemented by the browser adapter.
    WorkspaceTabsInspect => "workspace.tabs.inspect",
    WorkspaceTabsEnsure => "workspace.tabs.ensure",
    WorkspaceTabCreate => "workspace.tab.create",
    TabFocus => "tab.focus",
    TabClose => "tab.close",
    NavigateUrl => "navigate.url",
    NavigateBack => "navigate.back",
    NavigateForward => "navigate.forward",
    NavigateReload => "navigate.reload",
    PageSnapshot => "page.snapshot",
    PageReadText => "page.read_text",
    PageFind => "page.find",
    ScreenshotViewport => "screenshot.viewport",
    ScreenshotRegion => "screenshot.region",
    ElementResolve => "element.resolve",
    TargetCue => "target.cue",
    PointerClick => "pointer.click",
    PointerHover => "pointer.hover",
    PointerDrag => "pointer.drag",
    TextType => "text.type",
    KeyPress => "key.press",
    WheelScroll => "wheel.scroll",
    ScrollTargetIntoView => "scroll.target_into_view",
    ScrollViewportToOffset => "scroll.viewport_to_offset",
    FormInspect => "form.inspect",
    FormSetValue => "form.set_value",
    WaitDelay => "wait.delay",
    WaitUntil => "wait.until",
    DialogInspect => "dialog.inspect",
    DialogAccept => "dialog.accept",
    DialogDismiss => "dialog.dismiss",
    DialogRespond => "dialog.respond",
    ViewportResize => "viewport.resize",
    UploadFiles => "upload.files",
    UploadImage => "upload.image",
    ConsoleRead => "console.read",
    NetworkRead => "network.read",
    PageEvaluate => "page.evaluate",
    RecordingStart => "recording.start",
    RecordingStop => "recording.stop",
    PointsRescale => "points.rescale",
    NarrationShow => "narration.show",
    TabUrlQuery => "tab.url_query",
}

macro_rules! stable_control_ids {
    ($(#[$enum_meta:meta])* $($variant:ident => $wire:literal,)+) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum BrowserControlId {
            $($variant,)+
        }

        impl BrowserControlId {
            /// Every one-way browser control id in stable wire order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Return the stable physical-control spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }

            /// Parse one exact stable physical-control spelling.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for BrowserControlId {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for BrowserControlId {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown one-way browser control: {value}"
                    ))
                })
            }
        }

        impl std::fmt::Display for BrowserControlId {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_control_ids! {
    /// Closed service-to-browser vocabulary for physical work that has no reply.
    RecordingLeaseRenew => "recording.lease_renew",
    RecordingCancel => "recording.cancel",
    NarrationClear => "narration.clear",
    NotificationShow => "notification.show",
    AttentionRequired => "attention.required",
    AttentionResolved => "attention.resolved",
}

macro_rules! stable_event_ids {
    ($(#[$enum_meta:meta])* $($variant:ident => $wire:literal,)+) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum BrowserEventId {
            $($variant,)+
        }

        impl BrowserEventId {
            /// Every unsolicited physical browser event id in stable wire order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Return the stable physical-event spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire,)+
                }
            }

            /// Parse one exact stable physical-event spelling.
            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for BrowserEventId {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for BrowserEventId {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown unsolicited browser event: {value}"
                    ))
                })
            }
        }

        impl std::fmt::Display for BrowserEventId {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

stable_event_ids! {
    /// Closed browser-to-service vocabulary for unsolicited physical events consumed by core.
    RecordingFrame => "recording.frame",
    RecordingEnded => "recording.ended",
}

/// Why the browser-side recording relay ended without a correlated stop reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingEndReason {
    /// The absolute browser-side capture deadline elapsed.
    HardTimeout,
    /// The tab or debugger attachment disappeared.
    BrowserDetached,
    /// The service health lease expired.
    LeaseExpired,
}

impl RecordingEndReason {
    /// Return the stable semantic reason spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HardTimeout => "hard_timeout",
            Self::BrowserDetached => "browser_detached",
            Self::LeaseExpired => "lease_expired",
        }
    }

    /// Parse one exact stable semantic reason spelling.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hard_timeout" => Some(Self::HardTimeout),
            "browser_detached" => Some(Self::BrowserDetached),
            "lease_expired" => Some(Self::LeaseExpired),
            _ => None,
        }
    }
}

/// One policy-free service-to-browser control that expects no reply.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserControl {
    /// Closed physical control identity.
    id: BrowserControlId,
    /// Control-specific canonical data.
    input: Value,
    /// Runtime dispatch authority retained until the one-way enqueue boundary.
    #[serde(skip)]
    authority: MechanismAuthority,
}

impl BrowserControl {
    fn new(
        id: BrowserControlId,
        input: Value,
        authority: MechanismAuthority,
    ) -> Result<Self, ToolError> {
        if !input.is_object() {
            return Err(ToolError::invalid_request(format!(
                "browser control {} input must be an object",
                id.as_str()
            )));
        }
        Ok(Self {
            id,
            input,
            authority,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_only(id: BrowserControlId, input: Value) -> Self {
        debug_assert!(input.is_object());
        Self {
            id,
            input,
            authority: MechanismAuthority::Test,
        }
    }

    /// Return the closed physical control identity.
    pub const fn id(&self) -> BrowserControlId {
        self.id
    }

    /// Return the canonical control input.
    pub fn input(&self) -> &Value {
        &self.input
    }

    /// Return the final safety checks carried by this control's closed authority source.
    pub(crate) const fn final_admission(&self) -> FinalAdmission {
        match self.authority {
            MechanismAuthority::Operation => FinalAdmission::STRICT,
            MechanismAuthority::Auxiliary(purpose) => FinalAdmission::for_auxiliary(purpose),
            #[cfg(test)]
            MechanismAuthority::Test => FinalAdmission::STRICT,
        }
    }

    /// Construct a one-way control emitted by one response-dependent canonical operation.
    ///
    /// Cross-cutting browser supervision uses [`Self::for_auxiliary`]. Semantic handlers use this
    /// constructor so a new control cannot silently escape that operation's declared physical
    /// plan.
    pub fn for_operation(
        operation: OperationKey,
        id: BrowserControlId,
        input: Value,
    ) -> Result<Self, ToolError> {
        let Some(plan) = dynamic_operation_plan(operation) else {
            return Err(ToolError::invalid_request(format!(
                "operation {} / {} has no dynamic browser-control plan",
                operation.id, operation.intent
            )));
        };
        if !plan.allowed_controls.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser control {id} is not allowed for operation {} / {}",
                operation.id, operation.intent
            )));
        }
        Self::new(id, input, MechanismAuthority::Operation)
    }

    /// Construct an operation-declared recording teardown with supervisor admission posture.
    ///
    /// This is the one bounded exception for an uncertain recording start: the canonical
    /// operation must explicitly allow `recording.cancel`, but cleanup may still run after a
    /// takeover or attention transition to prevent an orphaned capture relay. Panic remains an
    /// unconditional outbound refusal.
    pub(crate) fn for_operation_cleanup(
        operation: OperationKey,
        id: BrowserControlId,
        input: Value,
    ) -> Result<Self, ToolError> {
        if id != BrowserControlId::RecordingCancel {
            return Err(ToolError::invalid_request(
                "only recording.cancel has an operation-owned cleanup posture",
            ));
        }
        let Some(plan) = dynamic_operation_plan(operation) else {
            return Err(ToolError::invalid_request(format!(
                "operation {} / {} has no dynamic browser-control plan",
                operation.id, operation.intent
            )));
        };
        if !plan.allowed_controls.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser control {id} is not allowed for operation {} / {}",
                operation.id, operation.intent
            )));
        }
        Self::new(
            id,
            input,
            MechanismAuthority::Auxiliary(BrowserAuxiliaryPurpose::RecordingSupervisor),
        )
    }

    /// Construct a one-way control owned by one closed cross-cutting browser purpose.
    pub(crate) fn for_auxiliary(
        purpose: BrowserAuxiliaryPurpose,
        id: BrowserControlId,
        input: Value,
    ) -> Result<Self, ToolError> {
        let plan = auxiliary_plan(purpose);
        if !plan.allowed_controls.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser control {id} is not allowed for auxiliary purpose {}",
                purpose.as_str()
            )));
        }
        Self::new(id, input, MechanismAuthority::Auxiliary(purpose))
    }
}

/// One unsolicited policy-free browser event with canonical physical data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserEvent {
    /// Closed physical event identity.
    pub id: BrowserEventId,
    /// Event-specific canonical data.
    pub input: Value,
}

impl BrowserEvent {
    /// Construct one unsolicited browser event.
    pub fn new(id: BrowserEventId, input: Value) -> Result<Self, ToolError> {
        if !input.is_object() {
            return Err(ToolError::invalid_request(format!(
                "browser event {} input must be an object",
                id.as_str()
            )));
        }
        Ok(Self { id, input })
    }

    /// Construct an infallible event from a statically object-shaped value.
    pub(crate) fn object(id: BrowserEventId, input: Value) -> Self {
        debug_assert!(input.is_object());
        Self { id, input }
    }
}

/// One physical browser request with mechanism-specific canonical input.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MechanismRequest {
    /// Closed physical identity.
    id: MechanismId,
    /// Mechanism-specific data. This never contains a model-facing action discriminator.
    input: Value,
    /// Runtime dispatch authority retained through tab rewriting and legacy serialization.
    #[serde(skip)]
    authority: MechanismAuthority,
}

/// Where one typed physical request obtained authority to reach the browser.
///
/// This value is deliberately not serialized. It is a service-side proof used to select the
/// final enqueue checks after every semantic, scheduling, and compatibility translation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MechanismAuthority {
    Operation,
    Auxiliary(BrowserAuxiliaryPurpose),
    #[cfg(test)]
    Test,
}

/// Safety checks retained until the exact browser-frame enqueue boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FinalAdmission {
    check_hold: bool,
    check_attention: bool,
}

impl FinalAdmission {
    const STRICT: Self = Self {
        check_hold: true,
        check_attention: true,
    };

    const fn for_auxiliary(purpose: BrowserAuxiliaryPurpose) -> Self {
        match purpose {
            // Parking follows a committed denied landing and must remain possible after that
            // denial opens the attention circuit. Panic and human takeover still win.
            BrowserAuxiliaryPurpose::SafetyPark => Self {
                check_hold: true,
                check_attention: false,
            },
            // Deadline/lease teardown is privacy-preserving browser supervision rather than a
            // user-requested effect. It may stop capture during takeover or attention, but the
            // process-wide panic check remains unconditional in the outbound chokepoint.
            BrowserAuxiliaryPurpose::RecordingSupervisor => Self {
                check_hold: false,
                check_attention: false,
            },
            BrowserAuxiliaryPurpose::RecordingHealth
            | BrowserAuxiliaryPurpose::RecordingInstrumentation
            | BrowserAuxiliaryPurpose::TabUrlProbe => Self::STRICT,
            // Presentation is non-authoritative and cannot borrow one workspace's attention
            // state. Attention overlays must also be able to announce the circuit that just
            // opened. Both still obey takeover and the unconditional panic check.
            BrowserAuxiliaryPurpose::Presentation
            | BrowserAuxiliaryPurpose::AttentionPresentation => Self {
                check_hold: true,
                check_attention: false,
            },
        }
    }

    /// Strict admission used by test-only physical requests and boundary fixtures.
    #[cfg(test)]
    pub(crate) const fn strict() -> Self {
        Self::STRICT
    }

    /// Whether a human takeover must refuse this request before enqueue.
    pub(crate) const fn checks_hold(self) -> bool {
        self.check_hold
    }

    /// Whether an open workspace attention circuit must refuse this request before enqueue.
    pub(crate) const fn checks_attention(self) -> bool {
        self.check_attention
    }
}

impl MechanismRequest {
    fn new(
        id: MechanismId,
        input: Value,
        authority: MechanismAuthority,
    ) -> Result<Self, ToolError> {
        if !input.is_object() {
            return Err(ToolError::invalid_request(format!(
                "mechanism {} input must be an object",
                id.as_str()
            )));
        }
        Ok(Self {
            id,
            input,
            authority,
        })
    }

    fn object(id: MechanismId, input: Value, authority: MechanismAuthority) -> Self {
        debug_assert!(input.is_object());
        Self {
            id,
            input,
            authority,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_only(id: MechanismId, input: Value) -> Self {
        Self::object(id, input, MechanismAuthority::Test)
    }

    /// Return the closed physical mechanism identity.
    pub const fn id(&self) -> MechanismId {
        self.id
    }

    /// Return the canonical mechanism input.
    pub fn input(&self) -> &Value {
        &self.input
    }

    /// Return the final safety checks carried by this request's closed authority source.
    pub(crate) const fn final_admission(&self) -> FinalAdmission {
        match self.authority {
            MechanismAuthority::Operation => FinalAdmission::STRICT,
            MechanismAuthority::Auxiliary(purpose) => FinalAdmission::for_auxiliary(purpose),
            #[cfg(test)]
            MechanismAuthority::Test => FinalAdmission::STRICT,
        }
    }

    /// Clone this authorized request with the exact tab selected for physical delivery.
    pub(crate) fn with_delivery_tab(&self, native: i64) -> Self {
        let mut request = self.clone();
        if let Some(input) = request.input.as_object_mut() {
            input.insert("tab".to_owned(), serde_json::json!(native));
        }
        request
    }

    /// Construct a request emitted by one response-dependent canonical operation.
    ///
    /// The closed dynamic plan is enforced at construction time. Direct operations continue
    /// through [`compile_operation`], while browser-wide instrumentation remains an explicit
    /// cross-cutting mechanism below any semantic handler.
    pub fn for_operation(
        operation: OperationKey,
        id: MechanismId,
        input: Value,
    ) -> Result<Self, ToolError> {
        let Some(plan) = dynamic_operation_plan(operation) else {
            return Err(ToolError::invalid_request(format!(
                "operation {} / {} has no dynamic browser-mechanism plan",
                operation.id, operation.intent
            )));
        };
        if !plan.allowed_mechanisms.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser mechanism {id} is not allowed for operation {} / {}",
                operation.id, operation.intent
            )));
        }
        Self::new(id, input, MechanismAuthority::Operation)
    }

    /// Construct a request owned by one closed cross-cutting browser purpose.
    pub(crate) fn for_auxiliary(
        purpose: BrowserAuxiliaryPurpose,
        id: MechanismId,
        input: Value,
    ) -> Result<Self, ToolError> {
        let plan = auxiliary_plan(purpose);
        if !plan.allowed_mechanisms.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser mechanism {id} is not allowed for auxiliary purpose {}",
                purpose.as_str()
            )));
        }
        Self::new(id, input, MechanismAuthority::Auxiliary(purpose))
    }
}

/// Closed physical authority for one response-dependent canonical operation.
///
/// The slices are allowed sets, not a claim that every invocation emits every item or that an
/// item appears only once. Their order follows the operation's normal execution sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicOperationPlan {
    /// Request/reply mechanisms the semantic handler may issue.
    pub allowed_mechanisms: &'static [MechanismId],
    /// One-way controls the semantic handler may trigger.
    pub allowed_controls: &'static [BrowserControlId],
}

impl DynamicOperationPlan {
    const fn new(
        allowed_mechanisms: &'static [MechanismId],
        allowed_controls: &'static [BrowserControlId],
    ) -> Self {
        Self {
            allowed_mechanisms,
            allowed_controls,
        }
    }
}

/// Closed cross-cutting browser purposes below canonical operation handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserAuxiliaryPurpose {
    /// Best-effort parking after a committed landing fails the post-landing policy check.
    SafetyPark,
    /// Deadline and teardown work owned by the recording supervisor.
    RecordingSupervisor,
    /// Periodic capture-health renewal, which must stop at every user safety boundary.
    RecordingHealth,
    /// Coordinate normalization injected only while annotating an active recording.
    RecordingInstrumentation,
    /// Read-only URL resolution used by governance before a canonical operation dispatch.
    TabUrlProbe,
    /// Non-authoritative narration cleanup and user notification rendering.
    Presentation,
    /// Attention overlay state rendered after the service has made its control-plane decision.
    AttentionPresentation,
}

impl BrowserAuxiliaryPurpose {
    /// Every auxiliary purpose in stable audit/test order.
    pub const ALL: &'static [Self] = &[
        Self::SafetyPark,
        Self::RecordingSupervisor,
        Self::RecordingHealth,
        Self::RecordingInstrumentation,
        Self::TabUrlProbe,
        Self::Presentation,
        Self::AttentionPresentation,
    ];

    /// Return the stable internal purpose spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SafetyPark => "safety_park",
            Self::RecordingSupervisor => "recording_supervisor",
            Self::RecordingHealth => "recording_health",
            Self::RecordingInstrumentation => "recording_instrumentation",
            Self::TabUrlProbe => "tab_url_probe",
            Self::Presentation => "presentation",
            Self::AttentionPresentation => "attention_presentation",
        }
    }
}

/// Closed physical authority for one cross-cutting browser purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxiliaryBrowserPlan {
    /// Request/reply mechanisms this purpose may issue.
    pub allowed_mechanisms: &'static [MechanismId],
    /// One-way controls this purpose may issue.
    pub allowed_controls: &'static [BrowserControlId],
}

/// Return the exact physical authority of one cross-cutting browser purpose.
pub const fn auxiliary_plan(purpose: BrowserAuxiliaryPurpose) -> AuxiliaryBrowserPlan {
    use BrowserAuxiliaryPurpose as P;
    use BrowserControlId as C;
    use MechanismId as M;

    match purpose {
        P::SafetyPark => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::NavigateUrl],
            allowed_controls: &[],
        },
        P::RecordingSupervisor => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::RecordingStop],
            allowed_controls: &[C::RecordingCancel],
        },
        P::RecordingHealth => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[],
            allowed_controls: &[C::RecordingLeaseRenew],
        },
        P::RecordingInstrumentation => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::PointsRescale],
            allowed_controls: &[],
        },
        P::TabUrlProbe => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::TabUrlQuery],
            allowed_controls: &[],
        },
        P::Presentation => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[],
            allowed_controls: &[C::NarrationClear, C::NotificationShow],
        },
        P::AttentionPresentation => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[],
            allowed_controls: &[C::AttentionRequired, C::AttentionResolved],
        },
    }
}

/// How one canonical operation obtains physical browser work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMechanismPlan {
    /// One request is compiled directly from the canonical arguments.
    Direct,
    /// A service handler may issue a response-dependent mechanism sequence.
    Dynamic(DynamicOperationPlan),
    /// Child canonical operations define the physical sequence.
    Composition,
    /// The operation is wholly service-local.
    Local,
}

/// Return the physical planning class for one implemented canonical operation.
pub fn operation_plan(key: OperationKey) -> Option<OperationMechanismPlan> {
    use IntentId::*;
    use OperationId::*;

    if let Some(plan) = dynamic_operation_plan(key) {
        return Some(OperationMechanismPlan::Dynamic(plan));
    }

    let plan = match (key.id, key.intent) {
        (BrowserTabs, TabsList | TabsNew | TabsFocus | TabsClose)
        | (BrowserNavigate, NavigateUrl | NavigateBack | NavigateForward | NavigateReload)
        | (BrowserSnapshot, SnapshotCapture)
        | (BrowserRead, ReadText)
        | (BrowserFind, FindQuery)
        | (BrowserScreenshot, ScreenshotViewport | ScreenshotRegion)
        | (BrowserFill, FillField)
        | (BrowserWait, WaitDelay | WaitUntil)
        | (BrowserDialog, DialogStatus | DialogAccept | DialogDismiss | DialogRespond)
        | (
            BrowserInput,
            InputPointerClick
            | InputPointerRightClick
            | InputPointerDoubleClick
            | InputPointerTripleClick
            | InputPointerHover
            | InputPointerDrag
            | InputTypeText
            | InputPressKey
            | InputWheel
            | InputScrollToOffset,
        )
        | (BrowserViewport, ViewportResizeWindow)
        | (BrowserUpload, UploadClientFiles)
        | (BrowserConsole, ConsoleRead | ConsoleReadAndClear)
        | (BrowserNetwork, NetworkRead | NetworkReadAndClear)
        | (BrowserEvaluate, EvaluateJavascript)
        | (BrowserPresent, PresentNarrate) => OperationMechanismPlan::Direct,

        (BrowserFlow, FlowExecute | FlowPreflight) => OperationMechanismPlan::Composition,
        (WorkflowPlan, PlanUpdate)
        | (BrowserContext, ContextDescribe)
        | (BrowserRecord, RecordStatus) => OperationMechanismPlan::Local,
        _ => return None,
    };
    Some(plan)
}

/// Return the exact physical authority of one response-dependent canonical operation.
pub fn dynamic_operation_plan(key: OperationKey) -> Option<DynamicOperationPlan> {
    use BrowserControlId as C;
    use IntentId::*;
    use MechanismId as M;
    use OperationId::*;

    let plan = match (key.id, key.intent) {
        (BrowserAct, ActClick | ActRightClick | ActDoubleClick | ActTripleClick) => {
            DynamicOperationPlan::new(
                &[
                    M::ElementResolve,
                    M::TargetCue,
                    M::PointerClick,
                    M::WaitUntil,
                ],
                &[],
            )
        }
        (BrowserAct, ActHover) => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerHover,
                M::WaitUntil,
            ],
            &[],
        ),
        (BrowserAct, ActScrollIntoView) => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::ScrollTargetIntoView,
                M::WaitUntil,
            ],
            &[],
        ),
        (BrowserAct, ActSetValue) => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::FormSetValue,
                M::WaitUntil,
            ],
            &[],
        ),
        (BrowserFill, FillFields) => {
            DynamicOperationPlan::new(&[M::FormInspect, M::FormSetValue], &[])
        }
        (BrowserFill, FillFieldsAndSubmit) => {
            DynamicOperationPlan::new(&[M::FormInspect, M::FormSetValue, M::PointerClick], &[])
        }
        (BrowserUpload, UploadCapturedArtifact) => {
            DynamicOperationPlan::new(&[M::UploadImage], &[])
        }
        (BrowserRecord, RecordStart) => {
            DynamicOperationPlan::new(&[M::RecordingStart], &[C::RecordingCancel])
        }
        (BrowserRecord, RecordStop) => {
            DynamicOperationPlan::new(&[M::RecordingStop], &[C::RecordingCancel])
        }
        (BrowserRecord, RecordClear) => DynamicOperationPlan::new(&[], &[C::RecordingCancel]),
        (BrowserRecord, RecordExport) => {
            DynamicOperationPlan::new(&[M::RecordingStop, M::UploadImage], &[C::RecordingCancel])
        }
        _ => return None,
    };
    Some(plan)
}

/// Compile a direct canonical operation to one physical browser request.
///
/// Dynamic, composition, and local operations return `Ok(None)`. An unimplemented family/intent
/// pair fails closed rather than being guessed from similar names.
pub fn compile_operation(
    key: OperationKey,
    arguments: &Value,
) -> Result<Option<MechanismRequest>, ToolError> {
    let Some(plan) = operation_plan(key) else {
        return Err(ToolError::invalid_request(format!(
            "operation {} / {} has no physical mechanism plan",
            key.id, key.intent
        )));
    };
    if plan != OperationMechanismPlan::Direct {
        return Ok(None);
    }

    let mut input = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("canonical arguments must be an object"))?;
    let id = direct_mechanism(key, &mut input)?;
    Ok(Some(MechanismRequest::object(
        id,
        Value::Object(input),
        MechanismAuthority::Operation,
    )))
}

fn direct_mechanism(
    key: OperationKey,
    input: &mut Map<String, Value>,
) -> Result<MechanismId, ToolError> {
    use IntentId::*;
    use OperationId::*;

    let id = match (key.id, key.intent) {
        (BrowserTabs, TabsList) => match input.get("create_if_empty") {
            None | Some(Value::Bool(false)) => MechanismId::WorkspaceTabsInspect,
            Some(Value::Bool(true)) => {
                input.remove("create_if_empty");
                MechanismId::WorkspaceTabsEnsure
            }
            Some(_) => {
                return Err(ToolError::invalid_request(
                    "browser.tabs list create_if_empty must be a boolean",
                ));
            }
        },
        (BrowserTabs, TabsNew) => MechanismId::WorkspaceTabCreate,
        (BrowserTabs, TabsFocus) => MechanismId::TabFocus,
        (BrowserTabs, TabsClose) => MechanismId::TabClose,
        (BrowserNavigate, NavigateUrl) => MechanismId::NavigateUrl,
        (BrowserNavigate, NavigateBack) => MechanismId::NavigateBack,
        (BrowserNavigate, NavigateForward) => MechanismId::NavigateForward,
        (BrowserNavigate, NavigateReload) => MechanismId::NavigateReload,
        (BrowserSnapshot, SnapshotCapture) => MechanismId::PageSnapshot,
        (BrowserRead, ReadText) => MechanismId::PageReadText,
        (BrowserFind, FindQuery) => MechanismId::PageFind,
        (BrowserScreenshot, ScreenshotViewport) => MechanismId::ScreenshotViewport,
        (BrowserScreenshot, ScreenshotRegion) => MechanismId::ScreenshotRegion,
        (BrowserFill, FillField) => MechanismId::FormSetValue,
        (BrowserWait, WaitDelay) => MechanismId::WaitDelay,
        (BrowserWait, WaitUntil) => MechanismId::WaitUntil,
        (BrowserDialog, DialogStatus) => MechanismId::DialogInspect,
        (BrowserDialog, DialogAccept) => MechanismId::DialogAccept,
        (BrowserDialog, DialogDismiss) => MechanismId::DialogDismiss,
        (BrowserDialog, DialogRespond) => MechanismId::DialogRespond,
        (BrowserInput, InputPointerClick) => pointer_click(input, "left", 1),
        (BrowserInput, InputPointerRightClick) => pointer_click(input, "right", 1),
        (BrowserInput, InputPointerDoubleClick) => pointer_click(input, "left", 2),
        (BrowserInput, InputPointerTripleClick) => pointer_click(input, "left", 3),
        (BrowserInput, InputPointerHover) => MechanismId::PointerHover,
        (BrowserInput, InputPointerDrag) => MechanismId::PointerDrag,
        (BrowserInput, InputTypeText) => MechanismId::TextType,
        (BrowserInput, InputPressKey) => MechanismId::KeyPress,
        (BrowserInput, InputWheel) => MechanismId::WheelScroll,
        (BrowserInput, InputScrollToOffset) => MechanismId::ScrollViewportToOffset,
        (BrowserViewport, ViewportResizeWindow) => MechanismId::ViewportResize,
        (BrowserUpload, UploadClientFiles) => MechanismId::UploadFiles,
        (BrowserConsole, ConsoleRead) => MechanismId::ConsoleRead,
        (BrowserConsole, ConsoleReadAndClear) => {
            input.insert("clear".into(), Value::Bool(true));
            MechanismId::ConsoleRead
        }
        (BrowserNetwork, NetworkRead) => MechanismId::NetworkRead,
        (BrowserNetwork, NetworkReadAndClear) => {
            input.insert("clear".into(), Value::Bool(true));
            MechanismId::NetworkRead
        }
        (BrowserEvaluate, EvaluateJavascript) => MechanismId::PageEvaluate,
        (BrowserPresent, PresentNarrate) => MechanismId::NarrationShow,
        _ => {
            return Err(ToolError::invalid_request(format!(
                "direct operation {} / {} has no physical mechanism",
                key.id, key.intent
            )));
        }
    };
    Ok(id)
}

fn pointer_click(input: &mut Map<String, Value>, button: &'static str, count: u64) -> MechanismId {
    input.insert("button".into(), json!(button));
    input.insert("count".into(), json!(count));
    MechanismId::PointerClick
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn mechanism_ids_are_unique_stable_and_fail_closed() {
        let mut seen = HashSet::new();
        for id in MechanismId::ALL {
            assert!(seen.insert(id.as_str()));
            assert_eq!(MechanismId::parse(id.as_str()), Some(*id));
            let wire = serde_json::to_string(id).expect("serialize");
            assert_eq!(serde_json::from_str::<MechanismId>(&wire).unwrap(), *id);
        }
        assert!(MechanismId::parse("computer").is_none());
        assert!(serde_json::from_str::<MechanismId>("\"computer\"").is_err());
    }

    #[test]
    fn control_ids_are_unique_stable_and_fail_closed() {
        let mut seen = HashSet::new();
        for id in BrowserControlId::ALL {
            assert!(seen.insert(id.as_str()));
            assert_eq!(BrowserControlId::parse(id.as_str()), Some(*id));
            let wire = serde_json::to_string(id).expect("serialize");
            assert_eq!(
                serde_json::from_str::<BrowserControlId>(&wire).unwrap(),
                *id
            );
            let control = BrowserControl::test_only(*id, json!({}));
            assert_eq!(
                serde_json::to_value(&control).unwrap(),
                json!({"id": id.as_str(), "input": {}})
            );
        }
        assert!(BrowserControlId::parse("gif_capture_cancel").is_none());
        assert!(serde_json::from_str::<BrowserControlId>("\"gif_capture_cancel\"").is_err());
        assert!(BrowserControl::new(
            BrowserControlId::NarrationClear,
            json!([]),
            MechanismAuthority::Test,
        )
        .is_err());
    }

    #[test]
    fn event_ids_and_recording_reasons_are_closed_and_round_trip() {
        let mut seen = HashSet::new();
        for id in BrowserEventId::ALL {
            assert!(seen.insert(id.as_str()));
            assert_eq!(BrowserEventId::parse(id.as_str()), Some(*id));
            let wire = serde_json::to_string(id).expect("serialize");
            assert_eq!(serde_json::from_str::<BrowserEventId>(&wire).unwrap(), *id);
            let event = BrowserEvent::new(*id, json!({})).unwrap();
            let round_trip: BrowserEvent =
                serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
            assert_eq!(round_trip, event);
        }
        assert!(BrowserEventId::parse("gif_frame").is_none());
        assert!(serde_json::from_str::<BrowserEventId>("\"gif_frame\"").is_err());
        assert!(BrowserEvent::new(BrowserEventId::RecordingFrame, Value::Null).is_err());

        for reason in [
            RecordingEndReason::HardTimeout,
            RecordingEndReason::BrowserDetached,
            RecordingEndReason::LeaseExpired,
        ] {
            assert_eq!(RecordingEndReason::parse(reason.as_str()), Some(reason));
            let wire = serde_json::to_string(&reason).unwrap();
            assert_eq!(
                serde_json::from_str::<RecordingEndReason>(&wire).unwrap(),
                reason
            );
        }
        assert!(RecordingEndReason::parse("invented").is_none());
        assert!(serde_json::from_str::<RecordingEndReason>("\"invented\"").is_err());
    }

    #[test]
    fn request_authority_retains_the_exact_final_admission_policy() {
        let operation = MechanismRequest::test_only(MechanismId::WaitDelay, json!({}));
        assert!(operation.final_admission().checks_hold());
        assert!(operation.final_admission().checks_attention());

        for (purpose, hold, attention) in [
            (BrowserAuxiliaryPurpose::SafetyPark, true, false),
            (BrowserAuxiliaryPurpose::RecordingSupervisor, false, false),
            (
                BrowserAuxiliaryPurpose::RecordingInstrumentation,
                true,
                true,
            ),
            (BrowserAuxiliaryPurpose::TabUrlProbe, true, true),
        ] {
            let id = auxiliary_plan(purpose).allowed_mechanisms[0];
            let request = MechanismRequest::for_auxiliary(purpose, id, json!({})).unwrap();
            assert_eq!(request.final_admission().checks_hold(), hold);
            assert_eq!(request.final_admission().checks_attention(), attention);
        }

        let renewal = BrowserControl::for_auxiliary(
            BrowserAuxiliaryPurpose::RecordingHealth,
            BrowserControlId::RecordingLeaseRenew,
            json!({}),
        )
        .unwrap();
        assert!(renewal.final_admission().checks_hold());
        assert!(renewal.final_admission().checks_attention());

        assert_eq!(
            serde_json::to_value(&operation).unwrap(),
            json!({"id":"wait.delay","input":{}}),
            "runtime authority never crosses the browser compatibility wire"
        );

        for intent in [
            IntentId::RecordStart,
            IntentId::RecordStop,
            IntentId::RecordClear,
            IntentId::RecordExport,
        ] {
            let cleanup = BrowserControl::for_operation_cleanup(
                OperationKey::new(OperationId::BrowserRecord, intent),
                BrowserControlId::RecordingCancel,
                json!({}),
            )
            .expect("recording operation declares cleanup");
            assert!(!cleanup.final_admission().checks_hold());
            assert!(!cleanup.final_admission().checks_attention());
        }
        assert!(BrowserControl::for_operation_cleanup(
            OperationKey::new(OperationId::BrowserAct, IntentId::ActClick),
            BrowserControlId::RecordingCancel,
            json!({}),
        )
        .is_err());
    }

    #[test]
    fn pointer_variants_compile_to_one_typed_mechanism() {
        for (intent, button, count) in [
            (IntentId::InputPointerClick, "left", 1),
            (IntentId::InputPointerRightClick, "right", 1),
            (IntentId::InputPointerDoubleClick, "left", 2),
            (IntentId::InputPointerTripleClick, "left", 3),
        ] {
            let request = compile_operation(
                OperationKey::new(OperationId::BrowserInput, intent),
                &json!({"tab":1,"point":[10,20]}),
            )
            .unwrap()
            .unwrap();
            assert_eq!(request.id, MechanismId::PointerClick);
            assert_eq!(request.input["button"], button);
            assert_eq!(request.input["count"], count);
            assert!(request.input.get("action").is_none());
        }
    }

    #[test]
    fn presence_sensitive_legacy_inputs_survive_canonical_compilation() {
        let tabs = OperationKey::new(OperationId::BrowserTabs, IntentId::TabsList);
        let absent = compile_operation(tabs, &json!({})).unwrap().unwrap();
        assert_eq!(absent.id, MechanismId::WorkspaceTabsInspect);
        assert!(absent.input.get("create_if_empty").is_none());
        let explicit_false = compile_operation(tabs, &json!({"create_if_empty":false}))
            .unwrap()
            .unwrap();
        assert_eq!(explicit_false.id, MechanismId::WorkspaceTabsInspect);
        assert_eq!(explicit_false.input["create_if_empty"], false);
        let explicit_true = compile_operation(tabs, &json!({"create_if_empty":true}))
            .unwrap()
            .unwrap();
        assert_eq!(explicit_true.id, MechanismId::WorkspaceTabsEnsure);
        assert!(explicit_true.input.get("create_if_empty").is_none());

        for (id, plain, clear) in [
            (
                OperationId::BrowserConsole,
                IntentId::ConsoleRead,
                IntentId::ConsoleReadAndClear,
            ),
            (
                OperationId::BrowserNetwork,
                IntentId::NetworkRead,
                IntentId::NetworkReadAndClear,
            ),
        ] {
            let plain = compile_operation(OperationKey::new(id, plain), &json!({}))
                .unwrap()
                .unwrap();
            assert!(plain.input.get("clear").is_none());
            let clear = compile_operation(OperationKey::new(id, clear), &json!({}))
                .unwrap()
                .unwrap();
            assert_eq!(clear.input["clear"], true);
        }
    }

    #[test]
    fn local_and_composition_operations_never_get_placeholder_mechanisms() {
        for key in [
            OperationKey::new(OperationId::BrowserAct, IntentId::ActClick),
            OperationKey::new(OperationId::BrowserFill, IntentId::FillFields),
            OperationKey::new(OperationId::BrowserFlow, IntentId::FlowExecute),
            OperationKey::new(OperationId::WorkflowPlan, IntentId::PlanUpdate),
            OperationKey::new(OperationId::BrowserContext, IntentId::ContextDescribe),
        ] {
            assert_eq!(compile_operation(key, &json!({})).unwrap(), None);
        }
    }

    #[test]
    fn unsupported_family_intent_pairs_fail_closed() {
        assert!(compile_operation(
            OperationKey::new(OperationId::BrowserContext, IntentId::ActClick),
            &json!({})
        )
        .is_err());
    }
}
