//! Inter-instance IPC between the mcp-server-role and native-host-role instances.
//!
//! Transport: a **local socket** -- a Windows named pipe (`tokio::net::windows::named_pipe`) or a
//! Unix domain socket (`tokio::net::Unix*`), chosen at compile time. No localhost TCP, no network
//! dependency, and (unlike a TCP port) it can be access-controlled to the current user.
//!
//! We use **tokio-native** transports rather than the `interprocess` crate: interprocess's own async
//! Windows named-pipe layer does not reliably wake a pending read when the peer process dies (its
//! "linger pool" deliberately delays EOF), so a killed mcp-server left the native-host as a zombie
//! that never observed the disconnect. tokio's NamedPipe/UnixStream are first-class mio/IOCP sources
//! whose reads surface `Ok(0)`/`BrokenPipe` promptly on peer death -- no application heartbeat.
//!
//! Ownership (mirrors the reference's proven ordering): the **mcp-server** instance (launched first
//! by the MCP client, long-lived) owns the endpoint and [`serve`]s it; the **native-host** instance
//! (launched by Chrome, short-lived, may relaunch on service-worker wake) [`connect`]s with retry
//! and relays frames between the extension and the mcp-server ([`relay_native_host`]). Single active
//! session: a second mcp-server refuses with [`Error::SessionBusy`].

use crate::transport::executor::{AttachOutcome, Browser};
use crate::transport::native::host;
use crate::{Error, Result};
use tokio::time::{sleep, Duration};

/// Default endpoint base name; override with `GHOSTLIGHT_ENDPOINT` (used by tests and advanced
/// deployments that run more than one isolated instance on a host). Each platform derives the real
/// path from it: `\\.\pipe\<name>` on Windows, `<runtime-dir>/ghostlight/<name>.sock` on Unix.
const DEFAULT_ENDPOINT: &str = "org.sylin.ghostlight.v1";

