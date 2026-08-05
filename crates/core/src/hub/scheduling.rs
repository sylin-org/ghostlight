// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Resource-scoped command scheduling for browser-bound work (ADR-0080).
//!
//! This module owns dispatch admission, not browser transport. Page commands serialize on a
//! browser surface, topology commands serialize per workspace, and browser-wide commands exclude all
//! child work in the same browser slot. Producers retain FIFO order while round-robin selection
//! prevents one workspace producer from monopolizing a shared resource.

use crate::work::CancellationToken;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;
use tokio::sync::oneshot;

/// Default maximum queued commands for one resource.
pub const MAX_QUEUED_PER_RESOURCE: usize = 32;

/// Default maximum queued commands for one producer across all resources.
pub const MAX_QUEUED_PER_PRODUCER: usize = 128;

/// Default maximum queued commands across the service.
pub const MAX_QUEUED_GLOBAL: usize = 1024;

/// Default maximum time a command may wait before dispatch.
pub const DEFAULT_QUEUE_DEADLINE: Duration = Duration::from_secs(30);

/// The stable identity of a native browser tab within a browser connection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BrowserSurface {
    /// Service-assigned browser connection slot.
    pub browser_slot: u32,
    /// Browser-native tab identifier.
    pub native_tab: i64,
}

/// A producer of scheduled browser work, normally one service-owned workspace.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProducerId(String);

impl ProducerId {
    /// Construct a producer identifier from its stable service-local value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the stable service-local value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The resource on which a browser command must serialize.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ScheduleKey {
    /// Page state belonging to one native tab.
    Surface(BrowserSurface),
    /// Tab topology observed and mutated by one workspace within a browser slot.
    WorkspaceTopology {
        /// Service-assigned browser connection slot.
        browser_slot: u32,
        /// Stable workspace identity used to isolate topology ordering.
        workspace_key: String,
    },
    /// Browser-wide state. This excludes every child resource in the slot.
    Browser {
        /// Service-assigned browser connection slot.
        browser_slot: u32,
    },
}

impl ScheduleKey {
    fn browser_slot(&self) -> u32 {
        match self {
            Self::Surface(surface) => surface.browser_slot,
            Self::WorkspaceTopology { browser_slot, .. } | Self::Browser { browser_slot } => {
                *browser_slot
            }
        }
    }
}

/// Queue sizing and deadline policy for one scheduler instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerLimits {
    /// Maximum queued commands for one resource.
    pub per_resource: usize,
    /// Maximum queued commands for one producer across resources.
    pub per_producer: usize,
    /// Maximum queued commands across the service.
    pub global: usize,
    /// Maximum time a command may wait before dispatch.
    pub queue_deadline: Duration,
}

impl Default for SchedulerLimits {
    fn default() -> Self {
        Self {
            per_resource: MAX_QUEUED_PER_RESOURCE,
            per_producer: MAX_QUEUED_PER_PRODUCER,
            global: MAX_QUEUED_GLOBAL,
            queue_deadline: DEFAULT_QUEUE_DEADLINE,
        }
    }
}

/// Why queued work was retired before dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetirementReason {
    /// The governing authority changed while the command was queued.
    AuthorityChanged,
    /// A governance hold became active.
    Hold,
    /// The panic control was activated.
    Panic,
    /// The browser requires interactive user attention.
    Attention,
    /// The producing workspace retired.
    SessionEnded,
    /// The browser or tab resource ceased to exist.
    ResourceDestroyed,
    /// The native connection disappeared without proof that the browser process ended.
    BrowserDisconnected,
    /// The caller cancelled this queued work before browser dispatch.
    Cancelled,
}

/// A command that never reached browser dispatch.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ScheduleFailure {
    /// The command could not resolve a live browser resource.
    #[error("browser command has no live target: {reason}")]
    TargetUnavailable {
        /// Stable reason safe to return to the caller.
        reason: &'static str,
    },
    /// A queue bound rejected the command.
    #[error("browser command queue is full ({scope})")]
    Overloaded {
        /// The bound that rejected admission.
        scope: &'static str,
    },
    /// The command exhausted its queue deadline.
    #[error("browser command expired before dispatch")]
    QueueDeadline,
    /// Authority changed while the command waited.
    #[error("browser command retired before dispatch: authority changed")]
    AuthorityChanged,
    /// A lifecycle event retired the command.
    #[error("browser command retired before dispatch: {reason:?}")]
    Retired {
        /// The lifecycle event that retired the command.
        reason: RetirementReason,
    },
    /// Prior work has an unknown outcome on this resource.
    #[error("browser resource is quarantined after command {command_id} had an unknown outcome")]
    ResourceUncertain {
        /// The exact command whose terminal state is unknown.
        command_id: u64,
    },
    /// Cooperative cancellation won before browser dispatch.
    #[error("browser command was cancelled before dispatch")]
    Cancelled,
}

/// The dispatch path represented by an execution context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionClass {
    /// A normal resource-scoped browser command.
    Scheduled,
    /// Presentation traffic that does not compete with page mutation.
    Presentation,
    /// A service-local operation with no browser resource.
    Local,
    /// Safety or protocol recovery traffic that bypasses ordinary queues.
    SafetyProtocol,
}

/// Proof that a command is admitted to browser dispatch.
///
/// Scheduled contexts hold a resource lease until the last clone drops. Bypass contexts are typed
/// so transport APIs can reject unclassified sends without making presentation traffic contend
/// with page work.
#[derive(Clone)]
pub struct ExecutionContext {
    class: ExecutionClass,
    lease: Option<Arc<Lease>>,
}

impl fmt::Debug for ExecutionContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutionContext")
            .field("class", &self.class)
            .field("command_id", &self.command_id())
            .field("key", &self.key())
            .finish()
    }
}

