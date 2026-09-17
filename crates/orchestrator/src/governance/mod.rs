// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Authority snapshots, final-boundary admission, runtime controls, and minimized audit intent.

pub mod audit;
pub mod documents;
pub use crate::audit::{AuditSink, JsonlAuditSink};
pub mod effective;
pub mod evidence;
pub mod inspection;
pub mod managed;
pub mod manifest;
pub mod paths;

use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
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

use crate::language::audit::{AuditProjection, AuditRefusal};
use crate::language::outcome::Observed;

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
    /// Policy requires working audit storage at the next browser boundary.
    AuditUnavailable,
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
            Self::AuditUnavailable => "audit_unavailable",
        }
    }
}

/// Stable policy rule names used for attribution and denial ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyRule {
    /// A configured requirement prevents browser work while storage is unavailable.
    AuditAvailability,
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
            Self::AuditAvailability => "audit_availability",
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

#[derive(Clone, Debug)]
struct LayerOutcome {
    grants: Vec<u16>,
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
        self.decide_requirements(requirements).0
    }

    fn decide_requirements(&self, requirements: CapabilitySet) -> (Decision, Vec<LayerOutcome>) {
        if !self.valid {
            return (Decision::deny(ReasonCode::InvalidAuthority), vec![]);
        }
        if requirements.is_empty() {
            return (Decision::allow(), vec![]);
        }
        let outcomes: Vec<_> = self
            .layers
            .iter()
            .map(|layer| decide_resource_less(layer, requirements))
            .collect();
        let policy_decision = self.resolve_outcomes(&outcomes);
        if let Some(decision) = policy_decision.filter(|decision| !decision.allowed) {
            return (decision, outcomes);
        }
        (policy_decision.unwrap_or_else(Decision::allow), outcomes)
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
        self.decide_landing(requirements.into(), url).0
    }

    fn decide_landing(
        &self,
        requirements: CapabilitySet,
        url: &str,
    ) -> (Decision, Vec<LayerOutcome>) {
        if !self.valid {
            return (Decision::deny(ReasonCode::InvalidAuthority), vec![]);
        }
        let Ok(parsed) = Url::parse(url) else {
            return (Decision::deny(ReasonCode::HostDenied), vec![]);
        };
        if !matches!(parsed.scheme(), "http" | "https")
            || protected_by_policy(&parsed, &self.sacred_hosts)
        {
            return (Decision::deny(ReasonCode::ProtectedHost), vec![]);
        }
        let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
            return (Decision::deny(ReasonCode::HostDenied), vec![]);
        };
        if requirements.is_empty() {
            return (Decision::allow(), vec![]);
        }
        let outcomes: Vec<_> = self
            .layers
            .iter()
            .map(|layer| decide_for_host(layer, requirements, &host))
            .collect();
        let policy_decision = self.resolve_outcomes(&outcomes);
        if let Some(decision) = policy_decision.filter(|decision| !decision.allowed) {
            return (decision, outcomes);
        }
        (policy_decision.unwrap_or_else(Decision::allow), outcomes)
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

    /// Policy tier and grant id for one decision, when authored policy decided it.
    #[must_use]
    pub fn attribution(&self, decision: Decision) -> Option<(&'static str, Option<&str>)> {
        let attribution = decision.attribution?;
        let layer = self.layers.get(usize::from(attribution.layer))?;
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
            grants: vec![],
            denial: None,
            mode: grant.mode.or(layer.manifest.mode).unwrap_or_default(),
        };
    }
    LayerOutcome {
        grants: vec![],
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
        return LayerOutcome {
            grants: layer
                .manifest
                .grants
                .iter()
                .enumerate()
                .filter(|(_, grant)| grant.allowed_set().intersects(requirements))
                .map(|(index, _)| u16::try_from(index).expect("bounded grants"))
                .collect(),
            denial: None,
            mode,
        };
    }
    LayerOutcome {
        grants: vec![],
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
                    return LayerOutcome {
                        grants: vec![grant_index],
                        denial: None,
                        mode,
                    };
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
        grants: vec![],
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
        let sacred_hosts = self.snapshot().sacred_hosts().to_vec();
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

    /// Build one immutable snapshot from configured policy authority.
    #[must_use]
    pub fn snapshot(&self) -> AuthoritySnapshot {
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
        assemble(sources, managed_sequence, valid)
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

/// Fold resolved policy layers into one immutable snapshot.
fn assemble(
    sources: [(Option<manifest::Manifest>, AuthorityTier); 2],
    managed_sequence: Option<u64>,
    valid: bool,
) -> AuthoritySnapshot {
    let mut layers = Vec::new();
    let mut tab_close_allowed = true;
    let mut tab_close_source = None;
    let mut preserve_target_names = true;
    let mut sacred_hosts = Vec::new();
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
    let id = authority_id(&layers, valid);

    AuthoritySnapshot {
        id,
        managed_sequence,
        layers,
        tab_close_allowed,
        tab_close_source,
        preserve_target_names,
        sacred_hosts,
        valid,
    }
}

impl GovernanceFacade {
    /// Combine independent global human control with the workspace's review requirement.
    #[must_use]
    pub fn session_decision(&self, needs_attention: bool) -> Decision {
        let global = self.runtime_decision();
        if global.allowed && needs_attention {
            Decision::refused(ReasonCode::RuntimeAttention)
        } else {
            global
        }
    }

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

fn authority_id(layers: &[PolicyLayer], valid: bool) -> String {
    let mut identity = String::new();
    identity.push_str(if valid { "valid\n" } else { "invalid\n" });
    for layer in layers {
        identity.push_str(layer.tier.as_str());
        identity.push(':');
        identity.push_str(&layer.manifest.hash);
        identity.push('\n');
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
/// Historical receipts ignore additional fields without retaining or reserializing them.
/// This is a persistence reader, never an authority or command input (ADR-0163).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Child receipts whose storage could not be confirmed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub unconfirmed_history_steps: u32,
    /// Closed document coverage facts, excluding human-only embedded host names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<crate::language::coverage::Coverage>,
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
    /// Closed language-authored failure metadata. Legacy arbitrary facts are discarded on read.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::language::audit::read_refusal"
    )]
    pub refusal_facts: Option<AuditRefusal>,
    /// Payload-free counts and recovery cause for a flow or sequence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<crate::language::composition::CompositionProgress>,
    /// Child correlation; absence identifies a direct operation or parent aggregate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<crate::language::history::StepReceipt>,
    /// Canonical child tools, without caller-supplied labels or arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub composition_tools: Vec<String>,
    /// Evidence from actual permission evaluations under this invocation's snapshot.
    #[serde(default)]
    pub permissions: evidence::PermissionTrace,
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
    /// The peer executable's observed file name beside that channel (ADR-0105 stage 2).
    ///
    /// Claimed and observed attribution stay separate fields; the name is the bounded lowercase
    /// file name only, never the path, and it is never an authority input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_image: Option<String>,
    /// Bounded original-connection evidence; claimed labels never enter this durable type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<crate::provenance::Attribution>,
    /// Persistent browser family or product name that performed this work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
}

fn is_zero(value: &u32) -> bool {
    *value == 0
}

impl AuditRecord {
    /// Construct a content-minimized audit record at the current time.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn now(
        invocation: &str,
        workspace: &str,
        tool: &'static str,
        capabilities: impl Into<CapabilitySet>,
        authority: &str,
        decision: Decision,
        status: &str,
        effect: &str,
        language: &AuditProjection,
        duration_ms: u64,
    ) -> Self {
        Self {
            unconfirmed_history_steps: language.unconfirmed_history_steps(),
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
            summary: language.summary().chars().take(500).collect(),
            refusal_facts: language.refusal().cloned(),
            composition: language.composition(),
            coverage: language.coverage().cloned(),
            step: None,
            composition_tools: language.tools().to_vec(),
            permissions: evidence::PermissionTrace::default(),
            duration_ms,
            observed: Observed::default(),
            channel: None,
            peer_image: None,
            provenance: None,
            browser: None,
        }
    }

    /// Attach the browser family or product name that performed the work.
    #[must_use]
    pub fn with_browser(mut self, browser: Option<String>) -> Self {
        self.browser = browser;
        self
    }

    /// Attach the intake the work arrived on.
    #[must_use]
    pub fn from_channel(mut self, channel: Option<IntakeChannel>) -> Self {
        self.channel = channel;
        self
    }

    /// Attach the observed peer image name beside the claimed channel (ADR-0105 stage 2).
    #[must_use]
    pub fn with_peer_image(mut self, peer_image: Option<String>) -> Self {
        self.peer_image = peer_image;
        self
    }

    /// Attach one immutable connection snapshot and align the legacy attribution fields.
    #[must_use]
    pub fn with_provenance(mut self, provenance: Option<&crate::provenance::Attribution>) -> Self {
        self.provenance = provenance.cloned();
        self.channel = provenance.map(|value| value.channel);
        self.peer_image = provenance
            .and_then(|value| value.observed_executable())
            .map(str::to_owned);
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

/// Governance configuration failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GovernanceError {
    /// A configured authority layer is invalid.
    #[error("invalid authority: {0}")]
    InvalidPolicy(String),
}

#[cfg(test)]
mod tests;
