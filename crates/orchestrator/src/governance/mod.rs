// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
// See docs/licenses/LicenseRef-Ghostlight-Commercial.txt.

//! Authority snapshots, final-boundary admission, runtime controls, and payload-free audit intent.

use std::collections::BTreeSet;
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
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

use crate::language::RequestRestrictions;

const RUNTIME_ACTIVE: u8 = 0;
const RUNTIME_HOLD: u8 = 1;
const RUNTIME_ATTENTION: u8 = 2;
const RUNTIME_END: u8 = 3;

/// Governed browser capability classes in increasing authority order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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
    capabilities: BTreeSet<Capability>,
    tab_close_allowed: bool,
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
        if !self.valid {
            return Decision::deny(ReasonCode::InvalidAuthority);
        }
        if self.capabilities.contains(&capability) {
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

    /// Decide an observed or requested landing at its final boundary.
    #[must_use]
    pub fn authorize_landing(&self, capability: Capability, url: &str) -> Decision {
        let capability_decision = self.authorize_capability(capability);
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
    pub fn snapshot(&self, restrictions: &RequestRestrictions) -> AuthoritySnapshot {
        let mut capabilities = all_capabilities();
        let mut tab_close_allowed = true;
        let mut allow_host_layers = Vec::new();
        let mut deny_hosts = Vec::new();
        let mut valid = true;

        if let Some(path) = &self.local_policy {
            match read_policy(path, false) {
                Ok(policy) => apply_policy(
                    &policy,
                    &mut capabilities,
                    &mut tab_close_allowed,
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
                    &mut allow_host_layers,
                    &mut deny_hosts,
                ),
                Err(_) => valid = false,
            }
        }
        if let Some(restricted) = &restrictions.restrict_capabilities {
            let requested: BTreeSet<_> = restricted
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
    #[serde(default)]
    allow_hosts: Option<Vec<String>>,
    #[serde(default)]
    deny_hosts: Vec<String>,
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
    capabilities: &mut BTreeSet<Capability>,
    tab_close_allowed: &mut bool,
    allow_host_layers: &mut Vec<Vec<String>>,
    deny_hosts: &mut Vec<String>,
) {
    if let Some(allowed) = &policy.allow_capabilities {
        let allowed: BTreeSet<_> = allowed.iter().copied().collect();
        *capabilities = capabilities.intersection(&allowed).copied().collect();
    }
    for denied in &policy.deny_capabilities {
        capabilities.remove(denied);
    }
    if policy.allow_tab_close == Some(false) {
        *tab_close_allowed = false;
    }
    if let Some(allowed) = &policy.allow_hosts {
        allow_host_layers.push(allowed.clone());
    }
    deny_hosts.extend(policy.deny_hosts.iter().cloned());
}

fn all_capabilities() -> BTreeSet<Capability> {
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

/// A payload-free audit record produced after a terminal outcome.
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
    /// Highest capability requested.
    pub capability: Capability,
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
}

impl AuditRecord {
    /// Construct a payload-free audit record at the current time.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn now(
        invocation: &str,
        workspace: &str,
        tool: &str,
        capability: Capability,
        authority: &str,
        decision: Decision,
        status: &str,
        effect: &str,
    ) -> Self {
        Self {
            timestamp_ms: unix_ms(),
            invocation: invocation.into(),
            workspace: workspace.into(),
            tool: tool.into(),
            capability,
            authority: authority.into(),
            allowed: decision.allowed,
            reason: decision.reason,
            status: status.into(),
            effect: effect.into(),
        }
    }
}

/// Separate payload-free audit output port.
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

    use super::{AuditRecord, Capability, Decision, GovernanceFacade, ReasonCode};

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

    #[test]
    fn audit_record_has_no_payload_fields() {
        let record = AuditRecord::now(
            "invocation_x",
            "workspace_x",
            "browser_fill_form",
            Capability::Write,
            "authority_x",
            Decision {
                allowed: true,
                reason: ReasonCode::Permitted,
            },
            "succeeded",
            "applied",
        );
        let value = serde_json::to_value(record).unwrap();
        let object = value.as_object().unwrap();
        for forbidden in [
            "url",
            "text",
            "content",
            "selector",
            "value",
            "screenshot",
            "dialog",
        ] {
            assert!(!object.contains_key(forbidden));
        }
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
