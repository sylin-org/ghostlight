# Ghostlight 1.0 architecture

## Shape and dependencies

Ghostlight has three process or trust-boundary components and one shared wire crate. The edge is
one executable that runs as two relays:

```text
MCP client
  -> ghostlight mcp (the edge as the MCP relay)
  -> relay protocol 4
  -> ghostlight-service
  -> relay protocol 4
  -> ghostlight chrome-extension://... (the edge as the browser relay)
  -> browser native messaging
  -> policy-free Chromium adapter
  -> advertised privileged browser mechanisms

local human
  -> ghostlight-desktop (the window and the tray, over the loopback workbench)
  -> typed WorkbenchFacade
  -> the same orchestrator contexts
```

- `crates/relay` is the frozen relay protocol 4: the endpoint, the peer checks, framing, the
  opening, start reasons, the session listener and the route an edge invocation takes. The browser
  adapter component lives in `crates/orchestrator/src/browser/adapter`, because only the service
  reads it: one module per browser family, holding the typed wire vocabulary of each level the
  extension can speak and every page asset that runs in a page.
- `crates/bridge` holds the service's typed diagnostics contracts.
- `crates/orchestrator` is the domain-driven modular monolith and service process, built as
  `ghostlight-service`.
- `crates/edge` is the `ghostlight` executable. It depends on no workspace crate but `relay`, and
  knows no verb. Run from `<home>/bin` with the first argument `mcp`, it is the hand-rolled
  JSON-RPC MCP stdio relay. With a first argument that is a Chrome extension origin, it is the
  frame relay between native messaging and the service. Anything else, or nothing, it hands to
  `<home>/bin/ghostlight-service` with the same arguments. A copy that does not run from
  `<home>/bin` is a carrier and hands to the service image beside it. On Unix a hand-over is an
  exec. On Windows the edge is a console-subsystem process that runs the image as a child on its
  console, keeps running through Ctrl-C and Ctrl-Break and returns the child's exit code; its
  application manifest sets `consoleAllocationPolicy` to `detached`, so a double-click shows no
  console on Windows 11 24H2 and later. The release service image is a GUI-subsystem program, so
  a typed command attaches to its parent's console first and writes there and receives Ctrl-C
  itself; `serve`, the bare launch and `open` never attach.
  On Windows each executable names itself in Task Manager: "Ghostlight connection" for the edge
  (one per MCP client or browser), "Ghostlight" for the window and, because Tauri takes its
  description from the product name, "Ghostlight" for the service too.
- `extension` implements the full Chromium physical mechanism set, page-runtime installation,
  browser topology, content-free feedback, and the local toolbar/options experience.

Only process, lifecycle, and trust boundaries justify these crates. Product contexts remain
modules inside the orchestrator. The dependency direction points inward to `bridge` types and
orchestrator ports. The orchestrator does not depend on MCP or Chromium APIs.

The `ghostlight-service` executable links no UI toolkit and serves with no display. The shell,
`ghostlight-desktop`, draws the window, the tray and the notices, and presence (below) keeps it
running whenever the desktop can show it. A computer with no display, no web view runtime or no
tray still gets a service, and `status` and `doctor` say why there is no window.

## Local lifecycle

There is one Ghostlight home per OS user, `~/.ghostlight`, and every path derives from it
(ADR-0197 Decision 10): the installed executables `ghostlight` and `ghostlight-service` in `bin`, `run` (the service lock and, on Linux,
the relay socket), the state
directory `state` (audit log, MCP client locations, the extension handoff marker), the policy
directory `policy`, the Windows native-host manifest in `manifests`, and diagnostics in
`diagnostics` with the `diagnostics.on` marker.
`GHOSTLIGHT_TEST_HOME` names another home and exists only for tests and journeys. Every route
installs into `bin` (ADR-0197 Decision 13); registrations name the executables there.

Before any command runs, `main` applies the entry rule (`install/entry.rs`, carried out by
`cli/start.rs`). Only `bin/ghostlight-service` serves. Any other copy is a carrier: a command
that uses the service installs from the carrier's own directory when the installed service is
absent, older or damaged, then runs the installed one. `doctor`, `status`, `help` and `--version`
never install. A carrier that finds the development channel in `bin/install.json` installs
nothing and runs the installed service. The edge's own verbs, `mcp` and a Chrome extension
origin, are handed to `bin/ghostlight`: a service image that receives one hands it over,
installing first when it is a carrier and the installation is not the development channel.

`deployment install` (`install/deployment.rs`) is the only writer of `bin` (ADR-0197
Decision 14 and the development channel amendment of ADR-0199). Under the install lock it checks
the break migration's destinations
(`install/migration.rs`) so a conflict changes nothing, copies the 1.x policy, managed-policy and
client-location files into the home and leaves the originals in place (the 1.x service that is
still running keeps reading its policy from them, and a service or client started by the next
steps must find the copies), then compares `bin/install.json` with the
payload: an older payload is refused, the same intact version with an intact edge is left alone, the service image is
rename-swapped (`install/swap.rs`), and the edge is replaced only when the relay revision
changes, the edge in `bin` is missing or is not the one the record names, or `bin` still holds
the layout of an earlier development build, where the service image had the edge's name and two
connector images sat beside it; the installer stops those exact images and deletes the connector
files. It also stops what runs from each 1.x `bin/v<version>` directory, moves the audit log the
directory holds into `state` under a name that says which version it came from, and removes the
directory (`install/version_directories.rs`); a log that cannot be moved keeps its directory.
A release also refuses to install over the development channel and names the restore
commands of the release carriers (npm and the get scripts), after it has checked that the payload
is not `bin` itself. `--development` places the same files in `bin` the same way, but swaps the service
every time, whatever version is installed, and records `"channel": "development"` in
`install.json`. `--restore` installs this executable's own package the same way whatever is
installed, records the release channel, and is the one allowed move down. It is not offline: the
package it installs is a release carrier's, from npm or the get scripts. Until 2.0 is published,
the published npm package and the main-branch get scripts have no 2.0 `--restore`, so the way back
is a 2.0 build's own `deployment install --restore`, run from that build's directory. A carrier
that finds a development record with the executable it would run gone, and `doctor`, name the same
route. Earlier development
builds ran from `~/.ghostlight/dev`, selected by the marker `~/.ghostlight/development`. An
install that finds either stops what runs from there and removes both once `bin` holds the new
executables; an entry that cannot be removed is reported and removed by the next install.

