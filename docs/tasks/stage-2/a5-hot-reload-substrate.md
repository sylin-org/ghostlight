# A5: Hot-reload substrate (atomic snapshot swap + debounced file-watch + validate-then-swap)

## Goal

Make the in-force resolved `Config` reloadable at runtime with no process restart. Hold the
resolved snapshot behind a single atomic swap slot so every per-call read sees a consistent
snapshot and a re-resolve replaces it in one store. Provide a re-resolve function that re-runs
the layered resolver (from G02), validates a full candidate, and swaps it in only on success.
Add a debounced, cross-platform file-watch on the three configuration sources (user config file,
org policy file, active manifest source) that triggers a re-resolve when one of them changes.
Enforce the source-specific invalid-on-reload security rule: an invalid user config keeps the
last-good snapshot and warns; an invalid org policy is FAIL-CLOSED (keep the last-good org policy,
error, and never fall open to a weaker posture). Expose a change signal the tool-advertisement
layer (G14) can subscribe to, so it can later emit `notifications/tools/list_changed` when a
reload changes the permitted tool set. `config set` (G03, a separate process) and the future
options page cause an immediate re-resolve.

This task builds the reload SUBSTRATE. It does not build the resolver (G02), the CLI (G03), the
manifest engine (G12), or the `list_changed` emit (G14). It changes NO runtime behavior for the
all-open path: with no config files and no manifest, the resolved snapshot is `Config::minimal()`
forever and the watcher never produces a different snapshot, so every tool result stays
byte-identical to stage 1.

## Depends on

- `docs/tasks/stage-2/PLAN.md` -- Phase A, the "Resolved decisions" (hot-reload is first-class;
  Config is owned, not `Copy`), the "Cross-cutting workstream: hot-reload" section (the atomic
  swap, validate-then-swap, and the source-specific invalid-on-reload rule are quoted from there
  verbatim in intent), and the "Org policy loading" section (fail-closed, non-bypassable).
- `docs/design/ghostlight-service-architecture.md` -- sections 3 (bounded contexts: Configuration
  is core), 4 (the seam traits), and 5's "Config federation and org locks" plus the open decision
  on hot-reload (section 8/9). A5 is the concrete answer to that open decision for the config
  layers.
- `docs/tasks/stage-2/00-shared-format.md` -- the reconciled format reference. Load-bearing here:
  section 1.1 (user config file path), 1.2 (org policy file path; "No flag can bypass it"),
  section 2 / 2.1 (the five-layer model and the resolved triple), section 3.4 (the seven keys).
- **G01 (owned typed `Config`)** and **G02 (the layered resolver and file loaders)** must be
  landed. A5 consumes their public surface; it does not re-implement layer precedence, value
  validation, file paths, or parsing.

PREREQUISITE CHECK -- run this first, before writing any code:

Open `src/policy/mod.rs` and `src/policy/load.rs`. As of this prompt's authoring the tree is still
the stage-1 seed: `src/policy/mod.rs` has `KeyDef { key, description, minimal_default: bool }`, a
one-key `KEYS`, and a `#[derive(... Copy)] struct Config { secrets_redact: bool }`; there is no
`src/policy/layers.rs`, no `src/policy/load.rs`, and no `Config::from_resolution`. **If that is
still the state you find, G01 and G02 have NOT landed: STOP and report that A5 is blocked on G01
and G02.** Do not fold registry growth or the resolver into A5, and do not invent a resolver.

The exact G01/G02 integration points A5 binds to (names may differ slightly from what actually
landed -- re-read the files and adapt, keeping the semantics below):

- `crate::policy::Config` -- owned (non-`Copy`), `#[derive(Debug, Clone, PartialEq)]`, one accessor
  per key. A5 stores it inside `Arc<Config>` and swaps the `Arc`.
- `crate::policy::Config::from_resolution(&layers::Resolution) -> Config` (G02 section 3). The
  values in a `Resolution` are already validated by the loaders, so this conversion cannot fail;
  A5 treats a successful parse-and-resolve as the validation gate.
- `crate::policy::layers::{Resolution, LayerInputs, resolve}` (G02 section 1). `resolve` is
  infallible (the builtin layer defines every key).
- `crate::policy::load::{user_config_path, org_policy_path}` (G02 section 2.1). The user path is
  `Option<PathBuf>` (None when the platform config dir is unavailable); the org path is a fixed
  `PathBuf`.
- `crate::policy::load::{parse_user_config, parse_org_config, UserConfig, OrgConfig}` (G02
  sections 2.2, 2.3). A5 calls these granular parsers directly so it can apply the source-specific
  reload rule (G02's coarse `load_and_resolve` is all-or-nothing and fail-loud, which is correct
  for startup but wrong for reload). `UserConfig` carries `preset: Option<String>` and
  `values: serde_json::Map`; `OrgConfig` carries `mandatory` and `recommended` maps; both parsers
  return `crate::Result<...>` and the user parser also returns per-entry warnings.
- `crate::Error::Config(String)` (G02 section 4) -- the typed config error. A5 adds no new error
  variant; on reload it does not propagate errors, it logs them and keeps last-good (see below).

If any of these landed under different names or a different module path (for example the A1 module
reorg moved `policy/` under `governance/config/`), integrate against what exists and keep the
semantics. Do not guess at a shape that is not in the tree; if a required function is missing,
STOP and report it.

