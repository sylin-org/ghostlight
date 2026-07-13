// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-session identity (ADR-0030 Decision 4: "identity model (adapter-minted GUID; core stays
//! PID-agnostic)" and its transport-side amendment). The thin ADAPTER mints an opaque, unguessable
//! [`SessionGuid`] and presents it in the adapter/control session-hello (`src/hub/handshake.rs`,
//! PINS.md SS1); the LOCAL accept layer (`src/transport/native/ipc.rs`) captures the connecting
//! peer's OS credential ([`PeerCred`]/[`PeerUser`]) purely for admission control, binding a GUID to
//! its minting peer via [`SessionRegistry::admit`]. Lives in `src/hub`, NEVER in `src/governance`
//! (a7): the governance core gains no pid/ancestor/GUID concept from any of these types.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

pub use ghostlight_transport::session_guid::SessionGuid;

/// H4 (ADR-0030 Decision 6; PINS.md SS9 forward guidance): the ONE operation on the shared,
/// opaque-keyed owned-tab map (`ServiceContext::owned_tabs`) that answers both "do I own it" and
/// "can I adopt it", with no per-session record and no cross-referencing between sessions' own
/// state -- `map.entry(tab_id).or_insert_with(|| guid.clone())` (first-touch adoption always
/// succeeds for a tabId nobody yet owns) compared against the caller's own `guid`. Lives here,
/// alongside the other pure identity types, NEVER in `src/governance` (a7: the core stays
/// handle-agnostic, naming no tabId type).
///
/// Returns `true` iff `guid` owns (or has just first-touch-adopted) `tab_id`; `false` iff a
/// DIFFERENT guid already owns it, in which case the caller (`transport::mcp::server`) must
/// refuse the call with the uniform, leak-free "unknown tab" result (ADR-0030 Decision 6) BEFORE
/// ever resolving the tab's host. A lone session's own guid therefore first-touch-adopts every
/// tab it ever names, so this is a byte-identical pass-through for a single live session
/// (ADR-0030 "Preserved invariants": all-open byte-identity).
pub fn owns_or_adopts_tab(
    owned_tabs: &Mutex<HashMap<i64, SessionGuid>>,
    guid: &SessionGuid,
    tab_id: i64,
) -> bool {
    !matches!(claim_tab(owned_tabs, guid, tab_id), TabClaim::Refused)
}

/// H7 (ADR-0030 Decision 6/7; PINS.md SS9): the same first-touch-adoption operation as
/// [`owns_or_adopts_tab`], but reporting WHICH of the three outcomes occurred rather than
/// collapsing "already owned" and "newly adopted" into one boolean. The group-request emit path
/// (`transport::mcp::server::check_tab_ownership`) needs exactly this distinction: it must ask
/// the extension to (re)group a session's tabs on a NEWLY adopted tab, never on every call that
/// merely touches a tab the session already owned (ADR-0030 Migration H7: "groups on request
/// only", not on every dispatch). [`owns_or_adopts_tab`] is reimplemented in terms of this
/// function so the two never drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabClaim {
    /// `guid` already owned `tab_id`; the map is unchanged.
    Owned,
    /// `tab_id` was unclaimed; `guid` just first-touch-adopted it (the map now reflects this).
    Adopted,
    /// A DIFFERENT guid already owns `tab_id`; the map is unchanged.
    Refused,
}

pub fn claim_tab(
    owned_tabs: &Mutex<HashMap<i64, SessionGuid>>,
    guid: &SessionGuid,
    tab_id: i64,
) -> TabClaim {
    let mut map = owned_tabs.lock().unwrap_or_else(PoisonError::into_inner);
    match map.get(&tab_id) {
        Some(owner) if owner == guid => TabClaim::Owned,
        Some(_) => TabClaim::Refused,
        None => {
            map.insert(tab_id, guid.clone());
            TabClaim::Adopted
        }
    }
}

