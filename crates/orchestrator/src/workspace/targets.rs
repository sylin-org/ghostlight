//! A workspace's target records: the book that names each control once per document, and the
//! bounds on how many records and tombstones it keeps.

use std::collections::{HashMap, VecDeque};

use super::{short_handle, SelectedTab, StaleReason, TabHandle, TabState, TargetHandle};
use crate::browser::adapter::ObservedTarget;
use crate::language::outcome::TargetRole;
use crate::language::HandleKind;

/// How many document generations of target records a tab keeps behind its current one.
///
/// A handle from one of these still resolves to the stale-target refusal with its last-known
/// context, so the agent is told what replaced it. Older records add memory and no information.
pub(super) const TARGET_GENERATIONS_KEPT_BEHIND: u64 = 2;

/// The most target records one workspace holds, and the most tombstones it keeps.
///
/// Reads mint records as fast as pages show controls, so memory needs a hard bound. Records of
/// earlier generations go first, oldest first. Only when current-generation records alone are
/// over the limit does the least recently registered one go, so a page that re-renders its
/// controls within one document stays bounded too.
pub(super) const TARGET_RECORD_LIMIT: usize = 2000;

#[derive(Clone, Debug)]
pub(super) struct TargetState {
    pub(super) tab: TabHandle,
    pub(super) generation: u64,
    pub(super) locator: String,
    pub(super) credential_class: bool,
    pub(super) role: TargetRole,
    /// Page-authored role string, retained so a stale handle can be re-queried by what the
    /// page itself called this control.
    pub(super) role_page: String,
    /// Bounded accessible name, retained for the same re-query purpose.
    pub(super) name: String,
    /// When the control was last registered, on the book's clock, so the least recently seen
    /// current-generation record is the one a full book drops.
    pub(super) registered_at: u64,
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
pub(super) struct TargetBook {
    pub(super) records: HashMap<TargetHandle, TargetState>,
    pub(super) index: HashMap<(TabHandle, u64, String), TargetHandle>,
    pub(super) retired: HashMap<TargetHandle, StaleReason>,
    pub(super) retired_order: VecDeque<TargetHandle>,
    pub(super) clock: u64,
}

impl TargetBook {
    pub(super) fn contains(&self, handle: &TargetHandle) -> bool {
        self.records.contains_key(handle)
    }

    pub(super) fn get(&self, handle: &TargetHandle) -> Option<&TargetState> {
        self.records.get(handle)
    }

    /// Why a handle was retired, if it was and the tombstone is still kept.
    pub(super) fn retired_reason(&self, handle: &TargetHandle) -> Option<StaleReason> {
        self.retired.get(handle).copied()
    }

    /// Record one observed control and return its handle.
    ///
    /// A control already recorded for the same tab, generation and locator keeps its handle and
    /// takes the new observation's role, name and credential class. A handle the page reported
    /// removed lives again when its control is read again. Otherwise a fresh handle is minted.
    pub(super) fn register(&mut self, tab: &SelectedTab, target: &ObservedTarget) -> TargetHandle {
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
        let handle = TargetHandle(short_handle(HandleKind::Target, |candidate| {
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
    pub(super) fn retire(&mut self, handle: TargetHandle, reason: StaleReason) {
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
    pub(super) fn revive(&mut self, handle: &TargetHandle) {
        self.retired.remove(handle);
        self.retired_order.retain(|kept| kept != handle);
    }

    /// Retire a handle the page reported removed. The record stays, so the handle can be
    /// revived and a refusal can still search for what it named.
    pub(super) fn mark_removed(&mut self, handle: &TargetHandle) {
        if self.records.contains_key(handle) {
            self.retire(handle.clone(), StaleReason::Removed);
        }
    }

    /// Remove every record that `doomed` selects, leaving each a tombstone with `reason` and
    /// keeping the index in step.
    pub(super) fn retire_where(
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
    pub(super) fn remove_tab(&mut self, tab: &TabHandle) {
        self.retire_where(StaleReason::TabClosed, |_, record| &record.tab == tab);
    }

    /// Retire a tab's records older than the generations kept behind its current one.
    pub(super) fn prune_tab(&mut self, tab: &TabHandle, current: u64) {
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
    pub(super) fn enforce_limit(&mut self, tabs: &HashMap<TabHandle, TabState>) {
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
