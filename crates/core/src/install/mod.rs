// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Self-registering installer: `ghostlight install` / `uninstall` / `doctor`.
//!
//! Registers the native-messaging host (Windows registry / macOS+Linux file drop) for detected
//! Chromium browsers, and adds the `ghostlight` server to detected MCP clients (CLI where a safe
//! one exists, else a careful JSON merge). Idempotent; `--dry-run` writes nothing; every failure is
//! independent and prints exact manual steps. This is engine packaging, not governance.

pub mod clients;
pub mod merge;
pub mod native_host;
pub mod supervisor;

use crate::{Error, Result};
use native_host::{BrowserSpec, HostManifest, WowView};
use std::path::PathBuf;

/// Per-user vs system-wide registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    System,
}

/// Registry hive (Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    Hkcu,
    Hklm,
}

/// Which targets to act on.
#[derive(Debug, Clone)]
pub enum Selection {
    /// Every *detected* target (default).
    All,
    /// Every *known* target (--all-browsers / --all-clients).
    ForceAll,
    /// Exactly these ids, detected or not.
    Only(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub extension_id: Option<String>,
    pub dry_run: bool,
    pub system: bool,
    pub browsers: Selection,
    pub clients: Selection,
    /// Register the server to run in debug mode (adds `GHOSTLIGHT_DEBUG=1` to its env).
    pub debug: bool,
    /// Skip registering the OS auto-start supervisor (dev instances run `ghostlight service` in a
    /// terminal instead of an auto-started one that would hold the exe lock during rebuilds).
    pub no_supervisor: bool,
}
#[derive(Debug, Clone)]
pub struct UninstallOptions {
    pub dry_run: bool,
    pub system: bool,
    pub browsers: Selection,
    pub clients: Selection,
}
/// Injected filesystem roots so path computation is a pure function of inputs (testable on any OS).
#[derive(Debug, Clone)]
pub struct PlanCtx {
    pub current_exe: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub local: PathBuf,
}

impl PlanCtx {
    /// Resolve from the running environment (the I/O boundary).
    pub fn resolve() -> Result<Self> {
        let missing = |what: &str| Error::Unsupported(format!("cannot resolve {what} directory"));
        Ok(Self {
            current_exe: std::env::current_exe()?,
            home: dirs::home_dir().ok_or_else(|| missing("home"))?,
            config: dirs::config_dir().ok_or_else(|| missing("config"))?,
            local: dirs::data_local_dir().ok_or_else(|| missing("data-local"))?,
        })
    }
}

/// True if `bin` (or `bin.exe`/`bin.cmd`) is on PATH.
pub(crate) fn on_path(bin: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        dir.join(bin).is_file()
            || dir.join(format!("{bin}.exe")).is_file()
            || dir.join(format!("{bin}.cmd")).is_file()
    })
}

/// Append `.<ext>` to a path's *full* file name. Unlike `Path::with_extension`, which *replaces*
/// the trailing extension, this preserves it: `~/.claude.json` -> `~/.claude.json.tmp`, so a
/// staging/backup sibling is unambiguously tied to its target (and never collides across files
/// that share a stem-minus-extension).
pub(crate) fn append_extension(path: &std::path::Path, ext: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}

/// Read a config file, treating a missing file as empty (a legitimate new-file case) but surfacing
/// any *other* read error (notably non-UTF-8 or a permission failure on an existing file). This is
/// what keeps a merge from mistaking an unreadable-but-present config for empty and clobbering it.
fn read_config_or_empty(path: &std::path::Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Reject a `Selection::Only` referencing ids that match nothing (a typo'd `--browser`/`--client`),
/// rather than silently planning zero actions and exiting 0.
fn validate_selection(sel: &Selection, known: &[&str], kind: &str) -> Result<()> {
    if let Selection::Only(ids) = sel {
        let unknown: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|id| !known.contains(id))
            .collect();
        if !unknown.is_empty() {
            return Err(Error::Unsupported(format!(
                "unknown {kind} id(s): {}. valid ids: {}",
                unknown.join(", "),
                known.join(", ")
            )));
        }
    }
    Ok(())
}

/// The CLI ids the installer knows, for `--browser`/`--client` validation.
fn known_browser_ids() -> Vec<&'static str> {
    native_host::BROWSERS.iter().map(|b| b.id).collect()
}
fn known_client_ids() -> Vec<&'static str> {
    clients::CLIENTS.iter().map(|c| c.cli_id).collect()
}

// --- Actions ---

