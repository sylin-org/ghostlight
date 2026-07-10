// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Governance filesystem locations as an injectable unit (ADR-0056).
//!
//! [`GovernancePaths::production`] is the ONE place the fixed platform trust-anchor locations are
//! computed for the running service, and nothing overrides them at runtime, so the trust-anchor
//! non-overridability of ADR-0055 Decision 1 survives by construction. Tests and the
//! `ghostlight-lightbox` harness build a differently-wired instance via [`GovernancePaths::under`] --
//! a second composition root over the same library, never a runtime override of the production one.

use std::path::{Path, PathBuf};

/// The filesystem locations governance reads, injected at the composition root.
#[derive(Debug, Clone)]
pub struct GovernancePaths {
    /// The org policy file: the local, admin-provisioned manifest.
    pub org_policy: PathBuf,
    /// The managed:// bootstrap (`managed.json`), a sibling of the org policy file (ADR-0055).
    pub managed_bootstrap: PathBuf,
    /// The managed:// last-known-good cache; `None` when no data directory is available.
    pub managed_cache: Option<PathBuf>,
}

impl GovernancePaths {
    /// The real, fixed platform locations. The ONLY place these are computed in the running service;
    /// no flag, env var, or config key relocates them (ADR-0055 D1, ADR-0056 D1).
    pub fn production() -> Self {
        let org_policy = crate::governance::config::load::org_policy_path();
        let managed_bootstrap = match org_policy.parent() {
            Some(dir) => dir.join("managed.json"),
            None => org_policy.with_file_name("managed.json"),
        };
        let managed_cache = dirs::data_local_dir().map(|base| {
            base.join(ghostlight_transport::instance::Instance::resolve().dir_leaf())
                .join("managed-policy.bundle")
        });
        Self {
            org_policy,
            managed_bootstrap,
            managed_cache,
        }
    }

    /// A test / harness composition rooting every managed location under `dir` (ADR-0056): a second
    /// composition root, never a runtime override of the production one.
    pub fn under(dir: &Path) -> Self {
        Self {
            org_policy: dir.join("policy.json"),
            managed_bootstrap: dir.join("managed.json"),
            managed_cache: Some(dir.join("managed-policy.bundle")),
        }
    }
}
