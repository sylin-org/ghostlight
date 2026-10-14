//! The workspace aggregate, opaque handles, ownership, document generations, and leases.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    BrowserReadiness, ObservedTarget, PhysicalPoint, PhysicalRectangle, PhysicalTab,
    ViewportGeometry,
};

use crate::language::outcome::TargetRole;
use crate::provenance::{ConnectionDetails, ConnectionEvidence};
use ghostlight_bridge::service::{IntakeChannel, SessionMarker};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

/// Immutable user-facing summary of one admitted workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    /// Opaque workspace identity.
    pub id: String,
    /// Presentation-only client label, claimed by the edge.
    pub client_label: String,
    /// Current connections, each with its own immutable observed and claimed evidence.
    pub connections: Vec<ConnectionDetails>,
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

/// What one document-tree snapshot covered: its optional subtree root and its depth.
///
/// Two snapshots are comparable only when they cover the same thing. A shallower or differently
/// rooted tree differs from the last one without the page having changed at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotScope {
    /// The target handle the tree was rooted at, or none for the whole document.
    pub root: Option<String>,
    /// How many levels the tree was allowed to reach.
    pub max_depth: usize,
}

/// One stored semantic document-tree snapshot for a controlled tab.
#[derive(Clone, Debug)]
pub struct SnapshotState {
    /// Owning controlled tab.
    pub tab: TabHandle,
    /// Document generation observed.
    pub generation: u64,
    /// Root and depth the tree covered.
    pub scope: SnapshotScope,
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

/// What a stale target handle still knows, offered so its refusal can carry fresh candidates.
#[derive(Clone, Debug)]
pub struct StaleTargetContext {
    /// Physical tab the control lived on; still alive in this context.
    pub physical_id: u64,
    /// Last governed URL of that tab.
    pub url: String,
    /// Bounded accessible name the page gave the control.
    pub name: String,
    /// Page-authored role string for re-querying.
    pub role_page: String,
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

/// One tab a released workspace left open, claimed by the next open on its host from a session of
/// the same client.
///
/// Only Ghostlight's own released tabs become these. A tab the person opened never does, so
/// default reuse cannot take it over (ADR-0196 Decision 9).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleasedTab {
    /// Physical tab id inside the browser the claim named.
    pub physical_id: u64,
    /// The governed URL the tab held when its workspace released it.
    pub url: String,
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
    /// Page-authored role string, retained so a stale handle can be re-queried by what the
    /// page itself called this control.
    role_page: String,
    /// Bounded accessible name, retained for the same re-query purpose.
    name: String,
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
    connections: HashMap<String, Arc<ConnectionEvidence>>,
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
    /// Tabs released workspaces left behind, per browser, keyed by physical id.
    ///
    /// This is the only pool default reuse adopts from. A tab enters it at release but can be
    /// claimed only after its release close answered that the person's preserve-tabs setting kept
    /// it open, and only by a session of the client that released it. An entry leaves it when the
    /// tab closes, when any page commits in it (the person or the page took it back), when a
    /// workspace takes it, when its release close did anything but keep it, and when its browser
    /// disconnects, because a restarted browser reuses tab ids.
    released: HashMap<String, HashMap<u64, PooledTab>>,
}

/// One tab in the released pool.
#[derive(Debug)]
struct PooledTab {
    /// The governed URL the tab held when its workspace released it.
    url: String,
    /// The client label of the workspace that released the tab.
    ///
    /// Every controlled tab sits in the browser group named for its workspace's client label, and
    /// a reused tab is navigated where it is, never regrouped. So only a session of the same
    /// client may claim it; any other session would control a tab shown under another client's
    /// group (ADR-0066).
    client_label: String,
    /// Whether the release close answered that the browser keeps the tab open.
    ///
    /// Until it does, the browser may still bind the tab to the dead workspace, and that pending
    /// close could still forget, unretain, or close the tab under whoever took it. So a tab whose
    /// cleanup has not answered is never claimed (ADR-0196 Decision 8).
    kept_open: bool,
}

impl AggregateState {
    /// Record the tabs a leaving workspace still holds as waiting for their release cleanup.
    ///
    /// None of them is reusable yet: only a cleanup answer that the browser kept a tab open
    /// (`settle_released`) offers it to the next open on its host from a session of the same
    /// client. A held tab sits on a destination governance refused, and a tab with no governed URL
    /// never landed, so neither is recorded at all.
    fn remember_released(&mut self, workspace: &WorkspaceState) {
        let Some(browser) = workspace.browser.as_ref() else {
            return;
        };
        let tabs: Vec<_> = workspace
            .tabs
            .values()
            .filter(|tab| !tab.held && !tab.url.is_empty())
            .map(|tab| {
                (
                    tab.physical_id,
                    PooledTab {
                        url: tab.url.clone(),
                        client_label: workspace.client_label.clone(),
                        kept_open: false,
                    },
                )
            })
            .collect();
        if !tabs.is_empty() {
            self.released
                .entry(browser.clone())
                .or_default()
                .extend(tabs);
        }
    }

    /// Drop one tab from the released pool of its browser.
    fn forget_released(&mut self, browser: &str, physical_id: u64) {
        if let Some(pool) = self.released.get_mut(browser) {
            pool.remove(&physical_id);
            if pool.is_empty() {
                self.released.remove(browser);
            }
        }
    }

