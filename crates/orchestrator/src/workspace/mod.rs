//! The workspace aggregate, opaque handles, ownership, document generations, and leases.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use ghostlight_bridge::browser::{
    BrowserReadiness, ObservedTarget, PhysicalPoint, PhysicalRectangle, PhysicalTab,
    ViewportGeometry,
};

use crate::language::outcome::TargetRole;
use ghostlight_bridge::service::{IntakeChannel, SessionMarker};
use thiserror::Error;
use uuid::Uuid;

/// Immutable user-facing summary of one admitted workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    /// Opaque workspace identity.
    pub id: String,
    /// Presentation-only client label, claimed by the edge.
    pub client_label: String,
    /// Which intake admitted this workspace. Attribution only (ADR-0105).
    pub channel: IntakeChannel,
    /// Whether one invocation currently owns the workspace lease.
    pub leased: bool,
    /// Number of controlled tabs.
    pub tab_count: usize,
    /// Number of controlled tabs held by runtime governance.
    pub held_tab_count: usize,
}

/// Opaque admitted MCP workspace handle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// String form used at process boundaries and in audit.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Look a workspace up by the string form that crosses process boundaries.
///
/// The handle is exactly its string, so borrowing one is free and hashes identically.
impl std::borrow::Borrow<str> for WorkspaceId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// Opaque model-facing controlled tab handle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TabHandle(String);

