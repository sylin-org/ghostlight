//! `ghostlight doctor` -- the one-shot, read-only diagnosis that fuses installer registration
//! state, per-pid debug sessions, and a live probe of the IPC endpoint into a single report with
//! a truthful exit code.
//!
//! Doctor never writes, deletes, or kills anything: every finding is a hint the user (or a
//! script) acts on, never an action doctor takes for them. Its only side effect is
//! [`ipc::probe_endpoint`]'s single, harmless probe connection, which the report discloses.

use crate::install::native_host::WowView;
use crate::install::{clients, host_file_path, native_host, Hive, PlanCtx};
use crate::transport::native::ipc::{self, EndpointProbe};
use crate::Result;
use std::path::PathBuf;

/// Options for `ghostlight doctor`.
pub struct DoctorOptions {
    /// Show every debug session (not just the newest few) with its per-session counters.
    pub verbose: bool,
}

/// Run the diagnosis; prints the report and returns `Ok(true)` when healthy (no findings).
///
/// One-shot and read-only: no tokio runtime is spawned, and nothing is written, deleted, or
/// killed. An `Err` is returned only when [`PlanCtx::resolve`] itself fails (e.g. no resolvable
/// home directory on this platform); every other failure -- an unreadable state file, a missing
/// log directory, no debug instrumentation at all -- degrades to a printed finding, never an
/// early return, so doctor always produces a report.
pub fn run(opts: DoctorOptions) -> Result<bool> {
    let ctx = PlanCtx::resolve()?;

    println!("ghostlight doctor");
    println!();
    println!("Binary:");
    println!("  {:<9}{}", "path", ctx.current_exe.display());
    println!("  {:<9}{}", "version", env!("CARGO_PKG_VERSION"));

    let browsers = browser_rows(&ctx);
    println!();
    println!("Browsers:");
    for (display, detected, registered) in &browsers {
        print_row(display, *detected, *registered);
    }
    let any_browser_registered = browsers.iter().any(|(_, _, registered)| *registered);

    let mcp_clients = client_rows(&ctx);
    println!();
    println!("MCP clients:");
    for (display, detected, registered) in &mcp_clients {
        print_row(display, *detected, *registered);
    }
    let any_client_registered = mcp_clients.iter().any(|(_, _, registered)| *registered);

    let manifest_status = crate::governance::manifest::identity::manifest_status();
    println!();
    println!("Policy manifest:");
    for line in crate::governance::manifest::identity::manifest_section_lines(&manifest_status) {
        println!("{line}");
    }

    println!();
    println!("Governance:");
    for line in governance_section_lines() {
        println!("{line}");
    }

    let endpoint = ipc::default_endpoint();
    let endpoint_display = ipc::endpoint_display(&endpoint);
    let probe = ipc::probe_endpoint(&endpoint);
    println!();
    println!("IPC endpoint:");
    println!("  {:<9}{}", "path", endpoint_display);
    println!("  {:<9}{}", "state", state_line(&probe));

    let (log_dir, rows) = gather_sessions();
    println!();
    print_sessions(&log_dir, &rows, opts.verbose);

    let sessions_present = rows.iter().any(|r| matches!(r, SessionRow::Parsed(_)));
    let newest_server = rows.iter().find_map(|r| match r {
        SessionRow::Parsed(s) if s.role == "mcp-server" => Some(NewestServer {
            pid: s.pid,
            extension_connected: s.extension_connected,
            connects: s.connects,
        }),
        _ => None,
    });

    let obs = Observations {
        any_browser_registered,
        any_client_registered,
        probe,
        sessions_present,
        newest_server,
    };
    let problems = findings(&obs);

    println!();
    println!("Verdict:");
    if problems.is_empty() {
        // A newest_server is guaranteed here: every rule that fires with no mcp-server session
        // (rules 3, 5) or a disconnected one (rule 6) pushes a finding, so an empty vector implies
        // Accepts + a connected newest_server (see `findings`'s doc comment).
        let pid = obs
            .newest_server
            .expect("empty findings implies a connected newest mcp-server session")
            .pid;
        println!(
            "  OK: mcp-server (pid {pid}) is running, the extension is connected, and the IPC endpoint accepts connections."
        );
        Ok(true)
    } else {
        for problem in &problems {
            println!("  problem: {problem}");
        }
        Ok(false)
    }
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn print_row(display: &str, detected: bool, registered: bool) {
    println!(
        "  {:<16} detected={:<5} registered={}",
        display,
        yn(detected),
        yn(registered)
    );
}

/// (display name, detected?, registered?) for each known browser -- the same detection and
/// registration checks the pre-fusion `install::run_doctor` used (Windows: HKCU native view or
/// HKLM both views has the key's default value; Unix: the host manifest file exists).
fn browser_rows(ctx: &PlanCtx) -> Vec<(String, bool, bool)> {
    native_host::BROWSERS
        .iter()
        .map(|b| {
            let detected = native_host::detect_browser(b, ctx);
            let registered = if cfg!(windows) {
                let key = native_host::win_reg_key(b);
                native_host::read_default(Hive::Hkcu, &key, WowView::Native).is_some()
                    || native_host::read_default(Hive::Hklm, &key, WowView::Both).is_some()
            } else {
                host_file_path(b, ctx).exists()
            };
            (b.display.to_string(), detected, registered)
        })
        .collect()
}

/// (display name, detected?, registered?) for each known MCP client (registered means the config
/// file contains the literal substring `"ghostlight"`, quotes included, as today).
fn client_rows(ctx: &PlanCtx) -> Vec<(String, bool, bool)> {
    clients::CLIENTS
        .iter()
        .map(|c| {
            let detected = clients::detect(c, ctx);
            let registered = std::fs::read_to_string(clients::config_path(c, ctx))
                .map(|s| s.contains("\"ghostlight\""))
                .unwrap_or(false);
            (c.display.to_string(), detected, registered)
        })
        .collect()
}

/// Body lines of the doctor "Governance:" section (g15, shared format doc section 9.2).
///
/// Doctor is a standalone, one-shot CLI invocation with no live `Governance`/session state
/// and no `--manifest` flag of its own (that flag is server-role only); it resolves its OWN
/// view of the active manifest the same way a server launched in the same environment would,
/// using the real, already-tested `governance::manifest::source::load_policy` (org policy file,
/// else `GHOSTLIGHT_MANIFEST`, else none -- the only manifest signal available without a CLI
/// flag) and the real layered config resolver, then renders through the SAME pure
/// `governance::dispatch::governance_status` function `Governance::governance_status` uses, so
/// this section and a future `get_status` reply can never disagree (g15 constraint 12). Any
/// resolution failure degrades to a printed line rather than propagating (doctor's own
/// never-early-return posture).
fn governance_section_lines() -> Vec<String> {
    let user_manifest_source = std::env::var("GHOSTLIGHT_MANIFEST").ok();
    let loaded_policy = match crate::governance::manifest::source::load_policy(
        user_manifest_source.as_deref(),
        crate::browser::pattern::is_valid_pattern,
    ) {
        Ok(loaded) => loaded,
        Err(e) => return vec![format!("  manifest source is broken: {e}")],
    };

    let config_store = crate::governance::config::reload::ConfigStore::load_initial_with_policy(
        crate::browser::pattern::is_valid_pattern,
        &loaded_policy,
        user_manifest_source,
    );
    let config = match config_store {
        Ok(store) => store.current(),
        Err(e) => return vec![format!("  config resolution is broken: {e}")],
    };
    let config_mode = config.governance_mode();

    let Some(manifest) = &loaded_policy.manifest else {
        return render_governance_status(None);
    };
    let status = crate::governance::dispatch::governance_status(
        &manifest.grants,
        manifest.mode,
        config_mode,
    );
    render_governance_status(Some(status))
}

/// The pure rendering half of [`governance_section_lines`] (g15): the exact three wordings
/// the task doc specifies, keyed off the already-resolved [`GovernanceStatus`]. Factored out
/// so the exact line text is unit-testable without touching the filesystem or environment.
fn render_governance_status(
    status: Option<crate::governance::dispatch::GovernanceStatus>,
) -> Vec<String> {
    match status {
        None => vec!["  no manifest active (all-open); no grant-based denials".to_string()],
        Some(s) if s.shadow => vec![
            "  mode  observe (SHADOW: would-deny events are recorded to the audit log but are \
             NOT blocked; this is observation, not protection)"
                .to_string(),
        ],
        Some(s) => vec![format!(
            "  mode  {} (denied calls are blocked)",
            s.mode.as_str()
        )],
    }
}

fn state_line(probe: &EndpointProbe) -> String {
    match probe {
        EndpointProbe::Accepts => {
            "accepts connections (doctor made one brief probe connection)".to_string()
        }
        EndpointProbe::Absent => "absent (no mcp-server currently owns it)".to_string(),
        EndpointProbe::Rejects(detail) => format!("exists but rejected the probe: {detail}"),
    }
}

// --- Debug sessions ---

/// One `debug-state-<pid>.json` file, newest-first: either it parsed into a [`Session`], or it
/// did not (unreadable, non-JSON, or missing the required `pid` field) and is named for the
/// "skipping" row.
enum SessionRow {
    Parsed(Session),
    Unreadable(String),
}

/// One parsed `debug-state-<pid>.json` session, tolerant of both current- and old-format files
/// (old files predate the `role`/`client` fields and every counter this crate has ever added).
#[derive(Debug, Clone)]
struct Session {
    role: String,
    pid: u64,
    started_ms: u64,
    updated_ms: u64,
    extension_connected: bool,
    client: Option<String>,
    mcp_requests: u64,
    tool_calls: u64,
    tool_errors: u64,
    frames_out: u64,
    frames_in: u64,
    connects: u64,
    disconnects: u64,
}

/// Parse one `debug-state-<pid>.json` body, tolerantly (`serde_json::Value` lookups, every field
/// but `pid` defaults when absent). Returns `None` when `raw` is not valid JSON or has no numeric
/// `pid` -- a session with no pid cannot be named in a report row, so it is treated the same as
/// an unreadable file (the "skipping unreadable state file" row).
fn parse_session(raw: &str) -> Option<Session> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let pid = v.get("pid").and_then(serde_json::Value::as_u64)?;
    let role = v
        .get("role")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("mcp-server")
        .to_string();
    let get_u64 = |k: &str| v.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0);
    let counters = v.get("counters");
    let cn = |k: &str| {
        counters
            .and_then(|c| c.get(k))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    Some(Session {
        role,
        pid,
        started_ms: get_u64("started_ms"),
        updated_ms: get_u64("updated_ms"),
        extension_connected: v
            .get("extension_connected")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        client: v
            .get("client")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        mcp_requests: cn("mcp_requests"),
        tool_calls: cn("tool_calls"),
        tool_errors: cn("tool_errors"),
        frames_out: cn("frames_out"),
        frames_in: cn("frames_in"),
        connects: cn("connects"),
        disconnects: cn("disconnects"),
    })
}

