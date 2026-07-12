// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The service-side endpoint owners (ADR-0030): serve, claim, serve_adapters; split from the old ipc module by ADR-0046.

use crate::hub::outbound::browser::{AttachOutcome, Browser};
use crate::{Error, Result};
use ghostlight_transport::host;
use serde_json::{json, Value};

use ghostlight_transport::ipc::*;

/// Send the SERVICE's anti-squat proof (ADR-0030 Decision 8; PINS.md SS5.3): the lowercase-hex
/// HMAC-SHA256 of the EXACT hello bytes just read, keyed by this install's `hub-key` (created
/// lazily at `hub::run_service` startup). Sent AFTER admitting the hello, BEFORE `serve_session`,
/// so a proof failure never reaches the governance chokepoint.
async fn send_service_proof<S>(stream: &mut S, hello_bytes: &[u8]) -> Result<()>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    let key = ghostlight_transport::antisquat::load_or_create_hub_key().map_err(Error::Io)?;
    let mac = ghostlight_transport::antisquat::compute_mac_hex(&key, hello_bytes);
    let proof = json!({
        "hub": ghostlight_transport::handshake::HUB_PROTO,
        "role": ghostlight_transport::handshake::ROLE_SERVICE_PROOF,
        "mac": mac,
    });
    let proof_bytes = serde_json::to_vec(&proof)
        .map_err(|e| Error::NativeMessaging(format!("failed to encode the service-proof: {e}")))?;
    host::write_message(stream, &proof_bytes).await
}

/// Demux one ADAPTER/CONTROL connection (ADR-0030 Decision 1; PINS.md SS1, SS9): read the
/// session-hello FIRST (safe here -- unlike the extension endpoint, this peer always speaks
/// first), parse and admit its presented GUID (H3, ADR-0030 Decision 4), and route `"adapter"`
/// into the SAME governance chokepoint every transport calls
/// (`transport::mcp::server::serve_session`), never a second dispatch path. `"control"` is a
/// stateless read-only request/reply ([`answer_control_request`], CAP-MED-01) admitting no session;
/// an unknown or absent role, a malformed/empty guid, or a guid refused by
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
        Some(role) if role == ghostlight_transport::handshake::ROLE_ADAPTER => {
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
            // H5 (ADR-0030 Decision 3 + Decision 4; PINS.md SS4): per-peer (never global) mint
            // quota, checked BEFORE admission proceeds. Held for the connection's whole lifetime
            // (including a Refused admission below) so the slot frees only once this connection
            // genuinely ends -- the cap counts CONCURRENT sessions, not lifetime mints.
            let _mint_guard = match crate::hub::try_mint(&ctx.mint_quota, &peer_cred.user) {
                Ok(guard) => guard,
                Err(message) => {
                    tracing::warn!(
                        message = %message,
                        "adapter/control connection refused: per-peer mint quota exceeded"
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
                    // Anti-squat (ADR-0030 Decision 8; PINS.md SS5.3): prove this service's
                    // identity to the adapter AFTER admitting the hello, BEFORE serve_session --
                    // a proof failure (e.g. the per-install hub-key could not be prepared) must
                    // never reach the governance chokepoint.
                    if let Err(e) = send_service_proof(&mut stream, &hello_bytes).await {
                        tracing::warn!(
                            error = %e,
                            "could not prove this service's identity to the connecting adapter; refusing"
                        );
                        return;
                    }
                    if let Err(e) = crate::mcp::server::serve_session(stream, ctx, guid).await {
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
        Some(role) if role == ghostlight_transport::handshake::ROLE_CONTROL => {
            let live_sessions = ctx.live_sessions.load(std::sync::atomic::Ordering::Relaxed) as u64;
            answer_control_request(
                &mut stream,
                &hello,
                ctx.browser.browser_snapshot(),
                live_sessions,
            )
            .await;
        }
        other => {
            tracing::warn!(
                role = ?other,
                "adapter/control connection sent an unknown or absent role; refusing"
            );
        }
    }
}

/// Answer a [`ghostlight_transport::handshake::ROLE_CONTROL`] request (CAP-MED-01): a stateless,
/// read-only reply with NO session admission (no guid, no mint quota, no anti-squat proof, no
/// `serve_session`). Today the only request is `status`, which returns a liveness
/// [`ghostlight_transport::ipc::StatusReply`] -- whether the extension is attached and how many tool
/// sessions are live -- so `ghostlight doctor` can render a real Extension verdict without
/// `--debug`. An unknown request is ignored (the connection just closes), keeping the control
/// vocabulary forward-compatible. Access is already bounded to the same OS user by the endpoint's
/// owner-only transport ACL, and the reply carries only non-sensitive liveness, so no per-request
/// credential check is needed here. Takes the two liveness values directly (not the whole
/// `ServiceContext`) so the reply shape is unit-testable over an in-memory duplex.
async fn answer_control_request<S>(
    stream: &mut S,
    hello: &Value,
    browsers: Vec<ghostlight_transport::ipc::BrowserInfo>,
    live_sessions: u64,
) where
    S: tokio::io::AsyncWrite + Unpin,
{
    let request = hello.get("request").and_then(Value::as_str).unwrap_or("");
    if request != ghostlight_transport::handshake::CONTROL_REQUEST_STATUS {
        tracing::debug!(
            request,
            "control connection sent an unknown request; ignoring"
        );
        return;
    }
    let reply = ghostlight_transport::ipc::StatusReply {
        hub: ghostlight_transport::handshake::HUB_PROTO,
        extension_connected: !browsers.is_empty(),
        live_sessions,
        browsers,
    };
    let bytes = match serde_json::to_vec(&reply) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "failed to encode the control status reply");
            return;
        }
    };
    if let Err(e) = host::write_message(stream, &bytes).await {
        tracing::debug!(error = %e, "control status reply could not be written (peer gone)");
    }
}

