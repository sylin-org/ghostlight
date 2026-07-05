// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Inter-instance IPC between the persistent SERVICE, its ADAPTER peers, and the extension's
//! native-host relay.
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
//! TWO owner-only local endpoints (ADR-0030 Decision 1, the 2026-07-04 two-endpoint amendment;
//! PINS.md SS1): a peer's role is the endpoint it arrives at, never a discriminator byte on a
//! shared door.
//!
//! - The EXTENSION endpoint (mirrors the reference's proven ordering; [`default_endpoint`]) --
//!   UNCHANGED: server-speaks-first, no hello, ever. The SERVICE owns it for its whole life via
//!   [`serve`] (single active session -- a second claim refuses with [`Error::SessionBusy`], the
//!   single physical-extension-link invariant, unrelated to the election below); the **native-host**
//!   instance (launched by Chrome, short-lived, may relaunch on service-worker wake) [`connect`]s
//!   with retry and relays frames between the extension and the service ([`relay_native_host`]),
//!   sending nothing first, exactly as before this endpoint split existed.
//! - The ADAPTER/CONTROL endpoint (`<endpoint>-adapter`) -- the single-instance ELECTION target:
//!   [`claim_adapter_endpoint`] wins or loses ([`Error::SessionBusy`] on a loss now means "the
//!   singleton SERVICE is already up; connect to it as an ADAPTER instead", not "tools
//!   unavailable"). [`serve_adapters`] accepts speak-first sessions over the ALREADY-claimed
//!   listener and demuxes each connection's session-hello ([`handle_adapter_connection`]) into the
//!   SAME governance chokepoint every transport calls. [`relay_adapter`] is the thin ADAPTER's
//!   mirror of [`relay_native_host`] on this endpoint: it sends the hello, then raw-relays its
//!   stdio, never re-framing the data phase.

use crate::transport::executor::{AttachOutcome, Browser};
use crate::transport::native::host;
use crate::{Error, Result};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

/// Default endpoint base name; override with `GHOSTLIGHT_ENDPOINT` (used by tests and advanced
/// deployments that run more than one isolated instance on a host). Each platform derives the real
/// path from it: `\\.\pipe\<name>` on Windows, `<runtime-dir>/ghostlight/<name>.sock` on Unix.
const DEFAULT_ENDPOINT: &str = "org.sylin.ghostlight.v1";

