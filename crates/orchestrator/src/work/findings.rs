//! What one frame of a request learns while it runs, read once by that frame's completion.
//!
//! A frame is a request's top-level operation or one flow step. Its findings sit in a `RefCell`
//! on the frame's stack, so they are never keyed by an id, never stored on the executor and never
//! shared between frames or threads. Every write is one short method that releases its borrow
//! before it returns, so no borrow is held across a browser call.

use std::cell::RefCell;
use std::sync::atomic::AtomicBool;

use serde_json::Value;

use crate::governance::evidence::{PermissionCheck, PermissionTrace};
use crate::language::coverage::Coverage;
use crate::language::outcome::Observed;
use crate::language::{self, Operation};

/// The closed set of facts one frame gathers on its way to completion.
#[derive(Default)]
pub(super) struct Findings {
    /// What the page showed, merged at each browser crossing.
    pub(super) observed: Observed,
    /// The browser that received a command of this frame, by name.
    pub(super) reached: Option<String>,
    /// The browser this frame's crossings go to, by id, once the frame chose one.
    ///
    /// Until a tab binds the workspace to a browser, the frame's own first choice holds for the
    /// rest of the frame, so one frame never crosses to two browsers.
    pub(super) target: Option<String>,
    /// Every governance check of this frame.
    pub(super) permissions: PermissionTrace,
    /// Scoped reads and recording coverage, merged.
    pub(super) coverage: Option<Coverage>,
    /// The recovery a stale-target refusal offers, taken by the same frame's refusal.
    pub(super) stale_offer: Option<StaleOffer>,
}

/// The recovery a stale-target refusal offers (ADR-0198 Decision 28): controls the page found,
/// each already registered as a handle, and the one of them the page proved is the control the
/// stale handle named, when it did and the call can safely be repeated with it.
pub(super) struct StaleOffer {
    /// The facts for each candidate, as the model sees them.
    pub(super) candidates: Vec<Value>,
    /// The handle of the proven replacement.
    pub(super) replacement: Option<String>,
}

/// The state one frame owns on its stack: its preparation budget and its findings.
///
/// An `InvocationContext` borrows it for the frame's whole run.
pub(super) struct Frame {
    /// Whether this frame's preparation budget is still unspent. The first dispatch that settles
    /// takes it.
    pub(super) settlement_pending: AtomicBool,
    /// What this frame has learned so far.
    pub(super) findings: FrameFindings,
}

impl Frame {
    /// A fresh frame for `operation`, with its preparation budget unspent when it asked for one.
    pub(super) fn of(operation: &Operation) -> Self {
        Self {
            settlement_pending: AtomicBool::new(language::settlement::enabled(operation)),
            findings: FrameFindings::default(),
        }
    }
}

/// One frame's findings while it runs, written through its `InvocationContext`.
#[derive(Default)]
pub(super) struct FrameFindings(RefCell<Findings>);

impl FrameFindings {
    /// Fold what one crossing saw into what the frame already observed.
    pub(super) fn observe(&self, observed: Observed) {
        let mut findings = self.0.borrow_mut();
        findings.observed = std::mem::take(&mut findings.observed).merged(observed);
    }

    /// Remember the browser that received one of this frame's commands; the latest one stands.
    pub(super) fn reach(&self, browser: String) {
        self.0.borrow_mut().reached = Some(browser);
    }

    /// The browser this frame has reached so far, if any.
    pub(super) fn reached(&self) -> Option<String> {
        self.0.borrow().reached.clone()
    }

    /// Remember the browser this frame's crossings go to.
    pub(super) fn target(&self, browser: &str) {
        self.0.borrow_mut().target = Some(browser.into());
    }

    /// The browser this frame chose for its crossings, if it chose one.
    pub(super) fn targeted(&self) -> Option<String> {
        self.0.borrow().target.clone()
    }

    /// Keep the evidence of one governance check.
    pub(super) fn permit(&self, check: PermissionCheck) {
        self.0.borrow_mut().permissions.record(check);
    }

    /// Merge one coverage account into the frame's and return the merged coverage.
    pub(super) fn cover(&self, mut coverage: Coverage) -> Coverage {
        let mut findings = self.0.borrow_mut();
        if let Some(previous) = &findings.coverage {
            coverage.include(previous);
        }
        findings.coverage = Some(coverage.clone());
        coverage
    }

    /// Keep the recovery this frame's stale-target refusal will offer.
    pub(super) fn offer(&self, offer: StaleOffer) {
        self.0.borrow_mut().stale_offer = Some(offer);
    }

    /// Take the recovery a stale-target refusal offers, leaving none behind.
    pub(super) fn take_offer(&self) -> Option<StaleOffer> {
        self.0.borrow_mut().stale_offer.take()
    }

    /// Take everything the frame learned, for its completion, leaving nothing behind.
    pub(super) fn take(&self) -> Findings {
        self.0.take()
    }
}