/// Read the log dir's session state files (already newest-first) and parse each one. `None` for
/// the directory itself means "no log directory available on this platform" -- distinct from an
/// empty (or absent) directory, which yields `Some(dir)` with an empty row list.
fn gather_sessions() -> (Option<PathBuf>, Vec<SessionRow>) {
    let Some(dir) = crate::debug::log_dir() else {
        return (None, Vec::new());
    };
    let rows = crate::debug::session_state_files(&dir)
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            match std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| parse_session(&raw))
            {
                Some(session) => SessionRow::Parsed(session),
                None => SessionRow::Unreadable(name),
            }
        })
        .collect();
    (Some(dir), rows)
}

/// Print the "Debug sessions" section. Without `--verbose`, shows at most 6 rows and, if more
/// *sessions* (not unreadable-file rows) were parsed than shown, a trailing "N older" note; with
/// `--verbose`, shows every row plus a `counters:` line under each parsed one. The final
/// "extension last seen" line always considers every parsed native-host session, not just the
/// shown ones -- it is a diagnostic signal, not a decorative row, and must not go stale under the
/// row cap.
fn print_sessions(log_dir: &Option<PathBuf>, rows: &[SessionRow], verbose: bool) {
    let Some(dir) = log_dir else {
        println!("Debug sessions:");
        println!("  (no log directory available on this platform)");
        return;
    };
    println!("Debug sessions ({}):", dir.display());
    if rows.is_empty() {
        println!("  (none found; a session run with --debug or GHOSTLIGHT_DEBUG=1 writes them)");
        return;
    }

    let total_parsed = rows
        .iter()
        .filter(|r| matches!(r, SessionRow::Parsed(_)))
        .count();
    let cap = if verbose {
        rows.len()
    } else {
        rows.len().min(6)
    };
    let now = crate::debug::now_ms();
    let mut shown_parsed = 0usize;
    for row in rows.iter().take(cap) {
        match row {
            SessionRow::Unreadable(name) => {
                println!("  (skipping unreadable state file: {name})");
            }
            SessionRow::Parsed(s) => {
                shown_parsed += 1;
                println!("{}", session_row(s, now));
                if verbose {
                    println!(
                        "      counters: requests={} tools={} errors={} frames_out={} frames_in={} connects={} disconnects={}",
                        s.mcp_requests, s.tool_calls, s.tool_errors, s.frames_out, s.frames_in,
                        s.connects, s.disconnects
                    );
                }
            }
        }
    }
    if !verbose && total_parsed > shown_parsed {
        println!(
            "  (and {} older; use --verbose to show all)",
            total_parsed - shown_parsed
        );
    }

    if let Some(newest_host) = rows.iter().find_map(|r| match r {
        SessionRow::Parsed(s) if s.role != "mcp-server" => Some(s),
        _ => None,
    }) {
        println!(
            "  extension last seen {} ago (native-host pid {})",
            crate::debug::fmt_ms(now.saturating_sub(newest_host.updated_ms as u128)),
            newest_host.pid
        );
    }
}

