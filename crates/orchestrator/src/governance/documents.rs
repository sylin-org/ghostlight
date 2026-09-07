//! Monotonic document-coverage policy, independent of browser routing and presentation.

use serde::{Deserialize, Serialize};

use super::{effective::LayerKind, manifest::Manifest, AuthoritySnapshot, AuthorityTier};

/// Registered setting for how enforced document exclusions affect requested work.
pub const HANDLING_KEY: &str = "content.frames.handling";
/// Registered setting for proactive human coverage notices.
pub const NOTICE_KEY: &str = "content.frames.notice";

/// Required document coverage, ordered from least to most restrictive.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Handling {
    /// Useful permitted observations and independently permitted targets may proceed.
    #[default]
    PermittedContent,
    /// All documents required by this operation must be accessible.
    CompleteOperation,
    /// Every document on this page must be accessible for the requested capability.
    CompletePage,
}

impl Handling {
    /// Decode the closed registered vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "permitted_content" => Some(Self::PermittedContent),
            "complete_operation" => Some(Self::CompleteOperation),
            "complete_page" => Some(Self::CompletePage),
            _ => None,
        }
    }
}

/// Proactive human notice intensity; structured result truth is independent.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Notice {
    /// Details remain available for explicit review.
    OnDemand,
    /// Show an indication when an exclusion affects the requested work.
    #[default]
    WhenAffected,
    /// Show an indication whenever the current page contains an exclusion.
    WhenExcluded,
}

impl Notice {
    /// Decode the closed registered vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "on_demand" => Some(Self::OnDemand),
            "when_affected" => Some(Self::WhenAffected),
            "when_excluded" => Some(Self::WhenExcluded),
            _ => None,
        }
    }
}

/// Effective document settings and the authors of each choice.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DocumentPolicy {
    /// How excluded documents affect admission.
    pub handling: Handling,
    /// How exclusions appear proactively to the local human.
    pub notice: Notice,
    /// Author of the effective handling requirement; absent means the product default.
    pub handling_source: Option<LayerKind>,
    /// Author of the effective notice requirement; absent means the product default.
    pub notice_source: Option<LayerKind>,
    /// Organization requirement that local authoring cannot relax.
    pub organization_handling: Option<Handling>,
    /// Organization notice requirement that local authoring cannot relax.
    pub organization_notice: Option<Notice>,
}

impl DocumentPolicy {
    /// Resolve authored choices monotonically, applying defaults only when no layer authors one.
    #[must_use]
    pub fn resolve(organization: Option<&Manifest>, user: Option<&Manifest>) -> Self {
        let mut resolved = Self::default();
        for (manifest, source) in [
            (organization, LayerKind::Organization),
            (user, LayerKind::User),
        ] {
            let Some(manifest) = manifest else { continue };
            if let Some(value) = manifest
                .string_setting(HANDLING_KEY)
                .and_then(Handling::parse)
            {
                if source == LayerKind::Organization {
                    resolved.organization_handling = Some(value);
                }
                if resolved.handling_source.is_none() || value > resolved.handling {
                    resolved.handling = value;
                    resolved.handling_source = Some(source);
                }
            }
            if let Some(value) = manifest.string_setting(NOTICE_KEY).and_then(Notice::parse) {
                if source == LayerKind::Organization {
                    resolved.organization_notice = Some(value);
                }
                if resolved.notice_source.is_none() || value > resolved.notice {
                    resolved.notice = value;
                    resolved.notice_source = Some(source);
                }
            }
        }
        resolved
    }
}

impl AuthoritySnapshot {
    /// Prove that any supported HTTP document has the requested authority.
    #[must_use]
    pub fn permits_any_document(&self, requirements: super::CapabilitySet) -> bool {
        self.valid
            && self.sacred_hosts.is_empty()
            && self
                .request_hosts
                .as_ref()
                .is_none_or(|hosts| hosts.iter().any(|host| host == "*"))
            && self
                .request_capabilities
                .is_none_or(|allowed| requirements.is_subset_of(allowed))
            && self.layers.iter().all(|layer| {
                layer.manifest.grants.iter().any(|grant| {
                    grant.hosts.allow.iter().any(|host| host == "*")
                        && grant.hosts.deny.is_empty()
                        && requirements.is_subset_of(grant.allowed_set())
                })
            })
    }

    /// Resolve document coverage from this invocation's immutable authority layers.
    #[must_use]
    pub fn document_policy(&self) -> DocumentPolicy {
        let source = |tier| {
            self.layers
                .iter()
                .find(|layer| layer.tier == tier)
                .map(|layer| &layer.manifest)
        };
        DocumentPolicy::resolve(source(AuthorityTier::Managed), source(AuthorityTier::User))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(handling: &str, notice: &str) -> Manifest {
        super::super::manifest::parse(&format!(r#"{{"schema":3,"name":"test","version":"1","grants":[],"config":[{{"key":"{HANDLING_KEY}","value":"{handling}","level":"mandatory"}},{{"key":"{NOTICE_KEY}","value":"{notice}","level":"mandatory"}}]}}"#), "test").unwrap()
    }

    #[test]
    fn authored_choices_tighten_and_quiet_local_choice_can_replace_the_default() {
        let organization = policy("complete_operation", "when_affected");
        let user = policy("permitted_content", "on_demand");
        let resolved = DocumentPolicy::resolve(Some(&organization), Some(&user));
        assert_eq!(resolved.handling, Handling::CompleteOperation);
        assert_eq!(resolved.notice, Notice::WhenAffected);
        assert_eq!(resolved.handling_source, Some(LayerKind::Organization));
        assert_eq!(
            DocumentPolicy::resolve(None, Some(&user)).notice,
            Notice::OnDemand
        );
        let stricter = policy("complete_page", "when_excluded");
        let resolved = DocumentPolicy::resolve(Some(&organization), Some(&stricter));
        assert_eq!(resolved.handling, Handling::CompletePage);
        assert_eq!(resolved.notice, Notice::WhenExcluded);
        assert_eq!(resolved.handling_source, Some(LayerKind::User));
        assert_eq!(
            DocumentPolicy::default().handling,
            Handling::PermittedContent
        );
    }
}
