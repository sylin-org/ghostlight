//! Document-coverage questions the authority snapshot answers for browser work.

use super::AuthoritySnapshot;

impl AuthoritySnapshot {
    /// Prove that any supported HTTP document has the requested authority.
    #[must_use]
    pub fn permits_any_document(&self, requirements: super::CapabilitySet) -> bool {
        self.valid
            && self.layers.iter().all(|layer| {
                layer.never_touch.is_empty()
                    && layer
                        .manifest
                        .grants
                        .iter()
                        .zip(&layer.grant_hosts)
                        .any(|(grant, hosts)| {
                            hosts.allow.contains(&super::HostPattern::Any)
                                && hosts.deny.is_empty()
                                && requirements.is_subset_of(grant.allowed_set())
                        })
            })
    }
}
