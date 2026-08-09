// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Service-owned browser workspace lifecycle and ownership.
//!
//! A workspace is application continuity, not connection identity. The service mints every
//! handle, validates it only after owner-only local admission, pins it while work is active, and
//! expires detached state after a bounded grace period. The registry also owns tab membership so
//! a tab handle can never silently cross workspaces.

use crate::hub::peer::PeerUser;
use crate::hub::{try_mint, MintGuard, MintQuota};
use crate::tool::outcome::{NativeTabFact, OperationTopology};
use ghostlight_transport::operation::{
    BrowserResult, Operation, OperationKind, PageProvenance, ResultTab, TabHandle, MAX_RESULT_TABS,
};
use ghostlight_transport::workspace_id::WorkspaceId;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Detached workspaces remain reusable for this long unless active work pins them.
pub const WORKSPACE_IDLE_GRACE: Duration = Duration::from_secs(120);

const TAB_HANDLE_PREFIX: &str = "t_";

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
    tabs: HashMap<i64, TabHandle>,
    current_tab: Option<i64>,
    _quota: MintGuard,
}

#[derive(Clone)]
struct TabBinding {
    workspace: WorkspaceId,
    tab_id: i64,
}

struct RegistryState {
    entries: HashMap<WorkspaceId, WorkspaceEntry>,
    tab_owners: HashMap<i64, WorkspaceId>,
    tab_bindings: HashMap<TabHandle, TabBinding>,
    retired: Vec<RetiredWorkspace>,
}

impl RegistryState {
    fn mint_tab_handle(&self) -> TabHandle {
        loop {
            let raw = format!("{TAB_HANDLE_PREFIX}{}", uuid::Uuid::new_v4().simple());
            let handle = TabHandle::parse(&raw).expect("service-minted tab handle is valid");
            if !self.tab_bindings.contains_key(&handle) {
                return handle;
            }
        }
    }

    fn retire(&mut self, workspace: &WorkspaceId) -> bool {
        let Some(entry) = self.entries.remove(workspace) else {
            return false;
        };
        let mut bindings: Vec<(i64, TabHandle)> = entry.tabs.into_iter().collect();
        bindings.sort_by_key(|(tab_id, _)| *tab_id);
        for (tab_id, handle) in &bindings {
            if self.tab_owners.get(tab_id) == Some(workspace) {
                self.tab_owners.remove(tab_id);
            }
            if self
                .tab_bindings
                .get(handle)
                .is_some_and(|binding| &binding.workspace == workspace && binding.tab_id == *tab_id)
            {
                self.tab_bindings.remove(handle);
            }
        }
        self.retired.push(RetiredWorkspace {
            workspace: workspace.clone(),
            tabs: bindings.into_iter().map(|(tab_id, _)| tab_id).collect(),
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
                tab_bindings: HashMap::new(),
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
                tabs: HashMap::new(),
                current_tab: None,
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

    /// Return the opaque service handle for one exact owned native tab.
    pub fn tab_handle(&self, workspace: &WorkspaceId, tab_id: i64) -> Option<TabHandle> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .get(workspace)
            .and_then(|entry| entry.tabs.get(&tab_id))
            .cloned()
    }

    /// Resolve an opaque tab handle only inside its exact live workspace.
    ///
    /// The workspace remains the bearer authority. A tab handle is verification-only, and an
    /// unknown handle and a handle owned by another workspace deliberately have the same result.
    pub fn resolve_tab(&self, workspace: &WorkspaceId, handle: &TabHandle) -> Option<i64> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let binding = state.tab_bindings.get(handle)?;
        (&binding.workspace == workspace
            && state.tab_owners.get(&binding.tab_id) == Some(workspace))
        .then_some(binding.tab_id)
    }

    /// Return the deterministic current controlled tab for one live workspace.
    pub fn current_tab(&self, workspace: &WorkspaceId) -> Option<i64> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .get(workspace)
            .and_then(|entry| entry.current_tab)
    }

