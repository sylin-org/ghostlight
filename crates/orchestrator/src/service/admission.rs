//! Small bounded session queues. Fixed workers absorb bursts; independent controls keep a lane.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::work::{CancellationToken, PreparedInvocation};

const SESSION_WORK: usize = 32;
const SESSION_CONTROLS: usize = 8;
const SESSION_BYTES: usize = 16 * 1024 * 1024;
const SERVICE_BYTES: usize = 64 * 1024 * 1024;
const CONTROL_BYTES: usize = 8 * 1024 * 1024;
pub(super) const HANDSHAKES: usize = 16;
pub(super) const SESSIONS: usize = 64;

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
    pub id: String,
    pub prepared: Arc<PreparedInvocation>,
    pub cancellation: CancellationToken,
}

struct Active {
    job: Job,
    bytes: usize,
    _permit: Permit,
}

#[derive(Default)]
struct State {
    queues: [VecDeque<Job>; 2],
    active: HashMap<String, Active>,
    bytes: [usize; 2],
    counts: [usize; 2],
    stopped: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refused {
    Duplicate,
    Full,
    Closed,
}

pub(super) struct SessionQueue {
    state: Mutex<State>,
    available: Condvar,
    budget: QueueBudget,
}

impl SessionQueue {
    pub(super) fn new(budget: QueueBudget) -> Self {
        Self {
            state: Mutex::default(),
            available: Condvar::new(),
            budget,
        }
    }

    pub(super) fn admit(&self, job: Job, bytes: usize) -> Result<(), Refused> {
        let mut state = super::lock(&self.state);
        if state.stopped {
            return Err(Refused::Closed);
        }
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

    pub(super) fn complete(&self, id: &str) {
        let mut state = super::lock(&self.state);
        if let Some(active) = state.active.remove(id) {
            let lane = usize::from(active.job.prepared.independent());
            state.bytes[lane] -= active.bytes;
            state.counts[lane] -= 1;
        }
    }

    pub(super) fn cancel(&self, id: &str) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn job(id: &str, independent: bool) -> Job {
        Job {
            id: id.into(),
            cancellation: CancellationToken::default(),
            prepared: Arc::new(PreparedInvocation::new(
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

    #[test]
    fn duplicates_keep_the_original_token_and_queued_cancellation_bypasses_work() {
        let queue = SessionQueue::new(QueueBudget::default());
        let original = job("same", false);
        queue.admit(original.clone(), 512).unwrap();
        assert_eq!(
            queue.admit(job("same", false), 512),
            Err(Refused::Duplicate)
        );
        queue.cancel("same");
        assert!(original.cancellation.is_cancelled());
        assert_eq!(queue.next(true).unwrap().id, "same");
        queue.complete("same");
        queue.admit(job("same", false), 512).unwrap();
        assert!(!queue.next(false).unwrap().cancellation.is_cancelled());
    }

    #[test]
    fn bursts_are_fifo_and_full_work_preserves_controls_and_another_session() {
        let budget = QueueBudget::default();
        let queue = SessionQueue::new(budget.clone());
        for i in 0..SESSION_WORK {
            queue.admit(job(&i.to_string(), false), 512).unwrap();
        }
        assert_eq!(queue.admit(job("overflow", false), 512), Err(Refused::Full));
        queue.admit(job("control", true), 512).unwrap();
        assert_eq!(queue.next(true).unwrap().id, "control");
        let other = SessionQueue::new(budget);
        other.admit(job("other", false), 512).unwrap();
        assert_eq!(other.next(false).unwrap().id, "other");
        for i in 0..SESSION_WORK {
            assert_eq!(queue.next(false).unwrap().id, i.to_string());
            queue.complete(&i.to_string());
        }
        queue.admit(job("after", false), 512).unwrap();
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
        let queue = SessionQueue::new(budget.clone());
        queue.admit(job("big", false), 1024).unwrap();
        let other = SessionQueue::new(budget);
        assert_eq!(other.admit(job("next", false), 1), Err(Refused::Full));
        other.admit(job("status", true), 512).unwrap();
        queue.stop();
        assert!(queue.next(false).unwrap().cancellation.is_cancelled());
        queue.complete("big");
        other.admit(job("next", false), 1024).unwrap();
    }
}