/// One planned mutation, with everything needed to apply it, render it, and hint the manual step.
struct Action {
    label: String,
    detail: String,
    op: Op,
    noop: Option<&'static str>,
    manual: String,
}
enum Op {
    WriteFile {
        path: PathBuf,
        contents: String,
    },
    SetReg {
        hive: Hive,
        key: String,
        wow: WowView,
        value: String,
    },
    RemoveFile {
        path: PathBuf,
    },
    /// Copy the running binary to a per-instance native-host launcher (ADR-0044 Decision 4, non-
    /// default instances only). Creates parent dirs and overwrites, so a re-install refreshes it.
    CopyBinary {
        from: PathBuf,
        to: PathBuf,
    },
    /// Remove a plain file (a per-instance launcher copy) if present; absence is not an error.
    RemovePath {
        path: PathBuf,
    },
    DeleteReg {
        hive: Hive,
        key: String,
        wow: WowView,
    },
    /// Merge a change into a client config. The merge is (re)computed against the file's *current*
    /// contents at apply time -- not baked in at plan time -- so a concurrent write by a running
    /// client (notably Claude Code rewriting `~/.claude.json`) is not clobbered.
    Merge {
        path: PathBuf,
        dialect: merge::Dialect,
        change: MergeChange,
    },
    RunCli {
        program: String,
        argv: Vec<String>,
    },
    /// Nothing to write; print the manual hint (e.g. VS Code with no `code` CLI).
    Manual,
    /// This target could not be *planned* safely (e.g. a malformed config, or an unreadable file).
    /// Reported as a failure for this target alone -- it never aborts planning of the others.
    Blocked {
        detail: String,
    },
}

/// The intent of a [`Op::Merge`], applied against freshly-read file contents.
enum MergeChange {
    Add(merge::ServerEntry),
    Remove(String),
}

/// Outcome tally from applying (or previewing) a plan. `manual` (needs a hand-step) is kept
/// distinct from `failed` (a genuine error) so a manual-only run never reports as failed.
#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    done: usize,
    noop: usize,
    manual: usize,
    failed: usize,
}

// --- Selection ---

fn selected_browsers(sel: &Selection, ctx: &PlanCtx) -> Vec<&'static BrowserSpec> {
    native_host::BROWSERS
        .iter()
        .filter(|b| match sel {
            Selection::ForceAll => true,
            Selection::All => native_host::detect_browser(b, ctx),
            Selection::Only(ids) => ids.iter().any(|id| id == b.id),
        })
        .collect()
}
fn selected_clients(sel: &Selection, ctx: &PlanCtx) -> Vec<&'static clients::ClientSpec> {
    clients::CLIENTS
        .iter()
        .filter(|c| match sel {
            Selection::ForceAll => true,
            Selection::All => clients::detect(c, ctx),
            Selection::Only(ids) => ids.iter().any(|id| id == c.cli_id),
        })
        .collect()
}

fn scope_of(system: bool) -> Scope {
    if system {
        Scope::System
    } else {
        Scope::User
    }
}

// --- Plan: install ---

fn plan_install(opts: &InstallOptions, ctx: &PlanCtx) -> Result<Vec<Action>> {
    // ADR-0064: the host surface is instance-agnostic now (every instance registers its own host,
    // resolved from the environment via the `native_host::*` path helpers), so there is no longer a
    // separate `_for` seam that took an instance argument.
    plan_install_for(opts, ctx)
}

