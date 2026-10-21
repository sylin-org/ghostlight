//! One command over one protocol-4 connection.
//!
//! A command becomes a few primitives sent one after another. The Link sends each one, waits
//! for its receipt, forwards cancellation, honours the deadline, and keeps the facts the
//! receipts report: the URLs committed, the last tab read, and whether anything that can change
//! the browser has happened yet. That last fact decides every failure. Before an effect a
//! failure is plain and safe to retry; after one it is reported as an unknown effect, because
//! the browser may already have acted.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use ghostlight_relay::frame::{write_frame, write_frame_raw, FrameError};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::connection::{ReceiptResult, Receipts};
use super::scope::{PlannedCall, Scope};
use super::wire::{self, PrimitiveName};
use super::world::{self, EffectClass};
use crate::browser::transport::{SocketWriter, DELIVERY_TIMEOUT};
use crate::browser::{lock, BrowserDispatch, BrowserError, CONTROL_DELIVERY_TIMEOUT};
use crate::workspace::StaleReason;

/// How long a cancelled primitive may take to report what it did.
pub const CANCEL_GRACE: Duration = Duration::from_millis(500);
/// How much earlier than the service deadline a primitive's own budget ends, so the extension
/// answers with facts before the service stops waiting.
pub const RECEIPT_MARGIN: Duration = Duration::from_millis(250);
/// How often a waiting primitive looks at cancellation and the deadline.
const POLL: Duration = Duration::from_millis(20);
/// The prefix of every protocol-4 correlation.
const CORRELATION_PREFIX: &str = "p4_";
/// The prefix of every chunked transfer's identity.
const TRANSFER_PREFIX: &str = "transfer_";
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
    const NAME: PrimitiveName;
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
            const NAME: PrimitiveName = $name;
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
    PrimitiveName::ListTabs,
    Class::Read,
    ListTabs
);
spec!(
    wire::OpenTab,
    wire::TabOpened,
    PrimitiveName::OpenTab,
    Class::Effect,
    OpenTab
);
spec!(
    wire::Navigate,
    wire::Navigated,
    PrimitiveName::Navigate,
    Class::Effect,
    Navigate
);
spec!(
    wire::CloseTab,
    wire::TabClosed,
    PrimitiveName::CloseTab,
    Class::Effect,
    CloseTab
);
spec!(
    wire::SetZoom,
    wire::Zoomed,
    PrimitiveName::SetZoom,
    Class::Effect,
    SetZoom
);
spec!(
    wire::ResizeWindow,
    wire::WindowResized,
    PrimitiveName::ResizeWindow,
    Class::Effect,
    ResizeWindow
);
spec!(
    wire::EvaluateMain,
    wire::Evaluated,
    PrimitiveName::EvaluateMain,
    Class::Effect,
    EvaluateMain
);
spec!(
    wire::Input,
    wire::InputDone,
    PrimitiveName::Input,
    Class::Effect,
    Input
);
spec!(
    wire::Capture,
    wire::Captured,
    PrimitiveName::Capture,
    Class::Read,
    Capture
);
spec!(
    wire::DiagnosticsOp,
    wire::DiagnosticsFacts,
    PrimitiveName::Diagnostics,
    Class::Read,
    Diagnostics
);
spec!(
    wire::AccessibilitySnapshot,
    wire::AxSnapshot,
    PrimitiveName::AccessibilitySnapshot,
    Class::Read,
    AccessibilitySnapshot
);

/// `focus_tab` for one tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusTab(pub u64);

impl Spec for FocusTab {
    type Outcome = wire::TabFocused;
    const NAME: PrimitiveName = PrimitiveName::FocusTab;
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
    const NAME: PrimitiveName = PrimitiveName::Frames;
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
    const NAME: PrimitiveName = PrimitiveName::View;
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
    const NAME: PrimitiveName = PrimitiveName::Call;
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
    const NAME: PrimitiveName = PrimitiveName::Dialog;
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
    const NAME: PrimitiveName = PrimitiveName::Recording;
    const CLASS: Class = Class::Read;

