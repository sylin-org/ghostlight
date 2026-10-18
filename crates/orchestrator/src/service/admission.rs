//! Small bounded session queues. Fixed workers absorb bursts; independent controls keep a lane.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::lifetime::{Drain, InFlight};
use crate::language::outcome::CancelCause;
use crate::work::{InvocationId, PreparedInvocation};

/// The most ordinary calls one session holds at once, the running one included.
pub(super) const SESSION_WORK: usize = 32;
const SESSION_CONTROLS: usize = 8;
const SESSION_BYTES: usize = 16 * 1024 * 1024;
const SERVICE_BYTES: usize = 64 * 1024 * 1024;
const CONTROL_BYTES: usize = 8 * 1024 * 1024;
pub(super) const HANDSHAKES: usize = 16;
pub(super) const SESSIONS: usize = 64;
/// The most `desktop` streams at once: one shell and one being turned away or handed over.
pub(super) const DESKTOPS: usize = 2;

/// Shared counted capacity, released when its owning resource leaves scope.
#[derive(Clone)]
pub(super) struct Capacity {
    used: Arc<AtomicUsize>,
    limit: usize,
}

impl Capacity {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            used: Arc::default(),
            limit,
        }
    }

    pub(super) fn acquire(&self, amount: usize) -> Option<Permit> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(amount).filter(|next| *next <= self.limit)
            })
            .ok()
            .map(|_| Permit {
                used: self.used.clone(),
                amount,
            })
    }
}

pub(super) struct Permit {
    used: Arc<AtomicUsize>,
    amount: usize,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.used.fetch_sub(self.amount, Ordering::AcqRel);
    }
}

/// Separate memory reservations keep work bursts from consuming the control lane.
#[derive(Clone)]
pub(super) struct QueueBudget {
    work: Capacity,
    controls: Capacity,
}
impl Default for QueueBudget {
    fn default() -> Self {
        Self {
            work: Capacity::new(SERVICE_BYTES),
            controls: Capacity::new(CONTROL_BYTES),
        }
    }
}

/// An admitted request, held until its worker completes it, with the reservations it holds.
struct Active {
    request: Arc<PreparedInvocation>,
    bytes: usize,
    _permit: Permit,
    _flight: InFlight,
}

#[derive(Default)]
struct State {
    queues: [VecDeque<Arc<PreparedInvocation>>; 2],
    active: HashMap<InvocationId, Active>,
    bytes: [usize; 2],
    counts: [usize; 2],
    stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refused {
    Full,
    Closed,
    ShuttingDown,
}

pub(super) struct SessionQueue {
    state: Mutex<State>,
    available: Condvar,
    budget: QueueBudget,
    drain: Arc<Drain>,
}

impl SessionQueue {
    pub(super) fn new(budget: QueueBudget, drain: Arc<Drain>) -> Self {
        Self {
            state: Mutex::default(),
            available: Condvar::new(),
            budget,
            drain,
        }
    }

    /// Count an invocation that runs outside the queue, or refuse it because a quit has begun.
    pub(super) fn begin_unqueued(&self) -> Result<InFlight, Refused> {
        self.drain.admit().map_err(|_| Refused::ShuttingDown)
    }

    /// Queue `request` in its lane, reserving `bytes` of the session's and the service's budget.
    ///
    /// Its id is unique: the MCP session mints one per call and refuses a client id still
    /// running.
    pub(super) fn admit(
        &self,
        request: Arc<PreparedInvocation>,
        bytes: usize,
    ) -> Result<(), Refused> {
        let mut state = super::lock(&self.state);
        if state.stopped {
            return Err(Refused::Closed);
        }
        let flight = self.begin_unqueued()?;
        let lane = usize::from(request.independent());
        let count_limit = if lane == 0 {
            SESSION_WORK
        } else {
            SESSION_CONTROLS
        };
        if state.counts[lane] >= count_limit
            || bytes > SESSION_BYTES.saturating_sub(state.bytes[lane])
        {
            return Err(Refused::Full);
        }
        let capacity = if lane == 0 {
            &self.budget.work
        } else {
            &self.budget.controls
        };
        let permit = capacity.acquire(bytes).ok_or(Refused::Full)?;
        state.bytes[lane] += bytes;
        state.counts[lane] += 1;
        state.queues[lane].push_back(Arc::clone(&request));
        state.active.insert(
            request.id.clone(),
            Active {
                request,
                bytes,
                _permit: permit,
                _flight: flight,
            },
        );
        self.available.notify_all();
        Ok(())
    }

