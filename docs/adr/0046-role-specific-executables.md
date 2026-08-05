# 0046. Role-specific executables: ghostlight + two named adapters

- Status: Accepted (2026-07-08)

## Amendment by ADR-0096 (2026-08-04)

The three-lifetime rationale remains. The current executables are `ghostlight-mcp-connector` for
exact MCP stdio behavior, `ghostlight` for the persistent protocol-neutral service and CLI, and
`ghostlight-browser-connector` for Chromium native messaging. Their entry points and crate
dependencies carry the separation. The former adapter and relay names, byte-relay MCP path, and
process-global role marker are retired; see ADR-0096 and its naming amendment for the replacement.

## Relationship to other decisions

- SUPERSEDES the "single portable executable" aspect of ADR-0001 and the "dual-role binary" aspect
  of ADR-0002. The IPC architecture those ADRs established is UNCHANGED (a governed core reached over
  a local socket); what changes is that the roles stop sharing one executable file. Zero runtime
  dependencies and full portability are preserved -- the product is still self-contained native
  binaries, just three of them from one workspace.
- REFINES ADR-0044 (named instances): the whole-binary per-instance copy shrinks to a tiny copy of
  the ONE arg-free executable (the browser adapter). The service and the agent adapter carry
  `--instance` as a flag (they have argument room), so only `ghostlight-adapter-browser` uses the
  argv[0] multi-call signal, and its per-instance copy is a small relay, not the multi-MB brain.
- REFINES ADR-0045 (the resilient adapter): the reconnecting-adapter logic becomes its OWN
  executable (`ghostlight-adapter-agent`) rather than a role of the shared binary. The behavior is
  identical; it just lives where it belongs.

## Context

