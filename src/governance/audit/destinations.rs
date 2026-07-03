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
