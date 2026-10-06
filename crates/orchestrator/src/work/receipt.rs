//! One consuming resolution/completion seam for direct work, composition children and preparation facts.

use super::*;
use crate::governance::evidence::{PermissionCheck, PermissionTrace};
use crate::language::history::StepReceipt;

impl ApplicationExecutor {
    /// Consume direct or parent evidence before any completion reactions or audit writes.
    pub(super) fn finish(
        &self,
        evidence: WorkEvidence,
        completion: Completion<'_>,
    ) -> InvocationResult {
        self.complete_work(evidence, completion, None).caller
    }

    fn complete_work(
        &self,
        mut evidence: WorkEvidence,
        completion: Completion<'_>,
        step: Option<StepReceipt>,
    ) -> CompletedWork {
        let Completion {
            workspace,
            tool,
            requirements,
            snapshot,
            duration_ms,
            provenance,
            execution,
        } = completion;
        evidence.payload.facts["browser_attention"] =
            json!(self.governance.effective_authority().browser_attention);
        let coverage = self.take_coverage(evidence.invocation());
        if let Some(coverage) = &coverage {
            evidence.payload.facts["coverage"] = json!(coverage);
        }
        let observed = self.take_observation(evidence.invocation());
        let execution = std::mem::take(
            &mut *execution
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        // WorkEvidence and Resolution have no Clone. This ownership transfer is the completion
        // boundary: action truth is frozen before presentation, diagnostics or retained receipts.
        let (resolution, payload) = evidence.resolve(execution, coverage, observed);
        let language::resolution::Projection {
            mut caller,
            retained,
        } = language::resolution::project(&resolution, payload);
        let tool = language::audit::tool_name(tool);
        if let Some(physical_id) = resolution.physical_id() {
            self.workbench.browser_tab(
                resolution.invocation(),
                workspace.as_str(),
                physical_id,
                &self.workspaces,
            );
        }
        let event = match resolution.status() {
            Status::Blocked
                if !matches!(
                    resolution.decision().reason,
                    ReasonCode::AuditUnavailable
                        | ReasonCode::RuntimeHold
                        | ReasonCode::SessionEnded
                ) && !matches!(
                    resolution.cause(),
                    Some(
                        Refusal::CredentialAuthorization
                            | Refusal::BrowserAttentionProtected { .. }
                    )
                ) && (!matches!(
                    resolution.conclusion(),
                    Conclusion::Outcome(Outcome::CompositionRan(_))
                ) || !resolution.decision().allowed) =>
            {
                DomainEvent::WorkBlocked {
                    invocation: resolution.invocation().into(),
                    workspace: workspace.as_str().into(),
                    physical_id: resolution.physical_id(),
                    presentation: denial_presentation(tool, &resolution),
                }
            }
            Status::AttentionRequired => DomainEvent::AttentionRequired {
                invocation: resolution.invocation().into(),
                workspace: workspace.as_str().into(),
                physical_id: resolution.physical_id(),
            },
            _ => DomainEvent::WorkCompleted {
                invocation: resolution.invocation().into(),
                workspace: workspace.as_str().into(),
                physical_id: resolution.physical_id(),
            },
        };
        if step.is_none() {
            self.emit(event);
        } else {
            self.presentation.react(&DomainEvent::WorkCompleted {
                invocation: resolution.invocation().into(),
                workspace: workspace.as_str().into(),
                physical_id: resolution.physical_id(),
            });
        }
        let status = serde_json::to_value(resolution.status())
            .expect("closed status")
            .as_str()
            .expect("status string")
            .to_owned();
        let effect = serde_json::to_value(resolution.effect())
            .expect("closed effect")
            .as_str()
            .expect("effect string")
            .to_owned();
        self.diagnostics.sink().emit(
            if resolution.status() == Status::Failed {
                ghostlight_bridge::diagnostics::event::OPERATION_FAILED
            } else {
                ghostlight_bridge::diagnostics::event::OPERATION_COMPLETED
            },
            if resolution.status() == Status::Failed {
                ghostlight_bridge::diagnostics::Level::Warn
            } else {
                ghostlight_bridge::diagnostics::Level::Info
            },
            Some(resolution.invocation()),
            &format!("{tool} {status} {effect} {duration_ms}ms"),
        );
        let browser_label = self.workspaces.browser_of(workspace.as_str()).map(|id| {
            self.browser
                .browsers()
                .into_iter()
                .find(|browser| browser.id == id)
                .and_then(|browser| browser.name)
                .unwrap_or_else(|| "Chromium".into())
        });
        let mut record = AuditRecord::now(
            resolution.invocation(),
            workspace.as_str(),
            tool,
            requirements,
            snapshot.id(),
            resolution.decision(),
            &status,
            &effect,
            &retained,
            duration_ms,
        )
        .with_provenance(provenance.map(ConnectionEvidence::attribution))
        .with_policy(snapshot, resolution.decision())
        .with_observation(resolution.observed().clone())
        .with_browser(browser_label);
        record.step = step;
        record.repeat_safe = Some(resolution.repeat_safe());
        record.permissions = self.take_permissions(resolution.invocation());
        let storage = self.audit.record_with_provenance(&record, provenance);
        caller.summary = if storage == language::audit_health::Storage::Saved {
            language::audit_health::qualify_children(
                &caller.summary,
                resolution.unconfirmed_history_steps(),
            )
        } else {
            storage.qualify(&caller.summary)
        };
        caller.history_storage = storage;
        CompletedWork {
            resolution,
            caller,
            storage,
        }
    }

    /// Execute one ordinary child under the parent lease and snapshot, with separate effect evidence.
    pub(super) fn run_child(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
        step: StepReceipt,
    ) -> CompletedWork {
        let _ = self.take_permissions(context.invocation);
        let started = Instant::now();
        let execution = Mutex::new(ExecutionEvidence::default());
        let settlement_pending =
            std::sync::atomic::AtomicBool::new(language::settlement::enabled(operation));
        let context = InvocationContext {
            settlement_pending: &settlement_pending,
            requirements: language::capability_map::requirements(operation),
            execution: &execution,
            phase: Phase::RequestedEffect,
            ..*context
        };
        self.emit(DomainEvent::WorkPhaseStarted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: None,
            activity: operation_activity(operation),
        });
        let evidence = self.run(&context, lease, operation);
        self.complete_work(
            evidence,
            Completion {
                workspace: context.workspace,
                tool: operation.name(),
                requirements: context.requirements,
                snapshot: context.snapshot,
                duration_ms: elapsed_ms(started),
                provenance: context.provenance,
                execution: &execution,
            },
            Some(step),
        )
    }

    /// Record input preparation failure without fabricating a child invocation or browser effect.
    pub(super) fn record_preparation(
        &self,
        context: &InvocationContext<'_>,
        tool: &str,
        step: StepReceipt,
    ) -> language::audit_health::Storage {
        let mut record = AuditRecord::now(
            context.invocation,
            context.workspace.as_str(),
            language::audit::tool_name(tool),
            CapabilitySet::EMPTY,
            context.snapshot.id(),
            Decision::refused(ReasonCode::InvalidRequest),
            "not_started",
            "none",
            &Outcome::StepNotStarted.audit(),
            0,
        )
        .with_provenance(context.provenance.map(ConnectionEvidence::attribution));
        record.step = Some(step);
        self.audit
            .record_with_provenance(&record, context.provenance)
    }

    /// Retain the bounded evidence for the operation currently using this invocation.
    pub(super) fn retain_permission(
        &self,
        context: &InvocationContext<'_>,
        check: PermissionCheck,
    ) {
        self.permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(context.invocation.into())
            .or_default()
            .record(check);
    }

    fn take_permissions(&self, invocation: &str) -> PermissionTrace {
        self.permissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(invocation)
            .unwrap_or_default()
    }
}
