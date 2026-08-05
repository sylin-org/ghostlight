// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Service-owned browser workspace lifecycle and ownership.
//!
//! A workspace is application continuity, not connection identity. The service mints every
//! handle, validates it only after owner-only local admission, pins it while work is active, and
//! expires detached state after a bounded grace period. The registry also owns tab membership so
//! a tab handle can never silently cross workspaces.

use crate::hub::peer::PeerUser;
use crate::hub::{try_mint, MintGuard, MintQuota};
use ghostlight_transport::workspace_id::WorkspaceId;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Detached workspaces remain reusable for this long unless active work pins them.
pub const WORKSPACE_IDLE_GRACE: Duration = Duration::from_secs(120);

/// A leak-free workspace lookup failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WorkspaceError {
    /// The handle is absent, expired, or belongs to another admitted OS user.
    #[error("unknown workspace")]
    Unknown,
    /// The peer has reached its bounded live-workspace quota.
    #[error("workspace limit reached for this client")]
    Quota,
}

/// Result of claiming a browser tab for one workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabClaim {
    /// The workspace already owns the tab.
    Owned,
    /// The tab was unowned and is now bound to the workspace.
    Adopted,
    /// Another live workspace owns the tab.
    Refused,
}

struct WorkspaceEntry {
    owner: PeerUser,
    attached: usize,
    active: usize,
    idle_deadline: Option<Instant>,
    retire_when_idle: bool,
    tabs: HashSet<i64>,
    _quota: MintGuard,
}

struct RegistryState {
    entries: HashMap<WorkspaceId, WorkspaceEntry>,
    tab_owners: HashMap<i64, WorkspaceId>,
    retired: Vec<RetiredWorkspace>,
}

impl RegistryState {
    fn retire(&mut self, workspace: &WorkspaceId) -> bool {
        let Some(entry) = self.entries.remove(workspace) else {
            return false;
        };
        let mut tabs: Vec<i64> = entry.tabs.into_iter().collect();
        tabs.sort_unstable();
        for tab_id in &tabs {
            if self.tab_owners.get(tab_id) == Some(workspace) {
                self.tab_owners.remove(tab_id);
            }
        }
        self.retired.push(RetiredWorkspace {
            workspace: workspace.clone(),
            tabs,
        });
        true
    }
}

/// The one service-lifetime registry for workspace liveness and owned browser handles.
#[derive(Clone)]
pub struct WorkspaceRegistry {
    state: Arc<Mutex<RegistryState>>,
    quota: MintQuota,
    idle_grace: Duration,
}

/// Workspace state returned exactly once for service-side cleanup after retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredWorkspace {
    /// Opaque retired workspace handle.
    pub workspace: WorkspaceId,
    /// Composite tab ids formerly owned by the workspace.
    pub tabs: Vec<i64>,
}

/// Console-safe workspace summary. Bearer handles and OS-user principals are omitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    /// Number of connection-bound shores currently attached.
    pub attached: usize,
    /// Number of active work items pinning the workspace.
    pub active: usize,
    /// Full current composite tab-id membership.
    pub owned_tab_ids: Vec<i64>,
}

impl WorkspaceRegistry {
    /// Construct an empty registry using the production idle grace.
    pub fn new(quota: MintQuota) -> Self {
        Self::with_idle_grace(quota, WORKSPACE_IDLE_GRACE)
    }

