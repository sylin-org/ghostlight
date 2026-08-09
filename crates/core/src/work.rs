// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Protocol-neutral work admitted by the persistent service.
//!
//! Wire lifecycle and request correlation stop at the client edge. The service receives one
//! immutable [`WorkContext`] plus a cooperative [`CancellationToken`]. The token can retire work
//! before dispatch or between composition steps; it never claims to roll back an atomic browser
//! action that has already been sent.

use crate::governance::overlay::SessionOverlay;
use crate::governance::ports::ClientInfo;
use ghostlight_transport::operation::{Operation, OperationKind};
use ghostlight_transport::workspace_id::WorkspaceId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// Immutable product context for one admitted operation.
#[derive(Clone)]
pub struct WorkContext {
    workspace: Option<WorkspaceId>,
    operation: Operation,
    client: Option<ClientInfo>,
    restriction: Option<Arc<SessionOverlay>>,
}

impl WorkContext {
    /// Construct a complete context after workspace and restriction validation.
    pub fn new(
        workspace: Option<WorkspaceId>,
        operation: Operation,
        client: Option<ClientInfo>,
        restriction: Option<Arc<SessionOverlay>>,
    ) -> Self {
        Self {
            workspace,
            operation,
            client,
            restriction,
        }
    }

    /// Return the service-owned browser workspace for this work.
    pub fn workspace(&self) -> Option<&WorkspaceId> {
        self.workspace.as_ref()
    }

    /// Return the private browser-routing key used by existing service internals.
    ///
    /// Independent service-local tools have no workspace and share a non-bearer sentinel. No
    /// browser-bound descriptor may use that sentinel.
    pub(crate) fn routing_key(&self) -> &str {
        self.workspace
            .as_ref()
            .map(WorkspaceId::as_str)
            .unwrap_or("service-local")
    }

    /// Return the complete canonical operation admitted for this work.
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// Return the closed semantic key used by validation, governance, scheduling, and audit.
    pub const fn operation_kind(&self) -> OperationKind {
        self.operation.kind()
    }

    /// Return presentation metadata for this call only.
    pub fn client(&self) -> Option<&ClientInfo> {
        self.client.as_ref()
    }

    /// Return the validated tighten-only restriction for this call, when present.
    pub fn restriction(&self) -> Option<&SessionOverlay> {
        self.restriction.as_deref()
    }

    /// Derive one immutable child context for a canonical sequence step.
    pub(crate) fn child(&self, operation: Operation) -> Self {
        Self {
            workspace: self.workspace.clone(),
            operation,
            client: self.client.clone(),
            restriction: self.restriction.clone(),
        }
    }
}

/// Cloneable cooperative cancellation signal for one active work item.
#[derive(Clone, Default)]
pub struct CancellationToken {
    inner: Arc<CancellationState>,
}

#[derive(Default)]
struct CancellationState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancellationToken {
    /// Create an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the work cancelled and wake queue or composition waiters.
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notify.notify_waiters();
        }
    }

    /// Return whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    /// Wait until cancellation is requested.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let notified = self.inner.notify.notified();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancellation_is_sticky_and_wakes_waiters() {
        let token = CancellationToken::new();
        let waiter = token.clone();
        let task = tokio::spawn(async move {
            waiter.cancelled().await;
            waiter.is_cancelled()
        });
        token.cancel();
        assert!(task.await.unwrap());
        token.cancel();
        token.cancelled().await;
    }
}
