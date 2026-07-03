//! The ADR-0019 hot-reload substrate: the in-force resolved [`Config`] held behind an atomic
//! swap, a validate-then-swap re-resolve, a debounced file-watch on the three configuration
//! sources (user config file, org policy file, active manifest source), and a change signal
//! for the tool-advertisement layer (G14). The source-specific invalid-on-reload rule (lenient
//! user, fail-closed org) is a SECURITY rule, not a preference: a malformed org push never
//! drops an org lock or relaxes a value to a weaker layer.
//!
//! ADR-0025 extends this same substrate to the MANIFEST: [`ConfigStore::reresolve`] now also
//! performs a full org+user manifest re-selection on every settled change
//! ([`crate::governance::manifest::source::load_policy`], ADR-0023's single loader), publishing
//! the result on its own `watch::channel<Arc<LoadedPolicy>>` ([`ConfigStore::policy`]) using the
//! exact same keep-last-good, fail-closed-on-error posture the config layers already have. One
//! `load_policy` result feeds BOTH consumers: the org config layers (as before) and the
//! published policy (new). The subscription that turns a published policy into a live
//! `Governance` swap, a `list_changed` notification, and the two new session events lives in
//! `transport::mcp::server` (constraint 3: this module stays domain-agnostic and holds only the
//! channel and the store).
//!
//! The swap slot is `Mutex<Arc<Config>>`, not `ArcSwap`: the read is a per-call event on the
//! dispatch chokepoint, not a hot inner loop, and the critical section is a single `Arc` clone
//! (an atomic refcount bump) followed by an immediate unlock, so reads never contend for more
//! than a few nanoseconds. `Mutex<Arc<Config>>` is `std`-only and needs zero new dependencies,
//! preserving the single-binary / zero-runtime-dependencies posture (ADR-0001); `arc-swap`
//! would be a second new crate for a lock-free property this call site does not need.
//!
//! The watcher is a zero-dependency debounced mtime poll, not the `notify` crate: it watches
//! exactly three known file paths (not recursive directory trees) that change rarely, so
//! polling `std::fs::metadata` on three paths every [`POLL_INTERVAL`] is negligible cost and
//! needs no new crate. `notify` pulls a platform-backend dependency tree disproportionate for
//! three files. The watcher is written behind a small abstraction so `notify` would be a
//! drop-in replacement without touching [`ConfigStore::reresolve`] if sub-second latency ever
//! matters.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;
use tokio::sync::watch;

use super::load::{OrgConfig, UserConfig};
use super::{layers, load, Config};

/// Poll interval for the source watcher. The config files change rarely (a user edit, a
/// `config set`, or an MDM push), so a sub-second poll on three known paths is negligible cost.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

/// The in-force resolved configuration, held behind a single swappable slot so a re-resolve
/// replaces it atomically and every subsequent per-call read sees the new snapshot. Also holds
/// the last-good layer inputs the reloader falls back to (fail-closed for org policy) and the
/// change-signal channel G14 subscribes to.
pub struct ConfigStore {
    /// The in-force snapshot. A per-call read clones the `Arc` and releases the lock
    /// immediately; a reload stores a fresh `Arc` in one operation.
    snapshot: Mutex<Arc<Config>>,
    /// Monotonic reload generation; bumped on every successful swap. Lets a subscriber cheaply
    /// answer "did the snapshot change since I last looked".
    generation: AtomicU64,
    /// Broadcasts the new snapshot on every successful swap. G14 subscribes here to recompute
    /// the advertised tool set and emit `list_changed` when it differs.
    tx: watch::Sender<Arc<Config>>,
    /// Last successfully-applied layer inputs per source, retained so an invalid reload of one
    /// source keeps that source's last-good contribution.
    last_good: Mutex<LastGoodInputs>,
    /// The three fixed source paths watched for change.
    sources: WatchSources,
    /// Validates `content.security.sacred_domains` entries. Supplied by the caller (the
    /// browser plugin's real pattern-syntax checker) since this core module cannot name the
    /// browser plugin directly (the a7 arch-test forbids a `governance -> browser` edge; see
    /// RECONCILIATION.md section 2, the same integration point G01/G02 resolved).
    domain_pattern_valid: fn(&str) -> bool,
    /// The resolved user-supplied manifest source string (ADR-0025 Decision 1), retained so
    /// [`Self::reresolve`] can re-run the FULL org+user selection on every reload event instead
    /// of only re-reading the org file. `None` when no `--manifest`/`GHOSTLIGHT_MANIFEST` was
    /// given at startup (an `env://` source is retained here too, even though it has no file to
    /// watch, since selection itself must still be re-run on every org-file change).
    user_source: Option<String>,
    /// The in-force resolved policy snapshot (ADR-0025 Decision 2): mirrors `snapshot`'s
    /// `Mutex<Arc<T>>` idiom exactly, so a policy swap is decided and applied the same way a
    /// config swap is (`apply_plan`'s `changed` check, transposed to manifest identity).
    policy_snapshot: Mutex<Arc<crate::governance::manifest::source::LoadedPolicy>>,
    /// Broadcasts the new policy on every successful publish. `transport::mcp::server`'s
    /// policy-subscription task subscribes here to rebuild `Governance`, emit `list_changed`
    /// when the advertised set changed, and record the two ADR-0025 session events.
    policy_tx: watch::Sender<Arc<crate::governance::manifest::source::LoadedPolicy>>,
}

/// The last-good layer inputs, per source. On a reload where one source fails to load or
/// validate, the store re-composes from these so a failed source never weakens the resolved
/// posture (this is what makes org-policy failure fail-closed).
#[derive(Debug, Clone, Default)]
struct LastGoodInputs {
    /// Last-good org contribution (mandatory + recommended maps). Never dropped on an invalid
    /// org reload.
    org: OrgConfig,
    /// Last-good user-layer values.
    user: serde_json::Map<String, serde_json::Value>,
    /// Last-good declared preset name (G18), or `None` when no preset is declared. Retained
    /// the same way as `user`: a structurally-failed user file on reload keeps this too, so a
    /// transient bad edit never silently drops the preset selection.
    preset: Option<String>,
}

/// The fixed source paths the watcher polls. The manifest slot (ADR-0025 Decision 1) is the
/// user-supplied `file://` manifest path, set at construction whenever one was given at
/// startup -- INDEPENDENT of whether that source actually won selection (org always wins when
/// both are present, but an ignored user file must still be watched so the org-deletion
/// fallback stays live and a later edit to the user file reloads). An `env://` source, or no
/// user source at all, has no file to watch and leaves this `None`.
#[derive(Debug, Clone)]
struct WatchSources {
    user_config: Option<PathBuf>,
    org_policy: PathBuf,
    manifest: Option<PathBuf>,
}