- All release-1 (stage-1) tasks are assumed landed and merged to `main`; stage 2 branches off it.

## Project context

Browser MCP is governed browser automation. A single Rust binary is BOTH the MCP server (JSON-RPC
2.0 over stdio, hand-rolled on tokio, no MCP SDK crate) AND the Chrome native-messaging host; a
thin Manifest V3 extension executes CDP commands:

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

The two binary roles (mcp-server and native-host) are separate OS processes bridged by
tokio-native named-pipe / Unix-domain-socket IPC. Only the mcp-server role loads configuration
(G02); the native-host role is a stateless relay and the installer subcommands are synchronous CLI
roles.

Stage 1 hardened the engine and merged to `main`. Stage 2 is the governance layer per ADR-0013
(separable overlay; all-open stays first-class), ADR-0018 (observe-then-enforce sequencing),
ADR-0019 (layered configuration and typed key registry), and ADR-0021 (Ghostlight family baseline:
the S1 seam-only chassis plus the S4 pure-serializable PDP contract). The code is grouped into
`governance/` (domain-agnostic core: config, decision, audit, manifest), `browser/` (the domain
plugin: matcher, classification table, resolver, redaction, the extension wire), and `transport/`
(infra: MCP, native messaging, IPC, dispatch chokepoint). Configuration is a `governance/` (core)
concern; A5 lands there.

Hot-reload is a first-class Resolved Decision of the plan: the long-lived nature of the service
(and, later, the persistent-service split) means config and manifests take effect on change with
no restart. Enforcement is per-call at the dispatch chokepoint, which makes reload clean: each
call reads the current resolved snapshot, so "reload" is just produce-a-new-snapshot-and-swap-it.
A5 is that swap plus the watcher plus the fail-closed reload rule.

## Current behavior

Verified against the working tree at authoring time; line numbers drift, trust the prose.

- `src/policy/mod.rs` (104 lines) holds the stage-1 seed registry only: `KeyDef` with
  `minimal_default: bool`, one key (`content.security.secrets.redact`), and
  `#[derive(Debug, Clone, Copy)] struct Config { secrets_redact: bool }` with `Config::minimal()`
  and a `Default` that delegates to it. It declares `pub mod redact;` and nothing else. **G01 and
  G02 have NOT landed** (no typed registry, no `layers.rs`, no `load.rs`, no `from_resolution`, and
  `Config` is still `Copy`). A5 is blocked until they do; see the PREREQUISITE CHECK.
- `src/mcp/server.rs` builds `let config = Config::default();` once at startup (near the top of
  `run`) and threads `config` to `handle_line` / `handle_tools_call` by value (this works only
  because `Config` is `Copy` today). The only read is `config.secrets_redact()` in the
  redaction overlay. G02 replaces the startup line with a `load_and_resolve()` call and switches
  the handlers to `&Config`; A5 replaces the startup line again (see part 6) and switches the
  read to a per-call snapshot fetch.
- `src/dispatch.rs` is the no-op governance seam: `policy_check` always returns
  `PolicyDecision::Allow`, `audit` does nothing. A5 does not touch dispatch's decision or audit
  logic; the per-call snapshot read is what makes any future enforcement reload cleanly. STEP-0
  (no manifest -> `Allow`) is preserved by construction.
- `Cargo.toml` has `tokio` (so `tokio::sync::watch`, `tokio::spawn`, and `tokio::time` are
  available), `serde` / `serde_json` (with `preserve_order`), `dirs`, `tracing`, `thiserror`,
  `clap`. There is NO `arc-swap` and NO `notify`. A5 adds neither (see Constraints).
- `tests/mcp_protocol.rs` spawns the real binary with no manifest and asserts the all-open surface
  (13 tools, byte-equal `tools/list`). It must pass UNCHANGED. `tests/tool_schema_fidelity.rs`
  (the sacred schema guard) must pass unchanged.

## Required behavior

Seven parts. Land the reload substrate in a new file `src/policy/reload.rs`, declared
`pub mod reload;` in `src/policy/mod.rs` next to `pub mod layers;` / `pub mod load;`. (If the A1
reorg already moved `policy/` under `governance/config/`, create `reload.rs` there instead and
declare it in that module; keep every type and signature below.) Every public item gets a doc
comment; the module gets a module-level doc comment stating: this is the ADR-0019 hot-reload
substrate -- the in-force resolved `Config` held behind an atomic swap, a validate-then-swap
re-resolve, a debounced file-watch on the three sources, and a change signal for G14; the
source-specific invalid-on-reload rule (lenient user, fail-closed org) is a security rule, not a
preference.

### 1. The atomic snapshot store

Choose `Mutex<Arc<Config>>` for the swap slot, NOT `ArcSwap`. Justification (put a condensed form
in the module doc comment): the read is a per-call event on the dispatch chokepoint, not a hot
inner loop; the critical section is a single `Arc` clone (an atomic refcount bump) followed by an
immediate unlock, so reads never contend for more than a few nanoseconds and never block a swap
meaningfully. `Mutex<Arc<Config>>` is `std`-only and needs zero new dependencies, which preserves
the single-binary / zero-runtime-dependencies posture (ADR-0001). `arc-swap` would be a second new
crate for a lock-free property this call site does not need. (`RwLock<Arc<Config>>` is an
acceptable equivalent; a plain `Mutex` is chosen because the critical section is symmetric and
tiny, so reader-writer distinction buys nothing.)

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::watch;

