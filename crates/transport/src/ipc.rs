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
//! This crate holds the ADAPTER half of the IPC (ADR-0046): the endpoint-name derivation, the
//! dialing/probe helpers, the native-host relay, and the resilient adapter relay. The SERVICE half
//! (the endpoint owners `serve`/`claim_adapter_endpoint`/`serve_adapters`) lives in ghostlight-core.

use crate::host;
use crate::{Error, Result};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

/// Reconnect retry window (ADR-0045 amendment, 2026-07-08): a RECONNECT episode (an established
/// session whose service dropped) retries for up to this long -- covering a rebuild-length dev gap
/// (Ctrl-C -> cargo build -> rerun) and a crash/upgrade in production -- before the adapter exits
/// and the client-reload path becomes the fallback. Deliberately far wider than the first-connect
/// `supervisor::SELF_HEAL_RETRY_WINDOW`, which stays fail-fast.
pub const RECONNECT_RETRY_WINDOW: Duration = Duration::from_secs(120);

/// Reconnect retry interval (ADR-0045 amendment): how often a reconnect episode re-dials within
/// [`RECONNECT_RETRY_WINDOW`].
pub const RECONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// The endpoint name both roles use, in precedence order: the explicit `GHOSTLIGHT_ENDPOINT`
/// override (tests and advanced deployments), else the active instance's endpoint (ADR-0044:
/// `org.sylin.ghostlight.v1` for the default instance, `org.sylin.ghostlight.<n>.v1` for a named
/// one). Each platform derives the real path from it: `\\.\pipe\<name>` on Windows,
/// `<runtime-dir>/ghostlight/<name>.sock` on Unix.
pub fn default_endpoint() -> String {
    std::env::var("GHOSTLIGHT_ENDPOINT")
        .unwrap_or_else(|_| crate::instance::Instance::resolve().endpoint())
}

/// The ordered MAIN-endpoint candidates an adapter dials (ADR-0048 D2/D3), pure core: the
/// single-endpoint override wins, then the list override, then the selection's instances. Split
/// from [`endpoint_candidates`] so it is unit-testable without racing parallel tests over
/// process-global env state.
fn candidates_from(
    single: Option<&str>,
    list: Option<&str>,
    selection: &crate::instance::Selection,
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
    selection
        .candidates()
        .iter()
        .map(crate::instance::Instance::endpoint)
        .collect()
}

/// The ordered endpoint candidates for `selection` (ADR-0048 D2/D3): `GHOSTLIGHT_ENDPOINT` (one
/// pinned endpoint; tests and advanced deployments) wins, then `GHOSTLIGHT_ENDPOINTS` (a
/// comma-separated pinned candidate LIST -- the override integration tests' seam), then the
/// selection's instances' endpoints (`[dev, default]` when unpinned, exactly one when pinned).
pub fn endpoint_candidates(selection: &crate::instance::Selection) -> Vec<String> {
    candidates_from(
        std::env::var("GHOSTLIGHT_ENDPOINT").ok().as_deref(),
        std::env::var("GHOSTLIGHT_ENDPOINTS").ok().as_deref(),
        selection,
    )
}