    /// Construct an empty registry with an explicit grace, primarily for deterministic tests.
    pub fn with_idle_grace(quota: MintQuota, idle_grace: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(RegistryState {
                entries: HashMap::new(),
                tab_owners: HashMap::new(),
                retired: Vec::new(),
            })),
            quota,
            idle_grace,
        }
    }

    /// Mint a new service-owned workspace. `attached` is true only for a connection-bound shore.
    pub fn mint(&self, owner: &PeerUser, attached: bool) -> Result<WorkspaceId, WorkspaceError> {
        let quota = try_mint(&self.quota, owner).map_err(|_| WorkspaceError::Quota)?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let workspace = loop {
            let candidate = WorkspaceId::mint();
            if !state.entries.contains_key(&candidate) {
                break candidate;
            }
        };
        state.entries.insert(
            workspace.clone(),
            WorkspaceEntry {
                owner: owner.clone(),
                attached: usize::from(attached),
                active: 0,
                idle_deadline: (!attached).then(|| Instant::now() + self.idle_grace),
                retire_when_idle: false,
                tabs: HashSet::new(),
                _quota: quota,
            },
        );
        Ok(workspace)
    }

    /// Reattach a still-live workspace for a connection-bound protocol shore.
    pub fn attach(&self, workspace: &WorkspaceId, owner: &PeerUser) -> Result<(), WorkspaceError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = state
            .entries
            .get_mut(workspace)
            .filter(|entry| &entry.owner == owner && !entry.retire_when_idle)
            .ok_or(WorkspaceError::Unknown)?;
        entry.attached = entry.attached.saturating_add(1);
        entry.idle_deadline = None;
        Ok(())
    }

    /// Detach an uncleanly lost connection and start idle grace when no work remains.
    pub fn detach(&self, workspace: &WorkspaceId, owner: &PeerUser) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(entry) = state
            .entries
            .get_mut(workspace)
            .filter(|entry| &entry.owner == owner)
        else {
            return;
        };
        entry.attached = entry.attached.saturating_sub(1);
        if entry.attached == 0 && entry.active == 0 {
            entry.idle_deadline = Some(Instant::now() + self.idle_grace);
        }
    }

    /// Cleanly release a connection-bound workspace. Active work pins cleanup until it settles.
    pub fn release(&self, workspace: &WorkspaceId, owner: &PeerUser) -> Result<(), WorkspaceError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = state
            .entries
            .get_mut(workspace)
            .filter(|entry| &entry.owner == owner)
            .ok_or(WorkspaceError::Unknown)?;
        entry.attached = entry.attached.saturating_sub(1);
        entry.retire_when_idle = true;
        entry.idle_deadline = Some(Instant::now());
        if entry.active == 0 {
            state.retire(workspace);
        }
        Ok(())
    }

    /// Pin a workspace while one work item is active.
    pub fn lease(
        &self,
        workspace: &WorkspaceId,
        owner: &PeerUser,
    ) -> Result<WorkspaceLease, WorkspaceError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = state
            .entries
            .get_mut(workspace)
            .filter(|entry| &entry.owner == owner && !entry.retire_when_idle)
            .ok_or(WorkspaceError::Unknown)?;
        entry.active = entry.active.saturating_add(1);
        entry.idle_deadline = None;
        Ok(WorkspaceLease {
            registry: self.clone(),
            workspace: workspace.clone(),
        })
    }

    /// Claim or verify one tab as a member of `workspace`.
    ///
    /// This is trusted-result ingestion. Request admission must use [`Self::owns_tab`] and must
    /// never adopt an arbitrary caller-provided handle.
    pub fn claim_tab(&self, workspace: &WorkspaceId, tab_id: i64) -> TabClaim {
        self.claim_tabs(workspace, &[tab_id])
    }

    /// Return whether `tab_id` is already owned by this exact live workspace.
    ///
    /// This read-only check is the request-shore authority boundary. Unknown and cross-workspace
    /// handles deliberately have the same result and do not mutate membership.
    pub fn owns_tab(&self, workspace: &WorkspaceId, tab_id: i64) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tab_owners
            .get(&tab_id)
            == Some(workspace)
    }

    /// Atomically claim or verify every tab in one browser response.
    ///
    /// If any tab belongs to another workspace, none of the unowned tabs are adopted. This keeps
    /// a malformed or stale context-creation response from partially changing service state.
    pub fn claim_tabs(&self, workspace: &WorkspaceId, tab_ids: &[i64]) -> TabClaim {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.entries.contains_key(workspace) {
            return TabClaim::Refused;
        }
        if tab_ids.iter().any(|tab_id| {
            state
                .tab_owners
                .get(tab_id)
                .is_some_and(|owner| owner != workspace)
        }) {
            return TabClaim::Refused;
        }

        let mut adopted = false;
        for &tab_id in tab_ids {
            if state.tab_owners.contains_key(&tab_id) {
                continue;
            }
            state.tab_owners.insert(tab_id, workspace.clone());
            state
                .entries
                .get_mut(workspace)
                .expect("workspace existence checked above")
                .tabs
                .insert(tab_id);
            adopted = true;
        }
        if adopted {
            TabClaim::Adopted
        } else {
            TabClaim::Owned
        }
    }

    /// Return the full sorted set of tabs owned by one workspace.
    pub fn owned_tabs(&self, workspace: &WorkspaceId) -> Vec<i64> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut tabs: Vec<i64> = state
            .entries
            .get(workspace)
            .map(|entry| entry.tabs.iter().copied().collect())
            .unwrap_or_default();
        tabs.sort_unstable();
        tabs
    }

    /// Release one tab only if it is currently owned by `workspace`.
    pub fn release_tab(&self, workspace: &WorkspaceId, tab_id: i64) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.tab_owners.get(&tab_id) != Some(workspace) {
            return false;
        }
        state.tab_owners.remove(&tab_id);
        if let Some(entry) = state.entries.get_mut(workspace) {
            entry.tabs.remove(&tab_id);
        }
        true
    }

    /// Forget every owned tab from one restarted browser slot.
    ///
    /// Browser slots survive process restarts, but browser-native tab ids do not. Keeping their
    /// composite ownership would let a reused native id inherit the prior process's workspace.
    /// Workspace handles remain live; only browser-process-local members are removed.
    pub fn purge_browser_slot(&self, browser_slot: u32) -> Vec<i64> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut removed = state
            .tab_owners
            .keys()
            .copied()
            .filter(|tab_id| crate::constants::tab_id::decode(*tab_id).0 == browser_slot)
            .collect::<Vec<_>>();
        removed.sort_unstable();
        for tab_id in &removed {
            if let Some(workspace) = state.tab_owners.remove(tab_id) {
                if let Some(entry) = state.entries.get_mut(&workspace) {
                    entry.tabs.remove(tab_id);
                }
            }
        }
        removed
    }

    /// Remove detached entries whose grace elapsed, never removing active work.
    pub fn reap_expired(&self, now: Instant) -> Vec<WorkspaceId> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let expired: Vec<WorkspaceId> = state
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.active == 0
                    && entry.attached == 0
                    && entry.idle_deadline.is_some_and(|deadline| deadline <= now)
            })
            .map(|(workspace, _)| workspace.clone())
            .collect();
        for workspace in &expired {
            state.retire(workspace);
        }
        expired
    }

    /// Drain workspace ids retired since the last cleanup pass.
    pub fn take_retired(&self) -> Vec<RetiredWorkspace> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut state.retired)
    }

    /// Return whether a same-user workspace handle is currently live.
    pub fn contains(&self, workspace: &WorkspaceId, owner: &PeerUser) -> bool {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .get(workspace)
            .is_some_and(|entry| &entry.owner == owner && !entry.retire_when_idle)
    }

    /// Return a deterministic, bearer-redacted snapshot for the local management UI.
    pub fn summaries(&self) -> Vec<WorkspaceSummary> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut summaries: Vec<WorkspaceSummary> = state
            .entries
            .values()
            .map(|entry| {
                let mut owned_tab_ids: Vec<i64> = entry.tabs.iter().copied().collect();
                owned_tab_ids.sort_unstable();
                WorkspaceSummary {
                    attached: entry.attached,
                    active: entry.active,
                    owned_tab_ids,
                }
            })
            .collect();
        summaries.sort_by(|left, right| {
            left.owned_tab_ids
                .cmp(&right.owned_tab_ids)
                .then(left.attached.cmp(&right.attached))
                .then(left.active.cmp(&right.active))
        });
        summaries
    }
}

