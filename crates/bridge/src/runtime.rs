//! Local runtime endpoint discovery shared by the three native processes.
//!
//! Ordinary launches share the user's installation (ADR-0167). An explicit
//! `GHOSTLIGHT_RUNTIME_FILE` retains isolated test discovery and demand-start.

use std::env;
use std::fs;
#[cfg(not(target_os = "windows"))]
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Authenticated loopback endpoints published by the running service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEndpoint {
    /// TCP port for MCP-edge sessions.
    pub service_port: u16,
    /// TCP port for the browser relay.
    pub browser_port: u16,
    /// Per-service random authentication token.
    pub token: String,
    /// Service-edge bridge major offered to MCP connectors.
    pub service_bridge_major: u16,
    /// Browser-relay protocol major offered to native hosts.
    pub browser_relay_major: u16,
    /// Product version.
    pub service_version: String,
}

/// Shared endpoint and selected authority directory (ADR-0167).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiscovery {
    /// The runtime endpoint document the running service publishes.
    pub path: PathBuf,
    /// Selected authority directory, or the explicit fixture's directory.
    /// `None` means the user's installation has not yet been bootstrapped.
    pub service_directory: Option<PathBuf>,
}

/// Resolve local runtime discovery shared by the three native processes: the endpoint document
/// and, under an explicit override, the directory electing the demand-start authority.
pub fn runtime_discovery() -> io::Result<RuntimeDiscovery> {
    if let Some(path) = env::var_os("GHOSTLIGHT_RUNTIME_FILE").map(PathBuf::from) {
        return Ok(RuntimeDiscovery {
            service_directory: path.parent().map(Path::to_path_buf),
            path,
        });
    }
    let path = crate::installation::production_runtime()?;
    let service_directory =
        crate::installation::read(&path)?.map(|record| record.directory().to_path_buf());
    Ok(RuntimeDiscovery {
        path,
        service_directory,
    })
}

/// Resolve the runtime endpoint shared by the active sibling installation.
pub fn runtime_file() -> io::Result<PathBuf> {
    Ok(runtime_discovery()?.path)
}

/// Read the running service endpoint.
pub fn read_runtime(path: &Path) -> io::Result<RuntimeEndpoint> {
    const MAX_RUNTIME_BYTES: u64 = 64 * 1024;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_RUNTIME_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RUNTIME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "runtime discovery is too large",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Atomically replace the running service endpoint with owner-private permissions where supported.
pub fn write_runtime(path: &Path, endpoint: &RuntimeEndpoint) -> io::Result<()> {
    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_private_document(path, &bytes)
}

/// Publish a private local lifecycle document with the runtime writer's custody rules.
pub(crate) fn write_private_document(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "runtime path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    #[cfg(target_os = "windows")]
    let mut file = ghostlight_win_peer::create_private_file(&temporary)?;
    #[cfg(not(target_os = "windows"))]
    let mut file = {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&temporary)?
    };
    let published = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        // Both supported platforms replace atomically. A sharing/permission failure must
        // preserve the old record, never unlink the last good selection to retry publication.
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if published.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    published
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{read_runtime, write_runtime, RuntimeEndpoint};

    fn endpoint(port: u16) -> RuntimeEndpoint {
        RuntimeEndpoint {
            service_port: port,
            browser_port: port + 1,
            token: format!("runtime_{port}"),
            service_bridge_major: 1,
            browser_relay_major: 1,
            service_version: "1.0.0".into(),
        }
    }

    #[test]
    fn runtime_discovery_replaces_an_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "ghostlight-runtime-replace-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"old inherited discovery").unwrap();
        let legacy_temporary = path.with_extension("json.tmp");
        fs::write(&legacy_temporary, b"unowned old temporary").unwrap();
        write_runtime(&path, &endpoint(41000)).unwrap();
        write_runtime(&path, &endpoint(42000)).unwrap();
        assert_eq!(read_runtime(&path).unwrap(), endpoint(42000));
        assert_eq!(
            fs::read(&legacy_temporary).unwrap(),
            b"unowned old temporary"
        );
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(legacy_temporary).unwrap();
        fs::remove_file(path).unwrap();
    }
}
