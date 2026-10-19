//! The workspace aggregate, opaque handles, ownership, document generations, and leases.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    BrowserReadiness, ObservedTarget, PhysicalPoint, PhysicalRectangle, PhysicalTab,
    ViewportGeometry,
};

use crate::intake::session::SessionMarker;
use crate::language::outcome::{DialogKind, TargetRole};
use crate::provenance::{ConnectionDetails, ConnectionEvidence};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

mod stale;

pub use stale::StaleReason;

/// How many random characters follow the prefix of a handle Ghostlight issues.
///
/// Eight base32 characters are 40 random bits. Handles are checked against the workspace that
/// owns them, so they only have to be unique there and hard to confuse with a handle from an
/// earlier session, which a short counter would not be.
const HANDLE_SUFFIX_CHARS: usize = 8;

/// The characters a handle's random part is drawn from: lowercase RFC 4648 base32.
const HANDLE_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// How many document generations of target records a tab keeps behind its current one.
///
/// A handle from one of these still resolves to the stale-target refusal with its last-known
/// context, so the agent is told what replaced it. Older records add memory and no information.
const TARGET_GENERATIONS_KEPT_BEHIND: u64 = 2;

/// The most target records one workspace holds, and the most tombstones it keeps.
///
/// Reads mint records as fast as pages show controls, so memory needs a hard bound. Records of
/// earlier generations go first, oldest first. Only when current-generation records alone are
/// over the limit does the least recently registered one go, so a page that re-renders its
/// controls within one document stays bounded too.
const TARGET_RECORD_LIMIT: usize = 2000;

/// The most characters of a tab's URL the browser adapter reports, adapter protocol 4's
/// `url_chars` limit.
///
/// A remembered tab is compared by the URL bounded the same way, so the digest the workspace
/// remembers and the URL the browser lists read one string.
pub const TAB_URL_CHARS: usize = 8192;

/// The most characters of a tab title a binding keeps.
const TAB_TITLE_CHARS: usize = 500;

/// How many hexadecimal characters a [`UrlDigest`] has: one SHA-256 digest.
const URL_DIGEST_CHARS: usize = 64;

/// A digest of the URL a workspace last knew for one of its tabs.
///
/// A physical tab id names one tab only within one browser run, and a restarted browser hands its
/// ids out again. So a binding the workspace keeps across a gap in what it knows of its browser is
/// remembered by its id and this digest, and it is confirmed only by a live tab that has the same
/// id and shows the same URL (ADR-0203 Decision 1). The resume token carries the digest, so the
/// edge that holds the token holds no page address.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UrlDigest(String);

impl UrlDigest {
    /// The digest of `url`, bounded as the adapter bounds a tab's URL.
    #[must_use]
    pub fn of(url: &str) -> Self {
        use sha2::{Digest, Sha256};
        use std::fmt::Write as _;

        let bounded: String = url.chars().take(TAB_URL_CHARS).collect();
        let mut digest = String::with_capacity(URL_DIGEST_CHARS);
        for byte in Sha256::digest(bounded.as_bytes()) {
            let _ = write!(digest, "{byte:02x}");
        }
        Self(digest)
    }

    /// Read a digest [`Self::as_str`] wrote, as a resume token carries it. Anything else is no
    /// digest, and the binding it came with is not restored.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        (text.len() == URL_DIGEST_CHARS
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(text.to_owned()))
    }

    /// The digest as lowercase hexadecimal.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The live tab that confirms a tab remembered as `physical_id` at this digest: the one with
    /// that id, when it shows the remembered URL.
    ///
    /// This is the one rule for every remembered tab, a pending binding and a released tab alike.
    /// After a browser restart the id may name another tab, and a tab at another address is never
    /// taken for the one the workspace knew.
    #[must_use]
    pub fn confirmed_in<'a>(
        &self,
        physical_id: u64,
        live: &'a [PhysicalTab],
    ) -> Option<&'a PhysicalTab> {
        live.iter()
            .find(|tab| tab.tab_id == physical_id && Self::of(&tab.url) == *self)
    }
}

/// One tab binding a workspace holds, confirmed or pending, as the resume token carries it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabBinding {
    /// The model-facing tab handle.
    pub handle: String,
    /// The browser holding the tab.
    pub browser: String,
    /// The physical tab id inside that browser.
    pub physical_id: u64,
    /// The digest of the URL the workspace last knew for the tab.
    pub url: UrlDigest,
    /// Whether the binding waits for its browser to confirm it.
    pub pending: bool,
}

/// How a workspace holds one physical tab of one browser, as a command addressed to that tab
/// reads it before it crosses to the browser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingState {
    /// The workspace holds a binding of the tab that its browser has confirmed.
    Confirmed,
    /// The workspace holds a binding of the tab that waits for its browser to confirm it.
    Pending,
    /// The workspace holds no binding of the tab.
    Unbound,
}

/// The handles a call names that choose its tab: a tab, targets on a tab, or a view of one.
///
/// A call that names none of them works in the workspace's only tab.
#[derive(Clone, Debug, Default)]
pub struct TabRequest<'a> {
    /// The tab handle the call names.
    pub tab: Option<&'a str>,
    /// Every target handle the call names, each of which lives on one tab.
    pub targets: Vec<&'a str>,
    /// The view handle the call names, which was taken on one tab.
    pub view: Option<&'a str>,
}

/// Mint a short handle with `prefix` that `taken` does not already name.
fn short_handle(prefix: &str, taken: impl Fn(&str) -> bool) -> String {
    loop {
        let mut random = [0_u8; HANDLE_SUFFIX_CHARS];
        getrandom::getrandom(&mut random).expect("the operating system supplies random bytes");
        let mut handle = String::from(prefix);
        handle.extend(
            random
                .iter()
                .map(|byte| char::from(HANDLE_ALPHABET[usize::from(byte % 32)])),
        );
        if !taken(&handle) {
            return handle;
        }
    }
}

/// Immutable user-facing summary of one admitted workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    /// Opaque workspace identity.
    pub id: String,
    /// The connection that opened this workspace, which names it for as long as it lives, whether
    /// or not that connection is still current. Attribution only, never authority.
    pub opener: ConnectionDetails,
    /// Current connections, each with its own immutable observed and claimed evidence.
    pub connections: Vec<ConnectionDetails>,
    /// The browser this workspace works in, once its first work chose one, read under the same
    /// lock as the rest of the summary.
    pub browser: Option<String>,
    /// Number of controlled tabs, which a binding waiting for its browser is not yet.
    pub tab_count: usize,
}

impl WorkspaceSummary {
    /// Summarize one workspace as the aggregate holds it.
    fn of(id: &WorkspaceId, workspace: &WorkspaceState) -> Self {
        let mut connections: Vec<_> = workspace
            .connections
            .values()
            .map(|connection| connection.details())
            .collect();
        connections.sort_by(|a, b| a.connection_id.cmp(&b.connection_id));
        Self {
            id: id.as_str().into(),
            opener: workspace.opener.details(),
            connections,
            browser: workspace.browser.clone(),
            tab_count: workspace.confirmed_tabs().count(),
        }
    }
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
    /// The digest of the governed URL the tab held when its workspace released it.
    pub url: UrlDigest,
}

impl ReleasedTab {
    /// The live tab that is still this released tab, by the rule every remembered tab follows
    /// ([`UrlDigest::confirmed_in`]).
    #[must_use]
    pub fn confirmed_in<'a>(&self, live: &'a [PhysicalTab]) -> Option<&'a PhysicalTab> {
        self.url.confirmed_in(self.physical_id, live)
    }
}