/// RAII pin preventing workspace expiry while work is active.
pub struct WorkspaceLease {
    registry: WorkspaceRegistry,
    workspace: WorkspaceId,
}

impl WorkspaceLease {
    /// Return the pinned workspace.
    pub fn workspace(&self) -> &WorkspaceId {
        &self.workspace
    }
}

impl Drop for WorkspaceLease {
    fn drop(&mut self) {
        let mut state = self
            .registry
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(entry) = state.entries.get_mut(&self.workspace) else {
            return;
        };
        entry.active = entry.active.saturating_sub(1);
        if entry.active == 0 && entry.attached == 0 {
            entry.idle_deadline = Some(if entry.retire_when_idle {
                Instant::now()
            } else {
                Instant::now() + self.registry.idle_grace
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> WorkspaceRegistry {
        WorkspaceRegistry::with_idle_grace(
            Arc::new(Mutex::new(HashMap::new())),
            Duration::from_secs(5),
        )
    }

    #[test]
    fn service_mints_and_redacts_workspace_handles() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let workspace = registry.mint(&owner, false).unwrap();
        assert!(WorkspaceId::parse(workspace.as_str()).is_some());
        assert!(!format!("{workspace:?}").contains(workspace.as_str()));
        assert!(registry.contains(&workspace, &owner));
        assert!(!registry.contains(&workspace, &PeerUser("other".into())));
    }

    #[test]
    fn active_work_pins_a_detached_workspace_until_the_lease_drops() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let workspace = registry.mint(&owner, false).unwrap();
        let lease = registry.lease(&workspace, &owner).unwrap();
        assert!(registry
            .reap_expired(Instant::now() + Duration::from_secs(60))
            .is_empty());
        drop(lease);
        assert_eq!(
            registry.reap_expired(Instant::now() + Duration::from_secs(60)),
            vec![workspace]
        );
    }

    #[test]
    fn clean_release_waits_for_active_work_and_then_retires() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let workspace = registry.mint(&owner, true).unwrap();
        let lease = registry.lease(&workspace, &owner).unwrap();
        registry.release(&workspace, &owner).unwrap();
        assert!(registry.take_retired().is_empty());
        drop(lease);
        assert_eq!(
            registry.reap_expired(Instant::now()),
            vec![workspace.clone()]
        );
        assert_eq!(registry.take_retired()[0].workspace, workspace);
    }

    #[test]
    fn tab_membership_is_workspace_isolated_and_cleanup_is_complete() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let first = registry.mint(&owner, false).unwrap();
        let second = registry.mint(&owner, false).unwrap();
        assert_eq!(registry.claim_tab(&first, 7), TabClaim::Adopted);
        assert!(registry.owns_tab(&first, 7));
        assert!(!registry.owns_tab(&second, 7));
        assert!(!registry.owns_tab(&first, 8));
        assert_eq!(registry.claim_tab(&first, 7), TabClaim::Owned);
        assert_eq!(registry.claim_tab(&second, 7), TabClaim::Refused);
        assert_eq!(registry.owned_tabs(&first), vec![7]);
        assert!(registry.release_tab(&first, 7));
        assert_eq!(registry.claim_tab(&second, 7), TabClaim::Adopted);
    }