    /// An export into a page attaches a file, and an export to Downloads saves one. Either
    /// changes the browser, so a receipt lost after it was sent leaves the save unknown.
    fn carries_effect(&self) -> bool {
        matches!(
            self,
            Self::Export {
                destination: wire::RecordingTarget::Page { .. }
                    | wire::RecordingTarget::Download { .. },
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

/// One `call` to send: a tab's world calls, how the extension runs them, the currency check that
/// rides on them, how long the page may settle after them, and how long they may take.
///
/// A request with no budget of its own is bounded by what the deadline leaves, and one with a
/// budget is bounded by the smaller of the two.
#[derive(Clone, Debug)]
pub struct CallRequest<'p> {
    tab_id: u64,
    plans: &'p [PlannedCall],
    mode: wire::CallMode,
    guard: Option<wire::DocumentGuard>,
    settle_ms: u64,
    budget_ms: Option<u64>,
}

impl<'p> CallRequest<'p> {
    /// Run `plans` in `tab_id` as `mode` says, with no guard and no settle.
    #[must_use]
    pub fn new(tab_id: u64, plans: &'p [PlannedCall], mode: wire::CallMode) -> Self {
        Self {
            tab_id,
            plans,
            mode,
            guard: None,
            settle_ms: 0,
            budget_ms: None,
        }
    }

    /// Run `plans` in `tab_id` one after another.
    #[must_use]
    pub fn sequence(tab_id: u64, plans: &'p [PlannedCall]) -> Self {
        Self::new(tab_id, plans, wire::CallMode::Sequence)
    }

    /// Run `plans` in `tab_id` together.
    #[must_use]
    pub fn parallel(tab_id: u64, plans: &'p [PlannedCall]) -> Self {
        Self::new(tab_id, plans, wire::CallMode::Parallel)
    }

    /// Carry `guard`, so the extension refuses the calls if their documents changed.
    #[must_use]
    pub fn guard(mut self, guard: wire::DocumentGuard) -> Self {
        self.guard = Some(guard);
        self
    }

    /// Let the page settle for up to `settle_ms` after the calls.
    #[must_use]
    pub fn settle(mut self, settle_ms: u64) -> Self {
        self.settle_ms = settle_ms;
        self
    }

    /// Bound the calls by `budget_ms` when that is less than what the deadline leaves.
    #[must_use]
    pub fn budget(mut self, budget_ms: u64) -> Self {
        self.budget_ms = Some(budget_ms);
        self
    }

    /// Whether the request holds no world call.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}

/// The recording each workspace holds on one connection.
///
/// The extension keeps recordings without knowing workspaces, and a workspace holds at most one.
/// The service remembers which recording is whose, so a workspace reaches only its own and never
/// has to name it. It also remembers whether the recording has been saved, because a saved
/// recording may be replaced by the next start while a recording never saved may not.
#[derive(Debug, Default)]
pub struct RecordingIndex {
    by_workspace: Mutex<HashMap<String, HeldRecording>>,
}

/// One workspace's recording and whether a save has delivered it.
#[derive(Debug)]
struct HeldRecording {
    id: String,
    saved: bool,
}

impl RecordingIndex {
    /// Remember that `workspace` holds `recording_id`, not yet saved.
    pub fn learn(&self, workspace: &str, recording_id: &str) {
        lock(&self.by_workspace).insert(
            workspace.to_owned(),
            HeldRecording {
                id: recording_id.to_owned(),
                saved: false,
            },
        );
    }

    /// Remember that a save delivered the recording `workspace` holds.
    pub fn mark_saved(&self, workspace: &str) {
        if let Some(held) = lock(&self.by_workspace).get_mut(workspace) {
            held.saved = true;
        }
    }

    /// Forget the recording `workspace` held.
    pub fn forget(&self, workspace: &str) {
        lock(&self.by_workspace).remove(workspace);
    }

    /// The recording `workspace` holds, if any.
    #[must_use]
    pub fn id(&self, workspace: &str) -> Option<String> {
        lock(&self.by_workspace)
            .get(workspace)
            .map(|held| held.id.clone())
    }

    /// Whether a save has delivered the recording `workspace` holds.
    #[must_use]
    pub fn is_saved(&self, workspace: &str) -> bool {
        lock(&self.by_workspace)
            .get(workspace)
            .is_some_and(|held| held.saved)
    }
}

/// What a protocol-4 connection lends each command it carries.
#[derive(Clone, Debug)]
pub struct Line {
    /// The connection's writer.
    pub writer: Arc<SocketWriter>,
    /// The calls awaiting receipts on the connection.
    pub receipts: Arc<Receipts>,
    /// The recording each workspace holds on the connection.
    pub recordings: Arc<RecordingIndex>,
}

/// One command's primitives over one connection, and the facts their receipts reported.
pub struct Link<'a> {
    line: Line,
    workspace: &'a str,
    dispatch: BrowserDispatch<'a>,
    effect_started: bool,
    refused_invalid: bool,
    one_locator: bool,
    committed: Vec<String>,
    last_tab: Option<wire::Tab>,
}

/// What a primitive that may be refused as invalid came back with.
#[derive(Debug)]
pub enum Answer<T> {
    /// The primitive ran, and this is its outcome.
    Done(T),
    /// The adapter refused the request as invalid before it did anything. Nothing ran, so the
    /// caller may send a different request.
    Refused,
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
            refused_invalid: false,
            one_locator: false,
            committed: Vec::new(),
            last_tab: None,
        }
    }

    /// Note that the command names exactly one locator.
    ///
    /// Its documents are then that locator's document and the documents above it, so a
    /// receipt saying a document changed means the locator's document was replaced or removed,
    /// and the failure is a stale locator that navigated.
    pub fn names_one_locator(&mut self) {
        self.one_locator = true;
    }

    /// The workspace the command belongs to.
    #[must_use]
    pub const fn workspace(&self) -> &str {
        self.workspace
    }

    /// The recording each workspace holds on this connection.
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

    /// Send one primitive like [`Self::send`], except that a receipt refusing the request as
    /// invalid before anything was dispatched is an [`Answer::Refused`] and not a failure.
    ///
    /// The refusal is the adapter's own check of the request, made before it acquires the tab,
    /// so a caller that sends an alternative afterwards still has the command's single effect.
    ///
    /// # Errors
    ///
    /// Returns every other failure exactly as [`Self::send`] does.
    pub fn send_unless_refused<P: Spec>(
        &mut self,
        primitive: P,
    ) -> Result<Answer<P::Outcome>, BrowserError> {
        match self.send(primitive) {
            Ok(outcome) => Ok(Answer::Done(outcome)),
            Err(_) if self.refused_invalid && !self.effect_started => Ok(Answer::Refused),
            Err(error) => Err(error),
        }
    }

    /// Send one primitive and return its outcome with whether its receipt says an effectful
    /// primitive dispatched. Every failure has already counted that effect; a successful outcome
    /// leaves it to the caller, which may judge it more finely from the outcome itself.
    fn exchange<P: Spec>(&mut self, primitive: P) -> Result<(P::Outcome, bool), BrowserError> {
        let effectful = primitive.carries_effect();
        self.refused_invalid = false;
        self.ready()?;
        let correlation = format!("{CORRELATION_PREFIX}{}", Uuid::new_v4().simple());
        let payload = serde_json::to_vec(&wire::Outbound::Request {
            correlation: correlation.clone(),
            primitive: primitive.into_primitive(),
        })
        .map_err(|error| self.settle(BrowserError::Protocol(error.to_string())))?;
        if payload.len() > wire::TRANSFER_MAX_BYTES {
            return Err(self.settle(BrowserError::Primitive(format!(
                "browser request exceeds the {}-byte transfer bound",
                wire::TRANSFER_MAX_BYTES
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
        self.mark_written();
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

    /// Run `request`'s world calls in one `call` and return their answers, in order.
    ///
    /// Each answer is recorded in `scope` when there is one, and an effect-class call whose
    /// answer shows it ran marks the command as having acted. The call never takes longer than
    /// what the deadline leaves.
    ///
    /// # Errors
    ///
    /// Returns the receipt's failure, [`BrowserError::DialogOpen`] when a dialog refused every call
    /// before it started, or [`BrowserError::Protocol`] when the answers do not pair with the
    /// calls. Answers that do not pair cannot say which calls ran, so the receipt's
    /// `dispatched` flag decides whether that failure follows an effect.
    pub fn call(
        &mut self,
        request: CallRequest<'_>,
        mut scope: Option<&mut Scope>,
    ) -> Result<Answers, BrowserError> {
        let plans = request.plans;
        let left = self.budget_ms();
        let (called, dispatched) = self.exchange(WorldCalls {
            call: wire::Call {
                tab_id: request.tab_id,
                mode: request.mode,
                calls: plans.iter().map(|plan| plan.call.clone()).collect(),
                guard: request.guard,
                settle_ms: request.settle_ms,
                timeout_ms: request.budget_ms.map_or(left, |budget| budget.min(left)),
            },
            effectful: plans.iter().any(|plan| plan.effect == EffectClass::Effect),
        })?;
        if called.answers.len() != plans.len() {
            self.effect_started |= dispatched;
            return Err(self.settle(BrowserError::Protocol(ANSWERS_UNPAIRED.into())));
        }
        if !called.answers.is_empty() && called.answers.iter().all(refused_by_dialog) {
            // The page was never entered: a dialog blocks it, and no document can be read.
            return Err(self.settle(BrowserError::DialogOpen));
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
    /// A successful answer counts as an effect from its own facts, not from the receipt's
    /// `dispatched` flag, which is also set when only the precondition answered: a step was
    /// dispatched, a dialog or an unknown reason stopped the steps, or the precondition, of the
    /// class `precondition`, shows it changed the page. A precondition the page refused
    /// before any of that is a plain failure.
    ///
    /// # Errors
    ///
    /// Returns the receipt's failure.
    pub fn input(
        &mut self,
        mut input: wire::Input,
        precondition: EffectClass,
    ) -> Result<wire::InputDone, BrowserError> {
        input.timeout_ms = input.timeout_ms.min(self.budget_ms());
        let (done, _) = self.exchange(input)?;
        self.effect_started |= input_acted(&done, precondition);
        Ok(done)
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
        self.mark_written();
        write_frame(&mut output, &wire::Outbound::Notice(notice))
            .map_err(|_| BrowserError::DisconnectedAfterDispatch)
    }

    /// Tell the caller that the command reached the browser: a frame of it is being written.
    fn mark_written(&self) {
        self.dispatch.written.store(true, Ordering::SeqCst);
    }

    /// Ask the extension to interrupt the primitive with `correlation`. Best effort: a writer
    /// busy with another transfer is left alone rather than closed.
    pub fn cancel(&self, correlation: &str) {
        if let Ok(mut output) = self
            .line
            .writer
            .until(Instant::now() + CONTROL_DELIVERY_TIMEOUT)
        {
            let _ = write_frame(
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
        self.refused_invalid =
            receipt.failure == Some(wire::Failure::InvalidRequest) && !dispatched;
        let decoded = match (receipt.ok, receipt.failure) {
            (Some(ok), None) => serde_json::from_value(ok)
                .map_err(|error| BrowserError::Protocol(error.to_string())),
            (None, Some(failure)) => Err(self.failure::<P>(failure, receipt.message)),
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

    /// The error a receipt's failure code stands for. A document change is a stale locator for
    /// a command that names one locator, and an unavailable document otherwise.
    fn failure<P: Spec>(&self, failure: wire::Failure, message: Option<String>) -> BrowserError {
        let message = message.unwrap_or_else(|| failure_code(failure));
        match failure {
            wire::Failure::LocalInterlock => BrowserError::LocalInterlock(message),
            wire::Failure::DocumentChanged if self.one_locator => {
                BrowserError::stale(StaleReason::Navigated)
            }
            wire::Failure::DocumentChanged => BrowserError::DocumentUnavailable,
            wire::Failure::Cancelled => BrowserError::CancelledBeforeDispatch,
            wire::Failure::Timeout => BrowserError::DeadlineBeforeDispatch,
            wire::Failure::TabMissing => BrowserError::Primitive(TAB_MISSING.into()),
            wire::Failure::DialogOpen => BrowserError::DialogOpen,
            wire::Failure::NoHistoryEntry => BrowserError::NoHistoryEntry,
            wire::Failure::Unsupported => BrowserError::Unsupported {
                primitive: P::NAME.as_str().into(),
            },
            wire::Failure::InvalidRequest => BrowserError::Protocol(message),
            wire::Failure::DebuggerUnavailable
            | wire::Failure::BrowserRefused
            | wire::Failure::CaptureTooLarge
            | wire::Failure::SessionLost
            | wire::Failure::Other => BrowserError::Primitive(message),
        }
    }

    /// Restate an error once the command may have acted: an interruption becomes its after
    /// variant, and any other failure an unknown effect. Before any effect the error stands.
    #[must_use]
    pub fn settle(&self, error: BrowserError) -> BrowserError {
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
            BrowserError::PageRefused(refusal) => {
                BrowserError::EffectUnknown(refusal.as_str().into())
            }
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

/// Whether a world call's answer is a refusal because a JavaScript dialog blocks the page, before
/// the call started.
fn refused_by_dialog(answer: &wire::CallAnswer) -> bool {
    matches!(
        answer,
        wire::CallAnswer::Failed {
            code: wire::CallFailure::DialogOpen,
            started: false,
            ..
        }
    )
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

/// Whether input that was answered changed the page: a step was dispatched, a dialog or an
/// unknown reason stopped it, or an effect-class precondition shows it ran.
fn input_acted(done: &wire::InputDone, precondition: EffectClass) -> bool {
    done.completed_steps > 0
        || matches!(
            done.stopped,
            Some(wire::InputStop::Dialog { .. } | wire::InputStop::Other)
        )
        || (precondition == EffectClass::Effect && done.precondition.as_ref().is_some_and(acted))
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

/// Write one serialized request: whole when it fits one frame, otherwise as command chunks that
/// each carry the transfer's length and hash.
///
/// # Errors
///
/// Returns [`FrameError::TooLarge`] when the request exceeds the transfer bound, and the
/// writer's failure.
fn write_request_payload(
    writer: &mut impl std::io::Write,
    payload: &[u8],
    correlation: &str,
) -> Result<(), FrameError> {
    if payload.len() <= wire::DIRECT_FRAME_BYTES {
        return write_frame_raw(writer, payload);
    }
    if payload.len() > wire::TRANSFER_MAX_BYTES {
        return Err(FrameError::TooLarge);
    }
    let count = u16::try_from(payload.len().div_ceil(wire::CHUNK_PAYLOAD_BYTES))
        .map_err(|_| FrameError::TooLarge)?;
    if count > wire::TRANSFER_MAX_CHUNKS {
        return Err(FrameError::TooLarge);
    }
    let total_bytes = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge)?;
    let transfer_id = format!("{TRANSFER_PREFIX}{}", Uuid::new_v4().simple());
    let mut sha256 = String::with_capacity(64);
    for byte in Sha256::digest(payload) {
        write!(&mut sha256, "{byte:02x}").expect("writing to a string cannot fail");
    }
    for (index, chunk) in payload.chunks(wire::CHUNK_PAYLOAD_BYTES).enumerate() {
        write_frame(
            writer,
            &wire::Outbound::CommandChunk {
                transfer_id: transfer_id.clone(),
                correlation: correlation.into(),
                index: u16::try_from(index).map_err(|_| FrameError::TooLarge)?,
                count,
                total_bytes,
                sha256: sha256.clone(),
                data: base64::engine::general_purpose::STANDARD.encode(chunk),
            },
        )?;
    }
    Ok(())
}