impl ExecutionContext {
    /// Construct a presentation-only bypass context.
    pub fn presentation() -> Self {
        Self {
            class: ExecutionClass::Presentation,
            lease: None,
        }
    }

    /// Construct a service-local bypass context.
    pub fn local() -> Self {
        Self {
            class: ExecutionClass::Local,
            lease: None,
        }
    }

    /// Construct a safety or protocol recovery bypass context.
    pub fn safety_protocol() -> Self {
        Self {
            class: ExecutionClass::SafetyProtocol,
            lease: None,
        }
    }

    /// Return the dispatch class.
    pub fn class(&self) -> ExecutionClass {
        self.class
    }

    /// Return the scheduled command identifier, if this is a scheduled context.
    pub fn command_id(&self) -> Option<u64> {
        self.lease.as_ref().map(|lease| lease.command_id)
    }

    /// Return the scheduled resource, if this is a scheduled context.
    pub fn key(&self) -> Option<&ScheduleKey> {
        self.lease.as_ref().map(|lease| &lease.key)
    }

    /// Return the authority epoch captured when this work was admitted.
    pub fn authority_epoch(&self) -> Option<u64> {
        self.lease.as_ref().map(|lease| lease.authority_epoch)
    }

    /// Return whether this context authorizes dispatch to `key`.
    pub fn authorizes(&self, key: &ScheduleKey) -> bool {
        self.class == ExecutionClass::SafetyProtocol
            || self.lease.as_ref().is_some_and(|lease| &lease.key == key)
    }

    fn scheduled(inner: &Arc<SchedulerInner>, waiter: &Waiter, key: ScheduleKey) -> Self {
        Self {
            class: ExecutionClass::Scheduled,
            lease: Some(Arc::new(Lease {
                scheduler: Arc::downgrade(inner),
                key,
                command_id: waiter.command_id,
                authority_epoch: waiter.authority_epoch,
            })),
        }
    }
}

struct Lease {
    scheduler: Weak<SchedulerInner>,
    key: ScheduleKey,
    command_id: u64,
    authority_epoch: u64,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(inner) = self.scheduler.upgrade() {
            inner.release(&self.key, self.command_id);
        }
    }
}

/// The service-owned resource scheduler.
#[derive(Clone)]
pub struct CommandScheduler {
    inner: Arc<SchedulerInner>,
}

impl Default for CommandScheduler {
    fn default() -> Self {
        Self::new(SchedulerLimits::default())
    }
}

