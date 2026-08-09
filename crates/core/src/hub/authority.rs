// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Atomic, service-global authority snapshots (ADR-0080, ADR-0096).

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
    /// Client-neutral governance facade built from the policy for this epoch.
    pub governance: Arc<Governance>,
    /// Resolved policy retained for reload comparison and presentation.
    pub policy: Arc<LoadedPolicy>,
    /// Monotonic epoch shared with the config store and command scheduler.
    pub epoch: u64,
}

/// A service-global atomic authority slot.
///
/// Client presentation is request context, not authority state. Installing a new snapshot never
/// carries client presentation forward from the replaced governance facade.
pub struct AuthorityStore {
    snapshot: Mutex<Arc<AuthoritySnapshot>>,
    recorder: Arc<dyn AuditSink>,
}

impl AuthorityStore {
    /// Build a service-global authority store from a complete config+policy input pair.
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

    /// Clone the complete current snapshot under one short lock.
    pub fn current(&self) -> Arc<AuthoritySnapshot> {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Install one complete input pair without inheriting mutable client presentation.
    pub fn install(&self, inputs: &AuthorityInputs) -> Arc<AuthoritySnapshot> {
        let governance = build_governance(&inputs.policy, Arc::clone(&self.recorder));
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
    fn install_swaps_config_and_governance_without_inheriting_client_state() {
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
        assert!(next.governance.current_client().is_none());
    }
}
