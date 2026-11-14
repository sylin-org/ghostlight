//! Document admission at the executor's common browser boundary, before content extraction.
//!
//! This gate decides policy: which documents of a tab the work may touch, and the two rules for
//! a restricting policy that cannot judge a page (an inventory that may hide a document, and
//! unbounded work beside a document nobody can address). It does not own the shape of the
//! inventory (the adapter does, and a list that is not one tree is a protocol failure) or the
//! binding of a point or the focus to a document (the adapter does that when it sends the input,
//! through the scope this gate hands it). When the adapter finds such a subject in a document the
//! scope leaves out, it names the document, and this gate, which holds the decisions, says why.

use std::collections::HashMap;

use crate::browser::adapter::documents::{DocumentScope, DOCUMENT_LIMIT, DOCUMENT_URL_LIMIT};

use crate::language::coverage::{Coverage, HumanCoverage};

use super::*;
use crate::browser::adapter::{
    PhysicalClick, PhysicalDiagnosticsRead, PhysicalDrag, PhysicalFill, PhysicalPointClick,
    PhysicalPointDrag, PhysicalPointDrop, PhysicalPointWheel, PhysicalPressKey, PhysicalScroll,
    PhysicalSemantic, PhysicalTypeFocused, PhysicalTypeText, PhysicalViewPoint, PhysicalWait,
};

/// What a command asks of its tab's documents.
struct RequestScope {
    tab: u64,
    /// The handles the command names. Their documents are its subjects.
    locators: Vec<String>,
    /// The command works on whatever holds the focus, so the top document is not its subject.
    focused: bool,
    whole: bool,
    unbounded: bool,
}

