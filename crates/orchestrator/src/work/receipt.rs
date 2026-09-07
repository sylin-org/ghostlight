//! One receipt/completion seam for direct operations, composition children, and preparation facts.

use super::*;
use crate::governance::evidence::{PermissionCheck, PermissionTrace};
use crate::language::history::StepReceipt;

impl ApplicationExecutor {
    /// Complete a direct operation or the composition parent through the shared receipt seam.
    pub(super) fn finish(
        &self,
        gate: &CompletionGate,
        terminal: Terminal,
        completion: Completion<'_>,
    ) -> InvocationResult {
        self.complete_terminal(gate, terminal, completion, None)
            .result
    }

    fn complete_terminal(
        &self,
        gate: &CompletionGate,
        mut terminal: Terminal,
        completion: Completion<'_>,
        step: Option<StepReceipt>,
    ) -> Terminal {
        let Completion {
            workspace,
            tool,
            requirements,
            snapshot,
            duration_ms,
            channel,
            peer_image,
        } = completion;
        if let Some(coverage) = self.take_coverage(&terminal.result.invocation) {
            terminal.result.summary =
                language::coverage::qualify(&terminal.result.summary, &coverage);
            terminal.audit = terminal.audit.with_coverage(&coverage);
            terminal.result.facts["coverage"] = json!(coverage);
        }
        let tool = language::audit::tool_name(tool);
        let denial_attention = terminal.audit.composition().is_none()
            && terminal.result.status == Status::Blocked
            && self
                .governance
                .record_denial_attention(workspace.as_str(), terminal.decision);
        if denial_attention {
            self.require_session_attention(
                workspace,
                &terminal.result.invocation,
                crate::workspace::AttentionReason::RepeatedDenials,
            );
            // End this invocation even if a person resumes the session before its next child.
            // The policy explanation and permission evidence still identify the actual refusal.
            terminal.result.status = Status::AttentionRequired;
            terminal.result.repeat_safe = false;
        }
        let event = if self.workspaces.attention(workspace).is_some() {
            DomainEvent::AttentionRequired {
                invocation: terminal.result.invocation.clone(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            }
        } else {
            match terminal.result.status {
                Status::Blocked
                    if !matches!(
                        terminal.decision.reason,
                        ReasonCode::AuditUnavailable
                            | ReasonCode::RuntimeHold
                            | ReasonCode::SessionEnded
                    ) =>
                {
                    DomainEvent::WorkBlocked {
                        invocation: terminal.result.invocation.clone(),
                        workspace: workspace.as_str().into(),
                        physical_id: terminal.physical_id,
                        presentation: denial_presentation(tool, &terminal.result),
                    }
                }
                Status::AttentionRequired => DomainEvent::AttentionRequired {
                    invocation: terminal.result.invocation.clone(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                },
                _ => DomainEvent::WorkCompleted {
                    invocation: terminal.result.invocation.clone(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                },
            }
        };
        if step.is_none() {
            self.emit(event);
        }
        let status = serde_json::to_value(terminal.result.status)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        let effect = serde_json::to_value(terminal.result.effect)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        self.diagnostics.sink().emit(
            if terminal.result.status == Status::Failed {
                ghostlight_bridge::diagnostics::event::OPERATION_FAILED
            } else {
                ghostlight_bridge::diagnostics::event::OPERATION_COMPLETED
            },
            if terminal.result.status == Status::Failed {
                ghostlight_bridge::diagnostics::Level::Warn
            } else {
                ghostlight_bridge::diagnostics::Level::Info
            },
            Some(terminal.result.invocation.as_str()),
            &format!("{tool} {status} {effect} {}ms", duration_ms),
        );
        let observed = self
            .take_observation(&terminal.result.invocation)
            .merged(terminal.observed.clone());
        // Unconsumed candidate sets belong only to stale-target failures; drop strays here so
        // nothing leaks across invocations.
        let _ = self.take_stale_candidates(&terminal.result.invocation);
        let mut record = AuditRecord::now(
            &terminal.result.invocation,
            workspace.as_str(),
            tool,
            requirements,
            snapshot.id(),
            terminal.decision,
            &status,
            &effect,
            &terminal.audit,
            duration_ms,
        )
        .from_channel(channel)
        .with_peer_image(peer_image)
        .with_policy(snapshot, terminal.decision)
        .with_observation(observed);
        record.step = step;
        record.permissions = self.take_permissions(&terminal.result.invocation);
        let storage = self.audit.record(&record);
        terminal.result.summary = if storage == language::audit_health::Storage::Saved {
            language::audit_health::qualify_children(
                &terminal.result.summary,
                terminal.audit.unconfirmed_history_steps(),
            )
        } else {
            storage.qualify(&terminal.result.summary)
        };
        terminal.result.history_storage = storage;
        gate.complete(terminal.result)
            .expect("single executor completion path");
        terminal.result = gate.take().expect("completion committed");
        terminal
    }

    /// Execute and complete an ordinary child without taking another lease or snapshot.
    pub(super) fn run_child(
        &self,
        context: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
        step: StepReceipt,
    ) -> Terminal {
        let _ = self.take_permissions(context.invocation);
        let started = Instant::now();
        let context = InvocationContext {
            requirements: language::capability_map::requirements(operation),
            ..*context
        };
        let terminal = self.run(&context, lease, operation);
        self.complete_terminal(
            &CompletionGate::default(),
            terminal,
            Completion {
                workspace: context.workspace,
                tool: operation.name(),
                requirements: language::capability_map::requirements(operation),
                snapshot: context.snapshot,
                duration_ms: elapsed_ms(started),
                channel: self.workspaces.channel(context.workspace).ok(),
                peer_image: self.workspaces.peer_image(context.workspace).ok().flatten(),
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
        .from_channel(self.workspaces.channel(context.workspace).ok());
        record.step = Some(step);
        self.audit.record(&record)
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
