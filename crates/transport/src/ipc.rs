// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Local IPC primitives shared by the persistent service, the MCP edge, control clients, and the
//! browser-only native-host relay.
//!
//! Transport: a **local socket** -- a Windows named pipe (`tokio::net::windows::named_pipe`) or a
//! Unix domain socket (`tokio::net::Unix*`), chosen at compile time. No localhost TCP, no network
//! dependency, and (unlike a TCP port) it can be access-controlled to the current user.
//!
//! We use **tokio-native** transports rather than the `interprocess` crate: interprocess's own async
//! Windows named-pipe layer does not reliably wake a pending read when the peer process dies (its
//! "linger pool" deliberately delays EOF), so a killed service left the native host as a zombie
//! that never observed the disconnect. tokio's NamedPipe/UnixStream are first-class mio/IOCP sources
//! whose reads surface `Ok(0)`/`BrokenPipe` promptly on peer death -- no application heartbeat.
//!
//! This crate owns endpoint-name derivation, dialing and probe helpers, the MCP edge's authenticated
//! connection, and the resilient browser relay. Service-side endpoint owners live in
//! `ghostlight-core`.

use crate::host;
use crate::{Error, Result};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

/// Reconnect retry window (ADR-0045 amendment, 2026-07-08): a RECONNECT episode (an established
/// browser-relay connection whose service dropped) retries for up to this long -- covering a
/// rebuild-length dev gap and a crash/upgrade in production. Deliberately far wider than the MCP
/// edge's first-connect `supervisor::SELF_HEAL_RETRY_WINDOW`, which stays fail-fast.
pub const RECONNECT_RETRY_WINDOW: Duration = Duration::from_secs(120);

/// Reconnect retry interval (ADR-0045 amendment): how often a reconnect episode re-dials within
/// [`RECONNECT_RETRY_WINDOW`].
pub const RECONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// The base endpoint name both shores use, in precedence order: the explicit `GHOSTLIGHT_ENDPOINT`
/// override (tests and advanced deployments), else the active instance's endpoint (ADR-0044:
/// `org.sylin.ghostlight.v1` for the default instance, `org.sylin.ghostlight.<n>.v1` for a named
/// one). Each platform derives the real path from it: `\\.\pipe\<name>` on Windows,
/// `<runtime-dir>/ghostlight/<name>.sock` on Unix.
pub fn default_endpoint() -> String {
    std::env::var("GHOSTLIGHT_ENDPOINT")
        .unwrap_or_else(|_| crate::instance::Instance::resolve().endpoint())
}

/// The MAIN-endpoint candidates a client dials, pure core: the single-endpoint override wins, then
/// the list override, then `instance`'s own ONE endpoint (ADR-0064: a client pins exactly one
/// instance -- no more `[dev, default]` shadow). Split from [`endpoint_candidates`] so it is
/// unit-testable without racing parallel tests over process-global env state. Still returns a `Vec`
/// because `GHOSTLIGHT_ENDPOINTS` (the override integration tests' seam) can name several.
fn candidates_from(
    single: Option<&str>,
    list: Option<&str>,
    instance: &crate::instance::Instance,
) -> Vec<String> {
    if let Some(ep) = single.map(str::trim).filter(|s| !s.is_empty()) {
        return vec![ep.to_string()];
    }
    if let Some(raw) = list {
        let eps: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if !eps.is_empty() {
            return eps;
        }
    }
    vec![instance.endpoint()]
}

/// The endpoint candidates for `instance` (ADR-0064): `GHOSTLIGHT_ENDPOINT` (one pinned endpoint;
/// tests and advanced deployments) wins, then `GHOSTLIGHT_ENDPOINTS` (a comma-separated candidate
/// LIST -- the override integration tests' seam), then the instance's own single endpoint.
pub fn endpoint_candidates(instance: &crate::instance::Instance) -> Vec<String> {
    candidates_from(
        std::env::var("GHOSTLIGHT_ENDPOINT").ok().as_deref(),
        std::env::var("GHOSTLIGHT_ENDPOINTS").ok().as_deref(),
        instance,
    )
}

/// The typed MCP-edge/control endpoint's name. The literal `-adapter` suffix is retained as an
/// internal wire identity for upgrade compatibility; it does not denote an agent-role relay.
/// The same `pipe_path`/`socket_path` helper wraps both endpoints, so a test-unique
/// `GHOSTLIGHT_ENDPOINT` makes both unique.
pub fn adapter_endpoint_name(endpoint: &str) -> String {
    format!("{endpoint}-adapter")
}