use crate::policy::{Config, layers, load};
use crate::policy::load::{OrgConfig, UserConfig};

/// The in-force resolved configuration, held behind a single swappable slot so a
/// re-resolve replaces it atomically and every subsequent per-call read sees the
/// new snapshot. Also holds the last-good layer inputs the reloader falls back to
/// (fail-closed for org policy) and the change-signal channel G14 subscribes to.
pub struct ConfigStore {
    /// The in-force snapshot. A per-call read clones the `Arc` and releases the
    /// lock immediately; a reload stores a fresh `Arc` in one operation.
    snapshot: Mutex<Arc<Config>>,
    /// Monotonic reload generation; bumped on every successful swap. Lets a
    /// subscriber cheaply answer "did the snapshot change since I last looked".
    generation: AtomicU64,
    /// Broadcasts the new snapshot on every successful swap. G14 subscribes here
    /// to recompute the advertised tool set and emit list_changed when it differs.
    tx: watch::Sender<Arc<Config>>,
    /// Last successfully-applied layer inputs per source, retained so an invalid
    /// reload of one source keeps that source's last-good contribution.
    last_good: Mutex<LastGoodInputs>,
    /// The three fixed source paths watched for change.
    sources: WatchSources,
}

/// The last-good layer inputs, per source. On a reload where one source fails to
/// load or validate, the store re-composes from these so a failed source never
/// weakens the resolved posture (this is what makes org-policy failure fail-closed).
#[derive(Debug, Clone, Default)]
struct LastGoodInputs {
    /// Last-good org contribution (mandatory + recommended maps). Never dropped on
    /// an invalid org reload.
    org: OrgConfig,
    /// Last-good user-layer values.
    user: serde_json::Map<String, serde_json::Value>,
}

