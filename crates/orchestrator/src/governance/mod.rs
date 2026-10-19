// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Authority snapshots, final-boundary admission, runtime controls, and minimized audit intent.

pub mod audit;
pub mod documents;
pub use crate::audit::{AuditSink, JsonlAuditSink};
pub mod effective;
pub mod evidence;
mod file_state;
pub mod inspection;
pub mod managed;
pub mod manifest;
pub mod migrate;
pub mod paths;

use std::env;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::browser::adapter::{RuntimeControlIntent, RuntimeControlState};
use crate::browser::BrowserPort;
use crate::install::record::atomic_replace;
use crate::intake::session::IntakeChannel;
use crate::language::channel_switch::ChannelsOff;
use crate::language::outcome::PolicyAuthor;
use file_state::FileState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::{Host, Url};

const RUNTIME_ACTIVE: u8 = 0;
const RUNTIME_HOLD: u8 = 1;
const RUNTIME_END: u8 = 2;

/// Binary permission for complete screenshots, including visible embedded documents.
pub const SCREENSHOTS_ENABLED_KEY: &str = "browser.screenshots.enabled";

/// The switch for work from MCP agents: `false` refuses every request on the `mcp` channel.
pub const MCP_CHANNEL_ENABLED_KEY: &str = "channels.mcp.enabled";

/// The switch for work from the command line: `false` refuses every request on the `cli` channel.
pub const CLI_CHANNEL_ENABLED_KEY: &str = "channels.cli.enabled";

/// The tier word a record's `policy_tier` gives the organization's layer, in audit and history.
pub const MANAGED_TIER: &str = "managed";

/// The policy key that switches `channel` on or off (ADR-0202 Decision 22a). Unset means on.
#[must_use]
pub const fn channel_enabled_key(channel: IntakeChannel) -> &'static str {
    match channel {
        IntakeChannel::Mcp => MCP_CHANNEL_ENABLED_KEY,
        IntakeChannel::Cli => CLI_CHANNEL_ENABLED_KEY,
    }
}

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
    /// Screenshot capture was disabled by an authority layer.
    ScreenshotDenied,
    /// An authority layer turned off the channel the request arrived on.
    ChannelDenied,
    /// Host was not granted by every authority layer.
    HostDenied,
    /// Host or scheme is independently protected.
    ProtectedHost,
    /// The address is Ghostlight's own workbench origin on this computer.
    OwnWorkbench,
    /// Configured policy could not be validated.
    InvalidAuthority,
    /// Model-facing input did not match the catalog contract.
    InvalidRequest,
    /// Runtime control entered hold. Also reads the code `runtime_attention`; only
    /// `runtime_hold` is written.
    #[serde(alias = "runtime_attention")]
    RuntimeHold,
    /// Runtime control ended the session.
    SessionEnded,
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
            Self::ScreenshotDenied => "screenshot_denied",
            Self::ChannelDenied => "channel_denied",
            Self::HostDenied => "host_denied",
            Self::ProtectedHost => "protected_host",
            Self::OwnWorkbench => "own_workbench",
            Self::InvalidAuthority => "invalid_authority",
            Self::InvalidRequest => "invalid_request",
            Self::RuntimeHold => "runtime_hold",
            Self::SessionEnded => "session_ended",
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
    /// A binary setting prevents screenshots.
    Screenshot,
    /// A channel switch refuses the request's channel.
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
            Self::Screenshot => "screenshot",
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
}

/// A final-boundary permission decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decision {
    /// Whether the boundary is admitted.
    pub allowed: bool,
    /// Stable reason.
    pub reason: ReasonCode,
    attribution: Option<PolicyAttribution>,
}