/// One session row. mcp-server sessions additionally show the recorded client and extension link
/// state; every other role (today only native-host) shows just pid + timing.
fn session_row(s: &Session, now: u128) -> String {
    let started_ago = crate::debug::fmt_ms(now.saturating_sub(s.started_ms as u128));
    let active_ago = crate::debug::fmt_ms(now.saturating_sub(s.updated_ms as u128));
    if s.role == "mcp-server" {
        format!(
            "  {:<12} pid {}  started {} ago  active {} ago  client {}  extension {}",
            s.role,
            s.pid,
            started_ago,
            active_ago,
            s.client.as_deref().unwrap_or("(not recorded)"),
            if s.extension_connected {
                "connected"
            } else {
                "not connected"
            }
        )
    } else {
        format!(
            "  {:<12} pid {}  started {} ago  active {} ago",
            s.role, s.pid, started_ago, active_ago
        )
    }
}

// --- Verdict ---

/// Everything [`findings`] needs, gathered once so the rule evaluation itself is a pure function.
struct Observations {
    any_browser_registered: bool,
    any_client_registered: bool,
    probe: EndpointProbe,
    /// True when at least one debug-state file parsed, of either role.
    sessions_present: bool,
    /// The newest parsed mcp-server session, if any.
    newest_server: Option<NewestServer>,
}

