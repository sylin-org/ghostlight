// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
// See docs/licenses/LicenseRef-Ghostlight-Commercial.txt.

//! Authority snapshots, final-boundary admission, runtime controls, and minimized audit intent.

pub mod effective;
pub mod inspection;
pub mod managed;
pub mod manifest;
pub mod paths;

use std::collections::VecDeque;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ghostlight_bridge::browser::{RuntimeControlIntent, RuntimeControlState};
use ghostlight_bridge::service::IntakeChannel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

use crate::language::{outcome::Observed, RequestRestrictions};

const RUNTIME_ACTIVE: u8 = 0;
const RUNTIME_HOLD: u8 = 1;
const RUNTIME_ATTENTION: u8 = 2;
const RUNTIME_END: u8 = 3;
const DENIAL_ATTENTION_MATCHING_WINDOW_MS: u64 = 60_000;
const DENIAL_ATTENTION_ALL_WINDOW_MS: u64 = 120_000;
const DENIAL_ATTENTION_MATCHING_THRESHOLD: usize = 3;
const DENIAL_ATTENTION_ALL_THRESHOLD: usize = 5;
const DENIAL_ATTENTION_HISTORY_LIMIT: usize = 512;

/// One independent governed browser capability fact.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Observe browser facts.
    Read,
    /// Cause ordinary browser interaction.
    Action,
    /// Enter non-credential user data.
    Write,
    /// Commit a consequential submission.
    Execute,
}

impl Capability {
    /// Canonical presentation and serialization order. This is not an authority hierarchy.
    pub const ALL: [Self; 4] = [Self::Read, Self::Action, Self::Write, Self::Execute];

    /// Stable policy and audit vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Action => "action",
            Self::Write => "write",
            Self::Execute => "execute",
        }
    }
}

/// A complete independent RAWX requirement set.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct CapabilitySet(u8);

impl CapabilitySet {
    const READ_BIT: u8 = 1 << 0;
    const ACTION_BIT: u8 = 1 << 1;
    const WRITE_BIT: u8 = 1 << 2;
    const EXECUTE_BIT: u8 = 1 << 3;

    /// No RAWX authority is required.
    pub const EMPTY: Self = Self(0);
    /// Read authority only.
    pub const READ: Self = Self(Self::READ_BIT);
    /// Action authority only.
    pub const ACTION: Self = Self(Self::ACTION_BIT);
    /// Write authority only.
    pub const WRITE: Self = Self(Self::WRITE_BIT);
    /// Execute authority only.
    pub const EXECUTE: Self = Self(Self::EXECUTE_BIT);

    /// Build a set containing one capability.
    #[must_use]
    pub const fn one(capability: Capability) -> Self {
        match capability {
            Capability::Read => Self::READ,
            Capability::Action => Self::ACTION,
            Capability::Write => Self::WRITE,
            Capability::Execute => Self::EXECUTE,
        }
    }

    /// Return the union of two independent requirement sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether this set contains one capability.
    #[must_use]
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & Self::one(capability).0 != 0
    }

    /// Whether every requirement in this set is present in `allowed`.
    #[must_use]
    pub const fn is_subset_of(self, allowed: Self) -> bool {
        self.0 & !allowed.0 == 0
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Iterate in stable vocabulary order, never authority order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |capability| self.contains(*capability))
    }

    /// Human-plain compact label used by the local workbench.
    #[must_use]
    pub fn label(self) -> String {
        if self.is_empty() {
            return "local".into();
        }
        self.iter()
            .map(Capability::as_str)
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

impl From<Capability> for CapabilitySet {
    fn from(value: Capability) -> Self {
        Self::one(value)
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        iter.into_iter()
            .fold(Self::EMPTY, |set, capability| set.union(capability.into()))
    }
}

impl Serialize for CapabilitySet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for CapabilitySet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Vec::<Capability>::deserialize(deserializer).map(|values| values.into_iter().collect())
    }
}

impl FromStr for Capability {
    type Err = GovernanceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read" => Ok(Self::Read),
            "action" => Ok(Self::Action),
            "write" => Ok(Self::Write),
            "execute" => Ok(Self::Execute),
            _ => Err(GovernanceError::InvalidPolicy(format!(
                "unknown capability `{value}`"
            ))),
        }
    }
}

/// Stable reasons used for recovery, audit, and completion without payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// Authority permits the boundary.
    Permitted,
    /// Capability was not granted by every authority layer.
    CapabilityDenied,
    /// Model-driven tab closure was denied by an authority layer.
    TabCloseDenied,
    /// Host was not granted by every authority layer.
    HostDenied,
    /// Host or scheme is independently protected.
    ProtectedHost,
    /// Configured policy could not be validated.
    InvalidAuthority,
    /// Model-facing input did not match the catalog contract.
    InvalidRequest,
    /// Runtime control entered hold.
    RuntimeHold,
    /// Runtime control requires user attention.
    RuntimeAttention,
    /// Runtime control ended the session.
    SessionEnded,
    /// An authority layer does not admit this intake channel.
    ChannelDenied,
}

impl ReasonCode {
    /// Render the stable ASCII reason code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Permitted => "permitted",
            Self::CapabilityDenied => "capability_denied",
            Self::TabCloseDenied => "tab_close_denied",
            Self::HostDenied => "host_denied",
            Self::ProtectedHost => "protected_host",
            Self::InvalidAuthority => "invalid_authority",
            Self::InvalidRequest => "invalid_request",
            Self::RuntimeHold => "runtime_hold",
            Self::RuntimeAttention => "runtime_attention",
            Self::SessionEnded => "session_ended",
            Self::ChannelDenied => "channel_denied",
        }
    }
}

/// Stable policy rule names used for attribution and denial ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyRule {
    /// No grant covers the governed host.
    UnmatchedHost,
    /// A grant explicitly carves the host out.
    DeniedHost,
    /// A resolving grant lacks part of the required set.
    Capability,
    /// A monotonic setting prevents tab closure.
    TabClose,
    /// A manifest does not admit an intake channel.
    Channel,
}

impl PolicyRule {
    /// Stable denial and audit vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnmatchedHost => "unmatched_host",
            Self::DeniedHost => "denied_host",
            Self::Capability => "capability",
            Self::TabClose => "tab_close",
            Self::Channel => "channel",
        }
    }
}

/// Compact immutable reference into an authority snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PolicyAttribution {
    layer: u16,
    grant: Option<u16>,
    rule: PolicyRule,
    denial: [u8; 4],
    mode: manifest::PolicyMode,
}

/// A final-boundary permission decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decision {
    /// Whether the boundary is admitted.
    pub allowed: bool,
    /// Stable reason.
    pub reason: ReasonCode,
    /// Whether ordinary policy would deny but observe mode admitted the work.
    pub observed: bool,
    attribution: Option<PolicyAttribution>,
}

impl Decision {
    const fn allow() -> Self {
        Self {
            allowed: true,
            reason: ReasonCode::Permitted,
            observed: false,
            attribution: None,
        }
    }
    const fn deny(reason: ReasonCode) -> Self {
        Self {
            allowed: false,
            reason,
            observed: false,
            attribution: None,
        }
    }

    /// Construct a non-policy admission used by execution and runtime seams.
    #[must_use]
    pub const fn permitted() -> Self {
        Self::allow()
    }

    /// Construct a non-policy refusal used by validation and runtime seams.
    #[must_use]
    pub const fn refused(reason: ReasonCode) -> Self {
        Self::deny(reason)
    }

    fn policy(
        reason: ReasonCode,
        attribution: PolicyAttribution,
        effective_mode: manifest::PolicyMode,
    ) -> Self {
        if effective_mode == manifest::PolicyMode::Observe {
            Self {
                allowed: true,
                reason,
                observed: true,
                attribution: Some(attribution),
            }
        } else {
            Self {
                allowed: false,
                reason,
                observed: false,
                attribution: Some(attribution),
            }
        }
    }

    /// Stable denial id when an authored policy rule decided this boundary.
    #[must_use]
    pub fn denial_id(self) -> Option<String> {
        self.attribution.map(|attribution| {
            format!(
                "D-{:02x}{:02x}{:02x}{:02x}",
                attribution.denial[0],
                attribution.denial[1],
                attribution.denial[2],
                attribution.denial[3]
            )
        })
    }

    /// Stable authored rule when policy decided this boundary.
    #[must_use]
    pub fn policy_rule(self) -> Option<&'static str> {
        self.attribution.map(|value| value.rule.as_str())
    }

    /// Effective mode when authored policy decided this boundary.
    #[must_use]
    pub fn policy_mode(self) -> Option<&'static str> {
        self.attribution.map(|value| value.mode.as_str())
    }
}

/// One immutable effective authority for started work.
#[derive(Clone, Debug)]
pub struct AuthoritySnapshot {
    id: String,
    managed_sequence: Option<u64>,
    layers: Vec<PolicyLayer>,
    request_capabilities: Option<CapabilitySet>,
    request_hosts: Option<Vec<String>>,
    tab_close_allowed: bool,
    tab_close_source: Option<u16>,
    preserve_target_names: bool,
    sacred_hosts: Vec<String>,
    valid: bool,
}

/// Where one policy layer sits in the tighten-only authority order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityTier {
    Managed,
    User,
}

impl AuthorityTier {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::User => "user",
        }
    }
}

#[derive(Clone, Debug)]
struct PolicyLayer {
    tier: AuthorityTier,
    manifest: manifest::Manifest,
}

#[derive(Clone, Copy, Debug)]
struct RawDenial {
    reason: ReasonCode,
    rule: PolicyRule,
    grant: Option<u16>,
}

#[derive(Clone, Copy, Debug)]
struct LayerOutcome {
    denial: Option<RawDenial>,
    mode: manifest::PolicyMode,
}

impl AuthoritySnapshot {
    /// Opaque version recorded in audit.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Destinations policy marked never-touch, for the boundaries a person is shown.
    #[must_use]
    pub fn sacred_hosts(&self) -> &[String] {
        &self.sacred_hosts
    }

    /// Decide a capability at its final boundary.
    #[must_use]
    pub fn authorize_capability(&self, capability: Capability) -> Decision {
        self.authorize_requirements(capability.into())
    }