/// The endpoint name both roles use: the `GHOSTLIGHT_ENDPOINT` env override, else the default.
pub fn default_endpoint() -> String {
    std::env::var("GHOSTLIGHT_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

/// native-host role: connect to the mcp-server endpoint and relay frames between Chrome native
/// messaging (this process's stdin/stdout) and the mcp-server, until either side closes. Transport
/// agnostic: works over whichever local socket [`connect`] returns.
///
/// When the mcp-server dies, the tokio-native read on `ipc_read` returns (EOF/BrokenPipe) and the
/// `select!` completes, so the process exits and the extension reconnects to the next session --
/// no zombie. (Do NOT add a post-`select!` `ipc_write.shutdown().await`: on an already-dead Windows
/// pipe that write never completes and would itself hang the process. Dropping the halves on return
/// closes the handle synchronously, which is all we need.)
///
/// `debug` is env-gated (see `main::run_native_host_role`): Chrome inherits its own environment
/// when it launches this process and never passes `--debug`, so a native-host debug snapshot only
/// exists when Chrome itself was started with `GHOSTLIGHT_DEBUG=1`. Its absence is normal.
pub async fn relay_native_host(endpoint: &str, debug: &crate::debug::DebugSink) -> Result<()> {
    let stream = connect(endpoint).await?;
    debug.ipc_note("connected to mcp-server endpoint");
    let (mut ipc_read, mut ipc_write) = tokio::io::split(stream);
    let mut chrome_in = tokio::io::stdin();
    let mut chrome_out = tokio::io::stdout();

    // extension -> mcp-server
    let upstream = async {
        while let Ok(Some(frame)) = host::read_message(&mut chrome_in).await {
            debug.frame_in();
            if host::write_message(&mut ipc_write, &frame).await.is_err() {
                break;
            }
        }
    };
    // mcp-server -> extension
    let downstream = async {
        while let Ok(Some(frame)) = host::read_message(&mut ipc_read).await {
            if host::write_message(&mut chrome_out, &frame).await.is_err() {
                break;
            }
            debug.frame_out();
        }
    };

    tokio::select! {
        _ = upstream => {}
        _ = downstream => {}
    }
    debug.ipc_note("relay ended");
    Ok(())
}

/// Result of a one-shot, synchronous probe of the IPC endpoint (see [`probe_endpoint`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointProbe {
    /// No pipe/socket of this name exists: no mcp-server currently owns the endpoint.
    Absent,
    /// The endpoint exists and accepted a connection (opened and closed immediately).
    Accepts,
    /// The endpoint exists but the probe could not connect (detail explains why).
    Rejects(String),
}

// --- Windows: named pipes ---

#[cfg(windows)]
fn pipe_path(endpoint: &str) -> String {
    format!(r"\\.\pipe\{endpoint}")
}

/// Synchronously probe the named pipe (no tokio; used by `ghostlight doctor`, which runs with no
/// async runtime). Opens the pipe for read+write and immediately drops the handle -- no bytes are
/// written or read. Known, harmless side effect: probing a live *idle* server briefly wins the accept
/// slot, logging one phantom connect/disconnect pair in *that* server's own debug state. It never
/// disturbs an already-attached native-host: [`serve`] accepts ahead on a spare instance, so the
/// probe connects to the spare and [`crate::transport::executor::Browser::attach`] rejects it (AlreadyAttached)
/// and drops it without touching the live session.
#[cfg(windows)]
pub fn probe_endpoint(endpoint: &str) -> EndpointProbe {
    let path = pipe_path(endpoint);
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => {
            drop(file);
            EndpointProbe::Accepts
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => EndpointProbe::Absent,
        Err(e) if e.raw_os_error() == Some(231) => {
            EndpointProbe::Rejects("all pipe instances are busy".into())
        }
        Err(e) => EndpointProbe::Rejects(e.to_string()),
    }
}

/// Human-readable display of the endpoint's OS-level path (for `ghostlight doctor`'s report).
#[cfg(windows)]
pub fn endpoint_display(endpoint: &str) -> String {
    pipe_path(endpoint)
}

/// mcp-server role (Windows): own the named pipe (single active session) and serve native-host
/// connections. Each accepted connection is handed to [`Browser::attach`] until it closes.
#[cfg(windows)]
pub async fn serve(browser: Browser, endpoint: &str) -> Result<()> {
    let path = pipe_path(endpoint);
    let security = win_security::OwnerOnly::build();
    if security.is_none() {
        tracing::warn!("could not build an owner-only pipe DACL; falling back to the default DACL");
    }

    // First instance: `first_pipe_instance(true)` both enforces the single active session (creation
    // fails if a pipe of this name already exists) and prevents another local process from squatting
    // the name.
    let mut server =
        win_security::create_instance(&path, true, security.as_ref()).map_err(|e| {
            match e.raw_os_error() {
                // ACCESS_DENIED / PIPE_BUSY: a first instance already exists -> another session owns it.
                Some(5) | Some(231) => Error::SessionBusy,
                _ => Error::Ipc(format!("cannot create named pipe {path}: {e}")),
            }
        })?;
    tracing::info!(path, "mcp-server owns the named pipe (single session)");

    loop {
        server
            .connect()
            .await
            .map_err(|e| Error::Ipc(format!("named-pipe accept failed: {e}")))?;
        // Pre-create the NEXT instance before handling this one, so there is no window in which the
        // pipe name does not exist (a client connecting then would get NotFound).
        let next = win_security::create_instance(&path, false, security.as_ref())
            .map_err(|e| Error::Ipc(format!("cannot create next pipe instance: {e}")))?;
        let connected = std::mem::replace(&mut server, next);
        tracing::info!("native-host connected");
        // Accept-ahead: hand this connection to a spawned task and loop back immediately, so a spare
        // pipe instance is always waiting in ConnectNamedPipe. `Browser::attach` enforces the single
        // active session -- the real native-host claims the slot; a stray connection (e.g. a `doctor`
        // probe) is accepted here, rejected by attach as AlreadyAttached, and dropped without
        // disturbing the live session. Awaiting attach inline (the old behavior) parked the loop for
        // the whole session, leaving one consumable spare a probe could starve (ERROR_PIPE_BUSY 231
        // on every later probe until the session ended).
        let browser = browser.clone();
        tokio::spawn(async move {
            match browser.attach(connected).await {
                AttachOutcome::Detached => tracing::info!("native-host disconnected"),
                AttachOutcome::AlreadyAttached => {
                    tracing::debug!("dropped a stray connection; a session is already attached")
                }
            }
        });
    }
}

/// native-host role (Windows): open the mcp-server named pipe, retrying for ~30s so startup ordering
/// does not matter (the pipe may not exist yet, or all instances may be momentarily busy).
#[cfg(windows)]
pub async fn connect(endpoint: &str) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let path = pipe_path(endpoint);
    for _ in 0..60u32 {
        match ClientOptions::new().open(&path) {
            Ok(client) => return Ok(client),
            // PIPE_BUSY: all instances busy right now. NotFound: not created yet. Both -> retry.
            Err(e) if e.raw_os_error() == Some(231) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::Ipc(format!("cannot open named pipe {path}: {e}"))),
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err(Error::Ipc(
        "timed out connecting to the mcp-server named pipe".into(),
    ))
}

#[cfg(windows)]
mod win_security {
    //! Owner-only DACL for the named pipe, so only the creating user (and SYSTEM) can connect --
    //! closing the default-DACL gap where other local principals could reach the browser-control
    //! endpoint. Built once from an SDDL string and reused for every pipe instance.

    use std::ffi::c_void;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;

    const SDDL_REVISION_1: u32 = 1;

    /// Holds the OS-allocated security descriptor for the pipe. Intentionally leaked (one per
    /// process; `serve` lives for the whole process) to avoid a LocalFree dependency.
    pub struct OwnerOnly {
        descriptor: *mut c_void,
    }

    // Safety: `descriptor` is a process-stable, read-only OS security descriptor; it is only ever
    // passed by value to the OS at pipe creation, so moving/sharing the pointer across threads (as
    // the async `serve` future may) is sound.
    unsafe impl Send for OwnerOnly {}
    unsafe impl Sync for OwnerOnly {}

    impl OwnerOnly {
        /// Build a protected DACL granting generic-all to the object Owner (`OW`) and Local System
        /// (`SY`) only. Returns `None` if the descriptor cannot be constructed (caller falls back to
        /// the default DACL).
        pub fn build() -> Option<Self> {
            let sddl: Vec<u16> = "D:P(A;;GA;;;OW)(A;;GA;;;SY)"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut descriptor: *mut c_void = std::ptr::null_mut();
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || descriptor.is_null() {
                None
            } else {
                Some(Self { descriptor })
            }
        }

        fn attributes(&self) -> SECURITY_ATTRIBUTES {
            SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: self.descriptor,
                bInheritHandle: 0,
            }
        }
    }

    /// Create one named-pipe server instance, with the owner-only DACL when available.
    pub fn create_instance(
        path: &str,
        first: bool,
        security: Option<&OwnerOnly>,
    ) -> std::io::Result<NamedPipeServer> {
        let mut opts = ServerOptions::new();
        opts.first_pipe_instance(first);
        match security {
            Some(sec) => {
                let mut attrs = sec.attributes();
                // Safety: `attrs` outlives the call; its descriptor pointer is valid for the process.
                unsafe {
                    opts.create_with_security_attributes_raw(
                        path,
                        &mut attrs as *mut SECURITY_ATTRIBUTES as *mut c_void,
                    )
                }
            }
            None => opts.create(path),
        }
    }
}