/// The fixed source paths the watcher polls. The manifest slot is an integration
/// point for G12: today it is `None` (no manifest engine); when G12 lands a
/// file:// manifest source, its path is set here so an edit triggers a re-resolve.
/// An env:// or in-memory manifest source has no file to watch and is left `None`.
#[derive(Debug, Clone)]
struct WatchSources {
    user_config: Option<PathBuf>,
    org_policy: PathBuf,
    manifest: Option<PathBuf>,
}
```

Reads and subscription:

```rust
impl ConfigStore {
    /// The current in-force snapshot. This is the per-call read on the dispatch
    /// path: clone the `Arc` (cheap) and use it for the whole call, so a reload
    /// mid-call does not tear the snapshot the call already started with.
    pub fn current(&self) -> Arc<Config> {
        self.snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The current reload generation. Bumps by one on every successful swap.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Subscribe to snapshot changes. The receiver observes the new `Arc<Config>`
    /// after every successful swap. G14 uses this to recompute the advertised tool
    /// set and (when it changed) emit notifications/tools/list_changed. A5 provides
    /// the signal only; the emit itself is G14.
    pub fn subscribe(&self) -> watch::Receiver<Arc<Config>> {
        self.tx.subscribe()
    }
}
```

Poison handling: the critical sections here only clone or store an `Arc`, so they cannot panic and
the mutexes cannot realistically be poisoned; `unwrap_or_else(PoisonError::into_inner)` recovers
the guard defensively rather than panicking. Do not use `.unwrap()` on the locks.

### 2. Initial load (startup)

A5 owns the initial load so it can retain the last-good inputs the reloader needs. This REPLACES
the G02 startup pair (`let resolution = load_and_resolve()?; let config = from_resolution(...)`).

```rust
/// Poll interval for the source watcher. The config files change rarely (a user
/// edit, a `config set`, or an MDM push), so a sub-second poll on three known
/// paths is negligible cost.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

impl ConfigStore {
    /// Build the store from the initial layered load, called once at mcp-server
    /// startup. Startup keeps G02's FAIL-LOUD semantics: an invalid org policy
    /// file or a structurally broken user file at startup is a hard error and the
    /// server refuses to start (it must never boot open on a broken org push).
    /// The lenient, keep-last-good behavior is for RELOAD only (part 3), where a
    /// server is already running on a known-good snapshot.
    pub fn load_initial() -> crate::Result<Arc<ConfigStore>> {
        // Resolve the two source paths (G02 helpers). The org path is fixed and
        // non-bypassable; the user path is None when no platform config dir exists.
        let sources = WatchSources {
            user_config: load::user_config_path(),
            org_policy: load::org_policy_path(),
            manifest: None, // G12 integration point.
        };

        // Load both files strictly (fail-loud) for the FIRST resolution. Re-use
        // G02's granular parsers so the same read path is shared with reload; a
        // missing file is normal (empty inputs), any other outcome per G02's
        // strictness matrix (user: structural error is hard; org: any violation
        // is hard).
        let org = read_and_parse_org(&sources.org_policy)?; // Err -> propagate (fail-loud)
        let (user, user_warnings) =
            read_and_parse_user(sources.user_config.as_deref())?; // Err -> propagate

        for w in &user_warnings {
            tracing::warn!("config: {w}");
        }
        if let Some(name) = &user.preset {
            tracing::warn!(
                "config: preset '{name}' is declared in the user config file but preset defaults \
                 are not implemented yet, so it has no effect"
            );
        }

        let last_good = LastGoodInputs { org: org.clone(), user: user.values.clone() };
        let inputs = compose_inputs(&last_good);
        let resolution = layers::resolve(&inputs);
        let config = Arc::new(Config::from_resolution(&resolution));

        let (tx, _rx) = watch::channel(config.clone());
        Ok(Arc::new(ConfigStore {
            snapshot: Mutex::new(config),
            generation: AtomicU64::new(0),
            tx,
            last_good: Mutex::new(last_good),
            sources,
        }))
    }
}
```

Helpers (all in `reload.rs`):

- `fn compose_inputs(last_good: &LastGoodInputs) -> layers::LayerInputs` -- builds
  `LayerInputs { org_mandatory: last_good.org.mandatory.clone(), user: last_good.user.clone(),
  org_recommended: last_good.org.recommended.clone(), preset: Map::new() }`. The preset layer stays
  empty (presets are G18); do not populate it.
- `fn read_and_parse_org(path: &Path) -> crate::Result<OrgConfig>` -- `std::fs::read_to_string`;
  `ErrorKind::NotFound` -> `Ok(OrgConfig::default())` (absence is normal); any other IO error ->
  `Err(Error::Config(...))` naming the path (an org file that exists but is unreadable must not
  yield a weaker posture); otherwise `load::parse_org_config(&content, &path.display().to_string())`.
- `fn read_and_parse_user(path: Option<&Path>) -> crate::Result<(UserConfig, Vec<String>)>` --
  `None` or `ErrorKind::NotFound` -> `Ok((UserConfig::default(), Vec::new()))`; other IO error ->
  `Err`; otherwise `load::parse_user_config(...)`. (These wrap the same IO+parse that G02's
  `load_and_resolve` does; if G02 exposes a reusable pair, call it instead of duplicating.)

### 3. Re-resolve: validate-then-swap, with the source-specific security rule

Split a PURE planner from the IMPURE reload so the security rule is unit-testable with no
filesystem.

```rust
/// The pure reload plan: given fresh load attempts for each source and the current
/// last-good inputs, decide the new layer inputs, the new last-good, and the
/// per-source outcome. No I/O. This function encodes the security rule:
///
/// - User source Ok  -> adopt its values (and as new last-good); its per-entry
///   warnings are surfaced (WARN, not error).
/// - User source Err -> keep last-good user values; the structural failure is a
///   WARNING (a user file is user-serviceable; a broken one is stale, not fatal,
///   once the server is already running).
/// - Org source Ok   -> adopt it (and as new last-good).
/// - Org source Err  -> KEEP last-good org for BOTH the applied inputs and the new
///   last-good, and record an ERROR. FAIL-CLOSED: a malformed org push never drops
///   an org lock or relaxes an org value to a weaker layer. An org policy that
///   silently fails open is worse than a stale one.
fn plan_reload(
    org: Result<OrgConfig, String>,
    user: Result<(UserConfig, Vec<String>), String>,
    last_good: &LastGoodInputs,
) -> ReloadPlan
```

```rust
/// The outcome of [`plan_reload`]: the inputs to resolve, the new last-good to
/// retain, and human-readable messages split by severity.
struct ReloadPlan {
    inputs: layers::LayerInputs,
    new_last_good: LastGoodInputs,
    /// User per-entry problems and user structural failure (logged at WARN).
    warnings: Vec<String>,
    /// Org structural/validation failure (logged at ERROR; posture unchanged).
    errors: Vec<String>,
    org_failed: bool,
    user_failed: bool,
}
```

The impure re-resolve:

```rust
impl ConfigStore {
    /// Re-run the layered load and resolver and, only if a full candidate parses
    /// and validates, swap it into the snapshot slot. This is validate-then-swap:
    /// a half-written or invalid file never becomes the in-force snapshot. Applies
    /// the source-specific rule via [`plan_reload`]: a failed user source keeps the
    /// last-good user layer (WARN); a failed org source keeps the last-good org
    /// layer (ERROR, fail-closed). Returns a report for logging, the control-plane,
    /// and tests. Never returns an error (a running server is never taken down by a
    /// reload; it keeps its last-good snapshot).
    pub fn reresolve(&self) -> ReloadReport {
        let org = read_and_parse_org(&self.sources.org_policy).map_err(|e| e.to_string());
        let user = read_and_parse_user(self.sources.user_config.as_deref())
            .map_err(|e| e.to_string());

        let last_good = self.last_good.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let plan = plan_reload(org, user, &last_good);

        for w in &plan.warnings { tracing::warn!("config reload: {w}"); }
        for e in &plan.errors { tracing::error!("config reload: {e}"); }

        // "Validate" = the candidate parsed and resolved cleanly. Resolution values
        // are already validated by the loaders, so from_resolution cannot fail.
        let resolution = layers::resolve(&plan.inputs);
        let candidate = Arc::new(Config::from_resolution(&resolution));

        // Retain the new last-good regardless of swap (a failed source contributed
        // its own last-good back into the plan, so this never weakens org posture).
        *self.last_good.lock().unwrap_or_else(PoisonError::into_inner) = plan.new_last_good;

        let changed = {
            let mut slot = self.snapshot.lock().unwrap_or_else(PoisonError::into_inner);
            if **slot == *candidate {
                false
            } else {
                *slot = candidate.clone();
                true
            }
        };

        let generation = if changed {
            let g = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
            // watch::send only errs if there are no receivers, which is fine.
            let _ = self.tx.send(candidate);
            g
        } else {
            self.generation.load(Ordering::Acquire)
        };

        ReloadReport {
            swapped: changed,
            org_failed: plan.org_failed,
            user_failed: plan.user_failed,
            generation,
            warnings: plan.warnings,
            errors: plan.errors,
        }
    }

