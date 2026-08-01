// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Per-user OS autostart registration for the always-ready Ghostlight service (ADR-0030 Decision 8
//! amendment, H9; Windows mechanism replaced by ADR-0054). Registers `ghostlight service` to start
//! at login, then starts it once so the first session is already up; unregisters + stops it on
//! uninstall. Uses the SAME per-platform identifiers the adapter self-heal targets
//! (`ghostlight_transport::supervisor`), so there is one source of truth for the names both sides
//! address. Every mechanism is per-user and genuinely zero-admin -- NEVER elevated (Decision 8):
//! an HKCU Run key + detached start on Windows (a schtasks logon task needs elevation, issue #17),
//! a user launchd LaunchAgent on macOS, a systemd --user unit on Linux. Registration also activates
//! the selected installed engine immediately (ADR-0092). Applying these steps is best-effort: a
//! failure here is logged and never aborts the surrounding install/uninstall (the adapter self-heal
//! and manual `ghostlight service` remain fallbacks).

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
    /// Make `<exe> service` the active installed engine. A current installed predecessor is
    /// replaced under deploy locks; an external repository/dev engine is left in place.
    #[cfg(windows)]
    ActivateInstalled {
        exe: PathBuf,
        install_root: PathBuf,
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
pub fn register_steps(exe: &Path, ctx: &PlanCtx) -> Vec<SupervisorStep> {
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
        SupervisorStep::ActivateInstalled {
            exe,
            install_root: ctx.home.join(".ghostlight").join("bin"),
        },
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

/// PINNED: write the unit, then `systemctl --user daemon-reload`, enable it, and restart it so an
/// already-running predecessor cannot keep serving after an upgrade.
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
        SupervisorStep::Run(SupervisorCommand::new(
            "systemctl",
            vec!["--user".into(), "restart".into(), supervisor_unit()],
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

// --- Windows installed-engine activation (ADR-0092) ---

/// Upgrade activation is its own small lifecycle domain: it owns the endpoint-owner decision,
/// deploy-lock scope, exact process replacement, and bounded verification. Keeping those states in
/// one module prevents installer output or supervisor planning from becoming process-policy code.
#[cfg(windows)]
mod activation {
    use super::native_host;
    use ghostlight_transport::ipc::{self, EndpointProbe};
    use ghostlight_transport::proc::{self, ProcId};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    const SERVICE_EXE_NAME: &str = "ghostlight.exe";
    const RELAY_EXE_NAME: &str = "ghostlight-relay.exe";
    const OWNER_RETRIES: usize = 10;
    const OWNER_RETRY_INTERVAL: Duration = Duration::from_millis(50);
    const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(3);
    const ACTIVATION_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const LOCK_CONTENTS: &[u8] = b"ghostlight install\n";

    /// The externally meaningful end state of one activation attempt.
    pub(super) enum Outcome {
        Started,
        Replaced { previous: PathBuf },
        AlreadyCurrent,
        PreservedExternal { active: PathBuf },
    }

    #[derive(Debug, PartialEq, Eq)]
    enum OwnerKind {
        Current,
        InstalledPredecessor,
        External,
    }

    /// Locks created by this activation attempt. Drop removes only files this process created with
    /// `create_new`; a pre-existing deploy lock aborts activation and is never adopted or removed.
    struct DeployLocks {
        paths: Vec<PathBuf>,
    }

    impl DeployLocks {
        fn acquire(install_root: &Path) -> std::result::Result<Self, String> {
            let mut locks = Self { paths: Vec::new() };
            for directory in installed_engine_directories(install_root)? {
                let path = directory.join(ghostlight_transport::supervisor::DEPLOY_LOCK_NAME);
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| {
                        format!(
                            "cannot acquire deploy lock {}: {e}; another deploy may be active",
                            path.display()
                        )
                    })?;
                locks.paths.push(path.clone());
                file.write_all(LOCK_CONTENTS)
                    .map_err(|e| format!("cannot write deploy lock {}: {e}", path.display()))?;
                file.sync_all()
                    .map_err(|e| format!("cannot flush deploy lock {}: {e}", path.display()))?;
            }
            Ok(locks)
        }
    }

    impl Drop for DeployLocks {
        fn drop(&mut self) {
            for path in &self.paths {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Activate `exe` without displacing an explicitly running repository/dev engine. Only the
    /// exact owner of the resolved adapter endpoint may be replaced, and only after its executable
    /// path proves that it belongs to `install_root`.
    pub(super) fn activate(
        exe: &Path,
        install_root: &Path,
    ) -> std::result::Result<Outcome, String> {
        let exe = native_host::normalize_exe_path(exe);
        let install_root = native_host::normalize_exe_path(install_root);
        let _locks = DeployLocks::acquire(&install_root)?;
        let control_endpoint = ipc::adapter_endpoint_name(&ipc::default_endpoint());

        match active_owner(&control_endpoint)? {
            None => {
                start_and_verify(&exe, &control_endpoint)?;
                Ok(Outcome::Started)
            }
            Some((owner, active)) => match classify_owner(&active, &exe, &install_root) {
                OwnerKind::Current => Ok(Outcome::AlreadyCurrent),
                OwnerKind::External => Ok(Outcome::PreservedExternal { active }),
                OwnerKind::InstalledPredecessor => {
                    let confirmed = active_owner(&control_endpoint)?
                        .filter(|(current, path)| current == &owner && same_path(path, &active))
                        .ok_or_else(|| {
                            "the endpoint owner changed while upgrade activation was being prepared"
                                .to_string()
                        })?;
                    debug_assert_eq!(confirmed.0, owner);
                    if !proc::is_alive(owner) {
                        return Err(format!(
                            "installed service pid {} exited before it could be replaced",
                            owner.pid
                        ));
                    }
                    if !proc::terminate(owner.pid) {
                        return Err(format!(
                            "could not stop installed service pid {} ({})",
                            owner.pid,
                            active.display()
                        ));
                    }
                    wait_for_exit(owner)?;
                    start_and_verify(&exe, &control_endpoint)?;
                    Ok(Outcome::Replaced { previous: active })
                }
            },
        }
    }

    fn active_owner(endpoint: &str) -> std::result::Result<Option<(ProcId, PathBuf)>, String> {
        for attempt in 0..OWNER_RETRIES {
            if let Some(pid) = ipc::named_pipe_server_process_id(endpoint) {
                let path = proc::executable_path(pid).ok_or_else(|| {
                    format!("cannot verify the executable for endpoint owner pid {pid}")
                })?;
                return Ok(Some((ProcId::of(pid), path)));
            }
            if matches!(ipc::probe_endpoint(endpoint), EndpointProbe::Absent) {
                return Ok(None);
            }
            if attempt + 1 < OWNER_RETRIES {
                std::thread::sleep(OWNER_RETRY_INTERVAL);
            }
        }
        Err(format!(
            "cannot identify the process serving endpoint {}",
            ipc::endpoint_display(endpoint)
        ))
    }

    fn start_and_verify(exe: &Path, endpoint: &str) -> std::result::Result<(), String> {
        ghostlight_transport::supervisor::spawn_service_detached(exe)
            .map_err(|e| format!("cannot start \"{}\" service: {e}", exe.display()))?;
        let deadline = Instant::now() + ACTIVATION_TIMEOUT;
        loop {
            match active_owner(endpoint) {
                Ok(Some((_owner, active))) if same_path(&active, exe) => return Ok(()),
                Ok(Some((_owner, active))) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "{} still owns the endpoint after starting {}",
                            active.display(),
                            exe.display()
                        ));
                    }
                }
                Ok(None) | Err(_) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "{} did not claim the endpoint within {} seconds",
                            exe.display(),
                            ACTIVATION_TIMEOUT.as_secs()
                        ));
                    }
                }
            }
            std::thread::sleep(ACTIVATION_POLL_INTERVAL);
        }
    }

    fn wait_for_exit(owner: ProcId) -> std::result::Result<(), String> {
        let deadline = Instant::now() + ACTIVATION_TIMEOUT;
        while proc::is_alive(owner) {
            if Instant::now() >= deadline {
                return Err(format!(
                    "installed service pid {} did not stop within {} seconds",
                    owner.pid,
                    ACTIVATION_TIMEOUT.as_secs()
                ));
            }
            std::thread::sleep(ACTIVATION_POLL_INTERVAL);
        }
        Ok(())
    }

    fn installed_engine_directories(
        install_root: &Path,
    ) -> std::result::Result<Vec<PathBuf>, String> {
        if !install_root.exists() {
            return Ok(Vec::new());
        }
        let mut directories = Vec::new();
        if contains_engine(install_root) {
            directories.push(install_root.to_path_buf());
        }
        let entries = std::fs::read_dir(install_root).map_err(|e| {
            format!(
                "cannot inspect installed engine directory {}: {e}",
                install_root.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                format!(
                    "cannot inspect an entry under {}: {e}",
                    install_root.display()
                )
            })?;
            let path = entry.path();
            if path.is_dir() && contains_engine(&path) {
                directories.push(path);
            }
        }
        directories.sort();
        directories.dedup();
        Ok(directories)
    }

    fn contains_engine(directory: &Path) -> bool {
        directory.join(SERVICE_EXE_NAME).is_file() || directory.join(RELAY_EXE_NAME).is_file()
    }

    fn path_key(path: &Path) -> String {
        native_host::normalize_exe_path(path)
            .to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    }

    fn same_path(left: &Path, right: &Path) -> bool {
        path_key(left) == path_key(right)
    }

    fn classify_owner(active: &Path, current: &Path, install_root: &Path) -> OwnerKind {
        if same_path(active, current) {
            OwnerKind::Current
        } else if inside_root(active, install_root) {
            OwnerKind::InstalledPredecessor
        } else {
            OwnerKind::External
        }
    }

    fn inside_root(path: &Path, root: &Path) -> bool {
        let path = path_key(path);
        let mut root = path_key(root);
        root.push('\\');
        path.starts_with(&root)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn path_checks_distinguish_installed_current_and_external_engines() {
            let root = Path::new(r"C:\Users\u\.ghostlight\bin");
            let old = Path::new(r"C:\Users\u\.ghostlight\bin\v0.7.0\ghostlight.exe");
            let current = Path::new(r"c:/users/u/.ghostlight/bin/v0.7.1/ghostlight.exe");
            let current_same = Path::new(r"C:\USERS\U\.GHOSTLIGHT\BIN\V0.7.1\GHOSTLIGHT.EXE");
            let external = Path::new(r"F:\repo\browser-mcp\target\release\ghostlight.exe");
            let prefix_collision = Path::new(r"C:\Users\u\.ghostlight\binary\ghostlight.exe");

            assert!(inside_root(old, root));
            assert!(inside_root(current, root));
            assert!(same_path(current, current_same));
            assert!(!inside_root(external, root));
            assert!(!inside_root(prefix_collision, root));
            assert_eq!(
                classify_owner(current_same, current, root),
                OwnerKind::Current
            );
            assert_eq!(
                classify_owner(old, current, root),
                OwnerKind::InstalledPredecessor
            );
            assert_eq!(classify_owner(external, current, root), OwnerKind::External);
        }

        #[test]
        fn deploy_locks_cover_every_installed_engine_and_preserve_foreign_locks() {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock is after the Unix epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "ghostlight-activation-locks-{}-{unique}",
                std::process::id()
            ));
            let old = root.join("v0.7.0");
            let current = root.join("v0.7.1");
            std::fs::create_dir_all(&old).expect("create old engine directory");
            std::fs::create_dir_all(&current).expect("create current engine directory");
            std::fs::write(old.join(SERVICE_EXE_NAME), b"old").expect("write old engine marker");
            std::fs::write(current.join(RELAY_EXE_NAME), b"current")
                .expect("write current relay marker");
            let old_lock = old.join(ghostlight_transport::supervisor::DEPLOY_LOCK_NAME);
            let current_lock = current.join(ghostlight_transport::supervisor::DEPLOY_LOCK_NAME);

            let locks = DeployLocks::acquire(&root).expect("acquire every engine lock");
            assert!(old_lock.is_file());
            assert!(current_lock.is_file());
            drop(locks);
            assert!(!old_lock.exists());
            assert!(!current_lock.exists());

            std::fs::write(&old_lock, b"someone else\n").expect("write foreign lock");
            let error = DeployLocks::acquire(&root)
                .err()
                .expect("a foreign lock refuses activation");
            assert!(error.contains("another deploy may be active"));
            assert_eq!(
                std::fs::read(&old_lock).expect("foreign lock remains readable"),
                b"someone else\n"
            );

            std::fs::remove_dir_all(&root).expect("remove owned activation test directory");
        }
    }
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
            SupervisorStep::ActivateInstalled { exe, install_root } => {
                if dry_run {
                    println!(
                        "  [plan] {label:<28} activate installed service: \"{}\"",
                        exe.display(),
                    );
                    continue;
                }
                match activation::activate(exe, install_root) {
                    Ok(activation::Outcome::Started) => println!(
                        "  [ok]   {label:<28} activated: \"{}\" service",
                        exe.display()
                    ),
                    Ok(activation::Outcome::Replaced { previous }) => println!(
                        "  [ok]   {label:<28} replaced {} with \"{}\" service",
                        previous.display(),
                        exe.display()
                    ),
                    Ok(activation::Outcome::AlreadyCurrent) => println!(
                        "  [noop] {label:<28} \"{}\" already owns the endpoint",
                        exe.display()
                    ),
                    Ok(activation::Outcome::PreservedExternal { active }) => println!(
                        "  [noop] {label:<28} preserved external engine {} (the registered service was not forced over it)",
                        active.display()
                    ),
                    Err(e) => println!(
                        "  [warn] {label:<28} could not activate the installed service: {e} (best-effort; ignored -- start it manually with 'ghostlight service')"
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
        // value, and installed-engine activation.
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
                .any(|s| matches!(s, SupervisorStep::ActivateInstalled { .. })),
            "the selected installed service is activated right after install"
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
