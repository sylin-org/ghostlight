// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The Hub composition root (ADR-0030 Decision 2: "Extract the composition root into a
//! free-licensed `src/hub` module hosting `HubCore`").
//!
//! ADR-0030 Decision 8 (amended 2026-07-04, "the always-ready-service amendment"): role is decided
//! by ARGV, never a claim race. [`run_mcp_server`] is ALWAYS the thin ADAPTER (`ghostlight`, bare):
//! it connects to an already-running SERVICE, relays its stdio as a pure byte pipe, and dies with
//! its editor, asking the OS supervisor to self-heal-start the service if it is down
//! ([`run_as_adapter`], `supervisor::start_service`). [`run_service`] is the STANDALONE SERVICE
//! (`ghostlight service`): it owns the shared [`ServiceContext`], the extension endpoint, and the
//! adapter/control endpoint for its whole life, runs NO parent-death watchdog, and shuts down only
//! on a continuous idle-grace window ([`run_service_loop`]/`idle_grace_watch`). There is NO
//! promotion, NO in-process service, and NO on-demand in-editor spawn -- that mechanism (an
//! earlier H2/H6 draft) is DELETED, not built. The session-hello constants the ADAPTER/CONTROL
//! endpoint uses live in [`handshake`]; the OS-supervisor identifiers + self-heal in
//! [`supervisor`]; the per-install anti-squat secret + HMAC proof in [`antisquat`].
//!
//! ADR-0030 Decision 3 ("D1 -- the honest singleton queue"): the single MV3 service worker plus
//! the single native port is an ACCEPTED, TRUTHFUL serialization bottleneck -- fair ordering and
//! truthful failure on a real drop, never a hidden work-around. H5 lands the three properties
//! Decision 3 names: a bounded reconnect grace window (`hub::outbound::browser::Browser::attach`,
//! `GRACE_WINDOW`, strictly less than `TOOL_TIMEOUT`), a per-peer (never global) mint quota
//! (below, [`try_mint`]/[`PER_PEER_MINT_CAP`]), and mandatory oversize-reply chunking on the
//! service<->adapter/web hop (`transport::mcp::server::write_chunked`,
//! `SCREENSHOT_CHUNK_THRESHOLD`) so one session's large payload cannot head-of-line-block
//! another's small one. See `docs/adr/0004-reject-second-session.md`'s amendment note for the
//! cross-reference from the ORIGINAL single-session decision this multiplexes past.

use crate::browser::pattern;
use crate::governance::audit::Recorder;
use crate::governance::config::reload::ConfigStore;
use crate::governance::manifest::source;
use crate::governance::manifest::source::LoadedPolicy;
use crate::hub::outbound::browser::Browser;
use crate::native::ipc;
use crate::observability::DebugSink;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub mod antisquat;
pub mod handshake;
pub mod inbound;
pub mod manage;
pub mod outbound;
pub use ghostlight_transport::role;
pub mod session;
pub mod supervisor;

/// Idle-grace shutdown window (ADR-0030 Decision 8; PINNED, PINS.md SS5.4): the SERVICE exits only
/// after zero live sessions AND the extension link gone, CONTINUOUSLY, for this long. Never a
/// parent-death trigger -- the service has no client parent to watch.
pub const IDLE_GRACE: Duration = Duration::from_secs(30);

/// Idle-grace poll interval (author-pinned, PINS.md SS5.4; not itself an ADR-0030 value).
pub const IDLE_POLL: Duration = Duration::from_secs(1);

/// Per-peer (never global) mint quota (ADR-0030 Decision 3: "per-peer-identity mint/group
/// quotas (never a single global cap, which is itself a lockout DoS)"; Decision 4's "per-peer
/// rate-limit key" amendment). PINNED in `docs/tasks/hub/PINS.md` SS4: max CONCURRENT
/// adapter-minted [`session::SessionGuid`] sessions per minting peer identity.
pub const PER_PEER_MINT_CAP: usize = 32;

/// The paired per-peer live-tab-group cap (H7; PINNED in PINS.md SS4, equal to
/// [`PER_PEER_MINT_CAP`] by design -- "the paired ... equal by design"). Not yet consumed: H7
/// wires this in when it adds per-session tab groups.
pub const PER_PEER_GROUP_CAP: usize = 32;