    /// Whether a workspace other than `current`, working in the same browser, controls the tab.
    ///
    /// Physical tab ids are unique inside one browser and nowhere else: Chrome's tab 5 and Edge's
    /// tab 5 are different tabs and must not refuse each other.
    fn controlled_elsewhere(&self, current: &WorkspaceId, physical_id: u64) -> bool {
        let browser = self
            .workspaces
            .get(current)
            .and_then(|workspace| workspace.browser.as_deref());
        self.workspaces.iter().any(|(id, workspace)| {
            id != current
                && workspace.browser.as_deref() == browser
                && workspace
                    .tabs
                    .values()
                    .any(|known| known.physical_id == physical_id)
        })
    }

    /// The browser a workspace works in, if its first work chose one.
    fn browser_of(&self, workspace: &WorkspaceId) -> Option<String> {
        self.workspaces
            .get(workspace)
            .and_then(|state| state.browser.clone())
    }
}

/// The part of an ordinary web address that default reuse compares: host and explicit port.
///
/// Only `http` and `https` pages are reuse candidates; anything else has no comparable site.
fn reusable_site(url: &str) -> Option<(String, Option<u16>)> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some((parsed.host_str()?.to_ascii_lowercase(), parsed.port()))
}

/// Thread-safe entry to the single workspace aggregate.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceStore {
    inner: Arc<Mutex<AggregateState>>,
    leased: Arc<Condvar>,
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
                connections: {
                    let mut details: Vec<_> = workspace
                        .connections
                        .values()
                        .map(|connection| connection.details())
                        .collect();
                    details.sort_by(|a, b| a.connection_id.cmp(&b.connection_id));
                    details
                },
                channel: workspace.channel,
                leased: workspace.leased,
                tab_count: workspace.tabs.len(),
                held_tab_count: workspace.tabs.values().filter(|tab| tab.held).count(),
            })
            .collect();
        summaries.sort_by(|left, right| left.id.cmp(&right.id));
        summaries
    }

    /// Attach one actual connection, resuming only workspace continuity when an owner is supplied.
    /// The connection's evidence stays independent of every other connection in that workspace.
    pub fn connect(
        &self,
        connection: Arc<ConnectionEvidence>,
        session: Option<SessionMarker>,
    ) -> WorkspaceId {
        let mut aggregate = self.lock();
        let key = session.as_ref().map(SessionMarker::key);
        let existing = key.and_then(|key| {
            aggregate.workspaces.iter_mut().find(|(_, state)| {
                state
                    .session
                    .as_ref()
                    .is_some_and(|owner| owner.key() == key)
            })
        });
        let connection_id = connection
            .attribution()
            .connection_id
            .clone()
            .expect("a new connection has an id");
        if let Some((id, state)) = existing {
            state.connections.insert(connection_id, connection);
            return id.clone();
        }
        let id = WorkspaceId(format!("workspace_{}", Uuid::new_v4().simple()));
        aggregate.workspaces.insert(
            id.clone(),
            WorkspaceState {
                client_label: connection.reported_application().into(),
                channel: connection.attribution().channel,
                connections: HashMap::from([(connection_id, connection)]),
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

    /// Remove only the closing connection's live details; receipts retain their own snapshots.
    pub fn disconnect(&self, workspace: &WorkspaceId, connection_id: &str) {
        if let Some(state) = self.lock().workspaces.get_mut(workspace) {
            state.connections.remove(connection_id);
        }
    }

    /// Return evidence only when an in-process caller has exactly one possible connection.
    pub fn single_connection(&self, workspace: &WorkspaceId) -> Option<Arc<ConnectionEvidence>> {
        let state = self.lock();
        let connections = &state.workspaces.get(workspace)?.connections;
        (connections.len() == 1).then(|| connections.values().next().unwrap().clone())
    }

    #[cfg(test)]
    pub(crate) fn admit(
        &self,
        label: String,
        channel: IntakeChannel,
        peer_image: Option<String>,
    ) -> WorkspaceId {
        let peer = peer_image.map_or(
            crate::provenance::PeerObservation::NotRecorded,
            |executable| crate::provenance::PeerObservation::Observed { executable },
        );
        self.connect(
            Arc::new(ConnectionEvidence::with_peer(&label, channel, peer)),
            None,
        )
    }

    #[cfg(test)]
    pub(crate) fn resume_or_admit(
        &self,
        label: String,
        channel: IntakeChannel,
        marker: SessionMarker,
        peer_image: Option<String>,
    ) -> WorkspaceId {
        let peer = peer_image.map_or(
            crate::provenance::PeerObservation::NotRecorded,
            |executable| crate::provenance::PeerObservation::Observed { executable },
        );
        self.connect(
            Arc::new(ConnectionEvidence::with_peer(&label, channel, peer)),
            Some(marker),
        )
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
                state.remember_released(&workspace);
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

    /// Release an MCP workspace and return the physical tabs it owned.
    ///
    /// The tabs wait in the released pool for their cleanup. A tab becomes reusable by the next
    /// open on its host from a session of the same client only once `settle_released` reports
    /// that the browser kept it open (the person's preserve-tabs choice).
    pub fn release(&self, workspace: &WorkspaceId) -> ReleasedTabs {
        let mut state = self.lock();
        let Some(removed) = state.workspaces.remove(workspace) else {
            return ReleasedTabs::default();
        };
        state.remember_released(&removed);
        ReleasedTabs::from_state(removed)
    }

    /// Settle one released tab once its release close in `browser` has answered.
    ///
    /// `kept_open` is true only when the person's preserve-tabs setting refused the close. The
    /// browser has then dropped the dead workspace's binding and debugger hold and left the tab
    /// open, so the tab becomes reusable. Any other answer, a close that worked, a failure, or no
    /// answer in time, removes the tab, so a tab that closed or may still be closing is never
    /// claimed. A tab that already left the pool, because it closed or a page committed in it
    /// while its cleanup ran, stays out.
    pub fn settle_released(&self, browser: &str, physical_id: u64, kept_open: bool) {
        let mut state = self.lock();
        if !kept_open {
            state.forget_released(browser, physical_id);
            return;
        }
        if let Some(tab) = state
            .released
            .get_mut(browser)
            .and_then(|pool| pool.get_mut(&physical_id))
        {
            tab.kept_open = true;
        }
    }

    /// Take the released tab an open of `url` in `browser` by a session of `client_label` should
    /// reuse, if there is one.
    ///
    /// Candidates are tabs Ghostlight opened, a released workspace of the same client label left
    /// behind, and the browser kept open when that workspace's cleanup asked to close them, in
    /// this browser, on the same host and port. The label must match because a reused tab stays in
    /// the group it already sits in, and that group is named for the client that released it. An
    /// exact URL wins, then the lowest tab id. The claim removes the tab from the pool under the
    /// aggregate lock, so two sessions can never claim the same tab, and leaves another client's
    /// tabs in the pool. The caller still checks the live browser before using it.
    #[must_use]
    pub fn claim_released(
        &self,
        browser: &str,
        client_label: &str,
        url: &str,
    ) -> Option<ReleasedTab> {
        let site = reusable_site(url)?;
        let mut state = self.lock();
        let claimed = state
            .released
            .get(browser)?
            .iter()
            .filter(|(_, released)| {
                released.kept_open
                    && released.client_label == client_label
                    && reusable_site(&released.url).as_ref() == Some(&site)
            })
            .min_by_key(|(physical_id, released)| (released.url != url, **physical_id))
            .map(|(physical_id, released)| ReleasedTab {
                physical_id: *physical_id,
                url: released.url.clone(),
            })?;
        state.forget_released(browser, claimed.physical_id);
        Some(claimed)
    }

    /// Forget every released tab of a browser that disconnected.
    ///
    /// A browser that comes back may be a restarted one, and a restarted browser hands out tab ids
    /// again, so a remembered id could name a tab the person just opened.
    pub fn apply_browser_disconnect(&self, browser: &str) {
        self.lock().released.remove(browser);
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

    /// Acquire exclusive mutation ownership for one invocation, waiting until available or deadline.
    pub fn acquire_until(
        &self,
        workspace: &WorkspaceId,
        deadline: Instant,
        cancellation: &crate::work::CancellationToken,
    ) -> Result<WorkspaceLease, WorkspaceError> {
        let mut state = self.lock();
        loop {
            if cancellation.is_cancelled() {
                return Err(WorkspaceError::Busy);
            }
            let workspace_state = state
                .workspaces
                .get_mut(workspace)
                .ok_or(WorkspaceError::UnknownWorkspace)?;
            if !workspace_state.leased {
                workspace_state.leased = true;
                return Ok(WorkspaceLease {
                    store: self.clone(),
                    workspace: workspace.clone(),
                    released: false,
                });
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(WorkspaceError::Busy);
            }
            let wait = deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(50));
            let (next_state, _) = self
                .leased
                .wait_timeout(state, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state = next_state;
        }
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

    /// The client label of another admitted workspace that owns the requested tab handle.
    ///
    /// Returns None when no other workspace owns the handle, or when its owner has no client
    /// label: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn tab_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TabHandle(requested.into());
        self.labeled_owner(current, |workspace| workspace.tabs.contains_key(&handle))
    }

    /// The client label of another admitted workspace that owns the requested target handle.
    ///
    /// Returns None when no other workspace owns the handle, or when its owner has no client
    /// label: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn target_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TargetHandle(requested.into());
        self.labeled_owner(current, |workspace| workspace.targets.contains_key(&handle))
    }

    /// Name the other workspace that holds a handle, by its client label only.
    ///
    /// A workspace id is an internal key. No tool takes one, so naming it would only tell the
    /// caller where another session lives (ADR-0196 Decision 8). An owner without a label is
    /// therefore not attributed at all, and the refusal falls back to the generic sentence
    /// (ADR-0172 Decision 4).
    fn labeled_owner(
        &self,
        current: &WorkspaceId,
        holds: impl Fn(&WorkspaceState) -> bool,
    ) -> Option<String> {
        self.lock()
            .workspaces
            .iter()
            .find(|(id, workspace)| *id != current && holds(workspace))
            .map(|(_, workspace)| workspace.client_label.clone())
            .filter(|label| !label.is_empty())
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

    /// The physical ids of the tabs this workspace controls, in ascending order.
    #[must_use]
    pub fn physical_tabs(&self, workspace: &str) -> Vec<u64> {
        let mut tabs: Vec<u64> = self
            .lock()
            .workspaces
            .get(workspace)
            .map(|state| state.tabs.values().map(|tab| tab.physical_id).collect())
            .unwrap_or_default();
        tabs.sort_unstable();
        tabs
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

    /// Invalidate agent references after an unrelated page change, without governing browsing.
    ///
    /// A commit in a released tab means the person or the page took it back, since no Ghostlight
    /// work runs there, so the tab also leaves the released pool and is never reused.
    pub fn note_browser_landing(
        &self,
        browser: &str,
        physical_id: u64,
        url: &str,
    ) -> Option<(WorkspaceId, TabHandle)> {
        let mut state = self.lock();
        state.forget_released(browser, physical_id);
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
                tab.held = false;
                tab.url = url.into();
                tab.title.clear();
                return Some((workspace_id.clone(), handle.clone()));
            }
        }
        None
    }

    /// Resolve the owning workspace for an asynchronous physical browser event.
    ///
    /// A physical tab id is unique inside one browser and nowhere else, so the browser that
    /// reported the event is part of the key. Without it, Chrome's tab 5 and Edge's tab 5 are the
    /// same lookup, and one browser's state could change the other's cached references.
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

    /// Remove a tab closed outside an invocation, including from the released pool.
    pub fn apply_browser_close(&self, browser: &str, physical_id: u64) {
        let mut state = self.lock();
        state.forget_released(browser, physical_id);
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

    /// The client label of another admitted workspace that owns the requested tab handle, if it
    /// has one.
    #[must_use]
    pub fn tab_owner(&self, requested: &str) -> Option<String> {
        self.store.tab_owner(&self.workspace, requested)
    }

    /// The client label of another admitted workspace that owns the requested target handle, if
    /// it has one.
    #[must_use]
    pub fn target_owner(&self, requested: &str) -> Option<String> {
        self.store.target_owner(&self.workspace, requested)
    }

    /// Add a physical tab under a new opaque handle.
    ///
    /// The tab is refused when another workspace in the same browser already controls it. Once
    /// added it is this workspace's tab, so it leaves the released pool.
    pub fn add_tab(&self, tab: &PhysicalTab) -> Result<SelectedTab, WorkspaceError> {
        let mut state = self.store.lock();
        if state.controlled_elsewhere(&self.workspace, tab.tab_id) {
            return Err(WorkspaceError::PhysicalTabOwned);
        }
        if let Some(browser) = state.browser_of(&self.workspace) {
            state.forget_released(&browser, tab.tab_id);
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

    /// Follow a governed same-document address change without starting a new document.
    ///
    /// An anchor link or a `history.pushState` route keeps the document, so the generation and
    /// every target handle stay valid (ADR-0195 Decision 3). The tab's URL and title follow the
    /// page. Views expire, because the rendered page may have moved under their coordinates.
    pub fn apply_same_document(
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
        known.url.clone_from(&tab.url);
        known.title = bounded(&tab.title, 500);
        known.readiness = tab.readiness;
        known.active = tab.active;
        let current = selected(handle, known);
        workspace.views.retain(|_, view| &view.tab != handle);
        Ok(current)
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

    /// Store one semantic tree for a tab, superseding any earlier snapshot of that tab.
    pub fn register_snapshot(
        &self,
        tab: &SelectedTab,
        scope: SnapshotScope,
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
                scope,
                tree,
            },
        );
        Ok(handle)
    }

    /// Return the current-generation tree for a tab, if one is recorded for the same scope.
    #[must_use]
    pub fn previous_snapshot(
        &self,
        tab: &SelectedTab,
        scope: &SnapshotScope,
    ) -> Option<serde_json::Value> {
        let state = self.store.lock();
        let workspace = state.workspaces.get(&self.workspace)?;
        let known = workspace.tabs.get(&tab.handle)?;
        if known.generation != tab.generation {
            return None;
        }
        workspace
            .snapshots
            .values()
            .find(|snapshot| {
                snapshot.tab == tab.handle
                    && snapshot.generation == known.generation
                    && &snapshot.scope == scope
            })
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

    /// Confirm decisive physical closure, tolerating its earlier asynchronous close event.
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

    /// Recreate one dead tab binding under its existing durable handle.
    ///
    /// Continuation, not resurrection: the new physical tab starts a fresh document with no
    /// history, and the caller's handle keeps referring to "this workspace's tab slot".
    pub fn restore_tab(
        &self,
        handle: &str,
        tab: &PhysicalTab,
    ) -> Result<SelectedTab, WorkspaceError> {
        let handle = TabHandle(handle.into());
        let mut state = self.store.lock();
        let held = state
            .workspaces
            .get(&self.workspace)
            .and_then(|workspace| workspace.tabs.get(&handle))
            .map(|dead| dead.held)
            .unwrap_or(false);
        if state.controlled_elsewhere(&self.workspace, tab.tab_id) {
            return Err(WorkspaceError::PhysicalTabOwned);
        }
        if state
            .workspaces
            .iter()
            .any(|(id, candidate)| id != &self.workspace && candidate.tabs.contains_key(&handle))
        {
            return Err(WorkspaceError::NotOwnedTab);
        }
        if let Some(browser) = state.browser_of(&self.workspace) {
            state.forget_released(&browser, tab.tab_id);
        }
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        for known in workspace.tabs.values_mut() {
            known.active = false;
        }
        let value = TabState {
            physical_id: tab.tab_id,
            generation: 0,
            url: tab.url.clone(),
            title: tab.title.clone(),
            readiness: tab.readiness,
            active: true,
            held,
        };
        workspace.tabs.insert(handle.clone(), value.clone());
        Ok(selected(&handle, &value))
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
                    role_page: target.role.clone(),
                    name: target.name.clone(),
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

    /// Last-known context for a target handle that no longer resolves, so a refusal can offer
    /// fresh candidates from the live page. `None` when the handle never belonged to this
    /// workspace or its whole tab is gone.
    ///
    /// The context is deliberately narrow: the physical tab to re-query, the governed URL it
    /// last had, and what the page itself called the control. No page content travels.
    pub fn stale_target_context(&self, requested: &str) -> Option<StaleTargetContext> {
        let state = self.store.lock();
        let workspace = state.workspaces.get(&self.workspace)?;
        let target = workspace.targets.get(&TargetHandle(requested.into()))?;
        let tab = workspace.tabs.get(&target.tab)?;
        Some(StaleTargetContext {
            physical_id: tab.physical_id,
            url: tab.url.clone(),
            name: target.name.clone(),
            role_page: target.role_page.clone(),
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
        self.store.leased.notify_all();
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
    use crate::browser::adapter::{
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
        let workspace = store.admit("test".into(), IntakeChannel::Mcp, None);
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
    fn connection_details_stay_plural_and_detach_only_the_closing_peer() {
        use crate::provenance::{ConnectionEvidence, PeerObservation};
        use std::sync::Arc;
        let store = WorkspaceStore::default();
        let first = Arc::new(ConnectionEvidence::with_peer(
            "first",
            IntakeChannel::Mcp,
            PeerObservation::Unavailable,
        ));
        let second = Arc::new(ConnectionEvidence::with_peer(
            "second",
            IntakeChannel::Cli,
            PeerObservation::UnsupportedPlatform,
        ));
        let owner = SessionMarker::Declared {
            key: "shared".into(),
        };
        let workspace = store.connect(first.clone(), Some(owner.clone()));
        assert_eq!(store.connect(second.clone(), Some(owner)), workspace);
        assert_eq!(store.summaries()[0].connections.len(), 2);
        assert!(store.single_connection(&workspace).is_none());
        store.disconnect(
            &workspace,
            first.attribution().connection_id.as_deref().unwrap(),
        );
        assert_eq!(store.summaries()[0].connections, vec![second.details()]);
        assert_eq!(
            store.single_connection(&workspace).unwrap().attribution(),
            second.attribution()
        );
        assert_eq!(
            first.details().reported_application.as_deref(),
            Some("first")
        );
    }

    #[test]
    fn one_caller_resumes_its_own_workspace_and_a_recycled_pid_does_not() {
        let store = WorkspaceStore::default();
        let first =
            store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100), None);
        let again =
            store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100), None);
        assert_eq!(
            first, again,
            "the same caller must reach the same workspace"
        );

        // The negative control, and the reason identity is not pid plus name: a recycled pid
        // running the same program must not inherit the dead session's tabs.
        let recycled =
            store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 200), None);
        assert_ne!(first, recycled);

        // A connection with no marker is bound to that connection, as the MCP edge expects.
        let bound = store.admit("codex".into(), IntakeChannel::Mcp, None);
        assert!(!store.is_owned(&bound));
        assert!(store.is_owned(&first));
    }

    #[test]
    fn a_workspace_outlives_its_connection_and_dies_with_its_owner() {
        let store = WorkspaceStore::default();
        let owned =
            store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100), None);
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
        let owned =
            store.resume_or_admit("shell".into(), IntakeChannel::Cli, marker(4312, 100), None);
        let lease = store.acquire(&owned).unwrap();
        assert!(
            store.reap(&|_| false).is_empty(),
            "a leased workspace is mid-invocation; releasing it would pull the tabs out from under it"
        );
        drop(lease);
        assert_eq!(store.reap(&|_| false).len(), 0);
        assert!(!store.is_owned(&owned));
    }

    /// The client label the pool tests release and claim under unless a test names another.
    const TEST_CLIENT: &str = "test";

    /// Admit a workspace of `TEST_CLIENT` in `browser` and land one tab per `(id, url)` in it.
    fn workspace_with_tabs(
        store: &WorkspaceStore,
        browser: &str,
        tabs: &[(u64, &str)],
    ) -> WorkspaceId {
        client_workspace_with_tabs(store, TEST_CLIENT, browser, tabs)
    }

    /// Admit a workspace of `client` in `browser` and land one tab per `(id, url)` in it.
    fn client_workspace_with_tabs(
        store: &WorkspaceStore,
        client: &str,
        browser: &str,
        tabs: &[(u64, &str)],
    ) -> WorkspaceId {
        let workspace = store.admit(client.into(), IntakeChannel::Mcp, None);
        store.pin_browser(workspace.as_str(), browser).unwrap();
        let lease = store.acquire(&workspace).unwrap();
        for (id, url) in tabs {
            let tab = lease.add_tab(&physical(*id, url)).unwrap();
            lease
                .apply_landing(&tab.handle, &physical(*id, url))
                .unwrap();
        }
        workspace
    }

    /// The tab a `TEST_CLIENT` session opening `url` in `browser` claims, if any.
    fn claimed(store: &WorkspaceStore, browser: &str, url: &str) -> Option<u64> {
        claimed_by(store, browser, TEST_CLIENT, url)
    }

    /// The tab a session of `client` opening `url` in `browser` claims, if any.
    fn claimed_by(store: &WorkspaceStore, browser: &str, client: &str, url: &str) -> Option<u64> {
        store
            .claim_released(browser, client, url)
            .map(|released| released.physical_id)
    }

    /// Answer every release close as the person's preserve-tabs setting would: kept open.
    fn kept_open(store: &WorkspaceStore, released: &ReleasedTabs) {
        let browser = released.browser.as_deref().unwrap();
        for &physical_id in &released.physical_ids {
            store.settle_released(browser, physical_id, true);
        }
    }

    #[test]
    fn a_released_tab_is_claimable_only_after_its_close_kept_it_open() {
        let url = "https://example.com/";
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(1, url), (2, url), (3, url), (4, url)],
        );
        store.release(&workspace);
        assert_eq!(
            claimed(&store, TEST_BROWSER, url),
            None,
            "a tab whose release close has not answered is never claimed"
        );

        // Tab 1 closed. Tab 2 closed and tab 3 loaded a page while their cleanup ran, so a later
        // kept-open answer does not bring them back. An answer from another browser names
        // another tab 4.
        store.settle_released(TEST_BROWSER, 1, false);
        store.apply_browser_close(TEST_BROWSER, 2);
        store.settle_released(TEST_BROWSER, 2, true);
        store.note_browser_landing(TEST_BROWSER, 3, "https://example.com/mine");
        store.settle_released(TEST_BROWSER, 3, true);
        store.settle_released("browser_other", 4, true);
        assert_eq!(claimed(&store, TEST_BROWSER, url), None);

        store.settle_released(TEST_BROWSER, 4, true);
        assert_eq!(claimed(&store, TEST_BROWSER, url), Some(4));
        assert_eq!(claimed(&store, TEST_BROWSER, url), None);
    }

    #[test]
    fn released_tabs_are_claimed_by_host_exact_url_first_then_lowest_id() {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[
                (9, "https://example.com/a"),
                (4, "https://example.com/b"),
                (6, "https://example.com:8443/c"),
                (2, "https://other.test/"),
            ],
        );
        kept_open(&store, &store.release(&workspace));

        assert_eq!(
            claimed(&store, "browser_other", "https://example.com/a"),
            None
        );
        assert_eq!(claimed(&store, TEST_BROWSER, "file:///C:/a"), None);
        assert_eq!(
            store.claim_released(TEST_BROWSER, TEST_CLIENT, "https://example.com/a"),
            Some(super::ReleasedTab {
                physical_id: 9,
                url: "https://example.com/a".into(),
            }),
            "an exact URL wins over a lower id"
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com/a"),
            Some(4)
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com/a"),
            None,
            "a different port is a different site"
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com:8443/"),
            Some(6)
        );
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://OTHER.test/x"),
            Some(2)
        );
    }

    #[test]
    fn a_released_tab_is_claimed_only_by_a_session_of_the_client_that_released_it() {
        const SCRIPTS: &str = "ghostlight call";
        const AGENT: &str = "Claude Code";
        let store = WorkspaceStore::default();
        let scripts = client_workspace_with_tabs(
            &store,
            SCRIPTS,
            TEST_BROWSER,
            &[(3, "https://example.com/")],
        );
        kept_open(&store, &store.release(&scripts));
        let agent = client_workspace_with_tabs(
            &store,
            AGENT,
            TEST_BROWSER,
            &[(7, "https://example.com/other")],
        );
        kept_open(&store, &store.release(&agent));

        // Tab 3 sits in the "Ghostlight - ghostlight call" group. An exact URL and a lower id do
        // not make it the agent's: the agent gets the tab its own client released.
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, AGENT, "https://example.com/"),
            Some(7)
        );
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, AGENT, "https://example.com/"),
            None,
            "another client's tab is never the agent's to reuse"
        );
        // Asking left the scripts' tab in the pool for their next session.
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, SCRIPTS, "https://example.com/"),
            Some(3)
        );
    }

    #[test]
    fn released_tabs_leave_the_pool_when_closed_committed_taken_or_disconnected() {
        const SHELL: &str = "shell";
        let store = WorkspaceStore::default();
        let owned =
            store.resume_or_admit(SHELL.into(), IntakeChannel::Cli, marker(4312, 100), None);
        store.pin_browser(owned.as_str(), TEST_BROWSER).unwrap();
        let lease = store.acquire(&owned).unwrap();
        for id in [1, 2, 3, 4, 5] {
            let tab = lease
                .add_tab(&physical(id, "https://example.com/"))
                .unwrap();
            lease
                .apply_landing(&tab.handle, &physical(id, "https://example.com/"))
                .unwrap();
        }
        // A held tab sits on a refused destination and is never offered.
        let held = lease
            .tabs()
            .unwrap()
            .into_iter()
            .find(|tab| tab.physical_id == 5)
            .unwrap();
        lease.hold_tab(&held.handle).unwrap();
        drop(lease);
        // The reaper releases the workspace of a caller that is gone, and the browser keeps every
        // tab open. The held tab still never joins the pool.
        let reaped = store.reap(&|_| false);
        assert_eq!(reaped.len(), 1);
        kept_open(&store, &reaped[0]);

        store.apply_browser_close(TEST_BROWSER, 1);
        store.note_browser_landing(TEST_BROWSER, 2, "https://example.com/next");
        let taker = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        store
            .acquire(&taker)
            .unwrap()
            .add_tab(&physical(3, "https://example.com/"))
            .unwrap();
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, SHELL, "https://example.com/"),
            Some(4)
        );
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, SHELL, "https://example.com/"),
            None
        );

        let other = workspace_with_tabs(&store, TEST_BROWSER, &[(8, "https://example.com/")]);
        kept_open(&store, &store.release(&other));
        store.apply_browser_disconnect(TEST_BROWSER);
        assert_eq!(
            claimed(&store, TEST_BROWSER, "https://example.com/"),
            None,
            "a reconnected browser may have reused the id"
        );
    }

    #[test]
    fn the_same_tab_id_in_two_browsers_is_two_tabs() {
        let store = WorkspaceStore::default();
        let _chrome = workspace_with_tabs(&store, "browser_chrome", &[(5, "https://a.test/")]);
        let edge = workspace_with_tabs(&store, "browser_edge", &[]);
        let edge_lease = store.acquire(&edge).unwrap();
        assert!(edge_lease.add_tab(&physical(5, "https://b.test/")).is_ok());
        drop(edge_lease);

        let second_chrome = workspace_with_tabs(&store, "browser_chrome", &[]);
        let lease = store.acquire(&second_chrome).unwrap();
        assert_eq!(
            lease.add_tab(&physical(5, "https://a.test/")).unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
        assert_eq!(
            lease
                .restore_tab("tab_restored_too", &physical(5, "https://a.test/"))
                .unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
    }

    #[test]
    fn a_same_document_change_keeps_targets_and_expires_views() {
        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(7, "about:blank")).unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com/"))
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
        let geometry = ViewportGeometry {
            scope: CaptureScope::Viewport,
            page_x: 0.0,
            page_y: 0.0,
            css_width: 800.0,
            css_height: 600.0,
            visual_page_x: 0.0,
            visual_page_y: 0.0,
            visual_css_width: 800.0,
            visual_css_height: 600.0,
            device_scale: 1.0,
            zoom: 1.0,
            output_scale: 1.0,
        };
        let view = lease.register_view(&tab, geometry, 800, 600).unwrap();

        let routed = lease
            .apply_same_document(&tab.handle, &physical(7, "https://example.com/#next"))
            .unwrap();
        assert_eq!(routed.generation, tab.generation);
        assert_eq!(routed.url, "https://example.com/#next");
        assert!(lease.resolve_target(targets[0].0.as_str(), None).is_ok());
        assert_eq!(
            lease
                .resolve_view_point(view.as_str(), None, 1.0, 1.0)
                .unwrap_err(),
            WorkspaceError::StaleView
        );
        assert_eq!(
            lease
                .apply_same_document(&tab.handle, &physical(8, "https://example.com/"))
                .unwrap_err(),
            WorkspaceError::StaleTab
        );
    }

    #[test]
    fn handles_are_owned_and_targets_expire_on_commit() {
        let store = WorkspaceStore::default();
        let first = store.admit("first".into(), IntakeChannel::Mcp, None);
        let second = store.admit("second".into(), IntakeChannel::Mcp, None);
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
    fn another_workspace_is_named_only_by_its_client_label() {
        let store = WorkspaceStore::default();
        let caller = store.admit("caller".into(), IntakeChannel::Mcp, None);
        let labeled = store.admit("Codex".into(), IntakeChannel::Mcp, None);
        let unlabeled = store.admit(String::new(), IntakeChannel::Mcp, None);
        let handles = |workspace: &WorkspaceId, physical_id: u64| {
            let lease = store.acquire(workspace).unwrap();
            let tab = lease
                .add_tab(&physical(physical_id, "https://example.com"))
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
            (
                tab.handle.as_str().to_owned(),
                targets[0].0.as_str().to_owned(),
            )
        };
        let (labeled_tab, labeled_target) = handles(&labeled, 1);
        let (unlabeled_tab, unlabeled_target) = handles(&unlabeled, 2);

        assert_eq!(
            store.tab_owner(&caller, &labeled_tab).as_deref(),
            Some("Codex")
        );
        assert_eq!(
            store.target_owner(&caller, &labeled_target).as_deref(),
            Some("Codex")
        );
        // An unlabeled owner is never named, so its workspace id cannot stand in for a label.
        assert_eq!(store.tab_owner(&caller, &unlabeled_tab), None);
        assert_eq!(store.target_owner(&caller, &unlabeled_target), None);
        // A workspace's own handle has no other owner to name.
        assert_eq!(store.tab_owner(&labeled, &labeled_tab), None);
        assert_eq!(store.target_owner(&labeled, &labeled_target), None);
    }

    #[test]
    fn page_authored_roles_are_narrowed_before_target_state_is_stored() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp, None);
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
        let workspace = store.admit("test".into(), IntakeChannel::Mcp, None);
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
        let workspace = store.admit("test".into(), IntakeChannel::Mcp, None);
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
    fn snapshots_supersede_per_tab_go_stale_on_commit_and_die_with_the_tab() {
        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(7, "about:blank")).unwrap();
        let whole = super::SnapshotScope {
            root: None,
            max_depth: 6,
        };
        let _first = lease
            .register_snapshot(&tab, whole.clone(), serde_json::json!({"n":1}))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&tab, &whole),
            Some(serde_json::json!({"n":1})),
            "the just-registered tree is the current one"
        );
        let _second = lease
            .register_snapshot(&tab, whole.clone(), serde_json::json!({"n":2}))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&tab, &whole),
            Some(serde_json::json!({"n":2})),
            "a new snapshot supersedes the earlier one per tab"
        );
        let shallower = super::SnapshotScope {
            root: None,
            max_depth: 4,
        };
        let rooted = super::SnapshotScope {
            root: Some("target_one".into()),
            max_depth: 6,
        };
        assert_eq!(
            lease.previous_snapshot(&tab, &shallower),
            None,
            "a tree of another depth is not comparable"
        );
        assert_eq!(
            lease.previous_snapshot(&tab, &rooted),
            None,
            "a tree of another root is not comparable"
        );
        let landed = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        assert_eq!(
            lease.previous_snapshot(&landed, &whole),
            None,
            "commit stales snapshots"
        );
        assert!(lease
            .register_snapshot(&landed, whole.clone(), serde_json::json!({}))
            .is_ok());
        lease.confirm_tab_closed(&landed.handle).unwrap();
        assert_eq!(
            lease.previous_snapshot(&landed, &whole),
            None,
            "closure removes snapshots"
        );
        let other = admit_in_browser(&store);
        let other_lease = store.acquire(&other).unwrap();
        let stranger = other_lease.add_tab(&physical(9, "about:blank")).unwrap();
        assert_eq!(
            other_lease.previous_snapshot(&stranger, &whole),
            None,
            "a foreign workspace never sees another workspace's snapshots"
        );
    }

    #[test]
    fn captured_images_supersede_respect_the_ceiling_and_expire() {
        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease.add_tab(&physical(7, "about:blank")).unwrap();
        let first = lease
            .register_image(&tab, "image/jpeg", "AAAA", 3)
            .unwrap()
            .expect("under the ceiling");
        assert_eq!(
            lease.take_image(first.as_str(), &tab),
            Some(("image/jpeg".into(), "AAAA".into()))
        );
        let _second = lease
            .register_image(&tab, "image/jpeg", "BBBB", 3)
            .unwrap()
            .expect("superseding capture");
        assert_eq!(
            lease.take_image(first.as_str(), &tab),
            None,
            "a superseded asset is erased, not served"
        );
        let oversized = lease
            .register_image(&tab, "image/jpeg", "QAAA", 6_000_000)
            .unwrap();
        assert!(
            oversized.is_none(),
            "captures above the upload ceiling are refused"
        );
        let landed = lease
            .apply_landing(&tab.handle, &physical(7, "https://example.com"))
            .unwrap();
        assert_eq!(
            lease.take_image(first.as_str(), &landed),
            None,
            "commit stales assets"
        );
    }

    #[test]
    fn acquire_until_synchronizes_contention_via_condvar() {
        use crate::work::CancellationToken;
        use std::thread;
        use std::time::{Duration, Instant};

        let store = WorkspaceStore::default();
        let workspace = admit_in_browser(&store);
        let first_lease = store.acquire(&workspace).unwrap();

        // 1. Contention with deadline expiry:
        let cancellation = CancellationToken::default();
        let short_deadline = Instant::now() + Duration::from_millis(30);
        let expired = store.acquire_until(&workspace, short_deadline, &cancellation);
        assert_eq!(expired.err(), Some(WorkspaceError::Busy));

        // 2. Contention with cancellation:
        let cancelled = CancellationToken::default();
        cancelled.cancel();
        let long_deadline = Instant::now() + Duration::from_secs(5);
        let aborted = store.acquire_until(&workspace, long_deadline, &cancelled);
        assert_eq!(aborted.err(), Some(WorkspaceError::Busy));

        // 3. Contention resolved by dropping the active lease:
        let store_clone = store.clone();
        let workspace_clone = workspace.clone();
        let waiter = thread::spawn(move || {
            let cancellation = CancellationToken::default();
            let deadline = Instant::now() + Duration::from_secs(5);
            store_clone.acquire_until(&workspace_clone, deadline, &cancellation)
        });

        // Give the waiter thread a moment to enter wait on Condvar
        thread::sleep(Duration::from_millis(20));

        // Drop the first lease, which notifies Condvar
        drop(first_lease);

        let acquired = waiter.join().unwrap();
        assert!(
            acquired.is_ok(),
            "waiter successfully acquired lease after notification"
        );
    }
}
