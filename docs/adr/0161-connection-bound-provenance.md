# ADR-0161: Connection-bound provenance reporting

Observed executable reporting (ADR-0105 stage 2, observed peer identity) superseded by ADR-0197 Decision 22.

Amended by ADR-0202 (2026-10-18): when the source is read, the accept time, the claimed agent's catalog id kept as a labelled claim, and when "in" may be said.

- Status: Accepted; reporting foundation implemented and verified locally; not deployed
- Date: 2026-09-07
- Amends: ADR-0105's reporting stage
- Builds on: ADR-0106, ADR-0102, ADR-0103, ADR-0156, ADR-0159, ADR-0160

## Context

The C1 review found that workspace-level attribution could change while earlier work was still
running. When two connections shared a caller-owned session, admitting the later connection
replaced the executable identity used to record the earlier connection's work. The workbench also
resolved action labels through current session state, so reconnects could mislabel history.

A fresh-build process fixture then exposed a second attribution bug: the Windows observer matched
the accepted socket's local TCP-table row and identified the service itself. The existing test
connected both endpoints from one process, so either direction produced its expected PID. The
observer must match the inverse address tuple to identify the remote process.

The owner accepted option A: implement the reporting foundation first. It fixes attribution and
makes existing evidence understandable without adding a verifier. Signature and hash verification
remain a later C1 cycle with a concrete subject. This decision does not complete all of C1 or select
C2/C3 admission mechanisms.

## Decision

### Capture evidence once for each connection

The orchestrator captures immutable evidence at service admission. Each connection gets a fresh
service-issued opaque identifier and local observation time. Reconnecting or sharing an existing
workspace does not reuse another connection's evidence. Caller-owned workspace lifetime remains
ADR-0106's contract; a workspace may hold several active connections.

Every prepared invocation retains its originating connection's evidence before it enters a queue.
Direct work, composed children, preparation failures, and refusals carry that same evidence to the
existing completion path. Waiting, another connection's admission, and disconnection cannot
change an admitted action's attribution. Disconnect removes only that connection from the current
session details; completed and still-running actions retain their own evidence.

### Separate application claims from durable observations

The existing service hello's application label remains a claim. It is bounded to 100 characters
with control and direction-changing characters removed. Familiar names remain useful in current
sessions, operations, and the bounded live history projection. The raw claim is not written to
durable audit or process diagnostics.

Durable attribution contains only:

- the service-issued connection identifier and observation time;
- the declared intake channel, which remains attribution rather than authority;
- a bounded observed executable basename or an explicit observation state;
- the explicit signature state `not_checked`.

An observed executable basename is at most 120 characters, normalized and path-free. No PID,
full path, command line, raw operating-system error, caller-supplied label, or certificate dump is
added to durable records. These observations do not make the executable trusted or approved.
Neither the basename nor the claimed channel becomes an admission rule.

The live projection receives the bounded claim separately from the serializable audit record.
After a durable-history reload, that claim is absent. The workbench uses the retained executable
name or existing opaque workspace identity for compact attribution; it never borrows the label
from a currently connected application.

### Report the evidence the platform actually provides

Windows corrects the existing socket-owner observation in `ghostlight-win-peer`: given the
observer's local and peer addresses, the TCP-table lookup matches the peer as the row's local
endpoint and the observer as its remote endpoint. The observed executable belongs to the directly
connected process; an ordinary MCP connection normally identifies Ghostlight's connector. The
upstream application's identity and the instructions it supplies remain unverified. A signed
interpreter would likewise say nothing by itself about the script it runs.

Observation states are closed: `observed`, `unsupported_platform`, `unavailable`, and
`not_recorded`. The human surface explains unsupported observation as "Unavailable on this
platform", a failed supported observation as "Could not identify this connection", and missing
historical evidence as "Not recorded". Linux currently has no socket-peer observer in this seam
and reports the unsupported state explicitly.

Signature status is always "Not checked" in this cycle. There is no hash calculation, signature
verifier, verification cache, certificate approval, identity admission, or new policy setting.
Fresh observation on every connection replaces any need for a provenance cache in this scope.