impl TabHandle {
    /// String form returned to the model.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque model-facing document-bound target handle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetHandle(String);

impl TargetHandle {
    /// String form returned to the model.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque model-facing screenshot view handle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ViewHandle(String);

impl ViewHandle {
    /// String form returned to the model.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque generation-bound handle to one semantic document-tree snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SnapshotHandle(String);

impl SnapshotHandle {
    /// String form returned to the model.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One stored semantic document-tree snapshot for a controlled tab.
#[derive(Clone, Debug)]
pub struct SnapshotState {
    /// Owning controlled tab.
    pub tab: TabHandle,
    /// Document generation observed.
    pub generation: u64,
    /// Bounded structure-only tree.
    pub tree: serde_json::Value,
}

/// Opaque handle to one volatile captured-image asset.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ImageHandle(String);

impl ImageHandle {
    /// String form returned to the model.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One volatile captured-image asset held beside its view.
#[derive(Clone, Debug)]
pub struct ImageState {
    /// Owning controlled tab.
    pub tab: TabHandle,
    /// Document generation captured.
    pub generation: u64,
    /// Bounded media type of the capture.
    pub mime_type: String,
    /// Base64-encoded image bytes; never persisted anywhere else.
    pub data: String,
}

/// Immutable selected controlled-tab facts used by the executor.
#[derive(Clone, Debug)]
pub struct SelectedTab {
    /// Opaque model-facing handle.
    pub handle: TabHandle,
    /// Physical Chromium tab id.
    pub physical_id: u64,
    /// Current document generation.
    pub generation: u64,
    /// Last governed URL.
    pub url: String,
    /// Last bounded title.
    pub title: String,
    /// Last governed readiness.
    pub readiness: BrowserReadiness,
    /// Whether runtime governance holds the tab.
    pub held: bool,
    /// Whether the tab is active in the workspace.
    pub active: bool,
}

/// Immutable current target facts used at a browser-effect boundary.
#[derive(Clone, Debug)]
pub struct SelectedTarget {
    /// Opaque model-facing handle.
    pub handle: TargetHandle,
    /// Owning tab handle.
    pub tab: TabHandle,
    /// Browser-local locator never exposed to the model.
    pub locator: String,
    /// Credential classification last observed by the adapter.
    pub credential_class: bool,
    /// What kind of control this is, narrowed to Ghostlight's own closed vocabulary.
    pub role: TargetRole,
}

/// Immutable current screenshot transform used at a pointer-effect boundary.
#[derive(Clone, Debug)]
pub struct SelectedView {
    /// Opaque model-facing view handle.
    pub handle: ViewHandle,
    /// Owning tab handle.
    pub tab: TabHandle,
    /// Exact browser capture transform.
    pub viewport: ViewportGeometry,
    /// Returned image width in pixels.
    pub width: u32,
    /// Returned image height in pixels.
    pub height: u32,
}

/// The physical tabs a released workspace leaves behind, in the browser that holds them.
///
/// Tab ids travel with their browser or they mean nothing: cleaning up tab 5 without saying whose
/// tab 5 it is would close a stranger's tab in whichever browser answered.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReleasedTabs {
    /// The browser holding the tabs, absent when the workspace never opened one.
    pub browser: Option<String>,
    /// Physical tab ids the workspace owned.
    pub physical_ids: Vec<u64>,
}

impl ReleasedTabs {
    fn from_state(state: WorkspaceState) -> Self {
        Self {
            browser: state.browser,
            physical_ids: state
                .tabs
                .into_values()
                .map(|tab| tab.physical_id)
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
struct TargetState {
    tab: TabHandle,
    generation: u64,
    locator: String,
    credential_class: bool,
    role: TargetRole,
}

#[derive(Clone, Debug)]
struct ViewState {
    tab: TabHandle,
    generation: u64,
    viewport: ViewportGeometry,
    width: u32,
    height: u32,
}

#[derive(Clone, Debug)]
struct TabState {
    physical_id: u64,
    generation: u64,
    url: String,
    title: String,
    readiness: BrowserReadiness,
    active: bool,
    held: bool,
}

#[derive(Debug)]
struct WorkspaceState {
    client_label: String,
    channel: IntakeChannel,
    /// What owns this workspace, when it outlives the connection that opened it (ADR-0106).
    session: Option<SessionMarker>,
    /// Which browser this workspace works in, once its first work chose one.
    ///
    /// A workspace lives in one browser for its whole life. Its tabs, targets, and views are all
    /// physical things inside that one browser, and physical tab ids only mean anything there, so
    /// a workspace that could span two browsers could not name its own tabs unambiguously. When
    /// that browser stops, the binding stays: the work belongs to a person's Chrome profile, and
    /// silently finishing it in their Edge profile would be a different act than the one asked
    /// for (ADR-0084 D4).
    browser: Option<String>,
    leased: bool,
    tabs: HashMap<TabHandle, TabState>,
    targets: HashMap<TargetHandle, TargetState>,
    views: HashMap<ViewHandle, ViewState>,
    snapshots: HashMap<SnapshotHandle, SnapshotState>,
    images: HashMap<ImageHandle, ImageState>,
}

#[derive(Debug, Default)]
struct AggregateState {
    workspaces: HashMap<WorkspaceId, WorkspaceState>,
}

/// Thread-safe entry to the single workspace aggregate.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceStore {
    inner: Arc<Mutex<AggregateState>>,
}

impl WorkspaceStore {
    /// Return a stable, content-free snapshot of every admitted workspace.
    #[must_use]
    pub fn summaries(&self) -> Vec<WorkspaceSummary> {
        let state = self.lock();
        let mut summaries: Vec<_> = state
            .workspaces
            .iter()
            .map(|(id, workspace)| WorkspaceSummary {
                id: id.as_str().into(),
                client_label: workspace.client_label.clone(),
                channel: workspace.channel,
                leased: workspace.leased,
                tab_count: workspace.tabs.len(),
                held_tab_count: workspace.tabs.values().filter(|tab| tab.held).count(),
            })
            .collect();
        summaries.sort_by(|left, right| left.id.cmp(&right.id));
        summaries
    }

    /// Admit one edge connection as an isolated workspace bound to that connection.
    pub fn admit(&self, client_label: String, channel: IntakeChannel) -> WorkspaceId {
        self.open(client_label, channel, None)
    }

    /// Resume the workspace this session already owns, or open one for it.
    ///
    /// Two calls from the same caller reach the same tabs. That is the whole point: handles belong
    /// to a session, and a session is the caller rather than the socket.
    pub fn resume_or_admit(
        &self,
        client_label: String,
        channel: IntakeChannel,
        marker: SessionMarker,
    ) -> WorkspaceId {
        let key = marker.key();
        let existing = self
            .lock()
            .workspaces
            .iter()
            .find(|(_, state)| {
                state
                    .session
                    .as_ref()
                    .is_some_and(|owner| owner.key() == key)
            })
            .map(|(id, _)| id.clone());
        existing.unwrap_or_else(|| self.open(client_label, channel, Some(marker)))
    }

    /// Workspaces whose owner is gone, with the physical tabs they still hold.
    ///
    /// Liveness is supplied by the caller rather than observed here: the aggregate owns handles and
    /// ownership, not the operating system.
    pub fn reap(&self, alive: &dyn Fn(&SessionMarker) -> bool) -> Vec<ReleasedTabs> {
        let mut state = self.lock();
        let dead: Vec<WorkspaceId> = state
            .workspaces
            .iter()
            .filter(|(_, workspace)| {
                !workspace.leased
                    && workspace
                        .session
                        .as_ref()
                        .is_some_and(|owner| !alive(owner))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut released = Vec::new();
        for id in dead {
            if let Some(workspace) = state.workspaces.remove(&id) {
                released.push(ReleasedTabs::from_state(workspace));
            }
        }
        released.retain(|release| !release.physical_ids.is_empty());
        released
    }

    /// Every session marker currently owning a workspace.
    #[must_use]
    pub fn owners(&self) -> Vec<SessionMarker> {
        self.lock()
            .workspaces
            .values()
            .filter_map(|workspace| workspace.session.clone())
            .collect()
    }

    /// Whether this workspace outlives its connection.
    #[must_use]
    pub fn is_owned(&self, workspace: &WorkspaceId) -> bool {
        self.lock()
            .workspaces
            .get(workspace)
            .is_some_and(|state| state.session.is_some())
    }

    fn open(
        &self,
        client_label: String,
        channel: IntakeChannel,
        session: Option<SessionMarker>,
    ) -> WorkspaceId {
        let id = WorkspaceId(format!("workspace_{}", Uuid::new_v4().simple()));
        self.lock().workspaces.insert(
            id.clone(),
            WorkspaceState {
                client_label,
                channel,
                session,
                browser: None,
                leased: false,
                tabs: HashMap::new(),
                targets: HashMap::new(),
                views: HashMap::new(),
                snapshots: HashMap::new(),
                images: HashMap::new(),
            },
        );
        id
    }

    /// Release an MCP workspace and return the physical tabs it owned.
    pub fn release(&self, workspace: &WorkspaceId) -> ReleasedTabs {
        self.lock()
            .workspaces
            .remove(workspace)
            .map_or_else(ReleasedTabs::default, ReleasedTabs::from_state)
    }

    /// Acquire exclusive mutation ownership for one invocation.
    pub fn acquire(&self, workspace: &WorkspaceId) -> Result<WorkspaceLease, WorkspaceError> {
        let mut state = self.lock();
        let workspace_state = state
            .workspaces
            .get_mut(workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if workspace_state.leased {
            return Err(WorkspaceError::Busy);
        }
        workspace_state.leased = true;
        Ok(WorkspaceLease {
            store: self.clone(),
            workspace: workspace.clone(),
            released: false,
        })
    }

    /// Return presentation-only client label without using it for routing or authority.
    pub fn client_label(&self, workspace: &WorkspaceId) -> Result<String, WorkspaceError> {
        self.lock()
            .workspaces
            .get(workspace)
            .map(|state| state.client_label.clone())
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

    /// The intake that admitted a workspace, for attribution at completion.
    pub fn channel(&self, workspace: &WorkspaceId) -> Result<IntakeChannel, WorkspaceError> {
        self.lock()
            .workspaces
            .get(workspace)
            .map(|state| state.channel)
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

    /// Apply an asynchronous committed landing through the aggregate before later content use.
    /// Which browser this workspace works in, if its first work already chose one.
    #[must_use]
    pub fn browser_of(&self, workspace: &str) -> Option<String> {
        self.lock()
            .workspaces
            .get(workspace)
            .and_then(|state| state.browser.clone())
    }

    /// Bind a workspace to the browser its work belongs to.
    ///
    /// The first binding wins for the life of the workspace. Re-binding the same browser is the
    /// ordinary case and succeeds; naming a different one is refused rather than obeyed, because
    /// the tabs already open would stay behind in the browser that owns them.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::UnknownWorkspace`] when the workspace is gone, and
    /// [`WorkspaceError::BrowserPinned`] when it already works in a different browser.
    pub fn pin_browser(&self, workspace: &str, browser: &str) -> Result<(), WorkspaceError> {
        let mut state = self.lock();
        let workspace = state
            .workspaces
            .get_mut(workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        match workspace.browser.as_deref() {
            Some(pinned) if pinned == browser => Ok(()),
            Some(_) => Err(WorkspaceError::BrowserPinned),
            None => {
                workspace.browser = Some(browser.into());
                Ok(())
            }
        }
    }

    pub fn apply_browser_landing(
        &self,
        browser: &str,
        physical_id: u64,
        url: &str,
        allowed: bool,
    ) -> Option<(WorkspaceId, TabHandle)> {
        let mut state = self.lock();
        for (workspace_id, workspace) in state
            .workspaces
            .iter_mut()
            .filter(|(_, workspace)| workspace.browser.as_deref() == Some(browser))
        {
            if let Some((handle, tab)) = workspace
                .tabs
                .iter_mut()
                .find(|(_, tab)| tab.physical_id == physical_id)
            {
                tab.generation = tab.generation.saturating_add(1);
                tab.readiness = BrowserReadiness::Loading;
                tab.held = !allowed;
                if allowed {
                    tab.url = url.into();
                }
                return Some((workspace_id.clone(), handle.clone()));
            }
        }
        None
    }

    /// Resolve the owning workspace for an asynchronous physical browser event.
    ///
    /// A physical tab id is unique inside one browser and nowhere else, so the browser that
    /// reported the event is part of the key. Without it, Chrome's tab 5 and Edge's tab 5 are the
    /// same lookup, and one browser's navigation would be governed and audited against the
    /// other's tab.
    #[must_use]
    pub fn owner_of_physical(&self, browser: &str, physical_id: u64) -> Option<WorkspaceId> {
        let state = self.lock();
        state
            .workspaces
            .iter()
            .filter(|(_, workspace)| workspace.browser.as_deref() == Some(browser))
            .find_map(|(workspace_id, workspace)| {
                workspace
                    .tabs
                    .values()
                    .any(|tab| tab.physical_id == physical_id)
                    .then(|| workspace_id.clone())
            })
    }

    /// Apply asynchronous readiness only to a non-held controlled document.
    pub fn apply_browser_readiness(
        &self,
        browser: &str,
        physical_id: u64,
        readiness: BrowserReadiness,
    ) {
        let mut state = self.lock();
        for workspace in state
            .workspaces
            .values_mut()
            .filter(|workspace| workspace.browser.as_deref() == Some(browser))
        {
            if let Some(tab) = workspace
                .tabs
                .values_mut()
                .find(|tab| tab.physical_id == physical_id && !tab.held)
            {
                tab.readiness = readiness;
                return;
            }
        }
    }

    /// Remove a tab closed outside an invocation.
    pub fn apply_browser_close(&self, browser: &str, physical_id: u64) {
        let mut state = self.lock();
        for workspace in state
            .workspaces
            .values_mut()
            .filter(|workspace| workspace.browser.as_deref() == Some(browser))
        {
            let handle = workspace
                .tabs
                .iter()
                .find_map(|(handle, tab)| (tab.physical_id == physical_id).then(|| handle.clone()));
            if let Some(handle) = handle {
                workspace.tabs.remove(&handle);
                workspace.targets.retain(|_, target| target.tab != handle);
                workspace.views.retain(|_, view| view.tab != handle);
                return;
            }
        }
    }

    /// Adopt a physical child tab only through its already-owned opener.
    pub fn apply_browser_child(
        &self,
        browser: &str,
        opener_physical_id: u64,
        tab: &PhysicalTab,
    ) -> Option<(WorkspaceId, TabHandle)> {
        let mut state = self.lock();
        if state
            .workspaces
            .values()
            .filter(|workspace| workspace.browser.as_deref() == Some(browser))
            .any(|workspace| {
                workspace
                    .tabs
                    .values()
                    .any(|known| known.physical_id == tab.tab_id)
            })
        {
            return None;
        }
        for (workspace_id, workspace) in state
            .workspaces
            .iter_mut()
            .filter(|(_, workspace)| workspace.browser.as_deref() == Some(browser))
        {
            if !workspace
                .tabs
                .values()
                .any(|known| known.physical_id == opener_physical_id)
            {
                continue;
            }
            let handle = TabHandle(format!("tab_{}", Uuid::new_v4().simple()));
            workspace.tabs.insert(
                handle.clone(),
                TabState {
                    physical_id: tab.tab_id,
                    generation: 0,
                    url: String::new(),
                    title: String::new(),
                    readiness: tab.readiness,
                    active: tab.active,
                    held: false,
                },
            );
            return Some((workspace_id.clone(), handle));
        }
        None
    }

    fn lock(&self) -> MutexGuard<'_, AggregateState> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Exclusive workspace mutation lease held for one unit of work.
#[derive(Debug)]
pub struct WorkspaceLease {
    store: WorkspaceStore,
    workspace: WorkspaceId,
    released: bool,
}

impl WorkspaceLease {
    /// Opaque owning workspace id.
    #[must_use]
    pub fn workspace(&self) -> &WorkspaceId {
        &self.workspace
    }

    /// List current governed controlled-tab facts.
    pub fn tabs(&self) -> Result<Vec<SelectedTab>, WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let mut tabs: Vec<_> = workspace
            .tabs
            .iter()
            .map(|(handle, tab)| selected(handle, tab))
            .collect();
        tabs.sort_by(|left, right| left.handle.as_str().cmp(right.handle.as_str()));
        Ok(tabs)
    }

    /// Select an exact or unambiguous controlled tab.
    pub fn select_tab(&self, requested: Option<&str>) -> Result<SelectedTab, WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if let Some(requested) = requested {
            let handle = TabHandle(requested.into());
            if let Some(tab) = workspace.tabs.get(&handle) {
                return held_or_selected(&handle, tab);
            }
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.tabs.contains_key(&handle)
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedTab
            } else {
                WorkspaceError::StaleTab
            });
        }
        if workspace.tabs.len() == 1 {
            let (handle, tab) = workspace.tabs.iter().next().expect("length checked");
            return held_or_selected(handle, tab);
        }
        let mut active = workspace.tabs.iter().filter(|(_, tab)| tab.active);
        let Some((handle, tab)) = active.next() else {
            return Err(if workspace.tabs.is_empty() {
                WorkspaceError::NoTab
            } else {
                WorkspaceError::AmbiguousTab
            });
        };
        if active.next().is_some() {
            return Err(WorkspaceError::AmbiguousTab);
        }
        held_or_selected(handle, tab)
    }

    /// Add a physical tab under a new opaque handle.
    pub fn add_tab(&self, tab: &PhysicalTab) -> Result<SelectedTab, WorkspaceError> {
        let mut state = self.store.lock();
        if state.workspaces.iter().any(|(id, workspace)| {
            id != &self.workspace
                && workspace
                    .tabs
                    .values()
                    .any(|known| known.physical_id == tab.tab_id)
        }) {
            return Err(WorkspaceError::PhysicalTabOwned);
        }
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        for known in workspace.tabs.values_mut() {
            known.active = false;
        }
        let handle = TabHandle(format!("tab_{}", Uuid::new_v4().simple()));
        let value = TabState {
            physical_id: tab.tab_id,
            generation: 0,
            url: String::new(),
            title: String::new(),
            readiness: BrowserReadiness::Unknown,
            active: true,
            held: false,
        };
        workspace.tabs.insert(handle.clone(), value.clone());
        Ok(selected(&handle, &value))
    }

    /// Mark one controlled tab active and all sibling tabs inactive.
    pub fn mark_active(&self, handle: &TabHandle) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if !workspace.tabs.contains_key(handle) {
            return Err(WorkspaceError::StaleTab);
        }
        for (candidate, tab) in &mut workspace.tabs {
            tab.active = candidate == handle;
        }
        Ok(())
    }

    /// Apply one governed committed document and invalidate prior targets by generation.
    pub fn apply_landing(
        &self,
        handle: &TabHandle,
        tab: &PhysicalTab,
    ) -> Result<SelectedTab, WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let known = workspace
            .tabs
            .get_mut(handle)
            .ok_or(WorkspaceError::StaleTab)?;
        if known.physical_id != tab.tab_id {
            return Err(WorkspaceError::StaleTab);
        }
        known.generation = known.generation.saturating_add(1);
        known.url.clone_from(&tab.url);
        known.title = bounded(&tab.title, 500);
        known.readiness = tab.readiness;
        known.active = tab.active;
        known.held = false;
        Ok(selected(handle, known))
    }

    /// Update readiness without accepting new page content.
    pub fn update_readiness(
        &self,
        handle: &TabHandle,
        readiness: BrowserReadiness,
    ) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let tab = workspace
            .tabs
            .get_mut(handle)
            .ok_or(WorkspaceError::StaleTab)?;
        tab.readiness = readiness;
        Ok(())
    }

    /// Enter a tab hold after a denied committed landing.
    pub fn hold_tab(&self, handle: &TabHandle) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        workspace
            .tabs
            .get_mut(handle)
            .ok_or(WorkspaceError::StaleTab)?
            .held = true;
        Ok(())
    }

    /// Confirm decisive physical closure, tolerating its earlier asynchronous close event.
    /// Store one semantic tree for a tab, superseding any earlier snapshot.
    pub fn register_snapshot(
        &self,
        tab: &SelectedTab,
        tree: serde_json::Value,
    ) -> Result<SnapshotHandle, WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let known = workspace
            .tabs
            .get(&tab.handle)
            .ok_or(WorkspaceError::StaleTab)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace
            .snapshots
            .retain(|_, snapshot| snapshot.tab != tab.handle);
        let handle = SnapshotHandle(format!("snapshot_{}", Uuid::new_v4().simple()));
        workspace.snapshots.insert(
            handle.clone(),
            SnapshotState {
                tab: tab.handle.clone(),
                generation: known.generation,
                tree,
            },
        );
        Ok(handle)
    }

