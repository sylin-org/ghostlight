// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-user OS supervisor registration for the always-ready Ghostlight service (ADR-0030 Decision 8
//! amendment, H9). Registers `ghostlight service` to start at login and restart on crash, then
//! starts it once so the first session is already up; unregisters + stops it on uninstall. Uses the
//! SAME per-platform identifiers H6's adapter self-heal targets (`crate::hub::supervisor`), so there
//! is one source of truth for the names an installed supervisor and a self-healing adapter both
//! address. Every mechanism is per-user, zero-admin (Task Scheduler LeastPrivilege logon task / user
//! launchd LaunchAgent / systemd --user unit) -- NEVER elevated (Decision 8). Applying these steps is
//! best-effort: a failure here is logged and never aborts the surrounding install/uninstall (the
//! adapter self-heal and manual `ghostlight service` remain fallbacks).

use super::{native_host, PlanCtx};
#[cfg(target_os = "macos")]
use crate::hub::supervisor::SUPERVISOR_LABEL;
#[cfg(windows)]
use crate::hub::supervisor::SUPERVISOR_TASK_NAME;
#[cfg(all(unix, not(target_os = "macos")))]
use crate::hub::supervisor::SUPERVISOR_UNIT;
use std::path::{Path, PathBuf};

/// One external command to run best-effort (never fatal to the caller).
pub struct SupervisorCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl SupervisorCommand {
    fn new(program: &str, args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            args,
        }
    }
}

/// One step of registering/unregistering the supervisor: write its definition file, remove it, or
/// run an external command. Applied in order, each best-effort.
pub enum SupervisorStep {
    WriteFile { path: PathBuf, contents: String },
    RemoveFile { path: PathBuf },
    Run(SupervisorCommand),
}

// --- Windows: Task Scheduler (LeastPrivilege logon task) ---

/// PINNED (docs/tasks/hub/H9-installer-autostart.md): `schtasks /create /tn "Ghostlight Service"
/// /tr "\"<exe>\" service" /sc onlogon /rl limited /f`, then `schtasks /run /tn "Ghostlight Service"`.
#[cfg(windows)]
pub fn register_steps(exe: &Path, _ctx: &PlanCtx) -> Vec<SupervisorStep> {
    let exe = native_host::normalize_exe_path(exe);
    let tr = format!("\"{}\" service", exe.display());
    vec![
        SupervisorStep::Run(SupervisorCommand::new(
            "schtasks",
            vec![
                "/create".into(),
                "/tn".into(),
                SUPERVISOR_TASK_NAME.into(),
                "/tr".into(),
                tr,
                "/sc".into(),
                "onlogon".into(),
                "/rl".into(),
                "limited".into(),
                "/f".into(),
            ],
        )),
        SupervisorStep::Run(SupervisorCommand::new(
            "schtasks",
            vec!["/run".into(), "/tn".into(), SUPERVISOR_TASK_NAME.into()],
        )),
    ]
}

/// PINNED: `schtasks /delete /tn "Ghostlight Service" /f`.
#[cfg(windows)]
pub fn unregister_steps(_ctx: &PlanCtx) -> Vec<SupervisorStep> {
    vec![SupervisorStep::Run(SupervisorCommand::new(
        "schtasks",
        vec![
            "/delete".into(),
            "/tn".into(),
            SUPERVISOR_TASK_NAME.into(),
            "/f".into(),
        ],
    ))]
}

// --- macOS: launchd LaunchAgent (per-user gui/<uid> domain) ---

/// `~/Library/LaunchAgents/org.sylin.ghostlight.service.plist` (PINNED path).
#[cfg(target_os = "macos")]
pub fn plist_path(ctx: &PlanCtx) -> PathBuf {
    ctx.home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{SUPERVISOR_LABEL}.plist"))
}

#[cfg(target_os = "macos")]
fn render_plist(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n  \
<key>Label</key><string>{SUPERVISOR_LABEL}</string>\n  \
<key>ProgramArguments</key><array><string>{}</string><string>service</string></array>\n  \
<key>RunAtLoad</key><true/>\n  \
<key>KeepAlive</key><true/>\n\
</dict></plist>\n",
        exe.display()
    )
}

/// PINNED: write the plist, then `launchctl bootstrap gui/<uid> <plist-path>`, then
/// `launchctl kickstart -k gui/<uid>/org.sylin.ghostlight.service`.
#[cfg(target_os = "macos")]
pub fn register_steps(exe: &Path, ctx: &PlanCtx) -> Vec<SupervisorStep> {
    let exe = native_host::normalize_exe_path(exe);
    let path = plist_path(ctx);
    let uid = unsafe { libc::getuid() };
    vec![
        SupervisorStep::WriteFile {
            path: path.clone(),
            contents: render_plist(&exe),
        },
        SupervisorStep::Run(SupervisorCommand::new(
            "launchctl",
            vec![
                "bootstrap".into(),
                format!("gui/{uid}"),
                path.to_string_lossy().into_owned(),
            ],
        )),
        SupervisorStep::Run(SupervisorCommand::new(
            "launchctl",
            vec![
                "kickstart".into(),
                "-k".into(),
                format!("gui/{uid}/{SUPERVISOR_LABEL}"),
            ],
        )),
    ]
}

/// PINNED: `launchctl bootout gui/<uid>/org.sylin.ghostlight.service`, then remove the plist.
#[cfg(target_os = "macos")]
pub fn unregister_steps(ctx: &PlanCtx) -> Vec<SupervisorStep> {
    let uid = unsafe { libc::getuid() };
    vec![
        SupervisorStep::Run(SupervisorCommand::new(
            "launchctl",
            vec!["bootout".into(), format!("gui/{uid}/{SUPERVISOR_LABEL}")],
        )),
        SupervisorStep::RemoveFile {
            path: plist_path(ctx),
        },
    ]
}