impl CommandScheduler {
    /// Construct an empty scheduler with explicit bounds.
    pub fn new(limits: SchedulerLimits) -> Self {
        assert!(
            limits.per_resource > 0,
            "per-resource limit must be positive"
        );
        assert!(
            limits.per_producer > 0,
            "per-producer limit must be positive"
        );
        assert!(limits.global > 0, "global limit must be positive");
        Self {
            inner: Arc::new(SchedulerInner {
                limits,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// Wait for dispatch admission on `key` under `authority_epoch`.
    pub async fn acquire(
        &self,
        key: ScheduleKey,
        producer: ProducerId,
        authority_epoch: u64,
    ) -> Result<ExecutionContext, ScheduleFailure> {
        let (sender, receiver) = oneshot::channel();
        let command_id;
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            if authority_epoch != state.authority_epoch {
                return Err(ScheduleFailure::AuthorityChanged);
            }
            command_id = state.next_command_id;
            state.next_command_id = state.next_command_id.wrapping_add(1).max(1);
            let waiter = Waiter {
                command_id,
                producer,
                authority_epoch,
                sender,
            };
            state.enqueue(key, waiter, self.inner.limits)?;
            deliveries = state.drive_all();
        }
        self.inner.deliver(deliveries);

        match tokio::time::timeout(self.inner.limits.queue_deadline, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ScheduleFailure::Retired {
                reason: RetirementReason::ResourceDestroyed,
            }),
            Err(_) => {
                self.inner.cancel(command_id);
                Err(ScheduleFailure::QueueDeadline)
            }
        }
    }

    /// Wait for dispatch admission while permitting exact cancellation of this queued command.
    ///
    /// Cancellation removes only this command id. Other work from the same workspace producer is
    /// unaffected. Once a context is returned, the caller owns an admitted atomic dispatch and
    /// must use cooperative checks at its next safe boundary instead of aborting the browser send.
    pub async fn acquire_cancellable(
        &self,
        key: ScheduleKey,
        producer: ProducerId,
        authority_epoch: u64,
        cancellation: &CancellationToken,
    ) -> Result<ExecutionContext, ScheduleFailure> {
        if cancellation.is_cancelled() {
            return Err(ScheduleFailure::Cancelled);
        }

        let (sender, receiver) = oneshot::channel();
        let command_id;
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            if authority_epoch != state.authority_epoch {
                return Err(ScheduleFailure::AuthorityChanged);
            }
            command_id = state.next_command_id;
            state.next_command_id = state.next_command_id.wrapping_add(1).max(1);
            let waiter = Waiter {
                command_id,
                producer,
                authority_epoch,
                sender,
            };
            state.enqueue(key, waiter, self.inner.limits)?;
            deliveries = state.drive_all();
        }
        self.inner.deliver(deliveries);

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                self.inner.cancel(command_id);
                Err(ScheduleFailure::Cancelled)
            }
            result = tokio::time::timeout(self.inner.limits.queue_deadline, receiver) => {
                match result {
                    Ok(Ok(result)) => result,
                    Ok(Err(_)) => Err(ScheduleFailure::Retired {
                        reason: RetirementReason::ResourceDestroyed,
                    }),
                    Err(_) => {
                        self.inner.cancel(command_id);
                        Err(ScheduleFailure::QueueDeadline)
                    }
                }
            }
        }
    }

    /// Publish a new authority epoch and retire every queued command from older authority.
    pub fn advance_authority_epoch(&self, authority_epoch: u64) {
        let failures;
        {
            let mut state = self.inner.lock_state();
            if authority_epoch <= state.authority_epoch {
                return;
            }
            state.authority_epoch = authority_epoch;
            failures = state.retire_all(ScheduleFailure::AuthorityChanged);
        }
        self.inner.deliver(failures);
    }

    /// Retire queued work belonging to one producer. Active work keeps its admitted snapshot.
    pub fn retire_producer(&self, producer: &ProducerId, reason: RetirementReason) {
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            deliveries = state.retire_producer(producer, ScheduleFailure::Retired { reason });
        }
        self.inner.deliver(deliveries);
    }

    /// Retire all queued work after a service-wide safety or lifecycle event.
    pub fn retire_all(&self, reason: RetirementReason) {
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            deliveries = state.retire_all(ScheduleFailure::Retired { reason });
        }
        self.inner.deliver(deliveries);
    }

    /// Retire queued work for one disconnected browser without clearing active or uncertain state.
    pub fn retire_browser(&self, browser_slot: u32) {
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            deliveries = state.retire_browser(
                browser_slot,
                ScheduleFailure::Retired {
                    reason: RetirementReason::BrowserDisconnected,
                },
            );
        }
        self.inner.deliver(deliveries);
    }

    /// Quarantine an exact resource delivery after dispatch completed with an unknown outcome.
    ///
    /// Marking succeeds only while `command_id` is still the resource's active lease. This makes a
    /// delayed timeout harmless after tab or browser-generation destruction and avoids recreating
    /// retired resource state. Negotiated executor generation is retained as part of the exact
    /// terminal proof; legacy deliveries without it remain quarantined until resource destruction.
    pub fn mark_uncertain(
        &self,
        key: &ScheduleKey,
        command_id: u64,
        request_id: &str,
        executor_generation: Option<&str>,
    ) -> bool {
        let (marked, deliveries);
        {
            let mut state = self.inner.lock_state();
            (marked, deliveries) =
                state.mark_uncertain(key, command_id, request_id, executor_generation);
        }
        self.inner.deliver(deliveries);
        marked
    }

    /// Clear quarantine only for the exact executor, command, request, and resource terminal proof.
    pub fn reconcile(
        &self,
        key: &ScheduleKey,
        command_id: u64,
        request_id: &str,
        executor_generation: &str,
    ) -> bool {
        let (cleared, deliveries) = {
            let mut state = self.inner.lock_state();
            state.reconcile(key, command_id, request_id, executor_generation)
        };
        self.inner.deliver(deliveries);
        cleared
    }

    /// Retire queued work and forget quarantine for a tab the browser confirmed was destroyed.
    pub fn destroy_surface(&self, surface: BrowserSurface) {
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            deliveries = state.destroy_surface(surface);
        }
        self.inner.deliver(deliveries);
    }

    /// Retire queued work and forget all resources for a disconnected browser generation.
    pub fn destroy_browser(&self, browser_slot: u32) {
        let deliveries;
        {
            let mut state = self.inner.lock_state();
            deliveries = state.destroy_browser(browser_slot);
        }
        self.inner.deliver(deliveries);
    }

    #[cfg(test)]
    fn queued_count(&self) -> usize {
        self.inner.lock_state().queued_global
    }
}

struct SchedulerInner {
    limits: SchedulerLimits,
    state: Mutex<State>,
}

impl SchedulerInner {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn deliver(self: &Arc<Self>, deliveries: Vec<Delivery>) {
        for delivery in deliveries {
            match delivery.result {
                DeliveryResult::Grant(key) => {
                    let context = ExecutionContext::scheduled(self, &delivery.waiter, key);
                    if let Err(returned) = delivery.waiter.sender.send(Ok(context)) {
                        drop(returned);
                    }
                }
                DeliveryResult::Fail(error) => {
                    let _ = delivery.waiter.sender.send(Err(error));
                }
            }
        }
    }

    fn release(self: &Arc<Self>, key: &ScheduleKey, command_id: u64) {
        let deliveries = {
            let mut state = self.lock_state();
            state.release(key, command_id);
            state.drive_all()
        };
        self.deliver(deliveries);
    }

    fn cancel(self: &Arc<Self>, command_id: u64) {
        let deliveries = {
            let mut state = self.lock_state();
            state.cancel(command_id);
            state.drive_all()
        };
        self.deliver(deliveries);
    }
}

#[derive(Default)]
struct State {
    authority_epoch: u64,
    next_command_id: u64,
    browsers: HashMap<u32, BrowserQueues>,
    queued_global: usize,
    queued_by_producer: HashMap<ProducerId, usize>,
}

impl State {
    fn enqueue(
        &mut self,
        key: ScheduleKey,
        waiter: Waiter,
        limits: SchedulerLimits,
    ) -> Result<(), ScheduleFailure> {
        if self.queued_global >= limits.global {
            return Err(ScheduleFailure::Overloaded { scope: "global" });
        }
        if self
            .queued_by_producer
            .get(&waiter.producer)
            .copied()
            .unwrap_or_default()
            >= limits.per_producer
        {
            return Err(ScheduleFailure::Overloaded { scope: "producer" });
        }

        let slot = key.browser_slot();
        let browser = self.browsers.entry(slot).or_default();
        if let Some(uncertain) = browser.conflicting_uncertainty(&key) {
            return Err(ScheduleFailure::ResourceUncertain {
                command_id: uncertain.command_id,
            });
        }
        let queue = browser.queue_mut(&key);
        if queue.len() >= limits.per_resource {
            return Err(ScheduleFailure::Overloaded { scope: "resource" });
        }
        *self
            .queued_by_producer
            .entry(waiter.producer.clone())
            .or_default() += 1;
        self.queued_global += 1;
        queue.push(waiter);
        Ok(())
    }

