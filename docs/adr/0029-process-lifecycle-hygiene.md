# 0029. Process lifecycle hygiene: parent-death exit and doctor --fix

- Status: Accepted
- Date: 2026-07

## Context

The mcp-server role is spawned by the MCP client (Claude Code, Claude Desktop, VS Code)
over stdio. Its intended exit signal is stdin EOF: `server::run`'s read loop ends when
`lines.next_line()` returns `None`, the process unwinds, and the OS releases the IPC
endpoint so the next session can bind (ADR-0004 already noted "a crashed or stale session
must release the endpoint before a new one can bind ... keep that window short").

On Windows that signal is not reliable. When the parent is killed rather than closed
cleanly -- a VS Code window reload, a Claude Code auto-update that swaps the extension
host, a crash -- the child's stdin handle is not always closed, so the blocking ReadFile
that backs `tokio::io::stdin()` never returns EOF and the read loop blocks forever. The
process becomes an orphan that still exists but serves no one. The codebase already hit
this exact class of bug for the native-host role (see `main::run_native_host_role`'s note
about a parked ReadFile on Chrome's still-open stdin).

Observed in the field: `ghostlight doctor` reported the IPC endpoint absent while twelve
orphaned mcp-server processes were alive, several from a Claude Code version (2.1.200) that
had been replaced by 2.1.201 more than an hour earlier -- proof that stdin EOF never fired
for them. Because `serve` acquires the endpoint once and gives up on `SessionBusy` with no
re-acquisition, an orphan that once owned the endpoint leaves it unowned on death, and the
surviving orphans never take over. The extension then cannot connect to anything, and the
only recovery was killing processes by hand in Task Manager.

Per-process observability files (`debug-state-<pid>.json`) exist only under `--debug`, so
in a release build there is no process registry at all, and `doctor`'s session view is
empty.

## Decision

Make the server self-terminating, make staleness legible, and give the user a one-command
repair. Four parts, prioritized by invisibility (a user should never learn the word
"zombie").

1. **One shutdown coordinator, many detectors (the primary fix).** Shutdown is a single point
   of concern, not spread across the code. Every trigger is a pure *detector* that reports to
   one coordinator; the coordinator runs the ordered teardown (flush observability, then exit,
   releasing the endpoint) exactly once. No detector tears down or exits on its own. The
   detectors are stdin EOF (the MCP read loop returning) and the **parent-death watchdog**: at
   startup it records the parent process and polls its liveness on a light interval, signalling
   the coordinator once the parent is gone. The teardown uses `process::exit`, not an unwind,
   because a detector-triggered shutdown can leave the stdin read parked in a blocking ReadFile,
   and dropping the runtime would hang joining that thread (the same reason the native-host role
   exits directly). Detecting the parent lives behind a small `crate::proc` seam:
   - Windows: the parent pid (one CreateToolhelp32Snapshot walk at startup) plus the parent's
     creation time (GetProcessTimes). "Orphaned" means that pid is no longer *running* with the
     same creation time, so a reused pid reads as dead, not alive.
   - Unix: the original `getppid()`. "Orphaned" means `getppid()` no longer equals it (the
     kernel reparents an orphan to init/launchd). No pid-reuse hazard, since getppid reflects
     the real current parent.

   **Liveness is the termination signal, not object existence (a correctness landmine).** A
   Windows process object stays queryable via `OpenProcess` for as long as *any* handle to it is
   open, and a parent holds handles to its children -- so "OpenProcess succeeds" does NOT mean
   "running". The MCP client's own parent (e.g. VS Code) holds the client's handle, so a naive
   existence check would see a dead client as alive and the watchdog would never fire in
   production. `crate::proc` therefore tests liveness with `WaitForSingleObject(handle, 0)`: a
   process handle becomes signalled on termination and every handle observes it, so a
   terminated-but-held process correctly reads as dead. This is pinned by a regression test
   (`terminated_process_reads_as_dead_even_while_a_handle_is_held`). Do not "simplify" liveness
   back to an OpenProcess existence check.

2. **Liveness-aware doctor.** `doctor` cross-references each recorded session's pid against
   the OS: a dead pid is labelled "exited" (informational, not a problem), a live pid whose
   recorded parent is dead is an "orphan" (a problem), and a live pid with a live parent is
   healthy. Session snapshots gain the parent pid and (Windows) parent creation time so this
   classification is precise. Files are NOT deleted on clean exit -- post-mortem inspection
   via `ghostlight status` is worth keeping, and the 24h `cleanup_stale` sweep plus `--fix`
   bound the litter.

3. **`ghostlight doctor --fix` (the visible safety net).** An explicit, opt-in repair that
   reaps orphaned sessions -- alive process, dead recorded parent -- and removes state files
   whose process has exited, then reprints the verdict. This is the one place `doctor`'s
   otherwise strict "never writes, deletes, or kills anything" contract is relaxed, and only
   behind the flag. When the plain `doctor` verdict detects an orphan or a stale holder it
   names `ghostlight doctor --fix` as the remedy, at the point of pain.

4. **Startup sweep (self-healing glue).** The same orphan reaper runs once at mcp-server
   startup, before serving, so a fresh session tidies up after a predecessor that died
   uncleanly without the user ever invoking anything.

**Safety guardrails (binding on parts 3 and 4).** Reaping targets ONLY parent-dead
orphans. A process whose recorded parent is still alive is never killed -- that is a live
client's session (possibly a degraded second session per ADR-0004), not a zombie. The
current process never reaps itself. On Windows the parent match includes creation time, so
pid reuse cannot make a live process look orphaned; on the ambiguous Unix case the reaper
errs toward NOT killing (a reused parent pid reads as alive). Killing uses the OS terminate
primitive on the specific pid only.

## Consequences

- Orphaned mcp-servers stop accumulating: the watchdog ends each process within one poll
  interval of its client's death, in release and debug builds alike, and the endpoint frees
  for the next session. The field failure above cannot recur.
- `doctor` tells the truth about liveness instead of listing dead and hung processes
  identically, and now offers a repair rather than only ever printing "kill it yourself."
- Cost: one light polling task per server; a new `crate::proc` platform module; two added
  `windows-sys` features (`Win32_System_Threading`, `Win32_System_Diagnostics_ToolHelp`)
  and a `libc` dependency under `cfg(unix)`.
- Scope limit: the reaper reads the `--debug` session registry, so in a release build it
  finds nothing to reap -- acceptable because the watchdog prevents orphans there in the
  first place. A registry-independent OS enumeration reaper (finding orphaned ghostlight
  processes with no state file) is deferred; if it lands it reuses the same parent-dead
  guardrail. Recording an always-on minimal registry is the other option, also deferred.
- Rejected alternatives: a Windows Job Object with kill-on-close would bind child to parent,
  but the parent (the MCP client) creates the process and we cannot make it set that up; a
  stdin read timeout does not address a read that blocks rather than erroring; an
  application heartbeat was already rejected for the IPC layer (ADR-0003) and is equally
  unnecessary here, since parent liveness is the real signal.