/// The quota-exceeded result (PINNED in `docs/tasks/hub/PINS.md` SS4): a plain tool error, never
/// a governance denial-id -- this is a HUB admission decision, not a change to the 13+`explain`
/// tool surface.
pub const MINT_QUOTA_EXCEEDED: &str = "session limit reached for this client";

/// Shared per-peer mint-quota table (ADR-0030 Decision 3 + Decision 4): keyed on the peer's OS
/// credential ([`session::PeerUser`]), NEVER a single global counter. A `ServiceContext` field,
/// added the same way H3's `session_registry` and H4's `owned_tabs` were.
pub type MintQuota = Arc<Mutex<HashMap<session::PeerUser, usize>>>;

/// RAII handle for one minted, live slot against a peer's [`PER_PEER_MINT_CAP`]. Decrements the
/// SAME counter [`try_mint`] incremented when this drops (the connection/session ends), so the
/// cap counts CONCURRENT sessions, never lifetime mints.
#[must_use = "dropping the guard immediately frees the peer's mint-quota slot"]
pub struct MintGuard {
    quota: MintQuota,
    peer: session::PeerUser,
}

impl Drop for MintGuard {
    fn drop(&mut self) {
        let mut quota = self.quota.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = quota.get_mut(&self.peer) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                quota.remove(&self.peer);
            }
        }
    }
}

/// Check-and-increment `peer`'s live mint count against [`PER_PEER_MINT_CAP`] (ADR-0030
/// Decision 3: "per-peer-identity mint/group quotas"). `Ok` increments and returns a
/// [`MintGuard`] that frees the slot on drop; `Err` is the pinned [`MINT_QUOTA_EXCEEDED`] text,
/// with no state change -- a flooding peer is denied while every OTHER peer's own counter (and
/// thus its own admission) is completely unaffected (never a single global cap).
pub fn try_mint(
    quota: &MintQuota,
    peer: &session::PeerUser,
) -> std::result::Result<MintGuard, String> {
    let mut guard = quota.lock().unwrap_or_else(PoisonError::into_inner);
    let count = guard.entry(peer.clone()).or_insert(0);
    if *count >= PER_PEER_MINT_CAP {
        return Err(MINT_QUOTA_EXCEEDED.to_string());
    }
    *count += 1;
    drop(guard);
    Ok(MintGuard {
        quota: Arc::clone(quota),
        peer: peer.clone(),
    })
}

/// The thin ADAPTER entry point (ADR-0030 Decision 1; Decision 8 amendment; PINS.md SS5.1). Role
/// is decided by ARGV, never a claim race: a bare `ghostlight` invocation is ALWAYS the ADAPTER.
/// It NEVER claims the adapter/control endpoint, loads policy, builds a [`Browser`], or builds a
/// [`ServiceContext`] -- it only connects to an already-running SERVICE and relays. A `--manifest`
/// here is a client-side no-op (the running service's policy governs every session); this REPEALS
/// ADR-0004's degrade semantics at the MCP-client layer the other direction too: every MCP client,
/// not just the first, multiplexes through the one real service ([`run_as_adapter`]).
pub fn run_mcp_server(manifest: Option<String>, debug_on: bool) -> Result<()> {
    role::set_role(role::Role::Adapter);

    if manifest.is_some() || std::env::var_os("GHOSTLIGHT_MANIFEST").is_some() {
        tracing::warn!(
            "a --manifest on a client invocation is ignored; the running Ghostlight service's \
             policy governs all sessions"
        );
    }

    let sink = crate::observability::build_debug_sink(debug_on, "adapter");
    // Startup self-heal (ADR-0029 part 4; ADR-0030 Decision 8 re-scope, PINS.md SS5.5): reap any
    // orphaned predecessor ADAPTER whose editor exited but that did not terminate. The SERVICE has
    // no client parent and idle-graces instead (see `run_service_loop`), so it is never a reap
    // target. Best-effort and safe (only parent-dead orphans; see `doctor::reap`).
    crate::hub::manage::doctor::sweep_orphans();
    // The MCP client that spawned us, captured before the runtime starts (ADR-0029). None (no
    // resolvable parent) simply skips the watchdog below and leaves stdin EOF as the sole exit
    // trigger.
    let parent = crate::proc::parent();

    let rt = tokio::runtime::Runtime::new()?;
    let block_sink = sink.clone();
    let endpoint = ipc::default_endpoint();
    let code = rt.block_on(run_as_adapter(&endpoint, block_sink, parent));

    // The single ordered teardown. process::exit rather than unwinding: on a detector-triggered
    // shutdown the stdin read may still be parked in a blocking ReadFile, and dropping the runtime
    // would hang forever trying to join that thread (the same reason the native-host role exits
    // directly). Flush the final observability snapshot first.
    sink.flush();
    std::process::exit(code)
}