    /// Trigger an immediate re-resolve now, bypassing the poll interval. This is
    /// the hook for IN-PROCESS config writers: the future options-page settings
    /// protocol (native-messaging `set_config_key`) calls this so an edit takes
    /// effect immediately. `config set` (G03) runs in a SEPARATE CLI process and
    /// writes the file, so ITS trigger is the file-watch seeing the write (part 4),
    /// not this method. Tests also call this to drive a deterministic reload.
    pub fn notify_local_edit(&self) -> ReloadReport {
        self.reresolve()
    }
}
```

```rust
/// The result of a re-resolve, for logging, the control-plane, and tests.
#[derive(Debug, Clone)]
pub struct ReloadReport {
    /// True if a new, different snapshot was swapped in.
    pub swapped: bool,
    /// True if the org policy source failed to load/validate (last-good kept).
    pub org_failed: bool,
    /// True if the user config source failed structurally (last-good kept).
    pub user_failed: bool,
    /// The reload generation after this call.
    pub generation: u64,
    /// User-file warnings surfaced this reload.
    pub warnings: Vec<String>,
    /// Org-file errors surfaced this reload (posture unchanged; fail-closed).
    pub errors: Vec<String>,
}
```

`Config` must derive `PartialEq` (G01 already specifies this) so the `**slot == *candidate`
no-change check works; if it does not, add the derive in the G01 file.

### 4. The debounced, cross-platform file-watch

A zero-dependency debounced mtime poll, NOT the `notify` crate. Justification (module doc comment):
we watch exactly three known file paths, not recursive directory trees, and they change rarely;
polling `std::fs::metadata` on three paths every 750 ms is negligible and needs no new crate, which
keeps the zero-runtime-dependencies posture (ADR-0001). `notify` pulls a platform-backend
dependency tree (inotify/kqueue/ReadDirectoryChangesW plus its own transitive crates) that is
disproportionate for three files. The watcher is written behind a small abstraction so that if
sub-second latency ever matters, `notify` is a drop-in replacement without touching `reresolve`.

```rust
/// A cheap change fingerprint for a watched path: `None` when the file is absent,
/// or `(mtime, len)` when present. Absence is a distinct state, so a file being
/// created or deleted is detected, not just modified-in-place.
type Fingerprint = Option<(SystemTime, u64)>;

/// Compute the current fingerprint of a path. A metadata or mtime error is treated
/// as absence (`None`): an unreadable file is handled by the re-resolve's strict
/// IO-error path, not by the fingerprint.
fn fingerprint(path: &Path) -> Fingerprint {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
}
```

Debounce via a settle check (pure, unit-testable):

```rust
/// Per-path watch state: the last fingerprint the loop saw, and the fingerprint
/// that was in force at the last applied re-resolve.
#[derive(Debug, Clone, Default)]
struct PathWatch {
    last_seen: Fingerprint,
    last_applied: Fingerprint,
}

/// Decide whether a path's change has SETTLED and should trigger a re-resolve.
/// Debounce rule: a change fires only once the current fingerprint (a) differs
/// from `last_applied` (something changed since we last resolved) AND (b) equals
/// the immediately previous poll's fingerprint (the file has stopped changing).
/// This coalesces the multiple writes an editor or an MDM push emits and lets the
/// validate-then-swap backstop catch any half-written state that still slips
/// through. Returns the new PathWatch and whether to trigger.
fn settle(prev: &PathWatch, current: Fingerprint) -> (PathWatch, bool) {
    let stable = current == prev.last_seen;
    let changed = current != prev.last_applied;
    if stable && changed {
        (PathWatch { last_seen: current.clone(), last_applied: current }, true)
    } else {
        (PathWatch { last_seen: current, last_applied: prev.last_applied.clone() }, false)
    }
}
```

The watch loop:

```rust
impl ConfigStore {
    /// Spawn the debounced source watcher. mcp-server role ONLY (the native-host
    /// relay and the installer/config CLI roles must never start it). Polls the
    /// three source fingerprints every POLL_INTERVAL; when any source settles on a
    /// changed fingerprint, calls `reresolve()` once. Runs until the process exits.
    /// Takes `Arc<Self>` so the loop holds a strong reference to the store.
    pub fn spawn_watcher(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(POLL_INTERVAL);
            let mut watches: [PathWatch; 3] = Default::default();
            // Seed last_applied with the current fingerprints so the first poll does
            // not spuriously re-resolve the state we already loaded at startup.
            let paths = self.watched_paths();
            for (i, p) in paths.iter().enumerate() {
                let fp = p.as_ref().map(|p| fingerprint(p)).unwrap_or(None);
                watches[i] = PathWatch { last_seen: fp.clone(), last_applied: fp };
            }
            loop {
                interval.tick().await;
                let paths = self.watched_paths(); // manifest slot may change under G12
                let mut trigger = false;
                for (i, p) in paths.iter().enumerate() {
                    let fp = p.as_ref().map(|p| fingerprint(p)).unwrap_or(None);
                    let (next, fire) = settle(&watches[i], fp);
                    watches[i] = next;
                    trigger |= fire;
                }
                if trigger {
                    self.reresolve();
                }
            }
        });
    }

    /// The three watched paths in fixed order [user, org, manifest]; a `None` slot
    /// (no user config dir, or no file-based manifest source) is simply never a
    /// change. Recomputed each poll so a G12 manifest-source change is picked up.
    fn watched_paths(&self) -> [Option<PathBuf>; 3] {
        [
            self.sources.user_config.clone(),
            Some(self.sources.org_policy.clone()),
            self.sources.manifest.clone(),
        ]
    }
}
```

Note the manifest slot: A5 watches it if present but has nothing to parse (the manifest engine is
G12, out of scope). When G12 lands, setting `WatchSources.manifest` to the active file:// source
makes a manifest edit trigger `reresolve()`, which fires the change signal that G14 consumes. Add
an ASCII code comment at the manifest slot: `// INTEGRATION POINT (G12): set to the active file://
manifest path so an edit triggers a re-resolve and the G14 list_changed signal.`

### 5. The change signal for G14 (subscription contract)

The `watch::Receiver<Arc<Config>>` from `subscribe()` (part 1) IS the signal. Contract to document
on `subscribe`:

- After every successful swap, the receiver's `changed()` future resolves and `borrow()` yields
  the new `Arc<Config>`.
- G14 (out of scope here) will, on each change, recompute the permitted tool set from the current
  snapshot (and, once G12 lands, the resolved manifest that becomes part of the swapped state) and
  emit `notifications/tools/list_changed` only if the advertised set actually differs. A5 fires the
  signal on every config change; deciding whether the TOOL SET changed is G14's job.
- The initial `watch::channel(initial)` seed means a subscriber created before any reload sees the
  startup snapshot as its current value and only wakes on subsequent swaps.

Do NOT emit any MCP notification from A5. A5 provides the signal; the emit is G14.

### 6. Server wiring (`src/mcp/server.rs`)

Replace the G02 startup pair

```rust
let resolution = policy::load::load_and_resolve()?;
let config = Config::from_resolution(&resolution);
```

with

```rust
// Hot-reload substrate (ADR-0019): the resolved Config is held behind an atomic
// swap; the watcher re-resolves on a config/org/manifest change with no restart.
// With no files present this resolves to the built-in defaults, so all-open
// behavior is byte-identical to stage 1.
let store = policy::reload::ConfigStore::load_initial()?;
store.clone().spawn_watcher();
```

Then thread `Arc<ConfigStore>` where `Config` / `&Config` was threaded:

- Change `handle_line` / `handle_tools_call` to take `store: &Arc<policy::reload::ConfigStore>`
  (or clone the `Arc` into a spawned task, as G02/T04 do for the tools/call arm). At the point the
  handler needs the config, read a per-call snapshot: `let config = store.current();` and pass
  `&*config` (an `&Config`) to the existing readers (for example the redaction overlay
  `config.secrets_redact()`). The per-call read is the whole point: it is what makes a mid-session
  reload take effect on the very next call with no other plumbing.
- Keep every other server line unchanged: `dispatch::policy_check` / `dispatch::audit`, response
  shapes, the T04 bounded first-call wait (which now reads `store.current().first_call_wait_ms()`
  once per call, or the wiring G01 chose -- keep G01's wiring, just source the value from the
  per-call snapshot), and all JSON-RPC semantics.

Only the mcp-server role does this. `run_native_host_role` and the installer/config CLI
subcommands must NOT build a `ConfigStore` or spawn a watcher; do not touch `src/main.rs`.

### 7. Unit tests (inline `#[cfg(test)]` in `src/policy/reload.rs`)

