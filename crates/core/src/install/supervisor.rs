// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-user OS autostart registration for the always-ready Ghostlight service (ADR-0030 Decision 8
//! amendment, H9; Windows mechanism replaced by ADR-0054). Registers `ghostlight service` to start
//! at login, then starts it once so the first session is already up; unregisters + stops it on
//! uninstall. Uses the SAME per-platform identifiers the adapter self-heal targets
//! (`ghostlight_transport::supervisor`), so there is one source of truth for the names both sides
//! address. Every mechanism is per-user and genuinely zero-admin -- NEVER elevated (Decision 8):
//! an HKCU Run key + detached start on Windows (a schtasks logon task needs elevation, issue #17),
//! a user launchd LaunchAgent on macOS, a systemd --user unit on Linux. Applying these steps is
//! best-effort: a failure here is logged and never aborts the surrounding install/uninstall (the
//! adapter self-heal and manual `ghostlight service` remain fallbacks).

use super::{native_host, PlanCtx};
#[cfg(target_os = "macos")]
use ghostlight_transport::supervisor::supervisor_label;
#[cfg(windows)]
use ghostlight_transport::supervisor::supervisor_task_name;
#[cfg(all(unix, not(target_os = "macos")))]
use ghostlight_transport::supervisor::supervisor_unit;
use std::path::{Path, PathBuf};

/// One external command to run best-effort (never fatal to the caller). `quiet_failure` steps
/// report `[noop]` instead of `[warn]` on a non-zero exit -- for cleanup of things that usually do
/// not exist (the ADR-0054 legacy scheduled task).
pub struct SupervisorCommand {
    pub program: String,
    pub args: Vec<String>,
    pub quiet_failure: bool,
}

impl SupervisorCommand {
    fn new(program: &str, args: Vec<String>) -> Self {
        Self {
            program: program.to_string(),
            args,
            quiet_failure: false,
        }
    }

    #[cfg(windows)]
    fn quiet(program: &str, args: Vec<String>) -> Self {
        Self {
            quiet_failure: true,
            ..Self::new(program, args)
        }
    }
}

/// One step of registering/unregistering the supervisor: write its definition file, remove it,
/// run an external command, or (Windows, ADR-0054) touch the HKCU Run key / start the service
/// detached. Applied in order, each best-effort.
pub enum SupervisorStep {
    WriteFile {
        path: PathBuf,
        contents: String,
    },
    RemoveFile {
        path: PathBuf,
    },
    Run(SupervisorCommand),
    /// Set `HKCU\...\Run\<name>` = `<data>` (ADR-0054 Decision 1).
    #[cfg(windows)]
    SetRunValue {
        name: String,
        data: String,
    },
    /// Delete `HKCU\...\Run\<name>` (absent is a noop).
    #[cfg(windows)]
    RemoveRunValue {
        name: String,
    },
    /// Spawn `<exe> service` fully detached so the service is up immediately after install
    /// (ADR-0054 Decision 2; the same helper the adapter self-heal uses).
    #[cfg(windows)]
    StartDetached {
        exe: PathBuf,
    },
}

// --- Windows: HKCU Run key + detached start (ADR-0054; supersedes the schtasks logon task) ---

/// The Run-key DATA for this install: `"<exe>" service`, with `--instance <n>` for a named
/// instance. Pure, so the quoting is unit-testable.
#[cfg(windows)]
pub fn run_value_data(exe: &Path) -> String {
    match ghostlight_transport::instance::Instance::resolve().name() {
        Some(n) => format!("\"{}\" --instance {n} service", exe.display()),
        None => format!("\"{}\" service", exe.display()),
    }
}

