// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Session-pinned browser workspace targets.
//!
//! A workspace is placement, not authorization: it records the browser slot and native window
//! chosen for topology operations that do not yet carry a tab id. Tab ownership, managed-surface
//! checks, and governance remain the enforcement boundaries. Native window ids never leave the
//! browser adapter boundary or become model-facing identifiers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

/// One MCP session's selected browser placement target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WorkspaceTarget {
    /// Stable service-local browser slot.
    pub(super) browser_slot: u32,
    /// Adapter-native window id, valid only for the current browser-process generation.
    pub(super) native_window_id: i64,
}

/// Process-memory registry of MCP session workspace pins.
#[derive(Clone, Default)]
pub(super) struct WorkspaceRegistry {
    targets: Arc<Mutex<HashMap<String, WorkspaceTarget>>>,
}

impl WorkspaceRegistry {
    /// Return the target already pinned for `guid`, if any.
    pub(super) fn get(&self, guid: &str) -> Option<WorkspaceTarget> {
        self.targets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(guid)
            .copied()
    }

    /// Pin `guid` to its first observed target and return the authoritative target.
    ///
    /// First-wins preserves session stability if duplicate initial topology requests race. The
    /// client-topology scheduler serializes production calls, so a differing later candidate is a
    /// defensive case rather than an ordinary path.
    pub(super) fn pin(&self, guid: &str, target: WorkspaceTarget) -> WorkspaceTarget {
        *self
            .targets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(guid.to_string())
            .or_insert(target)
    }

    /// Remove pins whose native window ids were invalidated by a browser-process restart.
    pub(super) fn clear_browser(&self, browser_slot: u32) {
        self.targets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, target| target.browser_slot != browser_slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_pin_is_first_wins() {
        let registry = WorkspaceRegistry::default();
        let first = WorkspaceTarget {
            browser_slot: 1,
            native_window_id: 7,
        };
        let later = WorkspaceTarget {
            browser_slot: 1,
            native_window_id: 9,
        };
        assert_eq!(registry.pin("session", first), first);
        assert_eq!(registry.pin("session", later), first);
        assert_eq!(registry.get("session"), Some(first));
    }

    #[test]
    fn browser_restart_clears_only_its_pins() {
        let registry = WorkspaceRegistry::default();
        registry.pin(
            "a",
            WorkspaceTarget {
                browser_slot: 1,
                native_window_id: 7,
            },
        );
        let other = WorkspaceTarget {
            browser_slot: 2,
            native_window_id: 8,
        };
        registry.pin("b", other);
        registry.clear_browser(1);
        assert_eq!(registry.get("a"), None);
        assert_eq!(registry.get("b"), Some(other));
    }
}