fn plan_install_for(opts: &InstallOptions, ctx: &PlanCtx) -> Result<Vec<Action>> {
    let scope = scope_of(opts.system);
    let mut actions = Vec::new();

    // ADR-0064: every instance -- including dev -- registers its OWN native-messaging host (the
    // per-instance surface ADR-0044 derives), replacing ADR-0048 D6's thin dev-shadow-onto-default.
    // The bare block scopes the host-planning locals so they never leak into the client section.
    {
        // ADR-0044 Decision 4: the DEFAULT instance's manifest points at the bare binary; a
        // non-default instance's points at a per-instance copy Chrome launches by name (argv[0]).
        let (launcher, needs_copy) = native_host::instance_launcher(ctx);
        let manifest = HostManifest::resolve(&launcher, opts.extension_id.as_deref())?;
        let manifest_json = manifest.to_json();

        // Place the per-instance binary copy FIRST (before the manifest that references it). Overwrite
        // so a re-install refreshes it; a size match is treated as already-current for the report.
        if needs_copy {
            // ADR-0046: the per-instance copy is of the browser ADAPTER (the tiny pass-through Chrome
            // launches by name), never the multi-MB `ghostlight` brain -- so a service rebuild never
            // needs the copy refreshed.
            let copy_from = native_host::sibling_bin(&ctx.current_exe, "ghostlight-relay");
            let up_to_date = std::fs::metadata(&launcher)
                .ok()
                .zip(std::fs::metadata(&copy_from).ok())
                .map(|(a, b)| a.len() == b.len())
                .unwrap_or(false);
            actions.push(Action {
                label: "native host (instance binary)".into(),
                detail: launcher.display().to_string(),
                noop: up_to_date.then_some("already present"),
                manual: format!("copy {} to {}", copy_from.display(), launcher.display()),
                op: Op::CopyBinary {
                    from: copy_from,
                    to: launcher.clone(),
                },
            });
        }

        // --- native host, per selected browser ---
        if cfg!(windows) {
            // One shared manifest file, then a registry key per browser pointing at it.
            let manifest_path = native_host::win_manifest_path(ctx);
            let file_noop = file_matches(&manifest_path, &manifest_json);
            actions.push(Action {
                label: "native host (manifest)".into(),
                detail: manifest_path.display().to_string(),
                noop: file_noop.then_some("already up to date"),
                manual: format!(
                    "write this JSON to {}:\n{manifest_json}",
                    manifest_path.display()
                ),
                op: Op::WriteFile {
                    path: manifest_path.clone(),
                    contents: manifest_json.clone(),
                },
            });
            let value = manifest_path.to_string_lossy().into_owned();
            let hive = native_host::hive_for(scope);
            let wow = native_host::wow_for(scope);
            for b in selected_browsers(&opts.browsers, ctx) {
                let key = native_host::win_reg_key(b);
                let noop =
                    native_host::read_default(hive, &key, wow).as_deref() == Some(value.as_str());
                actions.push(Action {
                    label: format!("{} (native host)", b.display),
                    detail: format!("{hive:?} {key}"),
                    noop: noop.then_some("already registered"),
                    manual: format!("set (Default) of {hive:?}\\{key} to {value}"),
                    op: Op::SetReg {
                        hive,
                        key,
                        wow,
                        value: value.clone(),
                    },
                });
            }
        } else {
            for b in selected_browsers(&opts.browsers, ctx) {
                let path = host_file_path(b, ctx);
                let noop = file_matches(&path, &manifest_json);
                actions.push(Action {
                    label: format!("{} (native host)", b.display),
                    detail: path.display().to_string(),
                    noop: noop.then_some("already up to date"),
                    manual: format!("write this JSON to {}:\n{manifest_json}", path.display()),
                    op: Op::WriteFile {
                        path,
                        contents: manifest_json.clone(),
                    },
                });
            }
        }
    }

    // --- MCP clients ---
    // Per-client planning is infallible: a malformed/unreadable config blocks *that* client only
    // (an `Op::Blocked` action), never the whole run -- preserving the independent-failure contract.
    let mut entry = clients::server_entry(&ctx.current_exe);
    if opts.debug {
        entry
            .env
            .insert("GHOSTLIGHT_DEBUG".to_string(), "1".to_string());
    }
    for c in selected_clients(&opts.clients, ctx) {
        actions.push(plan_client_install(c, ctx, &entry));
    }
    Ok(actions)
}

/// Build a "could not plan this target" action -- reported as a lone failure, never aborts others.
fn blocked(label: String, detail: String, reason: String, manual: String) -> Action {
    Action {
        label,
        detail,
        noop: None,
        manual,
        op: Op::Blocked { detail: reason },
    }
}

