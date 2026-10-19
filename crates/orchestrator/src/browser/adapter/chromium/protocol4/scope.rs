//! Document scope for one protocol 4 request.
//!
//! The authority decides which documents a request may touch and hands the adapter a
//! [`DocumentScope`]. This module enforces that decision on the service side of the wire. It
//! admits the scope against a frame tree, turns every world message into a call only if its
//! target document is admitted, builds the currency guard that rides with the first primitive and
//! every input, and records what the request actually reached.
//!
//! Nothing here decides policy. A content call to a document outside `allowed` fails before
//! anything is sent.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use super::locator::Locator;
use super::tree::{document_id, FrameTree};
use super::wire::{CallAnswer, DocumentGuard, FrameRef, GuardDoc, WorldCall};
use super::world::{self, AssetErrorCode, EffectClass, Message, ScopeClass};
use crate::browser::adapter::documents::{DocumentObservation, DocumentScope};
use crate::browser::adapter::BrowserOutcome;
use crate::browser::BrowserError;

/// The member of an asset's answer that says its output was cut.
const TRUNCATED_FIELD: &str = "truncated";

/// One world call, admitted and ready to send, with what the scope needs to record its answer.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedCall {
    /// The call as the `call` primitive carries it.
    pub call: WorldCall,
    /// The document the call goes to; absent for the tab's current top document.
    pub doc: Option<FrameRef>,
    /// Which documents the message may reach.
    pub scope: ScopeClass,
    /// What the message can have changed.
    pub effect: EffectClass,
}

impl PlannedCall {
    fn new(frame: Option<&FrameRef>, owner_of: Option<&str>, message: &Message) -> Self {
        Self {
            call: WorldCall {
                frame: frame.cloned(),
                owner_of: owner_of.map(str::to_owned),
                message: message.to_json(),
            },
            doc: frame.cloned(),
            scope: message.scope_class(),
            effect: message.effect_class(),
        }
    }

    /// A call to a document that no request scope admitted. The caller has its own reason to
    /// trust the document, such as a locator the model already holds, and the call carries no
    /// guard from a scope.
    #[must_use]
    pub fn unscoped(doc: &FrameRef, message: Message) -> Self {
        Self::new(Some(doc), None, &message)
    }

    /// A call to whatever document is the tab's top when the call runs, with no loader check.
    #[must_use]
    pub fn top(message: Message) -> Self {
        Self::new(None, None, &message)
    }

    /// A call made inside `parent` that runs with the element embedding the child frame
    /// `owner_of` as its subject.
    #[must_use]
    pub fn owner(parent: &FrameRef, owner_of: &str, message: Message) -> Self {
        Self::new(Some(parent), Some(owner_of), &message)
    }
}

/// What a request reached.
#[derive(Debug, Default)]
struct Observation {
    visited: BTreeSet<FrameRef>,
    unavailable: BTreeSet<FrameRef>,
    limited: bool,
}

/// The documents one request may touch, and what it did to them.
#[derive(Debug)]
pub struct Scope {
    tree: FrameTree,
    allowed: BTreeSet<FrameRef>,
    subjects: Vec<FrameRef>,
    input_subjects: Vec<FrameRef>,
    strict: bool,
    watch: bool,
    settle_until: Instant,
    settle_requested: bool,
    observation: Observation,
}

impl Scope {
    /// Admit the authority's scope against the documents it was decided on.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] when the documents are not one tree, or
    /// when an allowed document is not a real document the adapter can address.
    pub fn admit(scope: &DocumentScope) -> Result<Self, BrowserError> {
        let tree = FrameTree::from_documents(&scope.documents)?;
        let allowed = scope
            .allowed
            .iter()
            .map(|id| {
                tree.supported_ref(id)
                    .ok_or(BrowserError::DocumentUnavailable)
            })
            .collect::<Result<_, _>>()?;
        let subjects = scope
            .subjects
            .iter()
            .filter_map(|id| tree.real_ref(id))
            .collect();
        let settle = Duration::from_millis(scope.settle_ms);
        Ok(Self {
            tree,
            allowed,
            subjects,
            input_subjects: Vec::new(),
            strict: scope.strict_tree,
            watch: scope.watch_changes,
            settle_until: Instant::now() + settle,
            settle_requested: scope.settle_requested,
            observation: Observation::default(),
        })
    }

