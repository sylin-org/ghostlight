//! Ghostlight binary -- a thin shell over the `ghostlight` library crate.
//!
//! Governed browser automation over the user's **own authenticated Chromium session**. In v1.0
//! this is the unconstrained engine (all-open); the governance overlay is a v1.5 addition.
//!
//! The same executable runs in several roles, selected at startup:
//! - **mcp-server** (default, no subcommand) -- launched by the MCP client over stdio. Owns the
//!   browser IPC endpoint, serves the native-host, and runs the JSON-RPC loop, forwarding tool
//!   calls to the extension via a shared
//!   [`Browser`](ghostlight::transport::executor::Browser) handle.
//! - **native-host** -- launched by Chrome via `connectNative` (Chrome passes the calling
//!   extension's origin, `chrome-extension://<id>/`, as an argument). Connects to the mcp-server
//!   endpoint and relays native-messaging frames to/from the extension.
//! - **install / uninstall / doctor** -- synchronous installer subcommands (no async runtime).
//!
//! `main` deliberately has no `#[tokio::main]`: the two async roles each build their own runtime,
//! and the installer needs none.

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use ghostlight::browser::pattern;
use ghostlight::debug::DebugSink;
use ghostlight::doctor::DoctorOptions;
use ghostlight::governance::manifest::source;
use ghostlight::install::{InstallOptions, Selection, UninstallOptions};
use ghostlight::native::ipc;
use ghostlight::transport::executor::Browser;

/// Ghostlight -- the user's own authenticated browser, for AI agents.
#[derive(Debug, Parser)]
#[command(name = "ghostlight", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// (server role) Capability-manifest source for the governance overlay (v1.5).
    /// Absent = all-open (the v1.0 default).
    #[arg(long, value_name = "SOURCE")]
    manifest: Option<String>,

    /// (server role) Enable observability: verbose tracing + a live state/event log that
    /// `ghostlight status` reads. Equivalent to setting `GHOSTLIGHT_DEBUG=1`.
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
    /// Inspect and edit the layered configuration (list / get / set / schema / docs).
    Config(ConfigArgs),
    /// Inspect and preview policy files.
    Policy(PolicyArgs),
}

#[derive(Debug, Args)]
struct PolicyArgs {
    #[command(subcommand)]
    command: PolicyCommand,
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// Render a policy manifest or config file as plain sentences.
    Explain(ExplainArgs),
    /// Replay recorded audit events through a candidate manifest.
    Simulate(SimulateArgs),
    /// Write an embedded example manifest as a starting point.
    Init(InitArgs),
}

#[derive(Debug, Args)]
struct ExplainArgs {
    /// Path to a policy manifest or a user configuration file.
    #[arg(value_name = "FILE")]
    file: std::path::PathBuf,
}

#[derive(Debug, Args)]
struct SimulateArgs {
    /// Path to the candidate policy manifest.
    #[arg(value_name = "MANIFEST")]
    manifest: std::path::PathBuf,
    /// Path to the audit JSON Lines file to replay.
    #[arg(long, value_name = "FILE")]
    replay: std::path::PathBuf,
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Embedded template name: enterprise-healthcare, developer-unrestricted, or qa-staging.
    #[arg(long, value_name = "NAME")]
    template: String,
    /// Output path. Defaults to policy.json in the current working directory.
    #[arg(long, value_name = "PATH")]
    out: Option<std::path::PathBuf>,
    /// Overwrite an existing output file.
    #[arg(long)]
    force: bool,
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
    /// Print the JSON Schema (draft 2020-12) for the user configuration file.
    Schema,
    /// Print the markdown key reference generated from the key registry.
    Docs,
    /// Select a named bundle of layer-4 defaults, after previewing what changes.
    Preset(PresetArgs),
}

#[derive(Debug, Args)]
struct PresetArgs {
    /// The preset to select.
    #[arg(value_enum)]
    preset: CliPreset,
    /// Print the diff and write nothing.
    #[arg(long)]
    dry_run: bool,
}

/// The CLI-facing spelling of a preset (hyphenated). The underscore alias on `FullyOpen` lets
/// `fully_open` (the wire form written to the user config file) also be typed directly.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliPreset {
    #[value(name = "fully-open", alias = "fully_open")]
    FullyOpen,
    Safe,
    Restricted,
}

impl From<CliPreset> for ghostlight::governance::config::Preset {
    fn from(p: CliPreset) -> Self {
        use ghostlight::governance::config::Preset;
        match p {
            CliPreset::FullyOpen => Preset::FullyOpen,
            CliPreset::Safe => Preset::Safe,
            CliPreset::Restricted => Preset::Restricted,
        }
    }
}

