// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
// See docs/licenses/LicenseRef-Ghostlight-Commercial.txt.

//! Authority snapshots, final-boundary admission, runtime controls, and minimized audit intent.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ghostlight_bridge::browser::{RuntimeControlIntent, RuntimeControlState};
use ghostlight_bridge::service::IntakeChannel;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::language::{outcome::Observed, RequestRestrictions};

const RUNTIME_ACTIVE: u8 = 0;
const RUNTIME_HOLD: u8 = 1;
const RUNTIME_ATTENTION: u8 = 2;
const RUNTIME_END: u8 = 3;

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

/// A final-boundary permission decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decision {
    /// Whether the boundary is admitted.
    pub allowed: bool,
    /// Stable reason.
    pub reason: ReasonCode,
}

impl Decision {
    const fn allow() -> Self {
        Self {
            allowed: true,
            reason: ReasonCode::Permitted,
        }
    }
    const fn deny(reason: ReasonCode) -> Self {
        Self {
            allowed: false,
            reason,
        }
    }
}

/// One immutable effective authority for started work.
#[derive(Clone, Debug)]
pub struct AuthoritySnapshot {
    id: String,
    capabilities: HashSet<Capability>,
    tab_close_allowed: bool,
    preserve_target_names: bool,
    allow_host_layers: Vec<Vec<String>>,
    deny_hosts: Vec<String>,
    valid: bool,
}

impl AuthoritySnapshot {
    /// Opaque version recorded in audit.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
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
        if requirements
            .iter()
            .all(|capability| self.capabilities.contains(&capability))
        {
            Decision::allow()
        } else {
            Decision::deny(ReasonCode::CapabilityDenied)
        }
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
            Decision::deny(ReasonCode::TabCloseDenied)
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
        let capability_decision = self.authorize_requirements(requirements.into());
        if !capability_decision.allowed {
            return capability_decision;
        }
        let Ok(parsed) = Url::parse(url) else {
            return Decision::deny(ReasonCode::HostDenied);
        };
        if protected_url(&parsed) {
            return Decision::deny(ReasonCode::ProtectedHost);
        }
        let Some(host) = parsed.host_str().map(str::to_ascii_lowercase) else {
            return Decision::deny(ReasonCode::HostDenied);
        };
        if self
            .deny_hosts
            .iter()
            .any(|pattern| host_matches(&host, pattern))
        {
            return Decision::deny(ReasonCode::HostDenied);
        }
        if self
            .allow_host_layers
            .iter()
            .all(|patterns| patterns.iter().any(|pattern| host_matches(&host, pattern)))
        {
            Decision::allow()
        } else {
            Decision::deny(ReasonCode::HostDenied)
        }
    }
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
    local_policy: Option<PathBuf>,
    managed_policy: Option<PathBuf>,
    runtime_control: Option<PathBuf>,
    controls: Arc<RuntimeControls>,
}

/// Content-free configuration facts for the local workbench.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GovernanceDiagnostics {
    /// Whether a local policy source is configured.
    pub local_policy_configured: bool,
    /// Whether the configured local policy can be read and validated.
    pub local_policy_valid: bool,
    /// Whether a managed authority source is configured.
    pub managed_authority_configured: bool,
    /// Whether the configured managed authority can be read and validated.
    pub managed_authority_valid: bool,
    /// Whether a runtime-control file is configured.
    pub runtime_control_file_configured: bool,
}

impl GovernanceFacade {
    /// Return content-free configuration health without exposing authority paths or rules.
    #[must_use]
    pub fn diagnostics(&self) -> GovernanceDiagnostics {
        GovernanceDiagnostics {
            local_policy_configured: self.local_policy.is_some(),
            local_policy_valid: self
                .local_policy
                .as_deref()
                .is_none_or(|path| read_policy(path, false).is_ok()),
            managed_authority_configured: self.managed_policy.is_some(),
            managed_authority_valid: self
                .managed_policy
                .as_deref()
                .is_none_or(|path| read_policy(path, true).is_ok()),
            runtime_control_file_configured: self.runtime_control.is_some(),
        }
    }

