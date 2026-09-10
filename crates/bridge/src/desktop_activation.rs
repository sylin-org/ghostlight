//! Closed Linux desktop activation mechanism, never product work or workbench reveal.
//!
//! The installer owns registration. Callers must pass the shared lifecycle admission gate
//! before requesting activation; the desktop claims the name only after native startup.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The sole installed host application that sandbox connectors may demand-start.
pub const BUS_NAME: &str = "org.sylin.ghostlight";
/// Flatpak application explicitly admitted by the initial Linux activation scope.
pub const FLATPAK_APP: &str = "org.chromium.Chromium";
const REGISTRATION_LIMIT: u64 = 16 * 1024;
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// Locate the default host user-data activation file, independent of sandbox XDG remapping.
/// Custom host XDG data roots are not inferred from the sandbox's private data directory.
#[must_use]
pub fn registration_path(home: &Path) -> PathBuf {
    home.join(".local/share/dbus-1/services")
        .join(format!("{BUS_NAME}.service"))
}

/// Render the exact one-executable, no-argument session activation registration.
pub fn registration_bytes(executable: &Path) -> io::Result<Vec<u8>> {
    let path = executable
        .to_str()
        .ok_or_else(|| invalid("non-UTF-8 executable path"))?;
    if !executable.is_absolute()
        || executable
            .file_name()
            .is_none_or(|name| name != "ghostlight")
        || path.chars().any(char::is_control)
    {
        return Err(invalid(
            "activation requires an absolute ghostlight executable",
        ));
    }
    // D-Bus Exec uses shell-style argument parsing, not shell execution. Escape the one
    // quoted argument; there are deliberately no field codes or application arguments.
    let quoted = path.replace('\\', "\\\\").replace('"', "\\\"");
    Ok(format!("[D-BUS Service]\nName={BUS_NAME}\nExec=\"{quoted}\"\n").into_bytes())
}

/// Verify byte-exact registration for an elected executable without following a file symlink.
pub fn registration_matches(path: &Path, executable: &Path) -> io::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > REGISTRATION_LIMIT {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(REGISTRATION_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes == registration_bytes(executable)?)
}

/// Ask only the installed name to start, after shared lease/deployment/retry admission.
/// A successful request is not a spawned PID, authenticated connection, or work result.
pub fn request_registered_start(home: &Path, executable: &Path) -> io::Result<()> {
    if !registration_matches(&registration_path(home), executable)? {
        return Err(invalid(
            "desktop activation does not name the elected installation",
        ));
    }
    within_deadline(CALL_TIMEOUT, async {
        let connection = zbus::Connection::session().await.map_err(bus_error)?;
        let proxy = zbus::fdo::DBusProxy::new(&connection)
            .await
            .map_err(bus_error)?;
        let name = BUS_NAME.try_into().map_err(bus_error)?;
        match proxy
            .start_service_by_name(name, 0)
            .await
            .map_err(bus_error)?
        {
            1 | 2 => Ok(()),
            _ => Err(invalid("unrecognized desktop activation acknowledgment")),
        }
    })
}

/// Refresh the host session bus after explicit installation changes its activation files.
pub fn refresh_registration_cache() -> io::Result<()> {
    within_deadline(CALL_TIMEOUT, async {
        let connection = zbus::Connection::session().await.map_err(bus_error)?;
        let proxy = zbus::fdo::DBusProxy::new(&connection)
            .await
            .map_err(bus_error)?;
        proxy.reload_config().await.map_err(bus_error)
    })
}

/// Lifetime of the installed activation name, held only by a ready host desktop authority.
pub struct ActivationNameLease {
    _connection: zbus::Connection,
}