Keep the pure logic testable with no filesystem and no environment mutation. The planner and the
settle function are pure; the store's swap and signal are driven through `notify_local_edit` /
constructed inputs, not through real files where avoidable. Required tests, by name and assertion:

Pure planner (the security rule):

1. `valid_reload_adopts_both_sources`: `plan_reload(Ok(org_a), Ok((user_a, warns)), &last_good_b)`
   yields inputs built from `org_a` + `user_a.values`, `new_last_good` equal to `org_a` + `user_a`,
   `org_failed == false`, `user_failed == false`, and the user warnings surfaced.
2. `invalid_user_keeps_last_good_user_and_warns`:
   `plan_reload(Ok(org_a), Err("bad user".into()), &last_good)` yields inputs whose user map equals
   `last_good.user`, `user_failed == true`, the failure recorded in `warnings` (NOT `errors`), and
   the org contribution taken from the fresh `org_a`.
3. `invalid_org_is_fail_closed`: construct `last_good` whose org has a mandatory entry
   (for example `audit.enabled = true`); `plan_reload(Err("bad org".into()), Ok((user_empty, [])),
   &last_good)` yields inputs whose `org_mandatory` STILL contains that entry (last-good org kept),
   `org_failed == true`, the failure recorded in `errors` (NOT `warnings`), and `new_last_good.org`
   equal to the last-good org (never dropped). Then `layers::resolve` + `Config::from_resolution`
   on those inputs must show the mandatory value STILL in force -- the posture did not weaken. This
   is THE security test; assert it end to end through the resolver.
