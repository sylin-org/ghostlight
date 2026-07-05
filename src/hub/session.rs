// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-session identity (ADR-0030 Decision 4: "identity model (adapter-minted GUID; core stays
//! PID-agnostic)" and its transport-side amendment). The thin ADAPTER mints an opaque, unguessable
//! [`SessionGuid`] and presents it in the adapter/control session-hello (`src/hub/handshake.rs`,
//! PINS.md SS1); the LOCAL accept layer (`src/transport/native/ipc.rs`) captures the connecting
//! peer's OS credential ([`PeerCred`]/[`PeerUser`]) purely for admission control, binding a GUID to
//! its minting peer via [`SessionRegistry::admit`]. Lives in `src/hub`, NEVER in `src/governance`
//! (a7): the governance core gains no pid/ancestor/GUID concept from any of these types.

use std::collections::HashMap;

/// An opaque, unguessable session identity minted by the adapter and presented to the service.
/// Canonical lowercase hyphenated UUIDv4 (36 chars). Secret material (ADR-0030 Decision 4:
/// "Treat the GUID as secret in logs/audit"): [`Display`](std::fmt::Display) and
/// [`Debug`](std::fmt::Debug) both render a fixed redacted placeholder rather than the raw
/// canonical string, so a `tracing::info!(guid = %guid, ...)` or `{:?}` never leaks it into a log
/// or audit sink. Use [`SessionGuid::as_str`] ONLY for the wire handshake and the routing-map key.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionGuid(String);

impl SessionGuid {
    /// Mint a fresh CSPRNG UUIDv4 (`uuid::Uuid::new_v4()`). The adapter role calls this ONCE per
    /// adapter process and reuses the same value for the process lifetime (ADR-0030 Decision 4:
    /// "Same adapter process reuses its GUID (same group); a new adapter process mints a new one").
    /// The SERVICE's own directly-served stdio session also mints one for itself (PINS.md SS9):
    /// every session gets a real GUID, closing an isolation gap an exempt lone session would
    /// otherwise leave in a later cross-session ownership map.
    pub fn mint() -> Self {
        Self(uuid::Uuid::new_v4().hyphenated().to_string())
    }

    /// Parse a presented string; `Some` iff it is a valid version-4 UUID in canonical (lowercase,
    /// hyphenated, unbraced) form -- the exact form a valid [`Self::mint`] output round-trips to.
    /// Any other UUID version, or a syntactically valid UUID in a non-canonical form (uppercase,
    /// braced, urn:), is refused, matching a malformed/empty presented guid the same way.
    pub fn parse(s: &str) -> Option<Self> {
        let parsed = uuid::Uuid::parse_str(s).ok()?;
        if parsed.get_version() != Some(uuid::Version::Random) {
            return None;
        }
        if parsed.hyphenated().to_string() != s {
            return None;
        }
        Some(Self(s.to_string()))
    }

    /// The raw canonical string (for the wire handshake and the routing-map key ONLY -- never a
    /// log or audit sink; see the redacted [`Display`](std::fmt::Display)/[`Debug`](std::fmt::Debug)
    /// impls below).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionGuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<redacted-session-guid>")
    }
}

impl std::fmt::Debug for SessionGuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SessionGuid(<redacted>)")
    }
}

/// The connecting peer's OS credential, captured by the LOCAL accept layer (`ipc::serve_adapters`)
/// purely for admission control and as the per-peer rate-limit key (ADR-0030 Decision 4
/// amendment). Lives in `src/hub`, NEVER in `src/governance` (a7). `user` is the peer's OS user
/// principal: the SID string on Windows, the uid on Unix. `pid` distinguishes processes for
/// logging only; admission compares `user`.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerCred {
    pub user: PeerUser,
    pub pid: u32,
}

/// Opaque OS-user principal; same-user comparison is `==`. `Hash` (PINS.md SS9): a later per-peer
/// quota table keyed by `PeerUser` requires it.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PeerUser(pub String);

/// Outcome of [`SessionRegistry::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Admitted,
    Refused,
}

/// The service's GUID -> bound-peer routing map (ADR-0030 Decision 2: per-session state lives in
/// `src/hub`). Keyed on the GUID's canonical string.
pub struct SessionRegistry {
    bindings: HashMap<String, PeerCred>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Admit a peer presenting a GUID. First presentation records the binding and returns
    /// [`Admission::Admitted`]. A re-presentation is `Admitted` iff the presenter is the SAME OS
    /// user as the bound peer (the sanctioned reuse path re-verifies same-user); a DIFFERENT user
    /// is [`Admission::Refused`] and the existing binding is left unchanged (ADR-0030 Decision 4:
    /// "refuse a GUID presented by a different peer, except the sanctioned reuse path which
    /// re-verifies same-user").
    pub fn admit(&mut self, guid: &SessionGuid, peer: &PeerCred) -> Admission {
        match self.bindings.get(guid.as_str()) {
            Some(bound) if bound.user == peer.user => Admission::Admitted,
            Some(_) => Admission::Refused,
            None => {
                self.bindings
                    .insert(guid.as_str().to_string(), peer.clone());
                Admission::Admitted
            }
        }
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_produces_a_parseable_v4_guid() {
        let g = SessionGuid::mint();
        assert!(SessionGuid::parse(g.as_str()).is_some());
    }

    #[test]
    fn parse_rejects_empty_and_malformed() {
        assert!(SessionGuid::parse("").is_none());
        assert!(SessionGuid::parse("not-a-uuid").is_none());
    }

    #[test]
    fn admit_binds_first_presentation_and_allows_same_user_reuse() {
        let g = SessionGuid::mint();
        let mut registry = SessionRegistry::new();
        let a = PeerCred {
            user: PeerUser("user-A".into()),
            pid: 1,
        };
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
    }

    #[test]
    fn admit_refuses_a_different_user() {
        let g = SessionGuid::mint();
        let mut registry = SessionRegistry::new();
        let a = PeerCred {
            user: PeerUser("user-A".into()),
            pid: 1,
        };
        let b = PeerCred {
            user: PeerUser("user-B".into()),
            pid: 2,
        };
        assert_eq!(registry.admit(&g, &a), Admission::Admitted);
        assert_eq!(registry.admit(&g, &b), Admission::Refused);
    }
}
