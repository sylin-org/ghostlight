//! Form fill: several fields, in one or more documents, then an optional submit.
//!
//! A fill validates before it edits. Its first round trip asks every document that holds a
//! field, all at once and inside the one settle budget, to check its fields and the submit
//! control and to wait for a stable form, so a field that cannot take its value refuses before
//! any other field is touched. Every document's refused fields come back together, named by
//! their place in the call.
//! The plans that come back say how each field takes its value. Consecutive fields the page sets
//! itself share one call; a text field is one input that focuses the control as its
//! precondition and types the value as trusted keystrokes. Then every document confirms its
//! fields kept their values and reports what the page holds in the fields worth reading back,
//! and only then is the submit control clicked. Fields the page changed back are not a failure
//! of the page asset: the fill reports them together, as a partly applied fill.
//!
//! The fill has its own time budget inside the command's deadline. Once a field was edited,
//! every failure, including a spent budget or runtime control holding the work between fields,
//! leaves the effect unknown.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::super::keys;
use super::super::link::{Answers, Link};
use super::super::locator::Locator;
use super::super::scope::{PlannedCall, Scope};
use super::super::wire::{CallAnswer, CallMode, DocumentGuard, FrameRef};
use super::super::world::{self, FieldKind, FillPlan, FillVerified, Submitted};
use super::{input_for, send_input, settled, value, OnDialog};
use crate::browser::adapter::{BrowserOutcome, FieldNow, PhysicalField};
use crate::browser::BrowserError;

/// The longest a fill waits for its values to hold, in milliseconds.
pub const FILL_RETAINED_LIMIT_MS: u64 = 2_000;
/// How long the page may settle after a fill with no submit, in milliseconds.
const FILLED_SETTLE_MS: u64 = 25;
/// How long the page may settle after a submit click, in milliseconds.
const SUBMIT_SETTLE_MS: u64 = 250;
/// The failure of a field whose locator names no document.
const NOT_FRAME_SCOPED: &str = "field target is not frame-scoped";
/// The failure of a submit control in no document that holds a field.
const ORPHAN_SUBMIT: &str = "submit control is not contained in the resolved form";
/// The failure of a plan that does not say how each field takes its value.
const INVALID_PLAN: &str = "content primitive returned an invalid fill plan";
/// The failure of a fill that ran out of its own time budget.
const BUDGET_EXHAUSTED: &str = "form fill exhausted its physical execution budget";
/// The failure of a verification that named a position the document does not have.
const INVALID_VERIFICATION: &str = "content primitive returned an invalid fill verification";

/// What a fill asks for.
#[derive(Clone, Copy, Debug)]
pub struct Fill<'a> {
    /// The tab.
    pub tab_id: u64,
    /// The fields, each with an encoded locator, in the order they are filled.
    pub fields: &'a [PhysicalField],
    /// The encoded locator of the control to click after the fields, when there is one.
    pub submit_locator: Option<&'a str>,
    /// The fill's own time budget, in milliseconds.
    pub timeout_ms: u64,
    /// Whether the request may fill credential-class controls.
    pub allow_credentials: bool,
}

/// The fields of one document, with document-local locators, in the order they were asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The document.
    pub doc: FrameRef,
    /// Its fields.
    pub fields: Vec<PhysicalField>,
    /// Where each of its fields stands in the whole call, so a document's refusal names the
    /// field the caller gave and not the field's place in its own document.
    pub positions: Vec<usize>,
}

/// Group fields by the document their locators name, documents in order of first appearance.
///
/// # Errors
///
/// Returns a plain failure for a field whose locator names no document.
pub fn group_by_document(fields: &[PhysicalField]) -> Result<Vec<Group>, BrowserError> {
    let mut groups: Vec<Group> = Vec::new();
    for (position, field) in fields.iter().enumerate() {
        let target = Locator::decode(&field.locator)
            .map_err(|_| BrowserError::Primitive(NOT_FRAME_SCOPED.into()))?;
        let local = PhysicalField {
            locator: target.local,
            value: field.value.clone(),
        };
        match groups.iter_mut().find(|group| group.doc == target.doc) {
            Some(group) => {
                group.fields.push(local);
                group.positions.push(position);
            }
            None => groups.push(Group {
                doc: target.doc,
                fields: vec![local],
                positions: vec![position],
            }),
        }
    }
    Ok(groups)
}