/// Pick the native-host connect target from ordered candidates (ADR-0048 D4): the first whose
/// endpoint EXISTS right now (probe != Absent -- a busy pipe is still a live service) wins; when
/// every candidate is absent, the LAST one (the default instance in the unpinned order), whose
/// `connect()` retry patience then covers a service that is still starting up. `probe` is
/// injected so this stays a pure, unit-testable decision.
fn pick_native_host_endpoint(
    endpoints: &[String],
    probe: impl Fn(&str) -> EndpointProbe,
) -> String {
    // ADR-0064/0065: the ordinary path has exactly one pinned endpoint. Return it directly: a
    // presence probe cannot affect the choice and creates a needless extra connection immediately
    // before the real browser hello. Under Windows named-pipe load that probe can race the next
    // accepted connection and make the identity frame appear on a stream whose hello was lost.
    if let [endpoint] = endpoints {
        return endpoint.clone();
    }
    for ep in endpoints {
        if probe(ep) != EndpointProbe::Absent {
            return ep.clone();
        }
    }
    endpoints.last().cloned().unwrap_or_default()
}

/// Browser-only native-host role: connect to the service's browser endpoint and relay frames
/// between Chrome native messaging (this process's stdin/stdout) and the service. Transport
/// agnostic: works over whichever local socket [`connect`] returns.
///
/// When the service dies, the tokio-native read on `ipc_read` returns (EOF/BrokenPipe) and the
/// relay reconnects while keeping Chrome's native port open. Only Chrome closing its input ends
/// the process. Do NOT add an `ipc_write.shutdown().await` on a dead Windows pipe: that write can
/// hang. Dropping the connection halves closes the handle synchronously before the next attempt.
///
/// `debug` is env-gated by the browser relay composition root: Chrome inherits its own environment
/// when it launches this process and never passes `--debug`, so a native-host debug snapshot only
/// exists when Chrome itself was started with `GHOSTLIGHT_DEBUG=1`. Its absence is normal.
///
/// ADR-0048 D4: `endpoints` is the ordered candidate list; the first candidate whose endpoint
/// exists is dialed. A fresh pick happens on every service reconnect while this one relay process
/// and Chrome native port stay alive.
///
/// ADR-0058: `hello` is this browser-role relay's opening frame (`ROLE_BROWSER`, carrying
/// this relay's own pid and its parent browser's [`crate::proc::ProcId`]), written once per service
/// connection immediately after `connect()` succeeds and before the generic byte-relay loop starts
/// -- the SAME "peer speaks first" shape the typed edge/control endpoint already uses, now also on
/// this endpoint (PINS.md SS1's "no hello" applied only while the extension was assumed a singleton).
pub async fn relay_native_host(
    endpoints: &[String],
    hello: &[u8],
    debug: &crate::observability::DebugSink,
) -> Result<()> {
    // ADR-0051 Phase 2: the binary wires Chrome's real stdio; the framed relay logic lives in
    // `relay_native_host_over`, injectable in-process for tests.
    relay_native_host_over(
        endpoints,
        hello,
        debug,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// [`relay_native_host`] with Chrome's stdio INJECTED (ADR-0051 Phase 2): the binary passes the
/// real `stdin`/`stdout`; tests pass in-memory streams.
///
/// ADR-0062: this is a browser-shore reconnect loop. A service
/// drop no longer ends the relay -- it reconnects to its one pinned instance's service (ADR-0064:
/// the relay targets exactly one endpoint) and replays the extension's cached opening identity
/// frame, keeping Chrome's native port alive the whole time. Only Chrome's stdin closing (the
/// browser is gone) ends it. The Chrome-frame reader lives in its own task feeding a channel, so a
/// frame is never cancelled mid-read and frames buffer across a brief reconnect instead of lost.
pub async fn relay_native_host_over<I, O>(
    endpoints: &[String],
    hello: &[u8],
    debug: &crate::observability::DebugSink,
    chrome_in: I,
    mut chrome_out: O,
) -> Result<()>
where
    I: tokio::io::AsyncRead + Unpin + Send + 'static,
    O: tokio::io::AsyncWrite + Unpin,
{
    // The long-lived Chrome->service frame reader (ADR-0062): reads complete native-messaging frames
    // and forwards each over the channel. NEVER inside a `select!`, so `read_message` is never
    // cancelled mid-frame; frames buffer in the channel across a reconnect, so a brief service gap
    // never loses one. Chrome's stdin EOF drops `tx`, so the relay loop sees the browser close.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    let reader_debug = debug.clone();
    tokio::spawn(async move {
        let mut chrome_in = chrome_in;
        while let Ok(Some(frame)) = host::read_message(&mut chrome_in).await {
            reader_debug.frame_in();
            if tx.send(frame).await.is_err() {
                break; // the relay loop is gone
            }
        }
    });

    // The extension's opening identity frame (ADR-0061), captured from the first channel frame and
    // replayed to every reconnected service (the extension does not re-send it -- its port stays up).
    let mut identity: Option<Vec<u8>> = None;
    let mut first = true;
    loop {
        let stream =
            connect_native_with_retry(endpoints, !first, hello, identity.as_deref()).await?;
        let (mut ipc_read, mut ipc_write) = tokio::io::split(stream);
        if first {
            debug.ipc_note("connected to mcp-server endpoint");
        } else {
            debug.note_reconnected();
        }

        let side = native_relay_session(
            &mut rx,
            &mut identity,
            &mut ipc_read,
            &mut ipc_write,
            &mut chrome_out,
            debug,
        )
        .await;
        first = false;
        match side {
            RelaySide::ClientClosed => {
                debug.ipc_note("native-host relay ended (browser closed)");
                return Ok(());
            }
            RelaySide::ServiceClosed => {
                debug.ipc_note("service dropped; reconnecting the native-host relay");
                // loop back and re-dial (re-resolving the endpoint, ADR-0048/0062).
            }
        }
    }
}

/// Connect the native-host relay to its pinned instance's service and finish the browser opening
/// frames (ADR-0062/0064). Dial, relay hello, and cached extension identity are one attempt: a stale
/// Windows pipe that accepts a dial but closes during identity replay is retried like any other
/// service-side drop. The FIRST attempt stays fail-fast; a RECONNECT is patient within
/// [`RECONNECT_RETRY_WINDOW`]. Unlike the MCP edge's typed connection, there is no anti-squat proof:
/// the browser endpoint's ACL is the transport boundary, and the hello carries no workspace handle.
async fn connect_native_with_retry(
    endpoints: &[String],
    reconnect: bool,
    hello: &[u8],
    identity: Option<&[u8]>,
) -> Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> {
    connect_native_with_retry_using(
        endpoints,
        reconnect,
        hello,
        identity,
        |endpoint| async move { connect(&endpoint).await },
    )
    .await
}

/// Retry body split from [`connect_native_with_retry`] so the stale-connection opening race has a
/// deterministic in-memory regression test. This is still one connection loop, not another relay
/// state machine: it returns only after the hello and optional identity are flushed to the stream.
async fn connect_native_with_retry_using<S, C, F>(
    endpoints: &[String],
    reconnect: bool,
    hello: &[u8],
    identity: Option<&[u8]>,
    mut connect_once: C,
) -> Result<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    C: FnMut(String) -> F,
    F: std::future::Future<Output = Result<S>>,
{
    let deadline = tokio::time::Instant::now() + RECONNECT_RETRY_WINDOW;
    loop {
        let endpoint = pick_native_host_endpoint(endpoints, probe_endpoint);
        let attempt = async {
            let mut stream = connect_once(endpoint).await?;
            host::write_message(&mut stream, hello).await?;
            if let Some(id) = identity {
                // Replay the cached identity immediately after the hello so the new service
                // re-admits the same browser UUID and slot. Borrow it: a failed attempt must leave
                // the identity intact for the next connection.
                host::write_message(&mut stream, id).await?;
            }
            Ok(stream)
        }
        .await;
        match attempt {
            Ok(stream) => return Ok(stream),
            Err(error) if !reconnect || tokio::time::Instant::now() >= deadline => {
                return Err(error);
            }
            Err(_) => sleep(RECONNECT_RETRY_INTERVAL).await,
        }
    }
}

/// Relay one service connection for the native-host role until one side closes (ADR-0062), and
/// report WHICH side so the caller exits or reconnects. The
/// Chrome->service direction reads complete frames from the channel (`rx.recv()` is cancellation-safe
/// -- an un-received frame stays queued for the next reconnect) and captures the FIRST frame as the
/// identity to replay; the service->Chrome direction classifies a read EOF/error as `ServiceClosed`
/// (reconnect) and only a Chrome write failure as `ClientClosed` (exit), the same Windows-broken-pipe
/// broken-pipe classification keeps the browser shore connected without replaying a browser
/// effect.
async fn native_relay_session<R, W, CO>(
    rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>,
    identity: &mut Option<Vec<u8>>,
    ipc_read: &mut R,
    ipc_write: &mut W,
    chrome_out: &mut CO,
    debug: &crate::observability::DebugSink,
) -> RelaySide
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
    CO: tokio::io::AsyncWrite + Unpin,
{
    let up = async {
        loop {
            match rx.recv().await {
                None => break RelaySide::ClientClosed, // the Chrome reader ended (browser closed)
                Some(frame) => {
                    if identity.is_none() {
                        // ADR-0061/0062: the extension's opening frame, cached (opaquely) for replay
                        // on every future reconnect. The relay still never parses it.
                        *identity = Some(frame.clone());
                    }
                    if host::write_message(ipc_write, &frame).await.is_err() {
                        break RelaySide::ServiceClosed; // the service is gone
                    }
                }
            }
        }
    };
    let down = async {
        loop {
            match host::read_message(ipc_read).await {
                Ok(Some(frame)) => {
                    if host::write_message(chrome_out, &frame).await.is_err() {
                        break RelaySide::ClientClosed; // writing to Chrome failed
                    }
                    debug.frame_out();
                }
                Ok(None) => break RelaySide::ServiceClosed, // service EOF
                Err(_) => break RelaySide::ServiceClosed,   // service read error (e.g. broken pipe)
            }
        }
    };
    tokio::select! {
        side = up => side,
        side = down => side,
    }
}

/// Which side of the browser relay closed: the classification that decides whether the relay
/// exits because Chrome is gone or reconnects to a restarted service.
enum RelaySide {
    /// Chrome closed its native-messaging stream, so the browser relay should exit.
    ClientClosed,
    /// The SERVICE dropped (restart, crash, upgrade, idle-grace) -> reconnect and replay.
    ServiceClosed,
}

fn mcp_edge_hello() -> serde_json::Value {
    json!({
        "hub": crate::handshake::HUB_PROTO,
        "role": crate::handshake::ROLE_MCP,
    })
}

async fn complete_mcp_edge_handshake<S>(mut stream: S) -> Result<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let hello_bytes = serde_json::to_vec(&mcp_edge_hello()).map_err(|error| {
        Error::NativeMessaging(format!("failed to encode MCP edge hello: {error}"))
    })?;
    host::write_message(&mut stream, &hello_bytes).await?;
    verify_service_proof(&mut stream, &hello_bytes).await?;
    Ok(stream)
}

/// A liveness snapshot returned by a [`crate::handshake::ROLE_CONTROL`] `status` request
/// (CAP-MED-01): the answer to "is the browser extension attached, and how many MCP-edge bridge streams are
/// live?" Non-sensitive by design -- it carries no workspace ids, identities, or tab details -- so it
/// is safe over the same-user-only control channel. `ghostlight doctor` renders it as the Extension
/// verdict without needing `--debug` instrumentation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusReply {
    /// The protocol major version the SERVICE answered with (always [`crate::handshake::HUB_PROTO`]).
    pub hub: u32,
    /// Whether a browser extension / native-host is currently attached to the service. Derived as
    /// `!browsers.is_empty()` (ADR-0058); kept as its own field for wire back-compat with an older
    /// `doctor` reading a newer service's reply mid-upgrade.
    pub extension_connected: bool,
    /// The number of live MCP edge bridge streams at the moment of the reply.
    pub live_sessions: u64,
    /// Every currently-attached browser (ADR-0058), most-recently-focused first. Non-sensitive:
    /// a server-assigned slot and a focus flag, nothing identifying about the user's machine.
    #[serde(default)]
    pub browsers: Vec<BrowserInfo>,
}