    #[test]
    fn multi_tab_claim_is_atomic_when_one_tab_belongs_elsewhere() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let first = registry.mint(&owner, false).unwrap();
        let second = registry.mint(&owner, false).unwrap();
        assert_eq!(registry.claim_tab(&first, 7), TabClaim::Adopted);

        assert_eq!(registry.claim_tabs(&second, &[8, 7]), TabClaim::Refused);
        assert!(registry.owned_tabs(&second).is_empty());
        assert_eq!(registry.claim_tab(&first, 8), TabClaim::Adopted);
    }

    #[test]
    fn browser_restart_purges_only_that_slots_tab_membership() {
        let registry = registry();
        let owner = PeerUser("owner".into());
        let workspace = registry.mint(&owner, false).unwrap();
        let old_a = crate::constants::tab_id::encode(2, 7);
        let old_b = crate::constants::tab_id::encode(2, 8);
        let other = crate::constants::tab_id::encode(3, 7);
        assert_eq!(
            registry.claim_tabs(&workspace, &[old_a, other, old_b]),
            TabClaim::Adopted
        );

        assert_eq!(registry.purge_browser_slot(2), vec![old_a, old_b]);
        assert_eq!(registry.owned_tabs(&workspace), vec![other]);
        assert_eq!(registry.claim_tab(&workspace, old_a), TabClaim::Adopted);
    }
}