/// How each field of each group takes its value, from the answers to preparing the groups.
///
/// # Errors
///
/// Returns every field the documents refused together, as [`BrowserError::FieldsRejected`] named
/// by position in the whole call; the page's other refusals of a field or of the submit control;
/// and a plain failure for a plan that does not name one known kind per field.
pub fn field_kinds(
    answers: &[CallAnswer],
    groups: &[Group],
) -> Result<Vec<Vec<FieldKind>>, BrowserError> {
    let mut rejected = Vec::new();
    let mut kinds = Vec::with_capacity(groups.len());
    for (group, answer) in groups.iter().zip(answers) {
        match value::<Value>(answer) {
            Ok(plan) => kinds.push(
                serde_json::from_value::<FillPlan>(plan)
                    .ok()
                    .map(|plan| plan.field_kinds)
                    .filter(|kinds| kinds.len() == group.fields.len())
                    .ok_or_else(|| BrowserError::Primitive(INVALID_PLAN.into()))?,
            ),
            Err(BrowserError::FieldsRejected { problems }) => {
                for mut problem in problems {
                    let position = group.positions.get(problem.field).copied();
                    match position {
                        Some(position) => problem.field = position,
                        None => return Err(BrowserError::Primitive(INVALID_PLAN.into())),
                    }
                    rejected.push(problem);
                }
            }
            Err(error) => return Err(error),
        }
    }
    if rejected.is_empty() {
        return Ok(kinds);
    }
    rejected.sort_by_key(|problem| problem.field);
    Err(BrowserError::FieldsRejected { problems: rejected })
}