/// One attached browser, as reported by `ghostlight doctor` (ADR-0058, amended by ADR-0061).
/// Deliberately does not carry a tab count: the service has no live source for "how many tabs does
/// this browser have" without a synchronous round-trip doctor's one-shot control query does not make
/// (that number is the extension's own `chrome.tabs.query` state, never mirrored server-side today)
/// -- a future addition, not a gap in this one.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowserInfo {
    /// The service-assigned slot (ADR-0061): a small, stable, non-zero number a browser connection
    /// is keyed by,
    /// mapped from the extension's persistent browser UUID. Replaces the pre-0061 `pid` (which could
    /// degrade to a colliding 0). Also the high bits of every composite tab id this browser owns.
    pub slot: u32,
    /// Whether this browser most recently reported window focus (the front of the focus chain).
    pub focused: bool,
}

/// Ask the running SERVICE for a control-plane liveness [`StatusReply`] (CAP-MED-01). Dials the
/// typed edge/control endpoint, sends a `control`/`status` hello, and reads the one framed reply.
///
/// SYNCHRONOUS by design: `ghostlight doctor` is a one-shot, runtime-free CLI (like
/// [`probe_endpoint`]), so this drives a private current-thread runtime for the single round-trip
/// and hands back a plain value. Returns `None` -- never an error -- when the service is absent, too
/// old to answer the control role (it drops the connection), or does not reply within a short
/// timeout, so a caller degrades to "unknown" gracefully across service versions.
pub fn query_status(endpoint: &str) -> Option<StatusReply> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;
    rt.block_on(async {
        tokio::time::timeout(Duration::from_secs(2), query_status_over(endpoint))
            .await
            .ok()?
            .ok()
    })
}