// --- Linux (and other non-macOS Unix): systemd --user ---

/// `~/.config/systemd/user/ghostlight.service` (PINNED path; `ctx.config` is the per-OS config base,
/// `~/.config` on Linux).
#[cfg(all(unix, not(target_os = "macos")))]
pub fn unit_path(ctx: &PlanCtx) -> PathBuf {
    ctx.config
        .join("systemd")
        .join("user")
        .join(SUPERVISOR_UNIT)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn render_unit(exe: &Path) -> String {
    format!(
        "[Unit]\n\
Description=Ghostlight Hub service\n\
[Service]\n\
ExecStart={} service\n\
Restart=on-failure\n\
[Install]\n\
WantedBy=default.target\n",
        exe.display()
    )
}

/// PINNED: write the unit, then `systemctl --user daemon-reload`, then
/// `systemctl --user enable --now ghostlight.service`.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn register_steps(exe: &Path, ctx: &PlanCtx) -> Vec<SupervisorStep> {
    let exe = native_host::normalize_exe_path(exe);
    vec![
        SupervisorStep::WriteFile {
            path: unit_path(ctx),
            contents: render_unit(&exe),
        },
        SupervisorStep::Run(SupervisorCommand::new(
            "systemctl",
            vec!["--user".into(), "daemon-reload".into()],
        )),
        SupervisorStep::Run(SupervisorCommand::new(
            "systemctl",
            vec![
                "--user".into(),
                "enable".into(),
                "--now".into(),
                SUPERVISOR_UNIT.into(),
            ],
        )),
    ]
}

/// PINNED: `systemctl --user disable --now ghostlight.service`, then remove the unit file.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn unregister_steps(ctx: &PlanCtx) -> Vec<SupervisorStep> {
    vec![
        SupervisorStep::Run(SupervisorCommand::new(
            "systemctl",
            vec![
                "--user".into(),
                "disable".into(),
                "--now".into(),
                SUPERVISOR_UNIT.into(),
            ],
        )),
        SupervisorStep::RemoveFile {
            path: unit_path(ctx),
        },
    ]
}

// --- Apply (best-effort; never returns an error) ---

/// Apply supervisor steps best-effort, printing progress in the same `[ok]`/`[warn]`/`[plan]`/
/// `[noop]` style the rest of the installer uses. Never aborts and never returns an error: a failed
/// step here is a WARNING (Required behavior item 4) -- the adapter self-heal
/// (`crate::hub::supervisor::start_service`) and manual `ghostlight service` remain fallbacks.
pub fn apply_steps(label: &str, steps: &[SupervisorStep], dry_run: bool) {
    for step in steps {
        match step {
            SupervisorStep::WriteFile { path, contents } => {
                if dry_run {
                    println!("  [plan] {label:<28} write {}", path.display());
                    continue;
                }
                match native_host::write_file_atomic(path, contents) {
                    Ok(()) => println!("  [ok]   {label:<28} wrote {}", path.display()),
                    Err(e) => println!(
                        "  [warn] {label:<28} could not write {}: {e}",
                        path.display()
                    ),
                }
            }
            SupervisorStep::RemoveFile { path } => {
                if dry_run {
                    println!("  [plan] {label:<28} remove {}", path.display());
                    continue;
                }
                match std::fs::remove_file(path) {
                    Ok(()) => println!("  [ok]   {label:<28} removed {}", path.display()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        println!("  [noop] {label:<28} {} (absent)", path.display());
                    }
                    Err(e) => println!(
                        "  [warn] {label:<28} could not remove {}: {e}",
                        path.display()
                    ),
                }
            }
            SupervisorStep::Run(cmd) => {
                if dry_run {
                    println!(
                        "  [plan] {label:<28} {} {}",
                        cmd.program,
                        cmd.args.join(" ")
                    );
                    continue;
                }
                match std::process::Command::new(&cmd.program)
                    .args(&cmd.args)
                    .status()
                {
                    Ok(status) if status.success() => {
                        println!(
                            "  [ok]   {label:<28} {} {}",
                            cmd.program,
                            cmd.args.join(" ")
                        );
                    }
                    Ok(status) => println!(
                        "  [warn] {label:<28} {} {} exited {status} (best-effort; ignored -- start it manually with 'ghostlight service')",
                        cmd.program,
                        cmd.args.join(" ")
                    ),
                    Err(e) => println!(
                        "  [warn] {label:<28} could not run {}: {e} (best-effort; ignored)",
                        cmd.program
                    ),
                }
            }
        }
    }
}

// Windows-only until macOS/Linux step tests exist: gating the whole module (not just the
// helper) keeps the non-Windows `-D warnings` gate green (`use super::*` in an otherwise
// empty module trips unused-imports there).
#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn test_ctx() -> PlanCtx {
        PlanCtx {
            current_exe: PathBuf::from("/abs/ghostlight"),
            home: PathBuf::from("/home/u"),
            config: PathBuf::from("/home/u/.config"),
            local: PathBuf::from("/home/u/.local/share"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_register_steps_never_elevate() {
        let ctx = test_ctx();
        let steps = register_steps(Path::new(r"C:\abs\ghostlight.exe"), &ctx);
        let create = steps
            .iter()
            .find_map(|s| match s {
                SupervisorStep::Run(c) if c.args.contains(&"/create".to_string()) => Some(c),
                _ => None,
            })
            .expect("a schtasks /create step exists");
        assert!(create.args.contains(&"limited".to_string()));
        assert!(!create.args.iter().any(|a| a.eq_ignore_ascii_case("/ru")));
    }
}