/// Fill fields and, when asked, click the submit control.
///
/// # Errors
///
/// Returns a plain failure for a malformed field or submit locator, a submit control outside
/// the fields' documents, or a field any document refuses while preparing; and an unknown
/// effect for any failure once a field was edited.
pub fn fill(
    l: &mut Link<'_>,
    scope: &mut Scope,
    request: Fill<'_>,
) -> Result<BrowserOutcome, BrowserError> {
    let budget = Instant::now() + Duration::from_millis(request.timeout_ms);
    let credentials = request.allow_credentials;
    let groups = group_by_document(request.fields)?;
    let submit = request.submit_locator.map(Locator::decode).transpose()?;
    if submit
        .as_ref()
        .is_some_and(|target| !groups.iter().any(|group| group.doc == target.doc))
    {
        return Err(BrowserError::Primitive(ORPHAN_SUBMIT.into()));
    }
    let tab_id = request.tab_id;

    let prepare = groups
        .iter()
        .map(|group| {
            let submit_local = submit
                .as_ref()
                .filter(|target| target.doc == group.doc)
                .map(|target| target.local.as_str());
            scope.call(
                &group.doc,
                world::prepare_fill(&group.fields, credentials, submit_local),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let guard = Some(scope.guard());
    // Preparing edits nothing and every call carries the same remaining settle budget, so the
    // documents prepare together: in sequence their waits would add up past that budget.
    let plans = send(
        l,
        scope,
        Batch {
            tab_id,
            plans: &prepare,
            mode: CallMode::Parallel,
            guard,
            settle_ms: 0,
            budget,
        },
    )?;
    let kinds = field_kinds(&plans, &groups)?;
    // Preparing is the one call of a fill that may follow a handle (ADR-0198 Decision 17); the
    // calls after it refuse a control that left the page.
    let rebound = plans.iter().any(world::answer_rebound);
    let mut filled = 0;
    for (group, kinds) in groups.iter().zip(kinds) {
        let mut run = Vec::new();
        let mut input_admitted = false;
        for (field, kind) in group.fields.iter().zip(kinds) {
            require_budget(l, budget)?;
            match kind {
                FieldKind::Local => {
                    run.push(scope.call(&group.doc, world::fill_local(field, credentials))?)
                }
                FieldKind::BrowserText => {
                    flush(l, scope, tab_id, &mut run, budget)?;
                    if !input_admitted {
                        scope.admit_input_subject(&group.doc)?;
                        input_admitted = true;
                    }
                    let focus =
                        scope.call(&group.doc, world::focus_text_field(field, credentials))?;
                    let text = field
                        .value
                        .text()
                        .ok_or_else(|| BrowserError::Primitive(INVALID_PLAN.into()))?;
                    let input = input_for(
                        l,
                        tab_id,
                        Some(focus),
                        keys::replace_text(&text),
                        Some(scope.guard()),
                        0,
                    )
                    .within(remaining_ms(budget));
                    send_input(l, input, OnDialog::EffectUnknown)?;
                }
            }
            filled += 1;
        }
        flush(l, scope, tab_id, &mut run, budget)?;
    }
    let limit = FILL_RETAINED_LIMIT_MS.min(remaining_ms(budget));
    let verify = groups
        .iter()
        .map(|group| {
            scope.call(
                &group.doc,
                world::verify_fill_retained(&group.fields, credentials, limit),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let retained = send(
        l,
        scope,
        Batch {
            tab_id,
            plans: &verify,
            mode: CallMode::Parallel,
            guard: None,
            settle_ms: if submit.is_none() {
                FILLED_SETTLE_MS
            } else {
                0
            },
            budget,
        },
    )?;
    let held = verified(l, &groups, &retained, filled)?;
    let submitted = match &submit {
        Some(target) => {
            require_budget(l, budget)?;
            let fields = groups
                .iter()
                .find(|group| group.doc == target.doc)
                .map_or(&[][..], |group| group.fields.as_slice());
            let plan = scope.call(
                &target.doc,
                world::submit_fill(fields, &target.local, credentials),
            )?;
            let answers = send(
                l,
                scope,
                Batch {
                    tab_id,
                    plans: &[plan],
                    mode: CallMode::Sequence,
                    guard: None,
                    settle_ms: SUBMIT_SETTLE_MS,
                    budget,
                },
            )?;
            settled::<Submitted>(l, &answers[0])?.submitted
        }
        None => false,
    };
    Ok(BrowserOutcome::Filled {
        tab: l.tab(tab_id)?.into(),
        filled_count: filled,
        held,
        submitted,
        rebound,
        committed_urls: l.committed(),
    })
}

/// What the page holds in the fields it reports, from every document's verification, by position
/// in the whole call.
///
/// # Errors
///
/// Returns every field the documents report changed back together, as
/// [`BrowserError::FieldsReverted`]; and any other failure of a verification as an unknown
/// effect, because the fields were already edited.
fn verified(
    l: &Link<'_>,
    groups: &[Group],
    answers: &[CallAnswer],
    total: usize,
) -> Result<Vec<FieldNow>, BrowserError> {
    let mut held = Vec::new();
    let mut reverted = Vec::new();
    for (group, answer) in groups.iter().zip(answers) {
        let position = |local: usize| {
            group
                .positions
                .get(local)
                .copied()
                .ok_or_else(|| BrowserError::Primitive(INVALID_VERIFICATION.into()))
        };
        match value::<FillVerified>(answer) {
            Ok(verified) => {
                for mut field in verified.fields {
                    field.field = position(field.field).map_err(|error| l.settle(error))?;
                    held.push(field);
                }
            }
            Err(BrowserError::FieldsRejected { problems }) => {
                for mut problem in problems {
                    problem.field = position(problem.field).map_err(|error| l.settle(error))?;
                    reverted.push(problem);
                }
            }
            Err(error) => return Err(l.settle(error)),
        }
    }
    if reverted.is_empty() {
        held.sort_by_key(|field| field.field);
        return Ok(held);
    }
    reverted.sort_by_key(|problem| problem.field);
    Err(BrowserError::FieldsReverted {
        set: total.saturating_sub(reverted.len()),
        problems: reverted,
    })
}

/// One `call` of a fill.
struct Batch<'p> {
    /// The tab.
    tab_id: u64,
    /// The world calls.
    plans: &'p [PlannedCall],
    /// How the extension runs them.
    mode: CallMode,
    /// The currency check that rides on the call, when there is one.
    guard: Option<DocumentGuard>,
    /// How long the page may settle after the calls, in milliseconds.
    settle_ms: u64,
    /// When the fill's own budget ends.
    budget: Instant,
}

/// Send one `call` bounded by the fill's budget. No calls sends nothing and answers nothing.
fn send(l: &mut Link<'_>, scope: &mut Scope, call: Batch<'_>) -> Result<Answers, BrowserError> {
    if call.plans.is_empty() {
        return Ok(Vec::new());
    }
    l.call(
        call.tab_id,
        call.plans,
        call.mode,
        call.guard,
        call.settle_ms,
        remaining_ms(call.budget),
        Some(&mut *scope),
    )
}

/// Fill the fields gathered in `run` in one sequence, and empty it.
fn flush(
    l: &mut Link<'_>,
    scope: &mut Scope,
    tab_id: u64,
    run: &mut Vec<PlannedCall>,
    budget: Instant,
) -> Result<(), BrowserError> {
    let plans = std::mem::take(run);
    let guard = Some(scope.guard());
    let answers = send(
        l,
        scope,
        Batch {
            tab_id,
            plans: &plans,
            mode: CallMode::Sequence,
            guard,
            settle_ms: 0,
            budget,
        },
    )?;
    for answer in &answers {
        settled::<Value>(l, answer)?;
    }
    Ok(())
}

/// Fail when the fill's own budget is spent: plainly before any edit, as an unknown effect
/// after one.
fn require_budget(l: &Link<'_>, budget: Instant) -> Result<(), BrowserError> {
    if Instant::now() >= budget {
        return Err(l.settle(BrowserError::Primitive(BUDGET_EXHAUSTED.into())));
    }
    Ok(())
}

/// What is left of the fill's budget, in milliseconds.
fn remaining_ms(budget: Instant) -> u64 {
    u64::try_from(budget.saturating_duration_since(Instant::now()).as_millis()).unwrap_or(u64::MAX)
}