impl ReleasedTabs {
    /// The confirmed tabs a leaving workspace held. A pending binding is never closed through the
    /// browser, because its id may name a tab the person opened since.
    fn from_state(state: WorkspaceState) -> Self {
        Self {
            physical_ids: state
                .confirmed_tabs()
                .map(|(_, tab)| tab.physical_id)
                .collect(),
            browser: state.browser,
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
    /// When the control was last registered, on the book's clock, so the least recently seen
    /// current-generation record is the one a full book drops.
    registered_at: u64,
}

/// A workspace's target records, with an index so a control keeps one handle per document.
///
/// The index maps a tab, a document generation and a locator to the handle that names that
/// control. Every insert and removal goes through this type, so the two maps cannot disagree.
///
/// A handle that stops working leaves a tombstone: the word that says why, kept so the next use
/// refuses at once and says it. Tombstones are bounded, oldest dropped first, and a handle whose
/// tombstone is gone reads as unknown.
#[derive(Debug, Default)]
struct TargetBook {
    records: HashMap<TargetHandle, TargetState>,
    index: HashMap<(TabHandle, u64, String), TargetHandle>,
    retired: HashMap<TargetHandle, StaleReason>,
    retired_order: VecDeque<TargetHandle>,
    clock: u64,
}

impl TargetBook {
    fn contains(&self, handle: &TargetHandle) -> bool {
        self.records.contains_key(handle)
    }

    fn get(&self, handle: &TargetHandle) -> Option<&TargetState> {
        self.records.get(handle)
    }

    /// Why a handle was retired, if it was and the tombstone is still kept.
    fn retired_reason(&self, handle: &TargetHandle) -> Option<StaleReason> {
        self.retired.get(handle).copied()
    }

    /// Record one observed control and return its handle.
    ///
    /// A control already recorded for the same tab, generation and locator keeps its handle and
    /// takes the new observation's role, name and credential class. A handle the page reported
    /// removed lives again when its control is read again. Otherwise a fresh handle is minted.
    fn register(&mut self, tab: &SelectedTab, target: &ObservedTarget) -> TargetHandle {
        self.clock += 1;
        let registered_at = self.clock;
        let key = (tab.handle.clone(), tab.generation, target.locator.clone());
        // A page authors its own role attribute, so it is narrowed here, at the one door
        // observed targets come through. The page's own string is never stored in the narrowed
        // role and so can never reach a sentence written to the audit.
        let role = TargetRole::classify(&target.role);
        if let Some(handle) = self.index.get(&key).cloned() {
            if let Some(record) = self.records.get_mut(&handle) {
                record.credential_class = target.credential_class;
                record.role = role;
                record.role_page.clone_from(&target.role);
                record.name.clone_from(&target.name);
                record.registered_at = registered_at;
                if self.retired_reason(&handle) == Some(StaleReason::Removed) {
                    self.revive(&handle);
                }
                return handle;
            }
        }
        let handle = TargetHandle(short_handle("target_", |candidate| {
            self.records.contains_key(&TargetHandle(candidate.into()))
        }));
        self.records.insert(
            handle.clone(),
            TargetState {
                tab: tab.handle.clone(),
                generation: tab.generation,
                locator: target.locator.clone(),
                credential_class: target.credential_class,
                role,
                role_page: target.role.clone(),
                name: target.name.clone(),
                registered_at,
            },
        );
        self.index.insert(key, handle.clone());
        handle
    }

    /// Leave a tombstone for a handle. The first reason a handle is given stays.
    fn retire(&mut self, handle: TargetHandle, reason: StaleReason) {
        if self.retired.contains_key(&handle) {
            return;
        }
        self.retired.insert(handle.clone(), reason);
        self.retired_order.push_back(handle);
        while self.retired_order.len() > TARGET_RECORD_LIMIT {
            if let Some(oldest) = self.retired_order.pop_front() {
                self.retired.remove(&oldest);
            }
        }
    }

    /// Take a handle's tombstone back, because its control is on the page again.
    fn revive(&mut self, handle: &TargetHandle) {
        self.retired.remove(handle);
        self.retired_order.retain(|kept| kept != handle);
    }

    /// Retire a handle the page reported removed. The record stays, so the handle can be
    /// revived and a refusal can still search for what it named.
    fn mark_removed(&mut self, handle: &TargetHandle) {
        if self.records.contains_key(handle) {
            self.retire(handle.clone(), StaleReason::Removed);
        }
    }

    /// Remove every record that `doomed` selects, leaving each a tombstone with `reason` and
    /// keeping the index in step.
    fn retire_where(
        &mut self,
        reason: StaleReason,
        doomed: impl Fn(&TargetHandle, &TargetState) -> bool,
    ) {
        let handles: Vec<TargetHandle> = self
            .records
            .iter()
            .filter(|(handle, record)| doomed(handle, record))
            .map(|(handle, _)| handle.clone())
            .collect();
        for handle in handles {
            self.records.remove(&handle);
            self.retire(handle, reason);
        }
        let records = &self.records;
        self.index.retain(|_, handle| records.contains_key(handle));
    }

    /// Retire all records of a closed tab.
    fn remove_tab(&mut self, tab: &TabHandle) {
        self.retire_where(StaleReason::TabClosed, |_, record| &record.tab == tab);
    }

    /// Retire a tab's records older than the generations kept behind its current one.
    fn prune_tab(&mut self, tab: &TabHandle, current: u64) {
        let oldest_kept = current.saturating_sub(TARGET_GENERATIONS_KEPT_BEHIND);
        self.retire_where(StaleReason::Navigated, |_, record| {
            &record.tab == tab && record.generation < oldest_kept
        });
    }

    /// Bring the book under the record limit.
    ///
    /// A record is current when its generation is its tab's present generation. Records of
    /// earlier generations go first, oldest first, and read as navigated. If current records
    /// alone are still over the limit, the least recently registered go and read as unknown.
    fn enforce_limit(&mut self, tabs: &HashMap<TabHandle, TabState>) {
        let Some(excess) = self.records.len().checked_sub(TARGET_RECORD_LIMIT) else {
            return;
        };
        let mut evictable: Vec<(u64, u64, TargetHandle)> = self
            .records
            .iter()
            .filter(|(_, record)| {
                tabs.get(&record.tab)
                    .is_some_and(|tab| record.generation != tab.generation)
            })
            .map(|(handle, record)| (record.generation, record.registered_at, handle.clone()))
            .collect();
        evictable
            .sort_unstable_by_key(|(generation, registered_at, _)| (*generation, *registered_at));
        evictable.truncate(excess);
        let evicted: std::collections::HashSet<TargetHandle> =
            evictable.into_iter().map(|(_, _, handle)| handle).collect();
        self.retire_where(StaleReason::Navigated, |handle, _| evicted.contains(handle));
        let Some(excess) = self.records.len().checked_sub(TARGET_RECORD_LIMIT) else {
            return;
        };
        let mut by_age: Vec<(u64, TargetHandle)> = self
            .records
            .iter()
            .map(|(handle, record)| (record.registered_at, handle.clone()))
            .collect();
        by_age.sort_unstable_by_key(|(registered_at, _)| *registered_at);
        by_age.truncate(excess);
        let evicted: std::collections::HashSet<TargetHandle> =
            by_age.into_iter().map(|(_, handle)| handle).collect();
        self.retire_where(StaleReason::Unknown, |handle, _| evicted.contains(handle));
    }
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
    /// The kind of JavaScript dialog the browser last reported open on this tab, if one is.
    dialog: Option<DialogKind>,
    /// The browser holding the tab. A confirmed binding is always in the workspace's browser; a
    /// pending one a resume token brought back names its own.
    browser: String,
    physical_id: u64,
    generation: u64,
    url: String,
    title: String,
    readiness: BrowserReadiness,
    held: bool,
    /// The URL the binding is remembered by while it waits for its browser to confirm it, or none
    /// once it is confirmed (ADR-0203 Decision 1).
    ///
    /// A pending binding is inert. It takes no commit, readiness or dialog event, nothing selects
    /// it, it counts as nobody's tab, and it is never closed through the browser. It stays in the
    /// one tabs map only so its handle is never minted again.
    pending: Option<UrlDigest>,
}

impl TabState {
    /// A fresh confirmed binding of `browser`'s tab `physical_id`, at generation 0 with nothing
    /// known of its page yet.
    fn bound(browser: &str, physical_id: u64) -> Self {
        Self {
            dialog: None,
            browser: browser.into(),
            physical_id,
            generation: 0,
            url: String::new(),
            title: String::new(),
            readiness: BrowserReadiness::Unknown,
            held: false,
            pending: None,
        }
    }

    /// Whether the binding is confirmed: the browser holding it has shown it since the last gap
    /// in what the service knew of that browser.
    const fn is_confirmed(&self) -> bool {
        self.pending.is_none()
    }

    /// Whether this is the confirmed binding of `browser`'s tab `physical_id`.
    fn is_confirmed_tab(&self, browser: &str, physical_id: u64) -> bool {
        self.is_confirmed() && self.physical_id == physical_id && self.browser == browser
    }

    /// The digest of the URL this binding is remembered by.
    fn remembered_url(&self) -> UrlDigest {
        self.pending
            .clone()
            .unwrap_or_else(|| UrlDigest::of(&self.url))
    }
}

#[derive(Debug)]
struct WorkspaceState {
    /// The connection that opened this workspace.
    ///
    /// It names the workspace for its whole life: its session card, its tab group and the released
    /// tabs it may reuse. A command-line workspace outlives its connections (ADR-0106), so it cannot
    /// be named from the current ones, and a group is named for the client that opened it
    /// (ADR-0066). A later connection joins `connections` and renames nothing.
    opener: Arc<ConnectionEvidence>,
    connections: HashMap<String, Arc<ConnectionEvidence>>,
    /// What owns this workspace, when it outlives the connection that opened it (ADR-0106).
    session: Option<SessionMarker>,
    /// Which browser this workspace works in, once its first tab bound one.
    ///
    /// Only binding a tab writes it ([`WorkspaceLease::add_tab`], [`WorkspaceLease::restore_tab`]
    /// and a confirmed pending binding, [`WorkspaceStore::confirm`]), so a call that binds no tab
    /// leaves the choice open.
    /// A workspace lives in one browser for its whole life. Its tabs, targets, and views are all
    /// physical things inside that one browser, and physical tab ids only mean anything there, so
    /// a workspace that could span two browsers could not name its own tabs unambiguously. When
    /// that browser stops, the binding stays: the work belongs to a person's Chrome profile, and
    /// silently finishing it in their Edge profile would be a different act than the one asked
    /// for (ADR-0084 D4).
    browser: Option<String>,
    leased: bool,
    /// Every tab binding, confirmed or pending.
    tabs: HashMap<TabHandle, TabState>,
    /// Pending bindings of another browser that this workspace forgot when its first tab bound
    /// its browser, until the call that bound it reports them.
    forgotten: Vec<TabHandle>,
    targets: TargetBook,
    views: HashMap<ViewHandle, ViewState>,
    snapshots: HashMap<SnapshotHandle, SnapshotState>,
    images: HashMap<ImageHandle, ImageState>,
}

impl WorkspaceState {
    /// The bindings the browser has confirmed, which are the workspace's controlled tabs.
    fn confirmed_tabs(&self) -> impl Iterator<Item = (&TabHandle, &TabState)> {
        self.tabs.iter().filter(|(_, tab)| tab.is_confirmed())
    }

    /// The confirmed binding `handle` names. A pending binding answers as a stale handle: nothing
    /// reads or acts on a tab its browser has not confirmed.
    fn confirmed(&self, handle: &TabHandle) -> Result<&TabState, WorkspaceError> {
        self.tabs
            .get(handle)
            .filter(|tab| tab.is_confirmed())
            .ok_or(WorkspaceError::StaleTab)
    }

    /// The confirmed binding `handle` names, to change it.
    fn confirmed_mut(&mut self, handle: &TabHandle) -> Result<&mut TabState, WorkspaceError> {
        self.tabs
            .get_mut(handle)
            .filter(|tab| tab.is_confirmed())
            .ok_or(WorkspaceError::StaleTab)
    }

    /// Bind the workspace to `browser` when no tab has bound it yet. The pending bindings of every
    /// other browser are then forgotten, because a workspace lives in one browser, and kept in
    /// `forgotten` until the call reports them.
    fn bind_browser(&mut self, browser: &str) {
        if self.browser.is_some() {
            return;
        }
        self.browser = Some(browser.into());
        let elsewhere: Vec<TabHandle> = self
            .tabs
            .iter()
            .filter(|(_, tab)| !tab.is_confirmed() && tab.browser != browser)
            .map(|(handle, _)| handle.clone())
            .collect();
        for handle in elsewhere {
            self.forget_tab(&handle);
            self.forgotten.push(handle);
        }
    }

    /// Forget the pending binding of `browser`'s tab `physical_id`, if this workspace has one.
    fn forget_pending_tab(&mut self, browser: &str, physical_id: u64) {
        let stale: Vec<TabHandle> = self
            .tabs
            .iter()
            .filter(|(_, tab)| {
                !tab.is_confirmed() && tab.physical_id == physical_id && tab.browser == browser
            })
            .map(|(handle, _)| handle.clone())
            .collect();
        for handle in stale {
            self.forget_tab(&handle);
        }
    }

    /// Forget one tab binding and everything minted on it: its target records, which leave
    /// tab-closed tombstones, its views, its snapshots and its images.
    ///
    /// Every end of a binding comes through here, so no capture, diff base or coordinate outlives
    /// the tab it was taken from, and a handle bound again starts with nothing.
    fn forget_tab(&mut self, handle: &TabHandle) {
        self.tabs.remove(handle);
        self.targets.remove_tab(handle);
        self.views.retain(|_, view| &view.tab != handle);
        self.snapshots.retain(|_, snapshot| &snapshot.tab != handle);
        self.images.retain(|_, image| &image.tab != handle);
    }
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
    /// workspace takes it, and when its release close did anything but keep it. A browser that
    /// reconnects may have restarted and handed its ids out again, so a claimed tab is reused only
    /// where the live browser confirms it ([`ReleasedTab::confirmed_in`]).
    released: HashMap<String, HashMap<u64, PooledTab>>,
}

/// One tab in the released pool.
#[derive(Debug)]
struct PooledTab {
    /// The governed URL the tab held when its workspace released it.
    url: String,
    /// The name the opener of the workspace that released the tab claimed.
    ///
    /// Every controlled tab sits in the browser group named for its workspace opener's claim, and
    /// a reused tab is navigated where it is, never regrouped. So only a session of the same
    /// client may claim it; any other session would control a tab shown under another client's
    /// group (ADR-0066).
    opener_claim: String,
    /// Whether the release close answered that the browser keeps the tab open.
    ///
    /// Until it does, the browser may still bind the tab to the dead workspace, and that pending
    /// close could still forget, unretain, or close the tab under whoever took it. So a tab whose
    /// cleanup has not answered is never claimed (ADR-0196 Decision 8).
    kept_open: bool,
}

impl AggregateState {
    /// Remove `workspace` and return the confirmed tabs it held, recorded in the released pool.
    fn release(&mut self, workspace: &WorkspaceId) -> ReleasedTabs {
        let Some(removed) = self.workspaces.remove(workspace) else {
            return ReleasedTabs::default();
        };
        self.remember_released(&removed);
        ReleasedTabs::from_state(removed)
    }

    /// Record the tabs a leaving workspace still holds as waiting for their release cleanup.
    ///
    /// None of them is reusable yet: only a cleanup answer that the browser kept a tab open
    /// (`settle_released`) offers it to the next open on its host from a session of the same
    /// client. A held tab sits on a destination governance refused, a tab with no governed URL
    /// never landed, and a pending binding may name a tab the person opened, so none of them is
    /// recorded at all.
    fn remember_released(&mut self, workspace: &WorkspaceState) {
        let Some(browser) = workspace.browser.as_ref() else {
            return;
        };
        let tabs: Vec<_> = workspace
            .confirmed_tabs()
            .map(|(_, tab)| tab)
            .filter(|tab| !tab.held && !tab.url.is_empty())
            .map(|tab| {
                (
                    tab.physical_id,
                    PooledTab {
                        url: tab.url.clone(),
                        opener_claim: workspace.opener.reported_application().into(),
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

    /// Whether a workspace other than `current` holds a confirmed binding of `browser`'s tab.
    ///
    /// Physical tab ids are unique inside one browser and nowhere else: Chrome's tab 5 and Edge's
    /// tab 5 are different tabs and must not refuse each other. A pending binding is nobody's tab
    /// yet, so it never refuses another workspace.
    fn controlled_elsewhere(&self, current: &WorkspaceId, browser: &str, physical_id: u64) -> bool {
        self.workspaces.iter().any(|(id, workspace)| {
            id != current
                && workspace
                    .tabs
                    .values()
                    .any(|known| known.is_confirmed_tab(browser, physical_id))
        })
    }

    /// Check that `current` may bind the physical tab `physical_id` of `browser`.
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::UnknownWorkspace`] when the workspace is gone,
    /// [`WorkspaceError::BrowserPinned`] when its first tab bound a different browser, and
    /// [`WorkspaceError::PhysicalTabOwned`] when another workspace in that browser controls the
    /// tab.
    fn admit_tab(
        &self,
        current: &WorkspaceId,
        browser: &str,
        physical_id: u64,
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .workspaces
            .get(current)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if workspace
            .browser
            .as_deref()
            .is_some_and(|bound| bound != browser)
        {
            return Err(WorkspaceError::BrowserPinned);
        }
        if self.controlled_elsewhere(current, browser, physical_id) {
            return Err(WorkspaceError::PhysicalTabOwned);
        }
        Ok(())
    }

    /// Bind a physical tab of `browser` that the browser just opened or adopted to `current` under
    /// `handle`.
    ///
    /// The tab leaves the released pool. The browser has just assigned or handed over its id, which
    /// proves any pending binding of that id stale, so every workspace forgets one. The first tab
    /// binds the workspace's browser, which no later tab changes. Whatever the handle carried
    /// before goes with it, so the binding starts clean at generation 0. Call [`Self::admit_tab`]
    /// first.
    fn bind_tab(
        &mut self,
        current: &WorkspaceId,
        browser: &str,
        handle: &TabHandle,
        value: TabState,
    ) -> Result<SelectedTab, WorkspaceError> {
        self.forget_released(browser, value.physical_id);
        if !self.workspaces.contains_key(current) {
            return Err(WorkspaceError::UnknownWorkspace);
        }
        for workspace in self.workspaces.values_mut() {
            workspace.forget_pending_tab(browser, value.physical_id);
        }
        let workspace = self
            .workspaces
            .get_mut(current)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        workspace.bind_browser(browser);
        workspace.forget_tab(handle);
        workspace.tabs.insert(handle.clone(), value.clone());
        Ok(selected(handle, &value))
    }

    /// Confirm or forget `current`'s pending bindings of `browser` against the tabs the browser
    /// lists now (ADR-0203 Decision 1).
    ///
    /// A binding is kept when a live tab has its id and shows the URL it is remembered by, and no
    /// other workspace holds a confirmed binding of that tab. It then takes the live title and
    /// readiness, keeps its generation and handles, and the first one binds the workspace's
    /// browser. Every other pending binding of `browser` is forgotten as closed.
    fn confirm(
        &mut self,
        current: &WorkspaceId,
        browser: &str,
        live: &[PhysicalTab],
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .workspaces
            .get(current)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let pending: Vec<(TabHandle, u64, UrlDigest)> = workspace
            .tabs
            .iter()
            .filter(|(_, tab)| tab.browser == browser)
            .filter_map(|(handle, tab)| {
                Some((handle.clone(), tab.physical_id, tab.pending.clone()?))
            })
            .collect();
        for (handle, physical_id, url) in pending {
            let shown = url
                .confirmed_in(physical_id, live)
                .filter(|_| self.admit_tab(current, browser, physical_id).is_ok())
                .cloned();
            if shown.is_some() {
                self.forget_released(browser, physical_id);
            }
            let workspace = self
                .workspaces
                .get_mut(current)
                .ok_or(WorkspaceError::UnknownWorkspace)?;
            let Some(shown) = shown else {
                workspace.forget_tab(&handle);
                continue;
            };
            if let Some(tab) = workspace.tabs.get_mut(&handle) {
                tab.pending = None;
                tab.url = shown.url;
                tab.title = bounded(&shown.title, TAB_TITLE_CHARS);
                tab.readiness = shown.readiness;
            }
            workspace.bind_browser(browser);
        }
        Ok(())
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
            .map(|(id, workspace)| WorkspaceSummary::of(id, workspace))
            .collect();
        summaries.sort_by(|left, right| left.id.cmp(&right.id));
        summaries
    }

    /// Attach one actual connection, resuming only workspace continuity when an owner is supplied.
    /// The connection's evidence stays independent of every other connection in that workspace,
    /// and only a connection that opens a new workspace becomes its opener.
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
                opener: Arc::clone(&connection),
                connections: HashMap::from([(connection_id, connection)]),
                session,
                browser: None,
                leased: false,
                tabs: HashMap::new(),
                forgotten: Vec::new(),
                targets: TargetBook::default(),
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
    #[cfg(test)]
    pub(crate) fn single_connection(
        &self,
        workspace: &WorkspaceId,
    ) -> Option<Arc<ConnectionEvidence>> {
        let state = self.lock();
        let connections = &state.workspaces.get(workspace)?.connections;
        (connections.len() == 1).then(|| connections.values().next().unwrap().clone())
    }

    #[cfg(test)]
    pub(crate) fn admit(
        &self,
        label: String,
        channel: crate::intake::session::IntakeChannel,
    ) -> WorkspaceId {
        self.connect(Arc::new(ConnectionEvidence::claimed(&label, channel)), None)
    }

    #[cfg(test)]
    pub(crate) fn resume_or_admit(
        &self,
        label: String,
        channel: crate::intake::session::IntakeChannel,
        marker: SessionMarker,
    ) -> WorkspaceId {
        self.connect(
            Arc::new(ConnectionEvidence::claimed(&label, channel)),
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

    /// Whether this workspace outlives its connection.
    #[must_use]
    pub fn is_owned(&self, workspace: &WorkspaceId) -> bool {
        self.lock()
            .workspaces
            .get(workspace)
            .is_some_and(|state| state.session.is_some())
    }

    /// Every admitted workspace.
    #[must_use]
    pub fn ids(&self) -> Vec<WorkspaceId> {
        self.lock().workspaces.keys().cloned().collect()
    }

    /// Release an MCP workspace and return the physical tabs it owned.
    ///
    /// The tabs wait in the released pool for their cleanup. A tab becomes reusable by the next
    /// open on its host from a session of the same client only once `settle_released` reports
    /// that the browser kept it open (the person's preserve-tabs choice).
    pub fn release(&self, workspace: &WorkspaceId) -> ReleasedTabs {
        self.lock().release(workspace)
    }

    /// Release `workspace` as [`Self::release`] does once no invocation holds its lease, waiting
    /// for that until `until` at the latest.
    ///
    /// Returns `None`, and leaves the workspace as it is, when a call still holds it at `until`.
    /// A workspace that is already gone releases no tab.
    pub fn release_unleased(
        &self,
        workspace: &WorkspaceId,
        until: Instant,
    ) -> Option<ReleasedTabs> {
        let mut state = self.lock();
        while state
            .workspaces
            .get(workspace)
            .is_some_and(|held| held.leased)
        {
            let wait = until.saturating_duration_since(Instant::now());
            if wait.is_zero() {
                return None;
            }
            state = self
                .leased
                .wait_timeout(state, wait)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
        Some(state.release(workspace))
    }

    /// Forget every workspace and every released tab without a browser command, at a service end
    /// that sends the browser none: the end of the session and the idle exit (ADR-0203 Decision 3).
    pub fn forget_all(&self) {
        let mut state = self.lock();
        state.workspaces.clear();
        state.released.clear();
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

    /// Take the released tab an open of `url` in `browser` by a session whose opener claimed
    /// `opener_claim` should reuse, if there is one.
    ///
    /// Candidates are tabs Ghostlight opened, a released workspace whose opener made the same
    /// claim left behind, and the browser kept open when that workspace's cleanup asked to close them, in
    /// this browser, on the same host and port. The label must match because a reused tab stays in
    /// the group it already sits in, and that group is named for the client that released it. An
    /// exact URL wins, then the lowest tab id. The claim removes the tab from the pool under the
    /// aggregate lock, so two sessions can never claim the same tab, and leaves another client's
    /// tabs in the pool. The caller still confirms it in the live browser before using it
    /// ([`ReleasedTab::confirmed_in`]).
    #[must_use]
    pub fn claim_released(
        &self,
        browser: &str,
        opener_claim: &str,
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
                    && released.opener_claim == opener_claim
                    && reusable_site(&released.url).as_ref() == Some(&site)
            })
            .min_by_key(|(physical_id, released)| (released.url != url, **physical_id))
            .map(|(physical_id, released)| ReleasedTab {
                physical_id: *physical_id,
                url: UrlDigest::of(&released.url),
            })?;
        state.forget_released(browser, claimed.physical_id);
        Some(claimed)
    }

    /// Mark every confirmed binding in `browser` pending, after a gap in what the service knows of
    /// that browser: its stream ended, or a new connection replaced it (ADR-0203 Decision 1).
    ///
    /// A browser that comes back may be a restarted one, and a restarted browser hands its tab ids
    /// out again, so a remembered id could name a tab the person just opened. Each binding is then
    /// remembered by its id and the URL the workspace last knew, until the browser lists its tabs
    /// again. The released pool keeps its tabs: a reuse confirms each one by the same rule.
    pub fn apply_browser_disconnect(&self, browser: &str) {
        let mut state = self.lock();
        for tab in state
            .workspaces
            .values_mut()
            .flat_map(|workspace| workspace.tabs.values_mut())
            .filter(|tab| tab.is_confirmed() && tab.browser == browser)
        {
            tab.pending = Some(UrlDigest::of(&tab.url));
        }
    }

    /// Bring back one binding a resume token remembered, pending until `browser` confirms it.
    ///
    /// A handle the workspace already holds is left as it is. The binding counts as nobody's tab
    /// until its browser lists a tab with its id at the URL it is remembered by.
    pub fn remember_tab(
        &self,
        workspace: &WorkspaceId,
        handle: &str,
        browser: &str,
        physical_id: u64,
        url: UrlDigest,
    ) {
        let mut state = self.lock();
        let Some(workspace) = state.workspaces.get_mut(workspace) else {
            return;
        };
        let handle = TabHandle(handle.into());
        if workspace.tabs.contains_key(&handle) {
            return;
        }
        workspace.tabs.insert(
            handle,
            TabState {
                pending: Some(url),
                ..TabState::bound(browser, physical_id)
            },
        );
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

    /// The workspace's controlled tabs, ordered by handle. A binding waiting for its browser to
    /// confirm it is not one yet.
    pub fn tabs(&self, workspace: &WorkspaceId) -> Result<Vec<SelectedTab>, WorkspaceError> {
        let state = self.lock();
        let workspace = state
            .workspaces
            .get(workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let mut tabs: Vec<_> = workspace
            .confirmed_tabs()
            .map(|(handle, tab)| selected(handle, tab))
            .collect();
        tabs.sort_by(|left, right| left.handle.as_str().cmp(right.handle.as_str()));
        Ok(tabs)
    }

    /// The name the client that opened a workspace claimed, which titles its tab group and picks
    /// the released tabs it may reuse. Presentation only, never routing or authority.
    pub fn opener_claim(&self, workspace: &WorkspaceId) -> Result<String, WorkspaceError> {
        self.lock()
            .workspaces
            .get(workspace)
            .map(|state| state.opener.reported_application().to_owned())
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

    /// The client label of another admitted workspace that owns the requested tab handle.
    ///
    /// Returns None when no other workspace owns the handle, or when its owner has no client
    /// label: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn tab_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TabHandle(requested.into());
        self.labeled_owner(current, |workspace| workspace.confirmed(&handle).is_ok())
    }

    /// The client label of another admitted workspace that owns the requested target handle.
    ///
    /// Returns None when no other workspace owns the handle, or when its owner has no client
    /// label: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn target_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TargetHandle(requested.into());
        self.labeled_owner(current, |workspace| workspace.targets.contains(&handle))
    }

    /// Last-known context for a target handle that no longer resolves, so a refusal can offer
    /// fresh candidates from the live page. `None` when the handle never belonged to this
    /// workspace, its record is gone, or its whole tab is gone.
    ///
    /// The context is deliberately narrow: the physical tab to re-query, the governed URL it
    /// last had, and what the page itself called the control. No page content travels.
    #[must_use]
    pub fn stale_target_context(
        &self,
        workspace: &WorkspaceId,
        requested: &str,
    ) -> Option<StaleTargetContext> {
        let state = self.lock();
        let workspace = state.workspaces.get(workspace)?;
        let target = workspace.targets.get(&TargetHandle(requested.into()))?;
        let tab = workspace.confirmed(&target.tab).ok()?;
        Some(StaleTargetContext {
            physical_id: tab.physical_id,
            url: tab.url.clone(),
            name: target.name.clone(),
            role_page: target.role_page.clone(),
        })
    }

    /// Record that the page reported a handle's control removed, so the handle refuses at once
    /// from now on. A handle this workspace never issued is left alone.
    pub fn mark_target_removed(&self, workspace: &WorkspaceId, requested: &str) {
        if let Some(workspace) = self.lock().workspaces.get_mut(workspace) {
            workspace
                .targets
                .mark_removed(&TargetHandle(requested.into()));
        }
    }

    /// Hand out handles for controls found while recovering from a stale handle, on the tab that
    /// handle belonged to. The controls were read from the tab's current document, so they take
    /// its current generation. With `same_document` the controls came from the stale handle's own
    /// document, and none is registered if the tab has since moved to another. Empty when the
    /// handle has no record or its tab is gone.
    pub fn register_recovered(
        &self,
        workspace: &WorkspaceId,
        stale: &str,
        same_document: bool,
        targets: &[ObservedTarget],
    ) -> Vec<(TargetHandle, ObservedTarget)> {
        let mut state = self.lock();
        let Some(workspace) = state.workspaces.get_mut(workspace) else {
            return Vec::new();
        };
        let Some(record) = workspace.targets.get(&TargetHandle(stale.into())) else {
            return Vec::new();
        };
        let Ok(tab) = workspace.confirmed(&record.tab) else {
            return Vec::new();
        };
        if same_document && tab.generation != record.generation {
            return Vec::new();
        }
        let selected = selected(&record.tab, tab);
        let mapped = targets
            .iter()
            .map(|target| {
                (
                    workspace.targets.register(&selected, target),
                    target.clone(),
                )
            })
            .collect();
        workspace.targets.enforce_limit(&workspace.tabs);
        mapped
    }

    /// Name the other workspace that holds a handle, by its opener's claimed name only.
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
            .map(|(_, workspace)| workspace.opener.reported_application().to_owned())
            .filter(|label| !label.is_empty())
    }

    /// Whether the workspace is still admitted: a released or reaped one is gone.
    #[must_use]
    pub fn contains(&self, workspace: &str) -> bool {
        self.lock().workspaces.contains_key(workspace)
    }

    /// Which browser this workspace works in, once its first tab bound one.
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
            .map(|state| {
                state
                    .confirmed_tabs()
                    .map(|(_, tab)| tab.physical_id)
                    .collect()
            })
            .unwrap_or_default();
        tabs.sort_unstable();
        tabs
    }

    /// Every tab binding this workspace holds, confirmed or pending, each in its own browser and
    /// with the URL it is remembered by, sorted by handle.
    ///
    /// Read without a lease, so a session can describe its tabs while its own work runs. A pending
    /// binding is listed too, so a resume token written before its browser returns still names it.
    #[must_use]
    pub fn tab_bindings(&self, workspace: &str) -> Vec<TabBinding> {
        let mut bindings: Vec<TabBinding> = self
            .lock()
            .workspaces
            .get(workspace)
            .map(|state| {
                state
                    .tabs
                    .iter()
                    .map(|(handle, tab)| TabBinding {
                        handle: handle.as_str().to_owned(),
                        browser: tab.browser.clone(),
                        physical_id: tab.physical_id,
                        url: tab.remembered_url(),
                        pending: !tab.is_confirmed(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        bindings.sort_by(|left, right| left.handle.cmp(&right.handle));
        bindings
    }

    /// How `workspace` holds `browser`'s tab `physical_id` now.
    ///
    /// Every command addressed to one tab reads this before it crosses, because a gap in what the
    /// service knows of the browser can open while a call runs (ADR-0203 Decision 1).
    #[must_use]
    pub fn binding_state(&self, workspace: &str, browser: &str, physical_id: u64) -> BindingState {
        let state = self.lock();
        let binding = state.workspaces.get(workspace).and_then(|workspace| {
            workspace
                .tabs
                .values()
                .find(|tab| tab.physical_id == physical_id && tab.browser == browser)
        });
        match binding {
            Some(tab) if tab.is_confirmed() => BindingState::Confirmed,
            Some(_) => BindingState::Pending,
            None => BindingState::Unbound,
        }
    }

    /// Confirm or forget `workspace`'s pending bindings of `browser` against `live`, the tabs
    /// that browser lists now (ADR-0203 Decision 1). Only the call that holds the workspace's
    /// lease confirms, before it selects a tab and again before a crossing whose tab went pending
    /// while it ran.
    ///
    /// A binding whose id the browser lists at the URL it is remembered by, and that no other
    /// workspace holds, is kept: it takes the live title and readiness and keeps its generation,
    /// handles and captures. The first one binds the workspace's browser. Every other pending
    /// binding of `browser` is forgotten as closed, and its handle then reads as no longer open.
    pub fn confirm(
        &self,
        workspace: &WorkspaceId,
        browser: &str,
        live: &[PhysicalTab],
    ) -> Result<(), WorkspaceError> {
        self.lock().confirm(workspace, browser, live)
    }

    /// Invalidate agent references after an unrelated page change, without governing browsing.
    ///
    /// A commit in a released tab means the person or the page took it back, since no Ghostlight
    /// work runs there, so the tab also leaves the released pool and is never reused. A pending
    /// binding takes no commit: its id may name another tab now, so only its browser's list
    /// confirms it.
    pub fn note_browser_landing(
        &self,
        browser: &str,
        physical_id: u64,
        url: &str,
    ) -> Option<(WorkspaceId, TabHandle)> {
        let mut state = self.lock();
        state.forget_released(browser, physical_id);
        for (workspace_id, workspace) in &mut state.workspaces {
            if let Some((handle, tab)) = workspace
                .tabs
                .iter_mut()
                .find(|(_, tab)| tab.is_confirmed_tab(browser, physical_id))
            {
                tab.generation = tab.generation.saturating_add(1);
                tab.readiness = BrowserReadiness::Loading;
                tab.held = false;
                tab.url = url.into();
                tab.title.clear();
                workspace.targets.prune_tab(handle, tab.generation);
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
    #[cfg(test)]
    #[must_use]
    pub fn owner_of_physical(&self, browser: &str, physical_id: u64) -> Option<WorkspaceId> {
        let state = self.lock();
        state
            .workspaces
            .iter()
            .find_map(|(workspace_id, workspace)| {
                workspace
                    .tabs
                    .values()
                    .any(|tab| tab.is_confirmed_tab(browser, physical_id))
                    .then(|| workspace_id.clone())
            })
    }

    /// Apply asynchronous readiness only to a non-held confirmed binding.
    pub fn apply_browser_readiness(
        &self,
        browser: &str,
        physical_id: u64,
        readiness: BrowserReadiness,
    ) {
        if let Some(tab) = self
            .lock()
            .workspaces
            .values_mut()
            .flat_map(|workspace| workspace.tabs.values_mut())
            .find(|tab| tab.is_confirmed_tab(browser, physical_id) && !tab.held)
        {
            tab.readiness = readiness;
        }
    }

    /// Record that a JavaScript dialog opened or closed on a controlled tab.
    ///
    /// The browser reports the change as it happens, which is before the call that caused it
    /// answers, so a result can say a dialog is open without asking the browser again.
    pub fn apply_browser_dialog(
        &self,
        browser: &str,
        physical_id: u64,
        dialog: Option<DialogKind>,
    ) {
        if let Some(tab) = self
            .lock()
            .workspaces
            .values_mut()
            .flat_map(|workspace| workspace.tabs.values_mut())
            .find(|tab| tab.is_confirmed_tab(browser, physical_id))
        {
            tab.dialog = dialog;
        }
    }

    /// The kind of dialog open on one of this workspace's tabs, if the browser reported one.
    #[must_use]
    pub fn dialog_open(&self, workspace: &WorkspaceId, physical_id: u64) -> Option<DialogKind> {
        self.lock()
            .workspaces
            .get(workspace)?
            .confirmed_tabs()
            .find(|(_, tab)| tab.physical_id == physical_id)?
            .1
            .dialog
    }

    /// Remove a tab closed outside an invocation, including from the released pool. A pending
    /// binding of the tab is forgotten too.
    pub fn apply_browser_close(&self, browser: &str, physical_id: u64) {
        let mut state = self.lock();
        state.forget_released(browser, physical_id);
        for workspace in state.workspaces.values_mut() {
            let closed: Vec<TabHandle> = workspace
                .tabs
                .iter()
                .filter(|(_, tab)| tab.physical_id == physical_id && tab.browser == browser)
                .map(|(handle, _)| handle.clone())
                .collect();
            for handle in closed {
                workspace.forget_tab(&handle);
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
        self.store.tabs(&self.workspace)
    }

    /// The browsers holding this workspace's pending bindings, sorted and each named once.
    #[must_use]
    pub fn pending_browsers(&self) -> Vec<String> {
        let state = self.store.lock();
        let mut browsers: Vec<String> = state
            .workspaces
            .get(&self.workspace)
            .map(|workspace| {
                workspace
                    .tabs
                    .values()
                    .filter(|tab| !tab.is_confirmed())
                    .map(|tab| tab.browser.clone())
                    .collect()
            })
            .unwrap_or_default();
        browsers.sort();
        browsers.dedup();
        browsers
    }

    /// The browser of the pending binding the call that names `request` would select, if that
    /// binding is pending.
    ///
    /// A named tab, a named target's tab or a named view's tab selects its binding. A call that
    /// names none of them selects the workspace's only tab, so while the workspace holds no
    /// confirmed tab and every pending binding is in one browser, it selects a binding of that
    /// browser.
    #[must_use]
    pub fn pending_browser_of(&self, request: &TabRequest<'_>) -> Option<String> {
        let state = self.store.lock();
        let workspace = state.workspaces.get(&self.workspace)?;
        let named: Vec<TabHandle> = request
            .tab
            .map(|tab| TabHandle(tab.into()))
            .into_iter()
            .chain(request.targets.iter().filter_map(|target| {
                workspace
                    .targets
                    .get(&TargetHandle((*target).into()))
                    .map(|record| record.tab.clone())
            }))
            .chain(request.view.and_then(|view| {
                workspace
                    .views
                    .get(&ViewHandle(view.into()))
                    .map(|view| view.tab.clone())
            }))
            .collect();
        if !named.is_empty() {
            return named.iter().find_map(|handle| {
                workspace
                    .tabs
                    .get(handle)
                    .filter(|binding| !binding.is_confirmed())
                    .map(|binding| binding.browser.clone())
            });
        }
        if workspace.confirmed_tabs().next().is_some() {
            return None;
        }
        let mut browsers = workspace
            .tabs
            .values()
            .filter(|tab| !tab.is_confirmed())
            .map(|tab| tab.browser.as_str());
        let browser = browsers.next()?;
        browsers
            .all(|other| other == browser)
            .then(|| browser.to_owned())
    }

    /// Forget every pending binding of `browser` without asking any browser, after that browser
    /// did not come back for a call that needed it.
    pub fn forget_pending(&self, browser: &str) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let stale: Vec<TabHandle> = workspace
            .tabs
            .iter()
            .filter(|(_, tab)| !tab.is_confirmed() && tab.browser == browser)
            .map(|(handle, _)| handle.clone())
            .collect();
        for handle in stale {
            workspace.forget_tab(&handle);
        }
        Ok(())
    }

    /// Take the pending bindings of other browsers this workspace forgot when its first tab bound
    /// its browser, so the call that bound it can say so. Each is reported once.
    #[must_use]
    pub fn take_forgotten(&self) -> Vec<TabHandle> {
        let mut state = self.store.lock();
        let mut forgotten = state
            .workspaces
            .get_mut(&self.workspace)
            .map(|workspace| std::mem::take(&mut workspace.forgotten))
            .unwrap_or_default();
        forgotten.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        forgotten
    }

    /// Select the named controlled tab, or the only one when the call names none.
    ///
    /// A call that names no tab is refused with [`WorkspaceError::AmbiguousTab`] while the
    /// workspace holds several: callers that share one connection share its workspace, so no
    /// tab among several is the one every caller means. A held tab is refused with
    /// [`WorkspaceError::Held`]: it shows a page governance refused, so nothing reads or acts on
    /// it there.
    pub fn select_tab(&self, requested: Option<&str>) -> Result<SelectedTab, WorkspaceError> {
        let tab = self.select_tab_to_leave(requested)?;
        if tab.held {
            return Err(WorkspaceError::Held);
        }
        Ok(tab)
    }

    /// Select a tab the way [`Self::select_tab`] does, a held one included.
    ///
    /// Only the two calls that take a tab away from the page that held it use this: a navigation
    /// of an existing tab, whose governed landing clears the hold, and a close (ADR-0203
    /// Decision 2.4). Each is authorized on its own, never on the page the held tab shows.
    pub fn select_tab_to_leave(
        &self,
        requested: Option<&str>,
    ) -> Result<SelectedTab, WorkspaceError> {
        let state = self.store.lock();
        let workspace = state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        if let Some(requested) = requested {
            let handle = TabHandle(requested.into());
            if let Ok(tab) = workspace.confirmed(&handle) {
                return Ok(selected(&handle, tab));
            }
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.confirmed(&handle).is_ok()
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedTab
            } else {
                WorkspaceError::StaleTab
            });
        }
        let mut tabs = workspace.confirmed_tabs();
        match (tabs.next(), tabs.next()) {
            (None, _) => Err(WorkspaceError::NoTab),
            (Some((handle, tab)), None) => Ok(selected(handle, tab)),
            (Some(_), Some(_)) => Err(WorkspaceError::AmbiguousTab),
        }
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

    /// Add a physical tab of `browser` that the browser just opened or adopted, under a new opaque
    /// handle.
    ///
    /// The workspace's first tab binds it to `browser` for its life; a tab of any other browser
    /// is then refused with [`WorkspaceError::BrowserPinned`]. The tab is refused when another
    /// workspace in the same browser already controls it. Once added it is this workspace's tab,
    /// so it leaves the released pool, and any pending binding of its id is forgotten.
    pub fn add_tab(&self, browser: &str, tab: &PhysicalTab) -> Result<SelectedTab, WorkspaceError> {
        let mut state = self.store.lock();
        state.admit_tab(&self.workspace, browser, tab.tab_id)?;
        let handle = {
            let tabs = &state
                .workspaces
                .get(&self.workspace)
                .ok_or(WorkspaceError::UnknownWorkspace)?
                .tabs;
            TabHandle(short_handle("tab_", |candidate| {
                tabs.contains_key(&TabHandle(candidate.into()))
            }))
        };
        let value = TabState::bound(browser, tab.tab_id);
        state.bind_tab(&self.workspace, browser, &handle, value)
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
        let known = workspace.confirmed_mut(handle)?;
        if known.physical_id != tab.tab_id {
            return Err(WorkspaceError::StaleTab);
        }
        known.generation = known.generation.saturating_add(1);
        known.url.clone_from(&tab.url);
        known.title = bounded(&tab.title, TAB_TITLE_CHARS);
        known.readiness = tab.readiness;
        known.held = false;
        let current = selected(handle, known);
        workspace.targets.prune_tab(handle, current.generation);
        Ok(current)
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
        let known = workspace.confirmed_mut(handle)?;
        if known.physical_id != tab.tab_id {
            return Err(WorkspaceError::StaleTab);
        }
        known.url.clone_from(&tab.url);
        known.title = bounded(&tab.title, TAB_TITLE_CHARS);
        known.readiness = tab.readiness;
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
        let tab = workspace.confirmed_mut(handle)?;
        tab.readiness = readiness;
        Ok(())
    }

    /// Hold a tab whose page governance refused, after a landing or a read found it there.
    ///
    /// Only [`Self::select_tab_to_leave`] selects the tab while it is held, and a governed landing
    /// clears the hold.
    pub fn hold_tab(&self, handle: &TabHandle) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        workspace.confirmed_mut(handle)?.held = true;
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
        let generation = workspace.confirmed(&tab.handle)?.generation;
        if generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace
            .snapshots
            .retain(|_, snapshot| snapshot.tab != tab.handle);
        let handle = SnapshotHandle(short_handle("snapshot_", |candidate| {
            workspace
                .snapshots
                .contains_key(&SnapshotHandle(candidate.into()))
        }));
        workspace.snapshots.insert(
            handle.clone(),
            SnapshotState {
                tab: tab.handle.clone(),
                generation,
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
        let known = workspace.confirmed(&tab.handle).ok()?;
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
        let generation = workspace.confirmed(&tab.handle)?.generation;
        if generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace.images.retain(|_, image| image.tab != tab.handle);
        let handle = ImageHandle(short_handle("image_", |candidate| {
            workspace
                .images
                .contains_key(&ImageHandle(candidate.into()))
        }));
        workspace.images.insert(
            handle.clone(),
            ImageState {
                tab: tab.handle.clone(),
                generation,
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
        let known = workspace.confirmed(&tab.handle).ok()?;
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
        workspace.forget_tab(handle);
        Ok(())
    }

    /// Recreate one dead tab binding under its existing durable handle.
    ///
    /// Continuation, not resurrection: the new physical tab starts a fresh document with no
    /// history, and the caller's handle keeps referring to "this workspace's tab slot". The
    /// binding starts at generation 0 with nothing the handle carried before, so no target, view,
    /// snapshot or image of the earlier page answers for the new one.
    ///
    /// The tab is a physical tab of `browser`, and binds the workspace to it as
    /// [`Self::add_tab`] does, with the same refusals.
    pub fn restore_tab(
        &self,
        handle: &str,
        browser: &str,
        tab: &PhysicalTab,
    ) -> Result<SelectedTab, WorkspaceError> {
        let handle = TabHandle(handle.into());
        let mut state = self.store.lock();
        state.admit_tab(&self.workspace, browser, tab.tab_id)?;
        if state
            .workspaces
            .iter()
            .any(|(id, candidate)| id != &self.workspace && candidate.confirmed(&handle).is_ok())
        {
            return Err(WorkspaceError::NotOwnedTab);
        }
        let value = TabState {
            url: tab.url.clone(),
            title: tab.title.clone(),
            readiness: tab.readiness,
            ..TabState::bound(browser, tab.tab_id)
        };
        state.bind_tab(&self.workspace, browser, &handle, value)
    }

    /// Map observed browser targets to opaque generation-bound handles.
    ///
    /// A control seen again in the same document keeps its handle.
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
        let known = workspace.confirmed(&tab.handle)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        let mapped = targets
            .iter()
            .map(|target| (workspace.targets.register(tab, target), target.clone()))
            .collect();
        workspace.targets.enforce_limit(&workspace.tabs);
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
        if let Some(reason) = workspace.targets.retired_reason(&handle) {
            return Err(WorkspaceError::StaleTarget { reason });
        }
        let Some(target) = workspace.targets.get(&handle) else {
            let owned_elsewhere = state.workspaces.iter().any(|(id, candidate)| {
                id != &self.workspace && candidate.targets.contains(&handle)
            });
            return Err(if owned_elsewhere {
                WorkspaceError::NotOwnedTarget
            } else {
                WorkspaceError::StaleTarget {
                    reason: StaleReason::Unknown,
                }
            });
        };
        let tab = workspace
            .confirmed(&target.tab)
            .map_err(|_| WorkspaceError::StaleTarget {
                reason: StaleReason::TabClosed,
            })?;
        if target.generation != tab.generation {
            return Err(WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated,
            });
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
        let known = workspace.confirmed(&tab.handle)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        workspace.views.retain(|_, view| view.tab != tab.handle);
        let handle = ViewHandle(short_handle("view_", |candidate| {
            workspace.views.contains_key(&ViewHandle(candidate.into()))
        }));
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
            .confirmed(&view.tab)
            .map_err(|_| WorkspaceError::StaleView)?;
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
            .confirmed(&view.tab)
            .map_err(|_| WorkspaceError::StaleView)?;
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
    /// Target handle no longer names a control, and says why.
    #[error("target handle is stale")]
    StaleTarget {
        /// Why the handle stopped working.
        reason: StaleReason,
    },
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
    use crate::intake::session::{IntakeChannel, SessionMarker};

    use crate::language::outcome::TargetRole;

    use super::{
        short_handle, BindingState, ReleasedTabs, SnapshotScope, StaleReason, TargetBook,
        TargetHandle, WorkspaceError, WorkspaceId, WorkspaceStore, HANDLE_ALPHABET,
        HANDLE_SUFFIX_CHARS,
    };
    use crate::language::outcome::DialogKind;

    const TEST_BROWSER: &str = "browser_test";

    /// Admit one test workspace. Its first tab binds it to [`TEST_BROWSER`].
    fn admit_test(store: &WorkspaceStore) -> WorkspaceId {
        store.admit("test".into(), IntakeChannel::Mcp)
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
    fn connection_details_stay_plural_and_detach_only_the_closing_connection() {
        use crate::provenance::ConnectionEvidence;
        use std::sync::Arc;
        let store = WorkspaceStore::default();
        let first = Arc::new(ConnectionEvidence::claimed("first", IntakeChannel::Mcp));
        let second = Arc::new(ConnectionEvidence::claimed("second", IntakeChannel::Cli));
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
    fn a_later_connection_neither_renames_the_session_nor_its_tab_group() {
        use crate::provenance::ConnectionEvidence;
        use std::sync::Arc;
        let store = WorkspaceStore::default();
        let opener = Arc::new(ConnectionEvidence::claimed(
            "claude-code",
            IntakeChannel::Mcp,
        ));
        let later = Arc::new(ConnectionEvidence::claimed(
            "codex-mcp-client",
            IntakeChannel::Mcp,
        ));
        let owner = SessionMarker::Declared {
            key: "shared".into(),
        };
        let workspace = store.connect(opener.clone(), Some(owner.clone()));
        assert_eq!(store.connect(later.clone(), Some(owner)), workspace);
        let summary = &store.summaries()[0];
        assert_eq!(summary.opener, opener.details());
        assert_eq!(summary.opener.agent.name, "Claude Code");
        assert_eq!(summary.connections.len(), 2);
        assert_eq!(store.opener_claim(&workspace).unwrap(), "claude-code");

        // The opener names the workspace after it leaves, as long as the workspace lives.
        store.disconnect(
            &workspace,
            opener.attribution().connection_id.as_deref().unwrap(),
        );
        let summary = &store.summaries()[0];
        assert_eq!(summary.connections, vec![later.details()]);
        assert_eq!(summary.opener, opener.details());
        assert_eq!(store.opener_claim(&workspace).unwrap(), "claude-code");
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
        let lease = store.acquire(&owned).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(41, "https://example.com/"))
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
        let workspace = store.admit(client.into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        for (id, url) in tabs {
            let tab = lease.add_tab(browser, &physical(*id, url)).unwrap();
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
                url: super::UrlDigest::of("https://example.com/a"),
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
    fn released_tabs_leave_the_pool_when_closed_committed_or_taken() {
        const SHELL: &str = "shell";
        let store = WorkspaceStore::default();
        let owned = store.resume_or_admit(SHELL.into(), IntakeChannel::Cli, marker(4312, 100));
        let lease = store.acquire(&owned).unwrap();
        for id in [1, 2, 3, 4, 5] {
            let tab = lease
                .add_tab(TEST_BROWSER, &physical(id, "https://example.com/"))
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
            .add_tab(TEST_BROWSER, &physical(3, "https://example.com/"))
            .unwrap();
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, SHELL, "https://example.com/"),
            Some(4)
        );
        assert_eq!(
            claimed_by(&store, TEST_BROWSER, SHELL, "https://example.com/"),
            None
        );
    }

    /// A released tab outlives its browser's reconnect, and a reuse takes it only where the live
    /// browser still shows it at its release address, the rule a pending binding follows.
    #[test]
    fn a_released_tab_is_reused_after_a_reconnect_only_where_its_browser_still_shows_it() {
        let url = "https://example.com/kept";
        let store = WorkspaceStore::default();
        let released = workspace_with_tabs(&store, TEST_BROWSER, &[(8, url), (9, url)]);
        kept_open(&store, &store.release(&released));
        store.apply_browser_disconnect(TEST_BROWSER);

        let restarted = [
            physical(8, "https://bank.example/the-persons-own"),
            physical(9, url),
        ];
        let first = store
            .claim_released(TEST_BROWSER, TEST_CLIENT, url)
            .unwrap();
        let second = store
            .claim_released(TEST_BROWSER, TEST_CLIENT, url)
            .unwrap();
        let (moved, kept) = if first.physical_id == 8 {
            (first, second)
        } else {
            (second, first)
        };
        assert_eq!(moved.confirmed_in(&restarted), None);
        assert_eq!(kept.confirmed_in(&restarted), Some(&restarted[1]));
    }

    /// The test URL a remembered binding is kept at, and the person's page a restarted browser
    /// shows under the same id.
    const AGENT_URL: &str = "https://example.com/agent";
    const PERSONS_URL: &str = "https://bank.example/the-persons-own";

    /// A workspace in `TEST_BROWSER` with tab 7 landed at [`AGENT_URL`], after its browser's
    /// stream ended. Returns the workspace and the tab's handle.
    fn pending_workspace(store: &WorkspaceStore) -> (WorkspaceId, String) {
        let workspace = workspace_with_tabs(store, TEST_BROWSER, &[(7, AGENT_URL)]);
        let handle = store.tab_bindings(workspace.as_str())[0].handle.clone();
        store.apply_browser_disconnect(TEST_BROWSER);
        (workspace, handle)
    }

    #[test]
    fn a_disconnect_makes_every_binding_pending_and_inert() {
        let store = WorkspaceStore::default();
        let (workspace, handle) = pending_workspace(&store);
        let bindings = store.tab_bindings(workspace.as_str());
        assert_eq!(bindings.len(), 1);
        assert!(bindings[0].pending);
        assert_eq!(bindings[0].browser, TEST_BROWSER);
        assert_eq!(bindings[0].url, super::UrlDigest::of(AGENT_URL));

        // Nothing selects it, lists it or counts it.
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.pending_browsers(), vec![TEST_BROWSER.to_owned()]);
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                tab: Some(&handle),
                ..Default::default()
            }),
            Some(TEST_BROWSER.into())
        );
        assert_eq!(
            lease.select_tab(Some(&handle)).unwrap_err(),
            WorkspaceError::StaleTab
        );
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
        assert!(lease.tabs().unwrap().is_empty());
        drop(lease);
        assert_eq!(store.summaries()[0].tab_count, 0);
        assert!(store.physical_tabs(workspace.as_str()).is_empty());

        // It takes no readiness or dialog event, and it is nobody's tab.
        store.apply_browser_dialog(TEST_BROWSER, 7, Some(DialogKind::Other));
        assert_eq!(store.dialog_open(&workspace, 7), None);
        assert_eq!(store.owner_of_physical(TEST_BROWSER, 7), None);
        let other = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        assert!(store
            .acquire(&other)
            .unwrap()
            .add_tab(TEST_BROWSER, &physical(9, AGENT_URL))
            .is_ok());

        // The person's close forgets it.
        store.apply_browser_close(TEST_BROWSER, 7);
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
    }

    #[test]
    fn a_call_selects_the_pending_binding_it_names_or_the_only_one() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(7, AGENT_URL))
            .unwrap();
        let target = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap()[0]
            .0
            .clone();
        drop(lease);
        store.apply_browser_disconnect(TEST_BROWSER);
        let lease = store.acquire(&workspace).unwrap();
        let away = Some(TEST_BROWSER.to_owned());

        // A call that names nothing selects the only binding, and a target names its tab.
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            away
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                targets: vec!["target_unknown", target.as_str()],
                ..Default::default()
            }),
            away
        );

        // With a confirmed tab beside it, or pending tabs in two browsers, a call that names
        // nothing selects no pending one.
        store.remember_tab(
            &workspace,
            "tab_away",
            "browser_away",
            9,
            super::UrlDigest::of(AGENT_URL),
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            None
        );
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, AGENT_URL)])
            .unwrap();
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest::default()),
            None
        );
        assert_eq!(
            lease.pending_browser_of(&super::TabRequest {
                tab: Some("tab_away"),
                ..Default::default()
            }),
            Some("browser_away".into()),
            "a named tab selects its binding wherever it is"
        );
    }

    #[test]
    fn a_crossing_reads_how_its_workspace_holds_the_tab_it_names() {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        let state = |id| store.binding_state(workspace.as_str(), TEST_BROWSER, id);
        assert_eq!(state(7), BindingState::Pending);
        assert_eq!(state(8), BindingState::Unbound);
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, AGENT_URL)])
            .unwrap();
        assert_eq!(state(7), BindingState::Confirmed);
        assert_eq!(
            store.binding_state(workspace.as_str(), "browser_elsewhere", 7),
            BindingState::Unbound
        );
    }

    #[test]
    fn a_confirmed_binding_keeps_its_handle_and_a_moved_one_is_forgotten() {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(7, AGENT_URL), (8, "https://example.com/second")],
        );
        let before = store.tab_bindings(workspace.as_str());
        store.apply_browser_disconnect(TEST_BROWSER);
        let lease = store.acquire(&workspace).unwrap();
        let mut live = physical(7, AGENT_URL);
        live.title = "Live title".into();
        store
            .confirm(&workspace, TEST_BROWSER, &[live, physical(8, PERSONS_URL)])
            .unwrap();

        let tabs = lease.tabs().unwrap();
        assert_eq!(tabs.len(), 1);
        let kept = before.iter().find(|tab| tab.physical_id == 7).unwrap();
        assert_eq!(tabs[0].handle.as_str(), kept.handle);
        assert_eq!(tabs[0].title, "Live title");
        assert_eq!(tabs[0].generation, 1, "the landed generation is kept");
        let moved = before.iter().find(|tab| tab.physical_id == 8).unwrap();
        assert_eq!(
            lease.select_tab(Some(&moved.handle)).unwrap_err(),
            WorkspaceError::StaleTab
        );
        assert!(lease.pending_browsers().is_empty());
    }

    #[test]
    fn a_new_tab_with_a_recycled_id_forgets_the_pending_binding() {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        // The restarted browser opened a fresh tab under the old id: the id is proved stale even
        // where the new tab shows the same address.
        let opener = workspace_with_tabs(&store, TEST_BROWSER, &[]);
        store
            .acquire(&opener)
            .unwrap()
            .add_tab(TEST_BROWSER, &physical(7, AGENT_URL))
            .unwrap();
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, AGENT_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
    }

    #[test]
    fn a_commit_never_confirms_a_pending_binding() {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        // The person's own page commits in the tab that took the old id.
        assert_eq!(
            store.note_browser_landing(TEST_BROWSER, 7, PERSONS_URL),
            None
        );
        let bindings = store.tab_bindings(workspace.as_str());
        assert!(bindings[0].pending);
        assert_eq!(bindings[0].url, super::UrlDigest::of(AGENT_URL));
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, PERSONS_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
    }

    #[test]
    fn a_release_after_a_browser_restart_closes_no_pending_tab() {
        let store = WorkspaceStore::default();
        let (workspace, _) = pending_workspace(&store);
        let released = store.release(&workspace);
        assert_eq!(released.browser.as_deref(), Some(TEST_BROWSER));
        assert!(released.physical_ids.is_empty());
        assert_eq!(claimed(&store, TEST_BROWSER, AGENT_URL), None);
    }

    #[test]
    fn a_confirmation_never_takes_a_tab_another_workspace_controls() {
        let store = WorkspaceStore::default();
        // Another session opened the tab while this one's binding waited in a resume token.
        let other = workspace_with_tabs(&store, TEST_BROWSER, &[(7, AGENT_URL)]);
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_kept",
            TEST_BROWSER,
            7,
            super::UrlDigest::of(AGENT_URL),
        );
        let lease = store.acquire(&workspace).unwrap();
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, AGENT_URL)])
            .unwrap();
        assert!(lease.tabs().unwrap().is_empty());
        assert!(store.tab_bindings(workspace.as_str()).is_empty());
        assert_eq!(store.tab_bindings(other.as_str()).len(), 1);
    }

    #[test]
    fn binding_a_browser_forgets_another_browsers_pending_tabs_and_says_so() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_away",
            "browser_away",
            7,
            super::UrlDigest::of(AGENT_URL),
        );
        let lease = store.acquire(&workspace).unwrap();
        lease
            .add_tab(TEST_BROWSER, &physical(3, "https://example.com/"))
            .unwrap();
        assert_eq!(
            lease
                .take_forgotten()
                .iter()
                .map(super::TabHandle::as_str)
                .collect::<Vec<_>>(),
            vec!["tab_away"]
        );
        assert!(lease.take_forgotten().is_empty(), "reported once");
        assert!(lease.pending_browsers().is_empty());
        assert_eq!(
            store.browser_of(workspace.as_str()).as_deref(),
            Some(TEST_BROWSER)
        );
    }

    #[test]
    fn a_remembered_tab_is_confirmed_by_its_own_browser_and_binds_it() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        store.remember_tab(
            &workspace,
            "tab_kept",
            TEST_BROWSER,
            7,
            super::UrlDigest::of(AGENT_URL),
        );
        assert_eq!(store.browser_of(workspace.as_str()), None);
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.forget_pending("browser_elsewhere"), Ok(()));
        store
            .confirm(&workspace, TEST_BROWSER, &[physical(7, AGENT_URL)])
            .unwrap();
        let tab = lease.select_tab(Some("tab_kept")).unwrap();
        assert_eq!(tab.physical_id, 7);
        assert_eq!(tab.url, AGENT_URL);
        assert_eq!(
            store.browser_of(workspace.as_str()).as_deref(),
            Some(TEST_BROWSER)
        );
    }

    #[test]
    fn a_url_digest_reads_the_url_bounded_as_the_adapter_bounds_it() {
        let long = format!("https://example.com/{}", "a".repeat(super::TAB_URL_CHARS));
        let bounded: String = long.chars().take(super::TAB_URL_CHARS).collect();
        assert_eq!(super::UrlDigest::of(&long), super::UrlDigest::of(&bounded));
        assert_ne!(
            super::UrlDigest::of(AGENT_URL),
            super::UrlDigest::of(PERSONS_URL)
        );
        let digest = super::UrlDigest::of(AGENT_URL);
        assert_eq!(super::UrlDigest::parse(digest.as_str()), Some(digest));
        assert_eq!(super::UrlDigest::parse(AGENT_URL), None);
        assert_eq!(super::UrlDigest::parse(""), None);

        let vocabulary: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/golden/protocol4/vocabulary.json"
        ))
        .unwrap();
        assert_eq!(
            vocabulary["limits"]["url_chars"],
            serde_json::json!(super::TAB_URL_CHARS),
            "the digest bounds a URL as the adapter does"
        );
        assert_eq!(
            vocabulary["limits"]["title_chars"],
            serde_json::json!(super::TAB_TITLE_CHARS)
        );
    }

    #[test]
    fn the_same_tab_id_in_two_browsers_is_two_tabs() {
        let store = WorkspaceStore::default();
        let _chrome = workspace_with_tabs(&store, "browser_chrome", &[(5, "https://a.test/")]);
        let edge = workspace_with_tabs(&store, "browser_edge", &[]);
        let edge_lease = store.acquire(&edge).unwrap();
        assert!(edge_lease
            .add_tab("browser_edge", &physical(5, "https://b.test/"))
            .is_ok());
        drop(edge_lease);

        let second_chrome = workspace_with_tabs(&store, "browser_chrome", &[]);
        let lease = store.acquire(&second_chrome).unwrap();
        assert_eq!(
            lease
                .add_tab("browser_chrome", &physical(5, "https://a.test/"))
                .unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
        assert_eq!(
            lease
                .restore_tab(
                    "tab_restored_too",
                    "browser_chrome",
                    &physical(5, "https://a.test/")
                )
                .unwrap_err(),
            WorkspaceError::PhysicalTabOwned
        );
    }

    #[test]
    fn a_same_document_change_keeps_targets_and_expires_views() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
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
                    ..Default::default()
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

    fn control(locator: &str, name: &str) -> ObservedTarget {
        ObservedTarget {
            locator: locator.into(),
            role: "button".into(),
            name: name.into(),
            state: vec![],
            credential_class: false,
            ..Default::default()
        }
    }

    fn controls(count: usize) -> Vec<ObservedTarget> {
        (0..count)
            .map(|index| control(&format!("locator_{index}"), "Button"))
            .collect()
    }

    /// Reading the same control again in the same document returns the same handle, and the
    /// record takes the newer observation's name.
    #[test]
    fn a_control_keeps_its_handle_within_a_document() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let first = lease
            .register_targets(&tab, &[control("#email", "Email")])
            .unwrap();
        let second = lease
            .register_targets(
                &tab,
                &[
                    control("#email", "Work email"),
                    control("#email", "Work email"),
                ],
            )
            .unwrap();
        assert_eq!(first[0].0, second[0].0);
        assert_eq!(second[0].0, second[1].0);
        let context = store
            .stale_target_context(&workspace, first[0].0.as_str())
            .expect("the record is kept");
        assert_eq!(context.name, "Work email");
        assert_eq!(store.lock().workspaces[&workspace].targets.records.len(), 1);
        assert!(lease.resolve_target(first[0].0.as_str(), None).is_ok());
    }

    /// A new document generation mints a new handle, and records more than two generations
    /// behind the current one are forgotten, so their handles are unknown and refuse as stale.
    #[test]
    fn a_new_generation_mints_new_handles_and_old_ones_are_forgotten() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let mut current = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/0"))
            .unwrap();
        let mut handles = Vec::new();
        for generation in 0..4 {
            let registered = lease
                .register_targets(&current, &[control("#email", "Email")])
                .unwrap();
            handles.push(registered[0].0.clone());
            if generation < 3 {
                current = lease
                    .apply_landing(
                        &tab.handle,
                        &physical(1, &format!("https://example.com/{}", generation + 1)),
                    )
                    .unwrap();
            }
        }
        let distinct: std::collections::HashSet<_> = handles.iter().collect();
        assert_eq!(distinct.len(), 4);
        // Three advances later the first generation's record is gone; the two before the
        // current one are still known.
        assert_eq!(store.lock().workspaces[&workspace].targets.records.len(), 3);
        assert!(store
            .stale_target_context(&workspace, handles[0].as_str())
            .is_none());
        assert!(store
            .stale_target_context(&workspace, handles[1].as_str())
            .is_some());
        // The forgotten handle is told apart from the two kept: both left the document.
        for old in &handles[..3] {
            assert_eq!(
                lease.resolve_target(old.as_str(), None).unwrap_err(),
                WorkspaceError::StaleTarget {
                    reason: StaleReason::Navigated
                }
            );
        }
        assert!(lease.resolve_target(handles[3].as_str(), None).is_ok());
    }

    /// Past the record limit, earlier generations are evicted oldest first. The current
    /// generation is touched only when it alone exceeds the limit, and then the least recently
    /// registered records go and read as unknown.
    #[test]
    fn the_record_limit_evicts_earlier_generations_before_the_current_one() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let first = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/1"))
            .unwrap();
        let earlier = lease.register_targets(&first, &controls(1500)).unwrap();
        let second = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/2"))
            .unwrap();
        let current = lease.register_targets(&second, &controls(1500)).unwrap();
        let records =
            |store: &WorkspaceStore| store.lock().workspaces[&workspace].targets.records.len();
        assert_eq!(records(&store), super::TARGET_RECORD_LIMIT);
        assert!(current
            .iter()
            .all(|(handle, _)| lease.resolve_target(handle.as_str(), None).is_ok()));
        let survivors = earlier
            .iter()
            .filter(|(handle, _)| {
                store
                    .stale_target_context(&workspace, handle.as_str())
                    .is_some()
            })
            .count();
        assert_eq!(survivors, super::TARGET_RECORD_LIMIT - 1500);
        let index_len = store.lock().workspaces[&workspace].targets.index.len();
        assert_eq!(index_len, super::TARGET_RECORD_LIMIT);

        let third = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/3"))
            .unwrap();
        let crowded = lease.register_targets(&third, &controls(2500)).unwrap();
        assert_eq!(records(&store), super::TARGET_RECORD_LIMIT);
        let dropped = 2500 - super::TARGET_RECORD_LIMIT;
        for (position, (handle, _)) in crowded.iter().enumerate() {
            let resolved = lease.resolve_target(handle.as_str(), None);
            if position < dropped {
                assert_eq!(
                    resolved.unwrap_err(),
                    WorkspaceError::StaleTarget {
                        reason: StaleReason::Unknown
                    }
                );
            } else {
                assert!(resolved.is_ok());
            }
        }
    }

    /// A tab that went on to another document, a tab that closed, and a control the page
    /// reported removed each leave their handle refusing at once with their own word.
    #[test]
    fn a_retired_handle_refuses_with_the_word_that_says_why() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let mut current = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/0"))
            .unwrap();
        let removed = lease
            .register_targets(&current, &[control("#gone", "Gone")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, removed.as_str());
        let refused = |handle: &TargetHandle| lease.resolve_target(handle.as_str(), None);
        assert_eq!(
            refused(&removed).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Removed
            }
        );
        let context = store
            .stale_target_context(&workspace, removed.as_str())
            .expect("a removed control keeps its record for the recovery search");
        assert_eq!(context.name, "Gone");

        let navigated = lease
            .register_targets(&current, &[control("#old", "Old")])
            .unwrap()[0]
            .0
            .clone();
        for generation in 1..=3 {
            current = lease
                .apply_landing(
                    &tab.handle,
                    &physical(1, &format!("https://example.com/{generation}")),
                )
                .unwrap();
        }
        assert_eq!(
            refused(&navigated).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated
            }
        );

        let closed = lease
            .register_targets(&current, &[control("#open", "Open")])
            .unwrap()[0]
            .0
            .clone();
        lease.confirm_tab_closed(&tab.handle).unwrap();
        assert_eq!(
            refused(&closed).unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::TabClosed
            }
        );
        assert_eq!(
            lease
                .resolve_target("target_neverissued", None)
                .unwrap_err(),
            WorkspaceError::StaleTarget {
                reason: StaleReason::Unknown
            }
        );
    }

    /// Controls found while recovering from a stale handle get handles on that handle's tab, in
    /// the document the controls were read from.
    #[test]
    fn recovered_controls_get_handles_on_the_stale_handles_tab() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let stale = lease
            .register_targets(&tab, &[control("#old", "Old")])
            .unwrap()[0]
            .0
            .clone();
        let found = [control("#new", "Old"), control("#other", "Old")];

        let same = store.register_recovered(&workspace, stale.as_str(), true, &found);

        assert_eq!(same.len(), 2);
        let again = store.register_recovered(&workspace, stale.as_str(), true, &found[..1]);
        assert_eq!(
            again[0].0, same[0].0,
            "a control keeps its handle in one document"
        );
        for (handle, _) in &same {
            assert!(lease.resolve_target(handle.as_str(), Some(&tab)).is_ok());
        }
        assert!(store
            .register_recovered(&workspace, "target_neverissued", true, &found)
            .is_empty());

        let moved = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com/next"))
            .unwrap();
        assert!(
            store
                .register_recovered(&workspace, stale.as_str(), true, &found)
                .is_empty(),
            "controls of the old document are not registered on the new one"
        );
        let current = store.register_recovered(&workspace, stale.as_str(), false, &found);
        assert_eq!(current.len(), 2);
        assert!(lease
            .resolve_target(current[0].0.as_str(), Some(&moved))
            .is_ok());
    }

    /// Reading a removed control again brings its handle back, and a handle the page never
    /// reported removed is not retired by asking.
    #[test]
    fn reading_a_removed_control_again_revives_its_handle() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let handle = lease
            .register_targets(&tab, &[control("#save", "Save")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, "target_neverissued");
        assert!(lease.resolve_target(handle.as_str(), None).is_ok());
        store.mark_target_removed(&workspace, handle.as_str());
        assert!(lease.resolve_target(handle.as_str(), None).is_err());
        let again = lease
            .register_targets(&tab, &[control("#save", "Save")])
            .unwrap()[0]
            .0
            .clone();
        assert_eq!(again, handle);
        assert!(lease.resolve_target(handle.as_str(), None).is_ok());
        let book = &store.lock().workspaces[&workspace].targets;
        assert!(book.retired.is_empty() && book.retired_order.is_empty());
    }

    /// A control the page proved after a re-render comes back under its old locator, so reading it
    /// again keeps its handle and the book does not grow with every render.
    #[test]
    fn a_re_render_the_page_proves_keeps_one_handle_and_one_record() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let handle = lease
            .register_targets(&tab, &[control("#more", "More options")])
            .unwrap()[0]
            .0
            .clone();
        store.mark_target_removed(&workspace, handle.as_str());
        assert!(lease.resolve_target(handle.as_str(), None).is_err());

        for _ in 0..5 {
            let again = lease
                .register_targets(&tab, &[control("#more", "More options")])
                .unwrap()[0]
                .0
                .clone();
            assert_eq!(again, handle, "the same locator is the same handle");
        }

        assert!(
            lease.resolve_target(handle.as_str(), Some(&tab)).is_ok(),
            "the revived handle resolves"
        );
        let book = &store.lock().workspaces[&workspace].targets;
        assert_eq!(book.records.len(), 1);
        assert_eq!(book.index.len(), 1);
        assert!(book.retired.is_empty());
    }

    /// Tombstones are bounded: the oldest are dropped, and a dropped one has no reason left.
    #[test]
    fn tombstones_are_bounded_and_the_oldest_are_dropped() {
        let mut book = TargetBook::default();
        let handle = |position: usize| TargetHandle(format!("target_{position}"));
        for position in 0..super::TARGET_RECORD_LIMIT + 10 {
            book.retire(handle(position), StaleReason::Navigated);
        }
        assert_eq!(book.retired.len(), super::TARGET_RECORD_LIMIT);
        assert_eq!(book.retired_order.len(), super::TARGET_RECORD_LIMIT);
        assert_eq!(book.retired_reason(&handle(0)), None);
        assert_eq!(book.retired_reason(&handle(9)), None);
        assert_eq!(
            book.retired_reason(&handle(10)),
            Some(StaleReason::Navigated)
        );
        // The first reason a handle is given stays.
        book.retire(handle(10), StaleReason::Unknown);
        assert_eq!(
            book.retired_reason(&handle(10)),
            Some(StaleReason::Navigated)
        );
    }

    /// Every handle Ghostlight issues is its prefix and eight base32 characters, however many a
    /// workspace holds, and no two of a kind collide.
    #[test]
    fn handles_are_short_base32_and_unique_within_their_workspace() {
        let shaped = |handle: &str, prefix: &str| {
            let suffix = handle.strip_prefix(prefix).expect("the prefix");
            suffix.len() == HANDLE_SUFFIX_CHARS
                && suffix.bytes().all(|byte| HANDLE_ALPHABET.contains(&byte))
        };
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        assert!(
            shaped(tab.handle.as_str(), "tab_"),
            "{}",
            tab.handle.as_str()
        );
        let tab = lease
            .apply_landing(&tab.handle, &physical(1, "https://example.com"))
            .unwrap();
        let observed: Vec<ObservedTarget> = (0..200)
            .map(|index| ObservedTarget {
                locator: format!("locator_{index}"),
                role: "button".into(),
                name: format!("Button {index}"),
                state: vec![],
                credential_class: false,
                ..Default::default()
            })
            .collect();
        let targets = lease.register_targets(&tab, &observed).unwrap();
        let mut seen = std::collections::HashSet::new();
        for (handle, _) in &targets {
            assert!(shaped(handle.as_str(), "target_"), "{}", handle.as_str());
            assert!(seen.insert(handle.as_str().to_owned()), "a handle repeated");
        }
        let view = lease
            .register_view(
                &tab,
                ViewportGeometry {
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
                },
                800,
                600,
            )
            .unwrap();
        assert!(shaped(view.as_str(), "view_"), "{}", view.as_str());
        let snapshot = lease
            .register_snapshot(
                &tab,
                SnapshotScope {
                    root: None,
                    max_depth: 3,
                },
                serde_json::json!({}),
            )
            .unwrap();
        assert!(
            shaped(snapshot.as_str(), "snapshot_"),
            "{}",
            snapshot.as_str()
        );
        let image = lease
            .register_image(&tab, "image/png", "AAAA", 3)
            .unwrap()
            .expect("a small capture is kept");
        assert!(shaped(image.as_str(), "image_"), "{}", image.as_str());
        assert_eq!(short_handle("tab_", |_| false).len(), "tab_".len() + 8);
    }

    /// The dialog the browser reports open on a tab is remembered for that tab alone, and a
    /// report that it closed forgets it.
    #[test]
    fn a_dialog_is_remembered_for_its_own_tab_until_it_closes() {
        let store = WorkspaceStore::default();
        let workspace = workspace_with_tabs(
            &store,
            TEST_BROWSER,
            &[(1, "https://a.test/"), (2, "https://b.test/")],
        );
        assert_eq!(store.dialog_open(&workspace, 1), None);
        store.apply_browser_dialog(TEST_BROWSER, 1, Some(DialogKind::Prompt));
        assert_eq!(store.dialog_open(&workspace, 1), Some(DialogKind::Prompt));
        assert_eq!(store.dialog_open(&workspace, 2), None);
        store.apply_browser_dialog("another_browser", 2, Some(DialogKind::Alert));
        assert_eq!(store.dialog_open(&workspace, 2), None);
        store.apply_browser_dialog(TEST_BROWSER, 1, None);
        assert_eq!(store.dialog_open(&workspace, 1), None);
    }

    #[test]
    fn handles_are_owned_and_targets_expire_on_commit() {
        let store = WorkspaceStore::default();
        let first = store.admit("first".into(), IntakeChannel::Mcp);
        let second = store.admit("second".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&first).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
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
                    ..Default::default()
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
            WorkspaceError::StaleTarget {
                reason: StaleReason::Navigated
            }
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
        let caller = store.admit("caller".into(), IntakeChannel::Mcp);
        let labeled = store.admit("Codex".into(), IntakeChannel::Mcp);
        let unlabeled = store.admit(String::new(), IntakeChannel::Mcp);
        let handles = |workspace: &WorkspaceId, physical_id: u64| {
            let lease = store.acquire(workspace).unwrap();
            let tab = lease
                .add_tab(TEST_BROWSER, &physical(physical_id, "https://example.com"))
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
                        ..Default::default()
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
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(1, "https://example.com"))
            .unwrap();
        let targets = lease
            .register_targets(
                &tab,
                &[ObservedTarget {
                    locator: "l1".into(),
                    role: "Save my document".into(),
                    name: "untrusted name".into(),
                    state: vec![],
                    credential_class: false,
                    ..Default::default()
                }],
            )
            .unwrap();

        let resolved = lease
            .resolve_target(targets[0].0.as_str(), Some(&tab))
            .unwrap();
        assert_eq!(resolved.role, TargetRole::Control);
    }

    #[test]
    fn omission_selects_only_a_sole_tab() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
        let first = lease
            .add_tab(TEST_BROWSER, &physical(1, "about:blank"))
            .unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, first.handle);
        let second = lease
            .add_tab(TEST_BROWSER, &physical(2, "about:blank"))
            .unwrap();
        assert_eq!(
            lease.select_tab(None).unwrap_err(),
            WorkspaceError::AmbiguousTab,
            "the newest of several tabs is not a default"
        );
        assert_eq!(
            lease
                .select_tab(Some(second.handle.as_str()))
                .unwrap()
                .handle,
            second.handle
        );
        lease.confirm_tab_closed(&second.handle).unwrap();
        assert_eq!(lease.select_tab(None).unwrap().handle, first.handle);
    }

    #[test]
    fn decisive_close_receipt_tolerates_an_earlier_async_close_event() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
        store.apply_browser_close(TEST_BROWSER, 7);
        assert!(lease.confirm_tab_closed(&tab.handle).is_ok());
        assert_eq!(lease.select_tab(None).unwrap_err(), WorkspaceError::NoTab);
    }

    /// One landed tab carrying a snapshot, an image, a view and a target, with their handles.
    fn tab_with_everything(
        lease: &super::WorkspaceLease,
        physical_id: u64,
    ) -> (
        super::SelectedTab,
        super::ImageHandle,
        super::ViewHandle,
        TargetHandle,
    ) {
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(physical_id, "about:blank"))
            .unwrap();
        let tab = lease
            .apply_landing(
                &tab.handle,
                &physical(physical_id, "https://example.com/old"),
            )
            .unwrap();
        lease
            .register_snapshot(&tab, whole_document(), serde_json::json!({"old":true}))
            .unwrap();
        let image = lease
            .register_image(&tab, "image/jpeg", "AAAA", 3)
            .unwrap()
            .expect("under the ceiling");
        let view = lease.register_view(&tab, viewport(), 800, 600).unwrap();
        let target = lease
            .register_targets(&tab, &[control("#old", "Old")])
            .unwrap()[0]
            .0
            .clone();
        (tab, image, view, target)
    }

    fn whole_document() -> SnapshotScope {
        SnapshotScope {
            root: None,
            max_depth: 6,
        }
    }

    fn viewport() -> ViewportGeometry {
        ViewportGeometry {
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
        }
    }

    /// The person's close forgets a tab the way Ghostlight's own close does: nothing minted on it
    /// stays behind in the workspace.
    #[test]
    fn a_tab_the_person_closes_leaves_no_snapshot_or_image() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let _ = tab_with_everything(&lease, 7);

        store.apply_browser_close(TEST_BROWSER, 7);

        let state = store.lock();
        let kept = &state.workspaces[&workspace];
        assert!(kept.tabs.is_empty());
        assert!(kept.snapshots.is_empty(), "no snapshot outlives its tab");
        assert!(kept.images.is_empty(), "no image outlives its tab");
        assert!(kept.views.is_empty());
        assert!(kept.targets.records.is_empty());
    }

    /// A handle bound again, whether its tab closed first or not, starts with nothing its earlier
    /// page left: the old capture, diff base, view and target never answer for the new page, even
    /// once the new page reaches the generation the old one had.
    #[test]
    fn a_recreated_handle_never_serves_the_closed_pages_capture_or_diff() {
        for closed_first in [false, true] {
            let store = WorkspaceStore::default();
            let workspace = admit_test(&store);
            let lease = store.acquire(&workspace).unwrap();
            let (old, image, view, target) = tab_with_everything(&lease, 7);
            if closed_first {
                store.apply_browser_close(TEST_BROWSER, 7);
            }

            let restored = lease
                .restore_tab(
                    old.handle.as_str(),
                    TEST_BROWSER,
                    &physical(9, "about:blank"),
                )
                .unwrap();
            assert_eq!(restored.generation, 0);
            let landed = lease
                .apply_landing(&restored.handle, &physical(9, "https://example.com/new"))
                .unwrap();
            assert_eq!(landed.generation, old.generation, "{closed_first}");

            assert_eq!(
                lease.previous_snapshot(&landed, &whole_document()),
                None,
                "the closed page's tree is no diff base ({closed_first})"
            );
            assert_eq!(lease.take_image(image.as_str(), &landed), None);
            assert_eq!(
                lease
                    .resolve_view_point(view.as_str(), None, 1.0, 1.0)
                    .unwrap_err(),
                WorkspaceError::StaleView
            );
            assert_eq!(
                lease.resolve_target(target.as_str(), None).unwrap_err(),
                WorkspaceError::StaleTarget {
                    reason: StaleReason::TabClosed
                }
            );
        }
    }

    #[test]
    fn screenshot_views_map_coordinates_and_expire_on_commit() {
        let store = WorkspaceStore::default();
        let workspace = store.admit("test".into(), IntakeChannel::Mcp);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
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
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
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
        let other = admit_test(&store);
        let other_lease = store.acquire(&other).unwrap();
        let stranger = other_lease
            .add_tab(TEST_BROWSER, &physical(9, "about:blank"))
            .unwrap();
        assert_eq!(
            other_lease.previous_snapshot(&stranger, &whole),
            None,
            "a foreign workspace never sees another workspace's snapshots"
        );
    }

    #[test]
    fn captured_images_supersede_respect_the_ceiling_and_expire() {
        let store = WorkspaceStore::default();
        let workspace = admit_test(&store);
        let lease = store.acquire(&workspace).unwrap();
        let tab = lease
            .add_tab(TEST_BROWSER, &physical(7, "about:blank"))
            .unwrap();
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
        let workspace = admit_test(&store);
        let first_lease = store.acquire(&workspace).unwrap();

        // 1. Contention with deadline expiry:
        let cancellation = CancellationToken::default();
        let short_deadline = Instant::now() + Duration::from_millis(30);
        let expired = store.acquire_until(&workspace, short_deadline, &cancellation);
        assert_eq!(expired.err(), Some(WorkspaceError::Busy));

        // 2. Contention with cancellation:
        let cancelled = CancellationToken::default();
        cancelled.cancel(crate::language::outcome::CancelCause::Client);
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