As soon as `bin` holds the executables, and before anything old is stopped,
`install/reconcile.rs` points every registration at `bin`: native-host manifests (reclaiming
one that names another path), owned MCP client entries, the Linux desktop entry, command link and
manual pages, and on Windows the Start-menu shortcut and Uninstall key (`install/windows_entry.rs`).
On Windows the command line is the `bin` directory on the per-user `Path` (`install/user_path.rs`):
install appends it, uninstall removes only it, a `WM_SETTINGCHANGE` broadcast tells new terminals,
and a container reads it, never writes it, and leaves a missing entry for a regular terminal like the other redirected registrations.
A client entry Ghostlight does not own, such as an `npx` launcher, is reported and left alone.
On Windows the one manifest lives in `manifests`, which no app container redirects, and each
browser's `HKCU` value names it; a value is written only when it differs from what is read
(`install/native_host.rs`). A process whose ancestor is a packaged app, and whose probe write below
`%LOCALAPPDATA%` lands in that app's private copy, is inside that app's container, where registry
and `AppData` writes reach only that app (a packaged terminal that is not redirected is not). There
the install (`install/container.rs`) writes the manifest, leaves those redirected registrations
unwritten, except the configuration of the container's own app, and reports them with the one
command that finishes them from a regular terminal, `<home>/bin/ghostlight.exe install`; `doctor` says it cannot see what other apps see. An install
outside a container moves the older manifest out of `%LOCALAPPDATA%\Ghostlight\NativeMessagingHosts`
and removes that directory when it is empty.
Only then does the installer stop the 1.x images (and look again for a few rounds for any that
respawned), ask the running service to quit and stop that exact image if it has not exited,
retire the copied originals (one a 1.x program rewrote after the copy is published over the copy
first; an original and a copy that both changed are kept, the original set aside as
`<name>.pre-2.0`, and the install reports it, so the next install is never blocked; a file
beside each copy records what was copied, so an install that fails before this step is finished
by the next one by the same rule),
archive the 1.x audit log, delete the 1.x lifecycle files with the installation record last,
start `bin/ghostlight-service serve --reason update`, and write `install.json` only once a service answers
a control status with an instance other than the one it replaced. The quit is a control `quit` for
replacement, and the installer waits up to 10 s for `run/service.lock` to be free; an earlier
development build that cannot be asked, or an installed image that still runs, is stopped by its
exact image.
Registering first matters: a browser relaunches its host from the registration the moment the old
one stops, and an old registration would start the 1.x connector again.
When the service starts from `bin`, it registers any browser that has no registration yet.

Every start of the service is `serve --reason <word>`, and the reason is one of six: `agent` (the
MCP relay, at its own start and for each held request), `browser` (the browser relay, once, at
its own start), `command` (`call`, when no service answers), `person` (`open` and a bare
launch), `recover` (the shell, after a loss without goodbye) and `update` (the installer). `serve`
requires the reason. `status`, `doctor`, `help` and `--version` never start the service. In
order, `serve` refuses an image other than `bin/ghostlight-service`,
refuses an elevated Windows token, applies the quit latch, takes the operating-system lock on
`run/service.lock`, and only then opens state. The lock is held until exit and admits the single
local authority; the endpoint's first-instance flag is a second refusal. A start that finds the
lock held exits 0 at once and touches nothing, because the service that holds it answers the
starter's own connection; only an `update` start waits, up to 30 s, so the service it replaces
can finish leaving. A service that is not accepting openings within 30 s of taking the lock exits
with a nonzero status and a diagnostics event, so a stalled start never blocks the next one. The
service binds the endpoint as soon as it has started, with the session listener and the quit signal
already installed (ADR-0199 Decision 5). The timed discovery of ADR-0166, which waited for a
desktop to be ready before the endpoint existed, is gone with the desktop.

