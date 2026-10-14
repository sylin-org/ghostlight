//! One command over one protocol-4 connection.
//!
//! A command becomes a few primitives sent one after another. The Link sends each one, waits
//! for its receipt, forwards cancellation, honours the deadline, and keeps the facts the
//! receipts report: the URLs committed, the last tab read, and whether anything that can change
//! the browser has happened yet. That last fact decides every failure. Before an effect a
//! failure is plain and safe to retry; after one it is reported as an unknown effect, because
//! the browser may already have acted.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ghostlight_bridge::framing::write_native;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::connection::{ReceiptResult, Receipts};
use super::scope::{PlannedCall, Scope};
use super::wire::{self, primitive_name};
use super::world::{self, EffectClass};
use crate::browser::adapter::COMMAND_TRANSFER_MAX_BYTES;
use crate::browser::transport::{SocketWriter, DELIVERY_TIMEOUT};
use crate::browser::{
    lock, write_request_payload, BrowserDispatch, BrowserError, CONTROL_DELIVERY_TIMEOUT,
};

/// How long a cancelled primitive may take to report what it did.
pub const CANCEL_GRACE: Duration = Duration::from_millis(500);
/// How much earlier than the service deadline a primitive's own budget ends, so the extension
/// answers with facts before the service stops waiting.
pub const RECEIPT_MARGIN: Duration = Duration::from_millis(250);
/// How often a waiting primitive looks at cancellation and the deadline.
const POLL: Duration = Duration::from_millis(20);
/// The prefix of every protocol-4 correlation.
const CORRELATION_PREFIX: &str = "p4_";
/// The revision an extension that answers `unsupported` is told the primitive needs.
pub const UNSUPPORTED_REQUIRED: u16 = 1;
/// The revision an extension that answers `unsupported` is said to have.
pub const UNSUPPORTED_ADVERTISED: u16 = 0;
/// The failure for a tab the browser does not have.
const TAB_MISSING: &str = "browser tab is missing";
/// The unknown effect of a command that runtime control held after it had acted.
const HELD_MID_ACTION: &str = "held during a multi-step action";
/// The protocol failure of a receipt that is both an outcome and a failure, or neither.
const ONE_OF_OK_AND_FAILURE: &str = "receipt must carry exactly one of ok and failure";
/// The protocol failure of a `call` whose answers do not pair with its calls.
const ANSWERS_UNPAIRED: &str = "call answers do not pair with its calls";

/// What a primitive can do to the browser, which decides how a failure after it is reported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
    /// Changes nothing.
    Read,
    /// Can change the browser or a page.
    Effect,
}

/// One primitive the Link can send: its arguments, its outcome and its effect class.
pub trait Spec {
    /// The `ok` object of the primitive's receipt.
    type Outcome: DeserializeOwned;
    /// The primitive's wire name.
    const NAME: &'static str;
    /// The primitive's class.
    const CLASS: Class;
    /// Whether this request can change the browser. Most primitives answer by class; a few
    /// decide by their arguments.
    fn carries_effect(&self) -> bool {
        Self::CLASS == Class::Effect
    }

    /// The request as the wire carries it.
    fn into_primitive(self) -> wire::Primitive;
}

/// Implement [`Spec`] for an argument type that is its primitive's whole payload.
macro_rules! spec {
    ($arguments:ty, $outcome:ty, $name:expr, $class:expr, $variant:ident) => {
        impl Spec for $arguments {
            type Outcome = $outcome;
            const NAME: &'static str = $name;
            const CLASS: Class = $class;

            fn into_primitive(self) -> wire::Primitive {
                wire::Primitive::$variant(self)
            }
        }
    };
}