The single multi-role binary made one executable file play three roles selected at runtime (by
argv[0], a subcommand, or a positional argument): the CLI + governed SERVICE, the MCP ADAPTER
(spawned by the agent's MCP client), and the native-messaging HOST (spawned by the browser). Those
roles have OPPOSITE lifecycles:

- The SERVICE is the churny part -- rebuilt constantly during development, upgraded in production.
- The ADAPTER and HOST are dumb, resilient pass-throughs -- spawned constantly by the agent's tool
  and by Chrome, and almost never changed.

Because they were the same file, the always-running pass-throughs fought the always-rebuilt service
over that file. The symptoms accumulated as scaffolding to make one file pretend to be several:

- **The exe-lock.** On Windows a running process locks its executable image. The agent's adapter
  (spawned by VS Code, running whenever the editor is open) held `ghostlight.exe`, so `cargo build`
  of the service failed. Stopping the terminal service did not help -- the editor's adapter still
  held the same file. No install flag fixes this; it is structural.
- **The multi-call copy.** Giving a non-default instance its own identity required copying the whole
  multi-MB binary just so Chrome could launch it under a per-instance name (ADR-0044 Decision 4).
- **The supervisor fight.** The OS supervisor auto-started the service from the build-output path,
  persistently holding the lock and competing for the single-owner endpoint with a hands-on dev
  service.

The root cause is one file with three lifecycles. Splitting the file removes the whole class of
problems instead of patching each symptom.

## Decision

### Decision 1: three role-specific executables, named for the side they face

One workspace produces three binaries:

| Executable | Role | Spawned by |
|---|---|---|
| `ghostlight` | CLI + governed service (the brain) | the user (`ghostlight install`), or the OS supervisor (`ghostlight service`) |
| `ghostlight-adapter-agent` | thin pass-through: the AI agent's MCP client <-> the core | the agent's tool (Claude Code, Cursor, VS Code) |
| `ghostlight-adapter-browser` | thin pass-through: the browser extension <-> the core | Chrome / Edge (native messaging) |

The two pass-throughs are named `ghostlight-adapter-<side>`: `adapter` states the role (a thin
connector), and the side (`agent` / `browser`) states which external party it faces. Category-first
so the two adapters sort adjacent and read as one family. The names tell the whole architecture at a
glance -- agent <-> ghostlight <-> browser -- with no protocol jargon (`mcp`, `host`) that only
means something to someone who already knows the internals. `ghostlight` stays bare: it is the
product, the command users type, and the hero name; the adapters are its named components
(the `git` / `git-lfs` shape).

### Decision 2: the crate split is what makes it work

Three executables alone would not fix the lock if they all relinked on a service edit. The split is
really a WORKSPACE split into two library crates plus the three thin binaries:

- `ghostlight-transport` -- the small, STABLE substrate the pass-throughs need: local-socket framing
  and dialing, the session hello + anti-squat proof, the resilient relay + reconnect + handshake
  replay, the instance derivation, endpoint/identity resolution, the OS-supervisor self-heal
  (identifiers + `start_service`), the parent-death watchdog, and the observability sink. Few, rarely
  changed dependencies.
- `ghostlight-core` -- the large, CHURNY part: governance, the tool/capability layer, the browser
  CDP protocol, the hub/service composition, the installer, and the CLI. Depends on
  `ghostlight-transport`.

The binaries: `ghostlight` depends on `ghostlight-core`; `ghostlight-adapter-agent` and
`ghostlight-adapter-browser` depend ONLY on `ghostlight-transport`. So editing governance or a tool
(in core) never relinks the adapters -- their files stay untouched, and a running adapter never
blocks a service rebuild. This crate boundary, not the file split by itself, is the actual fix.

### Decision 3: instance identity simplifies

- `ghostlight` (service + CLI) and `ghostlight-adapter-agent` take `--instance <n>` on the command
  line (the CLI is invoked directly; the agent adapter is launched from a client config that has
  argument room). No copy.
- `ghostlight-adapter-browser` is the ONLY arg-free launch (Chrome passes only the extension origin
  and `--parent-window`), so it keeps the ADR-0044 argv[0] multi-call signal: a non-default instance
  installs a tiny copy `ghostlight-adapter-browser-<n>` that the binary reads from its own name. The
  copy is a small relay, not the brain.
- The default instance stays byte-identical (ADR-0044 Decision 2 still holds): bare executable names,
  no copy, unsuffixed identifiers.

### Decision 4: distribution stays simple enough

The primary artifact is still `ghostlight`; the two adapters are small binaries the installer places
(the installer already writes native-host manifests and client configs, so placing two more files is
marginal). Packaging bundles the three; the one-line installers and the npm launcher fetch a small
archive rather than a lone binary. "Single file" is traded for "each process is one clean thing";
"zero runtime dependencies, fully portable" is kept.

## Consequences

### If taken

- The dev loop stops fighting the developer: edit core, `cargo build` the service, restart it; the
  agent and browser adapters keep running (different files) and the resilient reconnect (ADR-0045)
  reattaches them. No `--no-supervisor`, no separate target dir, no copy dance.
- Production upgrades get honestly transparent: the service file can be swapped while adapters run,
  because they are not that file.
- The role-multiplexing dissolves: no argv[0]/subcommand "which am I" branch. Each binary has one
  tiny `main`. The one remaining argv[0] read is the browser adapter's own instance name.
- The names document the architecture.

### Cost

- A workspace reorganization (two library crates + three binaries) and moving modules across the
  transport/core boundary. Most existing work ports rather than being discarded: `instance`, the
  identity derivation, the resilient-adapter relay, and the ADRs all move with minor edits; the
  install wiring is re-pointed at three executables; the role dispatch is deleted.
- Three binaries to build, sign, package, and version instead of one.
- A foundational reversal (ADR-0001/0002) that this ADR records deliberately, not by drift.

### Risks

- **Transport/core boundary creep.** If an adapter ever needs something from core, the lock returns.
  Mitigation: the adapters are dumb relays by contract (ADR-0005/0030) -- they need transport only,
  and a dependency on core is a design smell to reject, enforced by the crate graph itself.
- **Protocol skew between a stable adapter and a newer core.** The adapters are protocol-agnostic in
  the data phase (they relay bytes; only the hello/proof handshake is shared and stable), so a tool
  or governance change in core cannot desync them. The handshake is the one contract to keep stable.

## Migration outline

Each step keeps the tree green and is independently landable:

1. Introduce the workspace + `ghostlight-transport` crate; move the stable substrate into it, with
   the current binary still building on top. No behavior change.
2. Introduce `ghostlight-core` (the remainder), depending on transport. The single binary now builds
   from core. Still one executable; pure reorg.
3. Add `ghostlight-adapter-agent` (transport-only) with the resilient-adapter `main`; point client
   installs at it. Retire the adapter role from the main binary.
4. Add `ghostlight-adapter-browser` (transport-only); point native-host installs at it, including the
   per-instance copy for non-default instances. Retire the host role from the main binary.
5. Simplify `ghostlight`'s `main` to CLI + service only; delete the role dispatch. Update packaging
   and the installers to place three binaries.