impl From<ConfigArgs> for ghostlight::governance::config::cli::ConfigCommand {
    fn from(a: ConfigArgs) -> Self {
        use ghostlight::governance::config::cli::ConfigCommand;
        match a.action {
            ConfigAction::List => ConfigCommand::List,
            ConfigAction::Get { key } => ConfigCommand::Get { key },
            ConfigAction::Set { key, value } => ConfigCommand::Set { key, value },
            ConfigAction::Schema => ConfigCommand::Schema,
            ConfigAction::Docs => ConfigCommand::Docs,
            ConfigAction::Preset(PresetArgs { preset, dry_run }) => ConfigCommand::Preset {
                preset: preset.into(),
                dry_run,
            },
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
    /// Register the server to run in debug mode (sets GHOSTLIGHT_DEBUG=1 in its env).
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
    let debug_env = std::env::var_os("GHOSTLIGHT_DEBUG").is_some();
    let debug = debug_env || std::env::args().any(|a| a == "--debug");
    ghostlight::init_tracing(debug);

    // Role detection must precede clap: Chrome launches the native-messaging host with an extra
    // positional arg (the calling extension origin) that clap would reject.
    if std::env::args().any(|a| a.starts_with("chrome-extension://")) {
        return run_native_host_role(debug);
    }

    match Cli::parse() {
        Cli {
            command: Some(Command::Install(args)),
            ..
        } => ghostlight::install::run_install(args.into())?,
        Cli {
            command: Some(Command::Uninstall(args)),
            ..
        } => ghostlight::install::run_uninstall(args.into())?,
        Cli {
            command: Some(Command::Doctor(args)),
            ..
        } => {
            if !ghostlight::doctor::run(args.into())? {
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
        } => ghostlight::governance::config::cli::run(
            args.into(),
            ghostlight::browser::pattern::is_valid_pattern,
        )?,
        Cli {
            command:
                Some(Command::Policy(PolicyArgs {
                    command: PolicyCommand::Explain(ExplainArgs { file }),
                })),
            ..
        } => {
            let text = ghostlight::governance::explain::explain_file(
                &file,
                ghostlight::browser::pattern::is_valid_pattern,
            )?;
            print!("{text}");
        }
        Cli {
            command:
                Some(Command::Policy(PolicyArgs {
                    command: PolicyCommand::Simulate(SimulateArgs { manifest, replay }),
                })),
            ..
        } => {
            use std::io::Write;
            let outcome = ghostlight::governance::simulate::run_simulate(
                &manifest,
                &replay,
                ghostlight::browser::pattern::is_valid_pattern,
                ghostlight::browser::directory::requires,
                ghostlight::browser::polarity::evaluate_host,
            )?;
            print!("{}", outcome.report);
            std::io::stdout().flush().ok();
            std::process::exit(if outcome.would_deny == 0 { 0 } else { 2 });
        }
        Cli {
            command:
                Some(Command::Policy(PolicyArgs {
                    command:
                        PolicyCommand::Init(InitArgs {
                            template,
                            out,
                            force,
                        }),
                })),
            ..
        } => {
            let out_path = out.unwrap_or_else(|| std::path::PathBuf::from("policy.json"));
            let outcome = ghostlight::governance::templates::run_init(&template, &out_path, force)?;
            print!(
                "{}",
                ghostlight::governance::templates::render_orientation(&outcome)
            );
        }
        Cli {
            command: None,
            manifest,
            debug: debug_flag,
        } => run_server(manifest, debug_flag || debug_env)?,
    }
    Ok(())
}

/// `ghostlight status`: read and print the running server's live inner state.
fn run_status(args: StatusArgs) {
    if args.json {
        match ghostlight::debug::raw_state() {
            Some(s) => print!("{s}"),
            None => println!("no debug state found (start the server with --debug)"),
        }
    } else {
        println!("{}", ghostlight::debug::status_report());
    }
}

/// Native-host role: relay native-messaging frames between Chrome (stdio) and the mcp-server (IPC).
///
/// `debug` comes from the same detection `main` uses for every role (env var or `--debug`
/// argument), but Chrome itself never passes `--debug` when it launches this process -- it only
/// inherits whatever environment Chrome was started with. So a native-host debug snapshot exists
/// only when Chrome's own launching environment had `GHOSTLIGHT_DEBUG=1` set; its absence in a
/// normal launch is expected, not a problem (see `doctor`'s wording).
fn run_native_host_role(debug: bool) -> Result<()> {
    tracing::info!("ghostlight starting (native-host role, launched by the browser)");
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
                    Err(ghostlight::Error::SessionBusy) => tracing::warn!(
                        "another ghostlight session already owns the browser; tool calls in this \
                         session will report the extension as unavailable"
                    ),
                    Err(e) => tracing::error!(error = %e, "browser IPC endpoint failed"),
                }
            }
        });
        let result = ghostlight::mcp::server::run(browser, loaded_policy, user_source).await;
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
    let Some(dir) = ghostlight::debug::log_dir() else {
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
