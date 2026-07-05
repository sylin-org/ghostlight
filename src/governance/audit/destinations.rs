// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Where audit lines go: the default file path and the two write primitives (file, stderr).

/// Default audit file path (shared format doc section 1.4): `dirs::data_local_dir()` joined
/// with `ghostlight` then `audit.jsonl`. `dirs::data_local_dir()` maps exactly to the
/// section-1.4 table: `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS,
/// `~/.local/share` (or `XDG_DATA_HOME`) on Linux. `None` when the platform data directory
/// cannot be resolved.
pub fn default_audit_path() -> Option<std::path::PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("ghostlight")
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
