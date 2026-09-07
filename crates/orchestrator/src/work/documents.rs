//! Document admission at the executor's common browser boundary, before content extraction.

use ghostlight_bridge::browser::documents::{DocumentInventory, DocumentScope, DOCUMENT_LIMIT};

use crate::governance::documents::{Handling, Notice};
use crate::language::coverage::{Coverage, HumanCoverage};

use super::*;

struct RequestScope {
    tab: u64,
    locators: Vec<String>,
    points: Vec<PhysicalPoint>,
    focused: bool,
    whole: bool,
    capture: bool,
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
            capture: false,
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
            BrowserCommand::Screenshot {
                tab_id, locator, ..
            } => {
                scope.locators.extend(locator.clone());
                scope.whole = locator.is_none();
                scope.capture = true;
                *tab_id
            }
            BrowserCommand::ScreenshotRegion { tab_id, .. } => {
                scope.whole = true;
                scope.capture = true;
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
            BrowserCommand::InspectDialog { tab_id }
            | BrowserCommand::HandleDialog { tab_id, .. } => {
                // Browser dialogs may originate in any embedded document.
                scope.whole = true;
                scope.unbounded = true;
                *tab_id
            }
            BrowserCommand::SetZoom { tab_id, .. } => *tab_id,
            BrowserCommand::ExportRecording {
                destination:
                    ghostlight_bridge::browser::RecordingDestination::Target {
                        tab_id, locator, ..
                    },
                ..
            } => {
                scope.locators.push(locator.clone());
                *tab_id
            }
            BrowserCommand::ListTabs
            | BrowserCommand::FocusTab { .. }
            | BrowserCommand::OpenTab { .. }
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
            | BrowserCommand::Cancel { .. }
            | BrowserCommand::Present { .. }
            | BrowserCommand::DescribeDocuments { .. }
            | BrowserCommand::InDocuments { .. } => return None,
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
        let notice = context.snapshot.document_policy().notice;
        self.workbench.document_coverage(HumanCoverage {
            summary: language::coverage::human_summary(&coverage),
            explanation: language::coverage::HUMAN_EXPLANATION.into(),
            invocation: context.invocation.into(),
            proactive: match notice {
                Notice::OnDemand => false,
                Notice::WhenAffected => coverage.limited(),
                Notice::WhenExcluded => coverage.page_excluded_documents > 0 || coverage.limited(),
            },
            coverage: coverage.clone(),
            excluded_hosts: hosts,
        });
    }

    pub(super) fn dispatch_documents(
        &self,
        context: &InvocationContext<'_>,
        command: BrowserCommand,
    ) -> Result<BrowserOutcome, BrowserError> {
        let Some(requested) = RequestScope::from(&command) else {
            return self.dispatch_physical(context, command);
        };
        let inventory = match self.dispatch_physical(
            context,
            BrowserCommand::DescribeDocuments {
                tab_id: requested.tab,
                locators: requested.locators.clone(),
                points: requested.points.clone(),
                focused: requested.focused,
            },
        )? {
            BrowserOutcome::Documents { tab_id, inventory } if tab_id == requested.tab => inventory,
            _ => return Err(BrowserError::Protocol("expected document inventory".into())),
        };
        validate_inventory(&inventory)?;
        let policy = context.snapshot.document_policy();
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
        let denied = if policy.handling == Handling::CompletePage || requested.unbounded {
            first_denial
        } else if !requested.whole || policy.handling == Handling::CompleteOperation {
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
            || (policy.handling == Handling::CompleteOperation
                && coverage.unavailable_documents > 0)
            || (((requested.unbounded || requested.capture) && !unrestricted_documents
                || policy.handling == Handling::CompletePage)
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
                mask: (requested.capture && coverage.page_excluded_documents > 0)
                    .then(|| language::coverage::MASK_LABEL.into()),
                watch_changes: !unrestricted_documents,
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
                coverage.masked_regions = observation.masked_regions;
                self.retain_coverage(context, coverage.clone(), hosts);
                let mut result = *result;
                let observation_result = matches!(
                    result,
                    BrowserOutcome::Text { .. }
                        | BrowserOutcome::Targets { .. }
                        | BrowserOutcome::DocumentTree { .. }
                        | BrowserOutcome::Observed { .. }
                );
                if observation_result
                    && (coverage.inspected_documents == 0
                        || (policy.handling != Handling::PermittedContent
                            && coverage.unavailable_documents > 0))
                {
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
                || document.url.len() > 8192
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
    use ghostlight_bridge::browser::documents::PhysicalDocument;

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