fn plan_client_install(
    c: &clients::ClientSpec,
    ctx: &PlanCtx,
    entry: &merge::ServerEntry,
) -> Action {
    use clients::AddVia;
    let label = format!("{} (client)", c.display);
    match c.add_via {
        // VS Code's JSONC config can't be safely value-merged; drive it through its own CLI, or
        // print the exact command when `code` is not on PATH (no silent hand-merge of JSONC).
        AddVia::VsCodeCli => {
            let json = vscode_add_json(entry);
            let manual = format!("run: code --add-mcp {}", shell_single_quote(&json));
            if on_path("code") {
                Action {
                    label,
                    detail: "code --add-mcp".into(),
                    noop: None,
                    manual,
                    op: Op::RunCli {
                        program: "code".into(),
                        argv: vec!["--add-mcp".into(), json],
                    },
                }
            } else {
                Action {
                    label,
                    detail: "code CLI not found".into(),
                    noop: None,
                    manual,
                    op: Op::Manual,
                }
            }
        }
        // Every plain-JSON client (Claude Code/Desktop, Cursor) is an idempotent value-level merge.
        // Claude Code's entry lives in ~/.claude.json, which a running Claude Code also writes; the
        // merge is deferred to apply time (see `Op::Merge`) so we never clobber a concurrent write.
        AddVia::FileMerge => {
            let path = clients::config_path(c, ctx);
            let target = path.display().to_string();
            let manual = format!(
                "merge our server into {target} under \"{}\"",
                c.dialect.top_key()
            );
            // Missing config => empty (new file); an unreadable *existing* file blocks this client.
            let existing = match read_config_or_empty(&path) {
                Ok(s) => s,
                Err(e) => {
                    return blocked(label, target, format!("cannot read config: {e}"), manual)
                }
            };
            // Validate now (so `--dry-run` surfaces a non-mergeable config) and compute the no-op
            // state; the authoritative merge re-runs against fresh content at apply time.
            match merge::merge_server(&existing, c.dialect, entry)
                .and_then(|_| merge::server_matches(&existing, c.dialect, entry))
            {
                Ok(noop) => Action {
                    label,
                    detail: target,
                    noop: noop.then_some("already registered"),
                    manual,
                    op: Op::Merge {
                        path,
                        dialect: c.dialect,
                        change: MergeChange::Add(entry.clone()),
                    },
                },
                Err(e) => blocked(label, target, e.to_string(), manual),
            }
        }
    }
}

/// Wrap a value in single quotes for a display-only shell hint, escaping embedded single quotes.
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn vscode_add_json(entry: &merge::ServerEntry) -> String {
    let mut obj = serde_json::json!({
        "name": entry.name, "command": entry.command, "args": entry.args, "type": "stdio"
    });
    if !entry.env.is_empty() {
        obj["env"] = serde_json::json!(entry.env);
    }
    obj.to_string()
}

// --- Plan: uninstall ---

fn plan_uninstall(opts: &UninstallOptions, ctx: &PlanCtx) -> Result<Vec<Action>> {
    let scope = scope_of(opts.system);
    let mut actions = Vec::new();

    if cfg!(windows) {
        let hive = native_host::hive_for(scope);
        let wow = native_host::wow_for(scope);
        for b in selected_browsers(&opts.browsers, ctx) {
            let key = native_host::win_reg_key(b);
            let present = native_host::read_default(hive, &key, wow).is_some();
            actions.push(Action {
                label: format!("{} (native host)", b.display),
                detail: format!("{hive:?} {key}"),
                noop: (!present).then_some("not registered"),
                manual: format!("delete registry key {hive:?}\\{key}"),
                op: Op::DeleteReg { hive, key, wow },
            });
        }
        // Remove the shared manifest file too (only if it is ours).
        let manifest_path = native_host::win_manifest_path(ctx);
        actions.push(plan_host_removal(
            "native host (manifest)".into(),
            manifest_path,
        ));
    } else {
        for b in selected_browsers(&opts.browsers, ctx) {
            let path = host_file_path(b, ctx);
            actions.push(plan_host_removal(
                format!("{} (native host)", b.display),
                path,
            ));
        }
    }

    // Remove the per-instance binary copy (ADR-0044 Decision 4; non-default instances only). The
    // default instance has no copy, so this action is absent.
    let (launcher, is_copy) = native_host::instance_launcher(ctx);
    if is_copy {
        actions.push(Action {
            label: "native host (instance binary)".into(),
            detail: launcher.display().to_string(),
            noop: (!launcher.exists()).then_some("absent"),
            manual: format!("delete {}", launcher.display()),
            op: Op::RemovePath { path: launcher },
        });
    }

    // Per-client planning is infallible (see plan_install): one bad config blocks that client only.
    for c in selected_clients(&opts.clients, ctx) {
        actions.push(plan_client_uninstall(c, ctx));
    }
    Ok(actions)
}

