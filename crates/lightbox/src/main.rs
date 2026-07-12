// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Ghostlight Lightbox (ADR-0056): a dev-only integration harness. `lightbox list` shows the
//! scenarios; `lightbox run <name>` / `lightbox run --all` execute them with CI-friendly exit codes.
//! Each scenario is a second composition root over the real ghostlight-core library, wired to temp
//! dirs and a real localhost endpoint -- never a fixed admin location, never the deployed service.

mod fake_browser;
mod scenarios;
mod support;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lightbox",
    about = "Ghostlight Lightbox: dev-only integration harness (ADR-0056). Not a shipped artifact."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the available scenarios.
    List,
    /// Run one scenario by name, or every scenario with --all.
    Run {
        /// The scenario name (see `lightbox list`); omit and pass --all to run every scenario.
        name: Option<String>,
        /// Run every scenario.
        #[arg(long)]
        all: bool,
    },
    /// ADR-0059: an interactive, offline stand-in for the real browser-role relay + Chrome
    /// extension. Dials a REAL running service exactly as the real relay does; lets you drive
    /// wire-protocol behavior (routing, tabId encoding, focus, notifications) without Chrome.
    FakeBrowser {
        /// Which instance's endpoints to try (default: unpinned, `[dev, default]`).
        #[arg(long)]
        instance: Option<String>,
        /// The fake browser identity this session's hello presents.
        #[arg(long, default_value_t = 4_242_424)]
        pid: u32,
        /// Answer every tool_request/tab_url_request with a canned result automatically.
        #[arg(long)]
        auto_reply: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let registry = scenarios::registry();

    match cli.command {
        Command::List => {
            for (name, _) in &registry {
                println!("{name}");
            }
            ExitCode::SUCCESS
        }
        Command::Run { name, all } => {
            let to_run: Vec<scenarios::Scenario> = if all {
                registry
            } else if let Some(requested) = name {
                match registry.iter().find(|(k, _)| *k == requested) {
                    Some(&entry) => vec![entry],
                    None => {
                        eprintln!("unknown scenario: {requested}\ntry `lightbox list`");
                        return ExitCode::from(2);
                    }
                }
            } else {
                eprintln!("specify a scenario name or --all (see `lightbox list`)");
                return ExitCode::from(2);
            };

            let mut failed = 0usize;
            for (name, run) in &to_run {
                match run() {
                    Ok(()) => println!("ok    {name}"),
                    Err(e) => {
                        println!("FAIL  {name}: {e:#}");
                        failed += 1;
                    }
                }
            }
            if failed == 0 {
                println!("\n{} scenario(s) passed", to_run.len());
                ExitCode::SUCCESS
            } else {
                println!("\n{failed} of {} scenario(s) failed", to_run.len());
                ExitCode::FAILURE
            }
        }
        Command::FakeBrowser {
            instance,
            pid,
            auto_reply,
        } => {
            match fake_browser::run(fake_browser::FakeBrowserOptions {
                instance,
                pid,
                auto_reply,
            }) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("fake-browser: {e:#}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