/// The async half of [`query_status`]: one dial plus framed request/reply on the typed edge/control
/// endpoint, reusing the same [`dial_once`] and `host` framing as the MCP edge. No bridge or
/// workspace is admitted and no anti-squat proof is needed: this is a stateless control request.
async fn query_status_over(endpoint: &str) -> Result<StatusReply> {
    let mut stream = dial_once(&adapter_endpoint_name(endpoint)).await?;
    let hello = json!({
        "hub": crate::handshake::HUB_PROTO,
        "role": crate::handshake::ROLE_CONTROL,
        "request": crate::handshake::CONTROL_REQUEST_STATUS,
    });
    let hello_bytes = serde_json::to_vec(&hello)
        .map_err(|e| Error::NativeMessaging(format!("failed to encode the control hello: {e}")))?;
    host::write_message(&mut stream, &hello_bytes).await?;
    let reply = host::read_message(&mut stream).await?.ok_or_else(|| {
        Error::Ipc("the service closed the control connection with no reply".into())
    })?;
    serde_json::from_slice(&reply)
        .map_err(|e| Error::NativeMessaging(format!("malformed control status reply: {e}")))
}

/// Read and verify the SERVICE's anti-squat proof (ADR-0030 Decision 8; PINS.md SS5.3), which
/// follows the MCP edge's own hello. Any failure -- a missing/unreadable local `hub-key`, an
/// unreachable peer, a malformed frame, the wrong role, or a MAC mismatch -- collapses to the
/// SAME pinned refusal, so a squatter never learns which check caught it.
async fn verify_service_proof<S>(stream: &mut S, hello_bytes: &[u8]) -> Result<()>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let refuse = || Error::Ipc(crate::antisquat::REFUSAL_MESSAGE.to_string());
    let key = crate::antisquat::read_hub_key().map_err(|_| refuse())?;
    let proof_bytes = host::read_message(stream)
        .await
        .ok()
        .flatten()
        .ok_or_else(refuse)?;
    let proof: Value = serde_json::from_slice(&proof_bytes).map_err(|_| refuse())?;
    let verified = proof.get("role").and_then(Value::as_str)
        == Some(crate::handshake::ROLE_SERVICE_PROOF)
        && proof
            .get("mac")
            .and_then(Value::as_str)
            .map(|mac| crate::antisquat::verify_mac_hex(&key, hello_bytes, mac))
            .unwrap_or(false);
    if verified {
        Ok(())
    } else {
        tracing::error!("{}", crate::antisquat::REFUSAL_MESSAGE);
        Err(refuse())
    }
}