fn plan_client_uninstall(c: &clients::ClientSpec, ctx: &PlanCtx) -> Action {
    let label = format!("{} (client)", c.display);
    // The MCP server entry key for the active instance (ADR-0044): `ghostlight` for the default,
    // `ghostlight-<n>` for a named instance -- so uninstall removes only this instance's entry.
    let server = ghostlight_transport::instance::Instance::resolve().mcp_server_name();
    // VS Code's JSONC config can't be safely rewritten by a value-level merge -> manual removal.
    if c.is_jsonc {
        return Action {
            label,
            detail: "manual".into(),
            noop: None,
            manual: format!(
                "remove the \"{server}\" entry from the VS Code mcp.json 'servers' block"
            ),
            op: Op::Manual,
        };
    }
    // Every other client (incl. Claude Code, whose entry lives in ~/.claude.json) is removed by a
    // single idempotent value-level merge -- no subprocess, and a semantic no-op when absent.
    let path = clients::config_path(c, ctx);
    let target = path.display().to_string();
    let manual = format!("remove \"{server}\" from {target}");
    // Missing config => empty (nothing to remove); an unreadable *existing* file blocks this client.
    let existing = match read_config_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return blocked(label, target, format!("cannot read config: {e}"), manual),
    };
    // Validate now (a non-object root errors here, at dry-run) and compute the no-op state.
    match merge::remove_server(&existing, c.dialect, &server)
        .and_then(|_| merge::has_server(&existing, c.dialect, &server))
    {
        Ok(present) => Action {
            label,
            detail: target,
            noop: (!present).then_some("not present"),
            manual,
            op: Op::Merge {
                path,
                dialect: c.dialect,
                change: MergeChange::Remove(server),
            },
        },
        Err(e) => blocked(label, target, e.to_string(), manual),
    }
}

/// Plan the removal of a native-host manifest file, classifying by ownership so a foreign manifest
/// at the same path is reported as a manual skip (never falsely as removed). Infallible: a read
/// error (e.g. a locked file) blocks *this* removal only.
fn plan_host_removal(label: String, path: PathBuf) -> Action {
    let detail = path.display().to_string();
    let manual = format!("delete {detail}");
    match native_host::host_file_owner(&path) {
        Ok(None) => Action {
            label,
            detail,
            noop: Some("absent"),
            manual,
            op: Op::RemoveFile { path },
        },
        Ok(Some(true)) => Action {
            label,
            detail,
            noop: None,
            manual,
            op: Op::RemoveFile { path },
        },
        Ok(Some(false)) => Action {
            label,
            detail: format!("{detail} (not ours -- left untouched)"),
            noop: None,
            manual: format!(
                "a native-messaging manifest not owned by ghostlight exists at {detail}; \
                 remove it manually if you intended to"
            ),
            op: Op::Manual,
        },
        Err(e) => blocked(label, detail, format!("cannot read manifest: {e}"), manual),
    }
}

// --- Shared helpers ---

pub(crate) fn host_file_path(b: &BrowserSpec, ctx: &PlanCtx) -> PathBuf {
    if cfg!(target_os = "macos") {
        native_host::mac_host_path(b, ctx)
    } else {
        native_host::linux_host_path(b, ctx)
    }
}

fn file_matches(path: &std::path::Path, contents: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|c| c == contents)
        .unwrap_or(false)
}

// --- Apply ---

fn apply(actions: &[Action], dry_run: bool) -> Tally {
    let mut t = Tally::default();
    for a in actions {
        if let Some(reason) = a.noop {
            println!("  [noop] {:<28} {} ({reason})", a.label, a.detail);
            t.noop += 1;
            continue;
        }
        match &a.op {
            // A target we could not plan safely -- a lone failure, shown even in a dry-run preview.
            Op::Blocked { detail } => {
                println!("  [FAIL] {:<28} {} -> {detail}", a.label, a.detail);
                println!("         manual: {}", a.manual);
                t.failed += 1;
            }
            // Needs a hand-step (VS Code without a CLI, a foreign manifest). Not a failure.
            Op::Manual => {
                println!(
                    "  [skip] {:<28} {} -> manual step required",
                    a.label, a.detail
                );
                println!("         {}", a.manual);
                t.manual += 1;
            }
            _ if dry_run => {
                println!("  [plan] {:<28} {}", a.label, a.detail);
                t.done += 1; // "would apply" -- so the summary reports the real planned count
            }
            op => match apply_one(op) {
                Ok(()) => {
                    println!("  [ok]   {:<28} {}", a.label, a.detail);
                    t.done += 1;
                }
                Err(e) => {
                    println!("  [FAIL] {:<28} {} -> {e}", a.label, a.detail);
                    println!("         manual: {}", a.manual);
                    t.failed += 1;
                }
            },
        }
    }
    t
}

