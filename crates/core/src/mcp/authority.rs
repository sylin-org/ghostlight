// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Atomic, identity-bound authority snapshots for MCP sessions (ADR-0080).

use crate::browser::polarity;
use crate::governance::config::reload::AuthorityInputs;
use crate::governance::config::Config;
use crate::governance::dispatch::Governance;
use crate::governance::enforcement::LocalPdp;
use crate::governance::manifest::source::LoadedPolicy;
use crate::governance::ports::AuditSink;
use std::sync::{Arc, Mutex, PoisonError};

/// One immutable authority view used from scheduling admission through audit completion.
pub struct AuthoritySnapshot {
    /// Resolved configuration for this epoch.
    pub config: Arc<Config>,
    /// Identity-bound governance facade built from the policy for this epoch.
    pub governance: Arc<Governance>,
    /// Resolved policy retained for reload comparison and presentation.
    pub policy: Arc<LoadedPolicy>,
    /// Monotonic epoch shared with the config store and command scheduler.
    pub epoch: u64,
}

/// A per-session atomic authority slot.
pub struct AuthorityStore {
    snapshot: Mutex<Arc<AuthoritySnapshot>>,
    recorder: Arc<dyn AuditSink>,
}

impl AuthorityStore {
    /// Build a session authority store from a complete config+policy input pair.
    pub fn new(inputs: &AuthorityInputs, recorder: Arc<dyn AuditSink>) -> Self {
        let governance = Arc::new(build_governance(&inputs.policy, Arc::clone(&recorder)));
        Self {
            snapshot: Mutex::new(Arc::new(AuthoritySnapshot {
                config: inputs.config.clone(),
                governance,
                policy: inputs.policy.clone(),
                epoch: inputs.epoch,
            })),
            recorder,
        }
    }

    /// Build a fixed authority store around an already constructed governance facade.
    ///
    /// This preserves the long-standing in-process test seam while production sessions use
    /// [`Self::new`] and [`Self::install`] for reloadable authority.
    #[cfg(test)]
    pub(crate) fn from_existing(inputs: &AuthorityInputs, governance: Arc<Governance>) -> Self {
        Self {
            snapshot: Mutex::new(Arc::new(AuthoritySnapshot {
                config: inputs.config.clone(),
                governance,
                policy: inputs.policy.clone(),
                epoch: inputs.epoch,
            })),
            recorder: Arc::new(crate::governance::ports::NullSink),
        }
    }

    /// Clone the complete current snapshot under one short lock.
    pub fn current(&self) -> Arc<AuthoritySnapshot> {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Install one complete input pair, preserving the session's captured client identity.
    pub fn install(&self, inputs: &AuthorityInputs) -> Arc<AuthoritySnapshot> {
        let previous = self.current();
        let governance = build_governance(&inputs.policy, Arc::clone(&self.recorder));
        if let Some(client) = previous.governance.current_client() {
            governance.set_client(&client.name, &client.version);
        }
        let next = Arc::new(AuthoritySnapshot {
            config: inputs.config.clone(),
            governance: Arc::new(governance),
            policy: inputs.policy.clone(),
            epoch: inputs.epoch,
        });
        *self.snapshot.lock().unwrap_or_else(PoisonError::into_inner) = next.clone();
        next
    }
}

/// Build the governance facade for one resolved policy.
pub(crate) fn build_governance(policy: &LoadedPolicy, recorder: Arc<dyn AuditSink>) -> Governance {
    match &policy.manifest {
        Some(manifest) => Governance::governed(
            Box::new(LocalPdp::new(polarity::evaluate_host)),
            recorder,
            manifest.grants.clone(),
            manifest.hash.clone(),
            manifest.mode,
        ),
        None => Governance::all_open(recorder),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::manifest::source::LoadedPolicy;
    use crate::governance::ports::NullSink;

    #[test]
    fn install_swaps_config_and_governance_under_one_epoch_and_keeps_client() {
        let config = Arc::new(Config::minimal());
        let policy = Arc::new(LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        });
        let store = AuthorityStore::new(
            &AuthorityInputs {
                config: config.clone(),
                policy: policy.clone(),
                epoch: 4,
            },
            Arc::new(NullSink),
        );
        store.current().governance.set_client("test-client", "1");

        let next_config = Arc::new(Config::minimal());
        let next = store.install(&AuthorityInputs {
            config: next_config.clone(),
            policy,
            epoch: 5,
        });

        assert_eq!(next.epoch, 5);
        assert!(Arc::ptr_eq(&next.config, &next_config));
        assert_eq!(
            next.governance.current_client().unwrap().name,
            "test-client"
        );
    }
}
