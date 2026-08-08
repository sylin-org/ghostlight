// SPDX-License-Identifier: Apache-2.0 OR MIT
//! The Hub composition root (ADR-0030 Decision 2: "Extract the composition root into a
//! free-licensed `src/hub` module hosting `HubCore`").
//!
//! ADR-0096 keeps this as the persistent, protocol-neutral service. `ghostlight-mcp-connector` owns stdio,
//! JSON-RPC, and exact MCP revision state; it connects here through the owner-only typed local
//! bridge. [`run_service`] owns the shared [`ServiceContext`], bridge/control endpoint, and
//! extension endpoint for its whole life. It runs no parent-death watchdog and shuts down only on
//! a continuous idle-grace window ([`run_service_loop`]/`idle_grace_watch`). Bridge hello constants
//! live in [`handshake`], OS-supervisor identifiers and self-heal in [`supervisor`], and the
//! per-install anti-squat secret and HMAC proof in [`antisquat`].
//!
//! ADR-0030 Decision 3 ("D1 -- the honest singleton queue"): the single MV3 service worker plus
//! the single native port is an ACCEPTED, TRUTHFUL serialization bottleneck -- fair ordering and
//! truthful failure on a real drop, never a hidden work-around. H5 lands the three properties
//! Decision 3 names: a bounded reconnect grace window (`hub::outbound::browser::Browser::attach`,
//! `GRACE_WINDOW`, strictly less than `TOOL_TIMEOUT`), a per-peer (never global) mint quota
//! (below, [`try_mint`]/[`PER_PEER_MINT_CAP`]). See
//! `docs/adr/0004-reject-second-session.md`'s amendment note for the cross-reference from the
//! original single-session decision this multiplexes past.

use crate::browser::pattern;
use crate::governance::audit::Recorder;
use crate::governance::config::reload::{ConfigStore, PolicySource};
use crate::governance::manifest::identity::ManifestIdentity;
use crate::governance::manifest::source;
use crate::governance::manifest::source::LoadedPolicy;
use crate::governance::ports::AuditSink;
use crate::hub::authority::AuthorityStore;
use crate::hub::outbound::browser::Browser;
use anyhow::{Context, Result};
use ghostlight_transport::ipc;
use ghostlight_transport::observability::DebugSink;
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

pub use ghostlight_transport::{antisquat, handshake, supervisor};
pub mod authority;
pub mod bridge;
pub mod endpoint;
pub mod inbound;
pub mod manage;
pub mod outbound;
pub mod peer;
pub mod scheduling;
pub mod workspace;

/// Idle-grace shutdown window (ADR-0030 Decision 8; PINNED, PINS.md SS5.4): the SERVICE exits only
/// after zero live MCP-edge bridge streams AND the extension link gone, CONTINUOUSLY, for this
/// long. Never a
/// parent-death trigger -- the service has no client parent to watch.
pub const IDLE_GRACE: Duration = Duration::from_secs(30);

/// Idle-grace poll interval (author-pinned, PINS.md SS5.4; not itself an ADR-0030 value).
pub const IDLE_POLL: Duration = Duration::from_secs(1);

/// Per-peer (never global) mint quota (ADR-0030 Decision 3: "per-peer-identity mint/group
/// quotas (never a single global cap, which is itself a lockout DoS)"; Decision 4's "per-peer
/// rate-limit key" amendment). PINNED in `docs/tasks/hub/PINS.md` SS4: max CONCURRENT
/// service-minted workspaces per admitted peer identity.
pub const PER_PEER_MINT_CAP: usize = 32;

/// The paired per-peer live-tab-group cap (H7; PINNED in PINS.md SS4, equal to
/// [`PER_PEER_MINT_CAP`] by design -- "the paired ... equal by design"). Not yet consumed: H7
/// wires this in when it adds per-workspace tab groups.
pub const PER_PEER_GROUP_CAP: usize = 32;

/// The quota-exceeded result (PINNED in `docs/tasks/hub/PINS.md` SS4): a plain tool error, never
/// a governance denial-id -- this is a HUB admission decision, not a change to the 13+`explain`
/// tool surface.
pub const MINT_QUOTA_EXCEEDED: &str = "session limit reached for this client";

/// Shared per-peer mint-quota table (ADR-0030 Decision 3 + Decision 4): keyed on the peer's OS
/// credential ([`peer::PeerUser`]), NEVER a single global counter. A `ServiceContext` field,
/// added beside the service's workspace and tab registries.
pub type MintQuota = Arc<Mutex<HashMap<peer::PeerUser, usize>>>;

