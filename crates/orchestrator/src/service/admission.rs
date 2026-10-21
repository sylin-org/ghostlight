//! Small bounded session queues. Fixed workers absorb bursts; independent controls keep a lane.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::lifetime::{Drain, InFlight, ShuttingDown};
use crate::language::outcome::CancelCause;
use crate::work::{InvocationId, PreparedInvocation};

/// The most ordinary calls one session holds at once, the running one included.
const SESSION_WORK: usize = 32;
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
    /// Why the queue stopped admitting, once it has.
    stopped: Option<CancelCause>,
}

/// Why the queue did not admit a request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refused {
    /// The request's lane or the service's budget is full.
    Full,
    /// The queue admits nothing more, for this cause: Ghostlight is quitting, or the queue was
    /// stopped. The request is answered as cancelled before it started.
    Cancelled(CancelCause),
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

    /// Count an invocation that runs outside the queue, or refuse it because a quit has begun,
    /// naming the cause it is cancelled for.
    pub(super) fn begin_unqueued(&self) -> Result<InFlight, CancelCause> {
        self.drain
            .admit()
            .map_err(|ShuttingDown(quit)| CancelCause::Quit(quit))
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
        if let Some(cause) = state.stopped {
            return Err(Refused::Cancelled(cause));
        }
        let flight = self.begin_unqueued().map_err(Refused::Cancelled)?;
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
            if state.stopped.is_some() {
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
    /// Ghostlight's quit. Queued requests still drain through their workers as cancelled, and a
    /// request that arrives later is refused for the first cause.
    pub(super) fn stop(&self, cause: CancelCause) {
        let mut state = super::lock(&self.state);
        state.stopped.get_or_insert(cause);
        for active in state.active.values() {
            active.request.cancel(cause);
        }
        self.available.notify_all();
    }

    pub(super) fn wait_idle_or_timeout(&self, timeout: Duration) -> bool {
        let state = super::lock(&self.state);
        if state.stopped.is_some() {
            return false;
        }
        let (state, _) = self
            .available
            .wait_timeout(state, timeout)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.stopped.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intake::session::IntakeChannel;
    use crate::provenance::ConnectionEvidence;
    use crate::workspace::WorkspaceStore;
    use serde_json::json;

    /// A call to an ordinary tool, or to an independent one, under a fresh id, from its own
    /// connection.
    fn request(independent: bool) -> Arc<PreparedInvocation> {
        let connection = Arc::new(ConnectionEvidence::claimed(
            "queue test",
            IntakeChannel::Mcp,
        ));
        let workspace = WorkspaceStore::default().connect(Arc::clone(&connection), None);
        Arc::new(PreparedInvocation::new(
            InvocationId::mint(),
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

    /// ADR-0160's bounds, the quit's drain (docs/1.0/ARCHITECTURE.md) and ACCEPTANCE C22: from
    /// admission until it completes, however it ends, a call holds its bytes of the service's
    /// budget and is work in flight, so a quit waits for it and the idle exit counts it as held
    /// work; once it completes it holds neither, so no reservation outlives its call.
    #[test]
    fn a_call_holds_its_bytes_and_counts_as_work_in_flight_until_it_completes() {
        let drain = Arc::new(Drain::default());
        let budget = QueueBudget {
            work: Capacity::new(1024),
            controls: Capacity::new(1024),
        };
        let queue = SessionQueue::new(budget.clone(), Arc::clone(&drain));
        let other = SessionQueue::new(budget, Arc::clone(&drain));

        // A call that runs holds the service's whole ordinary budget and is work in flight.
        let running = request(false);
        queue.admit(Arc::clone(&running), 1024).unwrap();
        assert_eq!(queue.next(false).unwrap().id, running.id);
        assert_eq!(drain.in_flight(), 1, "the idle exit counts it as held work");
        assert!(!drain.wait_idle(Duration::ZERO), "a quit waits for it");
        assert_eq!(
            other.admit(request(false), 1),
            Err(Refused::Full),
            "another session's call meets the bytes it holds"
        );
        // The independent lane keeps a budget of its own.
        let control = request(true);
        other.admit(Arc::clone(&control), 512).unwrap();
        assert_eq!(other.next(true).unwrap().id, control.id);
        other.complete(&control.id);

        queue.complete(&running.id);
        assert_eq!(drain.in_flight(), 0);
        assert!(
            drain.wait_idle(Duration::ZERO),
            "nothing holds a quit any more"
        );

        // A call its session's end cancels drains through its worker and lets go the same way.
        let waiting = request(false);
        other.admit(Arc::clone(&waiting), 1024).unwrap();
        other.stop(CancelCause::SessionEnded);
        let cancelled = other.next(false).unwrap();
        assert_eq!(cancelled.id, waiting.id);
        assert!(cancelled.is_cancelled());
        assert_eq!(
            drain.in_flight(),
            1,
            "it is work in flight until it completes"
        );
        other.complete(&cancelled.id);
        assert_eq!(drain.in_flight(), 0);
        queue.admit(request(false), 1024).unwrap();
    }
}
