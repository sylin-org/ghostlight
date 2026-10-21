//! Released tabs: the tabs a leaving workspace leaves behind, and the pool default reuse adopts
//! them from (ADR-0196 Decisions 8 and 9).

use url::Url;

use crate::browser::adapter::PhysicalTab;

use super::{AggregateState, UrlDigest, WorkspaceId, WorkspaceState, WorkspaceStore};

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
/// the same agent.
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
    /// The live tab that is still this released tab, by the URL rule a remembered tab follows
    /// when no extension life vouches for it ([`UrlDigest::confirmed_in`]).
    #[must_use]
    pub fn confirmed_in<'a>(&self, live: &'a [PhysicalTab]) -> Option<&'a PhysicalTab> {
        self.url.confirmed_in(self.physical_id, live)
    }
}

impl ReleasedTabs {
    /// The confirmed tabs a leaving workspace held. A pending binding is never closed through the
    /// browser, because its id may name a tab the person opened since.
    pub(super) fn from_state(state: WorkspaceState) -> Self {
        Self {
            physical_ids: state
                .confirmed_tabs()
                .map(|(_, tab)| tab.physical_id)
                .collect(),
            browser: state.browser,
        }
    }
}

/// One tab in the released pool.
#[derive(Debug)]
pub(super) struct PooledTab {
    /// The governed URL the tab held when its workspace released it.
    pub(super) url: String,
    /// The reuse key of the workspace that released the tab ([`Opener::reuse_key`]).
    ///
    /// Only a session with the same key may claim it: a client reuses only the tabs its own
    /// sessions kept (ADR-0201 Decision 2), as a courtesy between clients and never authority.
    /// The adopt puts the tab back in the claiming session's own group.
    pub(super) reuse_key: String,
    /// Whether the release close answered that the browser keeps the tab open.
    ///
    /// Until it does, the browser may still bind the tab to the dead workspace, and that pending
    /// close could still forget, unretain, or close the tab under whoever took it. So a tab whose
    /// cleanup has not answered is never claimed (ADR-0196 Decision 8).
    pub(super) kept_open: bool,
}

impl AggregateState {
    /// Remove `workspace` and return the confirmed tabs it held, recorded in the released pool.
    pub(super) fn release(&mut self, workspace: &WorkspaceId) -> ReleasedTabs {
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
    /// agent. A held tab sits on a destination governance refused, a tab with no governed URL
    /// never landed, and a pending binding may name a tab the person opened, so none of them is
    /// recorded at all. Nor is any tab of a workspace with no reuse key, which no one may reuse.
    pub(super) fn remember_released(&mut self, workspace: &WorkspaceState) {
        let (Some(browser), Some(reuse_key)) = (
            workspace.browser.as_ref(),
            workspace.named.reuse_key.as_ref(),
        ) else {
            return;
        };
        let tabs: Vec<_> = workspace
            .confirmed_tabs()
            .map(|(_, tab)| tab)
            .filter(|tab| tab.hold.is_none() && !tab.url.is_empty())
            .map(|tab| {
                (
                    tab.physical_id,
                    PooledTab {
                        url: tab.url.clone(),
                        reuse_key: reuse_key.clone(),
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
    pub(super) fn forget_released(&mut self, browser: &str, physical_id: u64) {
        if let Some(pool) = self.released.get_mut(browser) {
            pool.remove(&physical_id);
            if pool.is_empty() {
                self.released.remove(browser);
            }
        }
    }
}

impl WorkspaceStore {
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

    /// Take the released tab an open of `url` in `browser` by a session whose reuse key is
    /// `reuse_key` ([`Opener::reuse_key`]) should reuse, if there is one.
    ///
    /// Candidates are tabs Ghostlight opened, a released workspace with the same key left behind,
    /// and the browser kept open when that workspace's cleanup asked to close them, in this
    /// browser, on the same host and port. The key must match because a client reuses only the
    /// tabs its own sessions kept (ADR-0201 Decision 2), a courtesy between clients. An exact URL
    /// wins, then the lowest tab id. The claim removes the tab from the pool under the aggregate
    /// lock, so two sessions can never claim the same tab, and leaves another agent's tabs in the
    /// pool. The caller still confirms it in the live browser before using it
    /// ([`ReleasedTab::confirmed_in`]).
    #[must_use]
    pub fn claim_released(&self, browser: &str, reuse_key: &str, url: &str) -> Option<ReleasedTab> {
        let site = reusable_site(url)?;
        let mut state = self.lock();
        let claimed = state
            .released
            .get(browser)?
            .iter()
            .filter(|(_, released)| {
                released.kept_open
                    && released.reuse_key == reuse_key
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
}

/// The part of an ordinary web address that default reuse compares: host and explicit port.
///
/// Only `http` and `https` pages are reuse candidates; anything else has no comparable site.
fn reusable_site(url: &str) -> Option<(String, Option<u16>)> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some((
        crate::governance::host_pattern::url_host(&parsed)?,
        parsed.port(),
    ))
}
