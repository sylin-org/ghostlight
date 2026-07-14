// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial
//! Harness support (ADR-0056): a self-cleaning temp root, org-signing helpers, and a localhost
//! bundle server so scenarios exercise the REAL managed:// code (including the ureq/rustls fetch)
//! without touching a fixed admin location or the network.

use std::io::{BufRead as _, Read as _, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ghostlight_core::governance::crypto::admin as crypto_admin;
use ghostlight_core::governance::manifest::bundle;

static UNIQUE: AtomicU64 = AtomicU64::new(0);
static REUSE_PROCESS_BUILD: OnceLock<bool> = OnceLock::new();
static PROCESS_BINARIES: OnceLock<Result<ProcessBinaries, String>> = OnceLock::new();

struct ProcessBinaries {
    service: PathBuf,
    relay: PathBuf,
}

/// Configure whether process-boundary scenarios reuse the caller's Cargo target. Must be called
/// once, before any scenario resolves its binaries (ADR-0056 Decision 3).
pub fn configure_process_build(reuse_cache: bool) -> anyhow::Result<()> {
    REUSE_PROCESS_BUILD
        .set(reuse_cache)
        .map_err(|_| anyhow::anyhow!("process build configuration was already set"))
}

/// A child process that is always killed and reaped when its scenario scope ends.
pub struct ChildGuard(Child);

impl Deref for ChildGuard {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Mint a process-isolated endpoint name for one legacy scenario.
pub fn unique_endpoint(tag: &str) -> String {
    let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
    format!("lightbox-{tag}-{}-{n}", std::process::id())
}

/// Spawn the production service binary against a scenario-owned log directory and wait for its
/// first debug snapshot. The binary comes from Lightbox's isolated build target by default.
pub fn spawn_service(endpoint: &str, log_dir: &Path) -> anyhow::Result<ChildGuard> {
    spawn_service_inner(endpoint, log_dir, None, false).map(|(child, _)| child)
}

/// Spawn the production service with an ephemeral Console listener and return its bound port.
pub fn spawn_service_with_webapi(
    endpoint: &str,
    log_dir: &Path,
    user_config_dir: Option<&Path>,
) -> anyhow::Result<(ChildGuard, u16)> {
    let (child, port) = spawn_service_inner(endpoint, log_dir, user_config_dir, true)?;
    Ok((
        child,
        port.ok_or_else(|| anyhow::anyhow!("service did not publish a Console port"))?,
    ))
}

/// Spawn the production agent-role relay against a running scenario service.
pub fn spawn_adapter(endpoint: &str, log_dir: &Path) -> anyhow::Result<ChildGuard> {
    let mut command = relay_command()?;
    let child = command
        .arg("--role")
        .arg("agent")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_LOG_DIR", log_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildGuard(child))
}

/// Start a command under the child guard used by process-boundary scenarios.
pub fn spawn_guard(command: &mut Command) -> anyhow::Result<ChildGuard> {
    Ok(ChildGuard(command.spawn()?))
}

/// Construct a command for the isolated production service binary.
pub fn service_command() -> anyhow::Result<Command> {
    Ok(Command::new(&process_binaries()?.service))
}

/// Construct a command for the isolated production relay binary.
pub fn relay_command() -> anyhow::Result<Command> {
    Ok(Command::new(&process_binaries()?.relay))
}

/// Wait for at least `count` parseable debug-state files in a scenario log directory.
pub fn wait_for_debug_states(log_dir: &Path, count: usize, within: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + within;
    loop {
        let found = std::fs::read_dir(log_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| {
                        let name = entry.file_name();
                        let name = name.to_string_lossy();
                        name.starts_with("debug-state-") && name.ends_with(".json")
                    })
                    .filter(|entry| {
                        std::fs::read_to_string(entry.path())
                            .ok()
                            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                            .is_some()
                    })
                    .count()
            })
            .unwrap_or(0);
        if found >= count {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "expected {count} debug states under {} within {within:?}",
            log_dir.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll the newest debug state for `role` until `predicate` accepts it.
pub fn wait_state_for_role(
    log_dir: &Path,
    role: &str,
    within: Duration,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> anyhow::Result<serde_json::Value> {
    let deadline = Instant::now() + within;
    loop {
        let mut newest = None;
        for entry in std::fs::read_dir(log_dir)?.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("debug-state-") || !name.ends_with(".json") {
                continue;
            }
            let modified = entry.metadata()?.modified()?;
            let value: serde_json::Value = match std::fs::read_to_string(entry.path())
                .ok()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            {
                Some(value) if value["role"] == role => value,
                _ => continue,
            };
            if newest
                .as_ref()
                .map(|(time, _): &(std::time::SystemTime, serde_json::Value)| modified > *time)
                .unwrap_or(true)
            {
                newest = Some((modified, value));
            }
        }
        if let Some((_, value)) = newest {
            if predicate(&value) {
                return Ok(value);
            }
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "no {role} debug state satisfied the predicate within {within:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Poll the service debug state until it reports an attached extension.
pub fn wait_extension_connected(log_dir: &Path, within: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + within;
    loop {
        if newest_debug_state(log_dir)
            .as_deref()
            .is_some_and(|raw| raw.contains("\"extension_connected\": true"))
        {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "extension did not connect within {within:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Send the browser-role hello and persistent extension identity used by fake-extension scenarios.
pub async fn send_extension_attach_frames<W>(writer: &mut W) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let hello = ghostlight_transport::handshake::browser_hello_bytes(
        std::process::id(),
        Some(ghostlight_transport::proc::ProcId {
            pid: std::process::id(),
            created: 0,
        }),
    );
    ghostlight_transport::host::write_message(writer, &hello).await?;
    let identity = serde_json::to_vec(&serde_json::json!({
        "type": ghostlight_transport::handshake::EXTENSION_IDENTITY_TYPE,
        ghostlight_transport::handshake::BROWSER_ID_FIELD: format!("lightbox-{}", std::process::id()),
    }))?;
    ghostlight_transport::host::write_message(writer, &identity).await?;
    Ok(())
}

/// Answer one tab URL probe with a synthetic live HTTPS page for the requested tab.
pub async fn answer_tab_url<W>(writer: &mut W, request: &serde_json::Value) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let reply = serde_json::json!({
        "id": request["id"],
        "type": "tab_url_response",
        "result": { "url": format!("https://tab-{}.example.com/", request["tabId"]) },
    });
    ghostlight_transport::host::write_message(writer, &serde_json::to_vec(&reply)?).await?;
    Ok(())
}

fn spawn_service_inner(
    endpoint: &str,
    log_dir: &Path,
    user_config_dir: Option<&Path>,
    webapi: bool,
) -> anyhow::Result<(ChildGuard, Option<u16>)> {
    let binaries = process_binaries()?;
    std::fs::create_dir_all(log_dir)?;
    let mut command = Command::new(&binaries.service);
    command
        .arg("service")
        .env("GHOSTLIGHT_ENDPOINT", endpoint)
        .env("GHOSTLIGHT_DEBUG", "1")
        .env("GHOSTLIGHT_LOG_DIR", log_dir)
        .env("GHOSTLIGHT_AUDIT_DIR", log_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if webapi {
        command.env("GHOSTLIGHT_WEBAPI_PORT", "0");
    }
    if let Some(path) = user_config_dir {
        command.env("GHOSTLIGHT_USER_CONFIG_DIR", path);
    }
    let child = ChildGuard(command.spawn()?);
    let port = if webapi {
        Some(wait_for_webapi_port(log_dir, Duration::from_secs(15))?)
    } else {
        wait_for_debug_state(log_dir, Duration::from_secs(15))?;
        None
    };
    Ok((child, port))
}

fn process_binaries() -> anyhow::Result<&'static ProcessBinaries> {
    PROCESS_BINARIES
        .get_or_init(|| build_or_reuse_process_binaries().map_err(|e| format!("{e:#}")))
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}

fn build_or_reuse_process_binaries() -> anyhow::Result<ProcessBinaries> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow::anyhow!("Lightbox manifest has no workspace root"))?;
    let reuse = *REUSE_PROCESS_BUILD.get().unwrap_or(&false);
    let target = if reuse {
        std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo.join("target"))
    } else {
        repo.join("target").join("lightbox-under-test")
    };
    if !reuse {
        let status = Command::new("cargo")
            .current_dir(repo)
            .args([
                "build",
                "--package",
                "ghostlight",
                "--bin",
                "ghostlight",
                "--package",
                "ghostlight-relay",
                "--bin",
                "ghostlight-relay",
                "--target-dir",
            ])
            .arg(&target)
            .status()?;
        anyhow::ensure!(status.success(), "isolated process build failed: {status}");
    }
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let directory = target.join("debug");
    let binaries = ProcessBinaries {
        service: directory.join(format!("ghostlight{suffix}")),
        relay: directory.join(format!("ghostlight-relay{suffix}")),
    };
    anyhow::ensure!(
        binaries.service.is_file() && binaries.relay.is_file(),
        "process binaries are absent under {}; omit --reuse-cache to build them",
        directory.display()
    );
    Ok(binaries)
}

fn wait_for_debug_state(log_dir: &Path, within: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if newest_debug_state(log_dir).is_some() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!(
        "service wrote no debug state under {} within {within:?}",
        log_dir.display()
    )
}

fn wait_for_webapi_port(log_dir: &Path, within: Duration) -> anyhow::Result<u16> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(raw) = newest_debug_state(log_dir) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(port) = value.get("webapi_port").and_then(serde_json::Value::as_u64) {
                    return u16::try_from(port).map_err(Into::into);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!(
        "service published no Console port under {} within {within:?}",
        log_dir.display()
    )
}

fn newest_debug_state(log_dir: &Path) -> Option<String> {
    let mut newest = None;
    for entry in std::fs::read_dir(log_dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("debug-state-") || !name.ends_with(".json") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        if newest
            .as_ref()
            .map(|(time, _): &(std::time::SystemTime, PathBuf)| modified > *time)
            .unwrap_or(true)
        {
            newest = Some((modified, entry.path()));
        }
    }
    std::fs::read_to_string(newest?.1).ok()
}

/// A temp directory that removes itself on drop.
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub fn new(tag: &str) -> anyhow::Result<Self> {
        let n = UNIQUE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("lightbox-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A minimal schema-3 manifest value naming the org and one all-hosts read grant.
pub fn manifest(name: &str) -> serde_json::Value {
    serde_json::json!({
        "schema": 3,
        "name": name,
        "version": "1",
        "grants": [],
    })
}

/// Sign a policy bundle over `manifest` at `seq` with the Ed25519 `seed` (evaluation-grade key).
pub fn sign(seed: &[u8; 32], seq: u64, manifest: serde_json::Value) -> Vec<u8> {
    bundle::sign_bundle(seed, None, seq, manifest, None)
}

/// The org's Ed25519 public key as lowercase hex, for a `managed.json` bootstrap.
pub fn pubkey_hex(seed: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in crypto_admin::ed_public(seed) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Write a `managed.json` bootstrap at `path` pointing at `source`, trusting the `seed`'s public key.
pub fn write_bootstrap(path: &Path, source: &str, seed: &[u8; 32]) -> anyhow::Result<()> {
    let json = serde_json::json!({
        "source": source,
        "pubkey_ed25519": pubkey_hex(seed),
    });
    std::fs::write(path, serde_json::to_vec_pretty(&json)?)?;
    Ok(())
}

struct ServerState {
    bytes: Vec<u8>,
    etag: String,
    version: u64,
}

/// A localhost HTTP server that serves a policy bundle (with ETag / 304 support) until dropped. The
/// served bundle can be swapped mid-run ([`BundleServer::set_bundle`]) for the poll-update scenario.
pub struct BundleServer {
    addr: SocketAddr,
    state: Arc<Mutex<ServerState>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl BundleServer {
    pub fn start(bytes: Vec<u8>) -> anyhow::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(ServerState {
            bytes,
            etag: "\"v1\"".to_string(),
            version: 1,
        }));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (st, sd) = (state.clone(), shutdown.clone());
        let handle = std::thread::spawn(move || serve_loop(listener, st, sd));
        Ok(Self {
            addr,
            state,
            shutdown,
            handle: Some(handle),
        })
    }

    /// The URL a `managed.json` `source` should use.
    pub fn url(&self) -> String {
        format!("http://{}/policy.bundle", self.addr)
    }

    /// Swap the served bundle (bumps the ETag), simulating the org publishing a new policy.
    pub fn set_bundle(&self, bytes: Vec<u8>) {
        let mut s = self.state.lock().unwrap();
        s.version += 1;
        s.etag = format!("\"v{}\"", s.version);
        s.bytes = bytes;
    }
}

impl Drop for BundleServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Unblock the accept() so the loop can observe the shutdown flag and exit.
        let _ = TcpStream::connect(self.addr);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn serve_loop(listener: TcpListener, state: Arc<Mutex<ServerState>>, shutdown: Arc<AtomicBool>) {
    for stream in listener.incoming() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else { break };
        let mut reader = std::io::BufReader::new(match stream.try_clone() {
            Ok(s) => s,
            Err(_) => continue,
        });
        let mut if_none_match: Option<String> = None;
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            if line.to_ascii_lowercase().starts_with("if-none-match:") {
                if let Some(idx) = line.find(':') {
                    if_none_match = Some(line[idx + 1..].trim().to_string());
                }
            }
        }
        let s = state.lock().unwrap();
        let not_modified = if_none_match.as_deref() == Some(s.etag.as_str());
        let response = if not_modified {
            format!(
                "HTTP/1.1 304 Not Modified\r\nETag: {}\r\nConnection: close\r\n\r\n",
                s.etag
            )
        } else {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: {}\r\nConnection: close\r\n\r\n",
                s.bytes.len(),
                s.etag
            )
        };
        let _ = stream.write_all(response.as_bytes());
        if !not_modified {
            let _ = stream.write_all(&s.bytes);
        }
        let _ = stream.flush();
        // Drain any remaining request body so the client sees a clean close.
        let mut sink = [0u8; 256];
        let _ = stream.read(&mut sink);
    }
}