4. `both_sources_invalid_keeps_both_last_good`: both `Err`; inputs equal `compose_inputs(last_good)`
   exactly; `org_failed && user_failed`; org failure in `errors`, user failure in `warnings`.

Store swap and signal:

5. `current_returns_last_swapped`: build a store (via a small test constructor that takes an
   initial `Config`), assert `current()` equals the initial; drive a reload that produces a
   different `Config` and assert `current()` now equals it, and the previously-held `Arc` is
   still valid and still holds the old value (immutable snapshot).
6. `generation_and_signal_fire_only_on_change` (`#[tokio::test]`): subscribe; a reload that
   produces the SAME config does not bump `generation()` and does not wake the receiver; a reload
   that produces a DIFFERENT config bumps `generation` by one and the receiver observes the new
   `Arc<Config>` via `changed()` + `borrow()`.
7. `no_receivers_reload_still_swaps`: drop all receivers, drive a changing reload, assert it still
   swaps and bumps generation (the `watch::send` error on no-receivers is ignored, not fatal).

Pure debounce:

8. `settle_debounces_until_stable`: from a `PathWatch` whose `last_applied` is `Some(fp0)`: a poll
   with a new `Some(fp1)` (differs from both last_seen and last_applied) does NOT fire (not yet
   stable); a second poll with the same `Some(fp1)` DOES fire (stable and changed) and updates
   `last_applied` to `fp1`; a third poll with `Some(fp1)` does NOT fire (equals last_applied).
9. `settle_detects_create_and_delete`: `None -> Some(fp)` (twice to settle) fires; `Some(fp) ->
   None` (twice to settle) fires; a fingerprint that only flickers for one poll does not fire.

Startup fail-loud (keep it hermetic; test the planner/reader boundary, not real platform paths):

10. `initial_load_is_fail_loud_on_org_error`: exercise the reader/planner path used at startup with
    an injected org `Err`, asserting the startup path returns `Err` (fail-loud) while the RELOAD
    path with the same `Err` returns a `ReloadReport` with `org_failed == true` and no error
    returned (keep-last-good). If your `load_initial` reads real paths, factor the fail-loud vs
    keep-last-good decision into a testable helper and assert on that helper, so the test needs no
    real files.

Provide a `#[cfg(test)]` constructor (for example `ConfigStore::for_test(initial: Config,
last_good: LastGoodInputs) -> Arc<ConfigStore>`) that seeds the store without touching the
filesystem, and, where a test must drive a reload deterministically, a `#[cfg(test)]` seam that
lets the test supply the org/user load results (for example a `reload_with(org, user)` that calls
`plan_reload` then the same swap/signal code as `reresolve`). Do not add new integration tests
under `tests/`; existing ones must pass unchanged.

## Constraints

Hard rules; every one applies.

1. ASCII only in ALL code and docs: no em-dashes, no arrows, no curly quotes, anywhere (comments,
   tests, strings). Use Rust `\u{..}` escapes if a test needs a non-ASCII input.
2. All-open stays first-class and byte-identical: with no manifest and default config, every tool
   result is exactly what stage 1 produced. `load_initial` with both files absent resolves to
   `Config::minimal()`; the watcher sees three absent (or unchanged) paths and never produces a
   different snapshot; the per-call `current()` always returns that snapshot; STEP-0 in
   `dispatch.rs` is untouched. `tests/mcp_protocol.rs` must pass unchanged.
3. NEVER modify the tool schemas (`src/mcp/schemas/tools.json`), tool names, params, or
   descriptions; `tests/tool_schema_fidelity.rs` must pass unchanged (ADR-0007, the sacred
   surface). A5 does not touch tool advertisement at all (that is G14).
4. The extension holds mechanism only: no policy, access, or redaction decisions in extension JS.
   A5 changes no file under `extension/`.