    /// The next request of one lane, waiting for one; None once the queue has stopped and drained.
    pub(super) fn next(&self, independent: bool) -> Option<Arc<PreparedInvocation>> {
        let mut state = super::lock(&self.state);
        loop {
            // On disconnect drain cancelled work through completion; never execute its effects.
            if let Some(request) = state.queues[usize::from(independent)].pop_front() {
                return Some(request);
            }
            if state.stopped {
                return None;
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub(super) fn complete(&self, id: &InvocationId) {
        let mut state = super::lock(&self.state);
        if let Some(active) = state.active.remove(id) {
            let lane = usize::from(active.request.independent());
            state.bytes[lane] -= active.bytes;
            state.counts[lane] -= 1;
        }
    }

    /// The agent cancelled the request it named: queued, it completes as cancelled at once;
    /// running, its dispatch stops.
    pub(super) fn cancel(&self, id: &InvocationId) {
        if let Some(active) = super::lock(&self.state).active.get(id) {
            active.request.cancel(CancelCause::Client);
        }
        self.advance_cancelled();
    }

    pub(super) fn advance_cancelled(&self) {
        let mut state = super::lock(&self.state);
        let mut waiting = VecDeque::new();
        while let Some(request) = state.queues[0].pop_front() {
            if request.is_cancelled() || request.expired() {
                state.queues[1].push_back(request);
            } else {
                waiting.push_back(request);
            }
        }
        state.queues[0] = waiting;
        self.available.notify_all();
    }

    /// Every admitted request not yet completed, queued or running.
    pub(super) fn admitted(&self) -> Vec<Arc<PreparedInvocation>> {
        super::lock(&self.state)
            .active
            .values()
            .map(|active| Arc::clone(&active.request))
            .collect()
    }

    /// Admit nothing more and cancel every admitted request for `cause`, the session's end or
    /// Ghostlight's quit. Queued requests still drain through their workers as cancelled.
    pub(super) fn stop(&self, cause: CancelCause) {
        let mut state = super::lock(&self.state);
        state.stopped = true;
        for active in state.active.values() {
            active.request.cancel(cause);
        }
        self.available.notify_all();
    }

    pub(super) fn wait_idle_or_timeout(&self, timeout: Duration) -> bool {
        let state = super::lock(&self.state);
        if state.stopped {
            return false;
        }
        let (state, _) = self
            .available
            .wait_timeout(state, timeout)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        !state.stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intake::mcp::{McpSession, Step};
    use crate::intake::session::{IntakeChannel, ServiceRequest};
    use crate::provenance::ConnectionEvidence;
    use crate::workspace::WorkspaceStore;
    use serde_json::json;

    /// A call to an ordinary tool, or to an independent one, under `id`, from its own connection.
    fn request_named(id: InvocationId, independent: bool) -> Arc<PreparedInvocation> {
        let connection = Arc::new(ConnectionEvidence::claimed(
            "queue test",
            IntakeChannel::Mcp,
        ));
        let workspace = WorkspaceStore::default().connect(Arc::clone(&connection), None);
        Arc::new(PreparedInvocation::new(
            id,
            if independent {
                "policy_explain"
            } else {
                "browser_tabs"
            },
            if independent {
                json!({})
            } else {
                json!({"action":"list"})
            },
            workspace,
            connection,
        ))
    }

    /// A call under a fresh id.
    fn request(independent: bool) -> Arc<PreparedInvocation> {
        request_named(InvocationId::mint(), independent)
    }

    #[test]
    fn a_queued_cancellation_bypasses_work_and_touches_no_later_request() {
        let queue = SessionQueue::new(QueueBudget::default(), Arc::default());
        let cancelled = request(false);
        queue.admit(Arc::clone(&cancelled), 512).unwrap();
        queue.cancel(&cancelled.id);
        assert!(cancelled.is_cancelled());
        assert_eq!(queue.next(true).unwrap().id, cancelled.id);
        queue.complete(&cancelled.id);
        queue.admit(request(false), 512).unwrap();
        assert!(!queue.next(false).unwrap().is_cancelled());
    }

    #[test]
    fn a_cancel_reaches_the_invocation_the_client_named() {
        let mut session = McpSession::new();
        session.accept(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "queue test", "version": "1"}
            }})
            .to_string(),
        );
        session.accept(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        let mut forwarded = |line: &str| {
            session
                .accept(line)
                .into_iter()
                .find_map(|step| match step {
                    Step::Forward(request) => Some(request),
                    Step::Reply(_) => None,
                })
                .expect("the line is forwarded to the queue")
        };
        let queue = SessionQueue::new(QueueBudget::default(), Arc::default());
        let mut requests = Vec::new();
        for client_id in ["\"a\"", "7"] {
            let ServiceRequest::Invoke { id, .. } = forwarded(&format!(
                r#"{{"jsonrpc":"2.0","id":{client_id},"method":"tools/call","params":{{"name":"browser_tabs","arguments":{{"action":"list"}}}}}}"#
            )) else {
                panic!("a call is an invocation");
            };
            let request = request_named(id, false);
            queue.admit(Arc::clone(&request), 512).unwrap();
            requests.push(request);
        }
        let ServiceRequest::Cancel { id } = forwarded(
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}"#,
        ) else {
            panic!("a cancel of a running call is forwarded");
        };
        assert_eq!(id, requests[1].id);
        queue.cancel(&id);
        assert!(requests[1].is_cancelled());
        assert!(!requests[0].is_cancelled());
        assert_eq!(queue.next(true).unwrap().id, requests[1].id);
    }