/// Claim the fixed name without queueing or replacing another owner after desktop readiness.
/// An absent or nonmatching registration leaves ordinary native startup unchanged.
pub fn claim_ready_name(home: &Path, executable: &Path) -> io::Result<Option<ActivationNameLease>> {
    if !registration_matches(&registration_path(home), executable)? {
        return Ok(None);
    }
    within_deadline(CALL_TIMEOUT, async {
        let connection = zbus::Connection::session().await.map_err(bus_error)?;
        let reply = connection
            .request_name_with_flags(BUS_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
            .await
            .map_err(bus_error)?;
        match reply {
            zbus::fdo::RequestNameReply::PrimaryOwner
            | zbus::fdo::RequestNameReply::AlreadyOwner => Ok(Some(ActivationNameLease {
                _connection: connection,
            })),
            _ => Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "desktop activation name already owned",
            )),
        }
    })
}

fn within_deadline<T>(
    timeout: Duration,
    operation: impl std::future::Future<Output = io::Result<T>>,
) -> io::Result<T> {
    // Bound connect, authentication, Hello and activation together. A method-only timeout
    // does not cover an unresponsive authentication peer. Dropping the losing future cancels
    // that exchange without a detached timeout helper. An activation already submitted to
    // the bus can still finish; only the shared readiness seam may confirm its outcome.
    async_io::block_on(futures_lite::future::or(operation, async move {
        async_io::Timer::after(timeout).await;
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "desktop activation exchange timed out",
        ))
    }))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn bus_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bus_that_accepts_but_never_authenticates_cannot_hold_the_exchange() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.tmp");
        fs::create_dir_all(&base).unwrap();
        let root = base
            .canonicalize()
            .unwrap()
            .join(format!("b-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("bus");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let (stop, wait) = std::sync::mpsc::channel::<()>();
        let peer = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            let _ = wait.recv_timeout(Duration::from_secs(2));
        });
        let started = std::time::Instant::now();
        let result = within_deadline(Duration::from_millis(100), async {
            zbus::connection::Builder::address(format!("unix:path={}", path.display()).as_str())
                .map_err(bus_error)?
                .build()
                .await
                .map_err(bus_error)
        });
        let _ = stop.send(());
        peer.join().unwrap();
        assert!(matches!(result, Err(ref error) if error.kind() == io::ErrorKind::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(1));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registration_has_one_fixed_name_and_no_arguments() {
        assert_eq!(registration_bytes(Path::new("/opt/Ghostlight test/ghostlight")).unwrap(),
            b"[D-BUS Service]\nName=org.sylin.ghostlight\nExec=\"/opt/Ghostlight test/ghostlight\"\n");
        for path in [
            "relative/ghostlight",
            "/opt/other",
            "/opt/bad\nline/ghostlight",
        ] {
            assert!(registration_bytes(Path::new(path)).is_err());
        }
    }

    #[test]
    fn unregistered_installation_neither_requests_nor_claims_the_name() {
        let home = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("unregistered-{}", uuid::Uuid::new_v4()));
        let executable = Path::new("/selected/ghostlight");
        assert!(claim_ready_name(&home, executable).unwrap().is_none());
        assert_eq!(
            request_registered_start(&home, executable)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!home.exists());
    }

    #[test]
    fn sandbox_data_home_is_not_the_host_registration_root() {
        assert_eq!(
            registration_path(Path::new("/home/person")),
            Path::new("/home/person/.local/share/dbus-1/services/org.sylin.ghostlight.service")
        );
    }

    #[test]
    fn ownership_is_exact_and_foreign_missing_or_symlinked_registration_is_not_used() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.tmp")
            .join(format!("activation-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("registration.service");
        let executable = Path::new("/selected/ghostlight");
        assert!(!registration_matches(&file, executable).unwrap());
        fs::write(&file, registration_bytes(executable).unwrap()).unwrap();
        assert!(registration_matches(&file, executable).unwrap());
        assert!(!registration_matches(&file, Path::new("/other/ghostlight")).unwrap());
        let link = root.join("link.service");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(!registration_matches(&link, executable).unwrap());
        fs::write(&file, b"[D-BUS Service]\nExec=/foreign\n").unwrap();
        assert!(!registration_matches(&file, executable).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
}