spec!(
    wire::ListTabs,
    wire::Tabs,
    primitive_name::LIST_TABS,
    Class::Read,
    ListTabs
);
spec!(
    wire::OpenTab,
    wire::TabOpened,
    primitive_name::OPEN_TAB,
    Class::Effect,
    OpenTab
);
spec!(
    wire::Navigate,
    wire::Navigated,
    primitive_name::NAVIGATE,
    Class::Effect,
    Navigate
);
spec!(
    wire::CloseTab,
    wire::TabClosed,
    primitive_name::CLOSE_TAB,
    Class::Effect,
    CloseTab
);
spec!(
    wire::SetZoom,
    wire::Zoomed,
    primitive_name::SET_ZOOM,
    Class::Effect,
    SetZoom
);
spec!(
    wire::ResizeWindow,
    wire::WindowResized,
    primitive_name::RESIZE_WINDOW,
    Class::Effect,
    ResizeWindow
);
spec!(
    wire::EvaluateMain,
    wire::Evaluated,
    primitive_name::EVALUATE_MAIN,
    Class::Effect,
    EvaluateMain
);
spec!(
    wire::Input,
    wire::InputDone,
    primitive_name::INPUT,
    Class::Effect,
    Input
);
spec!(
    wire::Capture,
    wire::Captured,
    primitive_name::CAPTURE,
    Class::Read,
    Capture
);
spec!(
    wire::DiagnosticsOp,
    wire::DiagnosticsFacts,
    primitive_name::DIAGNOSTICS,
    Class::Read,
    Diagnostics
);
spec!(
    wire::AccessibilitySnapshot,
    wire::AxSnapshot,
    primitive_name::ACCESSIBILITY_SNAPSHOT,
    Class::Read,
    AccessibilitySnapshot
);

/// `focus_tab` for one tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusTab(pub u64);

impl Spec for FocusTab {
    type Outcome = wire::TabFocused;
    const NAME: &'static str = primitive_name::FOCUS_TAB;
    const CLASS: Class = Class::Effect;

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::FocusTab(wire::TabRef { tab_id: self.0 })
    }
}

/// `frames` for one tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Frames(pub u64);

impl Spec for Frames {
    type Outcome = wire::FrameInventory;
    const NAME: &'static str = primitive_name::FRAMES;
    const CLASS: Class = Class::Read;

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::Frames(wire::TabRef { tab_id: self.0 })
    }
}

/// `view` for one tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View(pub u64);

impl Spec for View {
    type Outcome = wire::ViewFacts;
    const NAME: &'static str = primitive_name::VIEW;
    const CLASS: Class = Class::Read;

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::View(wire::TabRef { tab_id: self.0 })
    }
}

/// `call`, with whether any of its world calls belongs to the effect class.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldCalls {
    /// The arguments.
    pub call: wire::Call,
    /// Whether any call is of the effect class.
    pub effectful: bool,
}

impl Spec for WorldCalls {
    type Outcome = wire::Called;
    const NAME: &'static str = primitive_name::CALL;
    const CLASS: Class = Class::Effect;

    fn carries_effect(&self) -> bool {
        self.effectful
    }

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::Call(self.call)
    }
}

impl Spec for wire::Dialog {
    type Outcome = wire::DialogFacts;
    const NAME: &'static str = primitive_name::DIALOG;
    const CLASS: Class = Class::Read;

    /// Reading a dialog's status changes nothing; handling it does.
    fn carries_effect(&self) -> bool {
        matches!(self.op, wire::DialogOp::Handle { .. })
    }

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::Dialog(self)
    }
}

impl Spec for wire::RecordingOp {
    type Outcome = wire::RecordingFacts;
    const NAME: &'static str = primitive_name::RECORDING;
    const CLASS: Class = Class::Read;

    /// Only an export into a page changes anything.
    fn carries_effect(&self) -> bool {
        matches!(
            self,
            Self::Export {
                destination: wire::RecordingTarget::Page { .. },
                ..
            }
        )
    }

    fn into_primitive(self) -> wire::Primitive {
        wire::Primitive::Recording(self)
    }
}

/// The answers of one `call`, in the order of its calls.
pub type Answers = Vec<wire::CallAnswer>;

/// The recordings each workspace started on one connection.
///
/// The extension keeps recordings without knowing workspaces. The service remembers which
/// workspace started each one, so a workspace reaches only its own recordings and an omitted id
/// can resolve to the only one it has.
#[derive(Debug, Default)]
pub struct RecordingIndex {
    by_workspace: Mutex<HashMap<String, Vec<String>>>,
}