    #[test]
    fn bursts_are_fifo_and_full_work_preserves_controls_and_another_session() {
        let budget = QueueBudget::default();
        let queue = SessionQueue::new(budget.clone(), Arc::default());
        let burst: Vec<_> = (0..SESSION_WORK).map(|_| request(false)).collect();
        for queued in &burst {
            queue.admit(Arc::clone(queued), 512).unwrap();
        }
        assert_eq!(queue.admit(request(false), 512), Err(Refused::Full));
        let control = request(true);
        queue.admit(Arc::clone(&control), 512).unwrap();
        assert_eq!(queue.next(true).unwrap().id, control.id);
        let other = SessionQueue::new(budget, Arc::default());
        let elsewhere = request(false);
        other.admit(Arc::clone(&elsewhere), 512).unwrap();
        assert_eq!(other.next(false).unwrap().id, elsewhere.id);
        for queued in &burst {
            assert_eq!(queue.next(false).unwrap().id, queued.id);
            queue.complete(&queued.id);
        }
        queue.admit(request(false), 512).unwrap();
    }

    #[test]
    fn memory_and_connection_reservations_release_on_every_exit() {
        let capacity = Capacity::new(2);
        let one = capacity.acquire(1).unwrap();
        let two = capacity.acquire(1).unwrap();
        assert!(capacity.acquire(1).is_none());
        drop(one);
        drop(two);
        assert!(capacity.acquire(2).is_some());
        let budget = QueueBudget {
            work: Capacity::new(1024),
            controls: Capacity::new(1024),
        };
        let queue = SessionQueue::new(budget.clone(), Arc::default());
        let big = request(false);
        queue.admit(Arc::clone(&big), 1024).unwrap();
        let other = SessionQueue::new(budget, Arc::default());
        assert_eq!(other.admit(request(false), 1), Err(Refused::Full));
        other.admit(request(true), 512).unwrap();
        queue.stop(CancelCause::SessionEnded);
        assert!(queue.next(false).unwrap().is_cancelled());
        queue.complete(&big.id);
        other.admit(request(false), 1024).unwrap();
    }

    #[test]
    fn a_quit_refuses_admission_and_a_finished_request_leaves_the_drain() {
        let drain = Arc::new(Drain::default());
        let queue = SessionQueue::new(QueueBudget::default(), drain.clone());
        let running = request(false);
        queue.admit(Arc::clone(&running), 512).unwrap();
        assert_eq!(drain.in_flight(), 1);
        drain.begin_quit();
        assert_eq!(queue.admit(request(false), 512), Err(Refused::ShuttingDown));
        assert_eq!(queue.begin_unqueued().err(), Some(Refused::ShuttingDown));
        assert_eq!(queue.next(false).unwrap().id, running.id);
        assert_eq!(drain.in_flight(), 1, "a request runs until it completes");
        queue.complete(&running.id);
        assert_eq!(drain.in_flight(), 0);
        assert!(drain.wait_idle(Duration::from_millis(10)));
    }
}