    /// Return the current-generation tree for a tab, if one is recorded.
    #[must_use]
    pub fn previous_snapshot(&self, tab: &SelectedTab) -> Option<serde_json::Value> {
        let state = self.store.lock();
        let workspace = state.workspaces.get(&self.workspace)?;
        let known = workspace.tabs.get(&tab.handle)?;
        if known.generation != tab.generation {
            return None;
        }
        workspace
            .snapshots
            .values()
            .find(|snapshot| snapshot.tab == tab.handle && snapshot.generation == known.generation)
            .map(|snapshot| snapshot.tree.clone())
    }

    /// Store one volatile captured image for a tab, superseding any earlier asset.
    /// The capture is refused when it exceeds the upload ceiling.
    pub fn register_image(
        &self,
        tab: &SelectedTab,
        mime_type: &str,
        data: &str,
        decoded_bytes: usize,
    ) -> Result<Option<ImageHandle>, WorkspaceError> {
        const UPLOAD_AGGREGATE_BYTES: usize = 5_000_000;
        if decoded_bytes > UPLOAD_AGGREGATE_BYTES {
            return Ok(None);
        }
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let known = workspace
            .tabs
            .get(&tab.handle)
            .ok_or(WorkspaceError::StaleTab)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace.images.retain(|_, image| image.tab != tab.handle);
        let handle = ImageHandle(format!("image_{}", Uuid::new_v4().simple()));
        workspace.images.insert(
            handle.clone(),
            ImageState {
                tab: tab.handle.clone(),
                generation: known.generation,
                mime_type: mime_type.to_string(),
                data: data.to_string(),
            },
        );
        Ok(Some(handle))
    }

