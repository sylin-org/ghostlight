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

/// The size cap for the audit file sink before it rotates (OPS-MED-01: the audit file must not
/// grow without bound during minimal-upkeep coasting). 50 MiB per generation; with a single
/// retained backup (`.1`), on-disk audit is bounded at ~2x this. The syslog and stderr sinks are
/// unaffected -- rotation is a file-sink concern only. High enough that a normal service run never
/// rotates mid-run, low enough that an unattended machine cannot fill its disk with audit lines.
pub const AUDIT_FILE_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Append one line to `path`, creating parent directories if needed. Writes the line bytes
/// followed by a single LF (never CRLF, on every platform: the JSON Lines rule, shared format
/// doc section 6). One open-append-close per record: simple, rotation-friendly, and cheap at
/// tool-call frequency. Rotates the file at [`AUDIT_FILE_MAX_BYTES`] before appending, keeping a
/// single previous generation, so the sink is bounded (OPS-MED-01).
pub fn append_line_to_file(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    rotate_if_needed(path, AUDIT_FILE_MAX_BYTES)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// The single-generation backup path for `path`: the same path with a `.1` suffix appended
/// (`audit.jsonl` -> `audit.jsonl.1`). Kept as a plain suffix so the backup sorts next to the
/// live file and is obvious to an operator.
fn rotated_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".1");
    std::path::PathBuf::from(name)
}

/// Rotate `path` when it has reached `max_bytes`, keeping exactly ONE previous generation: the
/// current file is renamed to its `.1` backup (replacing any earlier backup), and the next append
/// re-creates a fresh file. `max_bytes == 0` disables rotation (an unbounded escape hatch). A
/// missing file is size 0 and never rotates. Best-effort: a rotation error propagates so the
/// caller's `tracing::warn!` reports it, exactly like any other audit write failure.
fn rotate_if_needed(path: &std::path::Path, max_bytes: u64) -> std::io::Result<()> {
    if max_bytes == 0 {
        return Ok(());
    }
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if len < max_bytes {
        return Ok(());
    }
    let backup = rotated_path(path);
    // rename() fails on Windows if the destination exists, so clear the prior backup first. A
    // failed remove of a nonexistent file is fine; only the rename result gates rotation.
    let _ = std::fs::remove_file(&backup);
    std::fs::rename(path, &backup)
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

    fn rotate_temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ghostlight-audit-rotate-{}-{tag}.jsonl",
            std::process::id()
        ))
    }

    #[test]
    fn rotated_path_appends_a_dot_one_suffix() {
        let p = std::path::Path::new("/var/log/ghostlight/audit.jsonl");
        assert_eq!(
            rotated_path(p),
            std::path::PathBuf::from("/var/log/ghostlight/audit.jsonl.1")
        );
    }

    #[test]
    fn appending_past_the_cap_rotates_to_a_single_backup_and_starts_fresh() {
        let path = rotate_temp_path("cap");
        let backup = rotated_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);

        // A tiny cap so a couple of lines cross it. append_line_to_file uses the real const, so
        // drive rotation through the seam directly, then confirm the append re-creates the file.
        let first = "first-record-line-well-over-the-tiny-cap-below";
        std::fs::write(&path, format!("{first}\n")).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() >= 8);

        rotate_if_needed(&path, 8).unwrap();
        // The live file was renamed to its backup; the backup holds the old content.
        assert!(!path.exists(), "the live file was rotated away");
        let backup_content = std::fs::read_to_string(&backup).unwrap();
        assert!(backup_content.contains(first), "backup holds prior content");

        // A subsequent append re-creates a fresh live file with only the new line.
        append_line_to_file(&path, "second-record").unwrap();
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert_eq!(fresh, "second-record\n");

        std::fs::remove_file(&path).ok();
        std::fs::remove_file(&backup).ok();
    }

    #[test]
    fn rotation_keeps_only_one_generation() {
        let path = rotate_temp_path("one-gen");
        let backup = rotated_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);

        // First rotation: "old" becomes the backup.
        std::fs::write(&path, "old-generation\n").unwrap();
        rotate_if_needed(&path, 4).unwrap();
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "old-generation\n"
        );

        // Second rotation: "newer" replaces the backup; the older generation is discarded, so
        // total on-disk audit stays bounded at one live + one backup.
        std::fs::write(&path, "newer-generation\n").unwrap();
        rotate_if_needed(&path, 4).unwrap();
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "newer-generation\n"
        );

        std::fs::remove_file(&path).ok();
        std::fs::remove_file(&backup).ok();
    }

    #[test]
    fn a_zero_cap_disables_rotation() {
        let path = rotate_temp_path("zero-cap");
        let backup = rotated_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);

        std::fs::write(&path, "content\n").unwrap();
        rotate_if_needed(&path, 0).unwrap();
        assert!(path.exists(), "cap 0 never rotates");
        assert!(!backup.exists(), "no backup written when rotation disabled");

        std::fs::remove_file(&path).ok();
    }
}