    /// Decide a complete independent capability requirement set at its final boundary.
    #[must_use]
    pub fn authorize_requirements(&self, requirements: CapabilitySet) -> Decision {
        if !self.valid {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        if requirements.is_empty() {
            return Decision::allow();
        }
        let outcomes: Vec<_> = self
            .layers
            .iter()
            .map(|layer| decide_resource_less(layer, requirements))
            .collect();
        if let Some(decision) = self.resolve_outcomes(&outcomes) {
            return decision;
        }
        if self
            .request_capabilities
            .is_some_and(|allowed| !requirements.is_subset_of(allowed))
        {
            return self.session_denial(ReasonCode::CapabilityDenied, PolicyRule::Capability);
        }
        Decision::allow()
    }

    /// Decide whether model-driven tab closure is admitted by every authority layer.
    #[must_use]
    pub fn authorize_tab_close(&self) -> Decision {
        if !self.valid {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        if self.tab_close_allowed {
            Decision::allow()
        } else {
            let layer = self.tab_close_source.unwrap_or(0);
            let source = self
                .layers
                .get(usize::from(layer))
                .map_or(self.id.as_str(), |value| value.manifest.hash.as_str());
            Decision::policy(
                ReasonCode::TabCloseDenied,
                PolicyAttribution {
                    layer,
                    grant: None,
                    rule: PolicyRule::TabClose,
                    denial: denial_bytes(source, "", PolicyRule::TabClose),
                    mode: manifest::PolicyMode::Enforce,
                },
                manifest::PolicyMode::Enforce,
            )
        }
    }

    /// Whether browser-observed target names may be retained in action outcomes and audit.
    #[must_use]
    pub const fn preserves_target_names(&self) -> bool {
        self.preserve_target_names
    }

    /// Decide an observed or requested landing at its final boundary.
    #[must_use]
    pub fn authorize_landing(&self, requirements: impl Into<CapabilitySet>, url: &str) -> Decision {
        let requirements = requirements.into();
        if !self.valid {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        let Ok(parsed) = Url::parse(url) else {
            return Decision::deny(ReasonCode::HostDenied);
        };
        if protected_url(&parsed) || protected_by_policy(&parsed, &self.sacred_hosts) {
            return Decision::deny(ReasonCode::ProtectedHost);
        }
        let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
            return Decision::deny(ReasonCode::HostDenied);
        };
        if requirements.is_empty() {
            return Decision::allow();
        }
        let outcomes: Vec<_> = self
            .layers
            .iter()
            .map(|layer| decide_for_host(layer, requirements, &host))
            .collect();
        if let Some(decision) = self.resolve_outcomes(&outcomes) {
            return decision;
        }
        if self
            .request_capabilities
            .is_some_and(|allowed| !requirements.is_subset_of(allowed))
        {
            return self.session_denial(ReasonCode::CapabilityDenied, PolicyRule::Capability);
        }
        if self
            .request_hosts
            .as_ref()
            .is_some_and(|patterns| !patterns.iter().any(|pattern| host_matches(&host, pattern)))
        {
            return self.session_denial(ReasonCode::HostDenied, PolicyRule::UnmatchedHost);
        }
        Decision::allow()
    }

    /// Whether policy-aware discovery can prove that some host-scoped variant may proceed.
    ///
    /// Discovery is an optimization only. Final-boundary admission still resolves the real host
    /// and current immutable snapshot.
    #[must_use]
    pub fn could_admit(&self, requirements: CapabilitySet) -> bool {
        if !self.valid {
            return false;
        }
        if requirements.is_empty() {
            return true;
        }
        if self
            .request_capabilities
            .is_some_and(|allowed| !requirements.is_subset_of(allowed))
            || self.request_hosts.as_ref().is_some_and(Vec::is_empty)
        {
            return false;
        }
        let outcomes: Vec<_> = self
            .layers
            .iter()
            .map(|layer| decide_potential_host(layer, requirements))
            .collect();
        if outcomes.iter().all(|outcome| outcome.denial.is_none()) {
            return true;
        }
        outcomes
            .iter()
            .fold(manifest::PolicyMode::Observe, |mode, outcome| {
                mode.strictest(outcome.mode)
            })
            == manifest::PolicyMode::Observe
    }

    fn resolve_outcomes(&self, outcomes: &[LayerOutcome]) -> Option<Decision> {
        let denial = outcomes
            .iter()
            .enumerate()
            .find_map(|(index, outcome)| outcome.denial.map(|denial| (index, denial)))?;
        let effective_mode = outcomes
            .iter()
            .fold(manifest::PolicyMode::Observe, |mode, outcome| {
                mode.strictest(outcome.mode)
            });
        let layer = &self.layers[denial.0];
        let grant_id = denial
            .1
            .grant
            .and_then(|index| layer.manifest.grants.get(usize::from(index)))
            .map_or("", |grant| grant.id.as_str());
        Some(Decision::policy(
            denial.1.reason,
            PolicyAttribution {
                layer: u16::try_from(denial.0).expect("policy layer count is bounded"),
                grant: denial.1.grant,
                rule: denial.1.rule,
                denial: denial_bytes(&layer.manifest.hash, grant_id, denial.1.rule),
                mode: effective_mode,
            },
            effective_mode,
        ))
    }

    fn session_denial(&self, reason: ReasonCode, rule: PolicyRule) -> Decision {
        let layer = u16::try_from(self.layers.len()).expect("policy layer count is bounded");
        Decision::policy(
            reason,
            PolicyAttribution {
                layer,
                grant: None,
                rule,
                denial: denial_bytes(&self.id, "session", rule),
                mode: manifest::PolicyMode::Enforce,
            },
            manifest::PolicyMode::Enforce,
        )
    }

    /// Policy tier and grant id for one decision, when authored policy decided it.
    #[must_use]
    pub fn attribution(&self, decision: Decision) -> Option<(&'static str, Option<&str>)> {
        let attribution = decision.attribution?;
        let Some(layer) = self.layers.get(usize::from(attribution.layer)) else {
            return Some(("session", None));
        };
        let grant = attribution
            .grant
            .and_then(|index| layer.manifest.grants.get(usize::from(index)))
            .map(|grant| grant.id.as_str());
        Some((layer.tier.as_str(), grant))
    }
}

fn decide_potential_host(layer: &PolicyLayer, requirements: CapabilitySet) -> LayerOutcome {
    if let Some(grant) = layer.manifest.grants.iter().find(|grant| {
        requirements.is_subset_of(grant.allowed_set()) && grant_has_possible_host(grant)
    }) {
        return LayerOutcome {
            denial: None,
            mode: grant.mode.or(layer.manifest.mode).unwrap_or_default(),
        };
    }
    LayerOutcome {
        denial: Some(RawDenial {
            reason: ReasonCode::CapabilityDenied,
            rule: PolicyRule::Capability,
            grant: None,
        }),
        mode: layer.manifest.mode.unwrap_or_default(),
    }
}

fn grant_has_possible_host(grant: &manifest::Grant) -> bool {
    grant.hosts.allow.iter().any(|pattern| {
        let host = if pattern == "*" {
            "policy-probe.invalid".to_owned()
        } else if let Some(suffix) = pattern.strip_prefix("*.") {
            format!("policy-probe.{suffix}")
        } else {
            pattern.clone()
        };
        evaluate_host(&host, &grant.hosts) == HostOutcome::Allowed
    })
}

fn decide_resource_less(layer: &PolicyLayer, requirements: CapabilitySet) -> LayerOutcome {
    let allowed = layer
        .manifest
        .grants
        .iter()
        .fold(CapabilitySet::EMPTY, |set, grant| {
            set.union(grant.allowed_set())
        });
    let mode = layer
        .manifest
        .grants
        .iter()
        .filter(|grant| grant.allowed_set().intersects(requirements))
        .map(|grant| grant.mode.or(layer.manifest.mode).unwrap_or_default())
        .reduce(manifest::PolicyMode::strictest)
        .unwrap_or_else(|| layer.manifest.mode.unwrap_or_default());
    if requirements.is_subset_of(allowed) {
        return LayerOutcome { denial: None, mode };
    }
    LayerOutcome {
        denial: Some(RawDenial {
            reason: if layer.manifest.grants.is_empty() {
                ReasonCode::HostDenied
            } else {
                ReasonCode::CapabilityDenied
            },
            rule: if layer.manifest.grants.is_empty() {
                PolicyRule::UnmatchedHost
            } else {
                PolicyRule::Capability
            },
            grant: None,
        }),
        mode,
    }
}

fn decide_for_host(layer: &PolicyLayer, requirements: CapabilitySet, host: &str) -> LayerOutcome {
    let mut first_denial = None;
    let mut first_denial_mode = None;
    for (index, grant) in layer.manifest.grants.iter().enumerate() {
        let grant_index = u16::try_from(index).expect("grant count is bounded");
        let mode = grant.mode.or(layer.manifest.mode).unwrap_or_default();
        match evaluate_host(host, &grant.hosts) {
            HostOutcome::Allowed => {
                if requirements.is_subset_of(grant.allowed_set()) {
                    return LayerOutcome { denial: None, mode };
                }
                if first_denial.is_none() {
                    first_denial = Some(RawDenial {
                        reason: ReasonCode::CapabilityDenied,
                        rule: PolicyRule::Capability,
                        grant: Some(grant_index),
                    });
                    first_denial_mode = Some(mode);
                }
            }
            HostOutcome::Denied if first_denial.is_none() => {
                first_denial = Some(RawDenial {
                    reason: ReasonCode::HostDenied,
                    rule: PolicyRule::DeniedHost,
                    grant: Some(grant_index),
                });
                first_denial_mode = Some(mode);
            }
            HostOutcome::Denied | HostOutcome::Unmatched => {}
        }
    }
    LayerOutcome {
        denial: Some(first_denial.unwrap_or(RawDenial {
            reason: ReasonCode::HostDenied,
            rule: PolicyRule::UnmatchedHost,
            grant: None,
        })),
        mode: first_denial_mode.unwrap_or_else(|| layer.manifest.mode.unwrap_or_default()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostOutcome {
    Allowed,
    Denied,
    Unmatched,
}

fn evaluate_host(host: &str, rules: &manifest::HostRules) -> HostOutcome {
    let allow = rules
        .allow
        .iter()
        .filter_map(|pattern| pattern_specificity(host, pattern))
        .max();
    let deny = rules
        .deny
        .iter()
        .filter_map(|pattern| pattern_specificity(host, pattern))
        .max();
    match (allow, deny) {
        (None, None) => HostOutcome::Unmatched,
        (Some(_), None) => HostOutcome::Allowed,
        (None, Some(_)) => HostOutcome::Denied,
        (Some(allow), Some(deny)) if deny >= allow => HostOutcome::Denied,
        (Some(_), Some(_)) => HostOutcome::Allowed,
    }
}

fn pattern_specificity(host: &str, pattern: &str) -> Option<(u8, usize)> {
    let pattern = pattern.to_ascii_lowercase();
    if pattern == host {
        return Some((3, pattern.len()));
    }
    if pattern == "*" {
        return Some((1, 0));
    }
    let suffix = pattern.strip_prefix("*.")?;
    (host.len() > suffix.len()
        && host.ends_with(suffix)
        && host.as_bytes().get(host.len() - suffix.len() - 1) == Some(&b'.'))
    .then_some((2, suffix.len()))
}

fn denial_bytes(manifest_hash: &str, grant_id: &str, rule: PolicyRule) -> [u8; 4] {
    let digest =
        Sha256::digest(format!("{manifest_hash}\n{grant_id}\n{}", rule.as_str()).as_bytes());
    [digest[0], digest[1], digest[2], digest[3]]
}

fn protected_by_policy(url: &Url, sacred_hosts: &[String]) -> bool {
    url.host_str().is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        sacred_hosts
            .iter()
            .any(|pattern| host_matches(&host, pattern))
    })
}

/// Live controls checked immediately before effects and when browser events arrive.
#[derive(Debug, Default)]
pub struct RuntimeControls {
    state: AtomicU8,
}

impl RuntimeControls {
    /// Return the current final-boundary decision.
    #[must_use]
    pub fn decision(&self) -> Decision {
        match self.state.load(Ordering::SeqCst) {
            RUNTIME_ACTIVE => Decision::allow(),
            RUNTIME_HOLD => Decision::deny(ReasonCode::RuntimeHold),
            RUNTIME_ATTENTION => Decision::deny(ReasonCode::RuntimeAttention),
            _ => Decision::deny(ReasonCode::SessionEnded),
        }
    }

    /// Return the authoritative content-free runtime state.
    #[must_use]
    pub fn state(&self) -> RuntimeControlState {
        match self.state.load(Ordering::SeqCst) {
            RUNTIME_ACTIVE => RuntimeControlState::Active,
            RUNTIME_HOLD => RuntimeControlState::Held,
            RUNTIME_ATTENTION => RuntimeControlState::Attention,
            _ => RuntimeControlState::Ended,
        }
    }

    /// Apply one local human control intent and return the resulting state.
    pub fn apply_intent(&self, intent: RuntimeControlIntent) -> RuntimeControlState {
        match intent {
            RuntimeControlIntent::ToggleHold => {
                if self.state() == RuntimeControlState::Active {
                    self.hold();
                } else if matches!(
                    self.state(),
                    RuntimeControlState::Held | RuntimeControlState::Attention
                ) {
                    self.resume();
                }
            }
            RuntimeControlIntent::Hold if self.state() != RuntimeControlState::Ended => self.hold(),
            RuntimeControlIntent::Resume if self.state() != RuntimeControlState::Ended => {
                self.resume()
            }
            RuntimeControlIntent::EndSession => self.end_session(),
            RuntimeControlIntent::StartSession => self.resume(),
            RuntimeControlIntent::Hold | RuntimeControlIntent::Resume => {}
        }
        self.state()
    }

    /// Enter a runtime hold.
    pub fn hold(&self) {
        self.state.store(RUNTIME_HOLD, Ordering::SeqCst);
    }
    /// Require visible user attention.
    pub fn require_attention(&self) {
        self.state.store(RUNTIME_ATTENTION, Ordering::SeqCst);
    }
    /// End the admitted session.
    pub fn end_session(&self) {
        self.state.store(RUNTIME_END, Ordering::SeqCst);
    }
    /// Resume active work after an external authority decision.
    pub fn resume(&self) {
        self.state.store(RUNTIME_ACTIVE, Ordering::SeqCst);
    }
}

/// A small governance service facade used by the application executor.
#[derive(Clone, Debug)]
pub struct GovernanceFacade {
    policies: Arc<Mutex<PolicySources>>,
    runtime_control: Option<PathBuf>,
    controls: Arc<RuntimeControls>,
    denial_attention: Arc<Mutex<DenialAttention>>,
}

#[derive(Debug, Default)]
struct DenialAttention {
    attempts: VecDeque<DenialAttempt>,
}

#[derive(Debug)]
struct DenialAttempt {
    workspace: String,
    key: String,
    at_ms: u64,
}

impl DenialAttention {
    fn record(&mut self, workspace: &str, decision: Decision, at_ms: u64) -> bool {
        if decision.allowed || decision.observed || !attention_eligible(decision.reason) {
            return false;
        }
        let oldest = at_ms.saturating_sub(DENIAL_ATTENTION_ALL_WINDOW_MS);
        self.attempts.retain(|attempt| attempt.at_ms >= oldest);
        let key = decision
            .denial_id()
            .unwrap_or_else(|| decision.reason.as_str().into());
        self.attempts.push_back(DenialAttempt {
            workspace: workspace.into(),
            key: key.clone(),
            at_ms,
        });
        while self.attempts.len() > DENIAL_ATTENTION_HISTORY_LIMIT {
            self.attempts.pop_front();
        }
        let all = self
            .attempts
            .iter()
            .filter(|attempt| attempt.workspace == workspace)
            .count();
        let matching = self
            .attempts
            .iter()
            .filter(|attempt| {
                attempt.workspace == workspace
                    && attempt.key == key
                    && attempt.at_ms >= at_ms.saturating_sub(DENIAL_ATTENTION_MATCHING_WINDOW_MS)
            })
            .count();
        let attention = matching >= DENIAL_ATTENTION_MATCHING_THRESHOLD
            || all >= DENIAL_ATTENTION_ALL_THRESHOLD;
        if attention {
            self.attempts
                .retain(|attempt| attempt.workspace != workspace);
        }
        attention
    }
}

const fn attention_eligible(reason: ReasonCode) -> bool {
    matches!(
        reason,
        ReasonCode::CapabilityDenied
            | ReasonCode::TabCloseDenied
            | ReasonCode::HostDenied
            | ReasonCode::ProtectedHost
            | ReasonCode::InvalidAuthority
    )
}

#[derive(Debug)]
struct PolicySources {
    managed: PolicySource,
    managed_remote: Option<managed::ManagedAuthority>,
    user: PolicySource,
    user_origin: effective::UserLayerSource,
    /// The one path this window may write. Absent when Ghostlight does not own the user layer.
    owned_user_path: Option<PathBuf>,
}

impl PolicySources {
    fn new(local: Option<PathBuf>, managed: Option<PathBuf>) -> Self {
        Self {
            managed: PolicySource::new(managed, "managed"),
            managed_remote: None,
            user: PolicySource::new(local, "user"),
            user_origin: effective::UserLayerSource::Environment,
            owned_user_path: None,
        }
    }

    /// A user layer Ghostlight owns at an explicit path, for tests that must not touch this
    /// machine's real state directory.
    #[cfg(test)]
    fn owning(path: PathBuf, managed: Option<PathBuf>) -> Self {
        Self {
            managed: PolicySource::new(managed, "managed"),
            managed_remote: None,
            user: PolicySource::with_options(Some(path.clone()), "user", true),
            user_origin: effective::UserLayerSource::Workbench,
            owned_user_path: Some(path),
        }
    }

    /// Resolve the one user layer, in the order ADR-0122 Decision 4 fixes.
    ///
    /// A path someone else named wins and is never written back. Otherwise Ghostlight uses the file
    /// it owns, which is optional: a machine that has never authored one stays all-open rather than
    /// failing closed over a file that was never supposed to exist yet.
    fn production(local: Option<PathBuf>) -> Self {
        let (user, user_origin) = match local {
            Some(path) => (
                PolicySource::new(Some(path), "user"),
                effective::UserLayerSource::Environment,
            ),
            None => (
                PolicySource::with_options(paths::user_policy_path(), "user", true),
                effective::UserLayerSource::Workbench,
            ),
        };
        Self {
            managed: PolicySource::new(None, "managed"),
            managed_remote: Some(managed::ManagedAuthority::production()),
            user,
            user_origin,
            owned_user_path: match user_origin {
                effective::UserLayerSource::Workbench => paths::user_policy_path(),
                _ => None,
            },
        }
    }

    #[cfg(test)]
    fn with_managed_paths(paths: managed::ManagedPaths) -> Self {
        Self {
            managed: PolicySource::new(None, "managed"),
            managed_remote: Some(managed::ManagedAuthority::from_paths(paths)),
            user: PolicySource::new(None, "user"),
            user_origin: effective::UserLayerSource::Workbench,
            owned_user_path: None,
        }
    }

    fn refresh(&mut self) {
        if let Some(remote) = &mut self.managed_remote {
            remote.refresh();
        }
        self.managed.refresh("managed");
        self.user.refresh("user");
    }

    fn managed_configured(&self) -> bool {
        self.managed_remote
            .as_ref()
            .is_some_and(managed::ManagedAuthority::configured)
            || self.managed.configured()
    }

    fn managed_valid(&self) -> bool {
        self.managed_remote.as_ref().map_or(
            self.managed.last_load_valid,
            managed::ManagedAuthority::valid,
        )
    }

    fn managed_manifest(&self) -> Option<&manifest::Manifest> {
        self.managed_remote
            .as_ref()
            .and_then(managed::ManagedAuthority::manifest)
            .or(self.managed.active.as_ref())
    }

    fn managed_sequence(&self) -> Option<u64> {
        self.managed_remote
            .as_ref()
            .and_then(managed::ManagedAuthority::sequence)
    }

    fn managed_passport(&self) -> ManagedPolicyPassport {
        if let Some(remote) = &self.managed_remote {
            return remote.passport();
        }
        let configured = self.managed.configured();
        let verified = self.managed.active.is_some();
        ManagedPolicyPassport {
            configured,
            verified,
            freshness: if !configured {
                ManagedPolicyFreshness::NotConfigured
            } else if verified {
                ManagedPolicyFreshness::Fresh
            } else {
                ManagedPolicyFreshness::NoPolicy
            },
            sequence: None,
            organization: None,
            rationale: None,
            contacts: Vec::new(),
            source_class: if configured {
                ManagedPolicySource::File
            } else {
                ManagedPolicySource::None
            },
            last_success_ms: None,
            last_attempt_ms: None,
        }
    }
}

/// Render one active policy as the document it was authored as.
///
/// Serialization rather than a file read, because a signed organization policy arrives inside a
/// bundle and has no plain file to show. The canonical hash is never authored and never rendered.
fn document(policy: &manifest::Manifest) -> String {
    serde_json::to_string_pretty(policy).unwrap_or_else(|_| String::from("{}"))
}

/// Name a signed managed source by class, never by address.
fn passport_source(passport: &ManagedPolicyPassport) -> String {
    match passport.source_class {
        ManagedPolicySource::Https => "Signed bundle from an HTTPS source".into(),
        ManagedPolicySource::File => "Signed bundle from a local file".into(),
        ManagedPolicySource::None => "Signed bundle".into(),
    }
}

#[derive(Debug)]
struct PolicySource {
    path: Option<PathBuf>,
    /// Whether an absent file means "no layer" rather than "configured but unreadable".
    ///
    /// The user policy Ghostlight owns is optional: a machine with no such file is all-open, not
    /// failing closed. A path someone else named is not optional, because naming a file that is not
    /// there is a mistake worth refusing over.
    optional: bool,
    present: bool,
    active: Option<manifest::Manifest>,
    last_load_valid: bool,
    last_error: Option<String>,
}

impl PolicySource {
    fn new(path: Option<PathBuf>, tier: &str) -> Self {
        Self::with_options(path, tier, false)
    }

    fn with_options(path: Option<PathBuf>, tier: &str, optional: bool) -> Self {
        let mut source = Self {
            last_load_valid: path.is_none(),
            optional,
            present: path.is_some(),
            path,
            active: None,
            last_error: None,
        };
        source.refresh(tier);
        source
    }

    fn refresh(&mut self, tier: &str) {
        let Some(path) = &self.path else { return };
        if self.optional && !path.exists() {
            self.present = false;
            self.active = None;
            self.last_load_valid = true;
            self.last_error = None;
            return;
        }
        self.present = true;
        match read_policy(path) {
            Ok(policy) => {
                self.active = Some(policy);
                self.last_load_valid = true;
                self.last_error = None;
            }
            Err(error) => {
                let detail = error.to_string();
                if self.last_error.as_deref() != Some(detail.as_str()) {
                    if self.active.is_some() {
                        eprintln!(
                            "Ghostlight kept the last valid {tier} policy after reload failed: {detail}"
                        );
                    } else {
                        eprintln!("Ghostlight {tier} policy is not valid: {detail}");
                    }
                }
                self.last_load_valid = false;
                self.last_error = Some(detail);
            }
        }
    }

    fn configured(&self) -> bool {
        self.path.is_some() && self.present
    }

    fn has_authority(&self) -> bool {
        !self.configured() || self.active.is_some()
    }
}

/// Content-free configuration facts for the local workbench.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GovernanceDiagnostics {
    /// Whether a local policy source is configured.
    pub local_policy_configured: bool,
    /// Whether a validated local policy is currently applied.
    pub local_policy_active: bool,
    /// Whether the configured local policy can be read and validated.
    pub local_policy_valid: bool,
    /// Whether a managed authority source is configured.
    pub managed_authority_configured: bool,
    /// Whether a verified managed policy is currently applied.
    pub managed_authority_active: bool,
    /// Whether the configured managed authority can be read and validated.
    pub managed_authority_valid: bool,
    /// Whether a runtime-control file is configured.
    pub runtime_control_file_configured: bool,
    /// User-visible provenance for the active signed managed policy.
    pub managed_policy: ManagedPolicyPassport,
}

/// User-visible provenance for signed managed authority, without credentials or policy rules.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagedPolicyPassport {
    /// Whether an administrator-provisioned managed source exists.
    pub configured: bool,
    /// Whether a signed policy is currently verified and active.
    pub verified: bool,
    /// Whether the active policy is current, last-known-good, absent, or not configured.
    pub freshness: ManagedPolicyFreshness,
    /// Monotonic signed publish sequence, when active.
    pub sequence: Option<u64>,
    /// Signed organization display name, when supplied.
    pub organization: Option<String>,
    /// Signed organization explanation, when supplied.
    pub rationale: Option<String>,
    /// Signed organization contact channels.
    pub contacts: Vec<ManagedPolicyContact>,
    /// Content-free source class.
    pub source_class: ManagedPolicySource,
    /// Last successful verification or not-modified response time.
    pub last_success_ms: Option<u64>,
    /// Last source attempt time.
    pub last_attempt_ms: Option<u64>,
}

/// Current managed-policy freshness.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedPolicyFreshness {
    /// No administrator bootstrap is present.
    NotConfigured,
    /// A configured source has not supplied a valid signed policy.
    NoPolicy,
    /// The most recent source check succeeded.
    Fresh,
    /// A source failure left the last verified policy active.
    LastKnownGood,
}

