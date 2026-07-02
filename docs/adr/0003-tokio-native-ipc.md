# 0003. IPC transport: tokio-native named-pipe/UDS, single-session, no heartbeat

- Status: Accepted
- Date: 2026-07

## Context

The mcp-server and native-host roles bridge over a local socket (ADR-0002). Two forces
shape the transport choice. First, liveness: when the mcp-server dies, the native-host
must exit promptly, or it strands the Chrome extension on a dead bridge and blocks the
next session's server from binding the endpoint (commit 18b416b). Second, access: the
endpoint controls the user's browser, so no other local process may connect to it.

## Decision

The transport is tokio's native local socket (`tokio::net::windows::named_pipe` on
Windows, `tokio::net::Unix{Listener,Stream}` on Unix), not the `interprocess` crate and
not TCP (src/native/ipc.rs; commit 18b416b). These reads surface `Ok(0)`/`BrokenPipe`
promptly on peer death, so liveness is detected structurally by the relay's `select!`
completing; there is no application heartbeat.

Single active session is enforced at endpoint acquisition: on Windows,
`first_pipe_instance(true)` fails creation if the name exists (mapping to `SessionBusy`
and blocking name-squatting); on Unix, `bind` plus a probe-connect distinguishes a live
owner from a stale socket file. The relay role is a stateless pipe, so it
`std::process::exit`s once the relay ends rather than returning into a tokio runtime drop.
Access is locked down: an owner-only Windows pipe DACL (`D:P(A;;GA;;;OW)(A;;GA;;;SY)`),
and a Unix socket under a 0700 per-user dir at mode 0600 (not the permission-less abstract
namespace) (src/native/ipc.rs; commit 18b416b).

## Consequences

- Two independent causes of the native-host zombie are eliminated (commit 18b416b):
  (1) Transport: `interprocess`'s async Windows named-pipe read never woke on peer death
  (overlapped I/O plus an EOF-delaying "linger pool"), so the relay read hung forever;
  tokio-native reads surface EOF promptly. (2) Shutdown: even after the read returns,
  tokio's stdin reader parks a blocking thread in a ReadFile on Chrome's still-open stdin,
  and dropping the runtime hangs joining it; the stateless relay therefore process-exits.
- Rejected: a post-`select!` `ipc_write.shutdown().await`, which on an already-dead Windows
  pipe never completes and re-hangs the exit (src/native/ipc.rs).
- Zero new dependencies (tokio "net" was already enabled); `interprocess` is dropped.
- Trade-off: `process::exit` skips graceful teardown, acceptable because the relay role
  holds no state. Locked by tests/peer_death.rs, which force-kills the server and asserts
  the host exits within seconds.