/// The endpoint name both roles use: the `GHOSTLIGHT_ENDPOINT` env override, else the default.
pub fn default_endpoint() -> String {
    std::env::var("GHOSTLIGHT_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

/// The ADAPTER/CONTROL endpoint's name (ADR-0030 Decision 1; PINS.md SS1): the extension
/// endpoint's base name with the literal suffix `-adapter`, then wrapped by the SAME
/// `pipe_path`/`socket_path` helper the extension endpoint uses -- so a test-unique
/// `GHOSTLIGHT_ENDPOINT` automatically makes BOTH endpoints unique.
fn adapter_endpoint_name(endpoint: &str) -> String {
    format!("{endpoint}-adapter")
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

/// The thin ADAPTER role (ADR-0030 Decision 1): dial the SERVICE's ADAPTER/CONTROL endpoint,
/// send the `adapter` session-hello as ONE 4-byte-LE FRAMED message, then become a RAW
/// bidirectional byte relay between this process's stdio and the service stream until either
/// side closes.
///
/// CRITICAL (PINS.md SS1 pin 3): the session-hello is the ONLY framed message on this wire. The
/// DATA phase that follows is a RAW `tokio::io::copy` -- NEVER a `host::write_message`/
/// `read_message` framed copy -- because everything after the hello is newline-delimited
/// JSON-RPC, exactly what `serve_session`'s `BufReader::lines()` expects on the service side and
/// what an MCP client writes on this side. Mirrors [`relay_native_host`] ONLY in lifecycle shape
/// (the `select!` that exits on either side closing; deliberately no post-`select!`
/// `shutdown().await`, which can hang forever on an already-dead Windows pipe): it NEVER frames
/// the data phase the way `relay_native_host` does (that wire is framed end-to-end because it IS
/// the Chrome native-messaging wire; this wire is framed for the hello only, then raw).
///
/// The GUID member (H3, ADR-0030 Decision 4): minted ONCE here, as a local variable, before the
/// hello is built. `relay_adapter` runs exactly once per adapter process (called once from
/// `run_as_adapter`, never in a loop), so this already satisfies "same adapter process reuses its
/// GUID; a new adapter process mints a new one" with no `OnceLock` or extra plumbing needed.
pub async fn relay_adapter(endpoint: &str, debug: &crate::debug::DebugSink) -> Result<()> {
    let adapter_endpoint = adapter_endpoint_name(endpoint);
    let mut stream = connect(&adapter_endpoint).await?;
    debug.ipc_note("connected to the service's adapter/control endpoint");

    let guid = crate::hub::session::SessionGuid::mint();
    let hello = json!({
        "hub": crate::hub::handshake::HUB_PROTO,
        "role": crate::hub::handshake::ROLE_ADAPTER,
        "guid": guid.as_str(),
    });
    let hello_bytes = serde_json::to_vec(&hello)
        .map_err(|e| Error::NativeMessaging(format!("failed to encode the adapter hello: {e}")))?;
    host::write_message(&mut stream, &hello_bytes).await?;

    let (mut ipc_read, mut ipc_write) = tokio::io::split(stream);
    let mut client_in = tokio::io::stdin();
    let mut client_out = tokio::io::stdout();

    tokio::select! {
        _ = tokio::io::copy(&mut client_in, &mut ipc_write) => {}
        _ = tokio::io::copy(&mut ipc_read, &mut client_out) => {}
    }
    debug.ipc_note("adapter relay ended");
    Ok(())
}

/// Demux one ADAPTER/CONTROL connection (ADR-0030 Decision 1; PINS.md SS1, SS9): read the
/// session-hello FIRST (safe here -- unlike the extension endpoint, this peer always speaks
/// first), parse and admit its presented GUID (H3, ADR-0030 Decision 4), and route `"adapter"`
/// into the SAME governance chokepoint every transport calls
/// (`transport::mcp::server::serve_session`), never a second dispatch path. `"control"` is
/// reserved until H8; an unknown or absent role, a malformed/empty guid, or a guid refused by
/// [`crate::hub::session::SessionRegistry::admit`] are all refused cleanly, never a panic, and
/// never surface the presented GUID in a log. Runs entirely INSIDE the spawned per-connection
/// task (never inline in the accept loop), so a silent peer cannot head-of-line-block admission
/// of other adapters (ADR-0030 Decision 3). `peer_cred` is captured by the CONCRETE-platform
/// caller in [`serve_adapters`] (before the stream is erased to generic `S`) and threaded in as a
/// plain parameter -- this function itself never touches a raw OS handle.
async fn handle_adapter_connection<S>(
    ctx: crate::hub::ServiceContext,
    mut stream: S,
    peer_cred: crate::hub::session::PeerCred,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let hello_bytes = match host::read_message(&mut stream).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            tracing::debug!("adapter/control connection closed before sending a session-hello");
            return;
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to read the adapter/control session-hello");
            return;
        }
    };
    let hello: Value = match serde_json::from_slice(&hello_bytes) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "malformed adapter/control session-hello; refusing");
            return;
        }
    };
    match hello.get("role").and_then(Value::as_str) {
        Some(role) if role == crate::hub::handshake::ROLE_ADAPTER => {
            let presented_guid = hello.get("guid").and_then(Value::as_str).unwrap_or("");
            let guid = match crate::hub::session::SessionGuid::parse(presented_guid) {
                Some(guid) => guid,
                None => {
                    tracing::warn!(
                        "adapter session-hello carried a malformed or empty guid; refusing"
                    );
                    return;
                }
            };
            let admission = ctx
                .session_registry
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .admit(&guid, &peer_cred);
            match admission {
                crate::hub::session::Admission::Admitted => {
                    if let Err(e) =
                        crate::transport::mcp::server::serve_session(stream, ctx, guid).await
                    {
                        tracing::warn!(error = %e, "adapter session ended with an error");
                    }
                }
                crate::hub::session::Admission::Refused => {
                    tracing::warn!(
                        "adapter/control connection presented a guid already bound to a \
                         different peer; refusing"
                    );
                }
            }
        }
        Some(role) if role == crate::hub::handshake::ROLE_CONTROL => {
            tracing::debug!("the control role is reserved until H8; refusing the connection");
        }
        other => {
            tracing::warn!(
                role = ?other,
                "adapter/control connection sent an unknown or absent role; refusing"
            );
        }
    }
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