/// Managed-policy transport class without its address or credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedPolicySource {
    /// No managed source exists.
    None,
    /// A local file supplies signed bundles.
    File,
    /// An HTTPS endpoint supplies signed bundles.
    Https,
}

/// One signed organization contact channel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagedPolicyContact {
    /// Contact channel kind, such as email or URL.
    pub kind: String,
    /// Contact address.
    pub value: String,
    /// Optional organization-authored display label.
    pub label: Option<String>,
}

impl GovernanceFacade {
    /// Return content-free configuration health without exposing authority paths or rules.
    #[must_use]
    pub fn diagnostics(&self) -> GovernanceDiagnostics {
        self.refresh_policies();
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        GovernanceDiagnostics {
            local_policy_configured: policies.user.configured(),
            local_policy_active: policies.user.active.is_some(),
            local_policy_valid: policies.user.last_load_valid,
            managed_authority_configured: policies.managed_configured(),
            managed_authority_active: policies.managed_manifest().is_some(),
            managed_authority_valid: policies.managed_valid(),
            runtime_control_file_configured: self.runtime_control.is_some(),
            managed_policy: policies.managed_passport(),
        }
    }

    /// Construct the facade from explicit policy paths.
    #[must_use]
    pub fn new(local_policy: Option<PathBuf>, managed_policy: Option<PathBuf>) -> Self {
        Self {
            policies: Arc::new(Mutex::new(PolicySources::new(local_policy, managed_policy))),
            runtime_control: None,
            controls: Arc::new(RuntimeControls::default()),
            denial_attention: Arc::new(Mutex::new(DenialAttention::default())),
        }
    }