/// The standalone SERVICE entry point (ADR-0030 Decision 8 amendment; PINS.md SS5.1), run only
/// via the `ghostlight service` subcommand: loads policy (the ONLY role that does), then serves
/// forever until [`IDLE_GRACE`] elapses with no live sessions and the extension link gone. NEVER
/// captures a parent or runs the ADR-0029 watchdog -- that lifecycle belongs to the ADAPTER now.
pub fn run_service(manifest: Option<String>, debug_on: bool, keep_warm: bool) -> Result<()> {
    role::set_role(role::Role::Service);

    // Resolve the user-supplied manifest source (G12, shared format doc section 1.3): the
    // --manifest flag wins when both it and GHOSTLIGHT_MANIFEST are set. Plain synchronous
    // I/O, before the async runtime starts: a source that is SELECTED but cannot be read,
    // parsed, or validated is a fatal startup error (an org policy that fails open is worse
    // than a crash), so this must happen before a single JSON-RPC line is served.
    let user_source = manifest.or_else(|| std::env::var("GHOSTLIGHT_MANIFEST").ok());
    let loaded_policy = source::load_policy(user_source.as_deref(), pattern::is_valid_pattern)
        .with_context(|| "loading the governance manifest")?;

    match (&loaded_policy.manifest, &loaded_policy.origin) {
        (Some(m), Some(origin)) => tracing::info!(
            name = %m.name,
            version = %m.version,
            hash = %m.hash,
            mode = ?m.mode,
            origin = ?origin,
            debug_mode = debug_on,
            "ghostlight starting (service role; governance overlay active)"
        ),
        _ => tracing::info!(
            debug_mode = debug_on,
            "ghostlight starting (service role; no manifest: all-open)"
        ),
    }

    let sink = crate::observability::build_debug_sink(debug_on, "mcp-server");
    let rt = tokio::runtime::Runtime::new()?;
    let block_sink = sink.clone();
    let endpoint = ipc::default_endpoint();
    let code = rt.block_on(run_service_loop(
        endpoint,
        block_sink,
        loaded_policy,
        user_source,
        keep_warm,
    ));

    sink.flush();
    std::process::exit(code)
}