/// A single, non-retrying dial attempt at the typed edge/control endpoint (ADR-0030 Decision 8;
/// PINS.md SS5.2): unlike [`connect`] (which retries for ~30s so ordinary startup timing never
/// matters to the extension), this makes exactly ONE attempt so [`connect_and_handshake`] controls
/// its own bounded retry timing.
#[cfg(windows)]
async fn dial_once(endpoint: &str) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let path = pipe_path(endpoint);
    ClientOptions::new()
        .open(&path)
        .map_err(|e| Error::Ipc(format!("cannot open named pipe {path}: {e}")))
}

/// Unix variant of [`dial_once`] (see its doc above).
#[cfg(unix)]
async fn dial_once(endpoint: &str) -> Result<tokio::net::UnixStream> {
    use tokio::net::UnixStream;
    let path = socket_path(endpoint)?;
    UnixStream::connect(&path)
        .await
        .map_err(|e| Error::Ipc(format!("cannot connect to socket {}: {e}", path.display())))
}

/// Result of a one-shot, synchronous probe of the IPC endpoint (see [`probe_endpoint`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointProbe {
    /// No pipe/socket of this name exists: no service currently owns the endpoint.
    Absent,
    /// The endpoint exists and accepted a connection (opened and closed immediately).
    Accepts,
    /// The endpoint exists but the probe could not connect (detail explains why).
    Rejects(String),
}