    /// Return one current-generation captured image, if it is still owned here.
    #[must_use]
    pub fn take_image(&self, handle: &str, tab: &SelectedTab) -> Option<(String, String)> {
        let state = self.store.lock();
        let workspace = state.workspaces.get(&self.workspace)?;
        let known = workspace.tabs.get(&tab.handle)?;
        if known.generation != tab.generation {
            return None;
        }
        workspace
            .images
            .iter()
            .find(|(asset_handle, image)| {
                asset_handle.as_str() == handle
                    && image.tab == tab.handle
                    && image.generation == known.generation
            })
            .map(|(_, image)| (image.mime_type.clone(), image.data.clone()))
    }

    pub fn confirm_tab_closed(&self, handle: &TabHandle) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        workspace.tabs.remove(handle);
        workspace.targets.retain(|_, target| &target.tab != handle);
        workspace.views.retain(|_, view| &view.tab != handle);
        workspace
            .snapshots
            .retain(|_, snapshot| &snapshot.tab != handle);
        workspace.images.retain(|_, image| &image.tab != handle);
        Ok(())
    }

    /// Map observed browser targets to fresh opaque generation-bound handles.
    pub fn register_targets(
        &self,
        tab: &SelectedTab,
        targets: &[ObservedTarget],
    ) -> Result<Vec<(TargetHandle, ObservedTarget)>, WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let known = workspace
            .tabs
            .get(&tab.handle)
            .ok_or(WorkspaceError::StaleTab)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        let mut mapped = Vec::with_capacity(targets.len());
        for target in targets {
            let handle = TargetHandle(format!("target_{}", Uuid::new_v4().simple()));
            workspace.targets.insert(
                handle.clone(),
                TargetState {
                    tab: tab.handle.clone(),
                    generation: tab.generation,
                    locator: target.locator.clone(),
                    credential_class: target.credential_class,
                    // A page authors its own role attribute, so it is narrowed here, at the one
                    // door observed targets come through. The page's own string is never stored
                    // and so can never reach a sentence written to the audit.
                    role: TargetRole::classify(&target.role),
                },
            );
            mapped.push((handle, target.clone()));
        }
        Ok(mapped)
    }