    /// A facade whose user layer is one Ghostlight owns at an explicit path.
    #[cfg(test)]
    fn owning_user_policy(path: PathBuf, managed: Option<PathBuf>) -> Self {
        Self {
            policies: Arc::new(Mutex::new(PolicySources::owning(path, managed))),
            runtime_control: None,
            controls: Arc::new(RuntimeControls::default()),
            denial_attention: Arc::new(Mutex::new(DenialAttention::default())),
        }
    }

    #[cfg(test)]
    fn with_managed_paths(paths: managed::ManagedPaths) -> Self {
        Self {
            policies: Arc::new(Mutex::new(PolicySources::with_managed_paths(paths))),
            runtime_control: None,
            controls: Arc::new(RuntimeControls::default()),
            denial_attention: Arc::new(Mutex::new(DenialAttention::default())),
        }
    }

    /// Construct the facade from Ghostlight-specific environment variables.
    #[must_use]
    pub fn from_environment() -> Self {
        Self {
            policies: Arc::new(Mutex::new(PolicySources::production(
                env::var_os("GHOSTLIGHT_POLICY_FILE").map(PathBuf::from),
            ))),
            runtime_control: None,
            controls: Arc::new(RuntimeControls::default()),
            denial_attention: Arc::new(Mutex::new(DenialAttention::default())),
        }
        .with_runtime_control_file(
            env::var_os("GHOSTLIGHT_RUNTIME_CONTROL_FILE").map(PathBuf::from),
        )
    }

    /// Select an optional local runtime-control file for hold, attention, and end-session state.
    #[must_use]
    pub fn with_runtime_control_file(mut self, path: Option<PathBuf>) -> Self {
        self.runtime_control = path;
        self
    }

    /// Access live runtime controls.
    #[must_use]
    pub fn controls(&self) -> Arc<RuntimeControls> {
        Arc::clone(&self.controls)
    }

    /// Apply one extension-toolbar intent at the service authority owner.
    pub fn apply_runtime_intent(&self, intent: RuntimeControlIntent) -> RuntimeControlState {
        self.controls.apply_intent(intent)
    }

    /// Return the current authoritative content-free runtime state.
    #[must_use]
    pub fn runtime_state(&self) -> RuntimeControlState {
        self.controls.state()
    }

    /// Record one enforced workspace-local denial and report whether it crossed the attention
    /// threshold. The circuit is bounded, memory-only, and clears that workspace after firing.
    pub(crate) fn record_denial_attention(&self, workspace: &str, decision: Decision) -> bool {
        self.denial_attention
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record(workspace, decision, unix_ms())
    }

    /// Decide whether an intake channel may open a session at all.
    ///
    /// This is admission, not capability: an admitted channel is still bound by every ceiling the
    /// same layers impose, and no layer can raise one channel above another. Layers compose by
    /// intersection, so a managed refusal cannot be undone locally, and an invalid layer denies.
    #[must_use]
    pub fn admits_channel(&self, channel: IntakeChannel) -> Decision {
        self.refresh_policies();
        let key = match channel {
            IntakeChannel::Mcp => "channels.mcp.enabled",
            IntakeChannel::Cli => "channels.cli.enabled",
        };
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !policies.managed_valid() || !policies.user.has_authority() {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        for policy in [policies.managed_manifest(), policies.user.active.as_ref()]
            .into_iter()
            .flatten()
        {
            if policy.boolean_setting(key) == Some(false) {
                return Decision::deny(ReasonCode::ChannelDenied);
            }
        }
        Decision::allow()
    }

    /// Author the band chip without compiling the whole destination.
    ///
    /// The band redraws on every snapshot, so this reads what it needs under one lock rather than
    /// building the full view thirty times a minute.
    #[must_use]
    pub fn policy_chip(&self) -> effective::PolicyChip {
        self.refresh_policies();
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let organization = policies.managed_manifest();
        let valid = policies.managed_valid() && policies.user.has_authority();
        let situation = if !valid {
            effective::Situation::FailingClosed
        } else {
            match (organization.is_some(), policies.user.active.is_some()) {
                (true, true) => effective::Situation::Layered,
                (true, false) => effective::Situation::OrganizationOnly,
                (false, true) => effective::Situation::UserOnly,
                (false, false) => effective::Situation::AllOpen,
            }
        };
        let stale = matches!(
            policies.managed_passport().freshness,
            ManagedPolicyFreshness::LastKnownGood
        );
        let name = organization
            .and_then(|policy| policy.organization.as_ref())
            .map(|organization| organization.name.clone());
        effective::chip(situation, name.as_deref(), stale)
    }

    /// Compile the policy into the one answer a person arrives with.
    ///
    /// Assembled under a single lock so the sentence, the capability lines, and the rules behind
    /// them all describe the same instant. The words are authored in the orchestrator; the window
    /// renders them and computes nothing (ADR-0122 Decision 2).
    #[must_use]
    pub fn effective_authority(&self) -> effective::EffectiveAuthority {
        let sacred_hosts = self
            .snapshot(&RequestRestrictions::default())
            .sacred_hosts()
            .to_vec();
        self.refresh_policies();
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let organization = policies.managed_manifest().cloned();
        let user = policies.user.active.clone();
        let passport = policies.managed_passport();
        let authoring_allowed = organization
            .as_ref()
            .and_then(|policy| policy.boolean_setting("policy.user.enabled"))
            .unwrap_or(true);
        let inputs = effective::Inputs {
            organization: organization.as_ref(),
            user: user.as_ref(),
            valid: policies.managed_valid() && policies.user.has_authority(),
            sacred_hosts,
            organization_source: policies.managed.path.as_ref().map_or_else(
                || organization.as_ref().map(|_| passport_source(&passport)),
                |path| Some(path.display().to_string()),
            ),
            organization_document: organization.as_ref().map(document),
            user_source: policies
                .user
                .path
                .as_ref()
                .map(|path| path.display().to_string()),
            user_document: user.as_ref().map(document),
            user_layer_source: if user.is_some() {
                policies.user_origin
            } else {
                effective::UserLayerSource::None
            },
            owned_user_path: paths::user_policy_path().map(|path| path.display().to_string()),
            authoring_allowed,
            windows: cfg!(windows),
            passport: passport.clone(),
        };
        effective::compile(&inputs)
    }

    /// Resolve the effective missing-browser startup preference.
    #[must_use]
    pub fn browser_startup(&self) -> manifest::BrowserStartup {
        self.refresh_policies();
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inputs = effective::Inputs {
            organization: policies.managed_manifest(),
            user: policies.user.active.as_ref(),
            valid: policies.managed_valid() && policies.user.has_authority(),
            sacred_hosts: Vec::new(),
            organization_source: None,
            organization_document: None,
            user_source: None,
            user_document: None,
            user_layer_source: effective::UserLayerSource::None,
            owned_user_path: None,
            authoring_allowed: true,
            windows: cfg!(windows),
            passport: policies.managed_passport(),
        };
        effective::browser_startup(&inputs).value
    }

    /// Build one immutable snapshot and apply caller restrictions by intersection.
    #[must_use]
    pub fn snapshot(&self, restrictions: &RequestRestrictions) -> AuthoritySnapshot {
        self.refresh_policies();
        let (sources, managed_sequence, valid) = {
            let policies = self
                .policies
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                [
                    (policies.managed_manifest().cloned(), AuthorityTier::Managed),
                    (policies.user.active.clone(), AuthorityTier::User),
                ],
                policies.managed_sequence(),
                policies.managed_valid() && policies.user.has_authority(),
            )
        };
        assemble(restrictions, sources, managed_sequence, valid)
    }

    /// Build the snapshot a candidate user policy would produce, without applying it.
    ///
    /// The organization layer stays exactly as it is, because a preview that ignored the ceiling
    /// would answer a question nobody asked. Nothing here writes a file, changes authority, or
    /// records audit (ADR-0122 Decision 7).
    pub fn candidate_snapshot(&self, document: &str) -> Result<Candidate, manifest::ManifestError> {
        let candidate = manifest::parse(document, "this policy")?;
        let rules = candidate.grants.len();
        self.refresh_policies();
        let (managed, managed_sequence, managed_valid) = {
            let policies = self
                .policies
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                policies.managed_manifest().cloned(),
                policies.managed_sequence(),
                policies.managed_valid(),
            )
        };
        Ok(Candidate {
            snapshot: assemble(
                &RequestRestrictions::default(),
                [
                    (managed, AuthorityTier::Managed),
                    (Some(candidate), AuthorityTier::User),
                ],
                managed_sequence,
                managed_valid,
            ),
            rules,
        })
    }

    /// Whether an organization layer permits a locally authored user policy.
    ///
    /// This gates authoring, never enforcement. A user layer that already exists keeps applying,
    /// because it can only subtract: dropping it at decision time would restore authority no upper
    /// layer removed, which is the one thing the monotonic rule forbids. The switch exists so an
    /// organization can keep a fleet predictable, and it is not a security boundary -- a user layer
    /// could never widen anything in the first place (ADR-0122 Decision 5).
    #[must_use]
    pub fn user_authoring_allowed(&self) -> bool {
        self.refresh_policies();
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        policies
            .managed_manifest()
            .and_then(|policy| policy.boolean_setting("policy.user.enabled"))
            .unwrap_or(true)
    }
}

/// One candidate policy, decided but not applied.
///
/// The rule count travels with the snapshot because a policy with no rules allows nothing, and a
/// preview that reported only the resulting refusals would read as an accusation about the past
/// rather than a statement about an empty draft.
pub struct Candidate {
    /// The authority this candidate would produce, under the organization ceiling as it stands.
    pub snapshot: AuthoritySnapshot,
    /// How many rules the candidate authors.
    pub rules: usize,
}

/// Why a locally authored user policy could not be applied.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthoringError {
    /// An organization layer switched local authoring off.
    #[error("{0}")]
    NotAllowed(String),
    /// Ghostlight reads this policy but does not own its file.
    #[error("An environment variable points Ghostlight at a policy file it does not own, so this window cannot change it.")]
    NotOwned,
    /// This environment names no per-user state directory.
    #[error("This machine names no per-user state directory to keep a policy in.")]
    NoHome,
    /// The document is not a valid policy.
    #[error("{0}")]
    Invalid(String),
    /// Only an organization layer may author this setting.
    #[error("Only your organization can set {0}.")]
    OrganizationOnlySetting(String),
    /// The file could not be replaced.
    #[error("{0}")]
    Unwritable(String),
}