5. Rust 2021, `thiserror` for typed errors (A5 adds NO new error variant; it reuses
   `Error::Config` from G02 and logs rather than propagates on reload), doc comments on all public
   items and modules, `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean.
6. One task = one commit (code + tests + ledger/browser-test updates). Keep the tree green between
   tasks (full suite + clippy + fmt).
7. Windows dev gotcha: if `target/debug/browser-mcp.exe` is locked by a running session, rename it
   aside (`mv target/debug/browser-mcp.exe target/debug/browser-mcp.exe.old-1`) and rebuild, or
   stop the MCP client first.

Task-specific:

8. NO new runtime dependency. The swap slot is `Mutex<Arc<Config>>` (`std`); the watcher is a
   debounced mtime poll (`std::fs` + `tokio::time`); the signal is `tokio::sync::watch` (tokio is
   already a dependency). Do NOT add `arc-swap`, `notify`, `parking_lot`, `crossbeam`, or any other
   crate, and make no `Cargo.toml` change. (This deviates from the "one justified new dependency
   (notify)" option offered in the brief; the zero-dep choice is deliberate and justified in
   parts 1 and 4.)
9. Reload NEVER takes the running server down and NEVER weakens posture: `reresolve` does not
   return an error; an invalid org policy keeps the last-good org contribution and logs an ERROR;
   an invalid user config keeps the last-good user layer and logs a WARNING. This asymmetry is a
   security rule, not a preference. Startup (`load_initial`) is the one exception: it is fail-loud
   (an invalid org file or structurally broken user file at boot is fatal), because a server that
   has not started cannot serve a stale-but-safe snapshot.
10. Validate-then-swap: never store a candidate that did not parse and resolve cleanly. The swap
    slot only ever holds a snapshot built from validated resolution values.
11. The org policy path stays fixed and non-bypassable (shared format 1.2): A5 adds no flag,
    env var, or config key that relocates, disables, or skips the org file or the watcher.
12. stdout is reserved for the JSON-RPC stream. All warnings and errors go through `tracing`
    (stderr). The watcher and reload emit only `tracing` output, never `println!`.
13. Only the mcp-server role builds a `ConfigStore` and spawns the watcher. The native-host relay
    and the CLI subcommands must not. Do not touch `src/main.rs`.
14. Do NOT copy code from other projects; implement from the behavior described here.

## Verification

Run from the repository root:

1. `cargo fmt --check` passes.
2. `cargo clippy --all-targets -- -D warnings` passes.
3. `cargo test` passes, all green: the new `src/policy/reload.rs` unit tests (all named in part 7),
   `tests/tool_schema_fidelity.rs` unchanged, `tests/mcp_protocol.rs` unchanged, and every G01/G02
   test unchanged.
4. Grep the changed files for non-ASCII bytes (for example
   `rg -n "[^\x00-\x7F]" src/policy/reload.rs src/policy/mod.rs src/mcp/server.rs`); there must be
   none.
5. `git diff --stat` shows changes ONLY to: the new `src/policy/reload.rs`, `src/policy/mod.rs`
   (the `pub mod reload;` declaration and possibly a `PartialEq` derive on `Config` if G01 did not
   add it), and `src/mcp/server.rs` (the startup `ConfigStore` block and the per-call snapshot
   read). `src/main.rs`, `src/dispatch.rs`, `extension/`, and `src/mcp/schemas/tools.json` show no
   diff.
6. Manual hot-reload smoke (binary-only change; restart the MCP client to pick up the new binary;
   no extension reload needed). With the binary running and stderr visible:
   - Create the user config file at the platform path (`%APPDATA%\browser-mcp\config.json` on
     Windows) with `{"config": {"audit.enabled": true}}`. Within about one second, a
     `tracing` line shows a reload and no error; the new value is in force on the next tool call.
   - Overwrite the org policy file (`%ProgramData%\browser-mcp\policy.json`) with `not json`.
     Confirm an ERROR line and that the previous org posture is UNCHANGED (fail-closed): any
     org-locked value stays in force. Restore or delete the file; confirm the next reload recovers.
   - Overwrite the user config file with `not json`. Confirm a WARNING line and that the last-good
     user layer stays in force (the server keeps running). Delete the file; confirm the reload
     returns to defaults.
   - Delete both files; confirm behavior returns to byte-identical all-open (a normal session
     works exactly as before; `read_page` on a page with a password field still redacts under the
     safe default).
7. All-open regression: with no config files and no manifest, `tests/mcp_protocol.rs` (13 tools,
   byte-equal `tools/list`) passes unchanged, confirming the swap substrate did not perturb the
   ungoverned path.

## Out of scope

Fenced off; do not implement any of the following, even partially:

- The layered resolver, the five-layer precedence, value validation, file paths, and the two file
  parsers (G02). A5 CONSUMES `layers::resolve`, `Config::from_resolution`, `load::parse_org_config`,
  `load::parse_user_config`, and the path helpers; it does not re-implement them. If they are not in
  the tree, STOP (see the PREREQUISITE CHECK).
- The typed key registry growth and the owned `Config` (G01). A5 does not add or change keys or
  accessors; it swaps whole `Config` snapshots.
- The config CLI (G03). A5 does not add a `config` subcommand; `config set` is a separate process
  whose trigger reaches a running server through the file-watch, not an in-process call.
- The manifest engine (G12): no manifest parsing, no `Grant` type, no source-selection
  (`--manifest` / env / org-embedded), no manifest content hash, no manifest identity. A5 only
  leaves the `WatchSources.manifest` slot and a marked integration point for G12 to wire.
- The actual `notifications/tools/list_changed` emit and any tool-advertisement filtering (G14).
  A5 provides the `subscribe()` change signal ONLY; deciding whether the advertised tool set
  changed and emitting the MCP notification is G14.
- The native-messaging settings protocol and the extension options page (shared format section 9).
  A5 exposes `notify_local_edit()` as the in-process re-resolve hook that surface will call when it
  lands; A5 does not implement the protocol or touch `src/native/` or `extension/`.
- Enforcement, denials, sacred domains, shadow mode, audit records (later phases). A5 changes no
  dispatch decision or audit logic; the per-call snapshot read is the only dispatch-adjacent change,
  and it is behavior-preserving.
- The persistent-service split (Phase B of the architecture doc), any daemon lifecycle, idle
  shutdown, or the control-plane listener. A5 runs entirely inside today's single mcp-server
  process. (The `ReloadReport` is shaped so a future control-plane can report reload status, but
  A5 wires no control plane.)
- Adding `arc-swap`, `notify`, or any other dependency, and any `Cargo.toml` change.