/// The async body of [`run_service`] (ADR-0030 Decision 1, Decision 2, Decision 8; PINS.md SS5.1):
/// claim the ADAPTER/CONTROL endpoint as a single-instance guard (never a role election -- role
/// was already decided by argv), then own both local endpoints for the rest of this process's
/// life, and finally run the [`IDLE_GRACE`] watcher as the returning future. NEVER serves this
/// process's own stdio as a session (Decision 8 amendment: a standalone service has no stdio
/// session of its own) and NEVER captures a parent or runs the ADR-0029 watchdog.
async fn run_service_loop(
    endpoint: String,
    debug_sink: DebugSink,
    loaded_policy: LoadedPolicy,
    user_source: Option<String>,
    keep_warm: bool,
) -> i32 {
    let adapter_listener = match ipc::claim_adapter_endpoint(&endpoint).await {
        Ok(listener) => listener,
        Err(crate::Error::SessionBusy) => {
            tracing::info!("a Ghostlight service is already running on this endpoint; exiting");
            return 0;
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to claim the adapter/control endpoint");
            return 1;
        }
    };

    // Anti-squat (ADR-0030 Decision 8; PINS.md SS5.3): prepare the per-install secret now, before
    // the adapter/control endpoint is actually served below, so no connection can ever race the
    // key file's first creation. Best-effort: a failure here degrades anti-squat protection for
    // this run rather than refusing browser automation entirely (defense-in-depth, not a hard
    // requirement -- Decision 8).
    if let Err(e) = antisquat::load_or_create_hub_key() {
        tracing::warn!(
            error = %e,
            "could not prepare the per-install hub-key; anti-squat proofs will fail until this is fixed"
        );
    }

    let browser = Browser::with_debug(debug_sink.clone());

    // The EXTENSION endpoint: UNCHANGED, server-speaks-first, no hello (ADR-0030 Decision 1;
    // PINS.md SS1).
    tokio::spawn({
        let browser = browser.clone();
        let ext_endpoint = endpoint.clone();
        async move {
            match ipc::serve(browser, &ext_endpoint).await {
                Ok(()) => {}
                Err(crate::Error::SessionBusy) => tracing::warn!(
                    "another ghostlight session already owns the browser; tool calls in this \
                     session will report the extension as unavailable"
                ),
                Err(e) => tracing::error!(error = %e, "browser IPC endpoint failed"),
            }
        }
    });

    // Build the SHARED ServiceContext ONCE (PINS.md SS1 pin 4); every multiplexed adapter session
    // `serve_adapters` spawns clones it.
    let ctx = match ServiceContext::from_startup(browser, debug_sink, loaded_policy, user_source) {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!(error = %e, "failed to build the shared service context");
            return 1;
        }
    };

    // The inbound transports (ADR-0034): each transport is a blackbox that owns its listener
    // lifecycle and feeds sessions into the pipeline. The pipe transport takes the
    // already-claimed adapter listener; the web transport binds its own TCP listener.
    let pipe = inbound::pipe::PipeTransport::new(adapter_listener);
    tokio::spawn(pipe.run(ctx.clone()));

    if inbound::web::enabled(&ctx.store) {
        tokio::spawn(inbound::web::run(ctx.clone()));
    } else {
        tracing::info!(
            "inbound.web transport disabled by policy (inbound.web.enabled = false); not binding"
        );
    }

    // Idle-grace shutdown (ADR-0030 Decision 8; PINS.md SS5.4): normally the ONLY shutdown trigger
    // (never parent-death -- this process has no client parent to watch). With --keep-warm
    // (ADR-0045), idle-grace is disabled so a terminal-run dev service stays up between actions
    // instead of idle-shutting from under the developer; it then exits only when killed.
    if keep_warm {
        tracing::info!(
            "--keep-warm: idle-grace shutdown disabled; the service stays up until it is killed"
        );
        drop(ctx);
        std::future::pending::<i32>().await
    } else {
        idle_grace_watch(ctx).await
    }
}

/// The idle-grace watcher (ADR-0030 Decision 8; PINS.md SS5.4, transcribed verbatim): the SERVICE
/// exits once zero live sessions AND the extension link gone hold CONTINUOUSLY for [`IDLE_GRACE`];
/// any session or a reconnected extension resets the counter to zero.
async fn idle_grace_watch(ctx: ServiceContext) -> i32 {
    let mut idle_for = Duration::ZERO;
    loop {
        tokio::time::sleep(IDLE_POLL).await;
        let idle = ctx.live_sessions.load(std::sync::atomic::Ordering::Relaxed) == 0
            && !ctx.browser.is_connected();
        idle_for = if idle {
            idle_for + IDLE_POLL
        } else {
            Duration::ZERO
        };
        if idle_for >= IDLE_GRACE {
            tracing::info!(idle_for = ?IDLE_GRACE, "idle-grace elapsed; the service is shutting down");
            return 0;
        }
    }
}

/// The thin ADAPTER role's async body (ADR-0030 Decision 1; Decision 8 amendment): connect to the
/// already-running SERVICE and relay this process's stdio to it -- a pure byte relay, never a
/// rewriter (ADR-0030 "Preserved invariants"). If the service is not reachable,
/// `ipc::relay_adapter` asks the OS supervisor to self-heal-start it (PINS.md SS5.2) before
/// retrying. Dies with its editor via the SAME ADR-0029 parent-death watchdog the persistent
/// service used to run (re-scoped here, PINS.md SS5.5): stdin EOF is still the ordinary exit
/// trigger; the watchdog is the second, reliable one for an unclean kill.
async fn run_as_adapter(
    endpoint: &str,
    debug_sink: DebugSink,
    parent: Option<crate::proc::ProcId>,
) -> i32 {
    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());
    if let Some(parent) = parent {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            crate::transport::watchdog::wait_until_orphaned(parent).await;
            tracing::warn!(
                parent_pid = parent.pid,
                "MCP client exited; ordering shutdown"
            );
            shutdown.notify_one();
        });
    }

    tokio::select! {
        result = ipc::relay_adapter(endpoint, &debug_sink) => {
            match result {
                Ok(()) => 0,
                Err(e) => {
                    tracing::error!(error = %e, "adapter relay ended with an error");
                    1
                }
            }
        }
        _ = shutdown.notified() => 0,
    }
}

