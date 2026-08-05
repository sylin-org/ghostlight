// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Opaque identity for service-owned browser workspaces.
//!
//! A workspace can outlive one edge connection, so its identity is deliberately independent of
//! a process, transport stream, or protocol session. Only the service mints these handles. The
//! transport crate supplies the type because both shores must serialize it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An opaque, unguessable handle for one service-owned browser workspace.
///
/// The wire form is a canonical lowercase, hyphenated UUIDv4. The raw value is bearer material:
/// [`Display`](std::fmt::Display) and [`Debug`](std::fmt::Debug) are always redacted. Call
/// [`WorkspaceId::as_str`] only for a wire value or an exact routing-map key.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// Mint a fresh CSPRNG-backed UUIDv4.
    ///
    /// This function is public so the service executable can mint a handle. Edge code must only
    /// accept handles returned by the service.
    pub fn mint() -> Self {
        Self(uuid::Uuid::new_v4().hyphenated().to_string())
    }

    /// Parse a canonical lowercase, hyphenated UUIDv4 workspace handle.
    ///
    /// Non-v4 UUIDs and alternate spellings such as uppercase, braced, or URN forms are rejected.
    pub fn parse(value: &str) -> Option<Self> {
        let parsed = uuid::Uuid::parse_str(value).ok()?;
        if parsed.get_version() != Some(uuid::Version::Random) {
            return None;
        }
        if parsed.hyphenated().to_string() != value {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    /// Return the raw canonical handle for serialization or an exact routing-map key.
    ///
    /// Never pass this value to a log, audit record, error message, or metric label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkspaceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WorkspaceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value)
            .ok_or_else(|| serde::de::Error::custom("workspace handle is not a canonical UUIDv4"))
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("<redacted-workspace-id>")
    }
}

impl std::fmt::Debug for WorkspaceId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WorkspaceId(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_handle_round_trips_through_canonical_parse() {
        let workspace = WorkspaceId::mint();
        let parsed = WorkspaceId::parse(workspace.as_str()).expect("minted handle parses");
        assert_eq!(parsed, workspace);
    }

    #[test]
    fn parser_rejects_noncanonical_and_non_v4_values() {
        let workspace = WorkspaceId::mint();
        assert!(WorkspaceId::parse(&workspace.as_str().to_uppercase()).is_none());
        assert!(WorkspaceId::parse("00000000-0000-0000-0000-000000000000").is_none());
        assert!(WorkspaceId::parse("").is_none());
    }

    #[test]
    fn display_and_debug_never_reveal_bearer_material() {
        let workspace = WorkspaceId::mint();
        let raw = workspace.as_str().to_owned();
        let display = workspace.to_string();
        let debug = format!("{workspace:?}");

        assert_eq!(display, "<redacted-workspace-id>");
        assert_eq!(debug, "WorkspaceId(<redacted>)");
        assert!(!display.contains(&raw));
        assert!(!debug.contains(&raw));
    }

    #[test]
    fn serde_uses_raw_canonical_wire_form_and_validates_input() {
        let workspace = WorkspaceId::mint();
        let json = serde_json::to_string(&workspace).expect("serialize workspace");
        assert_eq!(json, format!("\"{}\"", workspace.as_str()));
        assert_eq!(
            serde_json::from_str::<WorkspaceId>(&json).expect("deserialize workspace"),
            workspace
        );
        assert!(serde_json::from_str::<WorkspaceId>("\"NOT-A-HANDLE\"").is_err());
    }
}
