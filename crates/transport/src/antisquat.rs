// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Anti-squat: per-install secret + HMAC proof (ADR-0030 Decision 8; PINS.md SS5.3).
//!
//! Defeats a NAIVE or CROSS-USER process that squats the well-known adapter/control endpoint name
//! without knowing the per-install secret: the genuine SERVICE proves possession of a 32-byte,
//! per-user secret (`hub-key`, under `crate::observability::shared_data_dir`) to every connecting
//! ADAPTER via an HMAC-SHA256 proof over the adapter's own hello bytes, before the adapter relays a
//! single byte of its stdio. This is defense-in-depth, not a same-user sandbox: a determined
//! same-user process can read any same-user file (Decision 8).
//!
//! The key is PER-INSTANCE (ADR-0064): it lives under the instance's own `log_dir`, so an adapter
//! and the service it connects to -- both pinned to the same instance -- resolve the SAME secret,
//! while two different instances (a `dev` alongside the default) each own an isolated key. This
//! replaces the ADR-0048 instance-INDEPENDENT `shared_data_dir` key, which only existed so an
//! unpinned (default-identity) adapter could verify a live `dev` service's proof -- a case that no
//! longer exists now that every client pins one instance. The threat model is unchanged (cross-user
//! squatting; the key is per-USER via the user-owned data dir), so per-instance keys lose no defense.
//! The DEFAULT instance's key location is byte-identical to before (`<data>/ghostlight/hub-key`).

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

/// The per-install secret's filename under the instance's `crate::observability::log_dir`
/// (PINS.md SS5.3): PER-USER and PER-INSTANCE (ADR-0064), never a machine-wide directory (that
/// corrected an earlier `%ProgramData%` draft). Created lazily on the first `run_service` start.
const HUB_KEY_FILE: &str = "hub-key";

/// The pinned refusal text (PINS.md SS5.3), logged and returned verbatim by the adapter on ANY
/// anti-squat failure (missing/unreadable key, unreachable peer, malformed frame, wrong role, MAC
/// mismatch) -- every failure mode collapses to this ONE message so a squatter cannot learn which
/// check caught it.
pub const REFUSAL_MESSAGE: &str = "refusing to connect: the Ghostlight service on this endpoint is not the one this user installed";

fn hub_key_path() -> std::io::Result<PathBuf> {
    // ADR-0064: the hub-key lives under the INSTANCE's own `log_dir`, so an adapter and the service
    // it dials -- both pinned to the same instance -- resolve the same secret, and two instances own
    // isolated keys. (The ADR-0048 instance-independent key only existed for the retired unpinned
    // adapter -> dev-service shadow.)
    let dir = crate::observability::log_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no per-user data directory available for the hub-key",
        )
    })?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(HUB_KEY_FILE))
}

/// Read and validate an existing key file's exact 32 bytes. `Ok(None)` iff it does not exist yet
/// (never an error -- both callers below decide what an absence means for their own role).
fn read_key_file(path: &Path) -> std::io::Result<Option<[u8; 32]>> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    if buf.len() != 32 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("hub-key has {} bytes, expected 32", buf.len()),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&buf);
    Ok(Some(key))
}

/// Load the per-install secret, creating it (32 CSPRNG bytes via `getrandom`) if absent. SERVICE
/// only: called once at `run_service` startup, before any adapter can possibly connect, so no
/// connection ever races the file's first creation (PINS.md SS5.3). `0600` on Unix after write;
/// the per-user `%LOCALAPPDATA%` ACL suffices on Windows (no DPAPI -- it adds no same-user defense
/// and no dependency).
pub fn load_or_create_hub_key() -> std::io::Result<[u8; 32]> {
    let path = hub_key_path()?;
    if let Some(key) = read_key_file(&path)? {
        return Ok(key);
    }
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

/// Read the per-install secret. ADAPTER only: a missing or wrong-length file is an error -- the
/// adapter never creates this file itself; only the SERVICE owns the secret's lifecycle.
pub fn read_hub_key() -> std::io::Result<[u8; 32]> {
    let path = hub_key_path()?;
    read_key_file(&path)?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no hub-key present yet"))
}

/// The lowercase-hex HMAC-SHA256 of `message` keyed by `key` (PINS.md SS5.3).
pub fn compute_mac_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac =
        <HmacSha256 as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    hex_encode(&mac.finalize().into_bytes())
}

/// Verify `mac_hex` (lowercase hex) is the correct, constant-time HMAC-SHA256 of `message` keyed
/// by `key` (`hmac::Mac::verify_slice`, PINS.md SS5.3). `false` for any malformed hex or a key
/// HMAC itself rejects (neither of which should occur with a real 32-byte hub-key).
pub fn verify_mac_hex(key: &[u8], message: &[u8], mac_hex: &str) -> bool {
    let Some(mac_bytes) = hex_decode(mac_hex) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(key) else {
        return false;
    };
    mac.update(message);
    mac.verify_slice(&mac_bytes).is_ok()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let bytes = [0u8, 1, 255, 16, 32];
        let hex = hex_encode(&bytes);
        assert_eq!(hex, "0001ff1020");
        assert_eq!(hex_decode(&hex).unwrap(), bytes);
    }

    #[test]
    fn hex_decode_rejects_odd_length_and_non_hex() {
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
    }

    #[test]
    fn compute_and_verify_round_trip() {
        let key = [7u8; 32];
        let message = b"hello adapter";
        let mac = compute_mac_hex(&key, message);
        assert!(verify_mac_hex(&key, message, &mac));
    }

    #[test]
    fn verify_rejects_a_wrong_key() {
        let key_a = [1u8; 32];
        let key_b = [2u8; 32];
        let message = b"hello adapter";
        let mac = compute_mac_hex(&key_a, message);
        assert!(!verify_mac_hex(&key_b, message, &mac));
    }

    #[test]
    fn verify_rejects_a_tampered_message() {
        let key = [3u8; 32];
        let mac = compute_mac_hex(&key, b"original");
        assert!(!verify_mac_hex(&key, b"tampered", &mac));
    }

    #[test]
    fn verify_rejects_malformed_hex() {
        let key = [4u8; 32];
        assert!(!verify_mac_hex(&key, b"message", "not-hex"));
    }
}