/// RAII handle for one minted, live slot against a peer's [`PER_PEER_MINT_CAP`]. Decrements the
/// SAME counter [`try_mint`] incremented when this drops (the workspace retires), so the cap
/// counts concurrent workspaces, never lifetime mints.
#[must_use = "dropping the guard immediately frees the peer's mint-quota slot"]
pub struct MintGuard {
    quota: MintQuota,
    peer: peer::PeerUser,
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
    peer: &peer::PeerUser,
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

/// Default managed:// re-poll interval when the bootstrap does not set `poll_seconds` (ADR-0055
/// Phase 4b): 15 minutes. A few-KB conditional re-fetch at this cadence is trivially cheap, and the
/// last-known-good cache means a missed poll changes nothing.
const MANAGED_POLL_DEFAULT_SECS: u64 = 900;

/// Map a resolved managed reconciliation into a [`LoadedPolicy`] (ADR-0055 Phase 4). The
/// last-known-good cache means an unreachable source still yields the cached policy; only a FIRST
/// boot with the source unreachable AND no cache yields no policy, which is a FATAL startup error --
/// a configured managed instance must never fall back to unrestricted (fail closed).
fn managed_loaded_policy(
    reconciled: crate::governance::managed::cache::Reconciled,
    paths: &crate::governance::paths::GovernancePaths,
) -> Result<LoadedPolicy> {
    use crate::governance::manifest::source::ManifestOrigin;
    if paths.org_policy.exists() {
        tracing::warn!(
            "both a managed.json bootstrap and a local org policy file are present; the managed:// \
             policy takes precedence and the local org policy file is ignored"
        );
    }
    match reconciled.active {
        Some(vm) => {
            tracing::info!(
                freshness = ?reconciled.freshness,
                seq = vm.seq,
                name = %vm.manifest.name,
                "managed policy active (org-authoritative)"
            );
            Ok(LoadedPolicy {
                manifest: Some(vm.manifest),
                origin: Some(ManifestOrigin::Managed),
                user_manifest_ignored: false,
            })
        }
        None => anyhow::bail!(
            "managed:// policy is configured but no policy is available (first boot with the source \
             unreachable and no cached policy); refusing to start unrestricted -- fail closed (ADR-0055)"
        ),
    }
}

/// The standalone SERVICE entry point (ADR-0030 Decision 8 amendment; PINS.md SS5.1), run only
/// via the `ghostlight service` subcommand: loads policy (the ONLY role that does), then serves
/// forever until [`IDLE_GRACE`] elapses with no live bridge peers and the extension link gone.
/// Parent-death cleanup belongs to `ghostlight-mcp-connector`, not this persistent service.
pub fn run_service(manifest: Option<String>, debug_on: bool, keep_warm: bool) -> Result<()> {
    // Resolve the user-supplied manifest source (G12, shared format doc section 1.3): the
    // --manifest flag wins when both it and GHOSTLIGHT_MANIFEST are set. Plain synchronous
    // I/O, before the async runtime starts: a source that is SELECTED but cannot be read,
    // parsed, or validated is a fatal startup error (an org policy that fails open is worse
    // than a crash), so this must happen before a single JSON-RPC line is served.
    let user_source = manifest.or_else(|| std::env::var("GHOSTLIGHT_MANIFEST").ok());

    // managed:// (ADR-0055 Phase 4): if the admin `managed.json` bootstrap is present it is the org
    // authority and takes precedence over the source-string loader. Resolved here, before the async
    // runtime, so a configured-but-unresolvable managed policy fails closed BEFORE a line is served
    // (the same fail-closed discipline the org policy file already has). `GovernancePaths::production`
    // is the sole computer of the fixed trust-anchor locations (ADR-0056).
    let paths = crate::governance::paths::GovernancePaths::production();
    // Read the bootstrap once to decide the policy SOURCE (which the ConfigStore re-resolves through
    // on every reload, so the file watcher can never clobber a managed policy -- ADR-0056) and the
    // managed poll interval. The initial resolution keeps startup fail-loud semantics (a
    // configured-but-unresolvable managed policy fails closed here, before a line is served).
    let managed_bootstrap = crate::governance::managed::load_bootstrap_at(&paths.managed_bootstrap)
        .with_context(|| "loading the managed policy bootstrap")?;
    let (loaded_policy, policy_source, managed_poll) = match managed_bootstrap {
        Some(bootstrap) => {
            let reconciled =
                crate::governance::managed::activate(&paths, pattern::is_valid_pattern)
                    .with_context(|| "resolving the managed policy")?
                    .ok_or_else(|| anyhow::anyhow!("managed bootstrap vanished during startup"))?;
            let loaded = managed_loaded_policy(reconciled, &paths)?;
            let poll = std::time::Duration::from_secs(
                bootstrap.poll_seconds.unwrap_or(MANAGED_POLL_DEFAULT_SECS),
            );
            (loaded, PolicySource::Managed { paths }, Some(poll))
        }
        None => {
            let loaded = source::load_policy(user_source.as_deref(), pattern::is_valid_pattern)
                .with_context(|| "loading the governance manifest")?;
            (loaded, PolicySource::SourceString { user_source }, None)
        }
    };

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

    let sink = ghostlight_transport::observability::build_debug_sink(debug_on, "mcp-server");
    let rt = tokio::runtime::Runtime::new()?;
    let block_sink = sink.clone();
    let endpoint = ipc::default_endpoint();
    let code = rt.block_on(run_service_loop(
        endpoint,
        block_sink,
        loaded_policy,
        policy_source,
        managed_poll,
        keep_warm,
    ));

    sink.flush();
    std::process::exit(code)
}

/// The async body of [`run_service`] (ADR-0030 Decision 1, Decision 2, Decision 8; PINS.md SS5.1):
/// claim the bridge/control endpoint as a single-instance guard (never a role election -- role
/// was already decided by argv), then own both local endpoints for the rest of this process's
/// life, and finally run the [`IDLE_GRACE`] watcher as the returning future. NEVER serves this
/// process's own stdio as a session (Decision 8 amendment: a standalone service has no stdio
/// session of its own) and NEVER captures a parent or runs the ADR-0029 watchdog.
async fn run_service_loop(
    endpoint: String,
    debug_sink: DebugSink,
    loaded_policy: LoadedPolicy,
    policy_source: PolicySource,
    managed_poll: Option<std::time::Duration>,
    keep_warm: bool,
) -> i32 {
    let local_listener = match endpoint::claim_adapter_endpoint(&endpoint).await {
        Ok(listener) => listener,
        Err(crate::Error::SessionBusy) => {
            tracing::info!("a Ghostlight service is already running on this endpoint; exiting");
            return 0;
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to claim the MCP-edge/control endpoint");
            return 1;
        }
    };

    // Anti-squat (ADR-0030 Decision 8; PINS.md SS5.3): prepare the per-install secret now, before
    // the MCP-edge/control endpoint is actually served below, so no connection can ever race the
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

    // The browser endpoint admits only the browser-only relay and its extension hello.
    tokio::spawn({
        let browser = browser.clone();
        let ext_endpoint = endpoint.clone();
        async move {
            match endpoint::serve(browser, &ext_endpoint).await {
                Ok(()) => {}
                Err(crate::Error::SessionBusy) => tracing::warn!(
                    "another ghostlight session already owns the browser; tool calls in this \
                     session will report the extension as unavailable"
                ),
                Err(e) => tracing::error!(error = %e, "browser IPC endpoint failed"),
            }
        }
    });

    // Build the shared ServiceContext once; each admitted bridge/control peer receives a clone.
    let ctx = match ServiceContext::from_startup(
        browser,
        debug_sink,
        loaded_policy,
        policy_source,
        managed_poll,
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!(error = %e, "failed to build the shared service context");
            return 1;
        }
    };