    /// Select one exact owned tab as current for later omission-tolerant calls.
    pub fn select_tab(&self, workspace: &WorkspaceId, tab_id: i64) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.tab_owners.get(&tab_id) != Some(workspace) {
            return false;
        }
        let Some(entry) = state.entries.get_mut(workspace) else {
            return false;
        };
        entry.current_tab = Some(tab_id);
        true
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
            let handle = state.mint_tab_handle();
            state.tab_owners.insert(tab_id, workspace.clone());
            state.tab_bindings.insert(
                handle.clone(),
                TabBinding {
                    workspace: workspace.clone(),
                    tab_id,
                },
            );
            state
                .entries
                .get_mut(workspace)
                .expect("workspace existence checked above")
                .tabs
                .insert(tab_id, handle);
            adopted = true;
        }
        if let Some(entry) = state.entries.get_mut(workspace) {
            if entry.current_tab.is_none() {
                entry.current_tab = entry.tabs.keys().copied().min();
            }
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
            .map(|entry| entry.tabs.keys().copied().collect())
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
        if let Some(handle) = state.entries.get_mut(workspace).and_then(|entry| {
            let handle = entry.tabs.remove(&tab_id);
            if entry.current_tab == Some(tab_id) {
                entry.current_tab = entry.tabs.keys().copied().min();
            }
            handle
        }) {
            state.tab_bindings.remove(&handle);
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
                if let Some(handle) = state.entries.get_mut(&workspace).and_then(|entry| {
                    let handle = entry.tabs.remove(tab_id);
                    if entry.current_tab == Some(*tab_id) {
                        entry.current_tab = entry.tabs.keys().copied().min();
                    }
                    handle
                }) {
                    state.tab_bindings.remove(&handle);
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

    /// Add workspace-issued opaque tab facts to a canonical result without changing legacy data.
    ///
    /// Direct results use the trusted result tab id when present and otherwise the admitted input
    /// tab. Flow results additionally enrich each non-flow step from its already normalized
    /// operation. Numeric compatibility fields remain in `data` for the frozen legacy encoder.
    pub fn enrich_canonical_result_tabs(
        &self,
        workspace: &WorkspaceId,
        operation: &Operation,
        result: &mut BrowserResult,
        topology: &OperationTopology,
    ) {
        self.claim_trusted_result_topology(workspace, operation, topology);
        let requested_tab = topology
            .affected_tab
            .or_else(|| canonical_requested_tab(self, workspace, operation));
        self.enrich_one_result(workspace, requested_tab, result, topology);
        if matches!(
            result.status,
            ghostlight_transport::operation::BrowserResultStatus::Ok
                | ghostlight_transport::operation::BrowserResultStatus::Partial
        ) {
            match operation.kind() {
                OperationKind::BrowserFocusTab => {
                    if let Some(tab_id) = requested_tab {
                        self.select_tab(workspace, tab_id);
                    }
                }
                OperationKind::BrowserOpenTab | OperationKind::BrowserNavigate => {
                    if let Some(tab) = result.tab.as_ref() {
                        if let Some(tab_id) = self.resolve_tab(workspace, &tab.id) {
                            self.select_tab(workspace, tab_id);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn claim_trusted_result_topology(
        &self,
        workspace: &WorkspaceId,
        operation: &Operation,
        topology: &OperationTopology,
    ) {
        if !matches!(
            operation.kind(),
            OperationKind::BrowserOpenTab
                | OperationKind::BrowserNavigate
                | OperationKind::BrowserListTabs
        ) {
            return;
        }
        let mut tabs = topology
            .inventory
            .iter()
            .map(|tab| tab.tab_id)
            .collect::<Vec<_>>();
        if let Some(tab) = topology.affected_tab {
            if !tabs.contains(&tab) {
                tabs.push(tab);
            }
        }
        let _ = self.claim_tabs(workspace, &tabs);
    }

    fn enrich_one_result(
        &self,
        workspace: &WorkspaceId,
        requested_tab: Option<i64>,
        result: &mut BrowserResult,
        topology: &OperationTopology,
    ) {
        self.invalidate_closed_result_tabs(workspace, topology);
        self.enrich_tab_inventory(workspace, result, topology);
        if result.tab.is_some() {
            apply_final_navigation_url(result, topology.final_navigation_url.clone());
            return;
        }
        let candidates = topology.candidates.clone();
        let selected = if let Some(tab_id) = requested_tab {
            let mut facts = ResultTabFacts {
                tab_id,
                url: None,
                title: None,
                redacted: None,
            };
            for matching in candidates
                .into_iter()
                .filter(|candidate| candidate.tab_id == tab_id)
            {
                facts.url = facts.url.or(matching.url);
                facts.title = facts.title.or(matching.title);
                facts.redacted = facts.redacted.or(matching.redacted);
            }
            self.tab_handle(workspace, tab_id)
                .map(|handle| (facts, handle))
        } else {
            candidates.into_iter().find_map(|facts| {
                self.tab_handle(workspace, facts.tab_id)
                    .map(|handle| (facts, handle))
            })
        };
        let Some((facts, handle)) = selected else {
            if requested_tab.is_none() && result.operation == OperationKind::BrowserOpenTab {
                result.tab = result.tabs.first().cloned();
                apply_final_navigation_url(result, topology.final_navigation_url.clone());
                extend_tab_provenance(result);
            }
            return;
        };

        let (url, title) = if let Some(final_url) = topology.final_navigation_url.clone() {
            (Some(final_url), None)
        } else if result.provenance.is_some() {
            (facts.url, facts.title)
        } else {
            (None, None)
        };
        result.tab = Some(ResultTab {
            id: handle,
            url,
            title,
            current: requested_tab
                .is_some_and(|tab_id| self.current_tab(workspace) == Some(tab_id)),
            redacted: facts.redacted,
        });
        extend_tab_provenance(result);
    }

    fn enrich_tab_inventory(
        &self,
        workspace: &WorkspaceId,
        result: &mut BrowserResult,
        topology: &OperationTopology,
    ) {
        if !result.tabs.is_empty() {
            return;
        }
        let include_page_facts = result.provenance.is_some();
        let mut seen = HashSet::new();
        for facts in topology.inventory.iter().cloned() {
            if result.tabs.len() == MAX_RESULT_TABS {
                break;
            }
            if !seen.insert(facts.tab_id) {
                continue;
            }
            let Some(handle) = self.tab_handle(workspace, facts.tab_id) else {
                continue;
            };
            result.tabs.push(ResultTab {
                id: handle,
                url: include_page_facts.then_some(facts.url).flatten(),
                title: include_page_facts.then_some(facts.title).flatten(),
                current: self.current_tab(workspace) == Some(facts.tab_id),
                redacted: facts.redacted,
            });
        }
        extend_tab_inventory_provenance(result);
    }

    fn invalidate_closed_result_tabs(&self, workspace: &WorkspaceId, topology: &OperationTopology) {
        for tab_id in &topology.closed_tabs {
            self.release_tab(workspace, *tab_id);
        }
    }

    /// Return a deterministic, bearer-redacted snapshot for the local management UI.
    pub fn summaries(&self) -> Vec<WorkspaceSummary> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut summaries: Vec<WorkspaceSummary> = state
            .entries
            .values()
            .map(|entry| {
                let mut owned_tab_ids: Vec<i64> = entry.tabs.keys().copied().collect();
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

type ResultTabFacts = NativeTabFact;

fn canonical_requested_tab(
    registry: &WorkspaceRegistry,
    workspace: &WorkspaceId,
    operation: &Operation,
) -> Option<i64> {
    use Operation as C;

    let explicit = match operation {
        C::BrowserFocusTab(arguments) | C::BrowserCloseTab(arguments) => Some(&arguments.tab),
        C::BrowserNavigate(arguments) => arguments.tab.as_ref(),
        C::BrowserGoBack(arguments)
        | C::BrowserGoForward(arguments)
        | C::BrowserReloadPage(arguments)
        | C::BrowserPressEscape(arguments)
        | C::BrowserGetDialog(arguments) => arguments.tab.as_ref(),
        C::BrowserInspectPage(arguments) => arguments.tab.as_ref(),
        C::BrowserReadPage(arguments) => arguments.tab.as_ref(),
        C::BrowserTakeScreenshot(arguments) => arguments.tab.as_ref(),
        C::BrowserClick(arguments) => arguments.tab.as_ref(),
        C::BrowserHover(arguments) | C::BrowserScrollToTarget(arguments) => arguments.tab.as_ref(),
        C::BrowserScrollPage(arguments) => arguments.tab.as_ref(),
        C::BrowserPressKey(arguments) => arguments.tab.as_ref(),
        C::BrowserDrag(arguments) => arguments.tab.as_ref(),
        C::BrowserFillForm(arguments) => arguments.tab.as_ref(),
        C::BrowserWaitFor(arguments) => arguments.tab.as_ref(),
        C::BrowserRunSequence(arguments) => arguments.tab.as_ref(),
        C::BrowserHandleDialog(arguments) => arguments.tab.as_ref(),
        C::BrowserGetStatus(_) | C::BrowserOpenTab(_) | C::BrowserListTabs(_) => None,
    };
    explicit
        .and_then(|handle| registry.resolve_tab(workspace, handle))
        .or_else(|| {
            (!matches!(operation, C::BrowserOpenTab(_) | C::BrowserListTabs(_)))
                .then(|| registry.current_tab(workspace))
                .flatten()
        })
}

fn apply_final_navigation_url(result: &mut BrowserResult, final_url: Option<String>) {
    let (Some(tab), Some(final_url)) = (result.tab.as_mut(), final_url) else {
        return;
    };
    tab.url = Some(final_url);
    tab.title = None;
}

fn extend_tab_provenance(result: &mut BrowserResult) {
    let Some(tab) = result.tab.as_ref() else {
        return;
    };
    let Some(provenance) = result.provenance.as_ref() else {
        return;
    };
    let mut fields = provenance.untrusted_fields().to_vec();
    if tab.url.is_some() && !fields.iter().any(|field| field == "/tab/url") {
        fields.push("/tab/url".to_string());
    }
    if tab.title.is_some() && !fields.iter().any(|field| field == "/tab/title") {
        fields.push("/tab/title".to_string());
    }
    result.provenance = Some(
        PageProvenance::new(
            fields,
            provenance.top_origin().map(str::to_owned),
            provenance.session_nonce().map(str::to_owned),
            provenance.frame_origin().map(str::to_owned),
        )
        .expect("existing provenance plus tab page fields remains valid"),
    );
}

fn extend_tab_inventory_provenance(result: &mut BrowserResult) {
    let Some(provenance) = result.provenance.as_ref() else {
        return;
    };
    let mut fields = provenance.untrusted_fields().to_vec();
    let original_len = fields.len();
    for (index, tab) in result.tabs.iter().enumerate() {
        if tab.url.is_some() {
            let pointer = format!("/tabs/{index}/url");
            if !fields.contains(&pointer) {
                fields.push(pointer);
            }
        }
        if tab.title.is_some() {
            let pointer = format!("/tabs/{index}/title");
            if !fields.contains(&pointer) {
                fields.push(pointer);
            }
        }
    }
    if fields.len() == original_len {
        return;
    }
    result.provenance = Some(
        PageProvenance::new(
            fields,
            provenance.top_origin().map(str::to_owned),
            provenance.session_nonce().map(str::to_owned),
            provenance.frame_origin().map(str::to_owned),
        )
        .expect("existing provenance plus tab inventory page fields remains valid"),
    );
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
