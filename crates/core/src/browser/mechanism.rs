// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Policy-free physical browser mechanisms below Ghostlight operations.
//!
//! [`OperationKind`] remains the service's validation, governance, scheduling, and audit identity.
//! A [`MechanismId`] names only a concrete browser-side capability. Public tool names and adapter
//! wire aliases never enter this module.

use crate::ToolError;
use ghostlight_transport::operation::OperationKind;
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
    WorkspaceTabOpen => "workspace.tab.open",
    TabFocus => "tab.focus",
    TabClose => "tab.close",
    NavigateUrl => "navigate.url",
    NavigateBack => "navigate.back",
    NavigateForward => "navigate.forward",
    NavigateReload => "navigate.reload",
    NavigationAwaitReadiness => "navigation.await_readiness",
    NavigationVerifyDocument => "navigation.verify_document",
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
    NarrationClear => "narration.clear",
    NotificationShow => "notification.show",
    AttentionRequired => "attention.required",
    AttentionResolved => "attention.resolved",
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
    #[serde(skip)]
    canonical_navigation_proof_required: bool,
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
            BrowserAuxiliaryPurpose::TabUrlProbe | BrowserAuxiliaryPurpose::NavigationReadiness => {
                Self::STRICT
            }
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
            canonical_navigation_proof_required: false,
        })
    }

    fn object(id: MechanismId, input: Value, authority: MechanismAuthority) -> Self {
        debug_assert!(input.is_object());
        Self {
            id,
            input,
            authority,
            canonical_navigation_proof_required: false,
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

    /// Require the exact negotiated committed-document protocol for a canonical navigation.
    pub(crate) fn require_canonical_navigation_proof(&mut self) {
        self.canonical_navigation_proof_required = true;
    }

    /// Whether this request belongs to the canonical navigation result contract.
    pub(crate) const fn canonical_navigation_proof_required(&self) -> bool {
        self.canonical_navigation_proof_required
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
        operation: OperationKind,
        id: MechanismId,
        input: Value,
    ) -> Result<Self, ToolError> {
        let Some(plan) = dynamic_operation_plan(operation) else {
            return Err(ToolError::invalid_request(format!(
                "operation {} has no dynamic browser-mechanism plan",
                operation.as_str()
            )));
        };
        if !plan.allowed_mechanisms.contains(&id) {
            return Err(ToolError::invalid_request(format!(
                "browser mechanism {id} is not allowed for operation {}",
                operation.as_str()
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
    /// Read-only URL resolution used by governance before a canonical operation dispatch.
    TabUrlProbe,
    /// Exact-document readiness observation after one canonical navigation dispatch.
    NavigationReadiness,
    /// Non-authoritative narration cleanup and user notification rendering.
    Presentation,
    /// Attention overlay state rendered after the service has made its control-plane decision.
    AttentionPresentation,
}

impl BrowserAuxiliaryPurpose {
    /// Every auxiliary purpose in stable audit/test order.
    pub const ALL: &'static [Self] = &[
        Self::SafetyPark,
        Self::TabUrlProbe,
        Self::NavigationReadiness,
        Self::Presentation,
        Self::AttentionPresentation,
    ];

    /// Return the stable internal purpose spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SafetyPark => "safety_park",
            Self::TabUrlProbe => "tab_url_probe",
            Self::NavigationReadiness => "navigation_readiness",
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
        P::TabUrlProbe => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::TabUrlQuery],
            allowed_controls: &[],
        },
        P::NavigationReadiness => AuxiliaryBrowserPlan {
            allowed_mechanisms: &[M::NavigationAwaitReadiness, M::NavigationVerifyDocument],
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
pub fn operation_plan(operation: OperationKind) -> OperationMechanismPlan {
    use OperationKind as O;

    if let Some(plan) = dynamic_operation_plan(operation) {
        return OperationMechanismPlan::Dynamic(plan);
    }

    match operation {
        O::BrowserGetStatus => OperationMechanismPlan::Local,
        O::BrowserRunSequence => OperationMechanismPlan::Composition,
        O::BrowserOpenTab => OperationMechanismPlan::Direct,
        O::BrowserNavigate => OperationMechanismPlan::Composition,
        O::BrowserListTabs
        | O::BrowserFocusTab
        | O::BrowserCloseTab
        | O::BrowserGoBack
        | O::BrowserGoForward
        | O::BrowserReloadPage
        | O::BrowserInspectPage
        | O::BrowserScrollPage
        | O::BrowserPressEscape
        | O::BrowserGetDialog
        | O::BrowserHandleDialog => OperationMechanismPlan::Direct,
        O::BrowserReadPage
        | O::BrowserTakeScreenshot
        | O::BrowserClick
        | O::BrowserHover
        | O::BrowserScrollToTarget
        | O::BrowserPressKey
        | O::BrowserDrag
        | O::BrowserFillForm
        | O::BrowserWaitFor => unreachable!("dynamic plans returned above"),
    }
}

/// Return the exact physical authority of one response-dependent canonical operation.
pub fn dynamic_operation_plan(operation: OperationKind) -> Option<DynamicOperationPlan> {
    use MechanismId as M;
    use OperationKind as O;

    let plan = match operation {
        O::BrowserWaitFor => DynamicOperationPlan::new(&[M::WaitUntil], &[]),
        O::BrowserClick => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerClick,
                M::WaitUntil,
            ],
            &[],
        ),
        O::BrowserHover => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerHover,
                M::WaitUntil,
            ],
            &[],
        ),
        O::BrowserScrollToTarget => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::ScrollTargetIntoView,
                M::WaitUntil,
            ],
            &[],
        ),
        O::BrowserPressKey => DynamicOperationPlan::new(
            &[M::ElementResolve, M::TargetCue, M::KeyPress, M::WaitUntil],
            &[],
        ),
        O::BrowserDrag => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::TargetCue,
                M::PointerDrag,
                M::WaitUntil,
            ],
            &[],
        ),
        O::BrowserTakeScreenshot => DynamicOperationPlan::new(
            &[
                M::ElementResolve,
                M::ScreenshotRegion,
                M::ScreenshotViewport,
            ],
            &[],
        ),
        O::BrowserReadPage => DynamicOperationPlan::new(&[M::ElementResolve, M::PageReadText], &[]),
        O::BrowserFillForm => {
            DynamicOperationPlan::new(&[M::FormInspect, M::FormSetValue, M::PointerClick], &[])
        }
        O::BrowserNavigate => {
            DynamicOperationPlan::new(&[M::WorkspaceTabOpen, M::NavigateUrl], &[])
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
    operation: OperationKind,
    arguments: &Value,
) -> Result<Option<MechanismRequest>, ToolError> {
    let plan = operation_plan(operation);
    let compound_direct = matches!(
        operation,
        OperationKind::BrowserNavigate if arguments.get("tab").is_some()
    );
    if plan != OperationMechanismPlan::Direct && !compound_direct {
        return Ok(None);
    }

    let mut input = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("canonical arguments must be an object"))?;
    let id = direct_mechanism(operation, &mut input)?;
    Ok(Some(MechanismRequest::object(
        id,
        Value::Object(input),
        MechanismAuthority::Operation,
    )))
}