fn apply_one(op: &Op) -> Result<()> {
    match op {
        Op::WriteFile { path, contents } => native_host::write_file_atomic(path, contents),
        Op::RemoveFile { path } => {
            native_host::remove_host_file_if_ours(path)?;
            Ok(())
        }
        Op::CopyBinary { from, to } => {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(from, to)?;
            Ok(())
        }
        Op::RemovePath { path } => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Io(e)),
        },
        Op::SetReg {
            hive,
            key,
            wow,
            value,
        } => native_host::set_default(*hive, key, *wow, value),
        Op::DeleteReg { hive, key, wow } => native_host::delete_key(*hive, key, *wow),
        Op::Merge {
            path,
            dialect,
            change,
        } => apply_merge(path, *dialect, change),
        Op::RunCli { program, argv } => {
            let status = std::process::Command::new(program)
                .args(argv)
                .status()
                .map_err(|e| Error::ClientRegistration(format!("{program}: {e}")))?;
            if status.success() {
                Ok(())
            } else {
                Err(Error::ClientRegistration(format!(
                    "{program} exited with {status}"
                )))
            }
        }
        // Manual/Blocked are dispatched in `apply` and never reach here; treat as no-ops for safety.
        Op::Manual | Op::Blocked { .. } => Ok(()),
    }
}

/// Apply a client-config [`Op::Merge`] against the file's *current* contents. Re-reads and
/// re-decides here (not at plan time) so a concurrent write is not clobbered, and only writes when
/// the change is semantically needed -- so an already-correct or already-absent entry never
/// reformats the file. Backs up the prior contents before an in-place rewrite.
fn apply_merge(
    path: &std::path::Path,
    dialect: merge::Dialect,
    change: &MergeChange,
) -> Result<()> {
    // Missing => empty (new file); an unreadable existing file errors rather than clobbering.
    let existing = read_config_or_empty(path).map_err(|e| {
        Error::MergeConflict(format!("{}: cannot read config: {e}", path.display()))
    })?;
    let conflict = |e| Error::MergeConflict(format!("{}: {e}", path.display()));
    let (needed, updated) = match change {
        MergeChange::Add(entry) => (
            !merge::server_matches(&existing, dialect, entry).map_err(conflict)?,
            merge::merge_server(&existing, dialect, entry).map_err(conflict)?,
        ),
        MergeChange::Remove(name) => (
            merge::has_server(&existing, dialect, name).map_err(conflict)?,
            merge::remove_server(&existing, dialect, name).map_err(conflict)?,
        ),
    };
    if !needed {
        return Ok(());
    }
    if path.exists() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::fs::copy(path, append_extension(path, &format!("bak-{nanos}")))?;
    }
    native_host::write_file_atomic(path, &updated)
}

// --- Entry points ---

/// `ghostlight install`
pub fn run_install(opts: InstallOptions) -> Result<()> {
    validate_selection(&opts.browsers, &known_browser_ids(), "browser")?;
    validate_selection(&opts.clients, &known_client_ids(), "client")?;
    let ctx = PlanCtx::resolve()?;
    let scope = scope_of(opts.system);
    let actions = plan_install(&opts, &ctx)?;
    println!(
        "ghostlight install ({})",
        if scope == Scope::System {
            "system-wide"
        } else {
            "per-user"
        }
    );
    let tally = apply(&actions, opts.dry_run);
    // The OS supervisor (auto-start) is ALWAYS per-user, regardless of --system (ADR-0030 Decision
    // 8): register it, then start it once so the first session is already up. Best-effort: never
    // folded into `tally`/`exit_result` -- a failure here warns (see supervisor::apply_steps) and
    // never turns an otherwise-successful install into a failure.
    println!("\nSupervisor (auto-start):");
    if opts.no_supervisor {
        // ADR-0046 dev loop: an auto-started dev service would hold the exe lock during a rebuild;
        // the developer runs `ghostlight service` in a terminal instead (see docs/DEV-LOOP.md).
        println!("  (skipped: --no-supervisor)");
    } else {
        supervisor::apply_steps(
            &ghostlight_transport::supervisor::supervisor_task_name(),
            &supervisor::register_steps(&ctx.current_exe, &ctx),
            opts.dry_run,
        );
    }
    finish(opts.dry_run, &tally);
    if opts.dry_run {
        println!("\nDry run -- nothing was written.");
    } else {
        println!(
            "\nNext: load the unpacked extension (chrome://extensions) and restart the browser."
        );
    }
    exit_result(&tally)
}

/// `ghostlight uninstall`
pub fn run_uninstall(opts: UninstallOptions) -> Result<()> {
    validate_selection(&opts.browsers, &known_browser_ids(), "browser")?;
    validate_selection(&opts.clients, &known_client_ids(), "client")?;
    let ctx = PlanCtx::resolve()?;
    let actions = plan_uninstall(&opts, &ctx)?;
    println!("ghostlight uninstall");
    let tally = apply(&actions, opts.dry_run);
    // Unregister + stop the OS supervisor, best-effort (see run_install's matching note).
    println!("\nSupervisor (auto-start):");
    supervisor::apply_steps(
        &ghostlight_transport::supervisor::supervisor_task_name(),
        &supervisor::unregister_steps(&ctx),
        opts.dry_run,
    );
    finish(opts.dry_run, &tally);
    exit_result(&tally)
}