Older receipts remain readable and retain their declared channel. A legacy `peer_image` field
cannot establish the remote executable because the prior observer selected the service's row.
Human projections therefore show its executable observation as "Not recorded"; they do not
promote that old basename to remote-peer evidence. Original historical files remain
untouched. Missing connection identifiers and observation times are not reconstructed as facts.

### Keep ordinary work quiet and details accessible

Keep the familiar client chips. A collapsed Connection details section exposes all active
connections in the existing session surface. Each action and recorded composition child has its
own collapsed details beside the existing permission and coverage details. Open details and
keyboard focus survive projection updates.

Details distinguish "Reported application", "Observed executable", and "Signature". The
orchestrator authors the observation explanation and connector/upstream distinction. No trusted
badge, routine popup, or all-open admission consequence follows from unchecked or unavailable
evidence. The UI escapes claims and renders evidence; it does not infer identity or policy.

### Preserve existing process boundaries

The orchestrator owns evidence capture, immutable invocation attribution, safe audit projection,
and human explanations. The existing service bridge already supplies the claimed label, intake,
and local socket; its contract is unchanged. The MCP connector only stops copying raw labels into
durable diagnostics. No browser connector or extension feature is required.

## Acceptance and remaining work

Regression evidence must show two connections sharing one workspace without rewriting each
other's direct, queued, composed, failed-preparation, or refused receipts. History must retain
observed evidence after disconnect and restart, omit raw claim sentinels from durable files, and
state unavailable observations accurately. Windows tests must use distinct processes so a tuple
direction error cannot pass by observing the same PID on both ends. The real bundled UI must
preserve collapsed/open behavior, escaped labels, focus, and narrow layout.

The [C1 task record](../tasks/security-hardening/c1-reporting-foundation.md) owns validation and
implementation status. Signature/hash verification remains deferred to a further C1 cycle.
C2/C3 still require their concrete subject and connection proof under ADR-0105. This reporting
foundation adds no host containment, endpoint-wide audit, or upstream application-authentication
claim. Source completion, local deployment, and publication remain separate states.

## Amendment (2026-10-17): the 2.0 window replaces the chips and session-level details

The owner reviewed the 2.0 window on 2026-10-17 and decided decisions 1 and 4 of
[docs/tasks/break-2.0/design.md](../tasks/break-2.0/design.md). They replace two parts of
"Keep ordinary work quiet and details accessible":

- The familiar client chips are replaced by one summary pill and, behind it, an agent shelf and a
  browser shelf.
- The collapsed session-level Connection details section is replaced by a card per agent. The
  card shows that agent's sessions in words, the trust note once, the intake only when it is
  unusual, and the connection id behind Copy id.

The per-action Connection details in the activity detail card stand as written. The live MCP
explanation now names the reported claim. The rest of the section is unchanged: the
orchestrator authors the explanation, the UI escapes claims and renders evidence, and no trusted
badge or admission consequence follows from unchecked evidence.

## Amendment (2026-10-18): every connection knows the app that started it

