// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Ghostlight binary -- a thin shell over the `ghostlight` library crate.
//!
//! Governed browser automation over the user's **own authenticated Chromium session**. In v1.0
//! this is the unconstrained engine (all-open); the governance overlay is a v1.5 addition.
//!
//! Since ADR-0046 this executable is the CLI + the standalone SERVICE; the thin pass-through relay
//! ships as a SEPARATE executable, so a service rebuild never relinks (locks) it:
//! - **service** (`ghostlight service`) -- the STANDALONE, persistent Hub. Owns the browser IPC
//!   endpoint and the adapter/control endpoint for its whole life, multiplexes any number of
//!   adapter sessions through the one governance chokepoint, and shuts down only on a continuous
//!   idle-grace window (never on any client's death).
//! - **install / uninstall / doctor / status / config / policy** -- synchronous subcommands.
//! - a BARE `ghostlight` (no subcommand) no longer serves MCP: it prints guidance pointing at
//!   `ghostlight-relay` and exits 2 (ADR-0046). The single `ghostlight-relay` binary carries both
//!   pass-through roles (MCP-client agent + Chrome native-messaging browser), selected at launch
//!   (ADR-0051 Phase 3); it is its own crate.
//!
//! `main` deliberately has no `#[tokio::main]`: the async roles each build their own runtime, and
//! the installer needs none.

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use ghostlight::hub::manage::doctor::DoctorOptions;
use ghostlight::install::{InstallOptions, Selection, UninstallOptions};

mod demo;

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

    /// Select a named, isolated instance (ADR-0044): its own endpoint, native host, MCP server
    /// name, supervisor, and user-config/log dirs, coexisting with the default deploy. Omit for the
    /// default instance. Also settable via GHOSTLIGHT_INSTANCE.
    #[arg(long, value_name = "NAME", global = true)]
    instance: Option<String>,

    /// (service role) Keep the service warm: skip the idle-grace shutdown so a terminal-run dev
    /// service stays up between actions (ADR-0045). Supervisor-launched services keep idle-grace.
    #[arg(long, global = true)]
    keep_warm: bool,
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
    /// Run the persistent Ghostlight Hub service (owns the browser link; multiplexes clients).
    Service,
    /// Drive a scripted tour of the public demo stage (sylin.org/ghostlight/demo).
    Demo(DemoArgs),
    /// Show or install a Ghostlight license (state never affects behavior; ADR-0028).
    License(LicenseArgs),
}

#[derive(Debug, Args)]
struct DemoArgs {
    /// The demo stage base URL. Defaults to the live site; override for a local `eleventy --serve`
    /// (e.g. http://localhost:8080/ghostlight/demo).
    #[arg(long, default_value = "https://sylin.org/ghostlight/demo")]
    base_url: String,
    /// Seconds to pause after each visible step so you can watch it happen. Default 3.
    #[arg(long, default_value_t = 3.0)]
    pause: f64,
    /// Seconds to wait right after the demo tab opens, so you can resize and position the
    /// browser window before the tour starts. Default 10.
    #[arg(long, default_value_t = 10.0)]
    setup_pause: f64,
    /// Seconds to breathe between the tour's sections (Desk, Form, Signals, ...). Default 5.
    #[arg(long, default_value_t = 5.0)]
    section_pause: f64,
}

#[derive(Debug, Args)]
struct LicenseArgs {
    #[command(subcommand)]
    command: LicenseSubcommand,
}

#[derive(Debug, Subcommand)]
enum LicenseSubcommand {
    /// Show the resolved license state (read-only; never affects behavior).
    Status {
        /// Inspect a specific license file or armored block instead of the installed one.
        #[arg(long, value_name = "PATH")]
        file: Option<std::path::PathBuf>,
    },
    /// Install a license from a file, an armored block, or stdin.
    Install {
        /// Path to a license file or armored block; omit to read from stdin.
        #[arg(value_name = "PATH")]
        source: Option<std::path::PathBuf>,
        /// Install to the org-wide location instead of the per-user location.
        #[arg(long)]
        org: bool,
    },
    /// (offline authoring) Sign claims into a license envelope.
    #[cfg(feature = "license-admin")]
    Sign {
        /// 32-byte Ed25519 seed file.
        #[arg(long, value_name = "FILE")]
        seed: std::path::PathBuf,
        /// 32-byte ML-DSA seed file (required for keygen >= 1, the composite generations).
        #[arg(long, value_name = "FILE")]
        mldsa_seed: Option<std::path::PathBuf>,
        /// Key generation index (0 = the public evaluation key; 1+ = composite production keys).
        #[arg(long)]
        keygen: u32,
        /// Claims JSON file.
        #[arg(long, value_name = "FILE")]
        claims: std::path::PathBuf,
        /// Output envelope path (default license.json). The armored block is printed to stdout.
        #[arg(long, value_name = "FILE")]
        out: Option<std::path::PathBuf>,
    },
    /// (offline authoring) Print the verifying key(s) for a seed, for embedding.
    #[cfg(feature = "license-admin")]
    Pubkey {
        /// 32-byte Ed25519 seed file.
        #[arg(long, value_name = "FILE")]
        seed: std::path::PathBuf,
        /// 32-byte ML-DSA seed file (prints the composite public key too).
        #[arg(long, value_name = "FILE")]
        mldsa_seed: Option<std::path::PathBuf>,
    },
}