/// SHARED per-service state (ADR-0030 Decision 2: "HubCore / ServiceContext vs per-session
/// state"): the one [`Browser`] handle, the [`ConfigStore`], the audit [`Recorder`], and (H3) the
/// GUID -> bound-peer [`session::SessionRegistry`] -- built ONCE at startup and handed to every
/// `transport::mcp::server::serve_session` invocation. PER-SESSION state (the swappable
/// `Governance`, the writer task, the policy-subscription task, and the `SessionGuid` itself) is
/// built PER session, inside `serve_session` itself, never here.
///
/// `Clone` (H2, PINS.md SS1 pin 4): built ONCE at service start, then cloned per session for
/// `serve_session` -- every field is a cheap `Arc` clone or an already-`Clone` value (`Browser`,
/// `LoadedPolicy`). Never call [`ServiceContext::from_startup`] per session: it spawns a
/// recorder-reload watcher task each time, so one per session would leak N duplicate watchers on
/// the one store.
#[derive(Clone)]
pub struct ServiceContext {
    pub browser: Browser,
    /// The capability registry (ADR-0034): the composition root's ordered list of outbound
    /// capability executors. Aggregates each capability's tool directory + agent guide into the
    /// single source consumed by `tools/list`, `explain`, enforcement, and the validator. Today
    /// only the browser capability is registered; the `browser` field above stays for the
    /// browser-specific dispatch paths (`call`, `tab_url`) the pipeline uses directly.
    pub capabilities: outbound::Registry,
    pub store: Arc<ConfigStore>,
    pub recorder: Arc<Recorder>,
    pub initial_policy: LoadedPolicy,
    pub session_registry: Arc<std::sync::Mutex<session::SessionRegistry>>,
    pub owned_tabs: Arc<std::sync::Mutex<HashMap<i64, session::SessionGuid>>>,
    pub mint_quota: MintQuota,
    pub live_sessions: Arc<AtomicUsize>,
    /// The service's observability sink (a clone of the one the browser holds). The inbound.web
    /// transport publishes its actual bound port through this once its listener binds, so a reader
    /// -- `status`, `doctor`, or a test -- learns the real port even when it was OS-assigned.
    pub debug_sink: DebugSink,
}

impl ServiceContext {
    /// The SHARED-lifetime startup sequence, moved verbatim from the pre-H1
    /// `transport::mcp::server::run` (store load -> `spawn_watcher` -> recorder build ->
    /// recorder-reload subscription spawn). This is a plain (non-async) fn that calls
    /// `tokio::spawn` internally; it is only ever invoked from within the tokio runtime (by
    /// `mcp::server::run`, itself polled inside `run_mcp_server`'s `rt.block_on` above).
    pub fn from_startup(
        browser: Browser,
        debug_sink: DebugSink,
        loaded_policy: LoadedPolicy,
        user_source: Option<String>,
    ) -> crate::Result<Self> {
        if let Some(manifest) = &loaded_policy.manifest {
            tracing::debug!(
                name = %manifest.name,
                version = %manifest.version,
                hash = %manifest.hash,
                "active manifest held for later governance tasks"
            );
        }

        let store = ConfigStore::load_initial_with_policy(
            pattern::is_valid_pattern,
            &loaded_policy,
            user_source,
        )?;
        store.clone().spawn_watcher();

        let recorder = Arc::new(Recorder::from_config(&store.current()));
        tokio::spawn({
            let recorder = Arc::clone(&recorder);
            let mut changes = store.subscribe();
            async move {
                while changes.changed().await.is_ok() {
                    let config = changes.borrow().clone();
                    recorder.reload(&config);
                }
            }
        });

        let capabilities = outbound::Registry::new(vec![Arc::new(
            outbound::browser::BrowserCapability::new(browser.clone()),
        )]);

        Ok(Self {
            browser,
            capabilities,
            store,
            recorder,
            initial_policy: loaded_policy.clone(),
            session_registry: Arc::new(std::sync::Mutex::new(session::SessionRegistry::new())),
            owned_tabs: Arc::new(std::sync::Mutex::new(HashMap::new())),
            mint_quota: Arc::new(Mutex::new(HashMap::new())),
            live_sessions: Arc::new(AtomicUsize::new(0)),
            debug_sink,
        })
    }
}