/// PINNED (ADR-0054): best-effort delete the legacy <=0.5.0 scheduled task, write the HKCU Run
/// value (name = [`supervisor_task_name`], the unchanged identity), then start the service once,
/// detached. The Run key is the one Windows logon-start mechanism a non-admin user can always
/// write -- `schtasks /sc onlogon` requires elevation (issue #17).
#[cfg(windows)]
pub fn register_steps(exe: &Path, _ctx: &PlanCtx) -> Vec<SupervisorStep> {
    let exe = native_host::normalize_exe_path(exe);
    vec![
        // Legacy migration (ADR-0054 D3): an elevated install from <=0.5.0 may hold the old task;
        // quiet because on almost every machine there is nothing to delete.
        SupervisorStep::Run(SupervisorCommand::quiet(
            "schtasks",
            vec![
                "/delete".into(),
                "/tn".into(),
                supervisor_task_name(),
                "/f".into(),
            ],
        )),
        SupervisorStep::SetRunValue {
            name: supervisor_task_name(),
            data: run_value_data(&exe),
        },
        SupervisorStep::StartDetached { exe },
    ]
}

/// PINNED (ADR-0054): delete the Run value; best-effort delete the legacy task too.
#[cfg(windows)]
pub fn unregister_steps(_ctx: &PlanCtx) -> Vec<SupervisorStep> {
    vec![
        SupervisorStep::RemoveRunValue {
            name: supervisor_task_name(),
        },
        SupervisorStep::Run(SupervisorCommand::quiet(
            "schtasks",
            vec![
                "/delete".into(),
                "/tn".into(),
                supervisor_task_name(),
                "/f".into(),
            ],
        )),
    ]
}

// --- macOS: launchd LaunchAgent (per-user gui/<uid> domain) ---

/// `~/Library/LaunchAgents/org.sylin.ghostlight.service.plist` (PINNED path).
#[cfg(target_os = "macos")]
pub fn plist_path(ctx: &PlanCtx) -> PathBuf {
    ctx.home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", supervisor_label()))
}