    /// Resolve a target and prove ownership plus current document generation.
    pub fn resolve_target(
        &self,
        requested: &str,
        selected_tab: Option<&SelectedTab>,
    ) -> Result<SelectedTarget, WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let handle = TargetHandle(requested.into());
        let Some(target) = workspace.targets.get(&handle) else {
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.targets.contains_key(&handle)
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedTarget
            } else {
                WorkspaceError::StaleTarget
            });
        };
        let tab = workspace
            .tabs
            .get(&target.tab)
            .ok_or(WorkspaceError::StaleTarget)?;
        if target.generation != tab.generation {
            return Err(WorkspaceError::StaleTarget);
        }
        if let Some(selected) = selected_tab {
            if selected.handle != target.tab {
                return Err(WorkspaceError::TargetTabMismatch);
            }
        }
        if tab.held {
            return Err(WorkspaceError::Held);
        }
        Ok(SelectedTarget {
            handle,
            tab: target.tab.clone(),
            locator: target.locator.clone(),
            credential_class: target.credential_class,
            role: target.role,
        })
    }

    /// Register one screenshot transform and supersede older views for the same tab.
    pub fn register_view(
        &self,
        tab: &SelectedTab,
        viewport: ViewportGeometry,
        width: u32,
        height: u32,
    ) -> Result<ViewHandle, WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let known = workspace
            .tabs
            .get(&tab.handle)
            .ok_or(WorkspaceError::StaleTab)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace.views.retain(|_, view| view.tab != tab.handle);
        let handle = ViewHandle(format!("view_{}", Uuid::new_v4().simple()));
        workspace.views.insert(
            handle.clone(),
            ViewState {
                tab: tab.handle.clone(),
                generation: tab.generation,
                viewport,
                width,
                height,
            },
        );
        Ok(handle)
    }

    /// Resolve an image point to page CSS coordinates with ownership and generation checks.
    pub fn resolve_view_point(
        &self,
        requested: &str,
        selected_tab: Option<&SelectedTab>,
        x: f64,
        y: f64,
    ) -> Result<(SelectedView, PhysicalPoint), WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let handle = ViewHandle(requested.into());
        let Some(view) = workspace.views.get(&handle) else {
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.views.contains_key(&handle)
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedView
            } else {
                WorkspaceError::StaleView
            });
        };
        let tab = workspace
            .tabs
            .get(&view.tab)
            .ok_or(WorkspaceError::StaleView)?;
        if view.generation != tab.generation {
            return Err(WorkspaceError::StaleView);
        }
        if selected_tab.is_some_and(|selected| selected.handle != view.tab) {
            return Err(WorkspaceError::ViewTabMismatch);
        }
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= f64::from(view.width)
            || y >= f64::from(view.height)
            || view.viewport.output_scale <= 0.0
        {
            return Err(WorkspaceError::ViewPointOutOfBounds);
        }
        let selected = SelectedView {
            handle,
            tab: view.tab.clone(),
            viewport: view.viewport,
            width: view.width,
            height: view.height,
        };
        let point = PhysicalPoint {
            x: view.viewport.page_x + x / view.viewport.output_scale,
            y: view.viewport.page_y + y / view.viewport.output_scale,
        };
        Ok((selected, point))
    }

    /// Resolve an image rectangle to page CSS coordinates with ownership and generation checks.
    pub fn resolve_view_region(
        &self,
        requested: &str,
        selected_tab: Option<&SelectedTab>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(SelectedView, PhysicalRectangle), WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let handle = ViewHandle(requested.into());
        let Some(view) = workspace.views.get(&handle) else {
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.views.contains_key(&handle)
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedView
            } else {
                WorkspaceError::StaleView
            });
        };
        let tab = workspace
            .tabs
            .get(&view.tab)
            .ok_or(WorkspaceError::StaleView)?;
        if view.generation != tab.generation {
            return Err(WorkspaceError::StaleView);
        }
        if selected_tab.is_some_and(|selected| selected.handle != view.tab) {
            return Err(WorkspaceError::ViewTabMismatch);
        }
        let right = x + width;
        let bottom = y + height;
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || x < 0.0
            || y < 0.0
            || width <= 0.0
            || height <= 0.0
            || !right.is_finite()
            || !bottom.is_finite()
            || right > f64::from(view.width)
            || bottom > f64::from(view.height)
            || view.viewport.output_scale <= 0.0
        {
            return Err(WorkspaceError::ViewRegionOutOfBounds);
        }
        let selected = SelectedView {
            handle,
            tab: view.tab.clone(),
            viewport: view.viewport,
            width: view.width,
            height: view.height,
        };
        let region = PhysicalRectangle {
            x: view.viewport.page_x + x / view.viewport.output_scale,
            y: view.viewport.page_y + y / view.viewport.output_scale,
            width: width / view.viewport.output_scale,
            height: height / view.viewport.output_scale,
        };
        Ok((selected, region))
    }

    /// Invalidate screenshot coordinates after a viewport-changing operation.
    pub fn invalidate_views(&self, tab: &TabHandle) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        workspace.views.retain(|_, view| &view.tab != tab);
        Ok(())
    }

    /// Invalidate every view whose physical tab belongs to a resized browser window.
    pub fn invalidate_views_for_physical(
        &self,
        physical_ids: &[u64],
    ) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let affected: Vec<_> = workspace
            .tabs
            .iter()
            .filter(|(_, tab)| physical_ids.contains(&tab.physical_id))
            .map(|(handle, _)| handle.clone())
            .collect();
        workspace
            .views
            .retain(|_, view| !affected.contains(&view.tab));
        Ok(())
    }
}