struct NewestServer {
    pid: u64,
    extension_connected: bool,
    connects: u64,
}

/// Evaluate the verdict rules, in order, against `obs`. Each rule appends at most one finding.
/// An empty result means every signal was healthy: the browser/client registration exists, the
/// endpoint accepted the probe, and the newest mcp-server session has the extension connected.
fn findings(obs: &Observations) -> Vec<String> {
    let mut out = Vec::new();

    if !obs.any_browser_registered {
        out.push(
            "the native messaging host is not registered for any browser: run ghostlight install, then reload the extension at chrome://extensions"
                .to_string(),
        );
    }
    if !obs.any_client_registered {
        out.push(
            "ghostlight is not registered with any MCP client: run ghostlight install".to_string(),
        );
    }

    match &obs.probe {
        EndpointProbe::Absent => {
            out.push(
                "no mcp-server is running (the IPC endpoint does not exist): start or restart your MCP client so it launches ghostlight"
                    .to_string(),
            );
        }
        EndpointProbe::Rejects(detail) => {
            out.push(match &obs.newest_server {
                Some(s) => format!(
                    "the IPC endpoint exists but rejected a connection ({detail}): a stale ghostlight process may still hold it; try killing pid {} and restarting your MCP client",
                    s.pid
                ),
                None => format!(
                    "the IPC endpoint exists but rejected a connection ({detail}): find and kill the stale ghostlight process with your process manager, then restart your MCP client"
                ),
            });
        }
        EndpointProbe::Accepts => match &obs.newest_server {
            None => out.push(
                "an mcp-server is running but wrote no debug state: restart the session with --debug (or GHOSTLIGHT_DEBUG=1) and re-run doctor for a full diagnosis"
                    .to_string(),
            ),
            Some(s) if !s.extension_connected => {
                if s.connects == 0 {
                    out.push(format!(
                        "the extension never connected in the newest session (pid {}): check that the extension is loaded and enabled at chrome://extensions and that the browser is running; if it persists, re-run ghostlight install and restart the browser",
                        s.pid
                    ));
                } else {
                    out.push(format!(
                        "the extension is disconnected from the mcp-server (pid {}; it connected {} time(s) earlier in this session): the extension service worker may be stopped; inspect it at chrome://extensions or restart the browser",
                        s.pid, s.connects
                    ));
                }
            }
            Some(_) => {}
        },
    }

    // Fires in addition to rule 3 or 4 (an Absent/Rejects endpoint with no debug instrumentation
    // at all is two distinct, independently actionable problems); Accepts already implies rule 5
    // covers the no-session case, so this never doubles up with it.
    if !obs.sessions_present && !matches!(obs.probe, EndpointProbe::Accepts) {
        out.push(
            "no debug instrumentation found: run a session with --debug (or set GHOSTLIGHT_DEBUG=1) and re-run doctor"
                .to_string(),
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_governance_status_none_is_the_all_open_line() {
        assert_eq!(
            render_governance_status(None),
            vec!["  no manifest active (all-open); no grant-based denials".to_string()]
        );
    }

    #[test]
    fn render_governance_status_shadow_true_prints_the_shadow_line() {
        let lines = render_governance_status(Some(crate::governance::dispatch::GovernanceStatus {
            mode: crate::governance::ports::EffectiveMode::Observe,
            shadow: true,
        }));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("mode  observe"));
        assert!(lines[0].contains("SHADOW"));
        assert!(lines[0].contains("NOT blocked"));
        assert!(lines[0].contains("observation, not protection"));
    }

    #[test]
    fn render_governance_status_enforce_prints_the_plain_line() {
        let lines = render_governance_status(Some(crate::governance::dispatch::GovernanceStatus {
            mode: crate::governance::ports::EffectiveMode::Enforce,
            shadow: false,
        }));
        assert_eq!(
            lines,
            vec!["  mode  enforce (denied calls are blocked)".to_string()]
        );
    }

    fn healthy_obs() -> Observations {
        Observations {
            any_browser_registered: true,
            any_client_registered: true,
            probe: EndpointProbe::Accepts,
            sessions_present: true,
            newest_server: Some(NewestServer {
                pid: 123,
                extension_connected: true,
                connects: 2,
            }),
        }
    }

    #[test]
    fn all_healthy_observations_produce_no_findings() {
        assert!(findings(&healthy_obs()).is_empty());
    }

    #[test]
    fn unregistered_browser_and_client_each_produce_their_own_finding() {
        let mut obs = healthy_obs();
        obs.any_browser_registered = false;
        let f = findings(&obs);
        assert!(f
            .iter()
            .any(|s| s.contains("not registered for any browser")));

        let mut obs = healthy_obs();
        obs.any_client_registered = false;
        let f = findings(&obs);
        assert!(f
            .iter()
            .any(|s| s.contains("not registered with any MCP client")));
    }

    #[test]
    fn absent_with_no_sessions_fires_exactly_rules_3_and_7_in_order() {
        let obs = Observations {
            any_browser_registered: true,
            any_client_registered: true,
            probe: EndpointProbe::Absent,
            sessions_present: false,
            newest_server: None,
        };
        let f = findings(&obs);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f[0].contains("no mcp-server is running"), "{f:?}");
        assert!(f[1].contains("no debug instrumentation found"), "{f:?}");
    }

    #[test]
    fn rejects_embeds_a_known_pid_and_falls_back_to_process_manager_without_one() {
        let mut obs = healthy_obs();
        obs.probe = EndpointProbe::Rejects("boom".into());
        let f = findings(&obs);
        assert!(f[0].contains("pid 123"), "{f:?}");
        assert!(f[0].contains("boom"), "{f:?}");

        let obs2 = Observations {
            any_browser_registered: true,
            any_client_registered: true,
            probe: EndpointProbe::Rejects("boom".into()),
            sessions_present: false,
            newest_server: None,
        };
        let f2 = findings(&obs2);
        assert!(f2[0].contains("process manager"), "{f2:?}");
    }

    #[test]
    fn accepts_with_no_server_session_fires_rule_5() {
        let obs = Observations {
            any_browser_registered: true,
            any_client_registered: true,
            probe: EndpointProbe::Accepts,
            sessions_present: false,
            newest_server: None,
        };
        let f = findings(&obs);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].contains("wrote no debug state"), "{f:?}");
    }

    #[test]
    fn accepts_with_a_disconnected_extension_distinguishes_never_connected_from_dropped() {
        let mut never = healthy_obs();
        never.newest_server = Some(NewestServer {
            pid: 5,
            extension_connected: false,
            connects: 0,
        });
        let f = findings(&never);
        assert!(f[0].contains("never connected"), "{f:?}");

        let mut dropped = healthy_obs();
        dropped.newest_server = Some(NewestServer {
            pid: 5,
            extension_connected: false,
            connects: 3,
        });
        let f2 = findings(&dropped);
        assert!(f2[0].contains("disconnected"), "{f2:?}");
        assert!(f2[0].contains("3 time(s)"), "{f2:?}");
    }

    #[test]
    fn parse_session_extracts_full_new_format_fields() {
        let raw = r#"{
            "pid": 42,
            "role": "mcp-server",
            "client": "claude-code 1.2.3",
            "started_ms": 1000,
            "updated_ms": 2000,
            "extension_connected": true,
            "counters": {
                "mcp_requests": 5, "tool_calls": 4, "tool_errors": 1,
                "frames_out": 10, "frames_in": 9, "connects": 2, "disconnects": 1
            }
        }"#;
        let s = parse_session(raw).unwrap();
        assert_eq!(s.pid, 42);
        assert_eq!(s.role, "mcp-server");
        assert_eq!(s.client.as_deref(), Some("claude-code 1.2.3"));
        assert!(s.extension_connected);
        assert_eq!(s.mcp_requests, 5);
        assert_eq!(s.tool_calls, 4);
        assert_eq!(s.tool_errors, 1);
        assert_eq!(s.frames_out, 10);
        assert_eq!(s.frames_in, 9);
        assert_eq!(s.connects, 2);
        assert_eq!(s.disconnects, 1);
    }

    #[test]
    fn parse_session_defaults_role_and_client_for_old_format_files() {
        let raw = r#"{
            "pid": 7, "started_ms": 1, "updated_ms": 2, "extension_connected": false,
            "counters": {}
        }"#;
        let s = parse_session(raw).unwrap();
        assert_eq!(s.role, "mcp-server");
        assert_eq!(s.client, None);
        assert_eq!(s.mcp_requests, 0);
    }

    #[test]
    fn parse_session_returns_none_for_garbage_or_a_missing_pid() {
        assert!(parse_session("not json").is_none());
        assert!(parse_session(r#"{"started_ms": 1, "role": "mcp-server"}"#).is_none());
    }
}