/// Compile one URL navigation transaction for either an existing tab or the first tab in a
/// workspace. A zero-state navigation is one physical `workspace.tab.open` transaction, so the
/// browser never exposes an intermediate blank page.
pub(crate) fn compile_navigation_transaction(
    arguments: &Value,
) -> Result<MechanismRequest, ToolError> {
    let mut input = arguments
        .as_object()
        .cloned()
        .ok_or_else(|| ToolError::invalid_request("canonical arguments must be an object"))?;
    ensure_navigation_readiness(&mut input);
    let mechanism = if input.get("tab").is_some() {
        MechanismId::NavigateUrl
    } else {
        MechanismId::WorkspaceTabOpen
    };
    MechanismRequest::for_operation(
        OperationKind::BrowserNavigate,
        mechanism,
        Value::Object(input),
    )
}

fn direct_mechanism(
    operation: OperationKind,
    input: &mut Map<String, Value>,
) -> Result<MechanismId, ToolError> {
    use OperationKind as O;

    let id = match operation {
        O::BrowserOpenTab => {
            if input.get("url").is_some() {
                ensure_navigation_readiness(input);
                MechanismId::WorkspaceTabOpen
            } else {
                MechanismId::WorkspaceTabCreate
            }
        }
        O::BrowserNavigate => {
            ensure_navigation_readiness(input);
            MechanismId::NavigateUrl
        }
        O::BrowserListTabs => match input.get("create_if_empty") {
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
        O::BrowserFocusTab => MechanismId::TabFocus,
        O::BrowserCloseTab => MechanismId::TabClose,
        O::BrowserGoBack => {
            ensure_navigation_readiness(input);
            MechanismId::NavigateBack
        }
        O::BrowserGoForward => {
            ensure_navigation_readiness(input);
            MechanismId::NavigateForward
        }
        O::BrowserReloadPage => {
            ensure_navigation_readiness(input);
            MechanismId::NavigateReload
        }
        O::BrowserInspectPage if input.get("query").is_some() => MechanismId::PageFind,
        O::BrowserInspectPage => MechanismId::PageSnapshot,
        O::BrowserScrollPage => MechanismId::WheelScroll,
        O::BrowserPressEscape => MechanismId::KeyPress,
        O::BrowserGetDialog => MechanismId::DialogInspect,
        O::BrowserHandleDialog => match input.get("action").and_then(Value::as_str) {
            Some("dismiss") => MechanismId::DialogDismiss,
            Some("respond") => MechanismId::DialogRespond,
            _ => MechanismId::DialogAccept,
        },
        _ => {
            return Err(ToolError::invalid_request(format!(
                "operation {} requires its semantic handler",
                operation.as_str()
            )));
        }
    };
    Ok(id)
}

fn ensure_navigation_readiness(input: &mut Map<String, Value>) {
    input.entry("readiness").or_insert_with(|| {
        json!({
            "settle": true,
            "timeout_ms": 10_000,
            "min_ms": 0,
        })
    });
}