/// The ADAPTER/CONTROL endpoint's platform listener handle (ADR-0030 Decision 1; PINS.md SS1
/// pin 1): cfg-split like the rest of this module -- there is no unified `Listener` type.
#[cfg(windows)]
pub type AdapterListener = tokio::net::windows::named_pipe::NamedPipeServer;

/// Claim the ADAPTER/CONTROL endpoint (Windows): the single-instance ELECTION target (ADR-0030
/// Decision 1, Decision 8; PINS.md SS1 pin 1). Performs the SAME bind-with-stale-heal [`serve`]
/// does today (`first_pipe_instance(true)`; ACCESS_DENIED / PIPE_BUSY -> [`Error::SessionBusy`])
/// and returns the claimed, not-yet-connected first pipe instance on a win. The caller must NOT
/// re-claim the name (a second claim here self-deadlocks) -- pass the returned listener straight
/// to [`serve_adapters`].
#[cfg(windows)]
pub async fn claim_adapter_endpoint(endpoint: &str) -> Result<AdapterListener> {
    let path = pipe_path(&adapter_endpoint_name(endpoint));
    let security = win_security::OwnerOnly::build();
    if security.is_none() {
        tracing::warn!(
            "could not build an owner-only pipe DACL for the adapter/control endpoint; falling \
             back to the default DACL"
        );
    }

    let server =
        win_security::create_instance(&path, true, security.as_ref()).map_err(|e| {
            match e.raw_os_error() {
                Some(5) | Some(231) => Error::SessionBusy,
                _ => Error::Ipc(format!(
                    "cannot create adapter/control named pipe {path}: {e}"
                )),
            }
        })?;
    tracing::info!(
        path,
        "service owns the adapter/control named pipe (single instance)"
    );
    Ok(server)
}

/// Accept loop for the ADAPTER/CONTROL endpoint (Windows), over the ALREADY-claimed listener
/// (never re-claims the name). Accept-ahead + spawn-per-connection, exactly like [`serve`]; the
/// session-hello is read and demuxed INSIDE the spawned task ([`handle_adapter_connection`]),
/// never inline, so a silent peer cannot head-of-line-block admission of other adapters
/// (ADR-0030 Decision 3). Re-derives the pipe path from [`default_endpoint`] (the same
/// process-wide endpoint [`claim_adapter_endpoint`] was called with) rather than taking it as a
/// parameter, matching the two-argument `serve_adapters(ctx, listener)` shape.
#[cfg(windows)]
pub async fn serve_adapters(
    ctx: crate::hub::ServiceContext,
    mut server: AdapterListener,
) -> Result<()> {
    let path = pipe_path(&adapter_endpoint_name(&default_endpoint()));
    let security = win_security::OwnerOnly::build();

    loop {
        server
            .connect()
            .await
            .map_err(|e| Error::Ipc(format!("adapter/control pipe accept failed: {e}")))?;
        // Capture the peer's OS credential on the CONCRETE, still-connected pipe instance (H3,
        // PINS.md SS9), before it is replaced by the next spare instance below and moved into the
        // spawned task (where it is erased to generic `S` and can no longer yield a raw handle).
        let peer_cred = capture_peer_cred(&server);
        let next = win_security::create_instance(&path, false, security.as_ref()).map_err(|e| {
            Error::Ipc(format!(
                "cannot create next adapter/control pipe instance: {e}"
            ))
        })?;
        let connected = std::mem::replace(&mut server, next);
        tracing::info!("adapter/control peer connected");
        match peer_cred {
            Ok(peer_cred) => {
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    handle_adapter_connection(ctx, connected, peer_cred).await;
                });
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "could not capture the adapter/control peer's OS credential; refusing"
                );
                drop(connected);
            }
        }
    }
}