// --- Unix: domain sockets ---

/// The Unix socket path: a user-owned `<runtime-or-cache-dir>/ghostlight/<endpoint>.sock`. The
/// parent dir is created 0700 and the socket 0600, so only the current user can reach it (unlike the
/// abstract namespace, which carries no filesystem permissions).
#[cfg(unix)]
fn socket_path(endpoint: &str) -> Result<std::path::PathBuf> {
    let base = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or_else(|| Error::Ipc("no user runtime/cache directory for the socket".into()))?;
    Ok(base.join("ghostlight").join(format!("{endpoint}.sock")))
}

/// Synchronously probe the Unix domain socket (no tokio; used by `ghostlight doctor`, which runs
/// with no async runtime). Connects and immediately drops the stream -- no bytes are written or
/// read. Known, harmless side effect: probing a live *idle* server briefly wins the accept slot,
/// logging one phantom connect/disconnect pair in *that* server's own debug state. It never disturbs
/// an already-attached native-host: [`serve`] spawns a handler per accepted connection and
/// [`crate::transport::executor::Browser::attach`] rejects a stray (AlreadyAttached), dropping it without
/// touching the live session.
#[cfg(unix)]
pub fn probe_endpoint(endpoint: &str) -> EndpointProbe {
    let path = match socket_path(endpoint) {
        Ok(p) => p,
        Err(e) => return EndpointProbe::Rejects(e.to_string()),
    };
    if !path.exists() {
        return EndpointProbe::Absent;
    }
    match std::os::unix::net::UnixStream::connect(&path) {
        Ok(stream) => {
            drop(stream);
            EndpointProbe::Accepts
        }
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            EndpointProbe::Rejects("socket file exists but nothing is listening (stale)".into())
        }
        Err(e) => EndpointProbe::Rejects(e.to_string()),
    }
}