    // The local ingress wrapper owns the already-claimed bridge/control listener lifecycle.
    let pipe = inbound::pipe::PipeTransport::new(local_listener);
    tokio::spawn(pipe.run(ctx.clone()));

    // The read-only management UI owns a separate loopback HTTP listener. It is not an MCP
    // transport and rejects WebSocket upgrades (ADR-0077).
    if manage::web::enabled(&ctx.store) {
        tokio::spawn(manage::web::run(ctx.clone()));
    } else {
        tracing::info!("manage.web listener not bound: manage.web.enabled is false");
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
/// exits once zero live MCP-edge bridge streams, zero settling work items, AND the extension link
/// gone hold CONTINUOUSLY for [`IDLE_GRACE`]; any bridge stream, work item, or reconnected
/// extension resets the counter to zero.
async fn idle_grace_watch(ctx: ServiceContext) -> i32 {
    let mut idle_for = Duration::ZERO;
    loop {
        tokio::time::sleep(IDLE_POLL).await;
        let idle = ctx.live_sessions.load(std::sync::atomic::Ordering::Relaxed) == 0
            && ctx.active_work.load(std::sync::atomic::Ordering::Relaxed) == 0
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

/// Shared protocol-neutral service state. The one [`Browser`] handle, [`ConfigStore`], audit
/// [`Recorder`], current authority, workspace registry, and canonical catalog signal are built
/// once at startup and cloned into each admitted bridge stream. MCP lifecycle and revision state
/// remain in `ghostlight-mcp-connector`; per-call state remains in the bridge work future.
///
/// Every field is a cheap `Arc` clone or an already-`Clone` value. Never call
/// [`ServiceContext::from_startup`] per bridge stream: it spawns service-global watcher and reaper
/// tasks, so doing so would duplicate them.
#[derive(Clone)]
pub struct ServiceContext {
    pub browser: Browser,
    pub store: Arc<ConfigStore>,
    pub recorder: Arc<Recorder>,
    /// One service-global, client-neutral authority snapshot slot.
    pub authority: Arc<AuthorityStore>,
    /// Service-owned browser workspaces and their handle membership.
    pub workspaces: workspace::WorkspaceRegistry,
    /// Monotonic generation signal for canonical catalog projections.
    pub catalog_generation: tokio::sync::watch::Sender<u64>,
    pub initial_policy: LoadedPolicy,
    /// Number of live typed bridge streams from `ghostlight-mcp-connector` edges.
    pub live_sessions: Arc<AtomicUsize>,
    /// Work futures still running or settling after their originating bridge stream closed.
    /// Kept separate from live_sessions because it is service liveness, not client presence.
    pub active_work: Arc<AtomicUsize>,
    /// The service's observability sink (a clone of the one the browser holds). The manage.web
    /// listener publishes its actual bound port through this once it binds, so a reader or test
    /// learns the real port even when it was OS-assigned.
    pub debug_sink: DebugSink,
}

impl ServiceContext {
    /// Build the service-global state and start its watcher and reaper tasks.
    ///
    /// This plain function calls `tokio::spawn` internally and therefore must run inside the
    /// service's Tokio runtime. Call it once per service process, never per bridge stream.
    pub fn from_startup(
        browser: Browser,
        debug_sink: DebugSink,
        loaded_policy: LoadedPolicy,
        policy_source: PolicySource,
        managed_poll: Option<std::time::Duration>,
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
            policy_source,
        )?;
        store.clone().spawn_watcher();
        // managed:// (ADR-0055 Phase 4b): a timer re-resolves through the SAME reresolve path the
        // watcher uses, so a newly published bundle is picked up live without a restart.
        if let Some(interval) = managed_poll {
            store.clone().spawn_managed_poll(interval);
        }

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

        // Licensing observability (ADR-0028 Decisions 1 and 3, refined 2026-07-10). The engine is
        // DORMANT unless governance is operationally in effect via an ORG-DEPLOYED policy: in the
        // free all-open path, and for a user-supplied `--manifest` / `GHOSTLIGHT_MANIFEST`, nothing
        // is resolved, stamped, or warned, so the audit stream stays byte-identical to a build with
        // no licensing at all. The recorder carries the stamp opaquely; all license logic lives in
        // `governance::license`. A `managed://` bundle is org-authoritative too (ADR-0055 Phase 4:
        // the strongest "an organization is operating governance" signal), so it joins `OrgPolicyFile`.
        let governance_operational = matches!(
            loaded_policy.origin,
            Some(crate::governance::manifest::source::ManifestOrigin::OrgPolicyFile)
                | Some(crate::governance::manifest::source::ManifestOrigin::Managed)
        );
        if governance_operational {
            let (license_state, license_path) = crate::governance::license::resolve_from_disk();
            let stamp = crate::governance::license::stamp_for(&license_state);
            recorder.set_license_stamp(stamp);
            if let Some(s) = stamp {
                tracing::warn!(
                    stamp = s,
                    path = ?license_path,
                    "license state is abnormal for an operational governance deployment; audit records will carry a license stamp until it is resolved"
                );
            }
            // ADR-0055 Impl.9c: under managed governance the tool-call audit stream carries the
            // org-signed policy sequence from the T2 status sidecar. Other operational origins
            // (OrgPolicyFile) leave policy_seq unset (default None), so their streams are unchanged.
            if matches!(
                loaded_policy.origin,
                Some(crate::governance::manifest::source::ManifestOrigin::Managed)
            ) {
                let paths = crate::governance::paths::GovernancePaths::production();
                if let Some(cache_path) = paths.managed_cache.as_ref() {
                    let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
                    if let Some(status) = crate::governance::managed::status::read_sidecar(&sidecar)
                    {
                        recorder.set_policy_seq(status.seq);
                    }
                }
            }
        }

        let authority = Arc::new(AuthorityStore::new(
            &store.current_authority(),
            recorder.clone() as Arc<dyn AuditSink>,
        ));
        if loaded_policy.user_manifest_ignored {
            authority
                .current()
                .governance
                .record_user_manifest_ignored();
        }
        let catalog_generation = tokio::sync::watch::channel(1u64).0;
        spawn_authority_watch(
            Arc::clone(&store),
            Arc::clone(&authority),
            Arc::clone(&recorder),
            browser.clone(),
            catalog_generation.clone(),
        );

        let mint_quota: MintQuota = Arc::new(Mutex::new(HashMap::new()));
        let workspaces = workspace::WorkspaceRegistry::new(Arc::clone(&mint_quota));
        browser.bind_workspace_registry(workspaces.clone());
        spawn_workspace_reaper(workspaces.clone(), browser.clone());

        Ok(Self {
            browser,
            store,
            recorder,
            authority,
            workspaces,
            catalog_generation,
            initial_policy: loaded_policy.clone(),
            live_sessions: Arc::new(AtomicUsize::new(0)),
            active_work: Arc::new(AtomicUsize::new(0)),
            debug_sink,
        })
    }
}

fn manifest_identity_of(policy: &LoadedPolicy) -> Option<ManifestIdentity> {
    policy.manifest.as_ref().map(|manifest| ManifestIdentity {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        hash: manifest.hash.clone(),
    })
}

fn spawn_authority_watch(
    store: Arc<ConfigStore>,
    authority: Arc<AuthorityStore>,
    recorder: Arc<Recorder>,
    browser: Browser,
    catalog_generation: tokio::sync::watch::Sender<u64>,
) {
    tokio::spawn(async move {
        let mut changes = store.subscribe_authority();
        let mut ignored_in_force = changes.borrow().policy.user_manifest_ignored;
        while changes.changed().await.is_ok() {
            let inputs = changes.borrow_and_update().clone();
            let outgoing = authority.current();
            let policy_changed =
                manifest_identity_of(&outgoing.policy) != manifest_identity_of(&inputs.policy);

            if policy_changed {
                browser.erase_all_recordings(crate::recording::StopReason::PolicyChanged);
            }

            match inputs.policy.origin {
                Some(crate::governance::manifest::source::ManifestOrigin::Managed) => {
                    let paths = crate::governance::paths::GovernancePaths::production();
                    if let Some(cache_path) = paths.managed_cache.as_ref() {
                        let sidecar = crate::governance::managed::status::sidecar_path(cache_path);
                        if let Some(status) =
                            crate::governance::managed::status::read_sidecar(&sidecar)
                        {
                            recorder.set_policy_seq(status.seq);
                        }
                    }
                }
                _ => recorder.set_policy_seq(None),
            }

            let before =
                crate::operation::registry::project_availability(&outgoing.governance, None, 0)
                    .operations;
            browser.scheduler().advance_authority_epoch(inputs.epoch);
            let next = authority.install(&inputs);
            let after = crate::operation::registry::project_availability(&next.governance, None, 0)
                .operations;

            if policy_changed {
                next.governance
                    .record_manifest_reload_with_client(manifest_identity_of(&inputs.policy), None);
            }
            if before != after {
                catalog_generation.send_modify(|generation| {
                    *generation = generation.wrapping_add(1).max(1);
                });
            }
            if policy_changed
                && crate::governance::ports::user_manifest_ignored_transitioned(
                    ignored_in_force,
                    inputs.policy.user_manifest_ignored,
                )
            {
                next.governance
                    .record_user_manifest_ignored_with_client(None);
            }
            ignored_in_force = inputs.policy.user_manifest_ignored;
        }
    });
}

fn spawn_workspace_reaper(workspaces: workspace::WorkspaceRegistry, browser: Browser) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            workspaces.reap_expired(std::time::Instant::now());
            for retired in workspaces.take_retired() {
                browser.cleanup_workspace(retired.workspace.as_str(), &retired.tabs);
            }
        }
    });
}