/// Capture the connecting peer's OS credential (Windows; ADR-0030 Decision 4 amendment; PINS.md
/// SS9): the pipe client's process id via `GetNamedPipeClientProcessId`, then that process's
/// token SID (as a string, via `OpenProcessToken` + `GetTokenInformation(TokenUser)` +
/// `ConvertSidToStringSidW`) as the OS-user principal admission compares. Called on the CONCRETE
/// `NamedPipeServer` handle, before it is erased to generic `S` -- [`handle_adapter_connection`]
/// itself never touches a raw OS handle.
#[cfg(windows)]
fn capture_peer_cred(pipe: &AdapterListener) -> Result<crate::hub::session::PeerCred> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// Closes the wrapped handle on every return path (including an early `?`).
    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    // SAFETY (this whole function): every Win32 call below is used exactly per its documented
    // contract (correct argument types/sizes, checked return codes, and every opened HANDLE is
    // wrapped in `OwnedHandle` so it is closed on every path, including an early `?` return).
    unsafe {
        let handle = pipe.as_raw_handle() as HANDLE;
        let mut pid: u32 = 0;
        if GetNamedPipeClientProcessId(handle, &mut pid) == 0 {
            return Err(Error::Ipc(format!(
                "cannot read the adapter/control peer's process id: {}",
                std::io::Error::last_os_error()
            )));
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return Err(Error::Ipc(format!(
                "cannot open the adapter/control peer process {pid}: {}",
                std::io::Error::last_os_error()
            )));
        }
        let process = OwnedHandle(process);

        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return Err(Error::Ipc(format!(
                "cannot open the adapter/control peer's process token: {}",
                std::io::Error::last_os_error()
            )));
        }
        let token = OwnedHandle(token);

        let mut needed: u32 = 0;
        // First call with a null buffer to learn the required size; ERROR_INSUFFICIENT_BUFFER is
        // expected here and not itself an error.
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        if needed == 0 {
            return Err(Error::Ipc(
                "GetTokenInformation reported zero size for TokenUser".into(),
            ));
        }
        let mut buf = vec![0u8; needed as usize];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            needed,
            &mut needed,
        ) == 0
        {
            return Err(Error::Ipc(format!(
                "cannot read the adapter/control peer's token user: {}",
                std::io::Error::last_os_error()
            )));
        }
        let token_user = &*(buf.as_ptr() as *const TOKEN_USER);

        let mut sid_str_ptr: windows_sys::core::PWSTR = std::ptr::null_mut();
        if ConvertSidToStringSidW(token_user.User.Sid, &mut sid_str_ptr) == 0
            || sid_str_ptr.is_null()
        {
            return Err(Error::Ipc(format!(
                "cannot convert the adapter/control peer's SID to a string: {}",
                std::io::Error::last_os_error()
            )));
        }
        let mut len = 0usize;
        while *sid_str_ptr.add(len) != 0 {
            len += 1;
        }
        let sid_string = String::from_utf16_lossy(std::slice::from_raw_parts(sid_str_ptr, len));
        LocalFree(sid_str_ptr as HLOCAL);

        Ok(crate::hub::session::PeerCred {
            user: crate::hub::session::PeerUser(sid_string),
            pid,
        })
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

/// The ADAPTER/CONTROL endpoint's platform listener handle (ADR-0030 Decision 1; PINS.md SS1
/// pin 1): cfg-split like the rest of this module -- there is no unified `Listener` type.
#[cfg(unix)]
pub type AdapterListener = tokio::net::UnixListener;