// --- Windows: named pipes ---

#[cfg(windows)]
pub fn pipe_path(endpoint: &str) -> String {
    format!(r"\\.\pipe\{endpoint}")
}

/// The process id of the Windows process serving `endpoint`, or `None` when the named pipe cannot
/// be opened or its owner cannot be queried. The caller supplies the exact endpoint name (for the
/// service control plane, [`adapter_endpoint_name`] of the base endpoint). This is the trusted OS
/// ownership primitive the installer pairs with process-image verification before replacing an
/// installed service during an upgrade.
#[cfg(windows)]
pub fn named_pipe_server_process_id(endpoint: &str) -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;

    let path = pipe_path(endpoint);
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .ok()?;
    let mut pid = 0u32;
    // Safety: `file` owns a valid named-pipe client handle for this call's duration and `pid` is
    // valid writable stack storage. The handle is closed normally when `file` drops.
    let ok = unsafe { GetNamedPipeServerProcessId(file.as_raw_handle() as _, &mut pid) };
    (ok != 0 && pid != 0).then_some(pid)
}

/// Synchronously probe the named pipe (no tokio; used by `ghostlight doctor`, which runs with no
/// async runtime). Opens the pipe for read+write and immediately drops the handle -- no bytes are
/// written or read. Known, harmless side effect: probing a live *idle* server briefly wins the accept
/// slot, logging one phantom connect/disconnect pair in *that* server's own debug state. It never
/// disturbs an already-attached native-host: `serve` accepts ahead on a spare instance, so the
/// probe connects to the spare and the browser executor rejects it (AlreadyAttached)
/// and drops it without touching the live browser link.
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

/// Browser relay (Windows): open the service's browser named pipe, retrying for about 30 seconds so startup ordering
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

// --- Unix: domain sockets ---