/// ADR-0047 D5: [`claim_tab`] with liveness-aware refusal -- a DIFFERENT owner only refuses the
/// claim while that owner has a live session; a dead session's tab is reassigned to the claimer
/// (first-touch adoption from the dead, reported as `Adopted` so the group request fires). An
/// unowned tab and a same-owner tab behave exactly as [`claim_tab`]; only the "different owner"
/// arm consults `live_guids` (the reference count [`ServiceContext::live_guids`] holds).
pub fn claim_tab_live(
    owned_tabs: &Mutex<HashMap<i64, SessionGuid>>,
    live_guids: &Mutex<HashMap<String, usize>>,
    guid: &SessionGuid,
    tab_id: i64,
) -> TabClaim {
    let mut map = owned_tabs.lock().unwrap_or_else(PoisonError::into_inner);
    match map.get(&tab_id) {
        Some(owner) if owner == guid => TabClaim::Owned,
        Some(owner) => {
            let live = live_guids
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(owner.as_str())
                .copied()
                .unwrap_or(0)
                > 0;
            if live {
                TabClaim::Refused
            } else {
                map.insert(tab_id, guid.clone());
                TabClaim::Adopted
            }
        }
        None => {
            map.insert(tab_id, guid.clone());
            TabClaim::Adopted
        }
    }
}

/// H7 (ADR-0030 Decision 6/7; PINS.md SS9): the FULL set of tabIds `guid` currently owns, read
/// from the shared map -- not just the tabId that just triggered a [`TabClaim::Adopted`]. The
/// group-request emit path names this whole set on every emit, so the extension's group for this
/// session always mirrors the service's authoritative ownership record exactly (ADR-0030 Decision
/// 6: "cross-session isolation is authoritative in the SERVICE"). Sorted for a deterministic,
/// testable order; the wire and the extension attach no meaning to the order itself.
pub fn owned_tab_ids(
    owned_tabs: &Mutex<HashMap<i64, SessionGuid>>,
    guid: &SessionGuid,
) -> Vec<i64> {
    let map = owned_tabs.lock().unwrap_or_else(PoisonError::into_inner);
    let mut ids: Vec<i64> = map
        .iter()
        .filter(|(_, owner)| *owner == guid)
        .map(|(&tab_id, _)| tab_id)
        .collect();
    ids.sort_unstable();
    ids
}

/// The stable per-CLIENT presentation key (ADR-0066 D1/D3): the MCP `clientInfo.name` the agent
/// presented, or the literal `Ghostlight` when no clientInfo was captured. This -- not the
/// per-process GUID -- is what the extension keys its `clientKey -> chromeGroupId` map on, so every
/// session of the same client (sequential OR concurrent) presents into the SAME Chrome tab group
/// instead of minting a fresh one. Deliberately just the name: `clientInfo` carries no
/// per-workspace discriminator, and "one group per client" is the owner-chosen behavior; this is
/// the single place to refine the key if a client ever reports a richer identity. Carries no GUID,
/// so it never touches ADR-0030 Decision 4's redaction path.
pub fn client_key(client_name: Option<&str>) -> String {
    client_name.unwrap_or("Ghostlight").to_string()
}

/// The per-client Chrome group title (ADR-0047 D4 as amended by ADR-0066 D2): `"\u{1F47B} "`
/// (the ghost glyph + a space) followed by the [`client_key`]. ADR-0066 DROPS the ADR-0047 ` (2)`/
/// ` (3)` dedup: concurrent same-client sessions now SHARE one group, so they share one title, and
/// the title stays a pure function of the client identity -- which lets the extension reclaim a
/// group after a browser restart by stripping the glyph-plus-space prefix back to the clientKey
/// (ADR-0066 D4). The per-guid `titles` cache is kept only so the title is byte-stable across
/// reconnects (ADR-0047 D2). Embeds no GUID (ADR-0030 Decision 4 redaction path untouched).
pub fn session_title(
    titles: &Mutex<HashMap<String, String>>,
    guid: &SessionGuid,
    client_name: Option<&str>,
) -> String {
    let mut map = titles.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(existing) = map.get(guid.as_str()) {
        return existing.clone();
    }
    let title = format!("\u{1F47B} {}", client_key(client_name));
    map.insert(guid.as_str().to_string(), title.clone());
    title
}