/// Claim the ADAPTER/CONTROL endpoint (Unix): the single-instance ELECTION target (ADR-0030
/// Decision 1, Decision 8; PINS.md SS1 pin 1). Performs the SAME bind-with-stale-heal [`serve`]
/// does today for the extension socket (on `AddrInUse`, probe-connect first: a live peer ->
/// [`Error::SessionBusy`], a dead socket -> remove and rebind) and returns the bound listener on
/// a win. The caller must NOT re-claim the name (a second bind here self-deadlocks: the process
/// would probe-connect to its own listener and read `SessionBusy`) -- pass the returned listener
/// straight to [`serve_adapters`].
#[cfg(unix)]
pub async fn claim_adapter_endpoint(endpoint: &str) -> Result<AdapterListener> {
    use tokio::net::{UnixListener, UnixStream};
    let path = socket_path(&adapter_endpoint_name(endpoint))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::Ipc(format!("cannot create socket dir: {e}")))?;
        set_mode(parent, 0o700);
    }

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).await.is_ok() {
                return Err(Error::SessionBusy);
            }
            std::fs::remove_file(&path).ok();
            UnixListener::bind(&path).map_err(|e| {
                Error::Ipc(format!(
                    "cannot bind adapter/control socket {}: {e}",
                    path.display()
                ))
            })?
        }
        Err(e) => {
            return Err(Error::Ipc(format!(
                "cannot bind adapter/control socket {}: {e}",
                path.display()
            )))
        }
    };
    set_mode(&path, 0o600);
    tracing::info!(
        path = %path.display(),
        "service owns the adapter/control unix socket (single instance)"
    );
    Ok(listener)
}

/// Accept loop for the ADAPTER/CONTROL endpoint (Unix), over the ALREADY-claimed listener. The
/// session-hello is read and demuxed INSIDE the spawned task ([`handle_adapter_connection`]),
/// never inline, so a silent peer cannot head-of-line-block admission of other adapters
/// (ADR-0030 Decision 3).
#[cfg(unix)]
pub async fn serve_adapters(
    ctx: crate::hub::ServiceContext,
    listener: AdapterListener,
) -> Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tracing::info!("adapter/control peer connected");
                // Capture the peer's OS credential on the CONCRETE, just-accepted `UnixStream`
                // (H3, PINS.md SS9), before it is moved into the spawned task (where it is erased
                // to generic `S` and can no longer yield a raw fd).
                match capture_peer_cred(&stream) {
                    Ok(peer_cred) => {
                        let ctx = ctx.clone();
                        tokio::spawn(async move {
                            handle_adapter_connection(ctx, stream, peer_cred).await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "could not capture the adapter/control peer's OS credential; refusing"
                        );
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "adapter/control socket accept failed"),
        }
    }
}

/// Capture the connecting peer's OS credential (Unix, non-macOS; ADR-0030 Decision 4 amendment;
/// PINS.md SS9): `SO_PEERCRED` on the accepted socket, yielding the peer's uid (the OS-user
/// principal admission compares) and pid (logging only).
#[cfg(all(unix, not(target_os = "macos")))]
fn capture_peer_cred(stream: &tokio::net::UnixStream) -> Result<crate::hub::session::PeerCred> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: `fd` is a live, borrowed socket fd for the duration of this call; `cred`/`len` are
    // valid, correctly-sized out-parameters for `SO_PEERCRED`.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut libc::ucred as *mut core::ffi::c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return Err(Error::Ipc(format!(
            "cannot read the adapter/control peer's credentials: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(crate::hub::session::PeerCred {
        user: crate::hub::session::PeerUser(cred.uid.to_string()),
        pid: cred.pid as u32,
    })
}

/// Capture the connecting peer's OS credential (macOS; ADR-0030 Decision 4 amendment; PINS.md
/// SS9): `getpeereid`, yielding the peer's uid (the OS-user principal admission compares).
/// `getpeereid` reports no pid; `pid: 0` here is logging-only and never compared by `admit`.
#[cfg(target_os = "macos")]
fn capture_peer_cred(stream: &tokio::net::UnixStream) -> Result<crate::hub::session::PeerCred> {
    use std::os::unix::io::AsRawFd;
    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: `fd` is a live, borrowed socket fd for the duration of this call; `uid`/`gid` are
    // valid out-parameters.
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc != 0 {
        return Err(Error::Ipc(format!(
            "cannot read the adapter/control peer's credentials: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(crate::hub::session::PeerCred {
        user: crate::hub::session::PeerUser(uid.to_string()),
        pid: 0,
    })
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