impl RecordingIndex {
    /// Remember that `workspace` started `recording_id`.
    pub fn learn(&self, workspace: &str, recording_id: &str) {
        let mut index = lock(&self.by_workspace);
        let ids = index.entry(workspace.to_owned()).or_default();
        if !ids.iter().any(|id| id == recording_id) {
            ids.push(recording_id.to_owned());
        }
    }

    /// Forget one recording of `workspace`.
    pub fn forget(&self, workspace: &str, recording_id: &str) {
        let mut index = lock(&self.by_workspace);
        if let Some(ids) = index.get_mut(workspace) {
            ids.retain(|id| id != recording_id);
            if ids.is_empty() {
                index.remove(workspace);
            }
        }
    }

    /// The recordings `workspace` started, oldest first.
    #[must_use]
    pub fn ids(&self, workspace: &str) -> Vec<String> {
        lock(&self.by_workspace)
            .get(workspace)
            .cloned()
            .unwrap_or_default()
    }
}

/// What a protocol-4 connection lends each command it carries.
#[derive(Clone, Debug)]
pub struct Line {
    /// The connection's writer.
    pub writer: Arc<SocketWriter>,
    /// The calls awaiting receipts on the connection.
    pub receipts: Arc<Receipts>,
    /// The recordings each workspace started on the connection.
    pub recordings: Arc<RecordingIndex>,
}

/// One command's primitives over one connection, and the facts their receipts reported.
pub struct Link<'a> {
    line: Line,
    workspace: &'a str,
    dispatch: BrowserDispatch<'a>,
    effect_started: bool,
    committed: Vec<String>,
    last_tab: Option<wire::Tab>,
}

impl<'a> Link<'a> {
    /// Carry one command of `workspace` over `line` under the caller's dispatch controls.
    #[must_use]
    pub fn new(line: Line, workspace: &'a str, dispatch: BrowserDispatch<'a>) -> Self {
        Self {
            line,
            workspace,
            dispatch,
            effect_started: false,
            committed: Vec::new(),
            last_tab: None,
        }
    }

    /// The workspace the command belongs to.
    #[must_use]
    pub const fn workspace(&self) -> &str {
        self.workspace
    }

    /// The recordings each workspace started on this connection.
    #[must_use]
    pub fn recordings(&self) -> &RecordingIndex {
        &self.line.recordings
    }

    /// Every top-frame URL the command's receipts reported committed, in order, each once.
    #[must_use]
    pub fn committed(&self) -> Vec<String> {
        self.committed.clone()
    }

    /// Whether anything the command sent may already have changed the browser.
    #[must_use]
    pub const fn effect_started(&self) -> bool {
        self.effect_started
    }

    /// The time a primitive may still take, in milliseconds: what is left before the deadline,
    /// less the margin its receipt needs to arrive in time.
    #[must_use]
    pub fn budget_ms(&self) -> u64 {
        let left = self
            .dispatch
            .deadline
            .saturating_duration_since(Instant::now())
            .saturating_sub(RECEIPT_MARGIN);
        u64::try_from(left.as_millis()).unwrap_or(u64::MAX)
    }

    /// Send one primitive and return its outcome.
    ///
    /// Before anything is written the Link checks cancellation and the deadline, and for an
    /// effectful primitive asks runtime control to admit it. While it waits, a cancellation is
    /// forwarded and the receipt gets a short grace to report what happened; a deadline is
    /// forwarded and ends the wait at once.
    ///
    /// # Errors
    ///
    /// Returns the receipt's failure mapped to a [`BrowserError`]. Once anything in the command
    /// may have changed the browser, every failure says the effect is unknown.
    pub fn send<P: Spec>(&mut self, primitive: P) -> Result<P::Outcome, BrowserError> {
        let (outcome, dispatched) = self.exchange(primitive)?;
        self.effect_started |= dispatched;
        Ok(outcome)
    }