fn finish(dry_run: bool, t: &Tally) {
    let manual = if t.manual > 0 {
        format!(", {} manual step(s)", t.manual)
    } else {
        String::new()
    };
    if dry_run {
        println!(
            "\nPlanned: {} change(s), {} already current{manual}, {} blocked.",
            t.done, t.noop, t.failed
        );
    } else {
        println!(
            "\nDone: {} applied, {} unchanged{manual}, {} failed.",
            t.done, t.noop, t.failed
        );
    }
}

fn exit_result(t: &Tally) -> Result<()> {
    // Non-zero only on a genuine failure that accomplished nothing; a manual step is guidance, not a
    // failure, and partial success (any applied or already-current target) stays a success exit.
    if t.failed > 0 && t.done == 0 && t.noop == 0 {
        Err(Error::ClientRegistration(
            "no targets could be registered (see the errors above)".into(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merge::{Dialect, ServerEntry};
    use std::collections::BTreeMap;

    fn entry() -> ServerEntry {
        ServerEntry {
            name: "ghostlight".into(),
            command: "/abs/ghostlight".into(),
            args: vec![],
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn append_extension_preserves_the_existing_extension() {
        assert_eq!(
            append_extension(std::path::Path::new("/home/u/.claude.json"), "tmp"),
            PathBuf::from("/home/u/.claude.json.tmp")
        );
        assert_eq!(
            append_extension(
                std::path::Path::new("/x/org.sylin.ghostlight.json"),
                "bak-9"
            ),
            PathBuf::from("/x/org.sylin.ghostlight.json.bak-9")
        );
    }

    #[test]
    fn validate_selection_rejects_unknown_ids_only() {
        let known = ["chrome", "edge"];
        assert!(validate_selection(&Selection::All, &known, "browser").is_ok());
        assert!(validate_selection(&Selection::ForceAll, &known, "browser").is_ok());
        assert!(
            validate_selection(&Selection::Only(vec!["chrome".into()]), &known, "browser").is_ok()
        );
        let err = validate_selection(&Selection::Only(vec!["chorme".into()]), &known, "browser")
            .unwrap_err();
        assert!(matches!(err, Error::Unsupported(m) if m.contains("chorme")));
    }

    fn bak_count(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
                    .count()
            })
            .unwrap_or(0)
    }

    #[test]
    fn apply_merge_is_idempotent_backs_up_only_on_change_and_preserves_siblings() {
        let dir = std::env::temp_dir().join(format!("ghostlight-merge-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".claude.json");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();

        // Add: our entry lands, sibling preserved, one backup taken (file existed).
        apply_merge(&path, Dialect::McpServers, &MergeChange::Add(entry())).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["ghostlight"]["command"], "/abs/ghostlight");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(bak_count(&dir), 1);

        // Add again: already correct -> no write, no new backup.
        apply_merge(&path, Dialect::McpServers, &MergeChange::Add(entry())).unwrap();
        assert_eq!(bak_count(&dir), 1);

        // Remove: our entry gone, sibling kept, a second backup taken.
        apply_merge(
            &path,
            Dialect::McpServers,
            &MergeChange::Remove("ghostlight".into()),
        )
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(v["mcpServers"].get("ghostlight").is_none());
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(bak_count(&dir), 2);

        // Remove again: absent -> no write, no new backup.
        apply_merge(
            &path,
            Dialect::McpServers,
            &MergeChange::Remove("ghostlight".into()),
        )
        .unwrap();
        assert_eq!(bak_count(&dir), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_merge_add_to_absent_file_creates_it_without_backup() {
        let dir = std::env::temp_dir().join(format!("ghostlight-merge-new-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mcp.json");

        apply_merge(&path, Dialect::McpServers, &MergeChange::Add(entry())).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["ghostlight"]["command"], "/abs/ghostlight");
        assert_eq!(
            bak_count(&dir),
            0,
            "a newly-created file has nothing to back up"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_config_or_empty_maps_missing_to_empty_but_surfaces_bad_bytes() {
        let dir = std::env::temp_dir().join(format!("ghostlight-rc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Missing file -> empty (a legitimate new-config case).
        assert_eq!(read_config_or_empty(&dir.join("nope.json")).unwrap(), "");
        // Present but non-UTF-8 -> error, never silently "".
        let bad = dir.join("bad.json");
        std::fs::write(&bad, [0xff, 0xfe, 0x00, 0x9f]).unwrap();
        assert!(read_config_or_empty(&bad).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn apply_merge_refuses_to_clobber_an_unreadable_existing_file() {
        let dir = std::env::temp_dir().join(format!("ghostlight-noclobber-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".claude.json");
        // Real bytes that are not valid UTF-8: read_to_string fails, but the file is NOT empty.
        std::fs::write(&path, [0xff, 0x00, 0x01, 0x02]).unwrap();
        let before = std::fs::read(&path).unwrap();

        let err = apply_merge(&path, Dialect::McpServers, &MergeChange::Add(entry())).unwrap_err();
        assert!(matches!(err, Error::MergeConflict(_)));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "an unreadable existing config must be left byte-for-byte untouched"
        );
        assert_eq!(bak_count(&dir), 0, "no backup, because nothing was written");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn plan_client_install_blocks_a_malformed_config_without_aborting() {
        let dir = std::env::temp_dir().join(format!("ghostlight-blocked-{}", std::process::id()));
        let home = dir.join("home");
        std::fs::create_dir_all(home.join(".cursor")).unwrap();
        // Cursor's config with an array root is un-mergeable (must not be clobbered).
        std::fs::write(home.join(".cursor").join("mcp.json"), "[]").unwrap();
        let ctx = PlanCtx {
            current_exe: PathBuf::from("/abs/ghostlight"),
            home,
            config: dir.join("config"),
            local: dir.join("local"),
        };
        let cursor = clients::client_by_id("cursor").unwrap();

        // Infallible: returns a Blocked action for this client instead of aborting the whole plan.
        let action = plan_client_install(cursor, &ctx, &entry());
        assert!(matches!(action.op, Op::Blocked { .. }));
        assert!(action.noop.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tally_separates_manual_from_failed_and_exit_reflects_it() {
        // A manual step is guidance, not a failure: manual-only stays a success exit.
        let manual = Action {
            label: "vscode".into(),
            detail: "no cli".into(),
            noop: None,
            manual: "run: code --add-mcp ...".into(),
            op: Op::Manual,
        };
        let t = apply(std::slice::from_ref(&manual), true);
        assert_eq!((t.done, t.noop, t.manual, t.failed), (0, 0, 1, 0));
        assert!(exit_result(&t).is_ok());

        // A blocked target is a genuine failure; with nothing else accomplished, exit is non-zero.
        let blk = blocked(
            "cursor".into(),
            "path".into(),
            "bad".into(),
            "fix it".into(),
        );
        let t2 = apply(std::slice::from_ref(&blk), true);
        assert_eq!((t2.done, t2.noop, t2.manual, t2.failed), (0, 0, 0, 1));
        assert!(exit_result(&t2).is_err());

        // But a failure alongside a success is partial success -> exit ok.
        let ok = Action {
            label: "chrome".into(),
            detail: "reg".into(),
            noop: Some("already registered"),
            manual: String::new(),
            op: Op::Manual, // op irrelevant: noop short-circuits first
        };
        let t3 = apply(&[ok, blk], true);
        assert_eq!((t3.noop, t3.failed), (1, 1));
        assert!(exit_result(&t3).is_ok());
    }

    /// ADR-0064: a plan carries BOTH native-host and MCP-client actions -- every instance registers
    /// its own host now (here the default instance, env unset). The dev instance's own-host
    /// registration is covered end to end by `tests/install_instance.rs` via the real `--instance`
    /// CLI path (which sets `GHOSTLIGHT_INSTANCE` in the child), avoiding a process-global env race.
    #[test]
    fn plan_install_plans_both_a_native_host_and_client_entries() {
        let dir = std::env::temp_dir().join(format!("ghostlight-planshape-{}", std::process::id()));
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".claude.json"), "{}").unwrap();
        let ctx = PlanCtx {
            current_exe: PathBuf::from("/abs/ghostlight"),
            home,
            config: dir.join("config"),
            local: dir.join("local"),
        };
        let opts = InstallOptions {
            extension_id: None,
            dry_run: true,
            system: false,
            browsers: Selection::ForceAll,
            clients: Selection::Only(vec!["claude-code".into()]),
            debug: false,
            no_supervisor: true,
        };
        let actions = plan_install_for(&opts, &ctx).unwrap();
        assert!(
            actions.iter().any(|a| a.label.contains("native host")),
            "the plan registers a native host"
        );
        assert!(
            actions.iter().any(|a| a.label.contains("(client)")),
            "the plan also registers MCP-client entries"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