/// Fail-closed guard for the pipe DACL (SEC-LOW-09): a Ghostlight pipe MUST carry the owner-only
/// DACL that restricts it to this user and Local System. If the descriptor could not be built,
/// REFUSE to create the pipe rather than fall back to the broader default named-pipe DACL, which
/// would silently weaken the cross-user guarantee. Constant-SDDL construction failure is
/// essentially theoretical, so this never fires in practice -- but "silently less safe" is not an
/// acceptable failure mode for the boundary that keeps other local users off this session's pipe.
#[cfg(windows)]
fn require_owner_only(built: Option<win_security::OwnerOnly>) -> Result<win_security::OwnerOnly> {
    built.ok_or_else(|| {
        Error::Ipc(
            "refusing to create the IPC pipe: could not build the owner-only DACL (restricting it \
             to this user and Local System); failing closed rather than opening it with the \
             broader default DACL (SEC-LOW-09)"
                .to_string(),
        )
    })
}

/// mcp-server role (Windows): own the named pipe (single active session) and serve native-host
/// connections. Each accepted connection is handed to [`Browser::attach`] until it closes.
#[cfg(windows)]
pub async fn serve(browser: Browser, endpoint: &str) -> Result<()> {
    let path = pipe_path(endpoint);
    // Fail closed (SEC-LOW-09): the pipe MUST carry the owner-only DACL; never fall back to the
    // broader default DACL.
    let security = require_owner_only(win_security::OwnerOnly::build())?;

    // First instance: `first_pipe_instance(true)` both enforces the single active session (creation
    // fails if a pipe of this name already exists) and prevents another local process from squatting
    // the name.
    let mut server = win_security::create_instance(&path, true, Some(&security)).map_err(|e| {
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
        let next = win_security::create_instance(&path, false, Some(&security))
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
    // Fail closed (SEC-LOW-09): the adapter/control pipe MUST carry the owner-only DACL.
    let security = require_owner_only(win_security::OwnerOnly::build())?;

    let server =
        win_security::create_instance(&path, true, Some(&security)).map_err(|e| {
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
    // Fail closed (SEC-LOW-09): the adapter/control pipe MUST carry the owner-only DACL.
    let security = require_owner_only(win_security::OwnerOnly::build())?;

    loop {
        server
            .connect()
            .await
            .map_err(|e| Error::Ipc(format!("adapter/control pipe accept failed: {e}")))?;
        // Capture the peer's OS credential on the CONCRETE, still-connected pipe instance (H3,
        // PINS.md SS9), before it is replaced by the next spare instance below and moved into the
        // spawned task (where it is erased to generic `S` and can no longer yield a raw handle).
        let peer_cred = capture_peer_cred(&server);
        let next = win_security::create_instance(&path, false, Some(&security)).map_err(|e| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tokio::time::{sleep, Duration};

    /// SEC-LOW-09: a missing owner-only DACL must ABORT pipe creation (fail closed), not fall
    /// back to the broader default DACL. The guard maps `None` to an `Ipc` error whose message
    /// names the fail-closed decision so an operator can see why the pipe refused to bind.
    #[cfg(windows)]
    #[test]
    fn require_owner_only_fails_closed_on_a_missing_dacl() {
        match require_owner_only(None) {
            Err(Error::Ipc(msg)) => {
                assert!(msg.contains("owner-only DACL"), "message: {msg}");
                assert!(msg.contains("failing closed"), "message: {msg}");
            }
            Err(other) => panic!("expected an Ipc error, got a different error: {other:?}"),
            Ok(_) => panic!("expected a fail-closed error, got Ok"),
        }
    }

    /// The owner-only DACL is a constant SDDL, so on a real Windows host it always converts:
    /// `build()` returns `Some` and `require_owner_only` passes it through.
    #[cfg(windows)]
    #[test]
    fn require_owner_only_passes_a_built_dacl_through() {
        let built = win_security::OwnerOnly::build();
        assert!(built.is_some(), "constant SDDL must convert on Windows");
        assert!(require_owner_only(built).is_ok());
    }

    /// CAP-MED-01: a `control`/`status` request produces exactly one framed [`StatusReply`] whose
    /// fields echo the liveness values, over an in-memory duplex (no spawned service).
    #[tokio::test]
    async fn control_status_request_writes_a_status_reply() {
        let (mut server_side, mut client_side) = tokio::io::duplex(4096);
        let hello = json!({
            "hub": ghostlight_transport::handshake::HUB_PROTO,
            "role": ghostlight_transport::handshake::ROLE_CONTROL,
            "request": ghostlight_transport::handshake::CONTROL_REQUEST_STATUS,
        });
        let browsers = vec![ghostlight_transport::ipc::BrowserInfo {
            slot: 1,
            focused: true,
        }];
        answer_control_request(&mut server_side, &hello, browsers, 3).await;

        let bytes = host::read_message(&mut client_side)
            .await
            .unwrap()
            .expect("a framed reply");
        let reply: ghostlight_transport::ipc::StatusReply = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reply.hub, ghostlight_transport::handshake::HUB_PROTO);
        assert!(reply.extension_connected);
        assert_eq!(reply.live_sessions, 3);
        assert_eq!(reply.browsers.len(), 1);
        assert_eq!(reply.browsers[0].slot, 1);
    }

    /// An unrecognized control request writes nothing and simply closes -- keeping the vocabulary
    /// forward-compatible (a future request name never crashes an older service).
    #[tokio::test]
    async fn unknown_control_request_writes_nothing() {
        let (mut server_side, mut client_side) = tokio::io::duplex(4096);
        let hello = json!({ "role": ghostlight_transport::handshake::ROLE_CONTROL, "request": "future-thing" });
        answer_control_request(&mut server_side, &hello, Vec::new(), 0).await;
        drop(server_side); // close the write half so the read observes EOF, not a hang
        assert!(
            host::read_message(&mut client_side)
                .await
                .unwrap()
                .is_none(),
            "an unknown request must produce no reply"
        );
    }

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
        // ADR-0058/0061: the first frames are the relay hello then the extension identity frame.
        let hello = ghostlight_transport::handshake::browser_hello_bytes(1, None);
        host::write_message(&mut wr, &hello).await.unwrap();
        let identity = serde_json::to_vec(&serde_json::json!({
            "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
            ghostlight_transport::handshake::BROWSER_ID_FIELD: "endpoint-fixture",
        }))
        .unwrap();
        host::write_message(&mut wr, &identity).await.unwrap();
        let fake = tokio::spawn(async move {
            let req = host::read_message(&mut rd).await.unwrap().unwrap();
            let v: Value = serde_json::from_slice(&req).unwrap();
            assert_eq!(
                v["guid"], "test-guid",
                "the tool envelope carries the session guid"
            );
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
            .call("test-guid", "navigate", &json!({}))
            .await
            .expect("tool call round-trips over the real IPC");
        assert_eq!(result["echoed"], "navigate");
        fake.await.unwrap();
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