impl GovernanceFacade {
    /// Replace this machine's user policy with a validated document.
    ///
    /// Validation happens before anything is replaced and the write is atomic, so pressing a button
    /// in the window can never leave Ghostlight configured with a policy it cannot read. That is the
    /// one failure mode a local authoring surface must not have (ADR-0122 Decision 4).
    pub fn apply_user_policy(&self, document: &str) -> Result<manifest::Manifest, AuthoringError> {
        let policy = manifest::parse(document, "this policy")
            .map_err(|error| AuthoringError::Invalid(error.to_string()))?;
        if policy.boolean_setting("policy.user.enabled").is_some() {
            return Err(AuthoringError::OrganizationOnlySetting(
                "policy.user.enabled".into(),
            ));
        }
        let path = self.writable_user_policy_path()?;
        let parent = path.parent().ok_or(AuthoringError::NoHome)?;
        fs::create_dir_all(parent)
            .map_err(|error| AuthoringError::Unwritable(error.to_string()))?;
        // A per-call unique name, not a fixed one, so two overlapping calls to apply_user_policy
        // never share a staging file and cannot interleave or clobber each other's write.
        let staged = path.with_extension(format!("json.{}.writing", uuid::Uuid::new_v4().simple()));
        fs::write(&staged, document.as_bytes())
            .map_err(|error| AuthoringError::Unwritable(error.to_string()))?;
        fs::rename(&staged, &path).map_err(|error| {
            let _ = fs::remove_file(&staged);
            AuthoringError::Unwritable(error.to_string())
        })?;
        self.refresh_policies();
        Ok(policy)
    }

    /// Remove this machine's user policy, returning authority to whatever remains above it.
    pub fn remove_user_policy(&self) -> Result<(), AuthoringError> {
        let path = self.writable_user_policy_path()?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(AuthoringError::Unwritable(error.to_string())),
        }
        self.refresh_policies();
        Ok(())
    }

    /// The user policy path this window may write, or why it may not.
    fn writable_user_policy_path(&self) -> Result<PathBuf, AuthoringError> {
        if !self.user_authoring_allowed() {
            let policies = self
                .policies
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let name = policies
                .managed_manifest()
                .and_then(|policy| policy.organization.as_ref())
                .map_or_else(
                    || "Your organization".to_owned(),
                    |organization| organization.name.clone(),
                );
            return Err(AuthoringError::NotAllowed(format!(
                "{name} does not allow rules to be set on this machine."
            )));
        }
        let policies = self
            .policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if policies.user_origin == effective::UserLayerSource::Environment {
            return Err(AuthoringError::NotOwned);
        }
        policies
            .owned_user_path
            .clone()
            .ok_or(AuthoringError::NoHome)
    }
}

/// Fold resolved layers and caller restrictions into one immutable snapshot.
fn assemble(
    restrictions: &RequestRestrictions,
    sources: [(Option<manifest::Manifest>, AuthorityTier); 2],
    managed_sequence: Option<u64>,
    valid: bool,
) -> AuthoritySnapshot {
    let mut layers = Vec::new();
    let mut tab_close_allowed = true;
    let mut tab_close_source = None;
    let mut preserve_target_names = true;
    let mut sacred_hosts = Vec::new();
    let mut valid = valid;
    for (policy, tier) in sources {
        let Some(policy) = policy else { continue };
        let index = u16::try_from(layers.len()).expect("policy layer count is bounded");
        if policy.boolean_setting("browser.tabs.allow_close") == Some(false) {
            tab_close_allowed = false;
            tab_close_source.get_or_insert(index);
        }
        if policy.boolean_setting("privacy.preserve_target_names") == Some(false) {
            preserve_target_names = false;
        }
        if let Some(patterns) = policy.string_array_setting("content.security.sacred_domains") {
            sacred_hosts.extend(patterns);
        }
        layers.push(PolicyLayer {
            tier,
            manifest: policy,
        });
    }
    let request_capabilities = restrictions.restrict_capabilities.as_ref().map(|values| {
        values.iter().fold(
            CapabilitySet::EMPTY,
            |set, value| match Capability::from_str(value) {
                Ok(capability) => set.union(capability.into()),
                Err(_) => {
                    valid = false;
                    set
                }
            },
        )
    });
    let request_hosts = restrictions.restrict_hosts.clone();
    if request_hosts.as_ref().is_some_and(|patterns| {
        patterns
            .iter()
            .any(|pattern| !manifest::valid_host_pattern(pattern))
    }) {
        valid = false;
    }
    let id = authority_id(
        &layers,
        request_capabilities,
        request_hosts.as_deref(),
        valid,
    );

    AuthoritySnapshot {
        id,
        managed_sequence,
        layers,
        request_capabilities,
        request_hosts,
        tab_close_allowed,
        tab_close_source,
        preserve_target_names,
        sacred_hosts,
        valid,
    }
}

impl GovernanceFacade {
    /// Check live runtime control at an effect boundary.
    #[must_use]
    pub fn runtime_decision(&self) -> Decision {
        if let Some(path) = &self.runtime_control {
            match fs::read_to_string(path).as_deref().map(str::trim) {
                Ok("active") => self.controls.resume(),
                Ok("hold") => self.controls.hold(),
                Ok("attention") => self.controls.require_attention(),
                Ok("end_session") => self.controls.end_session(),
                _ => self.controls.hold(),
            }
        }
        self.controls.decision()
    }

    fn refresh_policies(&self) {
        self.policies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .refresh();
    }
}

fn read_policy(path: &Path) -> Result<manifest::Manifest, GovernanceError> {
    let text = fs::read_to_string(path)
        .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))?;
    manifest::parse(&text, &path.display().to_string())
        .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))
}

fn authority_id(
    layers: &[PolicyLayer],
    request_capabilities: Option<CapabilitySet>,
    request_hosts: Option<&[String]>,
    valid: bool,
) -> String {
    let mut identity = String::new();
    identity.push_str(if valid { "valid\n" } else { "invalid\n" });
    for layer in layers {
        identity.push_str(layer.tier.as_str());
        identity.push(':');
        identity.push_str(&layer.manifest.hash);
        identity.push('\n');
    }
    if let Some(capabilities) = request_capabilities {
        identity.push_str("capabilities:");
        for capability in capabilities.iter() {
            identity.push_str(capability.as_str());
            identity.push(',');
        }
        identity.push('\n');
    }
    if let Some(hosts) = request_hosts {
        let mut hosts = hosts.to_vec();
        hosts.sort_unstable();
        identity.push_str("hosts:");
        for host in hosts {
            identity.push_str(&host.to_ascii_lowercase());
            identity.push(',');
        }
    }
    let digest = Sha256::digest(identity.as_bytes());
    format!("authority_{}", hex_prefix(&digest, 16))
}

fn hex_prefix(bytes: &[u8], count: usize) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(count * 2);
    for byte in bytes.iter().take(count) {
        write!(&mut output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn protected_url(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    // `url()` gives the typed host directly. The alternative -- `host_str()` then
    // `IpAddr::from_str` -- looked equivalent but was not: `host_str()` keeps the enclosing `[ ]`
    // brackets for an IPv6 host (`"[::1]"`), which `IpAddr::from_str` refuses to parse, so the
    // string round trip silently failed for every IPv6 literal, not only the mapped/compatible
    // forms this function was already missing. This is the one boundary documented as holding
    // unconditionally in every configuration; it must not depend on a string format nobody checked.
    match url.host() {
        Some(url::Host::Domain(host)) => {
            let host = host.to_ascii_lowercase();
            host == "localhost" || host.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(value)) => value.is_loopback() || value.is_link_local(),
        Some(url::Host::Ipv6(value)) => {
            value.is_loopback()
                || (value.segments()[0] & 0xffc0) == 0xfe80
                || embedded_ipv4(value).is_some_and(|v4| v4.is_loopback() || v4.is_link_local())
        }
        None => true,
    }
}

/// Recover the IPv4 address embedded in an IPv6 address carrying one, if any.
///
/// `Ipv6Addr::is_loopback()` only matches the literal `::1`; it does not unwrap an IPv4-mapped
/// (`::ffff:a.b.c.d`) or IPv4-compatible (`::a.b.c.d`) address, both of which are ordinary,
/// browser-parseable IPv6 literals naming a real IPv4 destination. Without this, a boundary
/// documented as holding unconditionally in every configuration -- including all-open, and while
/// ordinary policy observes -- would let `https://[::ffff:127.0.0.1]/` or a link-local
/// cloud-metadata address in mapped form through with no protection at all.
fn embedded_ipv4(v6: Ipv6Addr) -> Option<Ipv4Addr> {
    if let Some(v4) = v6.to_ipv4_mapped() {
        return Some(v4);
    }
    let segments = v6.segments();
    if segments[..6] == [0, 0, 0, 0, 0, 0] && (segments[6] != 0 || segments[7] != 0) {
        return Some(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ));
    }
    None
}

fn host_matches(host: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    if pattern == "*" {
        true
    } else if let Some(suffix) = pattern.strip_prefix("*.") {
        host.len() > suffix.len()
            && host.ends_with(suffix)
            && host.as_bytes().get(host.len() - suffix.len() - 1) == Some(&b'.')
    } else {
        host == pattern
    }
}

fn unix_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

/// A content-minimized audit record produced after a terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRecord {
    /// Wall-clock time in Unix milliseconds.
    pub timestamp_ms: u64,
    /// Opaque invocation handle.
    pub invocation: String,
    /// Opaque workspace handle.
    pub workspace: String,
    /// Exact catalog tool name.
    pub tool: String,
    /// Complete independent RAWX requirement set.
    #[serde(default)]
    pub capabilities: CapabilitySet,
    /// Singular field retained only to read audit lines written before ADR-0121.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<Capability>,
    /// Opaque immutable authority version.
    pub authority: String,
    /// Monotonic signed managed-policy publish sequence, when managed authority was active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_seq: Option<u64>,
    /// Whether final-boundary authority admitted the work.
    pub allowed: bool,
    /// Stable reason code.
    pub reason: ReasonCode,
    /// Whether policy shadowed a denial while allowing work to continue.
    #[serde(default)]
    pub policy_observed: bool,
    /// Effective enforce or observe mode when policy made this decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_mode: Option<String>,
    /// Stable authored rule when policy made this decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_rule: Option<String>,
    /// Stable content-free correlation handle for an authored denial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denial_id: Option<String>,
    /// Tighten-only tier that supplied the deciding rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_tier: Option<String>,
    /// Stable grant id that supplied the deciding rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant_id: Option<String>,
    /// Terminal status vocabulary.
    pub status: String,
    /// Terminal effect class.
    pub effect: String,
    /// Ghostlight-authored sentence naming what happened, with an optional governed target name.
    #[serde(default)]
    pub summary: String,
    /// How long the invocation took, from decode to terminal outcome.
    ///
    /// For a navigation this is the time to a governed, settled landing.
    #[serde(default)]
    pub duration_ms: u64,
    /// What the action did, merged from browser-seam landing facts and outcome measurements.
    #[serde(default)]
    pub observed: Observed,
    /// Which intake the work arrived on. Attribution only, never authority (ADR-0105).
    ///
    /// Absent when the workspace was already gone by the time the record was written.
    #[serde(default)]
    pub channel: Option<IntakeChannel>,
}