/// The ADAPTER/CONTROL endpoint's name (ADR-0030 Decision 1; PINS.md SS1): the extension
/// endpoint's base name with the literal suffix `-adapter`, then wrapped by the SAME
/// `pipe_path`/`socket_path` helper the extension endpoint uses -- so a test-unique
/// `GHOSTLIGHT_ENDPOINT` automatically makes BOTH endpoints unique.
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
    for ep in endpoints {
        if probe(ep) != EndpointProbe::Absent {
            return ep.clone();
        }
    }
    endpoints.last().cloned().unwrap_or_default()
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
///
/// ADR-0048 D4: `endpoints` is the ordered candidate list; the first candidate whose endpoint
/// exists is dialed (a fresh pick happens naturally per connect episode, because Chrome respawns
/// this process on every native-messaging reconnect).
///
/// ADR-0058: `hello` is this browser-role relay's session-hello frame (`ROLE_BROWSER`, carrying
/// this relay's own pid and its parent browser's [`crate::proc::ProcId`]), written once,
/// immediately after `connect()` succeeds and before the generic byte-relay loop starts -- the
/// SAME "peer speaks first" shape the adapter/control endpoint already uses, now also on this
/// endpoint (PINS.md SS1's "no hello" applied only while the extension was assumed a singleton).
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
pub async fn relay_native_host_over<I, O>(
    endpoints: &[String],
    hello: &[u8],
    debug: &crate::observability::DebugSink,
    mut chrome_in: I,
    mut chrome_out: O,
) -> Result<()>
where
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite + Unpin,
{
    let endpoint = pick_native_host_endpoint(endpoints, probe_endpoint);
    let stream = connect(&endpoint).await?;
    debug.ipc_note("connected to mcp-server endpoint");
    let (mut ipc_read, mut ipc_write) = tokio::io::split(stream);
    host::write_message(&mut ipc_write, hello).await?;

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

/// Which side of the adapter relay closed (ADR-0045): the classification that decides whether the
/// adapter EXITS (its MCP client is gone) or RECONNECTS to a restarted service.
enum RelaySide {
    /// The MCP client closed its stdio -> the adapter process should exit.
    ClientClosed,
    /// The SERVICE dropped (restart, crash, upgrade, idle-grace) -> reconnect and replay.
    ServiceClosed,
}

/// The captured MCP handshake preamble (ADR-0045 Decision 2): the client's `initialize` request
/// and its `notifications/initialized` notification, cached verbatim so they can be replayed to a
/// freshly restarted service, making a service restart invisible to the MCP client.
#[derive(Default)]
struct HandshakePreamble {
    initialize: Option<Vec<u8>>,
    initialized: Option<Vec<u8>>,
}

impl HandshakePreamble {
    /// True once both handshake messages are captured (so [`observe`](Self::observe) can stop).
    fn complete(&self) -> bool {
        self.initialize.is_some() && self.initialized.is_some()
    }

    /// Observe one complete client->service line during the first connection, caching it if it is
    /// the `initialize` request or the `initialized` notification. Everything after the handshake
    /// is ordinary application traffic and is never cached. A non-JSON or method-less line is
    /// ignored, never fatal.
    fn observe(&mut self, line: &[u8]) {
        if self.complete() {
            return;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            return;
        };
        match v.get("method").and_then(Value::as_str) {
            Some("initialize") if self.initialize.is_none() => {
                self.initialize = Some(line.to_vec());
            }
            Some("notifications/initialized") if self.initialized.is_none() => {
                self.initialized = Some(line.to_vec());
            }
            _ => {}
        }
    }

    /// Replay the captured handshake to a freshly connected service (ADR-0045 Decision 2): send
    /// `initialize`, read and DISCARD the service's `initialize` result (the client already has
    /// one from the first connection), then send `initialized`. The result is read byte-at-a-time
    /// ([`read_line_unbuffered`]) so no subsequent service->client bytes are swallowed into a
    /// throwaway buffer. A best-effort no-op if the handshake was never captured (a service that
    /// died mid-handshake): the reconnect then behaves no worse than today's client reload.
    async fn replay<R, W>(&self, ipc_read: &mut R, ipc_write: &mut W) -> Result<()>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;
        if let Some(init) = &self.initialize {
            ipc_write.write_all(init).await.map_err(Error::Io)?;
            ipc_write.flush().await.map_err(Error::Io)?;
            let _ = read_line_unbuffered(ipc_read).await.map_err(Error::Io)?;
        }
        if let Some(inited) = &self.initialized {
            ipc_write.write_all(inited).await.map_err(Error::Io)?;
            ipc_write.flush().await.map_err(Error::Io)?;
        }
        Ok(())
    }
}

/// Read exactly one newline-terminated line WITHOUT reading past it (unlike a `BufReader`, which
/// reads ahead and would swallow subsequent bytes). Used to discard the replayed `initialize`
/// result on reconnect. Returns the line including the trailing `\n`, or a short/empty line on EOF.
async fn read_line_unbuffered<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> std::io::Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if reader.read(&mut byte).await? == 0 {
            break; // EOF
        }
        line.push(byte[0]);
        if byte[0] == b'\n' {
            break;
        }
    }
    Ok(line)
}