    /// The tree the scope was admitted against.
    #[must_use]
    pub const fn tree(&self) -> &FrameTree {
        &self.tree
    }

    /// The top document.
    #[must_use]
    pub fn top(&self) -> FrameRef {
        self.tree.top().clone()
    }

    /// The documents the authority routed the request's locators, points or focus to.
    #[must_use]
    pub fn subjects(&self) -> &[FrameRef] {
        &self.subjects
    }

    /// The names of the allowed documents, sorted, so equal admitted sets name equally.
    #[must_use]
    pub fn allowed_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.allowed.iter().map(document_id).collect();
        ids.sort();
        ids
    }

    /// Whether the authority asked to watch the admitted documents. It does when its policy
    /// does not admit every document, so a change of the admitted set matters.
    #[must_use]
    pub const fn watches(&self) -> bool {
        self.watch
    }

    /// The documents a recording must stop at when they change: every real document of the
    /// tree, or `None` when the authority did not ask to watch them.
    #[must_use]
    pub fn watch(&self) -> Option<Vec<GuardDoc>> {
        self.watch
            .then(|| self.tree.real_documents().map(GuardDoc::from).collect())
    }

    /// The documents content may be read from, in tree order with the top first.
    #[must_use]
    pub fn content_docs(&self) -> Vec<FrameRef> {
        self.tree
            .real_documents()
            .map(|document| &document.doc)
            .filter(|doc| self.allowed.contains(*doc))
            .cloned()
            .collect()
    }

    /// The document an encoded locator names, with its local part, or the top document when
    /// there is no locator.
    ///
    /// # Errors
    ///
    /// Returns the plain failure for a malformed locator.
    pub fn target(
        &self,
        locator: Option<&str>,
    ) -> Result<(FrameRef, Option<String>), BrowserError> {
        match locator {
            Some(locator) => {
                let target = Locator::decode(locator)?;
                Ok((target.doc, Some(target.local)))
            }
            None => Ok((self.top(), None)),
        }
    }

    /// Plan one world call. Every call goes through here, so nothing reaches a document the
    /// authority did not admit: content messages need the document in `allowed`, metadata
    /// messages need it anywhere in the tree.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] before anything is sent when the document
    /// is not admitted for this message.
    pub fn call(&self, doc: &FrameRef, message: Message) -> Result<PlannedCall, BrowserError> {
        let admitted = match message.scope_class() {
            ScopeClass::Content => self.allowed.contains(doc),
            ScopeClass::Metadata => self.tree.contains(doc),
        };
        if !admitted {
            return Err(BrowserError::DocumentUnavailable);
        }
        let message = if message.takes_settle() {
            message.with_settle(self.remaining_settle_ms(), self.settle_requested)
        } else {
            message
        };
        Ok(PlannedCall::new(Some(doc), None, &message))
    }

    /// The currency check that rides on the first primitive of a request and on every input.
    ///
    /// A strict guard pins the whole tree: every document, its URL, and how many frames were
    /// unseen. Otherwise it pins only the subjects, the top and the input subjects with the
    /// documents above them, by identity and origin.
    #[must_use]
    pub fn guard(&self) -> DocumentGuard {
        if self.strict {
            DocumentGuard {
                documents: self.tree.real_documents().map(GuardDoc::from).collect(),
                exact: true,
                unseen: self.tree.placeholder_count(),
            }
        } else {
            let required = self
                .subjects
                .iter()
                .chain(&self.input_subjects)
                .chain(std::iter::once(self.tree.top()));
            DocumentGuard {
                documents: self
                    .tree
                    .closed_under_ancestors(required)
                    .into_iter()
                    .map(GuardDoc::from)
                    .collect(),
                exact: false,
                unseen: 0,
            }
        }
    }

    /// Admit the document a point or the focus resolved to as the subject of input.
    ///
    /// # Errors
    ///
    /// Returns [`BrowserError::DocumentUnavailable`] when the document is not allowed, before any
    /// input is sent.
    pub fn admit_input_subject(&mut self, subject: &FrameRef) -> Result<(), BrowserError> {
        if !self.allowed.contains(subject) {
            return Err(BrowserError::DocumentUnavailable);
        }
        self.input_subjects.push(subject.clone());
        Ok(())
    }

    /// Record what one answer says about the document it came from. Only content calls count:
    /// a document that answered, or waited out an unmet condition, was visited; one that failed
    /// is unavailable; a call that never ran says nothing.
    pub fn record(&mut self, planned: &PlannedCall, answer: &CallAnswer) {
        let Some(doc) = planned.doc.as_ref() else {
            return;
        };
        if planned.scope != ScopeClass::Content || !self.allowed.contains(doc) {
            return;
        }
        match world::decode_answer(answer) {
            Some(Ok(value)) => {
                self.observation.visited.insert(doc.clone());
                if value
                    .get(TRUNCATED_FIELD)
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                {
                    self.observation.limited = true;
                }
            }
            Some(Err(error)) if error.code == AssetErrorCode::Unsatisfied => {
                self.observation.visited.insert(doc.clone());
            }
            Some(Err(_)) => {
                self.observation.unavailable.insert(doc.clone());
            }
            None if matches!(answer, CallAnswer::Skipped) => {}
            None => {
                self.observation.unavailable.insert(doc.clone());
            }
        }
    }

    /// Note that the request left out content to stay inside a size bound.
    pub fn limit(&mut self) {
        self.observation.limited = true;
    }

    /// What the request reached, for the receipt. The size limit also holds when the result
    /// itself was cut.
    #[must_use]
    pub fn observation(&self, result: &BrowserOutcome) -> DocumentObservation {
        let ids = |reached: &BTreeSet<FrameRef>| {
            self.tree
                .real_documents()
                .filter(|document| reached.contains(&document.doc))
                .map(|document| document_id(&document.doc))
                .collect()
        };
        DocumentObservation {
            visited: ids(&self.observation.visited),
            unavailable: ids(&self.observation.unavailable),
            limited_by_size: self.observation.limited || truncated(result),
        }
    }

    /// Whether a point must be routed to its document before input. When every document is
    /// addressable and allowed, any point lands in an allowed document, so the round trips are
    /// skipped and an ungoverned page pays nothing for governance.
    #[must_use]
    pub fn needs_point_check(&self) -> bool {
        self.tree.placeholder_count() > 0
            || !self
                .tree
                .real_documents()
                .all(|document| document.supported && self.allowed.contains(&document.doc))
    }

    /// What is left of the request's preparation budget, in milliseconds, for the next call that
    /// waits for a page to settle. Every call draws on one shared budget.
    #[must_use]
    pub fn remaining_settle_ms(&self) -> u64 {
        let remaining = self.settle_until.saturating_duration_since(Instant::now());
        u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX)
    }
}

/// Whether an outcome says its own output was cut.
fn truncated(result: &BrowserOutcome) -> bool {
    matches!(
        result,
        BrowserOutcome::Text {
            truncated: true,
            ..
        } | BrowserOutcome::DocumentTree {
            truncated: true,
            ..
        } | BrowserOutcome::ScriptEvaluated {
            truncated: true,
            ..
        } | BrowserOutcome::DiagnosticsRead {
            truncated: true,
            ..
        }
    )
}

#[cfg(test)]
mod tests;