/// Human-readable display of the endpoint's OS-level path (for `ghostlight doctor`'s report), or
/// `(unresolvable: <error>)` when the socket path itself cannot be computed.
#[cfg(unix)]
pub fn endpoint_display(endpoint: &str) -> String {
    match socket_path(endpoint) {
        Ok(p) => p.display().to_string(),
        Err(e) => format!("(unresolvable: {e})"),
    }
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

/// mcp-server role (Unix): bind the socket (single active session) and serve native-host connections.
#[cfg(unix)]
pub async fn serve(browser: Browser, endpoint: &str) -> Result<()> {
    use tokio::net::{UnixListener, UnixStream};
    let path = socket_path(endpoint)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Ipc(format!("cannot create socket dir: {e}")))?;
        set_mode(parent, 0o700);
    }

    // Single session: bind; if the path is in use, a successful probe-connect means a live owner
    // (SessionBusy), otherwise the socket file is stale -- remove it and rebind.
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).await.is_ok() {
                return Err(Error::SessionBusy);
            }
            std::fs::remove_file(&path).ok();
            UnixListener::bind(&path)
                .map_err(|e| Error::Ipc(format!("cannot bind socket {}: {e}", path.display())))?
        }
        Err(e) => {
            return Err(Error::Ipc(format!(
                "cannot bind socket {}: {e}",
                path.display()
            )))
        }
    };
    set_mode(&path, 0o600);
    tracing::info!(path = %path.display(), "mcp-server owns the unix socket (single session)");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tracing::info!("native-host connected");
                // Accept-ahead (mirrors the Windows path): spawn the handler and keep accepting, so a
                // queued stray connection (e.g. a `doctor` probe) is not promoted into a session when
                // the loop resumes. `Browser::attach` enforces the single active session.
                let browser = browser.clone();
                tokio::spawn(async move {
                    match browser.attach(stream).await {
                        AttachOutcome::Detached => tracing::info!("native-host disconnected"),
                        AttachOutcome::AlreadyAttached => {
                            tracing::debug!(
                                "dropped a stray connection; a session is already attached"
                            )
                        }
                    }
                });
            }
            Err(e) => tracing::warn!(error = %e, "socket accept failed"),
        }
    }
}

/// native-host role (Unix): connect to the mcp-server socket, retrying for ~30s.
#[cfg(unix)]
pub async fn connect(endpoint: &str) -> Result<tokio::net::UnixStream> {
    use tokio::net::UnixStream;
    let path = socket_path(endpoint)?;
    for _ in 0..60u32 {
        if let Ok(stream) = UnixStream::connect(&path).await {
            return Ok(stream);
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err(Error::Ipc(
        "timed out connecting to the mcp-server unix socket".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[tokio::test]
    async fn serve_bridges_a_tool_call_over_the_real_ipc() {
        let endpoint = "ghostlight-test-serve-bridge";
        let browser = Browser::new();
        let serving = browser.clone();
        tokio::spawn(async move {
            let _ = serve(serving, endpoint).await;
        });

        // Fake native-host: connect (retrying until serve is listening) and answer one request.
        let stream = connect(endpoint).await.expect("connect to serve");
        let (mut rd, mut wr) = tokio::io::split(stream);
        let fake = tokio::spawn(async move {
            let req = host::read_message(&mut rd).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            let reply = json!({ "id": v["id"], "type": "tool_response", "result": { "echoed": v["tool"] } });
            host::write_message(&mut wr, &serde_json::to_vec(&reply).unwrap())
                .await
                .unwrap();
        });

        for _ in 0..200 {
            if browser.is_connected() {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
        let result = browser
            .call("navigate", &json!({}))
            .await
            .expect("tool call round-trips over the real IPC");
        assert_eq!(result["echoed"], "navigate");
        fake.await.unwrap();
    }

    #[test]
    fn probe_reports_absent_for_an_unused_endpoint() {
        let endpoint = format!("ghostlight-test-probe-absent-{}", std::process::id());
        assert_eq!(probe_endpoint(&endpoint), EndpointProbe::Absent);
    }

    #[tokio::test]
    async fn probe_reports_accepts_against_a_live_server() {
        let endpoint = format!("ghostlight-test-probe-accepts-{}", std::process::id());
        let browser = Browser::new();
        let serving_endpoint = endpoint.clone();
        tokio::spawn(async move {
            let _ = serve(browser, &serving_endpoint).await;
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let probe_endpoint_name = endpoint.clone();
            let outcome = tokio::task::spawn_blocking(move || probe_endpoint(&probe_endpoint_name))
                .await
                .unwrap();
            if outcome == EndpointProbe::Accepts {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!("probe never reported Accepts against a live server: {outcome:?}");
            }
            sleep(Duration::from_millis(20)).await;
        }
    }
}