    /// Send one primitive and return its outcome with whether its receipt says an effectful
    /// primitive dispatched. Every failure has already counted that effect; a successful outcome
    /// leaves it to the caller, which may judge it more finely from the outcome itself.
    fn exchange<P: Spec>(&mut self, primitive: P) -> Result<(P::Outcome, bool), BrowserError> {
        let effectful = primitive.carries_effect();
        self.ready()?;
        let correlation = format!("{CORRELATION_PREFIX}{}", Uuid::new_v4().simple());
        let payload = serde_json::to_vec(&wire::Outbound::Request {
            correlation: correlation.clone(),
            primitive: primitive.into_primitive(),
        })
        .map_err(|error| self.settle(BrowserError::Protocol(error.to_string())))?;
        if payload.len() > COMMAND_TRANSFER_MAX_BYTES {
            return Err(self.settle(BrowserError::Primitive(format!(
                "browser request exceeds the {COMMAND_TRANSFER_MAX_BYTES}-byte transfer bound"
            ))));
        }
        let writer = Arc::clone(&self.line.writer);
        let mut output = writer
            .until_unless(
                self.dispatch
                    .deadline
                    .min(Instant::now() + DELIVERY_TIMEOUT),
                || self.cancel_requested(),
            )
            .map_err(|_| self.unwritten())?;
        self.ready()?;
        if effectful {
            (self.dispatch.admit)().map_err(|error| self.settle(error))?;
        }
        let receiver = self.line.receipts.register(&correlation);
        if write_request_payload(&mut output, &payload, &correlation).is_err() {
            drop(output);
            self.line.receipts.forget(&correlation);
            // A frame cut off midway leaves the stream unreadable for every later frame.
            writer.close();
            return Err(self.lost(effectful));
        }
        drop(output);
        self.await_receipt::<P>(&correlation, &receiver, effectful)
    }