    fn drive_all(&mut self) -> Vec<Delivery> {
        let mut grants = Vec::new();
        for (slot, browser) in &mut self.browsers {
            grants.extend(BrowserQueues::with_slot(browser.drive(), *slot));
        }
        for delivery in &grants {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        grants
    }

    fn prune_idle(&mut self) {
        for browser in self.browsers.values_mut() {
            browser.prune_idle();
        }
        self.browsers.retain(|_, browser| !browser.is_idle());
    }

    fn account_removed(&mut self, producer: &ProducerId) {
        self.queued_global = self.queued_global.saturating_sub(1);
        if let Some(count) = self.queued_by_producer.get_mut(producer) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.queued_by_producer.remove(producer);
            }
        }
    }

    fn release(&mut self, key: &ScheduleKey, command_id: u64) {
        if let Some(browser) = self.browsers.get_mut(&key.browser_slot()) {
            browser.release(key, command_id);
        }
    }

    fn cancel(&mut self, command_id: u64) {
        let mut removed = None;
        for browser in self.browsers.values_mut() {
            if let Some(waiter) = browser.remove(command_id) {
                removed = Some(waiter);
                break;
            }
        }
        if let Some(waiter) = removed {
            self.account_removed(&waiter.producer);
        }
    }

    fn retire_all(&mut self, error: ScheduleFailure) -> Vec<Delivery> {
        let mut deliveries = Vec::new();
        for browser in self.browsers.values_mut() {
            deliveries.extend(browser.drain_all(error.clone()));
        }
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        deliveries
    }

    fn retire_producer(&mut self, producer: &ProducerId, error: ScheduleFailure) -> Vec<Delivery> {
        let mut deliveries = Vec::new();
        for browser in self.browsers.values_mut() {
            deliveries.extend(browser.drain_producer(producer, error.clone()));
        }
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        deliveries
    }

    fn retire_browser(&mut self, browser_slot: u32, error: ScheduleFailure) -> Vec<Delivery> {
        let Some(browser) = self.browsers.get_mut(&browser_slot) else {
            return Vec::new();
        };
        let deliveries = browser.drain_all(error);
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        deliveries
    }

    fn mark_uncertain(
        &mut self,
        key: &ScheduleKey,
        command_id: u64,
        request_id: &str,
        executor_generation: Option<&str>,
    ) -> (bool, Vec<Delivery>) {
        let Some(browser) = self.browsers.get_mut(&key.browser_slot()) else {
            return (false, Vec::new());
        };
        let Some(queue) = browser.queue_mut_existing(key) else {
            return (false, Vec::new());
        };
        if queue.active != Some(command_id) {
            return (false, Vec::new());
        }
        queue.uncertain = Some(UncertainDelivery {
            command_id,
            request_id: request_id.to_string(),
            executor_generation: executor_generation.map(str::to_string),
        });
        let waiters = queue.drain();
        let deliveries: Vec<_> = waiters
            .into_iter()
            .map(|waiter| Delivery::fail(waiter, ScheduleFailure::ResourceUncertain { command_id }))
            .collect();
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        (true, deliveries)
    }

    fn reconcile(
        &mut self,
        key: &ScheduleKey,
        command_id: u64,
        request_id: &str,
        executor_generation: &str,
    ) -> (bool, Vec<Delivery>) {
        let Some(browser) = self.browsers.get_mut(&key.browser_slot()) else {
            return (false, Vec::new());
        };
        let Some(queue) = browser.queue_mut_existing(key) else {
            return (false, Vec::new());
        };
        let Some(uncertain) = queue.uncertain.as_ref() else {
            return (false, Vec::new());
        };
        if uncertain.command_id != command_id
            || uncertain.request_id != request_id
            || uncertain.executor_generation.as_deref() != Some(executor_generation)
        {
            return (false, Vec::new());
        }
        queue.uncertain = None;
        let deliveries = BrowserQueues::with_slot(browser.drive(), key.browser_slot());
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        (true, deliveries)
    }

    fn destroy_surface(&mut self, surface: BrowserSurface) -> Vec<Delivery> {
        let Some(browser) = self.browsers.get_mut(&surface.browser_slot) else {
            return Vec::new();
        };
        let Some(mut queue) = browser
            .children
            .remove(&ChildKey::Surface(surface.native_tab))
        else {
            return Vec::new();
        };
        if queue.active.take().is_some() {
            browser.active_children = browser.active_children.saturating_sub(1);
        }
        let deliveries: Vec<_> = queue
            .drain()
            .into_iter()
            .map(|waiter| {
                Delivery::fail(
                    waiter,
                    ScheduleFailure::Retired {
                        reason: RetirementReason::ResourceDestroyed,
                    },
                )
            })
            .collect();
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        deliveries
    }

    fn destroy_browser(&mut self, browser_slot: u32) -> Vec<Delivery> {
        let Some(mut browser) = self.browsers.remove(&browser_slot) else {
            return Vec::new();
        };
        let deliveries = browser.drain_all(ScheduleFailure::Retired {
            reason: RetirementReason::ResourceDestroyed,
        });
        for delivery in &deliveries {
            self.account_removed(&delivery.waiter.producer);
        }
        self.prune_idle();
        deliveries
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ChildKey {
    Surface(i64),
    WorkspaceTopology(String),
}

#[derive(Default)]
struct BrowserQueues {
    browser: FairQueue,
    children: HashMap<ChildKey, FairQueue>,
    active_children: usize,
    prefer_children: bool,
}

impl BrowserQueues {
    fn queue_mut(&mut self, key: &ScheduleKey) -> &mut FairQueue {
        match key {
            ScheduleKey::Surface(surface) => self
                .children
                .entry(ChildKey::Surface(surface.native_tab))
                .or_default(),
            ScheduleKey::WorkspaceTopology { workspace_key, .. } => self
                .children
                .entry(ChildKey::WorkspaceTopology(workspace_key.clone()))
                .or_default(),
            ScheduleKey::Browser { .. } => &mut self.browser,
        }
    }

    fn prune_idle(&mut self) {
        self.children.retain(|_, queue| !queue.is_idle());
    }

    fn is_idle(&self) -> bool {
        self.browser.is_idle() && self.children.is_empty() && self.active_children == 0
    }

    fn queue_mut_existing(&mut self, key: &ScheduleKey) -> Option<&mut FairQueue> {
        match key {
            ScheduleKey::Surface(surface) => self
                .children
                .get_mut(&ChildKey::Surface(surface.native_tab)),
            ScheduleKey::WorkspaceTopology { workspace_key, .. } => self
                .children
                .get_mut(&ChildKey::WorkspaceTopology(workspace_key.clone())),
            ScheduleKey::Browser { .. } => Some(&mut self.browser),
        }
    }

    fn conflicting_uncertainty(&self, key: &ScheduleKey) -> Option<&UncertainDelivery> {
        match key {
            ScheduleKey::Browser { .. } => self.browser.uncertain.as_ref().or_else(|| {
                self.children
                    .values()
                    .find_map(|queue| queue.uncertain.as_ref())
            }),
            ScheduleKey::Surface(surface) => self.browser.uncertain.as_ref().or_else(|| {
                self.children
                    .get(&ChildKey::Surface(surface.native_tab))
                    .and_then(|queue| queue.uncertain.as_ref())
            }),
            ScheduleKey::WorkspaceTopology { workspace_key, .. } => {
                self.browser.uncertain.as_ref().or_else(|| {
                    self.children
                        .get(&ChildKey::WorkspaceTopology(workspace_key.clone()))
                        .and_then(|queue| queue.uncertain.as_ref())
                })
            }
        }
    }

    fn drive(&mut self) -> Vec<Delivery> {
        if self.browser.active.is_some() || self.browser.uncertain.is_some() {
            return Vec::new();
        }
        let browser_waiting = !self.browser.is_empty();
        let uncertain_child = self
            .children
            .values()
            .any(|queue| queue.uncertain.is_some());
        if (self.active_children > 0 || uncertain_child) && browser_waiting {
            return Vec::new();
        }

        let child_waiting = self
            .children
            .values()
            .any(|queue| queue.active.is_none() && !queue.is_empty() && queue.uncertain.is_none());
        if self.active_children == 0 && browser_waiting && (!self.prefer_children || !child_waiting)
        {
            let waiter = self.browser.pop().expect("browser queue was non-empty");
            self.browser.active = Some(waiter.command_id);
            self.prefer_children = true;
            return vec![Delivery::grant(
                waiter,
                ScheduleKey::Browser { browser_slot: 0 },
            )];
        }

        let mut deliveries = Vec::new();
        for (child_key, queue) in &mut self.children {
            if queue.active.is_some() || queue.is_empty() || queue.uncertain.is_some() {
                continue;
            }
            let waiter = queue.pop().expect("child queue was non-empty");
            queue.active = Some(waiter.command_id);
            self.active_children += 1;
            let key = match child_key {
                ChildKey::Surface(native_tab) => ScheduleKey::Surface(BrowserSurface {
                    browser_slot: 0,
                    native_tab: *native_tab,
                }),
                ChildKey::WorkspaceTopology(workspace_key) => ScheduleKey::WorkspaceTopology {
                    browser_slot: 0,
                    workspace_key: workspace_key.clone(),
                },
            };
            deliveries.push(Delivery::grant(waiter, key));
        }
        if !deliveries.is_empty() {
            self.prefer_children = false;
        }
        deliveries
    }

    fn with_slot(mut deliveries: Vec<Delivery>, browser_slot: u32) -> Vec<Delivery> {
        for delivery in &mut deliveries {
            if let DeliveryResult::Grant(key) = &mut delivery.result {
                match key {
                    ScheduleKey::Surface(surface) => surface.browser_slot = browser_slot,
                    ScheduleKey::WorkspaceTopology {
                        browser_slot: slot, ..
                    }
                    | ScheduleKey::Browser { browser_slot: slot } => *slot = browser_slot,
                }
            }
        }
        deliveries
    }

    fn release(&mut self, key: &ScheduleKey, command_id: u64) {
        let queue = self.queue_mut(key);
        if queue.active != Some(command_id) {
            return;
        }
        queue.active = None;
        if !matches!(key, ScheduleKey::Browser { .. }) {
            self.active_children = self.active_children.saturating_sub(1);
        }
    }

    fn remove(&mut self, command_id: u64) -> Option<Waiter> {
        self.browser.remove(command_id).or_else(|| {
            self.children
                .values_mut()
                .find_map(|queue| queue.remove(command_id))
        })
    }

    fn drain_all(&mut self, error: ScheduleFailure) -> Vec<Delivery> {
        let mut waiters = self.browser.drain();
        for queue in self.children.values_mut() {
            waiters.extend(queue.drain());
        }
        waiters
            .into_iter()
            .map(|waiter| Delivery::fail(waiter, error.clone()))
            .collect()
    }

    fn drain_producer(&mut self, producer: &ProducerId, error: ScheduleFailure) -> Vec<Delivery> {
        let mut waiters = self.browser.drain_producer(producer);
        for queue in self.children.values_mut() {
            waiters.extend(queue.drain_producer(producer));
        }
        waiters
            .into_iter()
            .map(|waiter| Delivery::fail(waiter, error.clone()))
            .collect()
    }
}

#[derive(Default)]
struct FairQueue {
    by_producer: HashMap<ProducerId, VecDeque<Waiter>>,
    producer_order: VecDeque<ProducerId>,
    len: usize,
    active: Option<u64>,
    uncertain: Option<UncertainDelivery>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UncertainDelivery {
    command_id: u64,
    request_id: String,
    executor_generation: Option<String>,
}

impl FairQueue {
    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn is_idle(&self) -> bool {
        self.is_empty() && self.active.is_none() && self.uncertain.is_none()
    }

    fn push(&mut self, waiter: Waiter) {
        let producer = waiter.producer.clone();
        let queue = self.by_producer.entry(producer.clone()).or_default();
        if queue.is_empty() {
            self.producer_order.push_back(producer);
        }
        queue.push_back(waiter);
        self.len += 1;
    }

    fn pop(&mut self) -> Option<Waiter> {
        while let Some(producer) = self.producer_order.pop_front() {
            let Some(queue) = self.by_producer.get_mut(&producer) else {
                continue;
            };
            let waiter = queue.pop_front();
            if queue.is_empty() {
                self.by_producer.remove(&producer);
            } else {
                self.producer_order.push_back(producer);
            }
            if waiter.is_some() {
                self.len = self.len.saturating_sub(1);
                return waiter;
            }
        }
        None
    }

    fn remove(&mut self, command_id: u64) -> Option<Waiter> {
        let producer = self.by_producer.iter().find_map(|(producer, queue)| {
            queue
                .iter()
                .any(|waiter| waiter.command_id == command_id)
                .then(|| producer.clone())
        })?;
        let queue = self.by_producer.get_mut(&producer)?;
        let position = queue
            .iter()
            .position(|waiter| waiter.command_id == command_id)?;
        let waiter = queue.remove(position);
        if queue.is_empty() {
            self.by_producer.remove(&producer);
            self.producer_order.retain(|queued| queued != &producer);
        }
        if waiter.is_some() {
            self.len = self.len.saturating_sub(1);
        }
        waiter
    }

    fn drain(&mut self) -> Vec<Waiter> {
        let mut waiters = Vec::with_capacity(self.len);
        while let Some(waiter) = self.pop() {
            waiters.push(waiter);
        }
        waiters
    }

    fn drain_producer(&mut self, producer: &ProducerId) -> Vec<Waiter> {
        let waiters: Vec<_> = self
            .by_producer
            .remove(producer)
            .map(|queue| queue.into_iter().collect())
            .unwrap_or_default();
        self.producer_order.retain(|queued| queued != producer);
        self.len = self.len.saturating_sub(waiters.len());
        waiters
    }
}

struct Waiter {
    command_id: u64,
    producer: ProducerId,
    authority_epoch: u64,
    sender: oneshot::Sender<Result<ExecutionContext, ScheduleFailure>>,
}

struct Delivery {
    waiter: Waiter,
    result: DeliveryResult,
}

impl Delivery {
    fn grant(waiter: Waiter, key: ScheduleKey) -> Self {
        Self {
            waiter,
            result: DeliveryResult::Grant(key),
        }
    }

    fn fail(waiter: Waiter, error: ScheduleFailure) -> Self {
        Self {
            waiter,
            result: DeliveryResult::Fail(error),
        }
    }
}

enum DeliveryResult {
    Grant(ScheduleKey),
    Fail(ScheduleFailure),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn scheduler() -> CommandScheduler {
        CommandScheduler::new(SchedulerLimits {
            per_resource: 8,
            per_producer: 16,
            global: 32,
            queue_deadline: Duration::from_millis(200),
        })
    }

    fn surface(tab: i64) -> ScheduleKey {
        ScheduleKey::Surface(BrowserSurface {
            browser_slot: 7,
            native_tab: tab,
        })
    }

    fn topology(workspace: &str) -> ScheduleKey {
        ScheduleKey::WorkspaceTopology {
            browser_slot: 7,
            workspace_key: workspace.to_string(),
        }
    }

    #[tokio::test]
    async fn serializes_one_surface_in_fifo_order() {
        let scheduler = scheduler();
        let first = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let queued = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(10), ProducerId::new("a"), 0)
                    .await
                    .unwrap()
            }
        });
        tokio::task::yield_now().await;
        assert!(!queued.is_finished());
        drop(first);
        let second = queued.await.unwrap();
        assert_eq!(second.key(), Some(&surface(10)));
    }

    #[tokio::test]
    async fn cancellation_retires_only_the_exact_queued_command() {
        let scheduler = scheduler();
        let held = scheduler
            .acquire(surface(10), ProducerId::new("held"), 0)
            .await
            .unwrap();
        let cancelled = CancellationToken::new();
        let queued_cancelled = tokio::spawn({
            let scheduler = scheduler.clone();
            let cancelled = cancelled.clone();
            async move {
                scheduler
                    .acquire_cancellable(
                        surface(10),
                        ProducerId::new("same-workspace"),
                        0,
                        &cancelled,
                    )
                    .await
            }
        });
        let queued_survivor = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(10), ProducerId::new("same-workspace"), 0)
                    .await
            }
        });
        tokio::task::yield_now().await;
        cancelled.cancel();
        assert_eq!(
            queued_cancelled.await.unwrap().unwrap_err(),
            ScheduleFailure::Cancelled
        );
        assert!(!queued_survivor.is_finished());
        drop(held);
        assert!(queued_survivor.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn different_surfaces_run_concurrently() {
        let scheduler = scheduler();
        let first = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let second = scheduler
            .acquire(surface(11), ProducerId::new("a"), 0)
            .await
            .unwrap();
        assert_ne!(first.command_id(), second.command_id());
    }

    #[tokio::test]
    async fn browser_wide_work_excludes_children() {
        let scheduler = scheduler();
        let child = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let browser = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(
                        ScheduleKey::Browser { browser_slot: 7 },
                        ProducerId::new("b"),
                        0,
                    )
                    .await
                    .unwrap()
            }
        });
        tokio::task::yield_now().await;
        let later_child = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(11), ProducerId::new("c"), 0)
                    .await
                    .unwrap()
            }
        });
        drop(child);
        let browser_context = browser.await.unwrap();
        assert!(!later_child.is_finished());
        drop(browser_context);
        later_child.await.unwrap();
    }

    #[tokio::test]
    async fn round_robins_producers_without_reordering_each_producer() {
        let scheduler = scheduler();
        let held = scheduler
            .acquire(surface(10), ProducerId::new("held"), 0)
            .await
            .unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for (producer, label) in [("a", "a1"), ("a", "a2"), ("b", "b1"), ("b", "b2")] {
            let scheduler = scheduler.clone();
            let order = Arc::clone(&order);
            let running = Arc::clone(&running);
            tasks.push(tokio::spawn(async move {
                let context = scheduler
                    .acquire(surface(10), ProducerId::new(producer), 0)
                    .await
                    .unwrap();
                assert_eq!(running.fetch_add(1, Ordering::SeqCst), 0);
                order.lock().unwrap().push(label);
                running.fetch_sub(1, Ordering::SeqCst);
                drop(context);
            }));
        }
        tokio::task::yield_now().await;
        drop(held);
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(*order.lock().unwrap(), ["a1", "b1", "a2", "b2"]);
    }

    #[tokio::test]
    async fn rejects_overload_without_dispatch() {
        let scheduler = CommandScheduler::new(SchedulerLimits {
            per_resource: 1,
            per_producer: 8,
            global: 8,
            queue_deadline: Duration::from_millis(200),
        });
        let held = scheduler
            .acquire(surface(10), ProducerId::new("held"), 0)
            .await
            .unwrap();
        let queued = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(10), ProducerId::new("a"), 0)
                    .await
            }
        });
        tokio::task::yield_now().await;
        let error = scheduler
            .acquire(surface(10), ProducerId::new("b"), 0)
            .await
            .unwrap_err();
        assert_eq!(error, ScheduleFailure::Overloaded { scope: "resource" });
        drop(held);
        queued.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn authority_change_retires_queued_but_not_active_work() {
        let scheduler = scheduler();
        let active = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let queued = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(10), ProducerId::new("b"), 0)
                    .await
            }
        });
        tokio::task::yield_now().await;
        scheduler.advance_authority_epoch(1);
        assert_eq!(
            queued.await.unwrap().unwrap_err(),
            ScheduleFailure::AuthorityChanged
        );
        assert_eq!(active.authority_epoch(), Some(0));
        drop(active);
        scheduler
            .acquire(surface(10), ProducerId::new("c"), 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn quarantine_clears_only_for_exact_terminal_delivery() {
        let scheduler = scheduler();
        let key = surface(10);
        let active = scheduler
            .acquire(key.clone(), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        assert!(scheduler.mark_uncertain(&key, command_id, "request-a", Some("executor-a")));
        drop(active);
        let error = scheduler
            .acquire(key.clone(), ProducerId::new("b"), 0)
            .await
            .unwrap_err();
        assert_eq!(error, ScheduleFailure::ResourceUncertain { command_id });
        assert!(!scheduler.reconcile(&key, command_id + 1, "request-a", "executor-a"));
        assert!(!scheduler.reconcile(&key, command_id, "request-b", "executor-a"));
        assert!(!scheduler.reconcile(&surface(11), command_id, "request-a", "executor-a"));
        assert!(!scheduler.reconcile(&key, command_id, "request-a", "executor-b"));
        assert!(scheduler.reconcile(&key, command_id, "request-a", "executor-a"));
        scheduler
            .acquire(key, ProducerId::new("b"), 0)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn legacy_quarantine_without_executor_proof_never_terminal_reconciles() {
        let scheduler = scheduler();
        let key = surface(10);
        let active = scheduler
            .acquire(key.clone(), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        assert!(scheduler.mark_uncertain(&key, command_id, "legacy-request", None));
        drop(active);

        assert!(!scheduler.reconcile(&key, command_id, "legacy-request", "claimed-generation"));
        assert_eq!(
            scheduler
                .acquire(key, ProducerId::new("b"), 0)
                .await
                .unwrap_err(),
            ScheduleFailure::ResourceUncertain { command_id }
        );
    }

    #[tokio::test]
    async fn generic_quarantine_is_local_but_blocks_conflicting_browser_work() {
        let scheduler = scheduler();
        let key = topology("workspace-a");
        let active = scheduler
            .acquire(key.clone(), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        assert!(scheduler.mark_uncertain(&key, command_id, "topology-request", Some("executor-a")));
        drop(active);

        assert_eq!(
            scheduler
                .acquire(key.clone(), ProducerId::new("b"), 0)
                .await
                .unwrap_err(),
            ScheduleFailure::ResourceUncertain { command_id }
        );
        let other = scheduler
            .acquire(topology("workspace-b"), ProducerId::new("c"), 0)
            .await
            .unwrap();
        assert_eq!(other.key(), Some(&topology("workspace-b")));
        assert_eq!(
            scheduler
                .acquire(
                    ScheduleKey::Browser { browser_slot: 7 },
                    ProducerId::new("d"),
                    0,
                )
                .await
                .unwrap_err(),
            ScheduleFailure::ResourceUncertain { command_id }
        );
    }

    #[tokio::test]
    async fn uncertain_browser_blocks_every_child_resource() {
        let scheduler = scheduler();
        let browser_key = ScheduleKey::Browser { browser_slot: 7 };
        let active = scheduler
            .acquire(browser_key.clone(), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        assert!(scheduler.mark_uncertain(
            &browser_key,
            command_id,
            "browser-request",
            Some("executor-a")
        ));
        drop(active);

        for key in [surface(10), topology("workspace-a")] {
            assert_eq!(
                scheduler
                    .acquire(key, ProducerId::new("b"), 0)
                    .await
                    .unwrap_err(),
                ScheduleFailure::ResourceUncertain { command_id }
            );
        }
    }

    #[tokio::test]
    async fn uncertainty_blocks_already_queued_hierarchical_rivals_until_reconciled() {
        let scheduler = scheduler();
        let child_key = surface(10);
        let child = scheduler
            .acquire(child_key.clone(), ProducerId::new("child"), 0)
            .await
            .unwrap();
        let child_command = child.command_id().unwrap();
        let browser_waiter = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(
                        ScheduleKey::Browser { browser_slot: 7 },
                        ProducerId::new("browser"),
                        0,
                    )
                    .await
            }
        });
        tokio::task::yield_now().await;
        assert!(scheduler.mark_uncertain(
            &child_key,
            child_command,
            "child-request",
            Some("executor-a")
        ));
        drop(child);
        tokio::task::yield_now().await;
        assert!(!browser_waiter.is_finished());
        assert!(scheduler.reconcile(&child_key, child_command, "child-request", "executor-a"));
        let browser = browser_waiter.await.unwrap().unwrap();

        let child_waiter = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(11), ProducerId::new("later-child"), 0)
                    .await
            }
        });
        tokio::task::yield_now().await;
        let browser_key = ScheduleKey::Browser { browser_slot: 7 };
        let browser_command = browser.command_id().unwrap();
        assert!(scheduler.mark_uncertain(
            &browser_key,
            browser_command,
            "browser-request",
            Some("executor-a")
        ));
        drop(browser);
        tokio::task::yield_now().await;
        assert!(!child_waiter.is_finished());
        assert!(scheduler.reconcile(
            &browser_key,
            browser_command,
            "browser-request",
            "executor-a"
        ));
        {
            let state = scheduler.inner.lock_state();
            let queues = state.browsers.get(&7).expect("browser queues remain");
            let child = queues
                .children
                .get(&ChildKey::Surface(11))
                .expect("queued child remains");
            assert!(
                child.active.is_some(),
                "reconciliation must grant the already-queued child (browser_active={:?}, browser_uncertain={}, active_children={}, child_len={}, child_uncertain={})",
                queues.browser.active,
                queues.browser.uncertain.is_some(),
                queues.active_children,
                child.len,
                child.uncertain.is_some()
            );
        }
        child_waiter.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn stale_timeout_cannot_recreate_a_destroyed_resource() {
        let scheduler = scheduler();
        let key = surface(10);
        let active = scheduler
            .acquire(key.clone(), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        scheduler.destroy_browser(7);
        assert!(!scheduler.mark_uncertain(&key, command_id, "old-request", Some("executor-a")));
        drop(active);
        scheduler
            .acquire(key, ProducerId::new("b"), 0)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn idle_resource_queues_are_pruned_after_release() {
        let scheduler = scheduler();
        let active = scheduler
            .acquire(topology("short-lived-workspace"), ProducerId::new("a"), 0)
            .await
            .unwrap();
        assert_eq!(scheduler.inner.lock_state().browsers.len(), 1);
        drop(active);
        assert!(scheduler.inner.lock_state().browsers.is_empty());
    }

    #[tokio::test]
    async fn queue_deadline_removes_waiter() {
        let scheduler = CommandScheduler::new(SchedulerLimits {
            per_resource: 8,
            per_producer: 8,
            global: 8,
            queue_deadline: Duration::from_millis(10),
        });
        let active = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let error = scheduler
            .acquire(surface(10), ProducerId::new("b"), 0)
            .await
            .unwrap_err();
        assert_eq!(error, ScheduleFailure::QueueDeadline);
        assert_eq!(scheduler.queued_count(), 0);
        drop(active);
    }

    #[tokio::test]
    async fn port_disconnect_retires_waiters_without_proving_active_work_ended() {
        let scheduler = scheduler();
        let active = scheduler
            .acquire(surface(10), ProducerId::new("a"), 0)
            .await
            .unwrap();
        let command_id = active.command_id().unwrap();
        let queued = tokio::spawn({
            let scheduler = scheduler.clone();
            async move {
                scheduler
                    .acquire(surface(10), ProducerId::new("b"), 0)
                    .await
            }
        });
        tokio::task::yield_now().await;

        scheduler.retire_browser(7);
        assert_eq!(
            queued.await.unwrap().unwrap_err(),
            ScheduleFailure::Retired {
                reason: RetirementReason::BrowserDisconnected
            }
        );
        assert!(scheduler.mark_uncertain(
            &surface(10),
            command_id,
            "disconnected-request",
            Some("executor-a")
        ));
        drop(active);
        assert_eq!(
            scheduler
                .acquire(surface(10), ProducerId::new("c"), 0)
                .await
                .unwrap_err(),
            ScheduleFailure::ResourceUncertain { command_id }
        );

        scheduler.destroy_browser(7);
        scheduler
            .acquire(surface(10), ProducerId::new("d"), 0)
            .await
            .unwrap();
    }
}