A person's Quit (the tray, the workbench, or control `quit` with reason `quit`) writes the latch
`state/quit` before the service finishes in-flight work and exits. While it exists a `browser` or
`recover` start exits 0 with a diagnostics event before it takes the lock, and any other start
deletes it after taking the lock and serves. So after Quit a browser restart does not bring
Ghostlight back; the next agent request, a command that needs it, or a person does. Before it
closes its streams the service sends a goodbye on each: `quit`, `replace` (the installer's quit)
or `uninstall`. The MCP relay answers its outstanding requests as cut off on every goodbye.
After `quit` it holds nothing and starts the service again only for the next request. After
`replace` it holds lines and reconnects without starting the service for up to the hold limit,
and then behaves as after a lost connection. After `uninstall` it exits. The browser relay
tells the extension the service is away after `quit` or `replace`, waits, never starts the
service, and exits when a service accepts again; after `uninstall` it exits at once. A stream that
ends with no goodbye is a crash and is handled as before.

A no-argument launch and the explicit local-human `ghostlight open` intent send control `show` to
a running service. When no service answers, they start `bin/ghostlight-service serve --reason person`
detached and wait for it to accept an opening. The service answers `show` by asking the attached
shell to show its window, or, when none is attached, by starting one with no argument and showing
the window when it attaches; the command line retries until the window is shown or 15 s have
passed. Nothing serves in the foreground. The request admits no workspace and adds no listener.

Presence (`service/presence.rs`) keeps the shell running whenever the desktop can show it. The
service starts `bin/ghostlight-desktop --tray` 2 s after it is ready when no shell is attached, and
2 s after the last shell stream ended with no goodbye. A `--tray` shell shows only the icon; a
request to show the window starts a shell with no argument, which opens its own window. After a
launch nothing else launches until a shell attaches or 15 s pass, at most three launches happen in
two minutes, and once they are spent the reason `did_not_stay` is latched until a person acts
(asks to show the window, or starts a shell that attaches). A shell that says it cannot show a
window (`unavailable`: no display, no web view runtime, no tray) or that closed with no tray is
recorded as the reason, spends no budget, and nothing launches until a person acts. Nothing
launches while the session is ending or a quit has begun, when `bin` holds no shell, or when no
display is reachable: on Linux `DISPLAY` or `WAYLAND_DISPLAY` is set in the service's environment,
and on Windows the process sits in the visible window station (so not in session 0 and not over an
OpenSSH session). The control `status` answer carries the reason as `window`.

The service owns the tray model and the shell draws it. The model is a state (ready, working,
paused or attention, from the same readiness the band shows; a service with no browser connected
looks ready), a tooltip (the sentence that counts connections) and four entries: Open and Quit
always, Pause while work is active, Resume while it is paused or ended. The first workbench frame
carries it and a `tray_model` frame follows each change. The shell draws the state as a small dot
over the original icon bytes at run time; no other artwork is shipped.

The tray and the service do not depend on a permanent window. Closing the window destroys only the
disposable workbench view in the shell, which stays with its icon; the shell tells the service
(`closed`), and the first time, the service explains the icon in one notice, once per home
(`state/tray-hint`). Windows Open focuses or restores the existing window and rebuilds it when
absent. Wayland cannot report or unset a client's minimized state, so Linux Open coalesces,
destroys any existing view, waits for the destroyed event, and rebuilds from the canonical
configuration. On Linux, abnormal WebKit renderer termination discards that exact window after the
signal callback; the next explicit Open creates a fresh WebView without an automatic crash loop.
Explicit Quit alone ends the service, and it holds (see above).
Every Ghostlight process honors the end of the session: the service, the edge in every role, and
every one-shot command. The listener is one module, `ghostlight_relay::os::session`. On Windows it is a
hidden top-level window on its own thread, never a message-only window, plus the console handler;
it answers `WM_QUERYENDSESSION` with TRUE at once, and the console handler returns FALSE so the
default exit is never suppressed. On Linux it is SIGTERM and SIGHUP through a handler that writes
one byte to a self-pipe and a thread that reads it. One process-wide state says the session is
ending. It is set at the first signal, the query included, and lifted only when the system
cancels the end. On Windows the state also asks the system (`GetSystemMetrics` with
`SM_SHUTTINGDOWN`), so a process born after the broadcast, such as a browser relay the
extension relaunches during a sign-out, refuses too. Linux has no equally cheap question: only a
process that received the signal refuses. While it holds, `start_service` and `spawn_detached`
refuse and nothing is spawned; browser recovery launches no browser and the install handoff opens no page.

On the final signal a process arms a 2 s fallback exit first. The service then says
`goodbye{session_end}` on every stream and leaves through the desktop's exit, without waiting for
work in flight and without setting the quit latch. An edge, which may find the service already
gone, exits 0 on its own signal and on a `session_end` goodbye, and a stream that drops while the
session is ending never leads to a start. A one-shot command leaves at once. Desktop retention is
disarmed while the state holds, so the native event loop never keeps the process alive.

## Fringe stability

Ghostlight's fringes are independently versioned compatibility products. Their size is not
constrained; their reasons to change are. The edge as the MCP relay changes only for MCP
negotiation and protocol compatibility and local service connection lifetime. As the browser relay
it changes only for native messaging, the local relay endpoint and its peer checks, framing, and connection
lifetime.
Each browser adapter changes only for privileged browser mechanisms, browser-execution integrity
and recovery, or the preserved Ghostlight user experience. Any product feature expressible through
existing physical browser capabilities changes only the orchestrator.

The browser relay treats adapter frames as opaque bounded bytes. A small relay protocol owns
the peer checks and backend availability separately from the adapter protocol. The relay keeps
Chromium's native port alive while the service is unavailable, reconnects to the endpoint of the
home, repeats only the relay and cached adapter handshakes, and never interprets or replays a
physical operation.

The adapter is a policy-free browser execution engine, not merely a collection of API calls. It
owns browser-local operation state, safe mechanism retries, duplicate-operation suppression,
physical observation, factual receipts, adapter resynchronization, and fail-safe local human
control. It does not own model-facing tools, workspace authority, governance, product recovery, or
journey composition. A physical effect is repeated only when the browser engine can prove that it
was not dispatched or that the physical sub-step is idempotent. Insufficient evidence produces an
unknown disposition.

Compatibility uses separate axes for the external MCP revision, the service edge bridge, the
browser relay, and the adapter protocol. Adapter behavior is selected by explicitly advertised
physical capabilities, never by parsing an implementation version. A new service may continue to
use an older adapter for every capability that adapter advertises.

Each adapter has one closed executable handler table. Its hello capability revisions are derived
from that table and its passive frame handlers, never maintained as a second declaration. Platform
is identity and presentation information; executor dispatch is capability-based. A vendor protocol
such as CDP is private adapter technique, not a second bridge vocabulary.

The browser adapter is a service component. `browser/adapter/mod.rs` names a closed `Family`
(Chromium) and re-exports the service's internal browser work vocabulary from
`browser/adapter/work.rs`: `BrowserCommand` in, `BrowserOutcome` back, `BrowserEvent` between
calls. Chromium speaks one wire, adapter protocol 4 (`adapter/chromium/protocol4`, specified in
`docs/design/adapter-protocol-4.md`). Its translator turns each unit of work into frozen
primitives and back. Any first frame that is not a protocol-4 hello hears `adapter_incompatible`
with a sentence telling the person to update the extension, and its socket closes.

Page-local observation and presentation ship as one service-owned source assembled in
`adapter/chromium/protocol4/assets.rs` from `assets/world.js` and the files its include marker
names. The process assembles and hashes it once and sends it in every `hello_accepted`; the
extension installs it into the isolated world `ghostlight` of every frame. Node tests for those
files sit under `assets/tests` and run with `node --test`. The golden fixtures in
`tests/golden/protocol4` pin the asset id, the wire and the world messages for both the service
and the extension.

## Stable bridges

The service bridge has generic messages for hello, catalog, invoke, cancel, result, and error, plus
authenticated pre-session openings for workbench activation and content-free readiness inspection.
Invocation carries a tool name and JSON input. Catalog, result, and readiness projections are
opaque to the bridge; their meaning remains in the orchestrator. One small typed content vocabulary
lets the edge render bounded screenshots and GIFs without knowing which tool produced them. MCP
request ids and protocol revisions do not cross this bridge. Readiness inspection returns before
workspace creation and never demand-starts the authority or writes audit.
The MCP edge renders each result as the lean model view in `structuredContent`, with the authored
summary and safe next steps in the ordinary text block; a session on an MCP revision before
2025-06-18 gets the same lean JSON in that block too. The view only omits fields at their default.
It never branches on a tool name, and the canonical envelope stays in the service for audit,
history and flow references. This keeps the edge free of product vocabulary.

The browser bridge has generic messages for hello, primitive request, receipt, browser event,
cancel, and presentation. It carries a closed primitive vocabulary but no model-facing tool
names, product defaults, governance decisions, recovery text, or result envelopes.

The service bounds unfinished exchanges and response delivery while keeping healthy idle peers
connected. Fixed per-connection workers absorb bursts in bounded ordinary and independent-control
queues. The executor decodes once at intake; waiting consumes the original deadline and reaches
the same completion/audit path. Sustained waits appear in the existing workbench operation list.
ADR-0160 records exact bounds and the limits of this local resilience guarantee.

Connection evidence is immutable at service admission (ADR-0161). Every prepared invocation keeps
its own evidence across queue waits, workspace sharing, composed children, preparation failure,
and refusal. The workspace lists active connections independently; closing one removes only its
live details. Receipt storage receives a bounded attribution projection with no reported application
text, while the bounded live human projection receives that claim separately. Restored history
never borrows a current session's identity. The service does not identify the program on the other
end of a connection, and provenance grants no authority. Audit lines with extra attribution
fields still read; the extra fields are ignored.

Every peer reaches the service through one endpoint (relay protocol 4, ADR-0199 Decision 5; the
contract is [relay-protocol-4](../design/relay-protocol-4.md)): a named pipe on Windows and a Unix
socket in `run` on Linux. The relay has no port, token or discovery file; the workbench's one
loopback port carries no relay peer. The endpoint belongs to the user and the home, only the same
user can open it, and each end checks the other's user before a payload byte moves. The relay framing, a four-byte little-endian length prefix, is Chromium's, so
the browser relay and the adapter protocol use it end to end and the native host can forward
bounded adapter frames without decoding them. A connection opens with one opening naming its role
(`mcp`, `browser`, `control` or `desktop`, the last only from a shell of the same build; `command` is refused as unsupported) and the relay
revision, and the service answers with its version, an instance that changes on every start and,
for `mcp`, the failure templates. An opening that names an earlier relay revision is refused before
any work is accepted. A `control` opening asks `readiness`, `status`, `show` or `quit`. Correlation
ids are opaque.

The browser hello also carries a persistent opaque adapter installation id, a restart-local adapter
epoch, an optional bounded product name, a reported attention fact, and versioned physical
capability declarations. They identify and describe one local extension installation for
diagnostics, compatibility, reconnect continuity, and routing. The installation id is the browser's
identity: it never grants authority and never replaces workspace ownership, but it is the key the
service routes on and the opaque handle a model may name. Human toolbar actions
cross the browser bridge as a closed control-intent vocabulary. The service applies them to its
authoritative runtime controls and publishes the resulting content-free control state back to the
adapter. Ended state is terminal for hold and resume; only explicit start-new-session intent creates
a new active runtime session.

Browsers are plural (ADR-0114). The service holds one connection per browser identity, so several
browsers are connected and worked in at once. A hello whose identity is already registered is the
same adapter arriving twice: it replaces that entry and closes the replaced stream, so a duplicate
connection collapses rather than lingering as a socket that silently discards work. Each workspace
binds to one browser for its life, physical tab ids resolve as `(browser, physical_id)`, and every
adapter event names the browser that produced it. A crossing with no binding takes an explicit
selection, then reported attention, then the sole connected browser, and otherwise refuses while
naming the candidates rather than choosing between two signed-in contexts. Attention is reported by
adapters that advertise it, never inferred from connection order, and a bound browser that
disconnects is waited for rather than replaced.

Waiting is bounded and lives at the one browser seam (ADR-0196 Decision 12). The port answers
`await_available` for one browser with available, not answering (a registered connection still
installing its page runtime, or one whose liveness lapsed), or absent. A bound workspace waits up
to `RETURN_GRACE`, about seven seconds and capped by the invocation deadline less the adapter
receipt reserve, and is never handed to browser recovery: recovery chooses or starts a browser,
and that workspace's browser is already chosen. A workspace with no binding waits within the
same grace before recovery only while the port reports evidence that a browser is returning, and
`await_any` reads that evidence again on every poll. Each piece lapses on its own: a registered
connection that is not available lasts as long as the connection, an initialized connection that
ended lasts `RETURN_GRACE` from its departure, and a freshly published endpoint lasts
`RELAY_RETURN_WINDOW` (1.5 seconds, two relay retries and the hello) from the publication, which
covers relays reconnecting after a service update. A relay that does reconnect registers inside
that window, and its registration then carries the wait through the page-runtime installation.
Reported attention is not evidence, because it never lapses. Once no evidence remains, recovery
runs at once, so a browser closed long ago or a cold start with no relay keeps nearly the whole
deadline for a launch.

An adapter that advertises end-to-end liveness acknowledges content-free service heartbeats at the
extension shore, independently of browser operations. Relay attachment and adapter availability are
separate facts: a missed liveness window stops new dispatch while leaving the opaque relay and
socket available for a later acknowledgement. Every physical dispatch is followed by its own
probe. At a call's deadline, a probe that has gone unanswered for at least five seconds marks the
adapter not answering, while a shorter deadline proves nothing: the acknowledgement may be queued
behind another call's large receipt on the same ordered stream. Either way the verdict changes
availability for new dispatch only. Calls already in flight keep waiting for their own receipts,
and only the 45-second heartbeat timeout or a real end-of-stream fails a connection's pending
calls. A best-effort cancel that finds the writer busy is skipped rather than closing the
connection. Work waiting for a silent adapter sends it one probe at once instead of waiting up to
20 seconds for the timer. Each lost and resumed acknowledgement is logged to process diagnostics
as `heartbeat_lost` and `heartbeat_resumed`. The browser relay remains unaware of both
frames.

## Orchestrator contexts

### Language

Owns the one 23-tool catalog, JSON schemas, descriptions, examples, MCP annotations, decoder
defaults, validation, typed operations, and mapping of domain outcomes to language
facts. Nothing outside this context defines a tool or an alternate client dialect. Each tool
advertises one portable object schema: every field any branch accepts, each as one schema that
admits every value some branch admits. Cross-field rules, such as which fields belong together,
are taught in descriptions and enforced by the decoder. Test-only self-checks validate every
example and a list of ordinary calls against the advertised schemas, and check that every next
step naming a tool names only fields that tool advertises. It also owns the server instructions every edge sends at initialization, and the record of which
operations accept `timeout_ms`. Work derives each invocation deadline from that record, and a
deadline refusal suggests a longer `timeout_ms` only where it exists and is still below its
maximum.

### Work

Owns invocation ids, lifecycle, deadlines, cancellation, flows, the application executor,
unit-of-work state, uncertainty, and the single completion path.

### Workspace

Owns MCP sessions, controlled tabs, opaque tab and target handles, document generations,
view handles and viewport transforms, selection, ownership, leases, the released-tab pool, stale
detection, and release on disconnect.

A connection binds to one workspace when it is admitted and keeps it until it closes. Nothing
moves a session into another workspace, and a handle from another session is refused before any
browser work (ADR-0196 Decision 8).

Physical tab ids are unique inside one browser only. Ownership checks compare a tab id within the
browser its workspace works in, so Chrome's tab 5 and Edge's tab 5 never refuse each other.

When a workspace is released, the tabs it still holds enter a released-tab pool keyed by browser,
with the governed URL each held at release and the releasing workspace's client label. A held tab,
or one that never landed, does not enter. Release cleanup then asks the browser to close each tab. A
tab becomes reusable only when the person's preserve-tabs setting refuses that close: the extension
has then already dropped the dead workspace's binding and debugger hold, and the tab stays open.
Until the close answers, no session can claim the tab, so a cleanup close never acts on a tab
another session took (ADR-0196 Decision 8). A close that worked, failed, or did not answer in time
removes the tab, so with preserve-tabs off nothing is reused. A tab also leaves the pool when it
closes, when any page commits in it, when a workspace takes it, and when its browser disconnects,
because a restarted browser hands out tab ids again. A close or commit that arrives while cleanup
runs keeps the tab out. Default reuse (`reuse:"domain"`) adopts only from this pool (ADR-0196
Decision 9). The executor claims the best same-host candidate released by a session of the same
client, under the aggregate lock, exact URL first and then lowest id, checks it against the live tab
list, adds it to the workspace, and sends `Navigate` with that exact tab id. The client label must
match because reuse never regroups a tab and the group title is `Ghostlight - <client label>`: a tab
another client released would stay under that client's group, so it is left in the pool and the open
creates a fresh tab. The label only narrows reuse and never grants authority. The pool is in memory
and does not survive a service restart. A person can still type into a preserved released tab
without loading a page, and a later reuse would then replace that page.

A same-document change keeps the document generation. When an action receipt reports an address
in the same origin and no committed document, the workspace updates the tab's URL and title and
expires its views, but target handles stay valid (ADR-0195 Decision 3). A committed document or an
address in another origin starts a new generation.

### Recording

Owns the model-facing record actions, start and disclosure authorization, the choice of
destination, and the output budget. It holds no frames, no encoder, and no capture maintenance
loop. ADR-0108 places recording identity, frames, bounds, deadlines, stop, retention, and erase in
the extension; ADR-0109 places the GIF encode and its delivery there too.

### Governance

Owns authority loading, immutable snapshots, request restriction, capabilities, scheme boundaries,
policy-protected hosts, admission, landing decisions, runtime controls, holds, and content-minimized
audit intent. Operation handlers see only the governance facade.

One refresher publishes the authority. The service starts a single thread that, once a second,
checks each policy file and the managed bootstrap by existence, modified time and size, reads only
what changed, runs the managed authority on its own schedule, and publishes one immutable
snapshot with the views the workbench reads. Every reader takes the published snapshot and reads
no file and no network, so a hand edit to a policy file takes effect within about a second, a
workbench edit at once, and an administrator's HTTPS policy fetch never runs inside a tool call.
Each change of runtime control state reaches every connected adapter once, through one observer
the service installs.

### Browser

Defines the physical primitive port and observed facts: tabs, windows, navigation commits,
readiness, semantic targets, text, screenshots, input metadata, dialogs, recording requests and
receipts, bounded console and network entries, and effect receipts. Action receipts may include the
role and accessible name of the element actually used; the extension never phrases or governs it.
Transport and Chrome details
implement this port outside the context. Diagnostic product defaults, authorization, host filtering,
and model-facing results remain in the orchestrator. Bounded problem/all projection, literal
matching, opaque cursors, and URL sanitization terminate in the extension at the Chromium shore.

### Presentation

Defines content-free feedback events: operation start, target indication, progress, completion,
and denial. Presentation reacts to domain events through a port. Its failures are
recorded but cannot affect authority or completion truth.

### Workbench

Owns the content-minimized desktop read model, bounded global search, high-signal notification
decisions, runtime-control intents, and explicit supported-harness management. It projects the
closed domain-event vocabulary through direct typed reactions and reconstructs bounded terminal
history from the existing durable audit file. The WebView owns only disposable view state.

Readiness is resolved from content-free facts, including governance's own enforcement validity, so
the header cannot read Ready while every governed call fails closed. Runtime-control intents
change governance state first. Governance then sends that state to adapters as a best-effort
notice, and the workbench confirms only the governance state, in words from `language/control.rs`.
Runtime control does not depend on a connected browser. Readiness offers it in every state, so
the WebView disables the header control only while it cannot reach the authority, because its
last snapshot outlives that connection.

Harness registration is an orchestrator-owned local-human capability. Each supported harness has
one explicit config resolver and schema. The resolver applies Windows or Linux environment
precedence before its documented fallback, including `CODEX_HOME` for Codex. Check is read-only.
Install and uninstall are serialized, merge only the `ghostlight` entry, keep unrelated siblings,
preserve JSONC and TOML comments, create a backup, and refuse malformed, unreadable, or
foreign-owned configuration rather than guessing. A separate confirmed Fix action may replace one
parseable foreign entry under Ghostlight's own key. The writer re-checks that state, preserves the
same surrounding document, and uses the same backup and atomic replacement seam. Fix is never an
automatic or aggregate action. The UI exposes no generic filesystem or process operation.

Linux desktop integration is another installer-owned local-human capability. A per-user install
owns one XDG desktop entry and one byte-identical icon. The entry invokes the exact installed
orchestrator with `open`; update and uninstall touch only explicitly owned files. The Debian package
owns the equivalent system entry and does not create a per-user shadow. Browser package provenance
is read-only and typed separately from native-host registration, so a current manifest cannot make
a sandboxed or absent browser look usable.

The loopback server in `workbench/web` is the typed inbound adapter over `WorkbenchFacade`. It
decides access per route in three contexts (Control with the per-start bearer, Watch, Closed) and
projects each part for its audience in the service. The route table is closed, and no route
clears history. One numbered event stream fans out to every screen, and the tray, the window, a
watching screen and the browser toolbar change the runtime through one facade call that publishes
it. File work runs outside the UI event loop. The notification port is best effort and
content-free. WebView, tray and notification failures cannot change domain truth, and a broken
window never stops agent work. Every Ghostlight web app shares one band and a Settings tab with a
rail of sections from `ui/kit`, drawn from the service's controls part; the person's Quit is one of its acts, in Control only.

The page reads its context from the service before it draws and renders what it is told: an act
its context cannot make is shown as state, with one line that leads to the Ghostlight window. It
speaks JSON and one live stream over loopback, in the window and in a tab alike, through one
transport.

`ghostlight-desktop` is the webview over the loopback workbench. It gets the workbench address and
the bearer over the relay's `desktop` role and gives the bearer to the page of that origin only,
which is therefore in the Control context. It registers no custom scheme and no page bridge.

The same stream carries what the service asks of the shell afterwards, all of it frames of the
same-build contract in `intake/desktop.rs` (goldens in `tests/golden/desktop/`). `show` brings the
window forward, opening it first, at a section of the page when it names one (the closed list of
page addresses, which the service checks and the shell only keeps script-safe). `notice` is one
operating-system notification with Ghostlight-authored words, best effort. `act` asks the shell to
open a link or a folder or to pick a file, and the shell answers each with `act_result`. A
notice, a link, a folder and a file dialog are therefore carried out by the shell, in its own
process and under its own application id, not by the service. A reader skips a frame kind it does
not know. One route sends each request to the attached shell. With none attached, a request to show
the window goes to presence and every other request is unavailable.

The shell tells the service two things about itself. `unavailable` says it cannot show what was
asked (`no_display`, `no_webview`, `no_tray`), and `closed` says the person closed the window and
whether the shell stays with its tray. It opens its stream before it initialises the display or
builds anything, so a shell that cannot run can say why before it ends; a `--tray` shell with no
tray host or no web view runtime reports and exits, and a later failure to build the window is
reported while the shell stays with its tray. The shell sends `closed` only for a person's close,
never for an update or a crash.

One shell runs at a time, because the service has one desktop place. A shell that attaches while
another is attached is sent `already_open` in place of the workbench and exits with status 0, and
the attached shell is sent `show`. There is no lock file.

The shell is started in one of three ways, by its argument. With none, a person started it: it
opens the window and, when no service answers, shows Starting, starts the service once with reason
`person` and retries the opening on the relay's backoff for 30 s. A service that answers that it is
shutting down is waited out first. `--tray` means the service started it to show the tray: it opens
no window, tries the opening once and, on no answer, a shutting-down answer or a refusal, exits
with status 0 having started nothing, so a presence start that races a person's Quit cannot undo
it. `--updating` is the window reopening after an update: it shows Updating, retries the opening
for 60 s, starts nothing, and then shows Stopped. The no-start rule of the last two holds until
the shell first attaches. After an attach every shell follows the same rules, and the service's
quit latch refuses a `recover` start after a person's Quit, so no shell can undo a Quit.

The window shows the workbench or one of four states of the shell's own: Starting, Updating,
Restarting, and Stopped ("Ghostlight stopped unexpectedly." with Start and Diagnostics). The
states are one built-in document in `states.rs` that takes the state's words and buttons as
parameters; it runs no script, and a button is a link to a fixed address that the webview refuses
and the shell acts on. A new workbench frame, after a service restart, rebuilds the webview with the
new origin and bearer; the shell never navigates a webview to another origin.

After an attach, a goodbye of quit, uninstall or session end ends the shell. A goodbye of replace is
the update hand-over. The deploy puts the new shell in `bin` before it asks the service to quit, so
the shell compares the identity of its own image (length and last-write time, read once at start)
with the file in `bin`. If they differ and the window is open, it starts `bin`'s shell with
`--updating` and exits, so the window closes and reopens on the new build; if the window is closed
it only exits, and presence starts a `--tray` shell. If they are the same, it shows Updating and
retries for 60 s. A stream that ends with no goodbye shows Restarting and starts the service with
reason `recover` up to three times. Stopped and a spent Restarting keep retrying the opening at
the backoff cap and start nothing; the Start button is a person's act. With a tray, the tooltip
says the state, every item but Open is disabled, Updating and Restarting do not open the window,
and Stopped does. The rules are a pure state machine in `machine.rs`.

The shell registers with the relay's session listener, the one every Ghostlight binary shares. Its
callback arms the fallback exit and posts an event to the loop, and nothing starts once the session
is ending.

## Chokepoints and unit of work

Every direct call and flow enters `ApplicationExecutor::execute`. One invocation owns one
unit of work until exactly one completion is committed.

The synchronous path is:

1. Decode and validate through language.
2. Snapshot configured local and managed authority.
3. Start work and emit `WorkStarted`.
4. Resolve ownership and acquire a workspace lease.
5. Ask the governance facade at the final boundary for each required capability or landing.
6. Dispatch physical effects only through the browser port.
7. Apply observed receipts to the workspace aggregate.
8. Complete through `CompletionGate`, which accepts one terminal outcome only.
9. Release the lease and synchronously deliver audit and presentation reactions.

An operation handler cannot mutate another context directly, call a transport, bypass
governance, or construct a client result around `CompletionGate`.

Flows retain the parent's lease and immutable authority snapshot. Their ordinary
child executors feed one accumulator in `work/composition.rs`; language owns its closed progress,
summary, and recovery in `language/composition.rs`, through `Outcome::CompositionRan`. One safe
projection carries that account into the parent audit record. Child payloads remain separate,
and per-step status/effect metadata survives flow-result omission. Direct operations and child
terminals enter `work/receipt.rs`; each child gets its own completion gate without reacquiring the
parent lease or snapshot. The same safe audit sink records children incrementally, identified by
parent invocation and position. Only the parent settles the lifecycle. Actual child denials have
one receipt; aggregate wrappers do not repeat them or create a denial counter.

`governance/evidence.rs` captures bounded permission checks from the admission evaluator, including
positive grant identities for each evaluated layer. Legacy receipts retain old request restriction
evidence without applying that retired mechanism to new calls.
`language/history.rs` owns the readable explanations. `workbench/history.rs` projects and restores
whole groups, at most 500 with at most 20 children each. Missing receipts stay unconfirmed; only
parent evidence establishes a never-run suffix. A composition-change projection updates the
existing feed without settling the parent or sending extra completion notifications. Expanded
view state remains disposable. Policy previews consider recorded children, not empty wrappers
(ADR-0156).

## Closed domain events

The in-process vocabulary is:

- `WorkStarted`
- `TabCreated`
- `DocumentCommitted`
- `TargetIndicated`
- `WorkProgressed`
- `HoldEntered`
- `WorkBlocked`
- `WorkCompleted`

Events describe completed state changes. They separate audit, presentation, workspace
bookkeeping, and lifecycle reactions. They do not grant authority, dispatch effects, or create
client success. Reactions are direct typed function calls over the closed enum, not a bus.

## Browser primitives

The closed adapter vocabulary is: install the service-owned page runtime, list tabs, focus tab,
atomically open and group a URL, navigate,
traverse history, reload, close tab, read text, read a composed document, inspect, find, screenshot,
screenshot region, describe targets,
activate a locator or physical point, scroll, set zoom, resize a window, hover, fill, type text,
press key, drag, upload supplied bytes, evaluate script, observe condition, inspect dialog, handle
dialog, start and stop screencast capture, observe bounded console and network entries, cancel, and
present. Receipts state whether no effect, a committed effect, or an uncertain effect was observed.
Browser events report document commits, readiness, dialog state, child-tab creation, tab close,
bounded diagnostic entries, control intent, and disconnect.

Runtime installation is a system bootstrap mechanism, not model-facing work. Raw BiDi and raw CDP
requests are not members of the vocabulary. The Chromium adapter may use CDP privately. A future
browser adapter is not an active product until it can execute the complete capability set in the
user's ordinary visible, authenticated, already-running session (ADR-0183).

Ordinary product features compose these primitives only in the orchestrator. A bridge or adapter
change requires a new physical browser capability or a bridge protocol requirement.

The composed semantic layer is the physical page-observation boundary. Inside each injected
http(s) frame, the extension walks rendered elements and text through open shadow roots and assigned
slots while excluding hidden content. A read includes visible form values; credential-class fields (as
classified by the write path) are masked, checkboxes and radios read as checked or unchecked, and
placeholders are never values. Fallback accessible names, find, and text conditions walk the same
composed tree as page text only: no field values, no checked words, no credential markers, and no
editable content, so what an agent typed never reads back as page text. The service worker joins text and rootless trees in stable
frame order under one page-wide character or node ceiling. Explicit article mode probes only the
top document first and uses the composed full-page read when no useful article exists. Closed
shadow roots stay closed, and child-frame origins do not become result or audit fields.

Ordinary textual form fills and rich-editor replacements use one budgeted physical transaction.
The orchestrator passes the adapter's remaining execution budget; the adapter preflights every
field, performs native editing, releases its debugger lease, and verifies the complete batch once.
Semantic setters remain for select, toggle, and file controls. Submission starts only after that
terminal verification and remains an explicit action. A failed native edit refuses without
fabricating input/change events or replacing the existing draft.

Fill and typing decode `user_authorized_credentials`, false by default, in the orchestrator's
language. It acknowledges an explicit user instruction for that request, not independently
verified intent or a policy grant. The executor carries it through a closed physical command field
to browser preflight and the immediate input check. Without it, credential detection returns
guidance before input and creates no workspace state. Grouped fill checks every field before
editing any. Values remain absent from audit, diagnostics, and presentation (ADR-0185).

Pointer geometry follows the same composed surface. Frame-box discovery reaches embeds inside open
roots. Point hit testing descends through open roots, then the service worker follows the embed at
the point through Chromium's parent-frame tree. CDP effects keep top-viewport coordinates while
DOM-local effects keep the deepest frame-local coordinates. Ambiguous embed ownership refuses
instead of guessing.

Screenshot receipts include the exact CSS viewport origin, CSS dimensions, device scale, zoom,
and output scale used for the image. The workspace context turns that physical transform into an
opaque short-lived view handle. The executor resolves image points and rectangles back to CSS
coordinates before sending pointer or region-capture primitives. A region capture validates the
source viewport and returns a new transform, which makes another region capture chainable. A
commit, zoom change, viewport mismatch, or newer screenshot invalidates the old view.

File upload is a cross-boundary capability with explicit owners. The language accepts bounded
absolute paths. The orchestrator validates and reads only those files after governance admission,
then sends bounded names, media types, and bytes through the physical bridge. The extension creates
browser `File` objects for the already selected file input. The relay never reads paths or files.
Credential-like metadata on that file control does not block explicitly requested file upload or
recording save; the physical file-target checks and configured authority remain in force.

Recording follows one-owner semantics, and the owner is the browser. The extension's registry,
which the service addresses by recording id on the wire (a session holds at most one, so no model
call names it), owns Chrome screencast start, frame acknowledgement, identity, compressed frames, fixed
bounds, autonomous stop, frozen retention, erase, the truthful recording indicator, and the
animated GIF encode. The orchestrator sends only start, status, stop, export, and discard requests.
The extension folds byte-identical successive JPEGs into one visual span before retention, so
capture time and compressed bytes are the ordinary limits. While recording, presentation disables
only the perpetual controlled-scope glow; transient action feedback remains available.

An export names one of three destinations and one output budget. Attaching to a page target and
writing through the browser's download mechanism both finish inside Chromium; only a client return
carries bytes out, and only that destination is bounded by the transfer ceiling. Encoding runs in
an offscreen document, because Chrome may evict a service worker mid-encode and because object
URLs do not exist in one. Fidelity is traded to meet a budget, never coverage: dropping a frame
folds its time into the frame before it, so a thinned replay still spans and plays for as long as
the work it recorded. Relays remain opaque. Recording frames never cross a process boundary at
all, and no recording bytes enter extension storage, service storage, logs, audit, or restart
state.

Diagnostics are off until an authorized `browser_diagnose` call enables bounded volatile rings for
one owned tab. The extension captures only the Chrome event facts that must originate there, owns
the bounded problem/all projection, literal matching, opaque cursor, and URL-sanitization
mechanisms, and never writes them to extension storage. The orchestrator owns authorization and
applies cross-origin authority plus final filtering before any model disclosure. Reads are
non-destructive and visually quiet. Diagnostic payloads never enter audit or presentation.

Process diagnostics are a separate, local surface (ADR-0145). When activated, all three
executables append bounded, content-free operational JSONL -- connection lifecycle, demand-start
outcomes, negotiation, operation boundaries -- into one directory in the Ghostlight home.
Activation is layered and eventual: `GHOSTLIGHT_DIAGNOSTICS_DIR` pins a process on at birth, and
a presence-only `diagnostics.on` marker in the home is applied live by an OS watch
with a 2-second safety-net re-check, whichever fires first. An OS person toggles it by hand,
with `ghostlight diagnostics on|off`, from the extension popup through the runtime-control path,
or from Settings > Diagnostics, which also opens the folder through a native-process act.
Records carry no URLs, page content, payloads, or credentials, so the folder is safe to share;
the schema is pinned by test, retention is automatic, and the governance audit remains the only
decision record.

## Governance

No policy configured means all four independent RAWX capabilities are open for HTTP(S), including
localhost, loopback, and link-local destinations. Host restrictions belong to policy
(ADR-0155). Non-HTTP(S) schemes, including browser-internal and extension pages, remain outside
the browser automation contract.

`GHOSTLIGHT_POLICY_FILE` selects an optional strict schema-4 local policy. Ordered grants combine
host allow and deny patterns with complete independent RAWX sets. Exact hosts outrank longer
suffix wildcards, which outrank `*`; an exact tie denies. Managed and local policy intersect. Sacred destinations compose by union. False tab-close and target-name
settings are monotonic. Every denial is enforced. A malformed cold source fails closed; an invalid
replacement keeps the last valid authority for future snapshots. A schema-3 file is invalid. The
user policy Ghostlight owns moves to schema 4 when it loads, with a `.schema3.bak` copy, if the move
keeps its meaning; administrator files are never rewritten.

Signed managed policy is opt-in through `%PROGRAMDATA%\Ghostlight\managed.json` on Windows or
`/etc/ghostlight/managed.json` on Linux. With no bootstrap, no policy network work occurs. A
bootstrap names a local file or HTTPS source plus the organization's public verification key and
may add a bearer token, CA pin, and polling interval. Ed25519 is required; when ML-DSA-65 is
configured, both signatures must verify. Monotonic publish sequence blocks rollback. Verified
bundles are cached and verified again on read. Bad or unreachable updates retain last-known-good;
a configured cold start without a valid source or cache fails closed. Signed policy has no time
expiry. The workbench Policy Passport surfaces content-minimized provenance and signed
organization contacts without policy rules or credentials.

Runtime control is one in-memory state with three values: active, held, and ended. Only a person's
control changes it, and no file or variable sets it. It never expands the immutable authority
snapshot.

One immutable configured authority snapshot is stored in the unit of work. Runtime hold,
end-session, and cancellation controls are checked immediately before browser dispatch and on
browser events.

Navigation prepares commit observation before dispatch. Every committed URL is checked before
content or readiness is accepted. A denied new-tab landing is closed only when tab-close authority
and the adapter's local physical interlock both permit that compensation. Otherwise it remains
visible and the result reports that compensation did not occur. A denied landing in an existing
tab enters hold and returns a committed, non-repeat-safe blocked outcome. Redirects receive the
same check.

Audit records include time, invocation id, workspace id, tool, complete RAWX requirement set,
authority version, managed sequence, mode, deciding tier, grant, rule, denial id, decision,
status, effect class, and reason code. They exclude request and page payloads except the governed
host and optional bounded target name described by the active privacy setting.

Every terminal supplies a language-owned `AuditProjection` alongside its client result. Audit
accepts this projection rather than a raw summary or result JSON. Refusal metadata is a closed
type; arbitrary browser error text stays in the permitted client result. The workbench renders
the retained sentence. Historical failure payloads are discarded when read, without editing the
historical file or claiming to sanitize its old summary text (ADR-0103 H1 amendment).

## Failure and recovery

Disconnect before a physical request has no effect. Disconnect after dispatch without a decisive
receipt yields `unknown`, never failure or success. Cancellation follows the same boundary, and
before dispatch it is reported as a cancellation, never as a disconnection. The other
before-dispatch facts keep their own names: a browser that is attached but not answering, a
browser that is not connected, a bound browser that is gone while a different one is connected,
and a connection busy with other work. A bridge contract failure has its own name too, but it
is not a before-dispatch fact: a receipt in the wrong shape arrives after dispatch, so its
sentence makes no claim about what was sent.
Stale tabs and targets fail before dispatch. Sequence failure reports completed step count and
never replays completed steps. Recovery suggestions come from typed reason codes and effect class,
not page content.

An unanswered post-dispatch liveness probe does not revise that invocation's unknown effect. Once
it has gone unanswered for the verdict window, it changes only the availability fact used by later
invocations. They wait the return grace, probing once, and stop before dispatch only if the
extension neither acknowledges nor reconnects within it.

## Extension product architecture

The extension has four small responsibilities with explicit boundaries:

1. The service worker owns native connection lifecycle, physical primitive dispatch, persistent
   adapter identity, and an opaque map from physical tabs to service-supplied workspace ids.
2. The page adapter owns document-local semantic discovery, open-shadow-tree traversal, target
   locators, actionability checks, physical DOM effects, observation, and presentation mounting.
3. The popup reports content-free connection, control, and work state. It sends human control
   intent but cannot decide policy or synthesize model results.
4. The options page owns adapter-local feedback, caption, diagnostic, and physical tab-preservation
   preferences only. A physical preference may refuse a primitive but can never grant authority.

Workspace tab grouping is browser mechanism. `OpenTab` carries the governed URL, the owning
workspace already present on every browser request, and the presentation-only exact title
`Ghostlight - <client label>`. The adapter resolves placement before opening: it reuses one
browser-wide exact-title group wherever that group currently resides, otherwise reuses a window
containing another Ghostlight group, otherwise creates a dedicated normal window containing the
requested URL. Destination resolution is serialized so concurrent opens cannot create duplicate
groups. The first tab for a workspace brings the Ghostlight window into view; later tabs do not
repeatedly steal window focus. The adapter colors the group blue, records the opaque workspace
association, and opens without an externally visible blank-tab step. Group ids are restart-local
hints and exact-title discovery repairs stale hints. The label never routes or grants authority. A
newly opened tab is not adopted through its opener: that relationship cannot distinguish an
agent popup from a human opening a link. Explicit executor work establishes ownership. Moving a
group or tab between windows does not change ownership or trigger automatic regrouping.

The adapter never chooses a tab to reuse. The executor sends every `OpenTab` with `reuse` set to
`never`, so an adapter that still carries its own same-host reuse ladder never runs it, and a
reused tab arrives as an ordinary `Navigate` naming the exact tab the service chose. A reused tab
is not regrouped, and it does not need to be: only a session of the client that released it can
reuse it, so it already sits in that client's `Ghostlight - <client label>` group, or wherever the
person moved it.

Document-commit events silently update cached destinations and invalidate agent references. They
never authorize browsing, hold tabs, write action history, or produce presentation. Even a
correlated event is not proof of agent authorship. The executor governs its requested operations
and returned landings at the existing boundaries. ADR-0164 owns this distinction.

Controlled debugger attachments emulate a focused page while the connected, compatible service
reports active runtime control (ADR-0168). This keeps focus-triggered page refreshes from
interrupting automation between calls. The existing debugger lifecycle owns per-tab serialization,
retention, and restoration. Pause and disconnection remove the override; stop and local
debugger release also detach. A released workspace loses debugger retention even when its tab is
preserved. Passive browser events never acquire this custody. The scope can outlive a model turn,
and unsaved values are not guaranteed to survive normal page lifecycle after release.
Extension reload clears session storage. The next explicit tab-scoped service request repairs
the opaque workspace association from that authoritative request before retaining its debugger;
it does not regroup or move the tab. Inventory and released-tab cleanup do not acquire custody.

The service worker persists only the installation id, adapter-local preferences, enough opaque
topology to recover after worker suspension, a bounded content-free operation disposition
journal, and opt-in structural developer diagnostics (ADR-0145). The developer flag enables
connection evidence plus a separate 400-record form trace in already-controlled top documents.
Form observation stops after ten minutes or when disabled/released, examines at most 100 ordinary
controls, and retains only emptiness/node counts and coalesced input/focus/lifecycle flags. It
never patches page behavior or turns human browsing into service work. The Options export
includes both local rings. URLs, page content, form values, scripts, file data, screenshots, receipts, and policy are
never stored. Popup status is derived from live native connection state, relay availability, and
service-published control state. The service's negotiated heartbeat supplies the end-to-end
availability fact and ordinary local traffic that keeps the idle adapter shore observable. The
native relay owns ordinary backend reconnection; the alarm-backed extension loop is the fallback
when the native host itself ends. Both paths repeat the same adapter identity and capability
declaration.

Model-driven close is dual gated. The orchestrator first admits the action capability and the
monotonic tab-close policy constraint. The extension then checks its default-on preserve-tabs
preference immediately before `chrome.tabs.remove`. A local refusal uses the existing content-free
adapter error channel and is rendered by the orchestrator as a blocked no-effect result. The local
preference is not advertised as a physical capability, does not renegotiate the adapter, and
cannot override service authority. Direct human closure through Chromium remains outside this
model-driven path.

The operation journal records only the current service epoch, correlation id, and one of accepted,
dispatched, completed, failed, or uncertain. Duplicate accepted work and known no-effect failures
may resume. Dispatched, completed-without-an-in-memory-receipt, and uncertain work report unknown
instead of repeating. Terminal records remain until the service acknowledges receipt, which keeps
normal operation bounded without evicting an unacknowledged effect.

The script evaluator parses source locally with pinned, packaged Acorn before any page evaluation.
It uses an async-body grammar for REPL syntax, verifies the parsed wrapper's exact body boundaries,
and selects the explicitly awaited function form only for returns outside nested functions.
Other scripts retain their original REPL source and scope. Local preparation failure is known
to precede evaluation; after dispatch, exceptions and transport failures retain uncertainty.
Exception descriptions and classes cannot initiate a second execution. The existing typed error
receipt carries that uncertainty through the orchestrator's single completion path.

Presentation is a document-local state machine. Signals carry only opaque ids, a closed event
kind, a closed Ghostlight activity treatment, fixed Ghostlight-authored phase and detail text,
and an optional physical locator. One small renderer implements the established sky-blue visual
vocabulary and owns its fixed palette, shapes, timing, easing, and reduced-motion treatments. It
has no broker, policy, catalog, or orchestration. Controlled scope and guardrail feedback remain
visible independently of decorative preferences. Screenshot capture hides the layer before pixels
are read. A new document remounts clean state; a terminal signal clears transient effects. A
bounded browser-local queue retains unseen denial signals for the relevant controlled tab without
changing focus, coalesces repeated notices per tab, and expires them after ten minutes. The toolbar
badge marks an unseen denial until the tab becomes visible and the five-second ribbon renders.

The manifest is part of the architecture contract. It preserves the established product name,
description, development key, host name, toolbar title, shortcut description, and byte-identical
icon assets. It connects the established toolbar and options experiences while including only
permissions required by the responsibilities above. Extension source remains framework-free
because its state and UI are small and Chrome-native.

## Unpacked extension continuity

For the narrow compatibility question consulted from the archived public development instructions:
Chromium loads `extension/` directly with id `cjcmhepmagomefjggkcohdbfemacojoa`, connects through
`org.sylin.ghostlight`, and reloads explicitly after changes. Repo-built shores remain under
`target/release`. No prior implementation code was reused.

## Explicit human control and final dispatch (ADR-0185)

Governance owns configured authority and explicit human runtime controls. Policy denials and
credential-input guidance create no automatic hold, rolling counters, incident ids,
or session recovery journey. Any later permitted request remains available under the same policy
and controls.

The page renderer creates only passive messages and animations, never buttons, focusable controls,
or a pointer-intercepting layer. That property holds when CSS fails. Runtime-state changes alone do not create a page dialog.
Interactive runtime control belongs in the Ghostlight window.

Obsolete request fields fail during decoding. Permission receipts retain the final decision and actual evaluated layers. The executor supplies a live
admission callback to the common browser port; the relay invokes it after writer acquisition and
before transmission. The same check guards compensating close. Control authority is not locked
while waiting for a browser receipt. Already dispatched work retains acknowledged and uncertain
effect facts, including acknowledged actions whose separate postcondition check was interrupted.

Completed landings are still checked against policy after dispatch. A later runtime control
change does not create a permanent policy hold on an otherwise permitted tab; the next command
checks the live control again. Recovery tests prove the same tab remains usable after Resume.

## Document authority and coverage (ADR-0158)

The common executor browser boundary obtains a bounded document inventory and exact request
subjects before extraction. Governance admits actual documents through the immutable snapshot;
the negotiated `document_scope` mechanism carries exact identities and permitted scope. The
extension uses Chrome document-targeted messaging and supplies physical coverage. Its locator
mechanism binds frame routing to document identity. Neither relay interprets policy.

Language owns the content-free coverage qualification. Completion adds bounded coverage to
model results and audit. Volatile workbench details hold excluded host names for the human only.
Screenshots use binary `browser.screenshots.enabled` permission (ADR-0194). Existing tab Read
and custody checks remain. Allowed images include visible embedded content without inventory or
masking. A false value in either layer refuses all four branches before dispatch. Screenshot coordinates confer no extra semantic or action authority.
Restricted recordings stop when their admitted document set changes. Every replay destination
rechecks recorded embedded sources; incomplete provenance cannot pass restricted disclosure.

## Audit availability (ADR-0159)

`audit.rs` owns one shared serialized recorder for completion and browser-event receipts. It
replaces the workbench's projecting-sink decorator. It retains bounded health/gap state, writes
through the existing safe audit record port, and publishes storage-qualified history and health
through the workbench projection. The JSONL sink reopens the destination and synchronizes writes.
Recovery probes run under the service lifetime, at most once per five seconds while unavailable.
There is no receipt backlog, delayed backfill, or browser-action replay.

Governance owns the monotonic `audit.availability` setting and attributes its strict refusal to
the immutable policy layer. The executor reads current storage health before browser preparation
and at the final dispatch callback after writer queuing. Already-admitted effects remain truthful.
Human controls, diagnostics, and policy explanation remain reachable. Storage health does not
mutate human runtime control. The extension and the edge remain free
of audit policy and health semantics.

Startup tolerates unreadable history or a failed destination, with explicit human health.
History and policy simulation use a bounded line reader, preserve valid adjacent receipts, and
report malformed/oversized omissions. Recovery markers are separate content-free JSONL entries,
not operation receipts. A failed sync can leave bytes behind, and a crash during an outage can lose
volatile gap counts; no atomic website/disk coupling or tamper-proof guarantee is made.

## Bounded page preparation (ADR-0195)

Work supplies one optional settlement budget through the negotiated document scope.
The adapter uses the published bounded observation mechanism before ordinary page work,
checking cancellation before and after that read. Later dispatch retains executor custody.
The page runtime shares the remaining budget across useful-content and form preparation.
Expiration permits current valid work; it never authorizes an unknown target or a replay.
Targeted validation follows the actual documents and ancestry, not unrelated active frames.
Broad restricted operations retain complete-tree checks where their promise requires them.
Composite waits preserve their primary condition independently of optional visual readiness.