impl RequestScope {
    fn from(command: &BrowserCommand) -> Option<Self> {
        let mut scope = Self {
            tab: 0,
            locators: vec![],
            focused: false,
            whole: false,
            unbounded: false,
        };
        scope.tab = match command {
            BrowserCommand::ReadText {
                tab_id, locator, ..
            }
            | BrowserCommand::InspectTree {
                tab_id, locator, ..
            }
            | BrowserCommand::Scroll(PhysicalScroll {
                tab_id, locator, ..
            }) => {
                scope.locators.extend(locator.clone());
                scope.whole = locator.is_none();
                *tab_id
            }
            BrowserCommand::ReadDocument { tab_id, .. }
            | BrowserCommand::Inspect { tab_id, .. }
            | BrowserCommand::Find { tab_id, .. }
            | BrowserCommand::QuerySemantic(PhysicalSemantic { tab_id, .. }) => {
                scope.whole = true;
                *tab_id
            }
            BrowserCommand::DescribeTargets { tab_id, locators } => {
                scope.locators = locators.clone();
                *tab_id
            }
            BrowserCommand::Activate(PhysicalClick {
                tab_id, locator, ..
            })
            | BrowserCommand::Hover { tab_id, locator }
            | BrowserCommand::TypeText(PhysicalTypeText {
                tab_id, locator, ..
            })
            | BrowserCommand::UploadFiles {
                tab_id, locator, ..
            } => {
                scope.locators.push(locator.clone());
                *tab_id
            }
            // A point names no document until the page shows where it is. The adapter binds
            // it when it sends the input, after the view check and the scroll into view.
            BrowserCommand::ActivatePoint(PhysicalPointClick {
                at: PhysicalViewPoint { tab_id, .. },
                ..
            })
            | BrowserCommand::WheelAt(PhysicalPointWheel {
                at: PhysicalViewPoint { tab_id, .. },
                ..
            })
            | BrowserCommand::HoverPoint(PhysicalViewPoint { tab_id, .. })
            | BrowserCommand::DropImageAt(PhysicalPointDrop {
                at: PhysicalViewPoint { tab_id, .. },
                ..
            }) => *tab_id,
            BrowserCommand::Fill(PhysicalFill {
                tab_id,
                fields,
                submit_locator,
                ..
            }) => {
                scope
                    .locators
                    .extend(fields.iter().map(|field| field.locator.clone()));
                scope.locators.extend(submit_locator.clone());
                *tab_id
            }
            BrowserCommand::DescribeFocused { tab_id }
            | BrowserCommand::TypeFocused(PhysicalTypeFocused { tab_id, .. }) => {
                scope.focused = true;
                *tab_id
            }
            BrowserCommand::PressKey(PhysicalPressKey {
                tab_id, locator, ..
            }) => {
                scope.locators.extend(locator.clone());
                scope.focused = locator.is_none();
                *tab_id
            }
            BrowserCommand::Drag(PhysicalDrag {
                tab_id,
                source_locator,
                destination_locator,
            }) => {
                scope
                    .locators
                    .extend([source_locator.clone(), destination_locator.clone()]);
                // Pointer travel can cross any intervening document.
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::DragPoints(PhysicalPointDrag { tab_id, .. }) => {
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::EvaluateScript { tab_id, .. }
            | BrowserCommand::ReadDiagnostics(PhysicalDiagnosticsRead { tab_id, .. })
            | BrowserCommand::StartRecording { tab_id } => {
                scope.whole = true;
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::Observe(PhysicalWait {
                tab_id,
                locator,
                condition,
                ..
            }) => {
                scope.locators.extend(locator.clone());
                scope.whole = condition.reads_whole_page();
                *tab_id
            }
            BrowserCommand::SetZoom { tab_id, .. } => *tab_id,
            BrowserCommand::ExportRecording {
                destination:
                    crate::browser::adapter::RecordingDestination::Target {
                        tab_id, locator, ..
                    },
                ..
            } => {
                scope.locators.push(locator.clone());
                *tab_id
            }
            BrowserCommand::ListTabs
            | BrowserCommand::Screenshot { .. }
            | BrowserCommand::ScreenshotRegion { .. }
            | BrowserCommand::FocusTab { .. }
            | BrowserCommand::OpenTab { .. }
            | BrowserCommand::AdoptTab { .. }
            | BrowserCommand::Navigate { .. }
            | BrowserCommand::NavigateDiscardingBeforeUnload { .. }
            | BrowserCommand::TraverseHistory { .. }
            | BrowserCommand::Reload { .. }
            | BrowserCommand::CloseTab { .. }
            | BrowserCommand::ResizeWindow { .. }
            | BrowserCommand::ClearDiagnostics { .. }
            | BrowserCommand::StatusRecording { .. }
            | BrowserCommand::StopRecording { .. }
            | BrowserCommand::ExportRecording { .. }
            | BrowserCommand::DiscardRecording { .. }
            | BrowserCommand::Present { .. }
            | BrowserCommand::DescribeDocuments { .. }
            | BrowserCommand::InDocuments { .. }
            | BrowserCommand::InspectDialog { .. }
            | BrowserCommand::HandleDialog { .. } => return None,
        };
        Some(scope)
    }
}

impl ApplicationExecutor {
    /// Merge `coverage` into this frame's findings and tell the window what the frame's reads
    /// have covered so far.
    pub(super) fn retain_coverage(
        &self,
        context: &InvocationContext<'_>,
        coverage: Coverage,
        hosts: Vec<String>,
    ) {
        let coverage = context.findings.cover(coverage);
        self.workbench.document_coverage(HumanCoverage {
            summary: language::coverage::human_summary(&coverage),
            explanation: language::coverage::HUMAN_EXPLANATION.into(),
            invocation: context.invocation.into(),
            proactive: coverage.limited(),
            coverage,
            excluded_hosts: hosts,
        });
    }

    /// Run a dialog command, authorized by the one document that raised the dialog.
    ///
    /// A JavaScript dialog blocks its renderer, so the document inventory a scoped command
    /// starts with cannot be read while one is open. The dialog primitive is answered from the
    /// extension's own record and names the url of the frame that raised the dialog, and that
    /// document is the subject of the decision: the dialog's message is page text from it. The
    /// status is read first and withheld until the document is allowed. A dialog that names no
    /// usable document cannot be authorized, so it fails closed. Handling asks for the status
    /// first too, and sends nothing when no dialog is showing.
    fn dispatch_dialog(
        &self,
        context: &InvocationContext<'_>,
        tab_id: u64,
        handling: Option<(bool, Option<String>)>,
    ) -> Result<BrowserOutcome, BrowserError> {
        let status = self.dispatch_physical(context, BrowserCommand::InspectDialog { tab_id })?;
        let BrowserOutcome::Dialog {
            tab_id: answered,
            present,
            url,
            ..
        } = &status
        else {
            return Err(BrowserError::Protocol("expected dialog status".into()));
        };
        if *answered != tab_id {
            return Err(BrowserError::Protocol("expected dialog status".into()));
        }
        if *present {
            let Some(url) = url
                .as_deref()
                .filter(|url| !url.is_empty() && url.len() <= DOCUMENT_URL_LIMIT)
            else {
                // The fact this site knows: a dialog is showing and the browser named no usable
                // url for the document that raised it, so nothing can authorize it.
                return Err(BrowserError::DocumentUnavailable(
                    DocumentCause::DialogDocumentUnknown,
                ));
            };
            // The raising frame may be embedded.
            let decision = self.authorize_document(context, context.requirements, url);
            if !decision.allowed {
                return Err(BrowserError::DocumentAccess(decision));
            }
        }
        let Some((accept, text)) = handling else {
            return Ok(status);
        };
        if !present {
            return Ok(BrowserOutcome::DialogAbsent { tab_id });
        }
        self.dispatch_physical(
            context,
            BrowserCommand::HandleDialog {
                tab_id,
                accept,
                text,
            },
        )
    }

    pub(super) fn dispatch_documents(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        let mut command = match command {
            BrowserCommand::InspectDialog { tab_id } => {
                return self.dispatch_dialog(context, tab_id, None);
            }
            BrowserCommand::HandleDialog {
                tab_id,
                accept,
                text,
            } => return self.dispatch_dialog(context, tab_id, Some((accept, text))),
            other => other,
        };
        if matches!(
            command,
            BrowserCommand::Screenshot { .. } | BrowserCommand::ScreenshotRegion { .. }
        ) {
            let decision = context.snapshot.authorize_screenshot();
            if !decision.allowed {
                context.findings.permit(context.snapshot.decision_evidence(
                    context.requirements,
                    None,
                    decision,
                ));
                return Err(BrowserError::DocumentAccess(decision));
            }
            if let BrowserCommand::Screenshot { settle_ms, .. }
            | BrowserCommand::ScreenshotRegion { settle_ms, .. } = &mut command
            {
                // Target lookup may already have spent this invocation's preparation.
                // Direct capture retains its binary permission and avoids document inventory.
                *settle_ms = if context.settlement_pending.swap(false, Ordering::SeqCst) {
                    preparation_budget_ms(context.deadline)
                } else {
                    0
                };
            }
            return self.dispatch_physical(context, command);
        }
        let Some(requested) = RequestScope::from(&command) else {
            return self.dispatch_physical(context, command);
        };
        let preparation_started = Instant::now();
        let settle_ms = if !matches!(
            command,
            BrowserCommand::Observe(PhysicalWait { .. })
                | BrowserCommand::DescribeTargets { .. }
                | BrowserCommand::DescribeFocused { .. }
                | BrowserCommand::ReadDiagnostics(PhysicalDiagnosticsRead { .. })
                | BrowserCommand::StartRecording { .. }
        ) && context.settlement_pending.swap(false, Ordering::SeqCst)
        {
            preparation_budget_ms(context.deadline)
        } else {
            0
        };
        let inventory = match self.dispatch_physical(
            context,
            BrowserCommand::DescribeDocuments {
                tab_id: requested.tab,
                locators: requested.locators.clone(),
                settle_ms,
            },
        )? {
            BrowserOutcome::Documents { tab_id, inventory } if tab_id == requested.tab => inventory,
            _ => return Err(BrowserError::Protocol("expected document inventory".into())),
        };
        inventory.validate()?;
        let mut allowed = Vec::new();
        let mut denials: HashMap<String, Decision> = HashMap::new();
        let mut hosts = Vec::new();
        let mut first_denial = None;
        let mut required_denial = None;
        let mut coverage = Coverage {
            targeted: !requested.whole,
            ..Coverage::default()
        };
        for document in &inventory.documents {
            let required = requested.whole
                || inventory.subjects.contains(&document.id)
                || (inventory.subjects.is_empty()
                    && !requested.focused
                    && document.parent.is_none());
            if !document.supported {
                coverage.unavailable_documents += usize::from(required);
                continue;
            }
            let (decision, mut evidence) = context
                .snapshot
                .authorize_with_evidence(context.requirements, Some(&document.url));
            if document.parent.is_some() {
                evidence.host = None;
            }
            context.findings.permit(evidence);
            if decision.allowed {
                allowed.push(document.id.clone());
            } else {
                denials.insert(document.id.clone(), decision);
                first_denial.get_or_insert(decision);
                coverage.page_excluded_documents += 1;
                if required {
                    coverage.excluded_documents += 1;
                    required_denial.get_or_insert(decision);
                }
                if let Some(host) = observed_host(&document.url) {
                    if !hosts.contains(&host) {
                        hosts.push(host);
                    }
                }
            }
        }
        let unrestricted_documents = context.snapshot.permits_any_document(context.requirements);
        // A document the inventory may hide is part of what the work asked for when the work
        // reads the whole page or is not bounded to the documents it names, and when the policy
        // would have to judge it. Work on named documents under a policy that admits every
        // document asked nothing of it: a handle's document is always one the inventory shows.
        if inventory.incomplete
            && (requested.whole || requested.unbounded || !unrestricted_documents)
        {
            coverage.unavailable_documents += 1;
        }
        let denied = if requested.unbounded {
            first_denial
        } else if !requested.whole {
            required_denial
        } else {
            None
        };
        self.retain_coverage(context, coverage.clone(), hosts.clone());
        if let Some(decision) = denied {
            return Err(BrowserError::DocumentAccess(decision));
        }
        // The fact this site knows: the inventory may hide a document, and the policy would
        // have to judge it. A policy that admits every document has nothing to judge there, so
        // the documents the inventory does show stay usable and coverage counts the gap
        // (ADR-0158 amendment of 2026-11-13).
        if inventory.incomplete && !unrestricted_documents {
            return Err(BrowserError::DocumentUnavailable(
                DocumentCause::InventoryIncomplete,
            ));
        }
        // The fact this site knows: the policy admits no document of the tab. When it refused
        // one, that refusal is the answer, whatever the work asked for. Otherwise the tab holds
        // no document the adapter can address: nothing in it has a web address.
        if allowed.is_empty() {
            return Err(match first_denial {
                Some(decision) => BrowserError::DocumentAccess(decision),
                None => BrowserError::DocumentUnavailable(DocumentCause::NoWebDocument),
            });
        }
        // The fact this site knows: the work is not bounded to the documents it names, the
        // policy leaves documents out, and the tab holds a document nobody can address, so the
        // work could reach a document the policy never judged.
        if requested.unbounded
            && !unrestricted_documents
            && inventory
                .documents
                .iter()
                .any(|document| !document.supported)
        {
            return Err(BrowserError::DocumentUnavailable(
                DocumentCause::UnboundedWork,
            ));
        }
        let tests_absence = matches!(&command, BrowserCommand::Observe(PhysicalWait { condition, .. }) if condition.tests_absence());
        let admitted = allowed.clone();
        let scoped = BrowserCommand::InDocuments {
            scope: DocumentScope {
                documents: inventory.documents,
                allowed,
                subjects: inventory.subjects,
                watch_changes: !unrestricted_documents,
                strict_tree: requested.unbounded && !unrestricted_documents,
                settle_ms: Duration::from_millis(settle_ms)
                    .saturating_sub(preparation_started.elapsed())
                    .as_millis() as u64,
                settle_requested: context.settlement_requested,
            },
            primitive: Box::new(command),
        };
        let result = self.dispatch_physical(context, scoped);
        match result {
            Ok(BrowserOutcome::InDocuments {
                observation,
                result,
            }) => {
                if observation.visited.len() > DOCUMENT_LIMIT
                    || observation.unavailable.len() > DOCUMENT_LIMIT
                {
                    return Err(BrowserError::Protocol(
                        "document receipt exceeds its bound".into(),
                    ));
                }
                let mut seen = std::collections::HashSet::new();
                if observation
                    .visited
                    .iter()
                    .any(|id| !admitted.contains(id) || !seen.insert(id))
                    || observation
                        .unavailable
                        .iter()
                        .any(|id| !admitted.contains(id))
                {
                    return Err(BrowserError::Protocol(
                        "document receipt exceeds admitted scope".into(),
                    ));
                }
                coverage.inspected_documents = observation.visited.len();
                coverage.unavailable_documents += observation.unavailable.len();
                coverage.limited_by_size = observation.limited_by_size;
                self.retain_coverage(context, coverage.clone(), hosts);
                let mut result = *result;
                let observation_result = matches!(
                    result,
                    BrowserOutcome::Text { .. }
                        | BrowserOutcome::Targets { .. }
                        | BrowserOutcome::DocumentTree { .. }
                        | BrowserOutcome::Observed { .. }
                );
                // The fact this site knows: the observation ran and no admitted document
                // answered it, so its result says nothing about the page. An adapter that knows
                // why nothing answered fails with that reason itself; one that reports an empty
                // result leaves only this: every document the gate admitted was there a moment
                // ago, so the page changed or stopped answering under the call, and a rerun
                // reads what the tab shows now.
                if observation_result && coverage.inspected_documents == 0 {
                    return Err(BrowserError::PageChanged);
                }
                // Absence across content nobody read is never established (ADR-0158 Decision
                // 4). The result stays unsatisfied, and the frame keeps why, so the sentence
                // does not say the text stayed.
                if let BrowserOutcome::Observed { satisfied, .. } = &mut result {
                    if tests_absence && coverage.limited() && *satisfied {
                        *satisfied = false;
                        context.findings.doubt_absence();
                    }
                }
                Ok(result)
            }
            Ok(_) => Err(BrowserError::Protocol(
                "expected document-bound receipt".into(),
            )),
            // The adapter bound a subject (a point, the focus) when it sent the input and found
            // it in a document the scope leaves out. Nothing was sent there. This gate holds the
            // reason: the policy refused that document, or nobody can address it.
            Err(BrowserError::OutsideScope { document }) => {
                let denied = denials.get(&document).copied();
                if denied.is_some() {
                    coverage.excluded_documents += 1;
                } else {
                    coverage.unavailable_documents += 1;
                }
                self.retain_coverage(context, coverage, hosts);
                Err(match denied {
                    Some(decision) => BrowserError::DocumentAccess(decision),
                    // The fact this site knows: the subject is in a document that is not one
                    // the adapter can address (no web address, or not described), so it could
                    // not be put to the policy.
                    None => BrowserError::DocumentUnavailable(DocumentCause::SubjectUnreachable),
                })
            }
            Err(error) => Err(error),
        }
    }
}

/// The one rule for how long an invocation may let its page settle: half of what its deadline
/// leaves, at most [`language::settlement::MAX_WAIT_MS`], so the work itself keeps the other half.
fn preparation_budget_ms(deadline: Instant) -> u64 {
    let left = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        / 2;
    u64::try_from(left)
        .unwrap_or(u64::MAX)
        .min(language::settlement::MAX_WAIT_MS)
}