impl ConfigStore {
    /// The current in-force snapshot. This is the per-call read on the dispatch path: clone
    /// the `Arc` (cheap) and use it for the whole call, so a reload mid-call does not tear the
    /// snapshot the call already started with.
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

    /// Subscribe to snapshot changes. The receiver observes the new `Arc<Config>` after every
    /// successful swap. G14 uses this to recompute the advertised tool set and (when it
    /// changed) emit `notifications/tools/list_changed`; this store fires the signal on every
    /// config change, deciding whether the TOOL SET changed is G14's job. A subscriber created
    /// before any reload sees the startup snapshot as its current value and only wakes on
    /// subsequent swaps. This module never emits any MCP notification itself.
    pub fn subscribe(&self) -> watch::Receiver<Arc<Config>> {
        self.tx.subscribe()
    }

    /// [`Self::load_initial_with_policy`] with an all-open policy (no manifest from any
    /// origin) and no user manifest source to watch. Kept as a zero-argument-beyond-checker
    /// convenience for callers with no manifest to thread through.
    pub fn load_initial(domain_pattern_valid: fn(&str) -> bool) -> crate::Result<Arc<ConfigStore>> {
        let all_open = crate::governance::manifest::source::LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        };
        Self::load_initial_with_policy(domain_pattern_valid, &all_open, None)
    }

    /// Build the store from the initial layered load, called once at mcp-server startup.
    /// Startup keeps the G02 FAIL-LOUD semantics: an invalid org policy file or a structurally
    /// broken user file at startup is a hard error and the server refuses to start (it must
    /// never boot open on a broken org push). The lenient, keep-last-good behavior is for
    /// RELOAD only ([`Self::reresolve`]), where a server is already running on a known-good
    /// snapshot. `domain_pattern_valid` validates `content.security.sacred_domains` entries
    /// (the browser plugin's real pattern-syntax checker); it is retained for every later
    /// reload.
    ///
    /// `loaded_policy` is the manifest already resolved once at startup by
    /// `governance::manifest::source::load_policy` (ADR-0023 Decision 1: `parse_manifest` is
    /// the sole reader/parser/validator of the policy file; this function never re-reads or
    /// re-parses it). Its org-sourced config entries populate the org layers (via
    /// `load::org_config_from_policy`); a user-supplied manifest's entries always land at the
    /// user layer instead (G12, shared format doc section 1.3 rule 2, regardless of their
    /// declared `level`), merged UNDER the user config FILE's own values so the file wins on a
    /// key collision (`config.json` is the user's own direct, immediate expression of
    /// preference, while a `--manifest` source is more likely an external or automated input).
    ///
    /// `user_source` (ADR-0025 Decision 1) is the SAME resolved `--manifest`/
    /// `GHOSTLIGHT_MANIFEST` source string `load_policy` above was given, retained so
    /// [`Self::reresolve`] can re-run the full org+user selection on every reload event. It also
    /// sets the watcher's manifest slot to the user source's PATH whenever it resolves to a
    /// `file://` (or bare-path) source -- independent of which origin actually won selection, so
    /// an ignored user file is still watched. `None` when no user source was given at all;
    /// callers with nothing to watch (the CLI, `doctor`, `load_initial`) pass `None`.
    pub fn load_initial_with_policy(
        domain_pattern_valid: fn(&str) -> bool,
        loaded_policy: &crate::governance::manifest::source::LoadedPolicy,
        user_source: Option<String>,
    ) -> crate::Result<Arc<ConfigStore>> {
        let manifest_watch_path = user_source.as_deref().and_then(|s| {
            match crate::governance::manifest::source::parse_source_string(s) {
                Ok(crate::governance::manifest::source::UserSource::FilePath(path)) => Some(path),
                _ => None,
            }
        });
        let sources = WatchSources {
            user_config: load::user_config_path(),
            org_policy: load::org_policy_path(),
            manifest: manifest_watch_path,
        };

        let org: crate::Result<OrgConfig> = Ok(load::org_config_from_policy(loaded_policy));
        let manifest_user_config =
            crate::governance::manifest::source::manifest_config_as_user_layer(loaded_policy);
        let user = read_and_parse_user(sources.user_config.as_deref(), domain_pattern_valid);
        let (mut last_good, warnings) = compose_initial(org, user)?;

        last_good.user = merge_manifest_user_config(manifest_user_config, last_good.user);

        for w in &warnings {
            tracing::warn!("config: {w}");
        }

        let inputs = compose_inputs(&last_good);
        let resolution = layers::resolve(&inputs);
        let config = Arc::new(Config::from_resolution(&resolution));

        let (tx, _rx) = watch::channel(config.clone());
        let policy_snapshot = Arc::new(loaded_policy.clone());
        let (policy_tx, _policy_rx) = watch::channel(policy_snapshot.clone());
        Ok(Arc::new(ConfigStore {
            snapshot: Mutex::new(config),
            generation: AtomicU64::new(0),
            tx,
            last_good: Mutex::new(last_good),
            sources,
            domain_pattern_valid,
            user_source,
            policy_snapshot: Mutex::new(policy_snapshot),
            policy_tx,
        }))
    }

    /// Subscribe to policy changes (ADR-0025 Decision 2): the receiver observes the new
    /// `Arc<LoadedPolicy>` after every successful publish (a settled, identity-changing reload
    /// of the org policy file or a watched user `file://` manifest source). A subscriber
    /// created before any reload sees the startup policy as its current value and only wakes on
    /// subsequent publishes. `transport::mcp::server`'s policy-subscription task uses this to
    /// rebuild `Governance`, emit `list_changed` when the advertised set changed, and record the
    /// two ADR-0025 session events; this store never emits any MCP notification or audit record
    /// itself.
    pub fn policy(
        &self,
    ) -> watch::Receiver<Arc<crate::governance::manifest::source::LoadedPolicy>> {
        self.policy_tx.subscribe()
    }

    /// Re-run the layered load and resolver and, only if a full candidate parses and
    /// validates, swap it into the snapshot slot. This is validate-then-swap: a half-written or
    /// invalid file never becomes the in-force snapshot. Applies the source-specific rule via
    /// [`plan_reload`]: a failed user source keeps the last-good user layer (WARN); a failed
    /// org source keeps the last-good org layer (ERROR, fail-closed). Returns a report for
    /// logging, the control-plane, and tests. Never returns an error: a running server is never
    /// taken down by a reload; it keeps its last-good snapshot.
    ///
    /// ADR-0025 Decision 3: this now also performs the FULL manifest re-selection (org file +
    /// the watched user `file://` source, exactly the same [`crate::governance::manifest::
    /// source::load_policy`] the startup path uses), re-evaluating the org-wins rule on every
    /// call so an org-file creation/deletion mid-session transitions exactly as startup would.
    /// One `load_policy` result feeds BOTH the org config layers (as before, via
    /// [`load::org_config_from_policy`]) and the published policy: a failure of that single
    /// call -- an invalid org file, OR a configured user `file://` source that has gone missing
    /// -- is treated as an org-slot failure (keep-last-good, ERROR), never a transition; only an
    /// actual `Ok` result (including a resolved all-open `LoadedPolicy` after an org-file
    /// deletion) can change what is published.
    pub fn reresolve(&self) -> ReloadReport {
        let policy_result = crate::governance::manifest::source::load_policy(
            self.user_source.as_deref(),
            self.domain_pattern_valid,
        )
        .map_err(|e| e.to_string());
        let user = read_and_parse_user(
            self.sources.user_config.as_deref(),
            self.domain_pattern_valid,
        )
        .map_err(|e| e.to_string());

        self.apply_policy_and_config(policy_result, user)
    }

    /// The shared core of [`Self::reresolve`] and the test-only [`Self::reload_with_policy`]:
    /// given a (possibly injected) `load_policy` result and user-config result, derive the org
    /// config layers from the SAME policy result (one parse feeds both consumers, ADR-0025
    /// Decision 3), fold in a user-sourced manifest's own config entries (mirroring
    /// [`Self::load_initial_with_policy`]'s startup merge, so an edit to a watched user
    /// manifest's config entries takes effect on reload too), apply the config swap via the
    /// existing [`plan_reload`]/[`Self::apply_plan`] machinery, and -- only on a successful
    /// policy result -- publish it if its manifest identity changed.
    fn apply_policy_and_config(
        &self,
        policy_result: Result<crate::governance::manifest::source::LoadedPolicy, String>,
        user: Result<(UserConfig, Vec<String>), String>,
    ) -> ReloadReport {
        let org = policy_result
            .as_ref()
            .map(load::org_config_from_policy)
            .map_err(Clone::clone);

        let last_good = self
            .last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut plan = plan_reload(org, user, &last_good);

        if let Ok(loaded_policy) = &policy_result {
            let manifest_user =
                crate::governance::manifest::source::manifest_config_as_user_layer(loaded_policy);
            plan.inputs.user = merge_manifest_user_config(manifest_user.clone(), plan.inputs.user);
            plan.new_last_good.user =
                merge_manifest_user_config(manifest_user, plan.new_last_good.user);
        }

        let report = self.apply_plan(plan);

        if let Ok(loaded_policy) = policy_result {
            self.maybe_publish_policy(loaded_policy);
        }

        report
    }

    /// Publish `loaded_policy` on the policy channel iff its manifest IDENTITY differs from
    /// what is currently in force (ADR-0025 Decision 3/4). A `load_policy` failure never reaches
    /// here: [`Self::apply_policy_and_config`] only calls this on `Ok`, so a failed reload keeps
    /// the last-good published policy exactly like it keeps the last-good config layers.
    fn maybe_publish_policy(
        &self,
        loaded_policy: crate::governance::manifest::source::LoadedPolicy,
    ) {
        let mut guard = self
            .policy_snapshot
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if manifest_identity_changed(&guard, &loaded_policy) {
            let new = Arc::new(loaded_policy);
            *guard = Arc::clone(&new);
            drop(guard);
            // watch::send only errs if there are no receivers, which is fine.
            let _ = self.policy_tx.send(new);
        }
    }

    /// Trigger an immediate re-resolve now, bypassing the poll interval. This is the hook for
    /// IN-PROCESS config writers: the future options-page settings protocol (native-messaging
    /// `set_config_key`) calls this so an edit takes effect immediately. `config set` (G03)
    /// runs in a SEPARATE CLI process and writes the file, so ITS trigger is the file-watch
    /// seeing the write, not this method.
    pub fn notify_local_edit(&self) -> ReloadReport {
        self.reresolve()
    }

    /// Apply a reload plan: log its messages, resolve and build the candidate `Config`, retain
    /// the new last-good inputs, and swap the snapshot only if it changed.
    fn apply_plan(&self, plan: ReloadPlan) -> ReloadReport {
        for w in &plan.warnings {
            tracing::warn!("config reload: {w}");
        }
        for e in &plan.errors {
            tracing::error!("config reload: {e}");
        }

        // "Validate" = the candidate parsed and resolved cleanly. Resolution values are
        // already validated by the loaders, so from_resolution cannot fail.
        let resolution = layers::resolve(&plan.inputs);
        let candidate = Arc::new(Config::from_resolution(&resolution));

        // Retain the new last-good regardless of swap (a failed source contributed its own
        // last-good back into the plan, so this never weakens org posture).
        *self
            .last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = plan.new_last_good;

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

    /// Spawn the debounced source watcher. mcp-server role ONLY (the native-host relay and the
    /// installer/config CLI roles must never start it). Polls the three source fingerprints
    /// every [`POLL_INTERVAL`]; when any source settles on a changed fingerprint, calls
    /// [`Self::reresolve`] once. Runs until the process exits. Takes `Arc<Self>` so the loop
    /// holds a strong reference to the store.
    pub fn spawn_watcher(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(POLL_INTERVAL);
            let mut watches: [PathWatch; 3] = Default::default();
            // Seed last_applied with the current fingerprints so the first poll does not
            // spuriously re-resolve the state we already loaded at startup.
            let paths = self.watched_paths();
            for (i, p) in paths.iter().enumerate() {
                let fp = p.as_deref().and_then(fingerprint);
                watches[i] = PathWatch {
                    last_seen: fp,
                    last_applied: fp,
                };
            }
            loop {
                interval.tick().await;
                let paths = self.watched_paths(); // the manifest slot may change under G12
                let mut trigger = false;
                for (i, p) in paths.iter().enumerate() {
                    let fp = p.as_deref().and_then(fingerprint);
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

    /// The three watched paths in fixed order [user, org, manifest]; a `None` slot (no user
    /// config dir, or no file-based manifest source) is simply never a change. Recomputed each
    /// poll so a G12 manifest-source change is picked up.
    fn watched_paths(&self) -> [Option<PathBuf>; 3] {
        [
            self.sources.user_config.clone(),
            Some(self.sources.org_policy.clone()),
            self.sources.manifest.clone(),
        ]
    }
}

/// The pure reload plan: given fresh load attempts for each source and the current last-good
/// inputs, decide the new layer inputs, the new last-good, and the per-source outcome. No I/O.
/// This function encodes the security rule:
///
/// - User source Ok  -> adopt its values (and as new last-good); its per-entry warnings are
///   surfaced (WARN, not error).
/// - User source Err -> keep last-good user values; the structural failure is a WARNING (a
///   user file is user-serviceable; a broken one is stale, not fatal, once the server is
///   already running).
/// - Org source Ok   -> adopt it (and as new last-good).
/// - Org source Err  -> KEEP last-good org for BOTH the applied inputs and the new last-good,
///   and record an ERROR. FAIL-CLOSED: a malformed org push never drops an org lock or relaxes
///   an org value to a weaker layer. An org policy that silently fails open is worse than a
///   stale one.
fn plan_reload(
    org: Result<OrgConfig, String>,
    user: Result<(UserConfig, Vec<String>), String>,
    last_good: &LastGoodInputs,
) -> ReloadPlan {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    let (org_result, org_failed) = match org {
        Ok(o) => (o, false),
        Err(e) => {
            errors.push(format!(
                "org policy failed to load/validate, keeping last-good: {e}"
            ));
            (last_good.org.clone(), true)
        }
    };

    let (user_values, preset_name, user_failed) = match user {
        Ok((parsed, entry_warnings)) => {
            warnings.extend(entry_warnings);
            (parsed.values, parsed.preset, false)
        }
        Err(e) => {
            warnings.push(format!(
                "user config failed to load, keeping last-good: {e}"
            ));
            (last_good.user.clone(), last_good.preset.clone(), true)
        }
    };

    let new_last_good = LastGoodInputs {
        org: org_result.clone(),
        user: user_values.clone(),
        preset: preset_name.clone(),
    };
    let inputs = load::layer_inputs(org_result, user_values, preset_name.as_deref());

    ReloadPlan {
        inputs,
        new_last_good,
        warnings,
        errors,
        org_failed,
        user_failed,
    }
}

/// The outcome of [`plan_reload`]: the inputs to resolve, the new last-good to retain, and
/// human-readable messages split by severity.
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

/// The result of a re-resolve, for logging, the control-plane, and tests.
#[derive(Debug, Clone)]
pub struct ReloadReport {
    /// True if a new, different snapshot was swapped in.
    pub swapped: bool,
    /// True if the org+user manifest re-selection ([`crate::governance::manifest::source::
    /// load_policy`]) failed to load/validate (last-good config layers AND last-good published
    /// policy both kept). Covers an invalid org file, a broken user-supplied manifest, or a
    /// configured user `file://` source that has gone missing (ADR-0025 Decision 1: a missing
    /// CONFIGURED source is a load error, not a transition to all-open).
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

/// Merge a user-supplied manifest's `config` entries under the user config FILE's own values
/// (G12, shared format doc section 1.3 rule 2): the file's entries are inserted last, so they
/// win on a key collision. Pure, so the precedence rule is testable without touching real
/// files or the manifest engine.
fn merge_manifest_user_config(
    manifest_user: serde_json::Map<String, serde_json::Value>,
    file_user: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    if manifest_user.is_empty() {
        return file_user;
    }
    let mut merged = manifest_user;
    merged.extend(file_user);
    merged
}

/// Build the layer inputs from the last-good state (used by startup and when every source
/// fails on reload). Delegates to [`load::layer_inputs`], the same composition [`plan_reload`]
/// and [`ConfigStore::load_initial_with_policy`] (the mcp-server startup path) use.
fn compose_inputs(last_good: &LastGoodInputs) -> layers::LayerInputs {
    load::layer_inputs(
        last_good.org.clone(),
        last_good.user.clone(),
        last_good.preset.as_deref(),
    )
}

/// The startup composition: given the raw org/user load results, compose the initial last-good
/// state or fail loud. Fail-loud on ANY error (org or user structural failure): a server that
/// has not started cannot serve a stale-but-safe snapshot. Factored out so the fail-loud
/// decision is testable without touching real files (contrast [`plan_reload`], which is the
/// keep-last-good decision used once the server is already running).
fn compose_initial(
    org: crate::Result<OrgConfig>,
    user: crate::Result<(UserConfig, Vec<String>)>,
) -> crate::Result<(LastGoodInputs, Vec<String>)> {
    let org = org?;
    let (user, warnings) = user?;
    let last_good = LastGoodInputs {
        org,
        user: user.values,
        preset: user.preset,
    };
    Ok((last_good, warnings))
}

/// Pure comparison of two [`LoadedPolicy`](crate::governance::manifest::source::LoadedPolicy)
/// values' manifest IDENTITY (ADR-0025 Decision 3/4): name, version, hash, and origin together.
/// A present<->absent transition on either side always counts as changed (an absent manifest
/// has no name/version/hash/origin to compare, so it is its own distinct identity). Used by
/// [`ConfigStore::maybe_publish_policy`] to decide whether a re-selected policy is actually a
/// NEW state worth publishing, versus e.g. the same org file re-read byte-identical.
fn manifest_identity_changed(
    old: &crate::governance::manifest::source::LoadedPolicy,
    new: &crate::governance::manifest::source::LoadedPolicy,
) -> bool {
    let key = |p: &crate::governance::manifest::source::LoadedPolicy| {
        p.manifest
            .as_ref()
            .map(|m| (m.name.clone(), m.version.clone(), m.hash.clone(), p.origin))
    };
    key(old) != key(new)
}

/// Read and parse the user config file. `None` path, or `ErrorKind::NotFound`, is normal
/// (absence yields the empty default); any other I/O error is a hard error.
fn read_and_parse_user(
    path: Option<&Path>,
    domain_pattern_valid: fn(&str) -> bool,
) -> crate::Result<(UserConfig, Vec<String>)> {
    let Some(path) = path else {
        return Ok((UserConfig::default(), Vec::new()));
    };
    match std::fs::read_to_string(path) {
        Ok(content) => {
            load::parse_user_config(&content, &path.display().to_string(), domain_pattern_valid)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok((UserConfig::default(), Vec::new()))
        }
        Err(e) => Err(crate::Error::Config(format!("{}: {e}", path.display()))),
    }
}

/// A cheap change fingerprint for a watched path: `None` when the file is absent, or
/// `(mtime, len)` when present. Absence is a distinct state, so a file being created or deleted
/// is detected, not just modified-in-place.
type Fingerprint = Option<(SystemTime, u64)>;

/// Compute the current fingerprint of a path. A metadata or mtime error is treated as absence
/// (`None`): an unreadable file is handled by the re-resolve's strict IO-error path, not by the
/// fingerprint.
fn fingerprint(path: &Path) -> Fingerprint {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())))
}

/// Per-path watch state: the last fingerprint the loop saw, and the fingerprint that was in
/// force at the last applied re-resolve.
#[derive(Debug, Clone, Copy, Default)]
struct PathWatch {
    last_seen: Fingerprint,
    last_applied: Fingerprint,
}

/// Decide whether a path's change has SETTLED and should trigger a re-resolve. Debounce rule: a
/// change fires only once the current fingerprint (a) differs from `last_applied` (something
/// changed since we last resolved) AND (b) equals the immediately previous poll's fingerprint
/// (the file has stopped changing). This coalesces the multiple writes an editor or an MDM push
/// emits and lets the validate-then-swap backstop catch any half-written state that still slips
/// through. Returns the new `PathWatch` and whether to trigger.
fn settle(prev: &PathWatch, current: Fingerprint) -> (PathWatch, bool) {
    let stable = current == prev.last_seen;
    let changed = current != prev.last_applied;
    if stable && changed {
        (
            PathWatch {
                last_seen: current,
                last_applied: current,
            },
            true,
        )
    } else {
        (
            PathWatch {
                last_seen: current,
                last_applied: prev.last_applied,
            },
            false,
        )
    }
}

#[cfg(test)]
impl ConfigStore {
    /// Crate-visible test constructor for other modules' test suites (for example
    /// `transport::mcp::server`'s server-wiring tests): seeds a store at `config` with empty
    /// last-good inputs, touching no filesystem. `LastGoodInputs` stays private to this module,
    /// so this is the seam other modules use instead.
    pub(crate) fn for_test_with_config(config: Config) -> Arc<ConfigStore> {
        Self::for_test(
            config,
            LastGoodInputs {
                org: OrgConfig::default(),
                user: serde_json::Map::new(),
                preset: None,
            },
        )
    }

    /// Test-only constructor: seeds the store without touching the filesystem. The policy
    /// channel seeds at all-open (no manifest from any origin), matching `load_initial`'s own
    /// convenience default.
    fn for_test(initial: Config, last_good: LastGoodInputs) -> Arc<ConfigStore> {
        let config = Arc::new(initial);
        let (tx, _rx) = watch::channel(config.clone());
        let all_open = Arc::new(crate::governance::manifest::source::LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        });
        let (policy_tx, _policy_rx) = watch::channel(all_open.clone());
        Arc::new(ConfigStore {
            snapshot: Mutex::new(config),
            generation: AtomicU64::new(0),
            tx,
            last_good: Mutex::new(last_good),
            sources: WatchSources {
                user_config: None,
                org_policy: PathBuf::new(),
                manifest: None,
            },
            domain_pattern_valid: |_| true,
            user_source: None,
            policy_snapshot: Mutex::new(all_open),
            policy_tx,
        })
    }

    /// Test-only constructor that ALSO seeds `user_source`, for the one ADR-0025 test
    /// (`user_manifest_deletion_keeps_last_good`) that needs `reresolve` to perform a REAL
    /// `source::load_policy` call against a controllable user `file://` path, rather than the
    /// fully injected [`Self::reload_with_policy`] seam every other test in this module uses.
    /// Otherwise identical to [`Self::for_test`] with `Config::minimal()` and empty last-good.
    fn for_test_with_user_source(user_source: String) -> Arc<ConfigStore> {
        let config = Arc::new(Config::minimal());
        let (tx, _rx) = watch::channel(config.clone());
        let all_open = Arc::new(crate::governance::manifest::source::LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        });
        let (policy_tx, _policy_rx) = watch::channel(all_open.clone());
        Arc::new(ConfigStore {
            snapshot: Mutex::new(config),
            generation: AtomicU64::new(0),
            tx,
            last_good: Mutex::new(LastGoodInputs::default()),
            sources: WatchSources {
                user_config: None,
                org_policy: load::org_policy_path(),
                manifest: None,
            },
            domain_pattern_valid: |_| true,
            user_source: Some(user_source),
            policy_snapshot: Mutex::new(all_open),
            policy_tx,
        })
    }

    /// Test-only: drive a reload deterministically with injected org/user load results,
    /// bypassing the filesystem reads `reresolve` performs.
    fn reload_with(
        &self,
        org: Result<OrgConfig, String>,
        user: Result<(UserConfig, Vec<String>), String>,
    ) -> ReloadReport {
        let last_good = self
            .last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let plan = plan_reload(org, user, &last_good);
        self.apply_plan(plan)
    }

    /// Test-only: drive the FULL ADR-0025 reload flow (config layers derived from the SAME
    /// `load_policy` result, plus the policy-channel publish decision) with an injected result,
    /// bypassing the real `source::load_policy` call (which reads the fixed platform org-policy
    /// path) so tests can exercise keep-last-good/publish decisions without touching real files
    /// or environment state shared across the whole test binary.
    fn reload_with_policy(
        &self,
        policy: Result<crate::governance::manifest::source::LoadedPolicy, String>,
        user: Result<(UserConfig, Vec<String>), String>,
    ) -> ReloadReport {
        self.apply_policy_and_config(policy, user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn manifest_user_config_merge_is_empty_when_manifest_contributes_nothing() {
        let file_user = serde_json::Map::from_iter([("a".to_string(), json!(1))]);
        let merged = merge_manifest_user_config(serde_json::Map::new(), file_user.clone());
        assert_eq!(merged, file_user);
    }

    #[test]
    fn manifest_user_config_merge_adds_manifest_only_keys() {
        let manifest_user =
            serde_json::Map::from_iter([("audit.enabled".to_string(), json!(true))]);
        let file_user = serde_json::Map::from_iter([("a".to_string(), json!(1))]);
        let merged = merge_manifest_user_config(manifest_user, file_user);
        assert_eq!(merged.get("audit.enabled"), Some(&json!(true)));
        assert_eq!(merged.get("a"), Some(&json!(1)));
    }

    #[test]
    fn manifest_user_config_merge_the_config_file_wins_on_collision() {
        let manifest_user = serde_json::Map::from_iter([("a".to_string(), json!("from-manifest"))]);
        let file_user = serde_json::Map::from_iter([("a".to_string(), json!("from-file"))]);
        let merged = merge_manifest_user_config(manifest_user, file_user);
        assert_eq!(merged.get("a"), Some(&json!("from-file")));
    }

    #[test]
    fn valid_reload_adopts_both_sources() {
        let last_good = LastGoodInputs {
            org: OrgConfig {
                mandatory: serde_json::Map::from_iter([("x".to_string(), json!("old"))]),
                recommended: serde_json::Map::new(),
            },
            user: serde_json::Map::from_iter([("y".to_string(), json!("old"))]),
            preset: None,
        };
        let org_a = OrgConfig {
            mandatory: serde_json::Map::from_iter([("a".to_string(), json!(1))]),
            recommended: serde_json::Map::from_iter([("b".to_string(), json!(2))]),
        };
        let user_a = UserConfig {
            preset: Some("fully_open".to_string()),
            values: serde_json::Map::from_iter([("c".to_string(), json!(3))]),
        };
        let warns = vec!["some warning".to_string()];

        let plan = plan_reload(
            Ok(org_a.clone()),
            Ok((user_a.clone(), warns.clone())),
            &last_good,
        );
        assert!(!plan.org_failed);
        assert!(!plan.user_failed);
        assert_eq!(plan.inputs.org_mandatory, org_a.mandatory);
        assert_eq!(plan.inputs.org_recommended, org_a.recommended);
        assert_eq!(plan.inputs.user, user_a.values);
        assert_eq!(
            plan.inputs.preset,
            super::super::preset_layer(super::super::Preset::FullyOpen)
        );
        assert_eq!(plan.new_last_good.org, org_a);
        assert_eq!(plan.new_last_good.user, user_a.values);
        assert_eq!(plan.new_last_good.preset, Some("fully_open".to_string()));
        assert_eq!(plan.warnings, warns);
        assert!(plan.errors.is_empty());
    }

    #[test]
    fn invalid_user_keeps_last_good_user_and_preset_and_warns() {
        let last_good = LastGoodInputs {
            org: OrgConfig::default(),
            user: serde_json::Map::from_iter([("keep".to_string(), json!(true))]),
            preset: Some("restricted".to_string()),
        };
        let org_a = OrgConfig {
            mandatory: serde_json::Map::from_iter([("m".to_string(), json!(1))]),
            recommended: serde_json::Map::new(),
        };
        let plan = plan_reload(Ok(org_a.clone()), Err("bad user".to_string()), &last_good);
        assert!(plan.user_failed);
        assert!(!plan.org_failed);
        assert_eq!(plan.inputs.user, last_good.user);
        assert_eq!(plan.inputs.org_mandatory, org_a.mandatory);
        assert_eq!(
            plan.inputs.preset,
            super::super::preset_layer(super::super::Preset::Restricted)
        );
        assert_eq!(plan.new_last_good.preset, Some("restricted".to_string()));
        assert!(plan.warnings.iter().any(|w| w.contains("bad user")));
        assert!(plan.errors.is_empty());
    }

    #[test]
    fn invalid_org_is_fail_closed() {
        let last_good = LastGoodInputs {
            org: OrgConfig {
                mandatory: serde_json::Map::from_iter([(
                    super::super::AUDIT_ENABLED.to_string(),
                    json!(true),
                )]),
                recommended: serde_json::Map::new(),
            },
            user: serde_json::Map::new(),
            preset: None,
        };
        let plan = plan_reload(
            Err("bad org".to_string()),
            Ok((UserConfig::default(), Vec::new())),
            &last_good,
        );
        assert!(plan.org_failed);
        assert!(!plan.user_failed);
        assert_eq!(
            plan.inputs.org_mandatory.get(super::super::AUDIT_ENABLED),
            Some(&json!(true))
        );
        assert!(plan.errors.iter().any(|e| e.contains("bad org")));
        assert!(plan.warnings.is_empty());
        assert_eq!(plan.new_last_good.org, last_good.org);

        // End to end through the resolver: the mandatory value must still be in force.
        let resolution = layers::resolve(&plan.inputs);
        let config = Config::from_resolution(&resolution);
        assert!(config.audit_enabled());
    }

    #[test]
    fn both_sources_invalid_keeps_both_last_good() {
        let last_good = LastGoodInputs {
            org: OrgConfig {
                mandatory: serde_json::Map::from_iter([("m".to_string(), json!(1))]),
                recommended: serde_json::Map::new(),
            },
            user: serde_json::Map::from_iter([("u".to_string(), json!(2))]),
            preset: Some("safe".to_string()),
        };
        let plan = plan_reload(
            Err("bad org".to_string()),
            Err("bad user".to_string()),
            &last_good,
        );
        assert!(plan.org_failed && plan.user_failed);
        let expected = compose_inputs(&last_good);
        assert_eq!(plan.inputs.org_mandatory, expected.org_mandatory);
        assert_eq!(plan.inputs.user, expected.user);
        assert_eq!(plan.inputs.org_recommended, expected.org_recommended);
        assert_eq!(plan.inputs.preset, expected.preset);
        assert_eq!(plan.new_last_good.preset, Some("safe".to_string()));
        assert!(plan.errors.iter().any(|e| e.contains("bad org")));
        assert!(plan.warnings.iter().any(|w| w.contains("bad user")));
    }

    fn org_with_audit_enabled(value: bool) -> OrgConfig {
        OrgConfig {
            mandatory: serde_json::Map::from_iter([(
                super::super::AUDIT_ENABLED.to_string(),
                json!(value),
            )]),
            recommended: serde_json::Map::new(),
        }
    }

    #[test]
    fn current_returns_last_swapped() {
        let initial = Config::minimal();
        let store = ConfigStore::for_test(initial.clone(), LastGoodInputs::default());
        let old = store.current();
        assert_eq!(*old, initial);

        let report = store.reload_with(
            Ok(org_with_audit_enabled(false)),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(report.swapped);
        let new = store.current();
        assert_ne!(*new, *old);
        // The previously-held Arc is still valid and still holds the old value.
        assert_eq!(*old, initial);
    }

    #[tokio::test]
    async fn generation_and_signal_fire_only_on_change() {
        let store = ConfigStore::for_test(Config::minimal(), LastGoodInputs::default());
        let mut rx = store.subscribe();
        assert_eq!(store.generation(), 0);

        // A reload that resolves to the SAME config: no bump, no wake.
        let report = store.reload_with(
            Ok(OrgConfig::default()),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(!report.swapped);
        assert_eq!(store.generation(), 0);
        let woke = tokio::time::timeout(std::time::Duration::from_millis(50), rx.changed()).await;
        assert!(woke.is_err(), "receiver must not wake on a no-op reload");

        // A reload that resolves to a DIFFERENT config: bumps generation and wakes the receiver.
        let report = store.reload_with(
            Ok(org_with_audit_enabled(false)),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(report.swapped);
        assert_eq!(store.generation(), 1);
        let woke = tokio::time::timeout(std::time::Duration::from_millis(50), rx.changed()).await;
        assert!(woke.is_ok(), "receiver must wake on a real change");
        assert!(!rx.borrow().audit_enabled());
    }

    #[test]
    fn no_receivers_reload_still_swaps() {
        // for_test's watch::channel receiver is dropped immediately, so there are zero
        // receivers when reload_with runs.
        let store = ConfigStore::for_test(Config::minimal(), LastGoodInputs::default());
        let report = store.reload_with(
            Ok(org_with_audit_enabled(false)),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(report.swapped);
        assert_eq!(report.generation, 1);
    }

    #[test]
    fn settle_debounces_until_stable() {
        let fp0 = Some((SystemTime::UNIX_EPOCH, 10));
        let fp1 = Some((
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1),
            20,
        ));
        let watch0 = PathWatch {
            last_seen: fp0,
            last_applied: fp0,
        };

        let (watch1, fired) = settle(&watch0, fp1);
        assert!(
            !fired,
            "first sighting of a new fingerprint must not fire yet"
        );
        assert_eq!(watch1.last_seen, fp1);
        assert_eq!(watch1.last_applied, fp0);

        let (watch2, fired) = settle(&watch1, fp1);
        assert!(
            fired,
            "a second poll seeing the same new fingerprint must fire"
        );
        assert_eq!(watch2.last_applied, fp1);

        let (_, fired) = settle(&watch2, fp1);
        assert!(
            !fired,
            "an unchanged fingerprint after applying must not fire again"
        );
    }

    #[test]
    fn settle_detects_create_and_delete() {
        let fp = Some((SystemTime::UNIX_EPOCH, 5));

        // None -> Some(fp): create (needs two polls to settle).
        let w0 = PathWatch::default();
        let (w1, fired) = settle(&w0, fp);
        assert!(!fired);
        let (w2, fired) = settle(&w1, fp);
        assert!(fired);

        // Some(fp) -> None: delete (needs two polls to settle).
        let (w3, fired) = settle(&w2, None);
        assert!(!fired);
        let (w4, fired) = settle(&w3, None);
        assert!(fired);

        // A flicker (one poll only) must not fire.
        let (w5, fired) = settle(&w4, fp);
        assert!(!fired, "first sighting, not yet stable");
        let (_, fired) = settle(&w5, None);
        assert!(!fired, "flickered back before stabilizing");
    }

    #[test]
    fn initial_load_is_fail_loud_on_org_error() {
        let org_err: crate::Result<OrgConfig> = Err(crate::Error::Config("bad org".into()));
        let user_ok: crate::Result<(UserConfig, Vec<String>)> =
            Ok((UserConfig::default(), Vec::new()));
        assert!(
            compose_initial(org_err, user_ok).is_err(),
            "startup must fail loud on an org error"
        );

        // The same failure, presented to the reload planner, must NOT propagate an error and
        // must keep the last-good org contribution instead.
        let last_good = LastGoodInputs {
            org: org_with_audit_enabled(true),
            user: serde_json::Map::new(),
            preset: None,
        };
        let plan = plan_reload(
            Err("bad org".to_string()),
            Ok((UserConfig::default(), Vec::new())),
            &last_good,
        );
        assert!(plan.org_failed);
        assert!(plan.errors.iter().any(|e| e.contains("bad org")));
        assert_eq!(
            plan.inputs.org_mandatory.get(super::super::AUDIT_ENABLED),
            Some(&json!(true))
        );
    }

    fn always_valid(_: &str) -> bool {
        true
    }

    /// ADR-0023: `load_initial_with_policy` derives the org layers straight from an
    /// org-sourced `LoadedPolicy`'s already-parsed config entries, with no second read of the
    /// org policy file. A mandatory `audit.enabled` entry ends up in force, locked, at the
    /// org-mandatory source.
    #[test]
    fn org_sourced_policy_config_reaches_the_org_layers() {
        let json = r#"{"schema":3,"name":"org","version":"1","grants":[],
            "config":[{"key":"audit.enabled","value":true,"level":"mandatory"}]}"#;
        let manifest =
            crate::governance::manifest::document::parse_manifest(json, "test", always_valid)
                .unwrap();
        let loaded_policy = crate::governance::manifest::source::LoadedPolicy {
            manifest: Some(manifest),
            origin: Some(crate::governance::manifest::source::ManifestOrigin::OrgPolicyFile),
            user_manifest_ignored: false,
        };

        let store =
            ConfigStore::load_initial_with_policy(always_valid, &loaded_policy, None).unwrap();
        assert!(store.current().audit_enabled());

        let last_good = store
            .last_good
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert_eq!(
            last_good.org.mandatory.get(super::super::AUDIT_ENABLED),
            Some(&json!(true))
        );

        let resolution = layers::resolve(&compose_inputs(&last_good));
        let resolved = resolution.get(super::super::AUDIT_ENABLED).unwrap();
        assert!(resolved.locked);
        assert_eq!(resolved.source, layers::Source::OrgMandatory);
    }

    #[test]
    fn compose_initial_folds_the_declared_preset_into_last_good() {
        let org_ok: crate::Result<OrgConfig> = Ok(OrgConfig::default());
        let user_ok: crate::Result<(UserConfig, Vec<String>)> = Ok((
            UserConfig {
                preset: Some("restricted".to_string()),
                values: serde_json::Map::new(),
            },
            Vec::new(),
        ));
        let (last_good, warnings) = compose_initial(org_ok, user_ok).unwrap();
        assert_eq!(last_good.preset, Some("restricted".to_string()));
        assert!(warnings.is_empty());

        let inputs = compose_inputs(&last_good);
        assert_eq!(
            inputs.preset,
            super::super::preset_layer(super::super::Preset::Restricted)
        );
    }

    // --- t06 (ADR-0025): manifest hot-reload ---

    fn all_open_policy() -> crate::governance::manifest::source::LoadedPolicy {
        crate::governance::manifest::source::LoadedPolicy {
            manifest: None,
            origin: None,
            user_manifest_ignored: false,
        }
    }

    fn loaded_policy_with(
        name: &str,
        version: &str,
        origin: crate::governance::manifest::source::ManifestOrigin,
    ) -> crate::governance::manifest::source::LoadedPolicy {
        let json = format!(r#"{{"schema":3,"name":"{name}","version":"{version}","grants":[]}}"#);
        let manifest =
            crate::governance::manifest::document::parse_manifest(&json, "test", always_valid)
                .unwrap();
        crate::governance::manifest::source::LoadedPolicy {
            manifest: Some(manifest),
            origin: Some(origin),
            user_manifest_ignored: false,
        }
    }

    /// Pure identity diffing (ADR-0025 Decision 3/4): both absent is unchanged; identical
    /// name/version/hash/origin is unchanged; an appear or a removal is always changed; a
    /// different version (a different hash too) is changed.
    #[test]
    fn manifest_identity_changed_detects_change_appearance_and_removal() {
        use crate::governance::manifest::source::ManifestOrigin;

        let none = all_open_policy();
        let a = loaded_policy_with("acme", "1", ManifestOrigin::OrgPolicyFile);
        let a_same = loaded_policy_with("acme", "1", ManifestOrigin::OrgPolicyFile);
        let a_v2 = loaded_policy_with("acme", "2", ManifestOrigin::OrgPolicyFile);

        assert!(
            !manifest_identity_changed(&none, &none),
            "both absent: unchanged"
        );
        assert!(
            !manifest_identity_changed(&a, &a_same),
            "identical name/version/hash/origin: unchanged"
        );
        assert!(manifest_identity_changed(&none, &a), "appeared");
        assert!(manifest_identity_changed(&a, &none), "removed");
        assert!(
            manifest_identity_changed(&a, &a_v2),
            "a different version (and hash) is changed"
        );
    }

    /// ADR-0025 Decision 3: a `load_policy` failure (here injected as the org slot's `Err`,
    /// exactly as a real invalid org file or a missing configured user file:// source would
    /// surface) keeps the last-good published policy -- the policy channel does NOT publish.
    #[test]
    fn keep_last_good_org_failure_does_not_publish_the_policy() {
        use crate::governance::manifest::source::ManifestOrigin;

        // Subscribe BEFORE any reload (the real production usage: `transport::mcp::server`'s
        // policy-subscription task subscribes once at startup, well before the watcher's first
        // real settled change). `watch::Sender::send` is a documented no-op on the shared value
        // when it has zero receivers (the same reason `ConfigStore::subscribe`'s own existing
        // config tests all subscribe before the reload they observe), so a receiver created
        // AFTER a publish would see a stale baseline -- not a production concern (a real
        // settled change needs two poll intervals to settle, ample time for the one, ever
        // subscriber to already exist), but this test must subscribe first to observe it.
        let store = ConfigStore::for_test(Config::minimal(), LastGoodInputs::default());
        let mut rx = store.policy();
        assert!(rx.borrow_and_update().manifest.is_none(), "seeded all-open");

        let governed = loaded_policy_with("acme", "1", ManifestOrigin::OrgPolicyFile);
        store.reload_with_policy(Ok(governed), Ok((UserConfig::default(), Vec::new())));
        assert!(rx.has_changed().unwrap(), "the first load publishes acme");
        assert_eq!(
            rx.borrow_and_update().manifest.as_ref().unwrap().name,
            "acme"
        );

        let report = store.reload_with_policy(
            Err("bad org".to_string()),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(report.org_failed);
        assert!(
            !rx.has_changed().unwrap(),
            "a load_policy failure must not publish a new policy"
        );
        assert_eq!(
            rx.borrow().manifest.as_ref().unwrap().name,
            "acme",
            "last-good manifest still in force"
        );
    }

    /// ADR-0025 Decision 1: org-file removal is a legitimate, first-class transition -- a
    /// resolved all-open `LoadedPolicy` (a successful `load_policy` result) DOES publish.
    #[test]
    fn org_removal_publishes_an_all_open_policy() {
        use crate::governance::manifest::source::ManifestOrigin;

        // Subscribe BEFORE any reload; see the comment in
        // `keep_last_good_org_failure_does_not_publish_the_policy` for why.
        let store = ConfigStore::for_test(Config::minimal(), LastGoodInputs::default());
        let mut rx = store.policy();

        let governed = loaded_policy_with("acme", "1", ManifestOrigin::OrgPolicyFile);
        store.reload_with_policy(Ok(governed), Ok((UserConfig::default(), Vec::new())));
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();

        let report = store.reload_with_policy(
            Ok(all_open_policy()),
            Ok((UserConfig::default(), Vec::new())),
        );
        assert!(!report.org_failed);
        assert!(
            rx.has_changed().unwrap(),
            "org-file removal must publish the new all-open policy"
        );
        assert_eq!(rx.borrow_and_update().manifest, None);
    }

    /// ADR-0025 Decision 1 (pinned edge): a CONFIGURED user file:// source that goes missing is
    /// a load error, not a transition to all-open -- unlike org-file removal above. Drives the
    /// REAL `source::load_policy` call (via `reresolve`) against a real, controllable temp file
    /// so this proves the actual `load_user_manifest` I/O-error path, not just the injected
    /// keep-last-good machinery every other test in this module exercises.
    #[test]
    fn user_manifest_deletion_keeps_last_good() {
        use crate::governance::manifest::source::ManifestOrigin;

        let org_path = load::org_policy_path();
        if org_path.exists() {
            eprintln!(
                "skipping the strict assertion: a real org policy file exists at {} on this \
                 machine",
                org_path.display()
            );
            return;
        }

        let path = std::env::temp_dir().join(format!(
            "ghostlight-t06-user-manifest-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"{"schema":3,"name":"user-file","version":"1","grants":[]}"#,
        )
        .unwrap();

        let store = ConfigStore::for_test_with_user_source(path.display().to_string());
        let mut rx = store.policy();
        assert!(rx.borrow_and_update().manifest.is_none(), "seeded all-open");

        let report = store.reresolve();
        assert!(
            !report.org_failed,
            "the file exists: load_policy must succeed"
        );
        assert!(
            rx.has_changed().unwrap(),
            "the first real load publishes the user-sourced policy"
        );
        assert_eq!(
            rx.borrow_and_update().origin,
            Some(ManifestOrigin::UserFile)
        );

        std::fs::remove_file(&path).unwrap();
        let report2 = store.reresolve();
        assert!(
            report2.org_failed,
            "a missing CONFIGURED user file:// source is a load error, not a transition to \
             all-open"
        );
        assert!(
            !rx.has_changed().unwrap(),
            "a load-error reresolve must not publish anything on the policy channel"
        );
        assert_eq!(
            rx.borrow().origin,
            Some(ManifestOrigin::UserFile),
            "the last-good (user-sourced) policy survives the missing-file reload"
        );
    }
}
