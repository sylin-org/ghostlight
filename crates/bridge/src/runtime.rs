//! Local runtime endpoint discovery shared by the three native processes.
//!
//! An explicit `GHOSTLIGHT_RUNTIME_FILE` elects both the endpoint document and the directory
//! whose authority demand-start launches (ADR-0150); without it, each installation resolves
//! beside its own executable (ADR-0124).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
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

/// Where local runtime discovery resolves for this process: the endpoint document's path and,
/// when an explicit override chose that document, the directory holding the trusted sibling
/// authority that demand-start must launch (ADR-0150).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDiscovery {
    /// The runtime endpoint document the running service publishes.
    pub path: PathBuf,
    /// The elected authority directory when `GHOSTLIGHT_RUNTIME_FILE` set the path; `None`
    /// keeps the default per-installation resolution beside this executable (ADR-0124).
    pub service_directory: Option<PathBuf>,
}

/// Resolve local runtime discovery shared by the three native processes: the endpoint document
/// and, under an explicit override, the directory electing the demand-start authority.
pub fn runtime_discovery() -> RuntimeDiscovery {
    runtime_discovery_from(
        env::var_os("GHOSTLIGHT_RUNTIME_FILE").map(PathBuf::from),
        env::current_exe().ok(),
        env::var_os("HOME").map(PathBuf::from),
        &env::temp_dir(),
        cfg!(target_os = "linux"),
    )
}

fn runtime_discovery_from(
    explicit: Option<PathBuf>,
    executable: Option<PathBuf>,
    home: Option<PathBuf>,
    temporary_directory: &Path,
    linux: bool,
) -> RuntimeDiscovery {
    match explicit {
        Some(path) => RuntimeDiscovery {
            service_directory: path.parent().map(Path::to_path_buf),
            path,
        },
        None => RuntimeDiscovery {
            service_directory: None,
            path: runtime_file_from(None, executable, home, temporary_directory, linux),
        },
    }
}

/// Resolve the runtime endpoint shared by the active sibling installation.
pub fn runtime_file() -> PathBuf {
    runtime_discovery().path
}

fn runtime_file_from(
    explicit: Option<PathBuf>,
    executable: Option<PathBuf>,
    home: Option<PathBuf>,
    temporary_directory: &Path,
    linux: bool,
) -> PathBuf {
    explicit
        .or_else(|| {
            let directory = executable.as_deref().and_then(Path::parent)?;
            if linux && directory == Path::new("/usr/bin") {
                return home.map(|home| home.join(".cache/ghostlight/ghostlight-runtime.json"));
            }
            Some(directory.join("ghostlight-runtime.json"))
        })
        .unwrap_or_else(|| temporary_directory.join("ghostlight-runtime.json"))
}

/// Read the running service endpoint.
pub fn read_runtime(path: &Path) -> io::Result<RuntimeEndpoint> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Atomically replace the running service endpoint with owner-private permissions where supported.
pub fn write_runtime(path: &Path, endpoint: &RuntimeEndpoint) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "runtime path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(endpoint)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    if let Err(error) = fs::rename(&temporary, path) {
        if path.exists()
            && matches!(
                error.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
            )
        {
            fs::remove_file(path)?;
            fs::rename(temporary, path)?;
        } else {
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use std::path::{Path, PathBuf};

    use super::{
        read_runtime, runtime_discovery_from, runtime_file_from, write_runtime, RuntimeEndpoint,
    };

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
        write_runtime(&path, &endpoint(41000)).unwrap();
        write_runtime(&path, &endpoint(42000)).unwrap();
        assert_eq!(read_runtime(&path).unwrap(), endpoint(42000));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn portable_sibling_processes_converge_beside_the_installation() {
        let installation = Path::new("/opt/ghostlight");
        let expected = installation.join("ghostlight-runtime.json");
        for executable in [
            "ghostlight",
            "ghostlight-mcp-connector",
            "ghostlight-browser-connector",
        ] {
            assert_eq!(
                runtime_file_from(
                    None,
                    Some(installation.join(executable)),
                    Some(PathBuf::from("/home/person")),
                    Path::new("/tmp"),
                    true,
                ),
                expected
            );
        }
        assert_eq!(
            runtime_file_from(
                Some(PathBuf::from("/explicit/runtime.json")),
                None,
                None,
                Path::new("/tmp"),
                true,
            ),
            PathBuf::from("/explicit/runtime.json")
        );
    }

    #[test]
    fn an_explicit_runtime_file_elects_its_own_directory() {
        let discovery = runtime_discovery_from(
            Some(PathBuf::from("/dev/tree/ghostlight-runtime.json")),
            Some(PathBuf::from("/installed/ghostlight-mcp-connector")),
            None,
            Path::new("/tmp"),
            true,
        );
        assert_eq!(
            discovery.path,
            PathBuf::from("/dev/tree/ghostlight-runtime.json")
        );
        assert_eq!(
            discovery.service_directory,
            Some(PathBuf::from("/dev/tree"))
        );
    }

    #[test]
    fn the_default_resolution_elects_no_foreign_directory() {
        let discovery = runtime_discovery_from(
            None,
            Some(PathBuf::from("/installed/ghostlight-mcp-connector")),
            Some(PathBuf::from("/home/person")),
            Path::new("/tmp"),
            true,
        );
        assert_eq!(
            discovery.path,
            PathBuf::from("/installed/ghostlight-runtime.json")
        );
        assert_eq!(discovery.service_directory, None);
    }

    #[test]
    fn linux_system_package_siblings_converge_in_the_user_cache() {
        let expected = PathBuf::from("/home/person/.cache/ghostlight/ghostlight-runtime.json");
        for executable in [
            "ghostlight",
            "ghostlight-mcp-connector",
            "ghostlight-browser-connector",
        ] {
            assert_eq!(
                runtime_file_from(
                    None,
                    Some(Path::new("/usr/bin").join(executable)),
                    Some(PathBuf::from("/home/person")),
                    Path::new("/tmp"),
                    true,
                ),
                expected
            );
        }
    }
}