impl Decision {
    const fn allow() -> Self {
        Self {
            allowed: true,
            reason: ReasonCode::Permitted,
            attribution: None,
        }
    }
    const fn deny(reason: ReasonCode) -> Self {
        Self {
            allowed: false,
            reason,
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

    const fn policy(reason: ReasonCode, attribution: PolicyAttribution) -> Self {
        Self {
            allowed: false,
            reason,
            attribution: Some(attribution),
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
    own_origin: OwnOrigin,
}

/// The loopback port of the workbench's own origin, shared by the facade and every snapshot it
/// builds so that a snapshot published before the port is known sees it as soon as it is set.
#[derive(Clone, Debug, Default)]
struct OwnOrigin(Arc<AtomicU16>);

impl OwnOrigin {
    /// No port is protected until the workbench binds.
    const UNSET: u16 = 0;

    fn set(&self, port: u16) {
        self.0.store(port, Ordering::Release);
    }

    /// True when `url` is an HTTP(S) address on a loopback host at the protected port.
    fn covers(&self, url: &Url) -> bool {
        let port = self.0.load(Ordering::Acquire);
        port != Self::UNSET
            && matches!(url.scheme(), "http" | "https")
            && url.port_or_known_default() == Some(port)
            && url.host().is_some_and(|host| is_loopback_host(&host))
    }
}

/// Whether a parsed host names this machine's loopback interface.
///
/// The URL parser has already normalized decimal, hexadecimal and short IPv4 spellings.
fn is_loopback_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => {
            let name = name.to_ascii_lowercase();
            name == "localhost" || name.ends_with(".localhost")
        }
        Host::Ipv4(address) => is_loopback_v4(*address),
        Host::Ipv6(address) => {
            *address == Ipv6Addr::LOCALHOST || address.to_ipv4_mapped().is_some_and(is_loopback_v4)
        }
    }
}

fn is_loopback_v4(address: Ipv4Addr) -> bool {
    address.is_loopback() || address == Ipv4Addr::UNSPECIFIED
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
            Self::Managed => MANAGED_TIER,
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
}

impl AuthoritySnapshot {
    /// Admit the complete rendered image, without per-document capture filtering.
    #[must_use]
    pub fn authorize_screenshot(&self) -> Decision {
        if !self.valid {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        for (index, layer) in self.layers.iter().enumerate() {
            if layer.manifest.boolean_setting(SCREENSHOTS_ENABLED_KEY) == Some(false) {
                return Decision::policy(
                    ReasonCode::ScreenshotDenied,
                    PolicyAttribution {
                        layer: u16::try_from(index).expect("policy layer count is bounded"),
                        grant: None,
                        rule: PolicyRule::Screenshot,
                        denial: denial_bytes(&layer.manifest.hash, "", PolicyRule::Screenshot),
                    },
                );
            }
        }
        Decision::allow()
    }

    /// The refusal of a request on `channel` when a layer turns that channel's switch off: the
    /// policy decision, and whose layer turned it off (ADR-0202 Decision 22a). `None` admits it.
    ///
    /// The channel is declared by Ghostlight's own peer, and any program running as the same user
    /// could declare another, so this is a guardrail for well-behaved agents, not a barrier
    /// against hostile software on the same account. Validity is not decided here: every
    /// boundary the request goes on to cross refuses invalid authority itself.
    #[must_use]
    pub fn channel_refusal(&self, channel: IntakeChannel) -> Option<(Decision, PolicyAuthor)> {
        let key = channel_enabled_key(channel);
        let (index, layer) = self
            .layers
            .iter()
            .enumerate()
            .find(|(_, layer)| layer.manifest.boolean_setting(key) == Some(false))?;
        let decision = Decision::policy(
            ReasonCode::ChannelDenied,
            PolicyAttribution {
                layer: u16::try_from(index).expect("policy layer count is bounded"),
                grant: None,
                rule: PolicyRule::Channel,
                denial: denial_bytes(&layer.manifest.hash, "", PolicyRule::Channel),
            },
        );
        let author = match layer.tier {
            AuthorityTier::Managed => PolicyAuthor::Organization,
            AuthorityTier::User => PolicyAuthor::User,
        };
        Some((decision, author))
    }

    /// The channel switches this authority turns off, read through [`Self::channel_refusal`], the
    /// check the executor makes per request, so what a person is told and what is enforced are one
    /// answer. The organization's name comes from its layer, for the sentences that name it.
    #[must_use]
    pub fn channels_off(&self) -> ChannelsOff<'_> {
        let author = |channel| self.channel_refusal(channel).map(|(_, author)| author);
        ChannelsOff {
            mcp: author(IntakeChannel::Mcp),
            cli: author(IntakeChannel::Cli),
            organization: self
                .layers
                .iter()
                .find(|layer| layer.tier == AuthorityTier::Managed)
                .and_then(|layer| layer.manifest.organization.as_ref())
                .map(|organization| organization.name.as_str()),
        }
    }

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
    #[cfg(test)]
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
                },
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
        if self.own_origin.covers(&parsed) {
            return (Decision::deny(ReasonCode::OwnWorkbench), vec![]);
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
        outcomes.iter().all(|outcome| outcome.denial.is_none())
    }

    fn resolve_outcomes(&self, outcomes: &[LayerOutcome]) -> Option<Decision> {
        let denial = outcomes
            .iter()
            .enumerate()
            .find_map(|(index, outcome)| outcome.denial.map(|denial| (index, denial)))?;
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
            },
        ))
    }

    /// Monotonic signed managed-policy publish sequence, when managed authority was active.
    #[must_use]
    pub const fn managed_sequence(&self) -> Option<u64> {
        self.managed_sequence
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
    if layer.manifest.grants.iter().any(|grant| {
        requirements.is_subset_of(grant.allowed_set()) && grant_has_possible_host(grant)
    }) {
        return LayerOutcome {
            grants: vec![],
            denial: None,
        };
    }
    LayerOutcome {
        grants: vec![],
        denial: Some(RawDenial {
            reason: ReasonCode::CapabilityDenied,
            rule: PolicyRule::Capability,
            grant: None,
        }),
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
    }
}

