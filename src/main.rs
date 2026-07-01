//! Browser MCP binary -- a thin shell over the `browser_mcp` library crate.
//!
//! Governed browser automation over the user's **own authenticated Chromium session**. In v1.0
//! this is the unconstrained engine (all-open); the governance overlay is a v1.5 addition.
//!
//! The same executable runs in several roles, selected at startup:
//! - **mcp-server** (default, no subcommand) -- launched by the MCP client over stdio. Owns the
//!   browser IPC endpoint, serves the native-host, and runs the JSON-RPC loop, forwarding tool
//!   calls to the extension via a shared [`Browser`](browser_mcp::browser::Browser) handle.
//! - **native-host** -- launched by Chrome via `connectNative` (Chrome passes the calling
//!   extension's origin, `chrome-extension://<id>/`, as an argument). Connects to the mcp-server
//!   endpoint and relays native-messaging frames to/from the extension.
//! - **install / uninstall / doctor** -- synchronous installer subcommands (no async runtime).
//!
//! `main` deliberately has no `#[tokio::main]`: the two async roles each build their own runtime,
//! and the installer needs none.

use anyhow::Result;
use browser_mcp::browser::Browser;
use browser_mcp::install::{DoctorOptions, InstallOptions, Selection, UninstallOptions};
use browser_mcp::native::ipc;
use clap::{Args, Parser, Subcommand};

/// Browser MCP -- the user's own authenticated browser, for AI agents.
#[derive(Debug, Parser)]
#[command(name = "browser-mcp", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// (server role) Capability-manifest source for the governance overlay (v1.5).
    /// Absent = all-open (the v1.0 default).
    #[arg(long, value_name = "SOURCE")]
    manifest: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Register the native host + add the MCP server to detected clients.
    Install(InstallArgs),
    /// Remove the native host registration + the MCP server from clients.
    Uninstall(UninstallArgs),
    /// Report browser + client detection and current registration state.
    Doctor(DoctorArgs),
}

#[derive(Debug, Args)]
struct InstallArgs {
    /// Unpacked-dev extension id (32 chars, a-p). Required until a build-time key ships.
    #[arg(long, value_name = "ID")]
    extension_id: Option<String>,
    /// Compute and print the plan; write nothing.
    #[arg(long)]
    dry_run: bool,
    /// System-wide registration (HKLM) instead of per-user (HKCU).
    #[arg(long)]
    system: bool,
    /// Register every known browser, not just detected ones.
    #[arg(long)]
    all_browsers: bool,
    /// Register only this browser id (repeatable): chrome, edge, brave, chromium.
    #[arg(long = "browser", value_name = "ID", conflicts_with = "all_browsers")]
    browsers: Vec<String>,
    /// Add to every known client, not just detected ones.
    #[arg(long)]
    all_clients: bool,
    /// Add only to this client id (repeatable): claude-code, claude-desktop, cursor, vscode.
    #[arg(long = "client", value_name = "ID", conflicts_with = "all_clients")]
    clients: Vec<String>,
}

#[derive(Debug, Args)]
struct UninstallArgs {
    /// Compute and print the plan; write nothing.
    #[arg(long)]
    dry_run: bool,
    /// System-wide (HKLM) instead of per-user (HKCU).
    #[arg(long)]
    system: bool,
    /// Act on every known browser, not just detected ones.
    #[arg(long)]
    all_browsers: bool,
    /// Act only on this browser id (repeatable).
    #[arg(long = "browser", value_name = "ID", conflicts_with = "all_browsers")]
    browsers: Vec<String>,
    /// Act on every known client, not just detected ones.
    #[arg(long)]
    all_clients: bool,
    /// Act only on this client id (repeatable).
    #[arg(long = "client", value_name = "ID", conflicts_with = "all_clients")]
    clients: Vec<String>,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Print extra detail.
    #[arg(long)]
    verbose: bool,
}

/// Resolve a `Selection` from `--<thing>` (Only) / `--all-<things>` (ForceAll) / default (All).
fn selection(only: Vec<String>, force_all: bool) -> Selection {
    if !only.is_empty() {
        Selection::Only(only)
    } else if force_all {
        Selection::ForceAll
    } else {
        Selection::All
    }
}

impl From<InstallArgs> for InstallOptions {
    fn from(a: InstallArgs) -> Self {
        InstallOptions {
            extension_id: a.extension_id,
            dry_run: a.dry_run,
            system: a.system,
            browsers: selection(a.browsers, a.all_browsers),
            clients: selection(a.clients, a.all_clients),
        }
    }
}

impl From<UninstallArgs> for UninstallOptions {
    fn from(a: UninstallArgs) -> Self {
        UninstallOptions {
            dry_run: a.dry_run,
            system: a.system,
            browsers: selection(a.browsers, a.all_browsers),
            clients: selection(a.clients, a.all_clients),
        }
    }
}

impl From<DoctorArgs> for DoctorOptions {
    fn from(a: DoctorArgs) -> Self {
        DoctorOptions { verbose: a.verbose }
    }
}

fn main() -> Result<()> {
    browser_mcp::init_tracing();

    // Role detection must precede clap: Chrome launches the native-messaging host with an extra
    // positional arg (the calling extension origin) that clap would reject.
    if std::env::args().any(|a| a.starts_with("chrome-extension://")) {
        return run_native_host_role();
    }

    match Cli::parse() {
        Cli {
            command: Some(Command::Install(args)),
            ..
        } => browser_mcp::install::run_install(args.into())?,
        Cli {
            command: Some(Command::Uninstall(args)),
            ..
        } => browser_mcp::install::run_uninstall(args.into())?,
        Cli {
            command: Some(Command::Doctor(args)),
            ..
        } => browser_mcp::install::run_doctor(args.into())?,
        Cli {
            command: None,
            manifest,
        } => run_server(manifest)?,
    }
    Ok(())
}

/// Native-host role: relay native-messaging frames between Chrome (stdio) and the mcp-server (IPC).
fn run_native_host_role() -> Result<()> {
    tracing::info!("browser-mcp starting (native-host role, launched by the browser)");
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { ipc::relay_native_host(&ipc::default_endpoint()).await })?;
    Ok(())
}

/// mcp-server role: own the browser IPC endpoint + serve the native-host in the background, run the
/// stdio MCP JSON-RPC loop in the foreground. Both share the [`Browser`] handle.
fn run_server(manifest: Option<String>) -> Result<()> {
    tracing::info!(
        ?manifest,
        "browser-mcp starting (mcp-server role; v1.0 engine -- all-open, no governance overlay)"
    );
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let browser = Browser::new();
        let endpoint = ipc::default_endpoint();
        tokio::spawn({
            let browser = browser.clone();
            async move {
                match ipc::serve(browser, &endpoint).await {
                    Ok(()) => {}
                    Err(browser_mcp::Error::SessionBusy) => tracing::warn!(
                        "another browser-mcp session already owns the browser; tool calls in this \
                         session will report the extension as unavailable"
                    ),
                    Err(e) => tracing::error!(error = %e, "browser IPC endpoint failed"),
                }
            }
        });
        browser_mcp::mcp::server::run(browser).await
    })?;
    Ok(())
}