/// The connecting peer's OS credential, captured by the LOCAL accept layer (`ipc::serve_adapters`)
/// purely for admission control and as the per-peer rate-limit key (ADR-0030 Decision 4
/// amendment). Lives in `src/hub`, NEVER in `src/governance` (a7). `user` is the peer's OS user
/// principal: the SID string on Windows, the uid on Unix. `pid` distinguishes processes for
/// logging only; admission compares `user`.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerCred {
    pub user: PeerUser,
    pub pid: u32,
}

/// Opaque OS-user principal; same-user comparison is `==`. `Hash` (PINS.md SS9): a later per-peer
/// quota table keyed by `PeerUser` requires it.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerUser(pub String);

/// Outcome of [`SessionRegistry::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Admitted,
    Refused,
}

/// The service's GUID -> bound-peer routing map (ADR-0030 Decision 2: per-session state lives in
/// `src/hub`). Keyed on the GUID's canonical string.
pub struct SessionRegistry {
    bindings: HashMap<String, PeerCred>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Admit a peer presenting a GUID. First presentation records the binding and returns
    /// [`Admission::Admitted`]. A re-presentation is `Admitted` iff the presenter is the SAME OS
    /// user as the bound peer (the sanctioned reuse path re-verifies same-user); a DIFFERENT user
    /// is [`Admission::Refused`] and the existing binding is left unchanged (ADR-0030 Decision 4:
    /// "refuse a GUID presented by a different peer, except the sanctioned reuse path which
    /// re-verifies same-user").
    pub fn admit(&mut self, guid: &SessionGuid, peer: &PeerCred) -> Admission {
        match self.bindings.get(guid.as_str()) {
            Some(bound) if bound.user == peer.user => Admission::Admitted,
            Some(_) => Admission::Refused,
            None => {
                self.bindings
                    .insert(guid.as_str().to_string(), peer.clone());
                Admission::Admitted
            }
        }
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// One admitted binding's Console-safe summary (PINS.md CS3/CS9, `docs/tasks/console`): the FIRST
/// 8 CHARACTERS of the GUID ONLY (never the full canonical string, ADR-0030 Decision 4), the
/// peer's OS process id (not secret), and its full current owned-tab set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub guid: String,
    pub pid: u32,
    pub owned_tab_ids: Vec<i64>,
}

/// Read-only snapshot for the Console's sessions view (PINS.md CS3). HONEST LIMITATION
/// (transcribed into the API response too, CS3): `registry`'s bindings are never pruned on
/// disconnect, so an entry here may no longer be live right now; pair this with
/// `ServiceContext.live_sessions` for an accurate CURRENT count. Acquires `registry`'s lock only
/// long enough to clone the (guid, PeerCred) pairs out, then drops it before acquiring
/// `owned_tabs`'s SEPARATE lock per entry (via the existing [`owned_tab_ids`]), so the two locks
/// are never held simultaneously.
pub fn live_session_summaries(
    registry: &Mutex<SessionRegistry>,
    owned_tabs: &Mutex<HashMap<i64, SessionGuid>>,
) -> Vec<SessionSummary> {
    let bindings: Vec<(String, PeerCred)> = {
        let reg = registry.lock().unwrap_or_else(PoisonError::into_inner);
        reg.bindings
            .iter()
            .map(|(g, c)| (g.clone(), c.clone()))
            .collect()
    };
    bindings
        .into_iter()
        .map(|(full_guid, cred)| {
            let guid = SessionGuid::parse(&full_guid)
                .expect("registry keys are valid canonical guids (only admit() inserts them)");
            SessionSummary {
                guid: full_guid[..8].to_string(),
                pid: cred.pid,
                owned_tab_ids: owned_tab_ids(owned_tabs, &guid),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admit_binds_first_presentation_and_allows_same_user_reuse() {
        let g = SessionGuid::mint();
        let mut registry = SessionRegistry::new();
        let a = PeerCred {
            user: PeerUser("user-A".into()),
            pid: 1,
        };
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
    }

    #[test]
    fn owns_or_adopts_tab_first_touch_then_refuses_a_different_guid() {
        let owned_tabs = Mutex::new(HashMap::new());
        let a = SessionGuid::mint();
        let b = SessionGuid::mint();
        assert!(
            owns_or_adopts_tab(&owned_tabs, &a, 5),
            "first-touch adoption succeeds for an unowned tabId"
        );
        assert!(
            owns_or_adopts_tab(&owned_tabs, &a, 5),
            "the SAME guid still owns the tabId it already adopted"
        );
        assert!(
            !owns_or_adopts_tab(&owned_tabs, &b, 5),
            "a DIFFERENT guid is refused; the existing binding is left untouched"
        );
        assert!(
            owns_or_adopts_tab(&owned_tabs, &a, 5),
            "the refused attempt above must not have disturbed A's ownership"
        );
    }

    /// H7 supplementary (not task-named; the pinned H7 assertions live in
    /// `tests/extension/grouping.test.js` for the extension-side grouping decision): `claim_tab`
    /// reports which of the three outcomes occurred, since `check_tab_ownership`'s group-request
    /// emit must fire on `Adopted` only, never on `Owned` or `Refused`.
    #[test]
    fn claim_tab_reports_owned_adopted_and_refused_distinctly() {
        let owned_tabs = Mutex::new(HashMap::new());
        let a = SessionGuid::mint();
        let b = SessionGuid::mint();
        assert_eq!(
            claim_tab(&owned_tabs, &a, 5),
            TabClaim::Adopted,
            "first touch of an unclaimed tabId is a fresh adoption"
        );
        assert_eq!(
            claim_tab(&owned_tabs, &a, 5),
            TabClaim::Owned,
            "the SAME guid re-touching its own tab is already-owned, not a new adoption"
        );
        assert_eq!(
            claim_tab(&owned_tabs, &b, 5),
            TabClaim::Refused,
            "a DIFFERENT guid touching an already-claimed tab is refused"
        );
    }

    /// H7 supplementary: `owned_tab_ids` reports the FULL, sorted, guid-filtered set -- not just
    /// the most recently touched tabId -- and never includes another session's tabs.
    #[test]
    fn owned_tab_ids_reports_the_full_sorted_set_for_one_guid_only() {
        let owned_tabs = Mutex::new(HashMap::new());
        let a = SessionGuid::mint();
        let b = SessionGuid::mint();
        assert_eq!(claim_tab(&owned_tabs, &a, 202), TabClaim::Adopted);
        assert_eq!(claim_tab(&owned_tabs, &a, 101), TabClaim::Adopted);
        assert_eq!(claim_tab(&owned_tabs, &b, 303), TabClaim::Adopted);
        assert_eq!(owned_tab_ids(&owned_tabs, &a), vec![101, 202]);
        assert_eq!(owned_tab_ids(&owned_tabs, &b), vec![303]);
    }

    /// ADR-0066 D1/D3: the clientKey is the plain client name (the reuse key the extension groups
    /// on), falling back to `Ghostlight` when no clientInfo was captured.
    #[test]
    fn client_key_is_the_client_name_with_ghostlight_fallback() {
        assert_eq!(client_key(Some("Claude Code")), "Claude Code");
        assert_eq!(client_key(Some("Cursor")), "Cursor");
        assert_eq!(client_key(None), "Ghostlight");
    }

    /// ADR-0066 D2 (amends ADR-0047 D4): the title is the ghost glyph + a space + the client name,
    /// with NO ` (2)`/` (3)` dedup -- two sessions of the SAME client get the SAME title (they now
    /// share one group), the title is byte-stable on a repeat call for a guid, and it falls back to
    /// `Ghostlight` when no client name was captured. The glyph-plus-space prefix is what the
    /// extension strips to reclaim a group by title after a browser restart (ADR-0066 D4).
    #[test]
    fn session_title_is_stable_per_client_without_dedup() {
        let t = Mutex::new(HashMap::new());
        let g1 = SessionGuid::mint();
        let g2 = SessionGuid::mint();
        let g3 = SessionGuid::mint();
        assert_eq!(
            session_title(&t, &g1, Some("Claude Code")),
            "\u{1F47B} Claude Code"
        );
        // A SECOND, distinct session of the same client gets the SAME title (no bump to (2)):
        // ADR-0066 shares one group across a client's sessions.
        assert_eq!(
            session_title(&t, &g2, Some("Claude Code")),
            "\u{1F47B} Claude Code"
        );
        // A repeat call for the SAME guid returns the cached title, unchanged.
        assert_eq!(
            session_title(&t, &g1, Some("Claude Code")),
            "\u{1F47B} Claude Code"
        );
        assert_eq!(session_title(&t, &g3, None), "\u{1F47B} Ghostlight");
        // The title is exactly the glyph, a space, then the clientKey -- the reclaim contract.
        assert_eq!(
            session_title(&t, &g1, Some("Claude Code")),
            format!("\u{1F47B} {}", client_key(Some("Claude Code")))
        );
    }

    /// ADR-0047 D5 (PINS P6): a tab owned by a session with no live connection is adoptable by a
    /// live session; once the new owner is live, the old owner cannot take it back.
    #[test]
    fn dead_owner_tab_is_adoptable_by_a_live_session() {
        let owned = Mutex::new(HashMap::new());
        let live = Mutex::new(HashMap::new());
        let a = SessionGuid::mint();
        let b = SessionGuid::mint();
        assert_eq!(claim_tab_live(&owned, &live, &a, 5), TabClaim::Adopted);
        // A holds no live connection (empty live map), so B adopts A's tab.
        assert_eq!(claim_tab_live(&owned, &live, &b, 5), TabClaim::Adopted);
        // Now B is live; A cannot take it back.
        live.lock().unwrap().insert(b.as_str().to_string(), 1);
        assert_eq!(claim_tab_live(&owned, &live, &a, 5), TabClaim::Refused);
    }

    /// ADR-0047 D5 (PINS P6): a live owner's tab stays refused to a different session.
    #[test]
    fn live_owner_tab_stays_refused() {
        let owned = Mutex::new(HashMap::new());
        let live = Mutex::new(HashMap::new());
        let a = SessionGuid::mint();
        let b = SessionGuid::mint();
        live.lock().unwrap().insert(a.as_str().to_string(), 1);
        assert_eq!(claim_tab_live(&owned, &live, &a, 5), TabClaim::Adopted);
        assert_eq!(claim_tab_live(&owned, &live, &b, 5), TabClaim::Refused);
    }

    #[test]
    fn admit_refuses_a_different_user() {
        let g = SessionGuid::mint();
        let mut registry = SessionRegistry::new();
        let a = PeerCred {
            user: PeerUser("user-A".into()),
            pid: 1,
        };
        let b = PeerCred {
            user: PeerUser("user-B".into()),
            pid: 2,
        };
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
        assert_eq!(registry.admit(&g, &b), Admission::Refused);
    }

    /// PINS.md CS3/CS9 (`docs/tasks/console`): `live_session_summaries` reports the TRUNCATED
    /// 8-character guid prefix (never the full canonical string), the admitted peer's pid, and
    /// the full owned-tab set for that guid, and never includes another guid's tabs.
    #[test]
    fn live_session_summaries_reports_truncated_guid_pid_and_owned_tabs() {
        let g = SessionGuid::mint();
        let other = SessionGuid::mint();
        let mut registry = SessionRegistry::new();
        let peer = PeerCred {
            user: PeerUser("user-A".into()),
            pid: 4242,
        };
        assert_eq!(registry.admit(&g, &peer), Admission::Admitted);
        let registry = Mutex::new(registry);

        let owned_tabs = Mutex::new(HashMap::new());
        assert_eq!(claim_tab(&owned_tabs, &g, 101), TabClaim::Adopted);
        assert_eq!(claim_tab(&owned_tabs, &g, 202), TabClaim::Adopted);
        assert_eq!(claim_tab(&owned_tabs, &other, 303), TabClaim::Adopted);

        let summaries = live_session_summaries(&registry, &owned_tabs);
        assert_eq!(summaries.len(), 1, "only the ADMITTED guid appears");

        let summary = &summaries[0];
        assert_eq!(summary.guid.len(), 8, "guid is truncated to 8 characters");
        assert_eq!(summary.guid, &g.as_str()[..8]);
        assert_ne!(
            summary.guid.len(),
            g.as_str().len(),
            "never the full canonical guid"
        );
        assert_eq!(summary.pid, 4242);
        assert_eq!(summary.owned_tab_ids, vec![101, 202]);
    }
}
