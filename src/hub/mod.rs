// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The Hub composition root (ADR-0030 Decision 2: "Extract the composition root into a
//! free-licensed `src/hub` module hosting `HubCore`"). As of H2 this module hosts the
//! service-or-adapter election (`run_mcp_server`), the persistent SERVICE role (`run_as_service`:
//! owns the shared [`ServiceContext`], the extension endpoint, and the adapter/control endpoint),
//! and the thin ADAPTER role (`run_as_adapter`: a byte relay to an already-running service). The
//! session-hello constants the ADAPTER/CONTROL endpoint uses live in [`handshake`].

use crate::browser::pattern;
use crate::debug::DebugSink;
use crate::governance::audit::Recorder;
use crate::governance::config::reload::ConfigStore;
use crate::governance::manifest::source;
use crate::governance::manifest::source::LoadedPolicy;
use crate::native::ipc;
use crate::transport::executor::Browser;
use anyhow::{Context, Result};
use std::sync::Arc;

pub mod handshake;
pub mod role;
pub mod session;

/// mcp-server invocation (ADR-0030 Decision 1): claims the ADAPTER/CONTROL endpoint FIRST (PINS.md
/// SS1 pin 1), before opening anything else, so this process learns whether it is the persistent
/// SERVICE or a thin ADAPTER before building a [`Browser`] or a [`ServiceContext`]. This REPEALS
/// ADR-0004's degrade semantics at the MCP-client layer: a second (or Nth) MCP client no longer
/// runs a doomed session against its own never-connecting `Browser` -- it multiplexes through the
/// winner's real one instead ([`run_as_adapter`]).
pub fn run_mcp_server(manifest: Option<String>, debug_on: bool) -> Result<()> {
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
            "ghostlight starting (mcp-server role; governance overlay active)"
        ),
        _ => tracing::info!(
            debug_mode = debug_on,
            "ghostlight starting (mcp-server role; no manifest: all-open)"
        ),
    }

    // The MCP client that spawned us, captured before the runtime starts (ADR-0029). The
    // parent-death watchdog below watches it (SERVICE role only -- ADR-0030 Decision 8 re-scopes
    // the reaper to the ADAPTER at H6; until then a lone-client SERVICE keeps today's behavior).
    // None (no resolvable parent) simply skips the watchdog and leaves stdin EOF as the sole exit
    // trigger, as before.
    let parent = crate::proc::parent();

    // Startup self-heal (ADR-0029 part 4): reap any orphaned predecessor -- a server whose client
    // exited but that did not terminate (e.g. one built before the watchdog, or killed uncleanly)
    // -- before we serve. Best-effort and safe (only parent-dead orphans; see doctor::reap): a
    // no-op in a release build (no session registry) and when nothing is orphaned. Runs before the
    // sink is enabled, so our own not-yet-written state file is never a self-reap candidate.
    crate::doctor::sweep_orphans();

    let sink = build_debug_sink(debug_on, "mcp-server");
    let rt = tokio::runtime::Runtime::new()?;

    let block_sink = sink.clone();
    let endpoint = ipc::default_endpoint();
    let code = rt.block_on(async move {
        // Claim the ADAPTER/CONTROL endpoint FIRST (ADR-0030 Decision 1; PINS.md SS1 pin 1): the
        // single-instance election. The winner IS the persistent SERVICE; a loser becomes the
        // thin ADAPTER. Neither branch opens the extension endpoint or builds a Browser/
        // ServiceContext until this is decided.
        match ipc::claim_adapter_endpoint(&endpoint).await {
            Ok(adapter_listener) => {
                run_as_service(
                    adapter_listener,
                    endpoint,
                    block_sink,
                    loaded_policy,
                    user_source,
                    parent,
                )
                .await
            }
            Err(crate::Error::SessionBusy) => run_as_adapter(&endpoint, block_sink).await,
            Err(e) => {
                tracing::error!(error = %e, "failed to claim the adapter/control endpoint");
                1
            }
        }
    });

    // The single ordered teardown. process::exit rather than unwinding: on a detector-triggered
    // shutdown the stdin read may still be parked in a blocking ReadFile, and dropping the runtime
    // would hang forever trying to join that thread (the same reason the native-host role exits
    // directly). Flush the final observability snapshot first; exiting then releases the IPC
    // endpoint for the next session.
    sink.flush();
    std::process::exit(code)
}

