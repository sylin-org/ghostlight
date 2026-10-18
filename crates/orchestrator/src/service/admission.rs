//! Small bounded session queues. Fixed workers absorb bursts; independent controls keep a lane.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use super::lifetime::{Drain, InFlight};
use crate::work::{CancellationToken, InvocationId, PreparedInvocation};

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

#[derive(Clone)]
pub(super) struct Job {
    pub id: InvocationId,
    pub prepared: Arc<PreparedInvocation>,
    pub cancellation: CancellationToken,
}

struct Active {
    job: Job,
    bytes: usize,
    _permit: Permit,
    _flight: InFlight,
}

#[derive(Default)]
struct State {
    queues: [VecDeque<Job>; 2],
    active: HashMap<InvocationId, Active>,
    bytes: [usize; 2],
    counts: [usize; 2],
    stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refused {
    Duplicate,
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

    pub(super) fn admit(&self, job: Job, bytes: usize) -> Result<(), Refused> {
        let mut state = super::lock(&self.state);
        if state.stopped {
            return Err(Refused::Closed);
        }
        let flight = self.begin_unqueued()?;
        // Never replace the cancellation token of a request already admitted.
        if state.active.contains_key(&job.id) {
            return Err(Refused::Duplicate);
        }
        let lane = usize::from(job.prepared.independent());
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
        state.queues[lane].push_back(job.clone());
        state.active.insert(
            job.id.clone(),
            Active {
                job,
                bytes,
                _permit: permit,
                _flight: flight,
            },
        );
        self.available.notify_all();
        Ok(())
    }

    pub(super) fn next(&self, independent: bool) -> Option<Job> {
        let mut state = super::lock(&self.state);
        loop {
            // On disconnect drain cancelled work through completion; never execute its effects.
            if let Some(job) = state.queues[usize::from(independent)].pop_front() {
                return Some(job);
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
            let lane = usize::from(active.job.prepared.independent());
            state.bytes[lane] -= active.bytes;
            state.counts[lane] -= 1;
        }
    }

    pub(super) fn cancel(&self, id: &InvocationId) {
        if let Some(active) = super::lock(&self.state).active.get(id) {
            active.job.cancellation.cancel();
        }
        self.advance_cancelled();
    }

    pub(super) fn advance_cancelled(&self) {
        let mut state = super::lock(&self.state);
        let mut waiting = VecDeque::new();
        while let Some(job) = state.queues[0].pop_front() {
            if job.cancellation.is_cancelled() || job.prepared.expired() {
                state.queues[1].push_back(job);
            } else {
                waiting.push_back(job);
            }
        }
        state.queues[0] = waiting;
        self.available.notify_all();
    }

    pub(super) fn jobs(&self) -> Vec<Job> {
        super::lock(&self.state)
            .active
            .values()
            .map(|active| active.job.clone())
            .collect()
    }

    pub(super) fn stop(&self) {
        let mut state = super::lock(&self.state);
        state.stopped = true;
        for active in state.active.values() {
            active.job.cancellation.cancel();
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
    use crate::intake::session::ServiceRequest;
    use serde_json::json;

    /// A queued call to an ordinary tool, or to an independent one, under `id`.
    fn job_named(id: InvocationId, independent: bool) -> Job {
        Job {
            id: id.clone(),
            cancellation: CancellationToken::default(),
            prepared: Arc::new(PreparedInvocation::new(
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
                None,
                None,
            )),
        }
    }

    /// A queued call under a fresh id.
    fn job(independent: bool) -> Job {
        job_named(InvocationId::mint(), independent)
    }

    #[test]
    fn duplicates_keep_the_original_token_and_queued_cancellation_bypasses_work() {
        let queue = SessionQueue::new(QueueBudget::default(), Arc::default());
        let original = job(false);
        let same = original.id.clone();
        queue.admit(original.clone(), 512).unwrap();
        assert_eq!(
            queue.admit(job_named(same.clone(), false), 512),
            Err(Refused::Duplicate)
        );
        queue.cancel(&same);
        assert!(original.cancellation.is_cancelled());
        assert_eq!(queue.next(true).unwrap().id, same);
        queue.complete(&same);
        queue.admit(job_named(same, false), 512).unwrap();
        assert!(!queue.next(false).unwrap().cancellation.is_cancelled());
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
        let mut jobs = Vec::new();
        for client_id in ["\"a\"", "7"] {
            let ServiceRequest::Invoke { id, .. } = forwarded(&format!(
                r#"{{"jsonrpc":"2.0","id":{client_id},"method":"tools/call","params":{{"name":"browser_tabs","arguments":{{"action":"list"}}}}}}"#
            )) else {
                panic!("a call is an invocation");
            };
            let job = job_named(id, false);
            queue.admit(job.clone(), 512).unwrap();
            jobs.push(job);
        }
        let ServiceRequest::Cancel { id } = forwarded(
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}"#,
        ) else {
            panic!("a cancel of a running call is forwarded");
        };
        assert_eq!(id, jobs[1].prepared.id);
        queue.cancel(&id);
        assert!(jobs[1].cancellation.is_cancelled());
        assert!(!jobs[0].cancellation.is_cancelled());
        assert_eq!(queue.next(true).unwrap().id, jobs[1].id);
    }

    #[test]
    fn bursts_are_fifo_and_full_work_preserves_controls_and_another_session() {
        let budget = QueueBudget::default();
        let queue = SessionQueue::new(budget.clone(), Arc::default());
        let burst: Vec<Job> = (0..SESSION_WORK).map(|_| job(false)).collect();
        for queued in &burst {
            queue.admit(queued.clone(), 512).unwrap();
        }
        assert_eq!(queue.admit(job(false), 512), Err(Refused::Full));
        let control = job(true);
        queue.admit(control.clone(), 512).unwrap();
        assert_eq!(queue.next(true).unwrap().id, control.id);
        let other = SessionQueue::new(budget, Arc::default());
        let elsewhere = job(false);
        other.admit(elsewhere.clone(), 512).unwrap();
        assert_eq!(other.next(false).unwrap().id, elsewhere.id);
        for queued in &burst {
            assert_eq!(queue.next(false).unwrap().id, queued.id);
            queue.complete(&queued.id);
        }
        queue.admit(job(false), 512).unwrap();
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
        let big = job(false);
        queue.admit(big.clone(), 1024).unwrap();
        let other = SessionQueue::new(budget, Arc::default());
        assert_eq!(other.admit(job(false), 1), Err(Refused::Full));
        other.admit(job(true), 512).unwrap();
        queue.stop();
        assert!(queue.next(false).unwrap().cancellation.is_cancelled());
        queue.complete(&big.id);
        other.admit(job(false), 1024).unwrap();
    }

    #[test]
    fn a_quit_refuses_admission_and_a_finished_job_leaves_the_drain() {
        let drain = Arc::new(Drain::default());
        let queue = SessionQueue::new(QueueBudget::default(), drain.clone());
        let running = job(false);
        queue.admit(running.clone(), 512).unwrap();
        assert_eq!(drain.in_flight(), 1);
        drain.begin_quit();
        assert_eq!(queue.admit(job(false), 512), Err(Refused::ShuttingDown));
        assert_eq!(queue.begin_unqueued().err(), Some(Refused::ShuttingDown));
        assert_eq!(queue.next(false).unwrap().id, running.id);
        assert_eq!(drain.in_flight(), 1, "a job runs until it completes");
        queue.complete(&running.id);
        assert_eq!(drain.in_flight(), 0);
        assert!(drain.wait_idle(Duration::from_millis(10)));
    }
}