impl Drop for WorkspaceLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        if let Some(workspace) = self.store.lock().workspaces.get_mut(&self.workspace) {
            workspace.leased = false;
        }
        self.released = true;
    }
}

fn selected(handle: &TabHandle, tab: &TabState) -> SelectedTab {
    SelectedTab {
        handle: handle.clone(),
        physical_id: tab.physical_id,
        generation: tab.generation,
        url: tab.url.clone(),
        title: tab.title.clone(),
        readiness: tab.readiness,
        held: tab.held,
        active: tab.active,
    }
}

fn held_or_selected(handle: &TabHandle, tab: &TabState) -> Result<SelectedTab, WorkspaceError> {
    if tab.held {
        Err(WorkspaceError::Held)
    } else {
        Ok(selected(handle, tab))
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

/// Workspace invariant or recovery failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WorkspaceError {
    /// MCP workspace is no longer admitted.
    #[error("workspace is no longer admitted")]
    UnknownWorkspace,
    /// Another invocation owns the workspace lease.
    #[error("workspace is busy")]
    Busy,
    /// No controlled tab exists.
    #[error("no controlled tab exists")]
    NoTab,
    /// More than one controlled tab is a plausible target.
    #[error("tab selection is ambiguous")]
    AmbiguousTab,
    /// Tab handle is closed or stale.
    #[error("tab handle is stale")]
    StaleTab,
    /// Tab belongs to another admitted workspace.
    #[error("tab is owned by another workspace")]
    NotOwnedTab,
    /// The workspace already works in a different browser.
    #[error("workspace is already working in another browser")]
    BrowserPinned,
    /// Target handle belongs to an old document or is unknown.
    #[error("target handle is stale")]
    StaleTarget,
    /// Target belongs to another admitted workspace.
    #[error("target is owned by another workspace")]
    NotOwnedTarget,
    /// View handle belongs to an old document, viewport, or is unknown.
    #[error("view handle is stale")]
    StaleView,
    /// View handle belongs to another admitted workspace.
    #[error("view is owned by another workspace")]
    NotOwnedView,
    /// Explicit tab does not own the target.
    #[error("target does not belong to the selected tab")]
    TargetTabMismatch,
    /// Explicit tab does not own the view.
    #[error("view does not belong to the selected tab")]
    ViewTabMismatch,
    /// Image coordinate is not finite or is outside the captured view.
    #[error("view coordinate is outside the captured image")]
    ViewPointOutOfBounds,
    /// Image rectangle is invalid or extends outside the captured view.
    #[error("view region is outside the captured image")]
    ViewRegionOutOfBounds,
    /// Runtime governance holds the tab.
    #[error("tab is held by runtime governance")]
    Held,
    /// Physical tab is already controlled by another workspace.
    #[error("physical tab is already controlled")]
    PhysicalTabOwned,
}

#[cfg(test)]
mod tests {
    use ghostlight_bridge::browser::{
        BrowserReadiness, CaptureScope, ObservedTarget, PhysicalTab, ViewportGeometry,
    };
    use ghostlight_bridge::service::{IntakeChannel, SessionMarker};

    use crate::language::outcome::TargetRole;

    use super::{ReleasedTabs, WorkspaceError, WorkspaceId, WorkspaceStore};

    const TEST_BROWSER: &str = "browser_test";

    /// Admit a workspace already working in one browser, the way real work arrives here.
    ///
    /// Nothing physical exists in a workspace until its first crossing binds it to a browser, so
    /// a test that exercises physical tabs starts from the same state.
    fn admit_in_browser(store: &WorkspaceStore) -> WorkspaceId {
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        store
            .pin_browser(workspace.as_str(), TEST_BROWSER)
            .expect("a fresh workspace binds to its first browser");
        workspace
    }

    fn physical(id: u64, url: &str) -> PhysicalTab {
        PhysicalTab {
            tab_id: id,
            title: "title".into(),
            url: url.into(),
            active: true,
            readiness: BrowserReadiness::Complete,
        }
    }

    fn marker(pid: u32, started_at: u64) -> SessionMarker {
        SessionMarker::Process {
            pid,
            started_at,
            name: "pwsh.exe".into(),
        }
    }

    #[test]
    fn one_caller_resumes_its_own_workspace_and_a_recycled_pid_does_not() {
        let store = WorkspaceStore::default();
        let first = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        let again = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        assert_eq!(
            first, again,
            "the same caller must reach the same workspace"
        );

        // The negative control, and the reason identity is not pid plus name: a recycled pid
        // running the same program must not inherit the dead session's tabs.
        let recycled = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 200));
        assert_ne!(first, recycled);

        // A connection with no marker is bound to that connection, as the MCP edge expects.
        let bound = store.admit("codex".into(), IntakeChannel::Mcp);
        assert!(!store.is_owned(&bound));
        assert!(store.is_owned(&first));
    }

    #[test]
    fn a_workspace_outlives_its_connection_and_dies_with_its_owner() {
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        store.pin_browser(owned.as_str(), TEST_BROWSER).unwrap();
        let lease = store.acquire(&owned).unwrap();
        let tab = lease
            .add_tab(&physical(41, "https://example.com/"))
            .unwrap();
        lease
            .apply_landing(&tab.handle, &physical(41, "https://example.com/"))
            .unwrap();
        drop(lease);

        // While the owner lives, nothing is reaped, so the next command still finds the tab.
        assert!(store.reap(&|_| true).is_empty());
        assert!(store.is_owned(&owned));

        // When the owner is gone the workspace goes with it, and hands back the tabs it held so
        // the caller's browser does not keep them forever.
        let abandoned = store.reap(&|_| false);
        assert_eq!(
            abandoned,
            vec![ReleasedTabs {
                browser: Some(TEST_BROWSER.into()),
                physical_ids: vec![41],
            }]
        );
        assert!(!store.is_owned(&owned));
        assert_eq!(store.summaries().len(), 0);
    }

    #[test]
    fn work_in_flight_is_never_reaped_underneath_itself() {
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100));
        let lease = store.acquire(&owned).unwrap();
        assert!(
            store.reap(&|_| false).is_empty(),
            "a leased workspace is mid-invocation; releasing it would pull the tabs out from under it"
        );
        drop(lease);
        assert_eq!(store.reap(&|_| false).len(), 0);
        assert!(!store.is_owned(&owned));
    }

    #[test]
    fn handles_are_owned_and_targets_expire_on_commit() {
        let store = WorkspaceStore::default();
        let first = store.admit("first".into(), IntakeChannel::Mcp);
        let second = store.admit("second".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&first).unwrap();
        let tab = lease.add_tab(&physical(1, "about:blank")).unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "button".into(),
                    name: "Go".into(),
                    state: vec![],
                    credential_class: false,
                }],
            )
            .unwrap();
        let handle = targets[0].0.as_str().to_owned();
        let resolved = lease.resolve_target(&handle, Some(&tab)).unwrap();
        assert_eq!(resolved.role, TargetRole::Button);
        let _new = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.org"))
            .unwrap();
        assert_eq!(
            lease.resolve_target(&handle, None).unwrap_err(),
            WorkspaceError::StaleTarget
        );
        drop(lease);
        let other = store.acquire(&second).unwrap();
        assert_eq!(
            other.select_tab(Some(tab.handle.as_str())).unwrap_err(),
            WorkspaceError::NotOwnedTab
        );
    }

    #[test]
    fn page_authored_roles_are_narrowed_before_target_state_is_stored() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(1, "https://example.com")).unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "Save my document".into(),
                    name: "untrusted name".into(),
                    state: vec![],
                    credential_class: false,
                }],
            )
            .unwrap();

        let resolved = lease
            .resolve_target(targets[0].0.as_str(), Some(&tab))
            .unwrap();
        assert_eq!(resolved.role, TargetRole::Control);
    }

    #[test]
    fn omission_selects_only_an_unambiguous_tab() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
        let first = lease.add_tab(&physical(1, "about:blank")).unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, first.handle);
        let second = lease.add_tab(&physical(2, "about:blank")).unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, second.handle);
    }

    #[test]
    fn decisive_close_receipt_tolerates_an_earlier_async_close_event() {
        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(7, "about:blank")).unwrap();
        store.apply_browser_close(TEST_BROWSER, 7);
        assert!(lease.confirm_tab_closed(&tab.handle).is_ok());
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
    }

    #[test]
    fn screenshot_views_map_coordinates_and_expire_on_commit() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(7, "about:blank")).unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        let geometry = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 10.0,
            page_y: 20.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 10.0,
            visual_page_y: 20.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 2.0,
            zoom: 1.0,
            output_scale: 0.5,
        };
        let view = lease.register_view(&tab, geometry, 400, 300).unwrap();
        let (_, point) = lease
            .resolve_view_point(view.as_str(), Some(&tab), 100.0, 50.0)
            .unwrap();
        assert_eq!(point.x, 210.0);
        assert_eq!(point.y, 120.0);
        assert_eq!(
            lease
                .resolve_view_point(view.as_str(), Some(&tab), 401.0, 1.0)
                .unwrap_err(),
            WorkspaceError::ViewPointOutOfBounds
        );
        let (_, region) = lease
            .resolve_view_region(view.as_str(), Some(&tab), 100.0, 50.0, 200.0, 100.0)
            .unwrap();
        assert_eq!(region.x, 210.0);
        assert_eq!(region.y, 120.0);
        assert_eq!(region.width, 400.0);
        assert_eq!(region.height, 200.0);
        assert_eq!(
            lease
                .resolve_view_region(view.as_str(), Some(&tab), 300.0, 0.0, 101.0, 10.0)
                .unwrap_err(),
            WorkspaceError::ViewRegionOutOfBounds
        );
        let magnified = lease
            .register_view(
                &tab,
                ViewportGeometry {
                    scope: CaptureScope::Region,
                    page_x: region.x,
                    page_y: region.y,
                    css_width: region.width,
                    css_height: region.height,
                    output_scale: 4.0,
                    ..geometry
                },
                1600,
                800,
            )
            .unwrap();
        assert_eq!(
            lease
                .resolve_view_region(view.as_str(), None, 0.0, 0.0, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
        let (_, chained) = lease
            .resolve_view_region(magnified.as_str(), Some(&tab), 400.0, 200.0, 400.0, 200.0)
            .unwrap();
        assert_eq!(chained.x, 310.0);
        assert_eq!(chained.y, 170.0);
        assert_eq!(chained.width, 100.0);
        assert_eq!(chained.height, 50.0);
        let _ = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.org"))
            .unwrap();
        assert_eq!(
            lease
                .resolve_view_point(magnified.as_str(), None, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
    }

    #[test]
    fn child_tabs_are_adopted_only_through_an_owned_opener() {
        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let lease = store.acquire(&workspace).unwrap();
        let _ = lease.add_tab(&physical(7, "about:blank")).unwrap();
        assert!(store
            .apply_browser_child(TEST_BROWSER, 7, &physical(8, "about:blank"))
            .is_some());
        assert!(store
            .apply_browser_child(TEST_BROWSER, 99, &physical(9, "about:blank"))
            .is_none());
        assert_eq!(lease.tabs().unwrap().len(), 2);
    }
}