#[cfg(target_os = "macos")]
fn render_plist(exe: &Path) -> String {
    let label = supervisor_label();
    // ProgramArguments: [<exe>, (--instance <n>)?, service] -- a non-default instance carries its
    // name so launchd starts the right stack.
    let mut prog_args = format!("<string>{}</string>", exe.display());
    if let Some(n) = ghostlight_transport::instance::Instance::resolve().name() {
        prog_args.push_str(&format!("<string>--instance</string><string>{n}</string>"));
    }
    prog_args.push_str("<string>service</string>");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\"><dict>\n  \
<key>Label</key><string>{label}</string>\n  \
<key>ProgramArguments</key><array>{prog_args}</array>\n  \
<key>RunAtLoad</key><true/>\n  \
<key>KeepAlive</key><true/>\n\
</dict></plist>\n"
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
                format!("gui/{uid}/{}", supervisor_label()),
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
            vec![
                "bootout".into(),
                format!("gui/{uid}/{}", supervisor_label()),
            ],
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
        .join(supervisor_unit())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn render_unit(exe: &Path) -> String {
    // A non-default instance carries `--instance <n>` so systemd starts the right stack.
    let instance_flag = match ghostlight_transport::instance::Instance::resolve().name() {
        Some(n) => format!(" --instance {n}"),
        None => String::new(),
    };
    format!(
        "[Unit]\n\
Description=Ghostlight Hub service\n\
[Service]\n\
ExecStart={}{instance_flag} service\n\
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
                supervisor_unit(),
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
                supervisor_unit(),
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
/// (`ghostlight_transport::supervisor::start_service`) and manual `ghostlight service` remain fallbacks.
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
                // Quiet steps suppress the command's own stderr too (schtasks prints
                // "ERROR: ..." for an absent task, which reads like a failure).
                let mut command = std::process::Command::new(&cmd.program);
                command.args(&cmd.args);
                if cmd.quiet_failure {
                    command
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null());
                }
                match command.status() {
                    Ok(status) if status.success() => {
                        println!(
                            "  [ok]   {label:<28} {} {}",
                            cmd.program,
                            cmd.args.join(" ")
                        );
                    }
                    Ok(_) if cmd.quiet_failure => {
                        println!(
                            "  [noop] {label:<28} {} {} (nothing to do)",
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
            #[cfg(windows)]
            SupervisorStep::SetRunValue { name, data } => {
                let key_path = ghostlight_transport::supervisor::RUN_KEY_PATH;
                if dry_run {
                    println!("  [plan] {label:<28} HKCU\\{key_path} \"{name}\" = {data}");
                    continue;
                }
                let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
                match hkcu
                    .create_subkey(key_path)
                    .and_then(|(key, _)| key.set_value(name, data))
                {
                    Ok(()) => println!("  [ok]   {label:<28} HKCU\\{key_path} \"{name}\" = {data}"),
                    Err(e) => println!(
                        "  [warn] {label:<28} could not write HKCU\\{key_path} \"{name}\": {e} (best-effort; ignored)"
                    ),
                }
            }
            #[cfg(windows)]
            SupervisorStep::RemoveRunValue { name } => {
                let key_path = ghostlight_transport::supervisor::RUN_KEY_PATH;
                if dry_run {
                    println!("  [plan] {label:<28} remove HKCU\\{key_path} \"{name}\"");
                    continue;
                }
                let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
                match hkcu
                    .open_subkey_with_flags(key_path, winreg::enums::KEY_SET_VALUE)
                    .and_then(|key| key.delete_value(name))
                {
                    Ok(()) => println!("  [ok]   {label:<28} removed HKCU\\{key_path} \"{name}\""),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        println!("  [noop] {label:<28} HKCU\\{key_path} \"{name}\" (absent)");
                    }
                    Err(e) => println!(
                        "  [warn] {label:<28} could not remove HKCU\\{key_path} \"{name}\": {e} (best-effort; ignored)"
                    ),
                }
            }
            #[cfg(windows)]
            SupervisorStep::StartDetached { exe } => {
                if dry_run {
                    println!(
                        "  [plan] {label:<28} start detached: \"{}\" service",
                        exe.display()
                    );
                    continue;
                }
                match ghostlight_transport::supervisor::spawn_service_detached(exe) {
                    Ok(()) => println!(
                        "  [ok]   {label:<28} started detached: \"{}\" service",
                        exe.display()
                    ),
                    Err(e) => println!(
                        "  [warn] {label:<28} could not start the service: {e} (best-effort; ignored -- start it manually with 'ghostlight service')"
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
    fn windows_register_steps_are_zero_elevation() {
        // ADR-0054: the schtasks logon task is GONE from registration (creating one requires
        // elevation, issue #17); what remains is the legacy-cleanup delete (quiet), the HKCU Run
        // value, and the detached start.
        let ctx = test_ctx();
        let steps = register_steps(Path::new(r"C:\abs\ghostlight.exe"), &ctx);
        assert!(
            !steps.iter().any(|s| matches!(
                s,
                SupervisorStep::Run(c) if c.args.contains(&"/create".to_string())
            )),
            "no scheduled-task creation anywhere"
        );
        let run_value = steps
            .iter()
            .find_map(|s| match s {
                SupervisorStep::SetRunValue { name, data } => Some((name, data)),
                _ => None,
            })
            .expect("an HKCU Run value step exists");
        assert_eq!(run_value.0, &supervisor_task_name());
        assert_eq!(run_value.1, r#""C:\abs\ghostlight.exe" service"#);
        assert!(
            steps
                .iter()
                .any(|s| matches!(s, SupervisorStep::StartDetached { .. })),
            "the service starts once, detached, right after install"
        );
        let legacy = steps
            .iter()
            .find_map(|s| match s {
                SupervisorStep::Run(c) if c.args.contains(&"/delete".to_string()) => Some(c),
                _ => None,
            })
            .expect("legacy task cleanup exists");
        assert!(
            legacy.quiet_failure,
            "absent legacy task reports noop, not warn"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_unregister_removes_both_mechanisms() {
        let ctx = test_ctx();
        let steps = unregister_steps(&ctx);
        assert!(steps
            .iter()
            .any(|s| matches!(s, SupervisorStep::RemoveRunValue { name } if name == &supervisor_task_name())));
        assert!(steps.iter().any(|s| matches!(
            s,
            SupervisorStep::Run(c) if c.args.contains(&"/delete".to_string()) && c.quiet_failure
        )));
    }
}
