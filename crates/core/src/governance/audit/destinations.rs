// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Where audit lines go: the default file path and the two write primitives (file, stderr).

/// Default audit file path (shared format doc section 1.4): `dirs::data_local_dir()` joined
/// with `ghostlight` then `audit.jsonl`. `dirs::data_local_dir()` maps exactly to the
/// section-1.4 table: `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS,
/// `~/.local/share` (or `XDG_DATA_HOME`) on Linux. `None` when the platform data directory
/// cannot be resolved.
///
/// ADR-0051 Phase 1: a `GHOSTLIGHT_AUDIT_DIR` env override redirects the default path to
/// `<GHOSTLIGHT_AUDIT_DIR>/audit.jsonl`, making it test-isolable. `dirs::data_local_dir()` ignores
/// env, so without this a spawned service writes to the machine's REAL audit file and parallel E2E
/// tests contend on it. This matches the existing `GHOSTLIGHT_LOG_DIR` / `GHOSTLIGHT_USER_CONFIG_DIR`
/// / `ProgramData` override precedent. The pure resolver `default_audit_path_from` is split out so it
/// unit-tests without racing the process-global env.
pub fn default_audit_path() -> Option<std::path::PathBuf> {
    default_audit_path_from(std::env::var_os("GHOSTLIGHT_AUDIT_DIR"))
}

fn default_audit_path_from(override_dir: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    if let Some(dir) = override_dir {
        return Some(std::path::PathBuf::from(dir).join("audit.jsonl"));
    }
    Some(
        dirs::data_local_dir()?
            .join(ghostlight_transport::instance::Instance::resolve().dir_leaf())
            .join("audit.jsonl"),
    )
}

/// Append one line to `path`, creating parent directories if needed. Writes the line bytes
/// followed by a single LF (never CRLF, on every platform: the JSON Lines rule, shared format
/// doc section 6). One open-append-close per record: simple, rotation-friendly, and cheap at
/// tool-call frequency.
pub fn append_line_to_file(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Write one line to stderr. stdout is reserved for the MCP protocol stream; stderr records
/// interleave with `tracing` output by design (that is what the `stderr` destination means).
pub fn write_line_to_stderr(line: &str) {
    eprintln!("{line}");
}

/// Send one RFC 5424 syslog datagram to `addr` over UDP, carrying `line` (the serialized JSONL
/// audit record, unchanged) as MSG. PRI 134 = facility 16 (local0) * 8 + severity 6 (info);
/// HOSTNAME, MSGID, and STRUCTURED-DATA are the RFC NILVALUE `-`; APP-NAME is `ghostlight`;
/// PROCID is this process's id. One socket per call, mirroring the open-per-record file
/// destination.
pub fn send_line_to_syslog(addr: std::net::SocketAddr, line: &str) -> std::io::Result<()> {
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let pid = std::process::id();
    let datagram = format!("<134>1 {ts} - ghostlight {pid} - - {line}");
    let udp_socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    udp_socket.send_to(datagram.as_bytes(), addr)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_dir_override_redirects_to_that_dir_plus_audit_jsonl() {
        // ADR-0051 Phase 1: an explicit GHOSTLIGHT_AUDIT_DIR value yields <dir>/audit.jsonl,
        // bypassing the platform default -- tested through the pure resolver so no env race.
        let got = default_audit_path_from(Some(std::ffi::OsString::from("/tmp/ghostlight-test")))
            .expect("override always resolves");
        assert_eq!(
            got,
            std::path::PathBuf::from("/tmp/ghostlight-test").join("audit.jsonl")
        );
    }

    #[test]
    fn no_override_falls_back_to_the_platform_default_ending_in_audit_jsonl() {
        // With no override, the resolver uses dirs::data_local_dir(); when that resolves, the path
        // still ends in the ghostlight instance leaf + audit.jsonl.
        if let Some(p) = default_audit_path_from(None) {
            assert!(
                p.ends_with("audit.jsonl"),
                "default path ends in audit.jsonl: {p:?}"
            );
        }
    }
}