    /// Construct the facade from explicit policy paths.
    #[must_use]
    pub fn new(local_policy: Option<PathBuf>, managed_policy: Option<PathBuf>) -> Self {
        Self {
            local_policy,
            managed_policy,
            runtime_control: None,
            controls: Arc::new(RuntimeControls::default()),
        }
    }

    /// Construct the facade from Ghostlight-specific environment variables.
    #[must_use]
    pub fn from_environment() -> Self {
        Self::new(
            env::var_os("GHOSTLIGHT_POLICY_FILE").map(PathBuf::from),
            env::var_os("GHOSTLIGHT_MANAGED_AUTHORITY_FILE").map(PathBuf::from),
        )
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

    /// Build one immutable snapshot and apply caller restrictions by intersection.
    /// Decide whether an intake channel may open a session at all.
    ///
    /// This is admission, not capability: an admitted channel is still bound by every ceiling the
    /// same layers impose, and no layer can raise one channel above another. Layers compose by
    /// intersection, so a managed refusal cannot be undone locally, and an invalid layer denies.
    #[must_use]
    pub fn admits_channel(&self, channel: IntakeChannel) -> Decision {
        for (path, managed) in [(&self.local_policy, false), (&self.managed_policy, true)] {
            let Some(path) = path else { continue };
            match read_policy(path, managed) {
                Ok(policy) => {
                    if let Some(channels) = &policy.channels {
                        if let Some(rule) = channels.get(&channel) {
                            if !rule.enabled {
                                return Decision::deny(ReasonCode::ChannelDenied);
                            }
                        }
                    }
                }
                Err(_) => return Decision::deny(ReasonCode::InvalidAuthority),
            }
        }
        Decision::allow()
    }

    pub fn snapshot(&self, restrictions: &RequestRestrictions) -> AuthoritySnapshot {
        let mut capabilities = all_capabilities();
        let mut tab_close_allowed = true;
        let mut preserve_target_names = true;
        let mut allow_host_layers = Vec::new();
        let mut deny_hosts = Vec::new();
        let mut valid = true;

        if let Some(path) = &self.local_policy {
            match read_policy(path, false) {
                Ok(policy) => apply_policy(
                    &policy,
                    &mut capabilities,
                    &mut tab_close_allowed,
                    &mut preserve_target_names,
                    &mut allow_host_layers,
                    &mut deny_hosts,
                ),
                Err(_) => valid = false,
            }
        }
        if let Some(path) = &self.managed_policy {
            match read_policy(path, true) {
                Ok(policy) => apply_policy(
                    &policy,
                    &mut capabilities,
                    &mut tab_close_allowed,
                    &mut preserve_target_names,
                    &mut allow_host_layers,
                    &mut deny_hosts,
                ),
                Err(_) => valid = false,
            }
        }
        if let Some(restricted) = &restrictions.restrict_capabilities {
            let requested: HashSet<_> = restricted
                .iter()
                .filter_map(|value| Capability::from_str(value).ok())
                .collect();
            capabilities = capabilities.intersection(&requested).copied().collect();
        }
        if let Some(hosts) = &restrictions.restrict_hosts {
            allow_host_layers.push(hosts.clone());
        }

        AuthoritySnapshot {
            id: format!("authority_{}", Uuid::new_v4().simple()),
            capabilities,
            tab_close_allowed,
            preserve_target_names,
            allow_host_layers,
            deny_hosts,
            valid,
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDocument {
    version: u32,
    #[serde(default)]
    managed: bool,
    #[serde(default)]
    expires_unix_ms: Option<u64>,
    #[serde(default)]
    allow_capabilities: Option<Vec<Capability>>,
    #[serde(default)]
    deny_capabilities: Vec<Capability>,
    #[serde(default)]
    allow_tab_close: Option<bool>,
    /// Whether action outcomes and audit retain bounded browser-observed target names.
    #[serde(default)]
    preserve_target_names: Option<bool>,
    #[serde(default)]
    allow_hosts: Option<Vec<String>>,
    #[serde(default)]
    deny_hosts: Vec<String>,
    /// Intake channels this layer takes control of.
    ///
    /// Absent means the layer restricts no channel, so an unconfigured Ghostlight admits every
    /// intake and the ungoverned path stays first-class (ADR-0013). Naming a channel is how a
    /// layer takes control of it, and taking control means saying yes explicitly.
    #[serde(default)]
    channels: Option<BTreeMap<IntakeChannel, ChannelRule>>,
}

/// What one authority layer says about one intake channel.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelRule {
    /// Whether this layer admits the channel. Absent means no, so `{}` fully disables it.
    #[serde(default)]
    enabled: bool,
}

fn read_policy(path: &Path, must_be_managed: bool) -> Result<PolicyDocument, GovernanceError> {
    let bytes =
        fs::read(path).map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))?;
    let policy: PolicyDocument = serde_json::from_slice(&bytes)
        .map_err(|error| GovernanceError::InvalidPolicy(error.to_string()))?;
    if policy.version != 1 {
        return Err(GovernanceError::InvalidPolicy(
            "unsupported policy version".into(),
        ));
    }
    if must_be_managed && !policy.managed {
        return Err(GovernanceError::InvalidPolicy(
            "managed marker missing".into(),
        ));
    }
    if must_be_managed {
        let expiry = policy
            .expires_unix_ms
            .ok_or_else(|| GovernanceError::InvalidPolicy("managed expiry missing".into()))?;
        if expiry <= unix_ms() {
            return Err(GovernanceError::InvalidPolicy(
                "managed authority expired".into(),
            ));
        }
    }
    validate_patterns(policy.allow_hosts.as_deref().unwrap_or_default())?;
    validate_patterns(&policy.deny_hosts)?;
    Ok(policy)
}

fn validate_patterns(patterns: &[String]) -> Result<(), GovernanceError> {
    if patterns.iter().any(|pattern| {
        pattern.trim().is_empty()
            || pattern.len() > 253
            || pattern.contains('/')
            || pattern.contains(':')
    }) {
        Err(GovernanceError::InvalidPolicy(
            "host patterns must be bounded hostnames".into(),
        ))
    } else {
        Ok(())
    }
}

fn apply_policy(
    policy: &PolicyDocument,
    capabilities: &mut HashSet<Capability>,
    tab_close_allowed: &mut bool,
    preserve_target_names: &mut bool,
    allow_host_layers: &mut Vec<Vec<String>>,
    deny_hosts: &mut Vec<String>,
) {
    if let Some(allowed) = &policy.allow_capabilities {
        let allowed: HashSet<_> = allowed.iter().copied().collect();
        *capabilities = capabilities.intersection(&allowed).copied().collect();
    }
    for denied in &policy.deny_capabilities {
        capabilities.remove(denied);
    }
    if policy.allow_tab_close == Some(false) {
        *tab_close_allowed = false;
    }
    if policy.preserve_target_names == Some(false) {
        *preserve_target_names = false;
    }
    if let Some(allowed) = &policy.allow_hosts {
        allow_host_layers.push(allowed.clone());
    }
    deny_hosts.extend(policy.deny_hosts.iter().cloned());
}

fn all_capabilities() -> HashSet<Capability> {
    [
        Capability::Read,
        Capability::Action,
        Capability::Write,
        Capability::Execute,
    ]
    .into_iter()
    .collect()
}

fn protected_url(url: &Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return true;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return true;
    };
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Ok(ip) = IpAddr::from_str(&host) {
        return match ip {
            IpAddr::V4(value) => value.is_loopback() || value.is_link_local(),
            IpAddr::V6(value) => value.is_loopback() || (value.segments()[0] & 0xffc0) == 0xfe80,
        };
    }
    false
}