impl AuditRecord {
    /// Construct a content-minimized audit record at the current time.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn now(
        invocation: &str,
        workspace: &str,
        tool: &str,
        capabilities: impl Into<CapabilitySet>,
        authority: &str,
        decision: Decision,
        status: &str,
        effect: &str,
        summary: &str,
        duration_ms: u64,
    ) -> Self {
        Self {
            timestamp_ms: unix_ms(),
            invocation: invocation.into(),
            workspace: workspace.into(),
            tool: tool.into(),
            capabilities: capabilities.into(),
            capability: None,
            authority: authority.into(),
            policy_seq: None,
            allowed: decision.allowed,
            reason: decision.reason,
            policy_observed: false,
            policy_mode: None,
            policy_rule: None,
            denial_id: None,
            policy_tier: None,
            grant_id: None,
            status: status.into(),
            effect: effect.into(),
            summary: summary.into(),
            duration_ms,
            observed: Observed::default(),
            channel: None,
        }
    }

    /// Attach the intake the work arrived on.
    #[must_use]
    pub fn from_channel(mut self, channel: Option<IntakeChannel>) -> Self {
        self.channel = channel;
        self
    }

    /// Attach content-free policy attribution from the immutable snapshot that made the decision.
    #[must_use]
    pub fn with_policy(mut self, snapshot: &AuthoritySnapshot, decision: Decision) -> Self {
        self.policy_seq = snapshot.managed_sequence;
        self.policy_observed = decision.observed;
        self.policy_mode = decision.policy_mode().map(str::to_owned);
        self.policy_rule = decision.policy_rule().map(str::to_owned);
        self.denial_id = decision.denial_id();
        if let Some((tier, grant)) = snapshot.attribution(decision) {
            self.policy_tier = Some(tier.into());
            self.grant_id = grant.map(str::to_owned);
        }
        self
    }

    /// Return the truthful requirement set, including a pre-ADR-0121 historical record.
    #[must_use]
    pub fn requirements(&self) -> CapabilitySet {
        self.capabilities
            .union(self.capability.map_or(CapabilitySet::EMPTY, Into::into))
    }

    /// Attach the action's closed observation after completion.
    #[must_use]
    pub fn with_observation(mut self, observed: Observed) -> Self {
        self.observed = observed;
        self
    }
}

/// Separate content-minimized audit output port.
pub trait AuditSink: Send + Sync {
    /// Append one terminal record.
    fn record(&self, record: &AuditRecord) -> io::Result<()>;
}

/// JSONL audit sink guarded for concurrent service invocations.
#[derive(Debug)]
pub struct JsonlAuditSink {
    file: Mutex<std::fs::File>,
}

impl JsonlAuditSink {
    /// Open or create a local append-only audit file.
    pub fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl AuditSink for JsonlAuditSink {
    fn record(&self, record: &AuditRecord) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("audit lock poisoned"))?;
        serde_json::to_writer(&mut *file, record).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        file.flush()
    }
}

