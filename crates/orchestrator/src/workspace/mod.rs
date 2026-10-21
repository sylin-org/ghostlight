//! The workspace aggregate, opaque handles, ownership, document generations, and leases.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::browser::adapter::{
    BrowserReadiness, ObservedTarget, PhysicalPoint, PhysicalRectangle, PhysicalTab,
    ViewportGeometry,
};

use crate::agent::Agent;
use crate::governance::{CapabilitySet, Decision};
use crate::intake::session::SessionMarker;
use crate::language::handle::WORKSPACE_SUFFIX_CHARS;
use crate::language::outcome::{DialogKind, TargetRole};
use crate::language::HandleKind;
use crate::provenance::{ConnectionDetails, ConnectionEvidence};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

mod released;
mod stale;
mod targets;

use released::PooledTab;
pub use released::{ReleasedTab, ReleasedTabs};
pub use stale::StaleReason;
use targets::TargetBook;

/// The characters a handle's random part is drawn from: lowercase RFC 4648 base32.
const HANDLE_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

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
    /// This is the rule for every remembered tab, a pending binding and a released tab alike,
    /// whenever no extension life proves that the browser is still in the run the tab was known
    /// in ([`Remembered::confirmed_in`]). After a browser restart the id may name another tab,
    /// and a tab at another address is never taken for the one the workspace knew.
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

/// What a pending binding is remembered by until its browser confirms it (ADR-0203 Decision 1).
#[derive(Clone, Debug, Eq, PartialEq)]
struct Remembered {
    /// The digest of the URL the workspace last knew for the tab.
    url: UrlDigest,
    /// The extension life the browser was in when the binding was last confirmed, when the
    /// browser named one.
    life: Option<String>,
}

impl Remembered {
    /// The live tab that confirms a binding remembered as `physical_id`, when its browser lists
    /// `live` in the extension life `life`.
    ///
    /// In the life the binding remembers, the browser is still in the run that gave the tab its
    /// id, and a run never hands an id out twice, so the tab with that id is the binding's tab
    /// whatever its page shows now: a single-page app changes its address without a navigation
    /// the service sees. In any other life, or with no life on either side, only the URL rule
    /// confirms it ([`UrlDigest::confirmed_in`]).
    fn confirmed_in<'a>(
        &self,
        physical_id: u64,
        life: Option<&str>,
        live: &'a [PhysicalTab],
    ) -> Option<&'a PhysicalTab> {
        if self.life.is_some() && self.life.as_deref() == life {
            return live.iter().find(|tab| tab.tab_id == physical_id);
        }
        self.url.confirmed_in(physical_id, live)
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
    /// The extension life a pending binding remembers, if its browser named one.
    ///
    /// A confirmed binding carries none here: it is in the life its browser's connection reports
    /// now, which the browser port knows ([`crate::browser::BrowserPort::life_of`]).
    pub life: Option<String>,
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