ADR-0197 Decision 22 removed the observed peer identity: the 1.x observer read the directly
connected executable, which on an ordinary MCP connection was Ghostlight's own connector and said
nothing about the agent. The window then said of every agent that its name "is what the agent
calls itself; Ghostlight does not verify it." On the owner's machine the three `ghostlight.exe
mcp` edges were children of `claude.exe`, Claude Desktop from the Microsoft Store, whose Code tab
runs Claude Code sessions, and their clients claimed "claude-code". The owner, 2026-10-18: "an
MCP connector that's invoked by claude-code.exe has a clear hint." This amendment brings the
observation back in a form that names the app above the connector, and supersedes Decision 22's
removal of observed peer identity.

### Observed, never claimed

- The source is decided by the service from operating-system facts about the connection's peer,
  never from a field a client writes. A `_source` in the JSON-RPC body would be forgeable by any
  client, so the MCP intake still reads only `name` and `arguments`.
- The relay's session listener already read the peer's process id to admit it
  (`GetNamedPipeClientProcessId` on Windows, `SO_PEERCRED` on Linux). The accepted connection now
  keeps it (`Connection::peer_process_id`). No byte on the wire changes, and every process-tree
  read stays in the relay's audited `os` module (`os::ancestor_images`): one process snapshot and
  query-only opens on Windows, `/proc` on Linux. That is the separation-of-concerns case for
  touching the relay seam: the fact is physically the listener's, and only the platform layer can
  read it.
- The walk reads at most 16 ancestors and ends at a parent that has exited, cannot be read, or
  started after its child (a reused id). A failed read leaves the source unknown and never fails
  the connection.

### Adaptive, not a table

- An ancestor is a known agent's app when its executable lies where that agent's app is installed
  on this computer, found on every look with the lookups the icon resolver already uses
  (`install::apps`): a Store app is the install folders Windows names for its package family, a
  program below a known folder is that program's folder, and a command on `PATH` or the program a
  Linux desktop entry starts is that one file. So a Store Claude Desktop is recognized by where it
  is installed, not by a list of executable names.
- The walk skips wrappers: shells, consoles, script interpreters, package runners, process
  wrappers such as `timeout`, and Ghostlight's own executables, named in one place
  (`install::apps::is_wrapper`). The icon resolver skips the same list in a launcher script.
- The source is the first ancestor that is a known agent's app, else the first that is not a
  wrapper, by its executable file name.

### One value on the connection

- `Source { agent, image }` is decided once when an `mcp` stream opens, before anything the
  client sends, and every connection evidence of that stream inherits it. (Amended by ADR-0202
  Decision 3: it is decided once per connection, after the service has answered the opening and,
  for a fresh session, `initialize`; nothing the client sends takes part, and the connection's
  time is its accept time.) `agent` is the catalog
  id when an agent's app started it; `image` is the executable's file name, never a path, since
  paths carry the person's name. The source's channel is the connection's existing intake channel
  (`mcp` or `cli`), so the source does not repeat it. Browser, control and desktop streams carry
  no work to attribute and decide no source.
- It is provenance for display and history, never authority: governance decides by domain and
  capability, not by who claims or launched, and nothing in governance or admission reads it.
- Subagents of one client session share one edge process, so they share one source. The
  never-guess rule for tabs (ADR-0198, amendment of 2026-10-18) is unchanged: a source does not
  tell callers on one connection apart.

### Up to the surface

The connection's agent keeps its claimed identity, and the source corroborates it. The words are
composed in `language::provenance` and reach the window's agent card, the activity detail card and
history receipts through the connection explanation:

- The source is the claimed agent's own app: the caveat goes, and the sentence says where it was
  seen, "Started by Claude Desktop."
- The claimed agent runs inside another agent's app: both, "Claude Code, in Claude Desktop."
  (Amended by ADR-0202 Decision 14: only when the catalog knows the claimed agent runs inside that
  app; otherwise both names are given without judging, as for a name the catalog does not know.)
- The claim and the source do not line up (a name the catalog does not know, or a source that is
  no agent's app): both, without judging either, `"claude-code" is what the agent calls itself.
  Started by WindowsTerminal.exe.`
- No source: the caveat stays as it was.
- A command-line session says where it came from: "A ghostlight call from WindowsTerminal.exe.
  Ghostlight does not verify the scripts and instructions it runs."
- History keeps no claim, so a recorded MCP action with a source says "Started by ..." and one
  without keeps the earlier explanation.
  (Amended by ADR-0202 Decisions 14 and 14a: the record keeps the claimed agent's catalog id,
  labelled a claim, never the client's words. One function in `language::provenance` names a
  restored row as it names a live one: by the claimed agent, else the source's catalog agent, else
  Unknown agent, so Claude Code work in Claude Desktop reads "Claude Code, in Claude Desktop."
  after a restart too.)

The watcher audience sees the same connection explanation it already saw; no part gains a key.

### Audit and history

The durable attribution carries `source` (`agent` and `image`) beside `connection_id`,
`observed_at_ms` and `channel`, content-free and path-free. Reading bounds both again: a path is
cut to its file name and a value that is not a catalog id is dropped. Records without a source,
and records from before, read unchanged. The SIEM guide documents the field, and the audit
goldens show it.

(Amended by ADR-0202 Decision 14a: the attribution also carries the optional `claimed_agent`, the
catalog id a client's claim names, labelled a claim. A claim the catalog does not know is not
kept. The SIEM guide and the audit goldens show it.)