/// Relay one service connection until one side closes (ADR-0045): forward queued client lines to
/// the service (capturing the handshake on the first connection) while streaming service replies
/// back to the client, and report WHICH side closed so the caller can exit or reconnect.
///
/// Cancellation safety is the crux. The client->service side reads COMPLETE lines from an mpsc
/// channel (`rx.recv()` is cancellation-safe: an un-received line stays queued for the next
/// reconnect, so a service drop never loses a queued request). The service->client side is a raw
/// `tokio::io::copy`, so large replies (screenshots) stream through unbuffered. The single request
/// that was mid-write to the service at the instant it dropped is lost and times out -- the
/// accepted baseline (ADR-0045 Decision 3); the client retries.
async fn relay_session<R, W, CO>(
    rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>,
    ipc_read: &mut R,
    ipc_write: &mut W,
    client_out: &mut CO,
    preamble: &mut HandshakePreamble,
    capture: bool,
) -> RelaySide
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
    CO: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncWriteExt;
    let up = async {
        loop {
            match rx.recv().await {
                None => break RelaySide::ClientClosed, // the stdin reader ended (client closed)
                Some(line) => {
                    if capture {
                        preamble.observe(&line);
                    }
                    if ipc_write.write_all(&line).await.is_err() || ipc_write.flush().await.is_err()
                    {
                        break RelaySide::ServiceClosed; // the service is gone
                    }
                }
            }
        }
    };
    let down = copy_service_to_client(ipc_read, client_out);
    tokio::select! {
        side = up => side,
        side = down => side,
    }
}

/// The service->client relay direction (ADR-0047 D6, amending ADR-0045): a manual copy loop so
/// the two failure sides classify differently. Reading 0 bytes OR a read error from the service
/// pipe is the SERVICE side ending (reconnect); only a failed write toward the client is the
/// CLIENT side ending (exit). The pre-0047 `tokio::io::copy` arm collapsed both error kinds into
/// ClientClosed, which on Windows (an abrupt service death often surfaces as ERROR_BROKEN_PIPE
/// on the read) exited the adapter and forced the client reload ADR-0045 exists to prevent.
async fn copy_service_to_client<R, W>(ipc_read: &mut R, client_out: &mut W) -> RelaySide
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut buf = [0u8; 8192];
    loop {
        match ipc_read.read(&mut buf).await {
            Ok(0) => return RelaySide::ServiceClosed, // service EOF
            Ok(n) => {
                if client_out.write_all(&buf[..n]).await.is_err()
                    || client_out.flush().await.is_err()
                {
                    return RelaySide::ClientClosed; // writing to the client failed
                }
            }
            Err(_) => return RelaySide::ServiceClosed, // service read error (e.g. broken pipe)
        }
    }
}

/// The thin ADAPTER role (ADR-0030 Decision 1 + Decision 8; ADR-0045): dial the SERVICE's
/// ADAPTER/CONTROL endpoint (self-healing the dial if the service is down, PINS.md SS5.2), send the
/// `adapter` session-hello, verify the SERVICE's anti-squat proof (PINS.md SS5.3), then relay
/// between this process's stdio and the service until one side closes.
///
/// RESILIENT (ADR-0045): unlike the pre-0045 adapter, which exited when the service stream closed
/// (forcing an MCP client reload on every service restart), this reconnects when the SERVICE drops
/// and replays the captured MCP handshake to the fresh service, so the client rides through a
/// rebuild/upgrade/crash transparently. It exits only when the CLIENT closes (or the parent-death
/// watchdog in `run_as_adapter` fires). The stdin reader lives in its OWN task feeding an mpsc
/// channel, so its `read_until` is never cancelled mid-line by the relay `select!` (which would
/// desync the JSON-RPC stream); queued client lines survive a reconnect.
///
/// The data phase is still newline-delimited JSON-RPC (never `host::write_message` framing) -- the
/// hello and proof are the only framed messages (PINS.md SS1 pin 3). One `SessionGuid` is minted
/// per adapter PROCESS and re-presented on every reconnect (ADR-0047 D2, superseding the pre-0047
/// fresh-guid-per-reconnect posture): the service's `SessionRegistry` sanctions the same user
/// re-presenting an identity, so tab ownership and this session's Chrome tab group survive the
/// service gap instead of being orphaned when the connection drops.
///
/// ADR-0048 D3: `endpoints` is the ORDERED main-endpoint candidate list (exactly one when
/// pinned; `[dev, default]` when unpinned). Every connect episode -- the first connect and each
/// reconnect tick -- walks the list in order, so a live dev instance shadows the default and a
/// dead one fails over to it at reconnect speed.
pub async fn relay_adapter(
    endpoints: &[String],
    debug: &crate::observability::DebugSink,
) -> Result<()> {
    // ADR-0051 Phase 2: the binary wires the process's REAL stdio; all the resilient reconnect +
    // handshake-replay logic lives in `relay_adapter_over`, which tests drive over injected in-memory
    // streams so none touches the real console stdin (whose missing EOF used to hang the relay tests).
    relay_adapter_over(endpoints, debug, tokio::io::stdin(), tokio::io::stdout()).await
}

