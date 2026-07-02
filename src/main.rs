//! Browser MCP binary -- a thin shell over the `browser_mcp` library crate.
//!
//! Governed browser automation over the user's **own authenticated Chromium session**. In v1.0
//! this is the unconstrained engine (all-open); the governance overlay is a v1.5 addition.
//!
//! The same executable runs in several roles, selected at startup:
//! - **mcp-server** (default, no subcommand) -- launched by the MCP client over stdio. Owns the
//!   browser IPC endpoint, serves the native-host, and runs the JSON-RPC loop, forwarding tool
//!   calls to the extension via a shared
//!   [`Browser`](browser_mcp::transport::executor::Browser) handle.
//! - **native-host** -- launched by Chrome via `connectNative` (Chrome passes the calling
//!   extension's origin, `chrome-extension://<id>/`, as an argument). Connects to the mcp-server
//!   endpoint and relays native-messaging frames to/from the extension.
//! - **install / uninstall / doctor** -- synchronous installer subcommands (no async runtime).
//!
//! `main` deliberately has no `#[tokio::main]`: the two async roles each build their own runtime,
//! and the installer needs none.

use anyhow::Result;
use browser_mcp::debug::DebugSink;
use browser_mcp::doctor::DoctorOptions;
use browser_mcp::install::{InstallOptions, Selection, UninstallOptions};
use browser_mcp::native::ipc;
use browser_mcp::transport::executor::Browser;
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

    /// (server role) Enable observability: verbose tracing + a live state/event log that
    /// `browser-mcp status` reads. Equivalent to setting `BROWSER_MCP_DEBUG=1`.
    #[arg(long)]
    debug: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Register the native host + add the MCP server to detected clients.
    Install(InstallArgs),
    /// Remove the native host registration + the MCP server from clients.
    Uninstall(UninstallArgs),
    /// Diagnose the whole chain: registration, debug sessions, IPC endpoint, extension link.
    Doctor(DoctorArgs),
    /// Show the running server's live inner state (needs a server started with --debug).
    Status(StatusArgs),
    /// Inspect and edit the layered configuration (list / get / set).
    Config(ConfigArgs),
}

#[derive(Debug, Args)]
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Show every key: effective value, source layer, lock state, description.
    List,
    /// Show one key's effective value, source layer, and lock state.
    Get {
        /// The dotted key name (see 'config list').
        key: String,
    },
    /// Set a key in the user layer. Refused when the organization locks the key.
    Set {
        /// The dotted key name.
        key: String,
        /// The raw value (bool: true/false; uint: digits; enum/string: verbatim;
        /// string list: a JSON array, e.g. ["example.com","*.example.com"]).
        value: String,
    },
}

impl From<ConfigArgs> for browser_mcp::governance::config::cli::ConfigCommand {
    fn from(a: ConfigArgs) -> Self {
        use browser_mcp::governance::config::cli::ConfigCommand;
        match a.action {
            ConfigAction::List => ConfigCommand::List,
            ConfigAction::Get { key } => ConfigCommand::Get { key },
            ConfigAction::Set { key, value } => ConfigCommand::Set { key, value },
        }
    }
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
    /// Register the server to run in debug mode (sets BROWSER_MCP_DEBUG=1 in its env).
    #[arg(long)]
    debug: bool,
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

#[derive(Debug, Args)]
struct StatusArgs {
    /// Print the raw debug-state.json instead of the formatted report.
    #[arg(long)]
    json: bool,
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
            debug: a.debug,
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
    // Debug mode can come from the flag (any position) or the env; detect it before clap so tracing
    // verbosity is set for every role, including the native-host relay.
    let debug_env = std::env::var_os("BROWSER_MCP_DEBUG").is_some();
    let debug = debug_env || std::env::args().any(|a| a == "--debug");
    browser_mcp::init_tracing(debug);

    // Role detection must precede clap: Chrome launches the native-messaging host with an extra
    // positional arg (the calling extension origin) that clap would reject.
    if std::env::args().any(|a| a.starts_with("chrome-extension://")) {
        return run_native_host_role(debug);
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
        } => {
            if !browser_mcp::doctor::run(args.into())? {
                std::process::exit(1);
            }
        }
        Cli {
            command: Some(Command::Status(args)),
            ..
        } => run_status(args),
        Cli {
            command: Some(Command::Config(args)),
            ..
        } => browser_mcp::governance::config::cli::run(
            args.into(),
            browser_mcp::browser::pattern::is_valid_pattern,
        )?,
        Cli {
            command: None,
            manifest,
            debug: debug_flag,
        } => run_server(manifest, debug_flag || debug_env)?,
    }
    Ok(())
}

/// `browser-mcp status`: read and print the running server's live inner state.
fn run_status(args: StatusArgs) {
    if args.json {
        match browser_mcp::debug::raw_state() {
            Some(s) => print!("{s}"),
            None => println!("no debug state found (start the server with --debug)"),
        }
    } else {
        println!("{}", browser_mcp::debug::status_report());
    }
}

/// Native-host role: relay native-messaging frames between Chrome (stdio) and the mcp-server (IPC).
///
/// `debug` comes from the same detection `main` uses for every role (env var or `--debug`
/// argument), but Chrome itself never passes `--debug` when it launches this process -- it only
/// inherits whatever environment Chrome was started with. So a native-host debug snapshot exists
/// only when Chrome's own launching environment had `BROWSER_MCP_DEBUG=1` set; its absence in a
/// normal launch is expected, not a problem (see `doctor`'s wording).
fn run_native_host_role(debug: bool) -> Result<()> {
    tracing::info!("browser-mcp starting (native-host role, launched by the browser)");
    let sink = build_debug_sink(debug, "native-host");
    let rt = tokio::runtime::Runtime::new()?;
    let result =
        rt.block_on(async { ipc::relay_native_host(&ipc::default_endpoint(), &sink).await });
    if let Err(e) = result {
        tracing::warn!(error = %e, "native-host relay ended with error");
    }
    sink.flush();
    // The relay has ended (the mcp-server or the extension went away). Exit the process directly
    // instead of returning: tokio's stdin reader parks a blocking thread in a ReadFile on Chrome's
    // still-open stdin, and dropping the runtime would hang forever trying to join it. This role is
    // a stateless relay with nothing else to flush, so an immediate exit is correct -- and it lets
    // Chrome observe the disconnect and reconnect to the next mcp-server session (no zombie).
    tracing::info!("native-host relay ended; exiting");
    std::process::exit(0);
}

/// mcp-server role: own the browser IPC endpoint + serve the native-host in the background, run the
/// stdio MCP JSON-RPC loop in the foreground. Both share the [`Browser`] handle.
fn run_server(manifest: Option<String>, debug_on: bool) -> Result<()> {
    tracing::info!(
        ?manifest,
        debug_mode = debug_on,
        "browser-mcp starting (mcp-server role; v1.0 engine -- all-open, no governance overlay)"
    );
    let sink = build_debug_sink(debug_on, "mcp-server");
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let browser = Browser::with_debug(sink.clone());
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
        let result = browser_mcp::mcp::server::run(browser).await;
        sink.flush(); // final snapshot after stdin closes
        result
    })?;
    Ok(())
}

/// Build the observability sink for `role` ("mcp-server" or "native-host"). Debug-off yields a
/// no-op sink; if the log directory cannot be prepared we warn and continue without observability
/// rather than failing the process.
fn build_debug_sink(debug: bool, role: &'static str) -> DebugSink {
    if !debug {
        return DebugSink::disabled();
    }
    let Some(dir) = browser_mcp::debug::log_dir() else {
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