/// A short, deterministic hash of an endpoint (16 hex chars = the first 8 bytes of its SHA-256),
/// used as a socket filename when the readable name would overflow the platform's socket-path
/// limit. Deterministic so every process (service, either shore, and `doctor`) that resolves the same
/// endpoint computes the same path.
#[cfg(unix)]
fn short_endpoint_hash(endpoint: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(endpoint.as_bytes());
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// The Unix socket path: a user-owned `<runtime-or-cache-dir>/ghostlight/<endpoint>.sock`. The
/// parent dir is created 0700 and the socket 0600, so only the current user can reach it (unlike the
/// abstract namespace, which carries no filesystem permissions).
///
/// A `sockaddr_un` caps the path at ~104 bytes including the NUL terminator (103 usable on macOS,
/// 107 on Linux); a long endpoint under a long base -- notably macOS, where `dirs::cache_dir` is
/// `~/Library/Caches` -- overflows it and `bind` fails with `ENAMETOOLONG`. The readable name is
/// kept whenever it fits (production endpoints are short); otherwise it falls back to a short
/// deterministic hash so the socket always binds. The hash keeps distinct endpoints distinct (the
/// `-adapter` typed edge/control socket and the bare browser socket hash to different names).
#[cfg(unix)]
pub fn socket_path(endpoint: &str) -> Result<std::path::PathBuf> {
    let base = crate::user_session::runtime_dir()
        .or_else(dirs::cache_dir)
        .ok_or_else(|| Error::Ipc("no user runtime/cache directory for the socket".into()))?;
    let dir = base.join("ghostlight");
    let readable = dir.join(format!("{endpoint}.sock"));
    // A conservative threshold under the smallest (macOS) usable limit, leaving margin for the NUL.
    const MAX_SOCKET_PATH: usize = 100;
    if readable.as_os_str().len() <= MAX_SOCKET_PATH {
        Ok(readable)
    } else {
        Ok(dir.join(format!("gl-{}.sock", short_endpoint_hash(endpoint))))
    }
}

/// Synchronously probe the Unix domain socket (no tokio; used by `ghostlight doctor`, which runs
/// with no async runtime). Connects and immediately drops the stream -- no bytes are written or
/// read. Known, harmless side effect: probing a live *idle* server briefly wins the accept slot,
/// logging one phantom connect/disconnect pair in *that* server's own debug state. It never disturbs
/// an already-attached native-host: `serve` spawns a handler per accepted connection and the
/// browser executor rejects a stray (AlreadyAttached), dropping it without
/// touching the live browser link.
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
pub fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

/// Browser relay (Unix): connect to the service's browser socket, retrying for about 30 seconds.
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

/// Connect an MCP edge to the owner-only service endpoint and verify the anti-squat proof.
#[cfg(windows)]
pub async fn connect_mcp_edge(
    endpoint: &str,
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    complete_mcp_edge_handshake(connect(endpoint).await?).await
}

/// Connect an MCP edge to the owner-only service endpoint and verify the anti-squat proof.
#[cfg(unix)]
pub async fn connect_mcp_edge(endpoint: &str) -> Result<tokio::net::UnixStream> {
    complete_mcp_edge_handshake(connect(endpoint).await?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_absent_for_an_unused_endpoint() {
        let endpoint = format!("ghostlight-test-probe-absent-{}", std::process::id());
        assert_eq!(probe_endpoint(&endpoint), EndpointProbe::Absent);
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "multi_thread")]
    async fn named_pipe_server_process_id_reports_the_exact_owner() {
        use tokio::net::windows::named_pipe::ServerOptions;

        let endpoint = format!(
            "ghostlight-test-owner-{}-{}",
            std::process::id(),
            crate::workspace_id::WorkspaceId::mint().as_str()
        );
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(pipe_path(&endpoint))
            .expect("create private named-pipe server");
        let lookup_endpoint = endpoint.clone();
        let lookup =
            tokio::task::spawn_blocking(move || named_pipe_server_process_id(&lookup_endpoint));
        server.connect().await.expect("accept owner-query client");
        assert_eq!(
            lookup.await.expect("owner-query task completed"),
            Some(std::process::id())
        );
    }

    /// ADR-0048 D2: candidate precedence -- the single override, the list override, then the
    /// selection's instances (dev first when unpinned). Pure: no env access.
    #[test]
    fn candidates_from_honors_the_precedence_order() {
        use crate::instance::Instance;
        let default = Instance::default();
        // Single-endpoint override wins over everything.
        assert_eq!(
            candidates_from(Some("ep-one"), Some("a,b"), &default),
            vec!["ep-one".to_string()]
        );
        // List override next (the GHOSTLIGHT_ENDPOINTS test seam; still multi-valued).
        assert_eq!(
            candidates_from(None, Some(" a , b ,,"), &default),
            vec!["a".to_string(), "b".to_string()]
        );
        // ADR-0064: no override -> the instance's OWN single endpoint (no [dev, default] shadow).
        assert_eq!(
            candidates_from(None, None, &default),
            vec!["org.sylin.ghostlight.v1".to_string()]
        );
        let qa = Instance::from_name("qa").unwrap();
        assert_eq!(
            candidates_from(None, None, &qa),
            vec!["org.sylin.ghostlight.qa.v1".to_string()]
        );
        // Blank overrides fall through rather than pinning an empty endpoint.
        assert_eq!(
            candidates_from(Some("  "), None, &qa),
            vec!["org.sylin.ghostlight.qa.v1".to_string()]
        );
    }

    /// ADR-0048 D4: the first PRESENT candidate wins; busy still counts as present.
    #[test]
    fn pick_native_host_endpoint_prefers_the_first_present_candidate() {
        let eps = vec!["dev-ep".to_string(), "default-ep".to_string()];
        let picked = pick_native_host_endpoint(&eps, |ep| {
            if ep == "dev-ep" {
                EndpointProbe::Accepts
            } else {
                EndpointProbe::Absent
            }
        });
        assert_eq!(picked, "dev-ep");
        let picked = pick_native_host_endpoint(&eps, |ep| {
            if ep == "dev-ep" {
                EndpointProbe::Rejects("busy".into())
            } else {
                EndpointProbe::Accepts
            }
        });
        assert_eq!(picked, "dev-ep");
    }

    /// ADR-0048 D4: all-absent falls to the LAST candidate (the default), preserving connect()'s
    /// startup patience toward the canonical target.
    #[test]
    fn pick_native_host_endpoint_falls_to_the_last_when_all_are_absent() {
        let eps = vec!["dev-ep".to_string(), "default-ep".to_string()];
        assert_eq!(
            pick_native_host_endpoint(&eps, |_| EndpointProbe::Absent),
            "default-ep"
        );
        let one = vec!["only-ep".to_string()];
        assert_eq!(
            pick_native_host_endpoint(&one, |_| EndpointProbe::Absent),
            "only-ep"
        );
    }

    #[test]
    fn pick_native_host_endpoint_does_not_probe_a_single_pinned_target() {
        let one = vec!["only-ep".to_string()];
        assert_eq!(
            pick_native_host_endpoint(&one, |_| panic!("a single target needs no probe")),
            "only-ep"
        );
    }

    /// ADR-0062: a Windows named pipe can accept a reconnect dial while its service is already
    /// closing. Failure to write the relay hello or cached identity is therefore part of the
    /// reconnect attempt, not a reason to end Chrome's native port.
    #[tokio::test]
    async fn reconnect_retries_when_opening_frames_hit_a_closed_service() {
        let endpoints = vec!["in-memory-browser-endpoint".to_string()];
        let hello = b"relay-hello";
        let identity = b"cached-extension-identity";
        let (closed_service, closed_relay) = tokio::io::duplex(1024);
        drop(closed_service);
        let (mut live_service, live_relay) = tokio::io::duplex(1024);
        let mut streams = std::collections::VecDeque::from([closed_relay, live_relay]);
        let mut attempts = 0usize;

        let relay =
            connect_native_with_retry_using(&endpoints, true, hello, Some(identity), |endpoint| {
                assert_eq!(endpoint, endpoints[0]);
                attempts += 1;
                std::future::ready(
                    streams
                        .pop_front()
                        .ok_or_else(|| Error::Ipc("unexpected extra reconnect attempt".into())),
                )
            })
            .await
            .expect("the live second stream completes the opening replay");

        assert_eq!(attempts, 2, "the failed opening must be retried once");
        assert_eq!(
            host::read_message(&mut live_service)
                .await
                .unwrap()
                .unwrap(),
            hello
        );
        assert_eq!(
            host::read_message(&mut live_service)
                .await
                .unwrap()
                .unwrap(),
            identity
        );
        drop(relay);
    }

    /// ADR-0062: the native-host relay connection captures the extension's opening frame as the
    /// identity to replay, forwards it to the service, and classifies a service drop as
    /// ServiceClosed (reconnect) -- the resilience that keeps Chrome's port alive across a restart.
    #[tokio::test]
    async fn native_relay_captures_identity_and_reports_service_close() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        let (service_peer, relay_stream) = tokio::io::duplex(64 * 1024);
        let (mut ipc_read, mut ipc_write) = tokio::io::split(relay_stream);
        let mut identity: Option<Vec<u8>> = None;
        let mut chrome_out = tokio::io::sink();
        let debug = crate::observability::DebugSink::disabled();

        // The extension's opening (identity) frame arrives; the service reads it, then drops.
        tx.send(b"identity-frame".to_vec()).await.unwrap();
        let service = tokio::spawn(async move {
            let mut service_peer = service_peer;
            let got = host::read_message(&mut service_peer)
                .await
                .unwrap()
                .unwrap();
            drop(service_peer); // the service closes -> the relay's `down` read observes EOF
            got
        });

        let side = native_relay_session(
            &mut rx,
            &mut identity,
            &mut ipc_read,
            &mut ipc_write,
            &mut chrome_out,
            &debug,
        )
        .await;

        assert!(matches!(side, RelaySide::ServiceClosed));
        assert_eq!(
            identity.as_deref(),
            Some(&b"identity-frame"[..]),
            "the opening frame is cached for replay on reconnect"
        );
        assert_eq!(service.await.unwrap(), b"identity-frame");
    }

    /// ADR-0062: when Chrome's frame reader ends (the browser is gone), the connection reports
    /// ClientClosed (exit) -- never a reconnect -- and no identity is captured.
    #[tokio::test]
    async fn native_relay_reports_client_close_when_chrome_reader_ends() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(8);
        drop(tx); // the Chrome-frame reader ended (browser closed)
        let (_service_peer, relay_stream) = tokio::io::duplex(1024);
        let (mut ipc_read, mut ipc_write) = tokio::io::split(relay_stream);
        let mut identity: Option<Vec<u8>> = None;
        let mut chrome_out = tokio::io::sink();

        let side = native_relay_session(
            &mut rx,
            &mut identity,
            &mut ipc_read,
            &mut ipc_write,
            &mut chrome_out,
            &crate::observability::DebugSink::disabled(),
        )
        .await;

        assert!(matches!(side, RelaySide::ClientClosed));
        assert!(identity.is_none());
    }
}