fn host_matches(host: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().to_ascii_lowercase();
    if let Some(suffix) = pattern.strip_prefix("*.") {
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
    /// Whether final-boundary authority admitted the work.
    pub allowed: bool,
    /// Stable reason code.
    pub reason: ReasonCode,
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
            allowed: decision.allowed,
            reason: decision.reason,
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

    use super::{AuditRecord, Capability, CapabilitySet, Decision, GovernanceFacade, ReasonCode};
    use crate::language::outcome::Observed;

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-1.0-{name}-{}.json",
            uuid::Uuid::new_v4()
        ))
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
    fn tab_close_policy_is_monotonic_across_authority_layers() {
        let local = temporary("local-tab-close");
        let managed = temporary("managed-tab-close");
        fs::write(&local, br#"{"version":1,"allow_tab_close":false}"#).unwrap();
        fs::write(
            &managed,
            br#"{"version":1,"managed":true,"expires_unix_ms":18446744073709551615,"allow_tab_close":true}"#,
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
        fs::write(&local, br#"{"version":1,"preserve_target_names":false}"#).unwrap();
        fs::write(
            &managed,
            br#"{"version":1,"managed":true,"expires_unix_ms":18446744073709551615,"preserve_target_names":true}"#,
        )
        .unwrap();

        let snapshot = GovernanceFacade::new(Some(local.clone()), Some(managed.clone()))
            .snapshot(&RequestRestrictions::default());
        assert!(!snapshot.preserves_target_names());

        fs::write(&local, br#"{"version":1,"preserve_target_names":true}"#).unwrap();
        fs::write(
            &managed,
            br#"{"version":1,"managed":true,"expires_unix_ms":18446744073709551615,"preserve_target_names":false}"#,
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
            br#"{"version":1,"allow_capabilities":["read","write"]}"#,
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
    fn maintained_policy_examples_match_the_version_one_decoder() {
        for (name, source, managed) in [
            (
                "research-read-only",
                include_str!("../../../../examples/research-read-only.json"),
                false,
            ),
            (
                "qa-staging",
                include_str!("../../../../examples/qa-staging.json"),
                false,
            ),
            (
                "enterprise-healthcare",
                include_str!("../../../../examples/enterprise-healthcare.json"),
                true,
            ),
            (
                "developer-unrestricted",
                include_str!("../../../../examples/developer-unrestricted.json"),
                false,
            ),
            (
                "developer-observe",
                include_str!("../../../../examples/developer-observe.json"),
                false,
            ),
            (
                "dev-live-test",
                include_str!("../../../../examples/dev-live-test.json"),
                false,
            ),
            (
                "demo-policy",
                include_str!("../../../../examples/demo-policy.json"),
                false,
            ),
            (
                "scripting-disabled",
                include_str!("../../../../examples/scripting-disabled.json"),
                false,
            ),
        ] {
            let path = temporary(name);
            fs::write(&path, source).unwrap();
            assert!(
                super::read_policy(&path, managed).is_ok(),
                "{name} must remain a valid 1.0 policy"
            );
            let _ = fs::remove_file(path);
        }
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
        fs::write(&path, br#"{"version":1,"allow_capabilities":["read"]}"#).unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        let first = facade.snapshot(&RequestRestrictions::default());
        fs::write(&path, br#"{"version":1,"allow_capabilities":["action"]}"#).unwrap();
        assert!(first.authorize_capability(Capability::Read).allowed);
        assert!(!first.authorize_capability(Capability::Action).allowed);
        let second = facade.snapshot(&RequestRestrictions::default());
        assert!(!second.authorize_capability(Capability::Read).allowed);
        assert!(second.authorize_capability(Capability::Action).allowed);
        let _ = fs::remove_file(path);
    }

    fn sample_record() -> AuditRecord {
        AuditRecord::now(
            "invocation_x",
            "workspace_x",
            "browser_fill_form",
            CapabilitySet::READ.union(CapabilitySet::WRITE),
            "authority_x",
            Decision {
                allowed: true,
                reason: ReasonCode::Permitted,
            },
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

        // An empty rule is a refusal: taking control of a channel means saying yes explicitly.
        let path = temporary("channel-empty");
        fs::write(&path, br#"{"version":1,"channels":{"cli":{}}}"#).unwrap();
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
            br#"{"version":1,"channels":{"cli":{"enabled":false},"mcp":{"enabled":true}}}"#,
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
            br#"{"version":1,"managed":true,"expires_unix_ms":99999999999999,"channels":{"cli":{}}}"#,
        )
        .unwrap();
        let local = temporary("channel-local");
        fs::write(
            &local,
            br#"{"version":1,"channels":{"cli":{"enabled":true}}}"#,
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
        fs::write(&path, br#"{"version":1,"channels":{"cli-tool":{}}}"#).unwrap();
        let facade = GovernanceFacade::new(Some(path.clone()), None);
        assert_eq!(
            facade.admits_channel(IntakeChannel::Cli).reason,
            ReasonCode::InvalidAuthority,
            "a misspelled channel must fail closed rather than restrict nothing"
        );
        let _ = fs::remove_file(path);
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
}