/// The persistent SERVICE role (ADR-0030 Decision 1, Decision 2): this process won the
/// ADAPTER/CONTROL endpoint election. For its whole life it owns BOTH local endpoints: the
/// UNCHANGED, hello-free EXTENSION endpoint (`ipc::serve` -> `Browser::attach`, server-speaks-
/// first, exactly as before this task) and the NEW ADAPTER/CONTROL endpoint (`ipc::serve_adapters`,
/// over the ALREADY-claimed listener -- never re-claims the name). This process's OWN stdio is
/// served as the first session, over the SAME shared [`ServiceContext`] every multiplexed adapter
/// session clones (Decision 2), so a lone client's extension path stays byte-identical to the
/// pre-H2 single-session behavior (ADR-0030 "Preserved invariants": all-open byte-identity).
async fn run_as_service(
    adapter_listener: ipc::AdapterListener,
    endpoint: String,
    debug_sink: DebugSink,
    loaded_policy: LoadedPolicy,
    user_source: Option<String>,
    parent: Option<crate::proc::ProcId>,
) -> i32 {
    // Role marker (ADR-0030 Decision 1 addendum; PINS.md SS8): this process won the
    // ADAPTER/CONTROL claim, so it IS the SERVICE. Recorded once, before anything else, so the
    // governance chokepoint's `assert_service_role` below can rely on it.
    role::set_role(role::Role::Service);

    let browser = Browser::with_debug(debug_sink);
    let shutdown = std::sync::Arc::new(tokio::sync::Notify::new());

    // Detector: parent-death watchdog. stdin EOF is the intended shutdown signal, but on Windows
    // a killed (not cleanly closed) client can leave our stdin read parked forever, so the read
    // loop alone would never notice the client is gone. The watchdog signals shutdown when the
    // parent process exits; it only signals -- the coordinator below does the teardown.
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

    // The EXTENSION endpoint: UNCHANGED, server-speaks-first, no hello (ADR-0030 Decision 1;
    // PINS.md SS1). Only the winner ever calls this; its own Ok/Err handling is independent of
    // the adapter/control election above (a stale extension-endpoint owner is a separate, rare
    // edge case that degrades quietly here, exactly as before this task).
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

    // Build the SHARED ServiceContext ONCE (PINS.md SS1 pin 4); every session -- this process's
    // own stdio below, and every multiplexed adapter session `serve_adapters` spawns -- clones it.
    let ctx = match ServiceContext::from_startup(browser, loaded_policy, user_source) {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!(error = %e, "failed to build the shared service context");
            return 1;
        }
    };

    // The ADAPTER/CONTROL endpoint: accept sessions over the ALREADY-claimed listener (never
    // re-claims the name -- PINS.md SS1 pin 1).
    tokio::spawn({
        let ctx = ctx.clone();
        async move {
            if let Err(e) = ipc::serve_adapters(ctx, adapter_listener).await {
                tracing::error!(error = %e, "adapter/control endpoint failed");
            }
        }
    });

    // The coordinator: whichever shutdown trigger fires first lands here. stdin EOF makes
    // `serve_session` return (after its own internal task cleanup); a detector signal arrives on
    // `shutdown`. Both paths fall through to the single teardown in `run_mcp_server`.
    //
    // H3 (PINS.md SS9): this process's own directly-served stdio session mints its OWN GUID --
    // every session gets a real one, including this lone-client path (closes an isolation gap a
    // `None`/exempt session would otherwise leave in a later cross-session ownership map).
    let own_guid = session::SessionGuid::mint();
    let stream = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());
    tokio::select! {
        result = crate::mcp::server::serve_session(stream, ctx, own_guid) => {
            match result {
                Ok(()) => 0,
                Err(e) => {
                    tracing::error!(error = %e, "mcp-server loop ended with an error");
                    1
                }
            }
        }
        _ = shutdown.notified() => 0,
    }
}

/// The thin ADAPTER role (ADR-0030 Decision 1): this process LOST the ADAPTER/CONTROL endpoint
/// election, meaning a SERVICE is already running. Relay this process's stdio to it -- a pure
/// byte relay, never a rewriter (ADR-0030 "Preserved invariants"). Connects to an already-running
/// service only; spawn-on-demand is H6, out of scope here.
async fn run_as_adapter(endpoint: &str, debug_sink: DebugSink) -> i32 {
    // Role marker (ADR-0030 Decision 1 addendum; PINS.md SS8): this process LOST the
    // ADAPTER/CONTROL claim, so it IS the thin ADAPTER. Recorded once, before anything else.
    role::set_role(role::Role::Adapter);

    match ipc::relay_adapter(endpoint, &debug_sink).await {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!(error = %e, "adapter relay ended with an error");
            1
        }
    }
}

/// Build the observability sink for `role` ("mcp-server" or "native-host"). Debug-off yields a
/// no-op sink; if the log directory cannot be prepared we warn and continue without observability
/// rather than failing the process.
pub fn build_debug_sink(debug: bool, role: &'static str) -> DebugSink {
    if !debug {
        return DebugSink::disabled();
    }
    let Some(dir) = crate::debug::log_dir() else {
        tracing::warn!("no log directory available; running without debug observability");
        return DebugSink::disabled();
    };
    match DebugSink::enabled(&dir, role) {
        Ok(sink) => {
            tracing::info!(dir = %dir.display(), role, "debug mode on: state + event log under this dir");
            sink
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not enable debug sink; continuing without it");
            DebugSink::disabled()
        }
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
    pub store: Arc<ConfigStore>,
    pub recorder: Arc<Recorder>,
    pub initial_policy: LoadedPolicy,
    /// H3 (PINS.md SS9): the GUID -> bound-peer admission table, shared by every session so a
    /// re-presented GUID is checked against the SAME registry regardless of which adapter
    /// connection presents it.
    pub session_registry: Arc<std::sync::Mutex<session::SessionRegistry>>,
}

impl ServiceContext {
    /// The SHARED-lifetime startup sequence, moved verbatim from the pre-H1
    /// `transport::mcp::server::run` (store load -> `spawn_watcher` -> recorder build ->
    /// recorder-reload subscription spawn). This is a plain (non-async) fn that calls
    /// `tokio::spawn` internally; it is only ever invoked from within the tokio runtime (by
    /// `mcp::server::run`, itself polled inside `run_mcp_server`'s `rt.block_on` above).
    pub fn from_startup(
        browser: Browser,
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

        Ok(Self {
            browser,
            store,
            recorder,
            initial_policy: loaded_policy.clone(),
            session_registry: Arc::new(std::sync::Mutex::new(session::SessionRegistry::new())),
        })
    }
}