/// [`relay_adapter`] with the client stdio INJECTED (ADR-0051 Phase 2). The binary passes real
/// `stdin`/`stdout`; tests pass in-memory streams (`tokio::io::duplex` / `empty` / `sink`) so the
/// reconnect + handshake-replay behavior is exercisable without the real console.
pub async fn relay_adapter_over<I, O>(
    endpoints: &[String],
    debug: &crate::observability::DebugSink,
    client_in: I,
    mut client_out: O,
) -> Result<()>
where
    I: tokio::io::AsyncRead + Unpin + Send + 'static,
    O: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    let adapter_endpoints: Vec<String> =
        endpoints.iter().map(|e| adapter_endpoint_name(e)).collect();

    // The long-lived stdin reader (ADR-0045): reads newline-delimited client lines and forwards
    // each as a complete line over the channel. It is NEVER inside a `select!`, so `read_until` is
    // never cancelled mid-line; lines buffer in the channel across a reconnect, so none are lost
    // while the service is briefly down. On stdin EOF it drops `tx`, so the relay loop sees the
    // client close.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(client_in);
        loop {
            let mut line = Vec::new();
            match reader.read_until(b'\n', &mut line).await {
                Ok(0) => break, // client closed stdin
                Ok(_) => {
                    if tx.send(line).await.is_err() {
                        break; // the relay loop is gone
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut preamble = HandshakePreamble::default();
    let mut first = true;

    // ADR-0047 D2: one identity for this adapter's whole life, re-presented on every reconnect so
    // the service-side ownership map and the extension's per-session groups survive a restart.
    let session_guid = crate::session_guid::SessionGuid::mint();
    // ADR-0051 P4.3b: records the structured `counters.identity_mints` AND the human event.
    debug.note_identity_minted();

    loop {
        // Connect AND handshake with a bounded retry (see [`connect_and_handshake`]): a service
        // that is mid-startup or mid-restart (endpoint claimed but not yet serving/proving) is
        // tolerated, not a fatal exit -- the crux that makes a reconnect actually resilient.
        let (stream, which) =
            connect_and_handshake(&adapter_endpoints, !first, &session_guid).await?;
        if first {
            debug.ipc_note("connected to the service's adapter/control endpoint");
        } else {
            // ADR-0051 P4.3b: records the structured `counters.reconnects` AND the human event.
            debug.note_reconnected();
        }
        if adapter_endpoints.len() > 1 {
            // ADR-0051 P4.3b: records the structured `counters.resolved_candidate`/`candidate_total`
            // AND the human event.
            debug.note_resolved_candidate((which + 1) as u32, adapter_endpoints.len() as u32);
        }

        let (mut ipc_read, mut ipc_write) = tokio::io::split(stream);

        // On a reconnect, make the fresh service believe this session already initialized, so the
        // client's subsequent (already-initialized) requests are accepted.
        if !first {
            preamble.replay(&mut ipc_read, &mut ipc_write).await?;
        }

        let outcome = relay_session(
            &mut rx,
            &mut ipc_read,
            &mut ipc_write,
            &mut client_out,
            &mut preamble,
            first,
        )
        .await;
        first = false;

        match outcome {
            RelaySide::ClientClosed => {
                debug.ipc_note("adapter relay ended (client closed)");
                return Ok(());
            }
            RelaySide::ServiceClosed => {
                debug.ipc_note("service dropped; reconnecting");
                // loop back and re-dial (self-healing the service start if needed).
            }
        }
    }
}

/// One connect + full session handshake attempt: dial the adapter/control endpoint, send the
/// `adapter` session-hello (the caller's stable per-process `SessionGuid`), and verify the
/// SERVICE's anti-squat proof (ADR-0030 Decision 8; PINS.md SS5.3). Returns the handshake-completed
/// stream, or an error
/// if ANY step fails (a down service, a torn-down connection mid-handshake, or a failed proof).
/// Grouping the handshake with the dial is what lets [`connect_and_handshake`] retry the WHOLE
/// thing, not just the dial.
async fn try_connect_once(
    adapter_endpoint: &str,
    guid: &crate::session_guid::SessionGuid,
) -> Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> {
    let mut stream = dial_once(adapter_endpoint).await?;
    let hello = adapter_hello(guid);
    let hello_bytes = serde_json::to_vec(&hello)
        .map_err(|e| Error::NativeMessaging(format!("failed to encode the adapter hello: {e}")))?;
    host::write_message(&mut stream, &hello_bytes).await?;
    verify_service_proof(&mut stream, &hello_bytes).await?;
    Ok(stream)
}

/// The adapter's session-hello JSON (ADR-0047 D2): built from the caller's stable per-process
/// `SessionGuid`. Same wire shape as before -- `{ hub, role, guid }` -- extracted so the guid
/// threading is unit-testable in isolation.
fn adapter_hello(guid: &crate::session_guid::SessionGuid) -> serde_json::Value {
    json!({
        "hub": crate::handshake::HUB_PROTO,
        "role": crate::handshake::ROLE_ADAPTER,
        "guid": guid.as_str(),
    })
}

/// A liveness snapshot returned by a [`crate::handshake::ROLE_CONTROL`] `status` request
/// (CAP-MED-01): the answer to "is the browser extension attached, and how many tool sessions are
/// live?" Non-sensitive by design -- it carries no session ids, identities, or tab details -- so it
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
    /// The number of live tool sessions (MCP adapters + web) at the moment of the reply.
    pub live_sessions: u64,
    /// Every currently-attached browser (ADR-0058), most-recently-focused first. Non-sensitive:
    /// a pid and a live tab count, nothing identifying beyond what the local OS process list
    /// already shows any same-user process.
    #[serde(default)]
    pub browsers: Vec<BrowserInfo>,
}

/// One attached browser, as reported by `ghostlight doctor` (ADR-0058). Deliberately does not
/// carry a tab count: the service has no live source for "how many tabs does this browser have"
/// without a synchronous round-trip doctor's one-shot control query does not make (that number is
/// the extension's own `chrome.tabs.query` state, never mirrored server-side today) -- a future
/// addition, not a gap in this one.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowserInfo {
    /// The browser (Chrome/Edge) process id -- the identity a session is keyed by.
    pub pid: u32,
    /// Whether this browser most recently reported window focus (the front of the focus chain).
    pub focused: bool,
}

/// Ask the running SERVICE for a control-plane liveness [`StatusReply`] (CAP-MED-01). Dials the
/// ADAPTER/CONTROL endpoint, sends a `control`/`status` hello, and reads the one framed reply.
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

/// The async half of [`query_status`]: one dial + framed request/reply on the ADAPTER/CONTROL
/// endpoint, reusing the same [`dial_once`] + `host` framing every adapter connection uses. No
/// session is admitted (no guid, no anti-squat proof): this is a stateless control request.
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

/// Connect to the SERVICE and complete the handshake, retrying the WHOLE attempt within a bounded
/// window (ADR-0030 Decision 8 amendment / self-heal, PINS.md SS5.2; extended for ADR-0045). On
/// the first failure, best-effort ask the OS supervisor to start the service
/// ([`crate::supervisor::start_service`]) exactly once, then retry every
/// `SELF_HEAL_RETRY_INTERVAL` for up to `SELF_HEAL_RETRY_WINDOW`.
///
/// The 0045 change: it retries the full connect+handshake, not just the dial. A service that is
/// mid-startup or mid-restart may have CLAIMED the endpoint (so the dial succeeds) while it is not
/// yet serving or its per-install hub-key is not yet written -- a transient failure (a torn-down
/// connection, os error 232, or a not-yet-verifiable proof). Retrying the whole handshake makes
/// that transient window survivable instead of a fatal adapter exit, which is exactly what makes a
/// cold-start (self-heal) connection and a reconnect actually resilient. It never accepts a bad
/// proof -- a genuine squatter simply keeps failing until the window elapses and the adapter exits.
///
/// Walks the ordered candidate list on every attempt (ADR-0048 D3) and returns the winning
/// candidate's index alongside the stream.
async fn connect_and_handshake(
    adapter_endpoints: &[String],
    reconnect: bool,
    guid: &crate::session_guid::SessionGuid,
) -> Result<(
    impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    usize,
)> {
    debug_assert!(!adapter_endpoints.is_empty());
    let mut last_err: Option<Error> = None;
    for (which, ep) in adapter_endpoints.iter().enumerate() {
        match try_connect_once(ep, guid).await {
            Ok(stream) => return Ok((stream, which)),
            Err(e) => last_err = Some(e),
        }
    }
    crate::supervisor::start_service();
    // Reconnect patience (ADR-0045 amendment): the FIRST connect stays fail-fast (3s) so a
    // misconfigured install errors quickly; a RECONNECT episode is patient (120s) so a
    // rebuild-length service gap or a prod crash/upgrade never forces a client reload.
    let (interval, window) = if reconnect {
        (RECONNECT_RETRY_INTERVAL, RECONNECT_RETRY_WINDOW)
    } else {
        (
            crate::supervisor::SELF_HEAL_RETRY_INTERVAL,
            crate::supervisor::SELF_HEAL_RETRY_WINDOW,
        )
    };
    let deadline = tokio::time::Instant::now() + window;
    loop {
        sleep(interval).await;
        for (which, ep) in adapter_endpoints.iter().enumerate() {
            match try_connect_once(ep, guid).await {
                Ok(stream) => return Ok((stream, which)),
                Err(e) => last_err = Some(e),
            }
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::error!("{}", crate::supervisor::SELF_HEAL_FAILURE_MESSAGE);
            return Err(last_err.expect("at least one candidate was tried"));
        }
    }
}

/// Read and verify the SERVICE's anti-squat proof (ADR-0030 Decision 8; PINS.md SS5.3), which
/// follows the adapter's own hello. Any failure -- a missing/unreadable local `hub-key`, an
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

/// A single, non-retrying dial attempt at the ADAPTER/CONTROL endpoint (ADR-0030 Decision 8;
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
    /// No pipe/socket of this name exists: no mcp-server currently owns the endpoint.
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

/// Synchronously probe the named pipe (no tokio; used by `ghostlight doctor`, which runs with no
/// async runtime). Opens the pipe for read+write and immediately drops the handle -- no bytes are
/// written or read. Known, harmless side effect: probing a live *idle* server briefly wins the accept
/// slot, logging one phantom connect/disconnect pair in *that* server's own debug state. It never
/// disturbs an already-attached native-host: `serve` accepts ahead on a spare instance, so the
/// probe connects to the spare and the browser executor rejects it (AlreadyAttached)
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

// --- Unix: domain sockets ---

/// A short, deterministic hash of an endpoint (16 hex chars = the first 8 bytes of its SHA-256),
/// used as a socket filename when the readable name would overflow the platform's socket-path
/// limit. Deterministic so every process (service, adapter, `doctor`) that resolves the same
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
/// `-adapter` control socket and the bare extension socket hash to different names).
#[cfg(unix)]
pub fn socket_path(endpoint: &str) -> Result<std::path::PathBuf> {
    let base = dirs::runtime_dir()
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
pub fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
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

    #[test]
    fn probe_reports_absent_for_an_unused_endpoint() {
        let endpoint = format!("ghostlight-test-probe-absent-{}", std::process::id());
        assert_eq!(probe_endpoint(&endpoint), EndpointProbe::Absent);
    }

    /// ADR-0048 D2: candidate precedence -- the single override, the list override, then the
    /// selection's instances (dev first when unpinned). Pure: no env access.
    #[test]
    fn candidates_from_honors_the_precedence_order() {
        use crate::instance::{Instance, Selection};
        let unpinned = Selection::Unpinned;
        assert_eq!(
            candidates_from(Some("ep-one"), Some("a,b"), &unpinned),
            vec!["ep-one".to_string()]
        );
        assert_eq!(
            candidates_from(None, Some(" a , b ,,"), &unpinned),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            candidates_from(None, None, &unpinned),
            vec![
                "org.sylin.ghostlight.dev.v1".to_string(),
                "org.sylin.ghostlight.v1".to_string()
            ]
        );
        let pinned = Selection::Pinned(Instance::from_name("qa").unwrap());
        assert_eq!(
            candidates_from(None, None, &pinned),
            vec!["org.sylin.ghostlight.qa.v1".to_string()]
        );
        // Blank overrides fall through rather than pinning an empty endpoint.
        assert_eq!(
            candidates_from(Some("  "), None, &pinned),
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
    fn preamble_captures_only_the_handshake() {
        let mut p = HandshakePreamble::default();
        assert!(!p.complete());
        // A pre-handshake application request is ignored.
        p.observe(br#"{"jsonrpc":"2.0","id":1,"method":"tools/call"}"#);
        assert!(p.initialize.is_none());
        // initialize is captured verbatim.
        let init = br#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#;
        p.observe(init);
        assert_eq!(p.initialize.as_deref(), Some(&init[..]));
        assert!(!p.complete());
        // Non-JSON is ignored, never fatal.
        p.observe(b"not json at all");
        // initialized completes the preamble.
        let inited = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        p.observe(inited);
        assert_eq!(p.initialized.as_deref(), Some(&inited[..]));
        assert!(p.complete());
        // Once complete, a second initialize never overwrites the captured one.
        p.observe(br#"{"jsonrpc":"2.0","id":9,"method":"initialize"}"#);
        assert_eq!(p.initialize.as_deref(), Some(&init[..]));
    }

    #[test]
    fn hello_carries_the_caller_guid() {
        // ADR-0047 D2 (PINS P3): the hello carries the CALLER's guid, the wire shape is unchanged
        // (`{hub, role, guid}`), and it is deterministic for a given guid (the SAME identity is
        // re-presented on every reconnect).
        let guid = crate::session_guid::SessionGuid::mint();
        let hello = adapter_hello(&guid);
        assert_eq!(hello["guid"], guid.as_str());
        assert_eq!(hello["role"], "adapter");
        assert_eq!(hello["hub"], 1);
        assert_eq!(adapter_hello(&guid), hello);
    }

    #[tokio::test]
    async fn read_line_unbuffered_reads_exactly_one_line_and_leaves_the_rest() {
        // Reading one line must NOT consume the following bytes (unlike a BufReader), so the
        // service->client raw copy that follows a reconnect replay is not corrupted.
        let mut reader: &[u8] = b"result\nLEFTOVER";
        let line = read_line_unbuffered(&mut reader).await.unwrap();
        assert_eq!(line, b"result\n".to_vec());
        assert_eq!(reader, &b"LEFTOVER"[..]);
        // A trailing line with no newline is returned on EOF; a further read yields empty.
        let tail = read_line_unbuffered(&mut reader).await.unwrap();
        assert_eq!(tail, b"LEFTOVER".to_vec());
        let eof = read_line_unbuffered(&mut reader).await.unwrap();
        assert!(eof.is_empty());
    }

    // ADR-0047 D6 (PINS P2): the service->client relay direction must classify a service-side EOF
    // OR read error as ServiceClosed (reconnect), and only a client-side write failure as
    // ClientClosed (exit).

    #[tokio::test]
    async fn down_eof_classifies_service_closed() {
        // duplex(64) -> (first, second); read from `first`, drop the ENTIRE `second` half so
        // `first` observes EOF (dropping only a split WriteHalf would leave the read pending).
        let (mut ipc_read, service_peer) = tokio::io::duplex(64);
        drop(service_peer);
        let mut client_out = tokio::io::sink();
        assert!(matches!(
            copy_service_to_client(&mut ipc_read, &mut client_out).await,
            RelaySide::ServiceClosed
        ));
    }

    #[tokio::test]
    async fn down_read_error_classifies_service_closed() {
        struct FailingReader;
        impl tokio::io::AsyncRead for FailingReader {
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
                _buf: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)))
            }
        }
        let mut ipc_read = FailingReader;
        let mut client_out = tokio::io::sink();
        assert!(matches!(
            copy_service_to_client(&mut ipc_read, &mut client_out).await,
            RelaySide::ServiceClosed
        ));
    }

    #[tokio::test]
    async fn down_client_write_error_classifies_client_closed() {
        struct FailingWriter;
        impl tokio::io::AsyncWrite for FailingWriter {
            fn poll_write(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
                _buf: &[u8],
            ) -> std::task::Poll<std::io::Result<usize>> {
                std::task::Poll::Ready(Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)))
            }
            fn poll_flush(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
            fn poll_shutdown(
                self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Ok(()))
            }
        }
        // Service side carries one pending byte; keep the write end alive so no EOF races the byte.
        use tokio::io::AsyncWriteExt;
        let (mut service_write, mut ipc_read) = tokio::io::duplex(64);
        service_write.write_all(b"x").await.unwrap();
        let mut client_out = FailingWriter;
        assert!(matches!(
            copy_service_to_client(&mut ipc_read, &mut client_out).await,
            RelaySide::ClientClosed
        ));
        drop(service_write);
    }
}