fn decide_for_host(layer: &PolicyLayer, requirements: CapabilitySet, host: &str) -> LayerOutcome {
    let mut first_denial = None;
    for (index, grant) in layer.manifest.grants.iter().enumerate() {
        let grant_index = u16::try_from(index).expect("grant count is bounded");
        match evaluate_host(host, &grant.hosts) {
            HostOutcome::Allowed => {
                if requirements.is_subset_of(grant.allowed_set()) {
                    return LayerOutcome {
                        grants: vec![grant_index],
                        denial: None,
                    };
                }
                if first_denial.is_none() {
                    first_denial = Some(RawDenial {
                        reason: ReasonCode::CapabilityDenied,
                        rule: PolicyRule::Capability,
                        grant: Some(grant_index),
                    });
                }
            }
            HostOutcome::Denied if first_denial.is_none() => {
                first_denial = Some(RawDenial {
                    reason: ReasonCode::HostDenied,
                    rule: PolicyRule::DeniedHost,
                    grant: Some(grant_index),
                });
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

/// Receives the runtime control state each time it changes, once per change.
pub type ControlObserver = Arc<dyn Fn(RuntimeControlState) + Send + Sync>;

/// Live controls checked immediately before effects and when browser events arrive.
#[derive(Default)]
pub struct RuntimeControls {
    state: AtomicU8,
    observer: Mutex<Option<ControlObserver>>,
}

impl std::fmt::Debug for RuntimeControls {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeControls")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl RuntimeControls {
    /// Return the current final-boundary decision.
    #[must_use]
    pub fn decision(&self) -> Decision {
        match self.state.load(Ordering::SeqCst) {
            RUNTIME_ACTIVE => Decision::allow(),
            RUNTIME_HOLD => Decision::deny(ReasonCode::RuntimeHold),
            _ => Decision::deny(ReasonCode::SessionEnded),
        }
    }

    /// Return the authoritative content-free runtime state.
    #[must_use]
    pub fn state(&self) -> RuntimeControlState {
        match self.state.load(Ordering::SeqCst) {
            RUNTIME_ACTIVE => RuntimeControlState::Active,
            RUNTIME_HOLD => RuntimeControlState::Held,
            _ => RuntimeControlState::Ended,
        }
    }

    /// Tell `observer` about every later change of state, replacing any earlier observer.
    ///
    /// A change from the tray, the workbench or an extension intent all end here, so each one
    /// reaches the observer exactly once. A request that leaves the state as it was does not.
    pub fn observe(&self, observer: ControlObserver) {
        *self
            .observer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(observer);
    }

    /// Apply one local human control intent and return the resulting state.
    ///
    /// A toggle asks for Hold while active and for Resume otherwise. The act then changes the
    /// state only when [`offers`] names it for the state it meets; any other request leaves the
    /// state as it was. The state is read and changed under the observer lock, so two intents
    /// that race decide one after the other and a Resume never undoes an End session it lost to.
    pub fn apply_intent(&self, intent: RuntimeControlIntent) -> RuntimeControlState {
        let observer = self.lock_observer();
        let state = self.state();
        let act = match intent {
            RuntimeControlIntent::ToggleHold if state == RuntimeControlState::Active => {
                RuntimeControlIntent::Hold
            }
            RuntimeControlIntent::ToggleHold => RuntimeControlIntent::Resume,
            act => act,
        };
        if offers(state).contains(&act) {
            let next = match act {
                RuntimeControlIntent::Hold => RUNTIME_HOLD,
                RuntimeControlIntent::EndSession => RUNTIME_END,
                RuntimeControlIntent::Resume | RuntimeControlIntent::StartSession => RUNTIME_ACTIVE,
                RuntimeControlIntent::ToggleHold => return state,
            };
            self.store(observer.as_ref(), next);
        }
        self.state()
    }

    /// Lock the observer slot.
    ///
    /// The lock is held across a store and its notice, so two changes reach the observer in the
    /// order they were made.
    fn lock_observer(&self) -> std::sync::MutexGuard<'_, Option<ControlObserver>> {
        self.observer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Store `next` and tell `observer` when it differs from the state before. The caller holds
    /// the observer lock.
    fn store(&self, observer: Option<&ControlObserver>, next: u8) {
        let previous = self.state.swap(next, Ordering::SeqCst);
        if previous != next {
            if let Some(observer) = observer {
                observer(self.state());
            }
        }
    }
}

/// The acts a person can take on browser work in `state`, in the order a surface shows them.
///
/// This is the one table of which runtime act each state accepts: [`RuntimeControls::apply_intent`]
/// changes the state only through an act it names. An ended session accepts only a new session,
/// so neither Resume nor a toggle undoes End session. [`RuntimeControlIntent::ToggleHold`] is
/// never offered: it is a request that becomes Hold or Resume by the state it meets.
#[must_use]
pub const fn offers(state: RuntimeControlState) -> &'static [RuntimeControlIntent] {
    match state {
        RuntimeControlState::Active => {
            &[RuntimeControlIntent::Hold, RuntimeControlIntent::EndSession]
        }
        RuntimeControlState::Held => &[
            RuntimeControlIntent::Resume,
            RuntimeControlIntent::EndSession,
        ],
        RuntimeControlState::Ended => &[RuntimeControlIntent::StartSession],
    }
}

/// A small governance service facade used by the application executor.
///
/// Every reader sees the one [`Published`] authority and takes no file or network step to get it.
/// The refresher is the only caller that checks files on a schedule or reaches the organization's
/// endpoint, so the managed fetch never runs on the invocation path (ADR-0197).
#[derive(Clone, Debug)]
pub struct GovernanceFacade {
    /// What every reader sees. Replaced whole; a reader locks it only to clone the `Arc`.
    published: Arc<Mutex<Arc<Published>>>,
    /// The local policy files and the last view of the managed authority. Writers lock it for the
    /// time a file read takes, never across a network call.
    sources: Arc<Mutex<PolicySources>>,
    /// The managed authority itself. Only the refresher locks it, and it holds the lock across a
    /// fetch.
    remote: Arc<Mutex<Option<managed::ManagedAuthority>>>,
    controls: Arc<RuntimeControls>,
    own_origin: OwnOrigin,
}

/// One immutable publication of authority, with every view the workbench reads built from it.
#[derive(Debug)]
struct Published {
    snapshot: Arc<AuthoritySnapshot>,
    diagnostics: GovernanceDiagnostics,
    chip: effective::PolicyChip,
    authority: effective::EffectiveAuthority,
    startup: manifest::BrowserStartup,
    /// The organization layer a candidate user policy is previewed under.
    managed: Option<manifest::Manifest>,
    managed_sequence: Option<u64>,
    managed_valid: bool,
}

impl Published {
    fn build(sources: &PolicySources, own_origin: &OwnOrigin) -> Self {
        let managed = sources.managed_manifest().cloned();
        let managed_sequence = sources.managed_sequence();
        let valid = sources.authority_valid();
        let snapshot = Arc::new(assemble(
            [
                (managed.clone(), AuthorityTier::Managed),
                (sources.user.active.clone(), AuthorityTier::User),
            ],
            managed_sequence,
            valid,
            own_origin,
        ));
        let passport = sources.managed_passport();
        let user = sources.user.active.as_ref();
        let organization = managed.as_ref();

        let situation = if !valid {
            effective::Situation::FailingClosed
        } else {
            match (organization.is_some(), user.is_some()) {
                (true, true) => effective::Situation::Layered,
                (true, false) => effective::Situation::OrganizationOnly,
                (false, true) => effective::Situation::UserOnly,
                (false, false) => effective::Situation::AllOpen,
            }
        };
        let name = organization
            .and_then(|policy| policy.organization.as_ref())
            .map(|organization| organization.name.clone());
        let channels_off = snapshot.channels_off();
        let chip = effective::chip(
            situation,
            name.as_deref(),
            matches!(passport.freshness, ManagedPolicyFreshness::LastKnownGood),
            channels_off,
        );

        let inputs = effective::Inputs {
            organization,
            user,
            valid,
            channels_off,
            sacred_hosts: snapshot.sacred_hosts().to_vec(),
            organization_source: sources.managed.path.as_ref().map_or_else(
                || organization.map(|_| passport_source(&passport)),
                |path| Some(path.display().to_string()),
            ),
            organization_document: organization.map(document),
            user_source: sources
                .user
                .path
                .as_ref()
                .map(|path| path.display().to_string()),
            user_document: user.map(document),
            user_layer_source: if user.is_some() {
                sources.user_origin
            } else {
                effective::UserLayerSource::None
            },
            owned_user_path: paths::user_policy_path().map(|path| path.display().to_string()),
            windows: cfg!(windows),
            passport: passport.clone(),
        };
        let authority = effective::compile(&inputs);
        let startup = authority.browser_startup.value;

        Self {
            snapshot,
            diagnostics: GovernanceDiagnostics {
                local_policy_configured: sources.user.configured(),
                local_policy_active: sources.user.active.is_some(),
                local_policy_valid: sources.user.last_load_valid,
                managed_authority_configured: sources.managed_configured(),
                managed_authority_active: organization.is_some(),
                managed_authority_valid: sources.managed_valid(),
                authority_valid: valid,
                managed_policy: passport,
            },
            chip,
            authority,
            startup,
            managed,
            managed_sequence,
            managed_valid: sources.managed_valid(),
        }
    }
}

/// What the facade last saw of the managed authority, kept beside the local files so a
/// publication never waits for a fetch.
#[derive(Clone, Debug)]
struct RemoteView {
    configured: bool,
    valid: bool,
    manifest: Option<manifest::Manifest>,
    sequence: Option<u64>,
    passport: ManagedPolicyPassport,
}

impl RemoteView {
    fn of(remote: &managed::ManagedAuthority) -> Self {
        Self {
            configured: remote.configured(),
            valid: remote.valid(),
            manifest: remote.manifest().cloned(),
            sequence: remote.sequence(),
            passport: remote.passport(),
        }
    }
}

#[derive(Debug)]
struct PolicySources {
    managed: PolicySource,
    remote: Option<RemoteView>,
    user: PolicySource,
    user_origin: effective::UserLayerSource,
    /// The one path this window may write. Absent when Ghostlight does not own the user layer.
    owned_user_path: Option<PathBuf>,
}

impl PolicySources {
    fn new(local: Option<PathBuf>, managed: Option<PathBuf>) -> Self {
        Self {
            managed: PolicySource::new(managed, "managed"),
            remote: None,
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
            remote: None,
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
            remote: None,
            user,
            user_origin,
            owned_user_path: match user_origin {
                effective::UserLayerSource::Workbench => paths::user_policy_path(),
                _ => None,
            },
        }
    }

    #[cfg(test)]
    fn remote_only() -> Self {
        Self {
            managed: PolicySource::new(None, "managed"),
            remote: None,
            user: PolicySource::new(None, "user"),
            user_origin: effective::UserLayerSource::Workbench,
            owned_user_path: None,
        }
    }

    /// Read both local files again, whatever their state.
    fn reload_local(&mut self) {
        self.managed.refresh("managed");
        self.user.refresh("user");
    }

    /// Read each local file that changed since it was last read. True when any did.
    fn reload_changed(&mut self) -> bool {
        let managed = self.managed.refresh_if_changed("managed");
        let user = self.user.refresh_if_changed("user");
        managed || user
    }

    fn managed_configured(&self) -> bool {
        self.remote.as_ref().is_some_and(|remote| remote.configured) || self.managed.configured()
    }

    fn managed_valid(&self) -> bool {
        self.remote
            .as_ref()
            .map_or(self.managed.last_load_valid, |remote| remote.valid)
    }

    /// Whether configured authority can be enforced, which is the one validity every surface uses.
    ///
    /// False means every governed call fails closed. A user policy whose reload failed while its
    /// last valid version is still applied stays valid here, because that version is enforced.
    fn authority_valid(&self) -> bool {
        self.managed_valid() && self.user.has_authority()
    }

    fn managed_manifest(&self) -> Option<&manifest::Manifest> {
        self.remote
            .as_ref()
            .and_then(|remote| remote.manifest.as_ref())
            .or(self.managed.active.as_ref())
    }

    fn managed_sequence(&self) -> Option<u64> {
        self.remote.as_ref().and_then(|remote| remote.sequence)
    }

    fn managed_passport(&self) -> ManagedPolicyPassport {
        if let Some(remote) = &self.remote {
            return remote.passport.clone();
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
    /// Whether Ghostlight owns this file.
    ///
    /// The user policy Ghostlight owns is optional: a machine with no such file is all-open, not
    /// failing closed. A path someone else named is not optional, because naming a file that is not
    /// there is a mistake worth refusing over. Ghostlight also moves a file it owns from the
    /// retired schema 3 to schema 4 when the move keeps its meaning; it never rewrites a file
    /// someone else named, or the organization's.
    owned: bool,
    present: bool,
    active: Option<manifest::Manifest>,
    last_load_valid: bool,
    last_error: Option<String>,
    /// The file as it was when last read, so an unchanged file is not read again.
    seen: Option<FileState>,
}

impl PolicySource {
    fn new(path: Option<PathBuf>, tier: &str) -> Self {
        Self::with_options(path, tier, false)
    }

    fn with_options(path: Option<PathBuf>, tier: &str, owned: bool) -> Self {
        let mut source = Self {
            last_load_valid: path.is_none(),
            owned,
            present: path.is_some(),
            path,
            active: None,
            last_error: None,
            seen: None,
        };
        source.refresh(tier);
        source
    }

    /// Read the file again only when its state differs from the last read. True when it did.
    fn refresh_if_changed(&mut self, tier: &str) -> bool {
        let Some(path) = &self.path else {
            return false;
        };
        if self.seen == Some(FileState::of(path)) {
            return false;
        }
        self.refresh(tier);
        true
    }

    /// Read the file now. The state is taken before the read, so a write that lands during it is
    /// seen on the next check.
    fn refresh(&mut self, tier: &str) {
        let Some(path) = &self.path else { return };
        self.seen = Some(FileState::of(path));
        if self.owned && !path.exists() {
            self.present = false;
            self.active = None;
            self.last_load_valid = true;
            self.last_error = None;
            return;
        }
        self.present = true;
        match read_policy(path, self.owned) {
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
    /// Whether the configured local policy's latest read validated.
    ///
    /// False after a failed reload even while the last valid version stays applied. Whether work
    /// is refused is `authority_valid`, not this.
    pub local_policy_valid: bool,
    /// Whether a managed authority source is configured.
    pub managed_authority_configured: bool,
    /// Whether a verified managed policy is currently applied.
    pub managed_authority_active: bool,
    /// Whether the configured managed authority can be read and validated.
    pub managed_authority_valid: bool,
    /// Whether configured authority is enforceable now, exactly as governance enforces it.
    ///
    /// False means every governed call fails closed. The workbench header, its Status card, and
    /// the Policy destination all read this one answer.
    pub authority_valid: bool,
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
        self.published().diagnostics.clone()
    }

    /// Construct the facade from explicit policy paths.
    #[must_use]
    pub fn new(local_policy: Option<PathBuf>, managed_policy: Option<PathBuf>) -> Self {
        Self::publishing(PolicySources::new(local_policy, managed_policy), None)
    }

    /// A facade whose user layer is one Ghostlight owns at an explicit path.
    #[cfg(test)]
    pub(crate) fn owning_user_policy(path: PathBuf, managed: Option<PathBuf>) -> Self {
        Self::publishing(PolicySources::owning(path, managed), None)
    }

    #[cfg(test)]
    fn with_managed_paths(paths: managed::ManagedPaths) -> Self {
        Self::publishing(
            PolicySources::remote_only(),
            Some(managed::ManagedAuthority::from_paths(paths)),
        )
    }

    /// Construct the facade from Ghostlight-specific environment variables.
    ///
    /// The first publication uses only what is on this machine: the policy files and the
    /// organization's last verified policy. The refresher fetches from the organization's
    /// endpoint afterwards, so a slow endpoint never delays a start.
    #[must_use]
    pub fn from_environment() -> Self {
        Self::publishing(
            PolicySources::production(env::var_os("GHOSTLIGHT_POLICY_FILE").map(PathBuf::from)),
            Some(managed::ManagedAuthority::production()),
        )
    }

    /// Load every local source once and publish the authority they give.
    fn publishing(
        mut sources: PolicySources,
        mut remote: Option<managed::ManagedAuthority>,
    ) -> Self {
        if let Some(remote) = remote.as_mut() {
            remote.load_offline();
            sources.remote = Some(RemoteView::of(remote));
        }
        let own_origin = OwnOrigin::default();
        let published = Arc::new(Published::build(&sources, &own_origin));
        Self {
            published: Arc::new(Mutex::new(published)),
            sources: Arc::new(Mutex::new(sources)),
            remote: Arc::new(Mutex::new(remote)),
            controls: Arc::new(RuntimeControls::default()),
            own_origin,
        }
    }

    fn published(&self) -> Arc<Published> {
        Arc::clone(&lock(&self.published))
    }

    /// Check the watched files and the managed authority, and publish what changed.
    ///
    /// The refresher calls this once a second. A policy file or the managed bootstrap is read
    /// again only when its existence, modified time or size differs from the last read, and the
    /// managed authority fetches from its endpoint on its own schedule. This is the only call
    /// that waits on the network, and no invocation makes it.
    pub fn refresh_tick(&self) {
        let remote = {
            let mut guard = lock(&self.remote);
            guard
                .as_mut()
                .filter(|remote| remote.needs_refresh())
                .map(|remote| {
                    remote.refresh();
                    RemoteView::of(remote)
                })
        };
        let mut sources = lock(&self.sources);
        let mut changed = remote.is_some();
        if remote.is_some() {
            sources.remote = remote;
        }
        changed |= sources.reload_changed();
        if changed {
            self.publish(&sources);
        }
    }

    /// Read the local policy files again now and publish, with no network work.
    ///
    /// A person who edits their policy in the workbench sees it take effect at once, not on the
    /// refresher's next check.
    fn refresh_local_now(&self) {
        let mut sources = lock(&self.sources);
        sources.reload_local();
        self.publish(&sources);
    }

    fn publish(&self, sources: &PolicySources) {
        *lock(&self.published) = Arc::new(Published::build(sources, &self.own_origin));
    }

    /// Hold the sources lock, as a refresher mid-read does.
    #[cfg(test)]
    fn hold_sources(&self) -> std::sync::MutexGuard<'_, PolicySources> {
        lock(&self.sources)
    }

    /// Access live runtime controls.
    #[must_use]
    pub fn controls(&self) -> Arc<RuntimeControls> {
        Arc::clone(&self.controls)
    }

    /// Send every change of the runtime control state to every browser `port` reaches.
    ///
    /// The notice is best effort and has no receipt: the state itself is what governance checks
    /// before each effect, and the port replays it to a browser that connects later.
    pub fn relay_runtime_state(&self, port: Arc<dyn BrowserPort>) {
        self.controls.observe(Arc::new(move |state| {
            let _ = port.publish_control_state(state);
        }));
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

    /// Author the band chip from the published authority.
    #[must_use]
    pub fn policy_chip(&self) -> effective::PolicyChip {
        self.published().chip.clone()
    }

    /// The compiled policy: the one answer a person arrives with.
    ///
    /// Every part is built from the same publication, so the sentence, the capability lines, and
    /// the rules behind them all describe the same instant. The words are authored in the
    /// orchestrator; the window renders them and computes nothing (ADR-0122 Decision 2).
    #[must_use]
    pub fn effective_authority(&self) -> effective::EffectiveAuthority {
        self.published().authority.clone()
    }

    /// Resolve the effective missing-browser startup preference.
    #[must_use]
    pub fn browser_startup(&self) -> manifest::BrowserStartup {
        self.published().startup
    }

    /// Protect the workbench's own loopback port: no agent can open or read an address on it.
    ///
    /// This is the one product-owned ceiling of ADR-0199's amendment. It applies to every
    /// snapshot, including those published earlier, and does not change the authority id.
    /// The workbench calls it once, when it binds.
    pub fn protect_workbench_port(&self, port: u16) {
        self.own_origin.set(port);
    }

    /// The published authority: an immutable snapshot that no call can change.
    ///
    /// This clones an `Arc` and reads no file and no network. A hand edit to a policy file shows
    /// here when the refresher next publishes it.
    #[must_use]
    pub fn snapshot(&self) -> Arc<AuthoritySnapshot> {
        Arc::clone(&self.published().snapshot)
    }

    /// Build the snapshot a candidate user policy would produce, without applying it.
    ///
    /// The organization layer stays exactly as it is, because a preview that ignored the ceiling
    /// would answer a question nobody asked. Nothing here writes a file, changes authority, or
    /// records audit (ADR-0122 Decision 7).
    pub fn candidate_snapshot(&self, document: &str) -> Result<Candidate, manifest::ManifestError> {
        let candidate = manifest::parse(document, "this policy")?;
        let rules = candidate.grants.len();
        let published = self.published();
        Ok(Candidate {
            snapshot: assemble(
                [
                    (published.managed.clone(), AuthorityTier::Managed),
                    (Some(candidate), AuthorityTier::User),
                ],
                published.managed_sequence,
                published.managed_valid,
                &self.own_origin,
            ),
            rules,
        })
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
    /// Ghostlight reads this policy but does not own its file.
    #[error("An environment variable points Ghostlight at a policy file it does not own, so this window cannot change it.")]
    NotOwned,
    /// This environment names no per-user state directory.
    #[error("This machine names no per-user state directory to keep a policy in.")]
    NoHome,
    /// The document is not a valid policy.
    #[error("{0}")]
    Invalid(String),
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
        let path = self.writable_user_policy_path()?;
        atomic_replace(&path, document.as_bytes())
            .map_err(|error| AuthoringError::Unwritable(error.to_string()))?;
        self.refresh_local_now();
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
        self.refresh_local_now();
        Ok(())
    }

    /// The user policy path this window may write, or why it may not.
    fn writable_user_policy_path(&self) -> Result<PathBuf, AuthoringError> {
        let sources = lock(&self.sources);
        if sources.user_origin == effective::UserLayerSource::Environment {
            return Err(AuthoringError::NotOwned);
        }
        sources
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
    own_origin: &OwnOrigin,
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
        own_origin: own_origin.clone(),
    }
}

impl GovernanceFacade {
    /// Check live runtime control at an effect boundary.
    #[must_use]
    pub fn runtime_decision(&self) -> Decision {
        self.controls.decision()
    }
}

/// Read and validate one policy file, moving it to the current schema first when Ghostlight owns
/// it and the move keeps its meaning. A file that cannot move keeps the error that names the
/// migrate command, so the layer stays invalid and fails closed.
fn read_policy(path: &Path, owned: bool) -> Result<manifest::Manifest, GovernanceError> {
    let text = fs::read_to_string(path)
        .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))?;
    let origin = path.display().to_string();
    match manifest::parse(&text, &origin) {
        Err(manifest::ManifestError::RetiredSchema { .. }) if owned => {
            if let Ok(Some((backup, moved))) = migrate::migrate_file(path) {
                eprintln!(
                    "{}",
                    crate::language::migration::upgraded_user_policy(path, &backup)
                );
                return manifest::parse(&moved.document, &origin)
                    .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()));
            }
        }
        _ => {}
    }
    manifest::parse(&text, &origin)
        .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

/// Governance configuration failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GovernanceError {
    /// A configured authority layer is invalid.
    #[error("invalid authority: {0}")]
    InvalidPolicy(String),
}

#[cfg(test)]
mod tests;