/// Governance configuration failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GovernanceError {
    /// A configured authority layer is invalid.
    #[error("invalid authority: {0}")]
    InvalidPolicy(String),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::language::RequestRestrictions;
    use ghostlight_bridge::browser::{RuntimeControlIntent, RuntimeControlState};

    use super::{
        AuditRecord, AuthoringError, Capability, CapabilitySet, Decision, DenialAttention,
        GovernanceFacade, ReasonCode,
    };
    use crate::language::outcome::Observed;

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-1.0-{name}-{}.json",
            uuid::Uuid::new_v4()
        ))
    }

    fn policy(name: &str, grants: &str, config: &str) -> String {
        format!(
            r#"{{"schema":3,"name":"{name}","version":"1","grants":{grants},"config":{config}}}"#
        )
    }

    fn all_open_grant() -> &'static str {
        r#"[{"id":"all","hosts":{"allow":["*"]},"allowed":["read","action","write","execute"]}]"#
    }

    fn snapshot_for(name: &str, source: impl AsRef<[u8]>) -> super::AuthoritySnapshot {
        let path = temporary(name);
        fs::write(&path, source).unwrap();
        let snapshot = GovernanceFacade::new(Some(path.clone()), None)
            .snapshot(&RequestRestrictions::default());
        let _ = fs::remove_file(path);
        snapshot
    }

    #[test]
    fn no_policy_allows_remote_browser_work_but_protected_hosts_remain_denied() {
        let facade = GovernanceFacade::new(None, None);
        let snapshot = facade.snapshot(&RequestRestrictions::default());
        assert!(snapshot.preserves_target_names());
        assert!(snapshot.authorize_tab_close().allowed);
        assert!(
            snapshot
                .authorize_landing(Capability::Action, "https://example.com")
                .allowed
        );
        assert_eq!(
            snapshot
                .authorize_landing(Capability::Action, "http://127.0.0.1:3000")
                .reason,
            ReasonCode::ProtectedHost
        );
        assert_eq!(
            snapshot
                .authorize_landing(Capability::Read, "http://169.254.169.254/latest")
                .reason,
            ReasonCode::ProtectedHost
        );
    }

    #[test]
    fn the_loopback_and_link_local_ceiling_holds_for_ipv4_embedded_in_ipv6() {
        // These are ordinary, browser-parseable literals naming the same protected destinations
        // as their plain IPv4 form. A ceiling that only recognized ::1 would let all of these
        // through untouched, in every configuration including all-open.
        let facade = GovernanceFacade::new(None, None);
        let snapshot = facade.snapshot(&RequestRestrictions::default());
        for url in [
            "https://[::1]/",
            "https://[::ffff:127.0.0.1]:9200/",
            "https://[::ffff:169.254.169.254]/latest/meta-data/",
            "https://[::127.0.0.1]/",
        ] {
            assert_eq!(
                snapshot.authorize_landing(Capability::Read, url).reason,
                ReasonCode::ProtectedHost,
                "{url} must be protected"
            );
        }
        // An ordinary global IPv6 address embedding neither loopback nor link-local octets must
        // not be swept up by the same check.
        assert!(
            snapshot
                .authorize_landing(Capability::Read, "https://[::ffff:8.8.8.8]/")
                .allowed
        );
    }

    #[test]
    fn tab_close_policy_is_monotonic_across_authority_layers() {
        let local = temporary("local-tab-close");
        let managed = temporary("managed-tab-close");
        fs::write(
            &local,
            policy(
                "local",
                all_open_grant(),
                r#"[{"key":"browser.tabs.allow_close","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        fs::write(
            &managed,
            policy(
                "managed",
                all_open_grant(),
                r#"[{"key":"browser.tabs.allow_close","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();
        let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()))
            .snapshot(&RequestRestrictions::default());
        assert_eq!(
            snapshot.authorize_tab_close().reason,
            ReasonCode::TabCloseDenied
        );
        assert!(snapshot.authorize_capability(Capability::Action).allowed);
        let _ = fs::remove_file(local);
        let _ = fs::remove_file(managed);
    }

    #[test]
    fn target_name_preservation_is_default_on_and_monotonic_across_layers() {
        let local = temporary("local-target-names");
        let managed = temporary("managed-target-names");
        fs::write(
            &local,
            policy(
                "local",
                all_open_grant(),
                r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        fs::write(
            &managed,
            policy(
                "managed",
                all_open_grant(),
                r#"[{"key":"privacy.preserve_target_names","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();

        let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()))
            .snapshot(&RequestRestrictions::default());
        assert!(!snapshot.preserves_target_names());

        fs::write(
            &local,
            policy(
                "local",
                all_open_grant(),
                r#"[{"key":"privacy.preserve_target_names","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();
        fs::write(
            &managed,
            policy(
                "managed",
                all_open_grant(),
                r#"[{"key":"privacy.preserve_target_names","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()))
            .snapshot(&RequestRestrictions::default());
        assert!(!snapshot.preserves_target_names());

        let _ = fs::remove_file(local);
        let _ = fs::remove_file(managed);
    }

    #[test]
    fn request_restrictions_only_tighten() {
        let facade = GovernanceFacade::new(None, None);
        let restrictions = RequestRestrictions {
            restrict_hosts: Some(vec!["example.com".into()]),
            restrict_capabilities: Some(vec!["read".into()]),
        };
        let snapshot = facade.snapshot(&restrictions);
        assert!(
            snapshot
                .authorize_landing(Capability::Read, "https://example.com")
                .allowed
        );
        assert_eq!(
            snapshot
                .authorize_landing(Capability::Action, "https://example.com")
                .reason,
            ReasonCode::CapabilityDenied
        );
        assert_eq!(
            snapshot
                .authorize_landing(Capability::Read, "https://example.org")
                .reason,
            ReasonCode::HostDenied
        );
    }

    #[test]
    fn authority_requires_every_independent_capability_in_a_compound_set() {
        let path = temporary("compound-capabilities");
        fs::write(
            &path,
            policy(
                "compound",
                r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]},{"id":"write","hosts":{"allow":["*"]},"allowed":["write"]}]"#,
                "[]",
            ),
        )
        .unwrap();
        let snapshot = GovernanceFacade::new(Some(path.clone()), None)
            .snapshot(&RequestRestrictions::default());
        assert!(
            snapshot
                .authorize_requirements(CapabilitySet::READ.union(CapabilitySet::WRITE))
                .allowed
        );
        assert_eq!(
            snapshot
                .authorize_requirements(
                    CapabilitySet::READ
                        .union(CapabilitySet::WRITE)
                        .union(CapabilitySet::ACTION),
                )
                .reason,
            ReasonCode::CapabilityDenied
        );
        assert!(
            snapshot
                .authorize_requirements(CapabilitySet::EMPTY)
                .allowed
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn host_specificity_denies_ties_but_more_specific_allows_win() {
        let exact_allow = snapshot_for(
            "exact-allow",
            policy(
                "exact allow",
                r#"[{"id":"g","hosts":{"allow":["admin.example.com"],"deny":["*.example.com"]},"allowed":["read"]}]"#,
                "[]",
            ),
        );
        assert!(
            exact_allow
                .authorize_landing(Capability::Read, "https://admin.example.com")
                .allowed
        );

        let exact_tie = snapshot_for(
            "exact-tie",
            policy(
                "exact tie",
                r#"[{"id":"g","hosts":{"allow":["admin.example.com"],"deny":["admin.example.com"]},"allowed":["read"]}]"#,
                "[]",
            ),
        );
        let denied = exact_tie.authorize_landing(Capability::Read, "https://admin.example.com");
        assert!(!denied.allowed);
        assert_eq!(denied.policy_rule(), Some("denied_host"));

        let longer_deny = snapshot_for(
            "longer-deny",
            policy(
                "longer deny",
                r#"[{"id":"g","hosts":{"allow":["*.example.com"],"deny":["*.secure.example.com"]},"allowed":["read"]}]"#,
                "[]",
            ),
        );
        assert!(
            !longer_deny
                .authorize_landing(Capability::Read, "https://a.secure.example.com")
                .allowed
        );
    }

    #[test]
    fn a_grant_deny_only_shrinks_that_grant_and_search_continues_for_an_admission() {
        let snapshot = snapshot_for(
            "grant-order",
            policy(
                "grant order",
                r#"[{"id":"carved","hosts":{"allow":["*.example.com"],"deny":["admin.example.com"]},"allowed":["read"]},{"id":"admin","hosts":{"allow":["admin.example.com"]},"allowed":["read"]}]"#,
                "[]",
            ),
        );
        assert!(
            snapshot
                .authorize_landing(Capability::Read, "https://admin.example.com")
                .allowed
        );

        let capabilities = snapshot_for(
            "grant-capability-order",
            policy(
                "grant capability order",
                r#"[{"id":"read","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"action","hosts":{"allow":["example.com"]},"allowed":["action"]}]"#,
                "[]",
            ),
        );
        assert!(
            capabilities
                .authorize_landing(Capability::Action, "https://example.com")
                .allowed,
            "the first host match is not a refusal when a later grant admits the full set"
        );
    }

    #[test]
    fn first_applicable_denial_is_stably_attributed() {
        let snapshot = snapshot_for(
            "first-denial",
            policy(
                "first denial",
                r#"[{"id":"first","hosts":{"allow":["example.com"]},"allowed":["read"]},{"id":"second","hosts":{"allow":["other.example"]},"allowed":["action"]}]"#,
                "[]",
            ),
        );
        let first = snapshot.authorize_landing(Capability::Action, "https://example.com");
        let second = snapshot.authorize_landing(Capability::Action, "https://example.com");
        assert!(!first.allowed);
        assert_eq!(first.policy_rule(), Some("capability"));
        assert_eq!(snapshot.attribution(first), Some(("user", Some("first"))));
        assert_eq!(first.denial_id(), second.denial_id());

        let record = AuditRecord::now(
            "invocation_policy",
            "workspace_policy",
            "browser_click",
            Capability::Action,
            snapshot.id(),
            first,
            "blocked",
            "none",
            "Authority blocked the action.",
            0,
        )
        .with_policy(&snapshot, first);
        assert_eq!(record.policy_tier.as_deref(), Some("user"));
        assert_eq!(record.grant_id.as_deref(), Some("first"));
        assert_eq!(record.policy_rule.as_deref(), Some("capability"));
        assert_eq!(record.policy_mode.as_deref(), Some("enforce"));
        assert_eq!(record.denial_id, first.denial_id());

        let changed = snapshot_for(
            "changed-denial",
            policy(
                "changed denial",
                r#"[{"id":"renamed","hosts":{"allow":["example.com"]},"allowed":["read"]}]"#,
                "[]",
            ),
        )
        .authorize_landing(Capability::Action, "https://example.com");
        assert_ne!(first.denial_id(), changed.denial_id());
    }

    #[test]
    fn observe_shadows_ordinary_denials_but_never_protected_resources() {
        let snapshot = snapshot_for(
            "observe",
            r#"{"schema":3,"name":"observe","version":"1","mode":"observe","grants":[]}"#,
        );
        let ordinary = snapshot.authorize_landing(Capability::Read, "https://example.com");
        assert!(ordinary.allowed);
        assert!(ordinary.observed);
        assert_eq!(ordinary.reason, ReasonCode::HostDenied);
        assert!(ordinary.denial_id().is_some());

        let protected = snapshot.authorize_landing(Capability::Read, "http://127.0.0.1");
        assert!(!protected.allowed);
        assert!(!protected.observed);
        assert_eq!(protected.reason, ReasonCode::ProtectedHost);

        let sacred = snapshot_for(
            "observe-sacred",
            r#"{"schema":3,"name":"observe sacred","version":"1","mode":"observe","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}],"config":[{"key":"content.security.sacred_domains","value":["example.com"],"level":"mandatory"}]}"#,
        );
        assert!(
            !sacred
                .authorize_landing(Capability::Read, "https://example.com")
                .allowed
        );
    }

    #[test]
    fn strictest_layer_mode_wins_and_snapshots_have_deterministic_identity() {
        let managed = temporary("mode-managed");
        let local = temporary("mode-local");
        fs::write(
            &managed,
            r#"{"schema":3,"name":"managed","version":"1","mode":"observe","grants":[]}"#,
        )
        .unwrap();
        fs::write(
            &local,
            r#"{"schema":3,"name":"local","version":"1","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
        let first = facade.snapshot(&RequestRestrictions::default());
        let same = facade.snapshot(&RequestRestrictions::default());
        let denied = first.authorize_landing(Capability::Read, "https://example.com");
        assert!(
            !denied.allowed,
            "the enforcing local tier prevents shadow admission"
        );
        assert_eq!(first.id(), same.id());

        fs::write(
            &local,
            r#"{"schema":3,"name":"local","version":"2","grants":[{"id":"all","hosts":{"allow":["*"]},"allowed":["read"]}]}"#,
        )
        .unwrap();
        let changed = facade.snapshot(&RequestRestrictions::default());
        assert_ne!(first.id(), changed.id());
        let _ = fs::remove_file(managed);
        let _ = fs::remove_file(local);
    }

    #[test]
    fn maintained_policy_examples_match_the_schema_three_decoder() {
        for (name, source) in [
            (
                "research-read-only",
                include_str!("../../../../examples/research-read-only.json"),
            ),
            (
                "qa-staging",
                include_str!("../../../../examples/qa-staging.json"),
            ),
            (
                "enterprise-healthcare",
                include_str!("../../../../examples/enterprise-healthcare.json"),
            ),
            (
                "developer-unrestricted",
                include_str!("../../../../examples/developer-unrestricted.json"),
            ),
            (
                "developer-observe",
                include_str!("../../../../examples/developer-observe.json"),
            ),
            (
                "dev-live-test",
                include_str!("../../../../examples/dev-live-test.json"),
            ),
            (
                "demo-policy",
                include_str!("../../../../examples/demo-policy.json"),
            ),
            (
                "scripting-disabled",
                include_str!("../../../../examples/scripting-disabled.json"),
            ),
            (
                "personal-starter",
                include_str!("../../../../examples/personal-starter.json"),
            ),
            (
                "personal-everywhere-except",
                include_str!("../../../../examples/personal-everywhere-except.json"),
            ),
            (
                "no-page-code",
                include_str!("../../../../examples/no-page-code.json"),
            ),
            (
                "organization-support",
                include_str!("../../../../examples/organization-support.json"),
            ),
            (
                "organization-locked-fleet",
                include_str!("../../../../examples/organization-locked-fleet.json"),
            ),
        ] {
            let path = temporary(name);
            fs::write(&path, source).unwrap();
            assert!(
                super::read_policy(&path).is_ok(),
                "{name} must remain a valid 1.0 policy"
            );
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn authoring_validates_before_it_replaces_and_never_leaves_the_product_failing_closed() {
        let path = temporary("authored-policy");
        let facade = GovernanceFacade::owning_user_policy(path.clone(), None);
        // A machine that has never authored one is open, not failing closed.
        assert!(
            facade
                .snapshot(&RequestRestrictions::default())
                .authorize_capability(Capability::Execute)
                .allowed
        );

        let good = r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"reading","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#;
        facade.apply_user_policy(good).unwrap();
        let applied = facade.snapshot(&RequestRestrictions::default());
        assert!(
            applied
                .authorize_landing(CapabilitySet::READ, "https://example.com")
                .allowed
        );
        assert!(
            !applied
                .authorize_landing(CapabilitySet::EXECUTE, "https://example.com")
                .allowed
        );

        // A rejected document leaves both the file and the authority exactly as they were.
        let error = facade.apply_user_policy("{\"schema\":3,").unwrap_err();
        assert!(matches!(error, AuthoringError::Invalid(_)));
        assert_eq!(fs::read_to_string(&path).unwrap(), good);
        assert!(
            facade
                .snapshot(&RequestRestrictions::default())
                .authorize_landing(CapabilitySet::READ, "https://example.com")
                .allowed
        );

        // The organization switch is not something a user layer may author for itself.
        let overreach = r#"{"schema":3,"name":"mine","version":"2","grants":[],"config":[{"key":"policy.user.enabled","value":true,"level":"mandatory"}]}"#;
        assert!(matches!(
            facade.apply_user_policy(overreach).unwrap_err(),
            AuthoringError::OrganizationOnlySetting(_)
        ));

        facade.remove_user_policy().unwrap();
        assert!(!path.exists());
        assert!(
            facade
                .snapshot(&RequestRestrictions::default())
                .authorize_capability(Capability::Execute)
                .allowed
        );
        // Removing what is already gone is not a failure.
        facade.remove_user_policy().unwrap();
    }

    #[test]
    fn a_policy_file_ghostlight_does_not_own_is_never_written_back() {
        let path = temporary("foreign-policy");
        fs::write(
            &path,
            br#"{"schema":3,"name":"theirs","version":"1","grants":[]}"#,
        )
        .unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        assert!(matches!(
            facade
                .apply_user_policy(r#"{"schema":3,"name":"mine","version":"1","grants":[]}"#)
                .unwrap_err(),
            AuthoringError::NotOwned
        ));
        assert!(matches!(
            facade.remove_user_policy().unwrap_err(),
            AuthoringError::NotOwned
        ));
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn a_candidate_policy_is_decided_under_the_organization_ceiling_without_being_applied() {
        let managed = temporary("preview-managed");
        let owned = temporary("preview-user");
        fs::write(
            &managed,
            br#"{"schema":3,"name":"org","version":"1","grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}]}"#,
        )
        .unwrap();
        let facade = GovernanceFacade::owning_user_policy(owned.clone(), Some(managed.clone()));

        let decided = facade
            .candidate_snapshot(
                r#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"wide","hosts":{"allow":["*"]},"allowed":["read","action","write"]}]}"#,
            )
            .unwrap();
        assert_eq!(decided.rules, 1);
        let candidate = decided.snapshot;
        // The candidate asks for more than the organization allows, so the ceiling still wins.
        assert!(
            candidate
                .authorize_landing(CapabilitySet::READ, "https://example.com")
                .allowed
        );
        assert!(
            !candidate
                .authorize_landing(CapabilitySet::WRITE, "https://example.com")
                .allowed
        );
        assert!(
            !candidate
                .authorize_landing(CapabilitySet::READ, "https://elsewhere.test")
                .allowed
        );
        // Previewing writes nothing.
        assert!(!owned.exists());
        assert!(facade.candidate_snapshot("not json").is_err());

        let _ = fs::remove_file(managed);
    }

    #[test]
    fn an_organization_may_switch_off_user_authoring_without_widening_authority() {
        let managed = temporary("authoring-managed");
        let local = temporary("authoring-local");
        fs::write(
            &managed,
            br#"{"schema":3,"name":"org","version":"1","grants":[{"id":"work","hosts":{"allow":["example.com"]},"allowed":["read","action"]}],"config":[{"key":"policy.user.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
        fs::write(
            &local,
            br#"{"schema":3,"name":"mine","version":"1","grants":[{"id":"narrow","hosts":{"allow":["example.com"]},"allowed":["read"]}]}"#,
        )
        .unwrap();

        let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
        assert!(!facade.user_authoring_allowed());
        // The switch gates authoring only. An existing user layer keeps subtracting, because
        // ignoring it would restore authority the organization never granted back.
        let snapshot = facade.snapshot(&RequestRestrictions::default());
        assert!(
            snapshot
                .authorize_landing(CapabilitySet::READ, "https://example.com")
                .allowed
        );
        assert!(
            !snapshot
                .authorize_landing(CapabilitySet::ACTION, "https://example.com")
                .allowed
        );

        assert!(GovernanceFacade::new(Some(local.clone()), None).user_authoring_allowed());
        let _ = fs::remove_file(managed);
        let _ = fs::remove_file(local);
    }

    #[test]
    fn invalid_managed_authority_fails_closed() {
        let path = temporary("invalid-managed");
        fs::write(&path, br#"{"version":1,"managed":false}"#).unwrap();
        let facade = GovernanceFacade::new(None, Some(path.clone()));
        let snapshot = facade.snapshot(&RequestRestrictions::default());
        assert_eq!(
            snapshot.authorize_capability(Capability::Read).reason,
            ReasonCode::InvalidAuthority
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn an_unreachable_managed_authority_fails_closed_from_cold_start() {
        let path = temporary("missing-managed");
        let _ = fs::remove_file(&path);
        let facade = GovernanceFacade::new(None, Some(path));
        let snapshot = facade.snapshot(&RequestRestrictions::default());
        assert_eq!(
            snapshot.authorize_capability(Capability::Read).reason,
            ReasonCode::InvalidAuthority
        );
    }

    #[test]
    fn snapshot_does_not_change_when_policy_file_changes() {
        let path = temporary("immutable");
        fs::write(
            &path,
            policy(
                "read",
                r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
                "[]",
            ),
        )
        .unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        let first = facade.snapshot(&RequestRestrictions::default());
        fs::write(
            &path,
            policy(
                "action",
                r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
                "[]",
            ),
        )
        .unwrap();
        assert!(first.authorize_capability(Capability::Read).allowed);
        assert!(!first.authorize_capability(Capability::Action).allowed);
        let second = facade.snapshot(&RequestRestrictions::default());
        assert!(!second.authorize_capability(Capability::Read).allowed);
        assert!(second.authorize_capability(Capability::Action).allowed);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_reload_keeps_last_valid_authority_until_a_valid_replacement_arrives() {
        let path = temporary("last-known-good");
        fs::write(
            &path,
            policy(
                "read",
                r#"[{"id":"read","hosts":{"allow":["*"]},"allowed":["read"]}]"#,
                "[]",
            ),
        )
        .unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        assert!(
            facade
                .snapshot(&RequestRestrictions::default())
                .authorize_capability(Capability::Read)
                .allowed
        );

        fs::write(&path, "{half-written").unwrap();
        let retained = facade.snapshot(&RequestRestrictions::default());
        assert!(retained.authorize_capability(Capability::Read).allowed);
        assert!(!retained.authorize_capability(Capability::Action).allowed);
        let retained_diagnostics = facade.diagnostics();
        assert!(retained_diagnostics.local_policy_active);
        assert!(!retained_diagnostics.local_policy_valid);

        fs::write(
            &path,
            policy(
                "action",
                r#"[{"id":"action","hosts":{"allow":["*"]},"allowed":["action"]}]"#,
                "[]",
            ),
        )
        .unwrap();
        let replaced = facade.snapshot(&RequestRestrictions::default());
        assert!(!replaced.authorize_capability(Capability::Read).allowed);
        assert!(replaced.authorize_capability(Capability::Action).allowed);
        let replaced_diagnostics = facade.diagnostics();
        assert!(replaced_diagnostics.local_policy_active);
        assert!(replaced_diagnostics.local_policy_valid);
        let _ = fs::remove_file(path);
    }

    fn sample_record() -> AuditRecord {
        AuditRecord::now(
            "invocation_x",
            "workspace_x",
            "browser_fill_form",
            CapabilitySet::READ.union(CapabilitySet::WRITE),
            "authority_x",
            Decision::permitted(),
            "succeeded",
            "applied",
            "Page text read.",
            1200,
        )
    }

    /// Every key anywhere in the record, so a payload cannot hide one level down.
    fn keys(value: &serde_json::Value, found: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, nested) in object {
                    found.push(key.clone());
                    keys(nested, found);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    keys(item, found);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn audit_record_has_no_payload_fields() {
        let record = sample_record().with_observation(Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(3),
            width: Some(1280),
            height: Some(720),
        });
        let value = serde_json::to_value(record).unwrap();
        let mut found = Vec::new();
        keys(&value, &mut found);
        // The walk must reach nested keys, or it would pass any payload hidden one level down.
        assert!(found.contains(&"observed".to_owned()));
        assert!(
            found.contains(&"host".to_owned()),
            "the walk stops at the top"
        );
        for forbidden in [
            "url",
            "text",
            "content",
            "selector",
            "value",
            "screenshot",
            "dialog",
            "path",
            "query",
        ] {
            assert!(
                !found.contains(&forbidden.to_owned()),
                "the record grew a {forbidden} field"
            );
        }
    }

    #[test]
    fn new_audit_records_serialize_the_complete_set_without_a_highest_capability() {
        let value = serde_json::to_value(sample_record()).unwrap();
        assert_eq!(value["capabilities"], serde_json::json!(["read", "write"]));
        assert!(value.get("capability").is_none());

        let historical: AuditRecord = serde_json::from_value(serde_json::json!({
            "timestamp_ms": 1,
            "invocation": "invocation_old",
            "workspace": "workspace_old",
            "tool": "browser_read",
            "capability": "read",
            "authority": "authority_old",
            "allowed": true,
            "reason": "permitted",
            "status": "succeeded",
            "effect": "none"
        }))
        .expect("historical audit record remains readable");
        assert_eq!(historical.requirements(), CapabilitySet::READ);
    }

    /// The record has exactly one URL-shaped field and it is a host.
    ///
    /// What the executor puts in it is guarded where it is extracted, in `work`; this pins that
    /// there is nowhere else for the rest of a URL to travel.
    #[test]
    fn an_observation_has_one_place_for_a_host_and_none_for_the_rest_of_a_url() {
        let record = sample_record().with_observation(Observed {
            host: Some("example.com".into()),
            readiness: Some("complete".into()),
            count: Some(3),
            width: Some(1280),
            height: Some(720),
        });
        let encoded = serde_json::to_value(&record).unwrap();
        let observed = encoded["observed"].as_object().unwrap();
        assert_eq!(observed["host"], "example.com");
        let mut text: Vec<&str> = observed
            .iter()
            .filter(|(_, value)| value.is_string())
            .map(|(key, _)| key.as_str())
            .collect();
        text.sort_unstable();
        assert_eq!(
            text,
            ["host", "readiness"],
            "an observation grew another field a URL could travel in"
        );
    }

    #[test]
    fn naming_a_channel_is_how_a_layer_takes_control_of_it() {
        use ghostlight_bridge::service::IntakeChannel;

        let unconfigured = GovernanceFacade::new(None, None);
        for channel in [IntakeChannel::Mcp, IntakeChannel::Cli] {
            assert!(
                unconfigured.admits_channel(channel).allowed,
                "an unconfigured Ghostlight must admit every intake"
            );
        }

        let path = temporary("channel-empty");
        fs::write(
            &path,
            policy(
                "channel-off",
                "[]",
                r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let empty = GovernanceFacade::new(Some(path.clone()), None);
        assert_eq!(
            empty.admits_channel(IntakeChannel::Cli).reason,
            ReasonCode::ChannelDenied
        );
        assert!(
            empty.admits_channel(IntakeChannel::Mcp).allowed,
            "a layer restricts only the channels it names"
        );
        let _ = fs::remove_file(path);

        let path = temporary("channel-false");
        fs::write(
            &path,
            policy(
                "channels",
                "[]",
                r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"},{"key":"channels.mcp.enabled","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();
        let explicit = GovernanceFacade::new(Some(path.clone()), None);
        assert!(!explicit.admits_channel(IntakeChannel::Cli).allowed);
        assert!(explicit.admits_channel(IntakeChannel::Mcp).allowed);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn a_local_layer_cannot_readmit_a_channel_managed_authority_refused() {
        use ghostlight_bridge::service::IntakeChannel;

        let managed = temporary("channel-managed");
        fs::write(
            &managed,
            policy(
                "managed",
                "[]",
                r#"[{"key":"channels.cli.enabled","value":false,"level":"mandatory"}]"#,
            ),
        )
        .unwrap();
        let local = temporary("channel-local");
        fs::write(
            &local,
            policy(
                "local",
                "[]",
                r#"[{"key":"channels.cli.enabled","value":true,"level":"recommended"}]"#,
            ),
        )
        .unwrap();

        let facade = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()));
        assert_eq!(
            facade.admits_channel(IntakeChannel::Cli).reason,
            ReasonCode::ChannelDenied,
            "layers compose by intersection; local cannot hand access back"
        );
        // The negative control: the same local layer alone does admit it, so the refusal above
        // is the managed layer's doing and not an accident of parsing.
        let alone = GovernanceFacade::new(Some(local.clone()), None);
        assert!(alone.admits_channel(IntakeChannel::Cli).allowed);
        let _ = fs::remove_file(managed);
        let _ = fs::remove_file(local);
    }

    #[test]
    fn an_unknown_channel_name_is_a_typo_not_a_silent_pass() {
        use ghostlight_bridge::service::IntakeChannel;

        let path = temporary("channel-typo");
        fs::write(
            &path,
            r#"{"schema":3,"name":"typo","version":"1","grants":[],"config":[{"key":"channels.cli-tool.enabled","value":false,"level":"mandatory"}]}"#,
        )
        .unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        assert_eq!(
            facade.admits_channel(IntakeChannel::Cli).reason,
            ReasonCode::InvalidAuthority,
            "a misspelled channel must fail closed rather than restrict nothing"
        );
        let _ = fs::remove_file(path);
    }

    /// A pause denies before any effect and a resume restores work, without anything waiting.
    ///
    /// ADR-0126 Decision 4 chose refusal over a held caller, so this is the whole mechanism: the
    /// final boundary answers deny while held and allow once resumed. There is no queue to drain
    /// and no deadline to reconcile.
    #[test]
    fn pause_prevents_the_next_browser_effect_and_resume_restores_it() {
        let facade = GovernanceFacade::new(None, None);
        assert!(facade.runtime_decision().allowed);

        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::Hold),
            RuntimeControlState::Held
        );
        assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
        assert!(!facade.runtime_decision().allowed);
        // Idempotent: pausing twice is still one paused state.
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::Hold),
            RuntimeControlState::Held
        );

        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::Resume),
            RuntimeControlState::Active
        );
        assert!(facade.runtime_decision().allowed);
    }

    /// Stop is terminal and idempotent, and a resume cannot undo it.
    ///
    /// Only an explicit new session leaves the ended state, so a model that retries after the stop
    /// directive gets the same refusal rather than quietly resuming the work a person interrupted.
    #[test]
    fn stop_is_terminal_and_idempotent() {
        let facade = GovernanceFacade::new(None, None);
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
            RuntimeControlState::Ended
        );
        assert_eq!(facade.runtime_decision().reason, ReasonCode::SessionEnded);
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
            RuntimeControlState::Ended
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::Resume),
            RuntimeControlState::Ended,
            "a resume must not undo a stop"
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::StartSession),
            RuntimeControlState::Active
        );
    }

    /// A policy attention hold is its own state, not the person's pause.
    #[test]
    fn attention_stays_distinct_from_a_human_pause() {
        let facade = GovernanceFacade::new(None, None);
        facade.apply_runtime_intent(RuntimeControlIntent::Hold);
        assert_eq!(facade.runtime_state(), RuntimeControlState::Held);
        assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
        assert_ne!(
            facade.runtime_decision().reason,
            ReasonCode::RuntimeAttention
        );
    }

    #[test]
    fn runtime_control_file_is_checked_at_each_final_boundary() {
        let path = temporary("runtime-control");
        fs::write(&path, "hold").unwrap();
        let facade =
            GovernanceFacade::new(None, None).with_runtime_control_file(Some(path.clone()));
        assert_eq!(facade.runtime_decision().reason, ReasonCode::RuntimeHold);
        fs::write(&path, "attention").unwrap();
        assert_eq!(
            facade.runtime_decision().reason,
            ReasonCode::RuntimeAttention
        );
        fs::write(&path, "end_session").unwrap();
        assert_eq!(facade.runtime_decision().reason, ReasonCode::SessionEnded);
        fs::write(&path, "active").unwrap();
        assert!(facade.runtime_decision().allowed);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn human_control_intents_are_authoritative_and_end_is_terminal() {
        let facade = GovernanceFacade::new(None, None);
        assert_eq!(facade.runtime_state(), RuntimeControlState::Active);
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::ToggleHold),
            RuntimeControlState::Held
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::ToggleHold),
            RuntimeControlState::Active
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::EndSession),
            RuntimeControlState::Ended
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::Resume),
            RuntimeControlState::Ended
        );
        assert_eq!(
            facade.apply_runtime_intent(RuntimeControlIntent::StartSession),
            RuntimeControlState::Active
        );
    }

    #[test]
    fn repeated_matching_denials_require_attention_per_workspace() {
        let mut attention = DenialAttention::default();
        let denied = Decision::deny(ReasonCode::CapabilityDenied);

        assert!(!attention.record("workspace_one", denied, 1_000));
        assert!(!attention.record("workspace_two", denied, 10_000));
        assert!(!attention.record("workspace_one", denied, 30_000));
        assert!(attention.record("workspace_one", denied, 60_000));
        assert!(!attention.record("workspace_one", denied, 61_000));
        assert!(!attention.record("workspace_two", denied, 61_000));
    }

    #[test]
    fn five_distinct_enforced_denials_require_attention_and_old_attempts_expire() {
        let mut attention = DenialAttention::default();
        for (index, reason) in [
            ReasonCode::CapabilityDenied,
            ReasonCode::TabCloseDenied,
            ReasonCode::HostDenied,
            ReasonCode::ProtectedHost,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(!attention.record("workspace", Decision::deny(reason), index as u64 * 1_000));
        }
        assert!(attention.record(
            "workspace",
            Decision::deny(ReasonCode::InvalidAuthority),
            4_000
        ));

        assert!(!attention.record(
            "expired",
            Decision::deny(ReasonCode::CapabilityDenied),
            1_000
        ));
        assert!(!attention.record(
            "expired",
            Decision::deny(ReasonCode::CapabilityDenied),
            122_000
        ));
    }
}