/// Mint a handle of `kind` that `taken` does not already name: its prefix and
/// [`WORKSPACE_SUFFIX_CHARS`] random base32 characters.
///
/// Eight base32 characters are 40 random bits. Handles are checked against the workspace that
/// owns them, so they only have to be unique there and hard to confuse with a handle from an
/// earlier session, which a short counter would not be.
fn short_handle(kind: HandleKind, taken: impl Fn(&str) -> bool) -> String {
    loop {
        let mut random = [0_u8; WORKSPACE_SUFFIX_CHARS];
        getrandom::getrandom(&mut random).expect("the operating system supplies random bytes");
        let mut handle = String::from(kind.prefix());
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

/// How the client that opened a workspace names it, read once when the workspace opened
/// (ADR-0203 Decision 9). Presentation and a courtesy between clients, never authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opener {
    /// The opener's agent, as the window names it ([`ConnectionEvidence::details`]). It is the one
    /// name for the workspace's tab group, its session card and a refusal that names it as
    /// another session.
    pub agent: Agent,
    /// The key of the kept tabs the workspace may reuse and leaves for reuse
    /// ([`ConnectionEvidence::reuse_key`]); none when its opener's claim names no agent.
    pub reuse_key: Option<String>,
}

impl Opener {
    /// How `connection` names a workspace it opens.
    fn of(connection: &ConnectionEvidence) -> Self {
        Self {
            agent: connection.details().agent,
            reuse_key: connection.reuse_key(),
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
    /// Why the tab is held, while it shows a page governance refused.
    pub hold: Option<TabHold>,
}

/// Why a tab is held: the refusal of the page it shows (ADR-0203 Decisions 2.4 and 5.5).
///
/// A tab is held from the moment governance refuses the page it landed on or was found on, until
/// a landing the service sees clears it or the tab is closed. Every refusal on the held tab names
/// the refused page's host and records the refusal's reason, so the hold is never mistaken for
/// the person's Pause.
///
/// `denial` carries its policy attribution as indices into the authority that decided it, so it
/// is true only under that authority: a refusal under another one decides `url` again. The
/// address stays in memory and is never audited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabHold {
    /// The decision that refused the page.
    pub denial: Decision,
    /// The id of the authority snapshot that made `denial`.
    pub authority: String,
    /// What the refused landing required.
    pub requirements: CapabilitySet,
    /// The address of the refused page.
    pub url: String,
}

impl TabHold {
    /// The host of the refused page, lowercase, when its address has one.
    #[must_use]
    pub fn host(&self) -> Option<String> {
        Url::parse(&self.url)
            .ok()?
            .host_str()
            .map(str::to_ascii_lowercase)
    }
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

#[derive(Clone, Debug)]
struct ViewState {
    tab: TabHandle,
    generation: u64,
    viewport: ViewportGeometry,
    width: u32,
    height: u32,
}

impl ViewState {
    /// The refusal of coordinates that reach outside this view's image, which names its size.
    const fn outside(&self) -> WorkspaceError {
        WorkspaceError::OutsideImage {
            width: self.width,
            height: self.height,
        }
    }
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
    /// Why the tab is held, while it shows a page governance refused.
    hold: Option<TabHold>,
    /// What the binding is remembered by while it waits for its browser to confirm it, or none
    /// once it is confirmed (ADR-0203 Decision 1).
    ///
    /// A pending binding is inert. It takes no commit, readiness or dialog event, nothing selects
    /// it, it counts as nobody's tab, and it is never closed through the browser. It stays in the
    /// one tabs map only so its handle is never minted again.
    pending: Option<Remembered>,
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
            hold: None,
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
        self.pending.as_ref().map_or_else(
            || UrlDigest::of(&self.url),
            |remembered| remembered.url.clone(),
        )
    }
}

#[derive(Debug)]
struct WorkspaceState {
    /// The connection that opened this workspace.
    ///
    /// It names the workspace for its whole life: its session card, its tab group and the released
    /// tabs it may reuse. A command-line workspace outlives its connections (ADR-0106), so it cannot
    /// be named from the current ones, and a group is named for the agent that opened it
    /// (ADR-0066). A later connection joins `connections` and renames nothing.
    opener: Arc<ConnectionEvidence>,
    /// How the opener names this workspace, read once when it opened.
    named: Opener,
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
    /// The one recording this workspace holds (ADR-0203 Decision 6.7). It outlives a relay restart
    /// and goes with the workspace.
    recording: Option<HeldRecording>,
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

    /// The confirmed binding of `tab`, while it still shows the document `tab` was selected on.
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::StaleTab`] when the binding is gone or its page has navigated since.
    fn current(&self, tab: &SelectedTab) -> Result<&TabState, WorkspaceError> {
        let known = self.confirmed(&tab.handle)?;
        if known.generation != tab.generation {
            return Err(WorkspaceError::StaleTab);
        }
        Ok(known)
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
    /// it open, and only by a session of the agent that released it. An entry leaves it when the
    /// tab closes, when any page commits in it (the person or the page took it back), when a
    /// workspace takes it, and when its release close did anything but keep it. A browser that
    /// reconnects may have restarted and handed its ids out again, so a claimed tab is reused only
    /// where the live browser confirms it ([`ReleasedTab::confirmed_in`]).
    released: HashMap<String, HashMap<u64, PooledTab>>,
}

impl AggregateState {
    /// Whether a workspace other than `current` matches `holds`: it holds the handle a refusal
    /// is about, so the refusal says the handle belongs to another session.
    fn owned_elsewhere(
        &self,
        current: &WorkspaceId,
        holds: impl Fn(&WorkspaceState) -> bool,
    ) -> bool {
        self.workspaces
            .iter()
            .any(|(id, workspace)| id != current && holds(workspace))
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
    /// lists now, in the extension life `life` (ADR-0203 Decision 1).
    ///
    /// A binding is kept when a live tab confirms it ([`Remembered::confirmed_in`]) and no other
    /// workspace holds a confirmed binding of that tab. It then takes the live URL, title and
    /// readiness and keeps its handles, and the first one binds the workspace's browser. Its
    /// generation moves on when the live URL is not the one it was remembered by: the page may
    /// have navigated while the service could not see it, so no target or capture of the earlier
    /// page answers for the live one. Every other pending binding of `browser` is forgotten as
    /// closed.
    fn confirm(
        &mut self,
        current: &WorkspaceId,
        browser: &str,
        life: Option<&str>,
        live: &[PhysicalTab],
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .workspaces
            .get(current)
            .ok_or(WorkspaceError::UnknownWorkspace)?;
        let pending: Vec<(TabHandle, u64, Remembered)> = workspace
            .tabs
            .iter()
            .filter(|(_, tab)| tab.browser == browser)
            .filter_map(|(handle, tab)| {
                Some((handle.clone(), tab.physical_id, tab.pending.clone()?))
            })
            .collect();
        for (handle, physical_id, remembered) in pending {
            let shown = remembered
                .confirmed_in(physical_id, life, live)
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
                if UrlDigest::of(&shown.url) != remembered.url {
                    tab.generation = tab.generation.saturating_add(1);
                    workspace.targets.prune_tab(&handle, tab.generation);
                }
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

/// The one recording a workspace holds, kept until it is discarded, its browser no longer has it,
/// or the workspace is released (ADR-0203 Decision 6.7).
///
/// The browser keeps recordings without knowing workspaces, so the workspace remembers which one
/// is its own and names it on every recording command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldRecording {
    /// The browser that holds the recording.
    pub browser: String,
    /// The recording's id in that browser.
    pub id: String,
    /// Whether a save has delivered it. A saved recording may be replaced by the next start; one
    /// never saved holds the workspace's slot until it is saved or discarded.
    pub saved: bool,
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
                named: Opener::of(&connection),
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
                recording: None,
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
    /// open on its host from a session of the same agent only once `settle_released` reports
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

    /// Mark every confirmed binding in `browser` pending, after a gap in what the service knows of
    /// that browser: its stream ended, or a new connection replaced it (ADR-0203 Decision 1).
    ///
    /// A browser that comes back may be a restarted one, and a restarted browser hands its tab ids
    /// out again, so a remembered id could name a tab the person just opened. Each binding is then
    /// remembered by its id, the URL the workspace last knew and `life`, the extension life the
    /// ended connection reported, until the browser lists its tabs again. The released pool keeps
    /// its tabs: a reuse confirms each one by the URL rule.
    pub fn apply_browser_disconnect(&self, browser: &str, life: Option<&str>) {
        let mut state = self.lock();
        for tab in state
            .workspaces
            .values_mut()
            .flat_map(|workspace| workspace.tabs.values_mut())
            .filter(|tab| tab.is_confirmed() && tab.browser == browser)
        {
            tab.pending = Some(Remembered {
                url: UrlDigest::of(&tab.url),
                life: life.map(str::to_owned),
            });
        }
    }

    /// Bring back one binding a resume token remembered, pending until `browser` confirms it.
    /// `life` is the extension life the token says that browser was in, if it says one.
    ///
    /// A handle the workspace already holds is left as it is. The binding counts as nobody's tab
    /// until its browser lists a tab that confirms it: in that life, the tab with its id; in any
    /// other, the tab with its id at the URL it is remembered by.
    pub fn remember_tab(
        &self,
        workspace: &WorkspaceId,
        handle: &str,
        browser: &str,
        physical_id: u64,
        url: UrlDigest,
        life: Option<&str>,
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
                pending: Some(Remembered {
                    url,
                    life: life.map(str::to_owned),
                }),
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

    /// How the client that opened a workspace names it: the agent that titles its tab group and
    /// the key of the released tabs it may reuse. Presentation only, never routing or authority.
    pub fn opener(&self, workspace: &WorkspaceId) -> Result<Opener, WorkspaceError> {
        self.lock()
            .workspaces
            .get(workspace)
            .map(|state| state.named.clone())
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

    /// The agent of another admitted workspace that owns the requested tab handle, as the
    /// window names it. None when no other workspace owns it, or when its agent is the unknown
    /// agent: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn tab_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TabHandle(requested.into());
        self.owner_name(current, |workspace| workspace.confirmed(&handle).is_ok())
    }

    /// The agent of another admitted workspace that owns the requested target handle, as the
    /// window names it. None when no other workspace owns it, or when its agent is the unknown
    /// agent: a refusal never names another session by its workspace id.
    #[must_use]
    pub fn target_owner(&self, current: &WorkspaceId, requested: &str) -> Option<String> {
        let handle = TargetHandle(requested.into());
        self.owner_name(current, |workspace| workspace.targets.contains(&handle))
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

    /// Name the other workspace that holds a handle by its opener's agent, the way the window
    /// names it ([`Opener::agent`]).
    ///
    /// A workspace id is an internal key. No tool takes one, so naming it would only tell the
    /// caller where another session lives (ADR-0196 Decision 8). An owner whose agent nothing
    /// names, the unknown agent, is therefore not attributed at all, and the refusal falls back
    /// to the generic sentence (ADR-0172 Decision 4). None when no other workspace holds it.
    fn owner_name(
        &self,
        current: &WorkspaceId,
        holds: impl Fn(&WorkspaceState) -> bool,
    ) -> Option<String> {
        self.lock()
            .workspaces
            .iter()
            .find(|(id, workspace)| *id != current && holds(workspace))
            .map(|(_, workspace)| workspace.named.agent.clone())
            .filter(|agent| *agent != Agent::unknown())
            .map(|agent| agent.name)
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

    /// Let `workspace` leave the browser it works in, after that browser left and the agent named
    /// another connected one (W-D5). The caller holds the workspace's lease.
    ///
    /// Every binding goes, so the old browser's tabs read as no longer open in this session, and
    /// the next tab binds the browser the agent named. The tabs themselves stay where they are.
    pub fn leave_browser(&self, workspace: &str) {
        if let Some(state) = self.lock().workspaces.get_mut(workspace) {
            let handles: Vec<TabHandle> = state.tabs.keys().cloned().collect();
            for handle in handles {
                state.forget_tab(&handle);
            }
            state.browser = None;
        }
    }

    /// The recording `workspace` holds in its browser, if any.
    ///
    /// A workspace lives in one browser, so a recording another browser held is not this
    /// workspace's to reach any more and reads as none.
    #[must_use]
    pub fn recording(&self, workspace: &WorkspaceId) -> Option<HeldRecording> {
        let state = self.lock();
        let held = state.workspaces.get(workspace)?;
        held.recording.clone().filter(|recording| {
            held.browser
                .as_deref()
                .is_none_or(|browser| browser == recording.browser)
        })
    }

    /// Remember that `workspace` holds the recording `id` in `browser`, not yet saved.
    pub fn hold_recording(&self, workspace: &WorkspaceId, browser: &str, id: &str) {
        if let Some(held) = self.lock().workspaces.get_mut(workspace) {
            held.recording = Some(HeldRecording {
                browser: browser.to_owned(),
                id: id.to_owned(),
                saved: false,
            });
        }
    }

    /// Remember that a save delivered the recording `workspace` holds.
    pub fn mark_recording_saved(&self, workspace: &WorkspaceId) {
        if let Some(recording) = self
            .lock()
            .workspaces
            .get_mut(workspace)
            .and_then(|held| held.recording.as_mut())
        {
            recording.saved = true;
        }
    }

    /// Forget the recording `workspace` held: it was discarded, or its browser no longer has it.
    pub fn forget_recording(&self, workspace: &WorkspaceId) {
        if let Some(held) = self.lock().workspaces.get_mut(workspace) {
            held.recording = None;
        }
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
                        life: tab
                            .pending
                            .as_ref()
                            .and_then(|remembered| remembered.life.clone()),
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
    /// that browser lists now, in `life`, the extension life its connection reported once the
    /// listing answered (ADR-0203 Decision 1). Only the call that holds the workspace's lease
    /// confirms, before it selects a tab and again before a crossing whose tab went pending while
    /// it ran.
    ///
    /// A binding the browser still shows, and that no other workspace holds, is kept: in the life
    /// it remembers, the tab with its id; in any other, the tab with its id at the URL it is
    /// remembered by. It takes the live URL, title and readiness and keeps its handles, and its
    /// generation moves on when that URL is not the one it was remembered by. The first one binds
    /// the workspace's browser. Every other pending binding of `browser` is forgotten as closed,
    /// and its handle then names no open tab.
    pub fn confirm(
        &self,
        workspace: &WorkspaceId,
        browser: &str,
        life: Option<&str>,
        live: &[PhysicalTab],
    ) -> Result<(), WorkspaceError> {
        self.lock().confirm(workspace, browser, life, live)
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
                tab.hold = None;
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
            .find(|tab| tab.is_confirmed_tab(browser, physical_id) && tab.hold.is_none())
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
        crate::sync::lock(&self.inner)
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
    /// This lease's workspace in the locked `state`.
    fn workspace_in<'s>(
        &self,
        state: &'s AggregateState,
    ) -> Result<&'s WorkspaceState, WorkspaceError> {
        state
            .workspaces
            .get(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

    /// This lease's workspace in the locked `state`, to change.
    fn workspace_in_mut<'s>(
        &self,
        state: &'s mut AggregateState,
    ) -> Result<&'s mut WorkspaceState, WorkspaceError> {
        state
            .workspaces
            .get_mut(&self.workspace)
            .ok_or(WorkspaceError::UnknownWorkspace)
    }

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
        let workspace = self.workspace_in_mut(&mut state)?;
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
        if let Some(hold) = tab.hold {
            return Err(WorkspaceError::Held(hold));
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
        let workspace = self.workspace_in(&state)?;
        if let Some(requested) = requested {
            let handle = TabHandle(requested.into());
            if let Ok(tab) = workspace.confirmed(&handle) {
                return Ok(selected(&handle, tab));
            }
            let owned_elsewhere = state.owned_elsewhere(&self.workspace, |candidate| {
                candidate.confirmed(&handle).is_ok()
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
            TabHandle(short_handle(HandleKind::Tab, |candidate| {
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
        let workspace = self.workspace_in_mut(&mut state)?;
        let known = workspace.confirmed_mut(handle)?;
        if known.physical_id != tab.tab_id {
            return Err(WorkspaceError::StaleTab);
        }
        known.generation = known.generation.saturating_add(1);
        known.url.clone_from(&tab.url);
        known.title = bounded(&tab.title, TAB_TITLE_CHARS);
        known.readiness = tab.readiness;
        known.hold = None;
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
        let workspace = self.workspace_in_mut(&mut state)?;
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
        let workspace = self.workspace_in_mut(&mut state)?;
        let tab = workspace.confirmed_mut(handle)?;
        tab.readiness = readiness;
        Ok(())
    }

    /// Hold a tab whose page governance refused, after a landing or a read found it there.
    ///
    /// Only [`Self::select_tab_to_leave`] selects the tab while it is held, and a governed landing
    /// clears the hold. `hold` is the refusal every later refusal on the tab repeats.
    pub fn hold_tab(&self, handle: &TabHandle, hold: TabHold) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = self.workspace_in_mut(&mut state)?;
        workspace.confirmed_mut(handle)?.hold = Some(hold);
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
        let workspace = self.workspace_in_mut(&mut state)?;
        let generation = workspace.current(tab)?.generation;
        workspace
            .snapshots
            .retain(|_, snapshot| snapshot.tab != tab.handle);
        let handle = SnapshotHandle(short_handle(HandleKind::Snapshot, |candidate| {
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
        let known = workspace.current(tab).ok()?;
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
        let workspace = self.workspace_in_mut(&mut state)?;
        let generation = workspace.current(tab)?.generation;
        workspace.images.retain(|_, image| image.tab != tab.handle);
        let handle = ImageHandle(short_handle(HandleKind::Image, |candidate| {
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
        let known = workspace.current(tab).ok()?;
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
        let workspace = self.workspace_in_mut(&mut state)?;
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
        if state.owned_elsewhere(&self.workspace, |candidate| {
            candidate.confirmed(&handle).is_ok()
        }) {
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
        let workspace = self.workspace_in_mut(&mut state)?;
        workspace.current(tab)?;
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
        let workspace = self.workspace_in(&state)?;
        let handle = TargetHandle(requested.into());
        if let Some(reason) = workspace.targets.retired_reason(&handle) {
            return Err(WorkspaceError::StaleTarget { reason });
        }
        let Some(target) = workspace.targets.get(&handle) else {
            let owned_elsewhere = state.owned_elsewhere(&self.workspace, |candidate| {
                candidate.targets.contains(&handle)
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
                return Err(WorkspaceError::HandleOnOtherTab {
                    tab: target.tab.clone(),
                });
            }
        }
        if let Some(hold) = &tab.hold {
            return Err(WorkspaceError::Held(hold.clone()));
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
        let workspace = self.workspace_in_mut(&mut state)?;
        workspace.current(tab)?;
        workspace.views.retain(|_, view| view.tab != tab.handle);
        let handle = ViewHandle(short_handle(HandleKind::View, |candidate| {
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

    /// The view `requested` names, with the view a result reports: owned by this workspace, on
    /// its tab's current document, on `selected_tab` when the call named one, and with a usable
    /// scale.
    ///
    /// # Errors
    ///
    /// [`WorkspaceError::NotOwnedView`] when another workspace holds the handle,
    /// [`WorkspaceError::StaleView`] when the view or its document is gone or its scale is
    /// unusable, and [`WorkspaceError::HandleOnOtherTab`] when it is on another tab than the call
    /// named.
    fn resolve_view<'s>(
        &self,
        state: &'s AggregateState,
        requested: &str,
        selected_tab: Option<&SelectedTab>,
    ) -> Result<(&'s ViewState, SelectedView), WorkspaceError> {
        let workspace = self.workspace_in(state)?;
        let handle = ViewHandle(requested.into());
        let Some(view) = workspace.views.get(&handle) else {
            let owned_elsewhere = state.owned_elsewhere(&self.workspace, |candidate| {
                candidate.views.contains_key(&handle)
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
            return Err(WorkspaceError::HandleOnOtherTab {
                tab: view.tab.clone(),
            });
        }
        if view.viewport.output_scale <= 0.0 {
            return Err(WorkspaceError::StaleView);
        }
        let selected = SelectedView {
            handle,
            tab: view.tab.clone(),
            viewport: view.viewport,
            width: view.width,
            height: view.height,
        };
        Ok((view, selected))
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
        let (view, selected) = self.resolve_view(&state, requested, selected_tab)?;
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= f64::from(view.width)
            || y >= f64::from(view.height)
        {
            return Err(view.outside());
        }
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
        let (view, selected) = self.resolve_view(&state, requested, selected_tab)?;
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
        {
            return Err(view.outside());
        }
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
        let workspace = self.workspace_in_mut(&mut state)?;
        workspace.views.retain(|_, view| &view.tab != tab);
        Ok(())
    }

    /// Invalidate every view whose physical tab belongs to a resized browser window.
    pub fn invalidate_views_for_physical(
        &self,
        physical_ids: &[u64],
    ) -> Result<(), WorkspaceError> {
        let mut state = self.store.lock();
        let workspace = self.workspace_in_mut(&mut state)?;
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
        hold: tab.hold.clone(),
    }
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

/// Workspace invariant or recovery failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
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
    /// The target or view handle belongs to another of this workspace's tabs than the one the
    /// call named.
    #[error("handle belongs to another tab of this workspace")]
    HandleOnOtherTab {
        /// The tab the handle belongs to.
        tab: TabHandle,
    },
    /// The fields and submit target of one fill are on more than one tab.
    #[error("fill targets are on several tabs")]
    FieldsOnSeveralTabs,
    /// An image coordinate or rectangle is not finite or reaches outside the captured image.
    #[error("view coordinate is outside the captured image")]
    OutsideImage {
        /// The image's width in pixels.
        width: u32,
        /// The image's height in pixels.
        height: u32,
    },
    /// The tab shows a page governance refused, for the reason the hold keeps.
    #[error("tab is held on a page governance refused")]
    Held(TabHold),
    /// Physical tab is already controlled by another workspace.
    #[error("physical tab is already controlled")]
    PhysicalTabOwned,
}

#[cfg(test)]
mod tests;
