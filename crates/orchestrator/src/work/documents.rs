//! Document admission at the executor's common browser boundary, before content extraction.

use crate::browser::adapter::documents::{DocumentInventory, DocumentScope, DOCUMENT_LIMIT};

use crate::language::coverage::{Coverage, HumanCoverage};

use super::*;

/// The longest document url the executor accepts from an adapter.
const URL_LIMIT: usize = 8192;

struct RequestScope {
    tab: u64,
    locators: Vec<String>,
    points: Vec<PhysicalPoint>,
    focused: bool,
    whole: bool,
    unbounded: bool,
}

impl RequestScope {
    fn from(command: &BrowserCommand) -> Option<Self> {
        let mut scope = Self {
            tab: 0,
            locators: vec![],
            points: vec![],
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
            | BrowserCommand::Scroll {
                tab_id, locator, ..
            } => {
                scope.locators.extend(locator.clone());
                scope.whole = locator.is_none();
                *tab_id
            }
            BrowserCommand::ReadDocument { tab_id, .. }
            | BrowserCommand::Inspect { tab_id, .. }
            | BrowserCommand::Find { tab_id, .. }
            | BrowserCommand::QuerySemantic { tab_id, .. } => {
                scope.whole = true;
                *tab_id
            }
            BrowserCommand::DescribeTargets { tab_id, locators } => {
                scope.locators = locators.clone();
                *tab_id
            }
            BrowserCommand::Activate {
                tab_id, locator, ..
            }
            | BrowserCommand::ActivateModified {
                tab_id, locator, ..
            }
            | BrowserCommand::Hover { tab_id, locator }
            | BrowserCommand::TypeText {
                tab_id, locator, ..
            }
            | BrowserCommand::UploadFiles {
                tab_id, locator, ..
            } => {
                scope.locators.push(locator.clone());
                *tab_id
            }
            BrowserCommand::ActivatePoint { tab_id, point, .. }
            | BrowserCommand::ActivatePointModified { tab_id, point, .. }
            | BrowserCommand::WheelAt { tab_id, point, .. }
            | BrowserCommand::HoverPoint { tab_id, point, .. }
            | BrowserCommand::DropImageAt { tab_id, point, .. } => {
                scope.points.push(*point);
                *tab_id
            }
            BrowserCommand::Fill {
                tab_id,
                fields,
                submit_locator,
                ..
            } => {
                scope
                    .locators
                    .extend(fields.iter().map(|field| field.locator.clone()));
                scope.locators.extend(submit_locator.clone());
                *tab_id
            }
            BrowserCommand::DescribeFocused { tab_id }
            | BrowserCommand::TypeFocused { tab_id, .. } => {
                scope.focused = true;
                *tab_id
            }
            BrowserCommand::PressKey {
                tab_id, locator, ..
            } => {
                scope.locators.extend(locator.clone());
                scope.focused = locator.is_none();
                *tab_id
            }
            BrowserCommand::Drag {
                tab_id,
                source_locator,
                destination_locator,
            } => {
                scope
                    .locators
                    .extend([source_locator.clone(), destination_locator.clone()]);
                // Pointer travel can cross any intervening document.
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::DragPoints {
                tab_id, start, end, ..
            } => {
                scope.points.extend([*start, *end]);
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::EvaluateScript { tab_id, .. }
            | BrowserCommand::ReadDiagnostics { tab_id, .. }
            | BrowserCommand::StartRecording { tab_id } => {
                scope.whole = true;
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::Observe {
                tab_id,
                locator,
                condition,
                ..
            } => {
                scope.locators.extend(locator.clone());
                scope.whole = matches!(condition.as_str(), "text_present" | "text_absent");
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
            | BrowserCommand::StatusRecording
            | BrowserCommand::StopRecording
            | BrowserCommand::ExportRecording { .. }
            | BrowserCommand::DiscardRecording
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
    pub(super) fn take_coverage(&self, invocation: &str) -> Option<Coverage> {
        self.coverage
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(invocation)
    }

    pub(super) fn retain_coverage(
        &self,
        context: &InvocationContext<'_>,
        mut coverage: Coverage,
        hosts: Vec<String>,
    ) {
        {
            let mut retained = self
                .coverage
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(previous) = retained.get(context.invocation) {
                coverage.include(previous);
            }
            retained.insert(context.invocation.into(), coverage.clone());
        }
        self.workbench.document_coverage(HumanCoverage {
            summary: language::coverage::human_summary(&coverage),
            explanation: language::coverage::HUMAN_EXPLANATION.into(),
            invocation: context.invocation.into(),
            proactive: coverage.limited(),
            coverage: coverage.clone(),
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
                .filter(|url| !url.is_empty() && url.len() <= URL_LIMIT)
            else {
                return Err(BrowserError::DocumentUnavailable);
            };
            let (decision, mut evidence) = context
                .snapshot
                .authorize_with_evidence(context.requirements, Some(url));
            // The raising frame may be embedded, and an embedded document's host never enters
            // the durable audit; the tab admission already records the top document's host.
            evidence.host = None;
            self.retain_permission(context, evidence);
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
                self.retain_permission(
                    context,
                    context
                        .snapshot
                        .decision_evidence(context.requirements, None, decision),
                );
                return Err(BrowserError::DocumentAccess(decision));
            }
            if let BrowserCommand::Screenshot { visual_settle, .. }
            | BrowserCommand::ScreenshotRegion { visual_settle, .. } = &mut command
            {
                // Target lookup may already have spent this invocation's preparation.
                // Direct capture retains its binary permission and avoids document inventory.
                *visual_settle = Some(context.settlement_pending.swap(false, Ordering::SeqCst));
            }
            return self.dispatch_physical(context, command);
        }
        let Some(requested) = RequestScope::from(&command) else {
            return self.dispatch_physical(context, command);
        };
        let preparation_started = Instant::now();
        let settle_ms = if !matches!(
            command,
            BrowserCommand::Observe { .. }
                | BrowserCommand::DescribeTargets { .. }
                | BrowserCommand::DescribeFocused { .. }
                | BrowserCommand::ReadDiagnostics { .. }
                | BrowserCommand::StartRecording { .. }
        ) && context.settlement_pending.swap(false, Ordering::SeqCst)
        {
            // Reserve at least half the remaining deadline for actual work.
            (context
                .deadline
                .saturating_duration_since(Instant::now())
                .as_millis() as u64
                / 2)
            .min(language::settlement::MAX_WAIT_MS)
        } else {
            0
        };
        let inventory = match self.dispatch_physical(
            context,
            BrowserCommand::DescribeDocuments {
                tab_id: requested.tab,
                locators: requested.locators.clone(),
                points: requested.points.clone(),
                focused: requested.focused,
                settle_ms,
            },
        )? {
            BrowserOutcome::Documents { tab_id, inventory } if tab_id == requested.tab => inventory,
            _ => return Err(BrowserError::Protocol("expected document inventory".into())),
        };
        validate_inventory(&inventory)?;
        let mut allowed = Vec::new();
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
            self.retain_permission(context, evidence);
            if decision.allowed {
                allowed.push(document.id.clone());
            } else {
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
        if inventory.incomplete || inventory.unresolved {
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
        let unrestricted_documents = context.snapshot.permits_any_document(context.requirements);
        if inventory.unresolved
            || inventory.incomplete
            || allowed.is_empty()
            || (requested.unbounded
                && !unrestricted_documents
                && inventory
                    .documents
                    .iter()
                    .any(|document| !document.supported))
        {
            return Err(BrowserError::DocumentUnavailable);
        }
        let tests_absence = matches!(&command, BrowserCommand::Observe { condition, .. } if condition.ends_with("_absent"));
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
                if observation_result && coverage.inspected_documents == 0 {
                    return Err(BrowserError::DocumentUnavailable);
                }
                if let BrowserOutcome::Observed { satisfied, .. } = &mut result {
                    if tests_absence && coverage.limited() {
                        *satisfied = false;
                    }
                }
                Ok(result)
            }
            Ok(_) => Err(BrowserError::Protocol(
                "expected document-bound receipt".into(),
            )),
            Err(error) => Err(error),
        }
    }
}

fn validate_inventory(inventory: &DocumentInventory) -> Result<(), BrowserError> {
    use std::collections::HashSet;
    let mut ids = HashSet::new();
    if inventory.documents.is_empty()
        || inventory.documents.len() > DOCUMENT_LIMIT
        || inventory.subjects.len() > DOCUMENT_LIMIT
        || inventory.documents.iter().any(|document| {
            document.id.is_empty()
                || document.id.len() > 160
                || document.url.len() > URL_LIMIT
                || !ids.insert(document.id.as_str())
        })
        || inventory
            .documents
            .iter()
            .filter(|document| document.parent.is_none())
            .count()
            != 1
        || inventory
            .subjects
            .iter()
            .any(|id| !ids.contains(id.as_str()))
    {
        return Err(BrowserError::DocumentUnavailable);
    }
    for document in &inventory.documents {
        let mut ancestry = HashSet::new();
        let mut current = document;
        while let Some(parent) = &current.parent {
            if !ancestry.insert(current.id.as_str()) {
                return Err(BrowserError::DocumentUnavailable);
            }
            current = inventory
                .documents
                .iter()
                .find(|item| &item.id == parent)
                .ok_or(BrowserError::DocumentUnavailable)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::adapter::documents::PhysicalDocument;

    #[test]
    fn a_disconnected_or_cyclic_graph_cannot_establish_coverage() {
        let mut inventory = DocumentInventory {
            documents: vec![
                PhysicalDocument {
                    id: "top".into(),
                    url: "https://example.com".into(),
                    parent: None,
                    supported: true,
                },
                PhysicalDocument {
                    id: "child".into(),
                    url: "https://example.com".into(),
                    parent: Some("absent".into()),
                    supported: true,
                },
            ],
            ..DocumentInventory::default()
        };
        assert_eq!(
            validate_inventory(&inventory),
            Err(BrowserError::DocumentUnavailable)
        );
        inventory.documents[1].parent = Some("child".into());
        assert_eq!(
            validate_inventory(&inventory),
            Err(BrowserError::DocumentUnavailable)
        );
        inventory.documents[1].parent = Some("top".into());
        assert!(validate_inventory(&inventory).is_ok());
    }
}