    /// Run world calls in one `call` and return their answers, in order.
    ///
    /// Each answer is recorded in `scope` when there is one, and an effect-class call whose
    /// answer shows it ran marks the command as having acted. `budget_ms` bounds the call, and
    /// never exceeds what the deadline leaves.
    ///
    /// # Errors
    ///
    /// Returns the receipt's failure, or [`BrowserError::Protocol`] when the answers do not pair
    /// with the calls. Answers that do not pair cannot say which calls ran, so the receipt's
    /// `dispatched` flag decides whether that failure follows an effect.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per field of the call primitive, plus the scope that records it"
    )]
    pub fn call(
        &mut self,
        tab_id: u64,
        plans: &[PlannedCall],
        mode: wire::CallMode,
        guard: Option<wire::DocumentGuard>,
        settle_ms: u64,
        budget_ms: u64,
        mut scope: Option<&mut Scope>,
    ) -> Result<Answers, BrowserError> {
        let (called, dispatched) = self.exchange(WorldCalls {
            call: wire::Call {
                tab_id,
                mode,
                calls: plans.iter().map(|plan| plan.call.clone()).collect(),
                guard,
                settle_ms,
                timeout_ms: budget_ms.min(self.budget_ms()),
            },
            effectful: plans.iter().any(|plan| plan.effect == EffectClass::Effect),
        })?;
        if called.answers.len() != plans.len() {
            self.effect_started |= dispatched;
            return Err(self.settle(BrowserError::Protocol(ANSWERS_UNPAIRED.into())));
        }
        for (plan, answer) in plans.iter().zip(&called.answers) {
            if let Some(scope) = scope.as_deref_mut() {
                scope.record(plan, answer);
            }
            if plan.effect == EffectClass::Effect && acted(answer) {
                self.effect_started = true;
            }
        }
        Ok(called.answers)
    }

    /// Dispatch input steps, bounded by what the deadline leaves.
    ///
    /// # Errors
    ///
    /// Returns the receipt's failure.
    pub fn input(&mut self, mut input: wire::Input) -> Result<wire::InputDone, BrowserError> {
        input.timeout_ms = input.timeout_ms.min(self.budget_ms());
        self.send(input)
    }

    /// The tab as the browser last reported it: from the command's last receipt when it named
    /// this tab, and otherwise from one tab listing.
    ///
    /// # Errors
    ///
    /// Returns a plain failure when the browser does not have the tab.
    pub fn tab(&mut self, tab_id: u64) -> Result<wire::Tab, BrowserError> {
        if let Some(tab) = self.last_tab.as_ref().filter(|tab| tab.tab_id == tab_id) {
            return Ok(tab.clone());
        }
        let listed = self.send(wire::ListTabs {
            tab_ids: Some(vec![tab_id]),
        })?;
        listed
            .tabs
            .into_iter()
            .find(|tab| tab.tab_id == tab_id)
            .ok_or_else(|| self.settle(BrowserError::Primitive(TAB_MISSING.into())))
    }

    /// Write one presentation notice. Nothing answers it.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::BusyBeforeDispatch`] when other work holds the writer for the
    /// whole control budget, and [`BrowserError::DisconnectedAfterDispatch`] when the write
    /// fails.
    pub fn notice(&self, notice: wire::Notice) -> Result<(), BrowserError> {
        let mut output = self
            .line
            .writer
            .until(Instant::now() + CONTROL_DELIVERY_TIMEOUT)
            .map_err(|_| BrowserError::BusyBeforeDispatch)?;
        write_native(&mut output, &wire::Outbound::Notice(notice))
            .map_err(|_| BrowserError::DisconnectedAfterDispatch)
    }

    /// Ask the extension to interrupt the primitive with `correlation`. Best effort: a writer
    /// busy with another transfer is left alone rather than closed.
    pub fn cancel(&self, correlation: &str) {
        if let Ok(mut output) = self
            .line
            .writer
            .until(Instant::now() + CONTROL_DELIVERY_TIMEOUT)
        {
            let _ = write_native(
                &mut output,
                &wire::Outbound::Cancel {
                    correlation: correlation.into(),
                },
            );
        }
    }

    /// Wait for one written primitive's receipt.
    fn await_receipt<P: Spec>(
        &mut self,
        correlation: &str,
        receiver: &Receiver<ReceiptResult>,
        effectful: bool,
    ) -> Result<(P::Outcome, bool), BrowserError> {
        loop {
            if self.cancel_requested() {
                self.cancel(correlation);
                let until = (Instant::now() + CANCEL_GRACE).min(self.dispatch.deadline);
                let answered =
                    receiver.recv_timeout(until.saturating_duration_since(Instant::now()));
                self.line.receipts.forget(correlation);
                let acted = match answered {
                    Ok(Ok(receipt)) => {
                        self.keep(&receipt);
                        effectful && receipt.dispatched
                    }
                    _ => effectful,
                };
                self.effect_started |= acted;
                return Err(if self.effect_started {
                    BrowserError::CancelledAfterDispatch
                } else {
                    BrowserError::CancelledBeforeDispatch
                });
            }
            let now = Instant::now();
            if now >= self.dispatch.deadline {
                self.cancel(correlation);
                self.line.receipts.forget(correlation);
                self.effect_started |= effectful;
                return Err(if self.effect_started {
                    BrowserError::DeadlineAfterDispatch
                } else {
                    BrowserError::DeadlineBeforeDispatch
                });
            }
            match receiver.recv_timeout(POLL.min(self.dispatch.deadline - now)) {
                Ok(Ok(receipt)) => return self.finish::<P>(receipt, effectful),
                Ok(Err(error)) => {
                    self.effect_started |= effectful;
                    return Err(self.settle(error));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(BrowserError::DisconnectedAfterDispatch)
                }
            }
        }
    }

    /// Turn one receipt into the primitive's outcome, with whether an effectful primitive
    /// dispatched, or into its failure, which counts that effect.
    fn finish<P: Spec>(
        &mut self,
        receipt: wire::Receipt,
        effectful: bool,
    ) -> Result<(P::Outcome, bool), BrowserError> {
        self.keep(&receipt);
        let dispatched = effectful && receipt.dispatched;
        let decoded = match (receipt.ok, receipt.failure) {
            (Some(ok), None) => serde_json::from_value(ok)
                .map_err(|error| BrowserError::Protocol(error.to_string())),
            (None, Some(failure)) => Err(Self::failure::<P>(failure, receipt.message)),
            _ => Err(BrowserError::Protocol(ONE_OF_OK_AND_FAILURE.into())),
        };
        decoded
            .map(|outcome| (outcome, dispatched))
            .map_err(|error| {
                self.effect_started |= dispatched;
                self.settle(error)
            })
    }

    /// Keep a receipt's facts: its commits, unioned in order without duplicates, and its tab.
    fn keep(&mut self, receipt: &wire::Receipt) {
        for url in &receipt.committed_urls {
            if !self.committed.contains(url) {
                self.committed.push(url.clone());
            }
        }
        if let Some(tab) = &receipt.tab {
            self.last_tab = Some(tab.clone());
        }
    }

    /// The error a receipt's failure code stands for.
    fn failure<P: Spec>(failure: wire::Failure, message: Option<String>) -> BrowserError {
        let message = message.unwrap_or_else(|| failure_code(failure));
        match failure {
            wire::Failure::LocalInterlock => BrowserError::LocalInterlock(message),
            wire::Failure::DocumentChanged => BrowserError::DocumentUnavailable,
            wire::Failure::Cancelled => BrowserError::CancelledBeforeDispatch,
            wire::Failure::Timeout => BrowserError::DeadlineBeforeDispatch,
            wire::Failure::TabMissing => BrowserError::Primitive(TAB_MISSING.into()),
            wire::Failure::Unsupported => BrowserError::CapabilityVersion {
                capability: P::NAME.into(),
                required: UNSUPPORTED_REQUIRED,
                advertised: UNSUPPORTED_ADVERTISED,
            },
            wire::Failure::InvalidRequest => BrowserError::Protocol(message),
            wire::Failure::DebuggerUnavailable
            | wire::Failure::BrowserRefused
            | wire::Failure::DialogOpen
            | wire::Failure::CaptureTooLarge
            | wire::Failure::SessionLost
            | wire::Failure::Other => BrowserError::Primitive(message),
        }
    }

    /// Restate an error once the command may have acted: an interruption becomes its after
    /// variant, and any other failure an unknown effect.
    fn settle(&self, error: BrowserError) -> BrowserError {
        if !self.effect_started || error.effect_unknown() {
            return error;
        }
        match error {
            BrowserError::CancelledBeforeDispatch => BrowserError::CancelledAfterDispatch,
            BrowserError::DeadlineBeforeDispatch => BrowserError::DeadlineAfterDispatch,
            BrowserError::DisconnectedBeforeDispatch => BrowserError::DisconnectedAfterDispatch,
            BrowserError::RuntimeControl(_) => BrowserError::EffectUnknown(HELD_MID_ACTION.into()),
            BrowserError::Primitive(message)
            | BrowserError::LocalInterlock(message)
            | BrowserError::Protocol(message) => BrowserError::EffectUnknown(message),
            other => BrowserError::EffectUnknown(other.to_string()),
        }
    }

    /// Fail before writing when the command was cancelled or its deadline passed.
    fn ready(&self) -> Result<(), BrowserError> {
        if self.cancel_requested() {
            return Err(self.settle(BrowserError::CancelledBeforeDispatch));
        }
        if Instant::now() >= self.dispatch.deadline {
            return Err(self.settle(BrowserError::DeadlineBeforeDispatch));
        }
        Ok(())
    }

    /// Why the writer could not be had: cancellation, the deadline, or other work holding it.
    fn unwritten(&self) -> BrowserError {
        self.ready()
            .err()
            .unwrap_or_else(|| self.settle(BrowserError::BusyBeforeDispatch))
    }

    /// The error of a write that failed. A read could not have acted; an effect may have
    /// reached the extension before the socket failed.
    fn lost(&self, effectful: bool) -> BrowserError {
        if effectful || self.effect_started {
            BrowserError::DisconnectedAfterDispatch
        } else {
            BrowserError::DisconnectedBeforeDispatch
        }
    }

    fn cancel_requested(&self) -> bool {
        self.dispatch.cancelled.load(Ordering::SeqCst)
    }
}

/// Whether an effect-class world call's answer shows it may have changed the page: it
/// succeeded, it failed with a code that can follow a mutation, or it failed after it started.
fn acted(answer: &wire::CallAnswer) -> bool {
    match answer {
        wire::CallAnswer::Returned { .. } => match world::decode_answer(answer) {
            Some(Err(error)) => !error.code.is_pre_effect(),
            Some(Ok(_)) | None => true,
        },
        wire::CallAnswer::Failed { started, .. } => *started,
        wire::CallAnswer::Skipped => false,
        wire::CallAnswer::Unknown => true,
    }
}

/// A failure code as the wire spells it, for a failure that came without a message.
fn failure_code(failure: wire::Failure) -> String {
    serde_json::to_value(failure)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