impl From<LicenseArgs> for ghostlight::governance::license::cli::LicenseCommand {
    fn from(a: LicenseArgs) -> Self {
        use ghostlight::governance::license::cli::LicenseCommand;
        match a.command {
            LicenseSubcommand::Status { file } => LicenseCommand::Status { file },
            LicenseSubcommand::Install { source, org } => LicenseCommand::Install { source, org },
            #[cfg(feature = "license-admin")]
            LicenseSubcommand::Sign {
                seed,
                mldsa_seed,
                keygen,
                claims,
                out,
            } => LicenseCommand::Sign {
                seed,
                mldsa_seed,
                keygen,
                claims,
                out,
            },
            #[cfg(feature = "license-admin")]
            LicenseSubcommand::Pubkey { seed, mldsa_seed } => {
                LicenseCommand::Pubkey { seed, mldsa_seed }
            }
        }
    }
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
    /// (org authoring) Sign a manifest into a managed:// policy bundle.
    Sign(PolicySignArgs),
    /// (org authoring) Print the org verifying key(s) for the managed.json bootstrap.
    Pubkey(PolicyPubkeyArgs),
    /// (org authoring) Sign a manifest and emit a ready managed.json bootstrap snippet.
    Publish(PolicySignArgs),
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
struct PolicySignArgs {
    /// 32-byte Ed25519 seed file (the org's private signing seed; e.g. `openssl rand 32`).
    #[arg(long, value_name = "FILE")]
    seed: std::path::PathBuf,
    /// 32-byte ML-DSA-65 seed file (composite / production signing; omit for an evaluation-grade
    /// Ed25519-only bundle).
    #[arg(long, value_name = "FILE")]
    mldsa_seed: Option<std::path::PathBuf>,
    /// Monotonic publish sequence: increase it with every release (anti-rollback, ADR-0055).
    #[arg(long)]
    seq: u64,
    /// Path to the policy manifest JSON to sign.
    #[arg(value_name = "MANIFEST")]
    manifest: std::path::PathBuf,
    /// Output bundle path (default policy.bundle.json). The armored block prints to stdout.
    #[arg(long, value_name = "FILE")]
    out: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
struct PolicyPubkeyArgs {
    /// 32-byte Ed25519 seed file.
    #[arg(long, value_name = "FILE")]
    seed: std::path::PathBuf,
    /// 32-byte ML-DSA-65 seed file (for a composite / production key).
    #[arg(long, value_name = "FILE")]
    mldsa_seed: Option<std::path::PathBuf>,
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
    /// Extra extension id to allow (the Web Store and unpacked-dev ids are always allowed).
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
    /// Add only to this client id (repeatable): claude-code, claude-desktop, cursor, vscode, codex.
    #[arg(long = "client", value_name = "ID", conflicts_with = "all_clients")]
    clients: Vec<String>,
    /// Register the server to run in debug mode (sets GHOSTLIGHT_DEBUG=1 in its env).
    #[arg(long)]
    debug: bool,
    /// Skip registering the OS auto-start supervisor (dev instances run 'ghostlight service' in
    /// a terminal instead).
    #[arg(long)]
    no_supervisor: bool,
    /// Do not open the browser-extension walkthrough after installation.
    #[arg(long)]
    no_open: bool,
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
    /// Repair, not just report: reap orphaned mcp-server sessions (alive process, exited client)
    /// and clear stale state files. The only doctor mode that kills or deletes anything (ADR-0029).
    #[arg(long)]
    fix: bool,
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
            no_supervisor: a.no_supervisor,
            no_open: a.no_open,
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
        DoctorOptions {
            verbose: a.verbose,
            fix: a.fix,
        }
    }
}

fn main() -> Result<()> {
    // Debug mode can come from the flag (any position) or the env; detect it before clap so tracing
    // verbosity is set for every role, including the native-host relay.
    let debug_env = std::env::var_os("GHOSTLIGHT_DEBUG").is_some();
    let debug = debug_env || std::env::args().any(|a| a == "--debug");
    ghostlight::init_tracing(debug);

    // Resolve the active instance (ADR-0044) BEFORE parsing the subcommand, and fold the winner
    // into GHOSTLIGHT_INSTANCE so every point-of-use derivation agrees. A malformed name is fatal
    // here, not silently degraded.
    if let Err(e) = resolve_instance() {
        eprintln!("ghostlight: {e}");
        std::process::exit(2);
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
            if !ghostlight::hub::manage::doctor::run(args.into())? {
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
            command:
                Some(Command::Policy(PolicyArgs {
                    command:
                        PolicyCommand::Sign(PolicySignArgs {
                            seed,
                            mldsa_seed,
                            seq,
                            manifest,
                            out,
                        }),
                })),
            ..
        } => ghostlight::governance::managed::cli::sign(seed, mldsa_seed, seq, manifest, out)?,
        Cli {
            command:
                Some(Command::Policy(PolicyArgs {
                    command: PolicyCommand::Pubkey(PolicyPubkeyArgs { seed, mldsa_seed }),
                })),
            ..
        } => ghostlight::governance::managed::cli::pubkey(seed, mldsa_seed)?,
        Cli {
            command:
                Some(Command::Policy(PolicyArgs {
                    command:
                        PolicyCommand::Publish(PolicySignArgs {
                            seed,
                            mldsa_seed,
                            seq,
                            manifest,
                            out,
                        }),
                })),
            ..
        } => ghostlight::governance::managed::cli::publish(seed, mldsa_seed, seq, manifest, out)?,
        Cli {
            command: Some(Command::Service),
            manifest,
            debug: debug_flag,
            keep_warm,
            ..
        } => ghostlight::hub::run_service(manifest, debug_flag || debug_env, keep_warm)?,
        Cli {
            command: Some(Command::Demo(args)),
            ..
        } => demo::run(
            &args.base_url,
            demo::Pacing {
                step_secs: args.pause,
                setup_secs: args.setup_pause,
                section_secs: args.section_pause,
            },
        )?,
        Cli {
            command: Some(Command::License(args)),
            ..
        } => ghostlight::governance::license::cli::run(args.into())?,
        Cli { command: None, .. } => {
            // ADR-0046 + ADR-0051 Phase 3: the bare `ghostlight` no longer serves MCP -- the MCP
            // client launches `ghostlight-relay --role agent`, which relays to the running service.
            eprintln!(
                "ghostlight no longer serves MCP directly; your MCP client launches ghostlight-relay."
            );
            eprintln!(
                "Run `ghostlight install` to update client registrations, then restart your editor."
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

/// Resolve the active instance (ADR-0044) and fold the winner into `GHOSTLIGHT_INSTANCE`, in
/// precedence order: the `--instance` flag, then an already-set `GHOSTLIGHT_INSTANCE`, then the
/// `argv[0]` basename (the multi-call native-host signal), then the default. Returns the
/// validation error for a malformed name so `main` can exit non-zero with a clear message rather
/// than silently degrading to the default (which could collide with a governed default install).
fn resolve_instance() -> std::result::Result<(), String> {
    use ghostlight::instance::Instance;
    // 1. An explicit --instance flag wins. An empty value means the default (leave the env unset).
    if let Some(flag) = instance_flag_value() {
        let name = flag.trim();
        if name.is_empty() {
            return Ok(());
        }
        Instance::validate(name)?;
        std::env::set_var(Instance::ENV_VAR, name);
        return Ok(());
    }
    // 2. An already-set GHOSTLIGHT_INSTANCE (from a test, the e2e harness, or an inherited env):
    // validate it strictly and keep it.
    if std::env::var_os(Instance::ENV_VAR).is_some() {
        Instance::validate_env()?;
        return Ok(());
    }
    // 3. The argv[0] basename: a `ghostlight-<n>` binary (the installer's per-instance copy that
    // Chrome launches as the native host) selects instance `<n>`. Bare `ghostlight` is the default.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(inst) = Instance::from_exe_stem(&exe) {
            if let Some(name) = inst.name() {
                std::env::set_var(Instance::ENV_VAR, name);
            }
        }
    }
    Ok(())
}

/// Scan argv for `--instance <value>` or `--instance=<value>`, returning the value if present.
/// Done before clap (like the `--debug` scan) so the native-host role -- which never reaches clap
/// -- and every other role resolve the instance identically.
fn instance_flag_value() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if let Some(v) = a.strip_prefix("--instance=") {
            return Some(v.to_string());
        }
        if a == "--instance" {
            return args.next();
        }
    }
    None
}

/// `ghostlight status`: read and print the running server's live inner state.
fn run_status(args: StatusArgs) {
    if args.json {
        match ghostlight::observability::raw_state() {
            Some(s) => print!("{s}"),
            None => println!("no debug state found (start the server with --debug)"),
        }
    } else {
        println!("{}", ghostlight::observability::status_report());
    }
}
