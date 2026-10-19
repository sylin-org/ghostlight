//! One receipt/completion seam for direct operations, composition children, and preparation facts.

use super::*;
use crate::language::history::StepReceipt;
use crate::provenance::Attribution;

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
            provenance,
            findings,
        } = completion;
        if let Some(coverage) = &findings.coverage {
            terminal.result.summary =
                language::coverage::qualify(&terminal.result.summary, coverage);
            terminal.audit = terminal.audit.with_coverage(coverage);
            terminal.result.facts["coverage"] = json!(coverage);
        }
        let tool = language::audit::tool_name(tool);
        let event = match terminal.result.status {
            Status::Blocked
                if !matches!(
                    terminal.decision.reason,
                    ReasonCode::AuditUnavailable
                        | ReasonCode::RuntimeHold
                        | ReasonCode::SessionEnded
                ) && terminal.audit.refusal()
                    != Some(&crate::language::audit::AuditRefusal::CredentialAuthorization)
                    && (terminal.audit.composition().is_none() || !terminal.decision.allowed) =>
            {
                DomainEvent::WorkBlocked {
                    invocation: terminal.result.invocation.to_string(),
                    workspace: workspace.as_str().into(),
                    physical_id: terminal.physical_id,
                    presentation: denial_presentation(tool, &terminal.result),
                }
            }
            _ => DomainEvent::WorkCompleted {
                invocation: terminal.result.invocation.to_string(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            },
        };
        if step.is_none() {
            self.emit(event);
        } else {
            // A child finishes its own decoration without settling its parent's lifecycle
            // or showing a guardrail notice for every unsuccessful child in a composition.
            self.presentation.react(&DomainEvent::WorkCompleted {
                invocation: terminal.result.invocation.to_string(),
                workspace: workspace.as_str().into(),
                physical_id: terminal.physical_id,
            });
        }
        let status = terminal.result.status.stored_name();
        let effect = terminal.result.effect.stored_name();
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
            &operation_line(
                tool,
                &status,
                &effect,
                duration_ms,
                provenance.attribution(),
            ),
        );
        let observed = findings.observed.merged(terminal.observed.clone());
        let mut record = AuditRecord::now(
            terminal.result.invocation.as_str(),
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
        .with_provenance(Some(provenance.attribution()))
        .with_policy(snapshot, terminal.decision)
        .with_observation(observed)
        // The record names a browser only when one received a command of this frame.
        .with_browser(findings.reached);
        record.step = step;
        record.permissions = findings.permissions;
        let storage = self.audit.record_with_provenance(&record, Some(provenance));
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
    ///
    /// The step runs in its own frame, so its record holds only its own checks, coverage and
    /// browser. The flow takes one fact back: the browser the step reached.
    pub(super) fn run_child(
        &self,
        flow: &InvocationContext<'_>,
        lease: &WorkspaceLease,
        operation: &Operation,
        step: StepReceipt,
    ) -> Terminal {
        let started = Instant::now();
        let frame = Frame::of(operation);
        let context = flow.step(operation, &frame);
        self.emit(DomainEvent::WorkPhaseStarted {
            invocation: context.invocation.into(),
            workspace: context.workspace.as_str().into(),
            physical_id: None,
            activity: operation_activity(operation),
        });
        let terminal = self.run(&context, lease, operation);
        if let Some(browser) = context.findings.reached() {
            flow.findings.reach(browser);
        }
        self.complete_terminal(
            &CompletionGate::default(),
            terminal,
            Completion::of(&context, operation.name(), started),
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
        let record = AuditRecord::not_started(
            context.invocation.as_str(),
            context.workspace.as_str(),
            language::audit::tool_name(tool),
            context.snapshot,
            context.provenance.attribution(),
            step,
        );
        self.audit
            .record_with_provenance(&record, Some(context.provenance))
    }
}

/// The diagnostics line of one completed operation: what ran, how it ended, how long it took and
/// the connection that sent it. Never the client's claim (ADR-0161).
fn operation_line(
    tool: &str,
    status: &str,
    effect: &str,
    duration_ms: u64,
    attribution: &Attribution,
) -> String {
    let line = format!("{tool} {status} {effect} {duration_ms}ms");
    match attribution.connection_id.as_deref() {
        Some(connection) => format!("{line} on {connection}"),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::operation_line;
    use crate::intake::session::IntakeChannel;
    use crate::provenance::{Attribution, ConnectionEvidence};

    #[test]
    fn operation_lines_name_the_connection() {
        let evidence = ConnectionEvidence::new(
            Attribution::accepted(IntakeChannel::Mcp),
            Some("PRIVATE_CLAIM"),
        );
        let attribution = evidence.attribution();
        let connection = attribution.connection_id.as_deref().unwrap();
        let line = operation_line("browser_read", "succeeded", "none", 12, attribution);
        assert_eq!(
            line,
            format!("browser_read succeeded none 12ms on {connection}")
        );
        assert!(!line.contains("PRIVATE_CLAIM"));

        let restored = Attribution {
            connection_id: None,
            ..Attribution::accepted(IntakeChannel::Cli)
        };
        assert_eq!(
            operation_line("browser_click", "failed", "unknown", 3, &restored),
            "browser_click failed unknown 3ms"
        );
    }
}
