// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Service-owned browser-profile bindings for unaddressed workspace calls.
//!
//! A binding records only which connected browser profile owns a workspace's Chrome mechanism.
//! Native windows, groups, and tabs remain browser-shore topology. Tab ownership, managed-surface
//! checks, and governance remain the enforcement boundaries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

/// Process-memory registry of workspace-to-browser-profile bindings.
#[derive(Clone, Default)]
pub(super) struct WorkspaceBindings {
    slots: Arc<Mutex<HashMap<String, u32>>>,
}

impl WorkspaceBindings {
    /// Return the browser slot already bound to the browser-wire `guid`, if any.
    pub(super) fn get(&self, guid: &str) -> Option<u32> {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(guid)
            .copied()
    }

    /// Bind the browser-wire `guid` to its first successful browser slot and return it.
    ///
    /// First-wins preserves browser-profile stability if duplicate initial topology requests race.
    /// Chrome-native window placement is deliberately absent from this state.
    pub(super) fn bind(&self, guid: &str, browser_slot: u32) -> u32 {
        *self
            .slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(guid.to_string())
            .or_insert(browser_slot)
    }

    /// Remove one retired workspace's browser-profile binding.
    pub(super) fn remove(&self, workspace: &str) {
        self.slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(workspace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_binding_is_first_wins() {
        let registry = WorkspaceBindings::default();
        assert_eq!(registry.bind("workspace", 1), 1);
        assert_eq!(registry.bind("workspace", 2), 1);
        assert_eq!(registry.get("workspace"), Some(1));
    }

    #[test]
    fn retiring_a_workspace_removes_only_its_binding() {
        let registry = WorkspaceBindings::default();
        registry.bind("a", 1);
        registry.bind("b", 2);
        registry.remove("a");
        assert_eq!(registry.get("a"), None);
        assert_eq!(registry.get("b"), Some(2));
    }
}
