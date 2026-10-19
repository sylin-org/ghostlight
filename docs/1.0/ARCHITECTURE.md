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
  console on Windows 11 24H2 and later. The service image is a console-subsystem program too,
  with the same manifest (the build script reads the edge's file by path), so a typed command
  writes to the terminal it runs in and receives Ctrl-C itself, and a start with no console to
  join shows none. Every start Ghostlight itself makes (edge, Chrome, agents, presence, the
  shell, open) goes through the relay's `start_service`, which creates the process with no window
  and detached from any console. On Windows 11 24H2 and later no direct start of the image shows a
  console either. Before 24H2 the Uninstall entry, which runs the image directly, prints its
  outcome in a console, and ADR-0199 Decision 4 accepts a brief flash on Windows 10.
  On Windows each executable names itself in Task Manager through its own version resource:
  "Ghostlight service" for the service, "Ghostlight" for the window and "Ghostlight connection"
  for the edge (one per MCP client or browser).
- `extension` implements the full Chromium physical mechanism set, page-runtime installation,
  browser topology, content-free feedback, and the local toolbar/options experience.

Only process, lifecycle, and trust boundaries justify these crates. Product contexts remain
modules inside the orchestrator. The dependency direction points inward to the orchestrator's
ports. The orchestrator does not depend on MCP or Chromium APIs.

The `ghostlight-service` executable links no UI toolkit and serves with no display. The shell,
`ghostlight-desktop`, draws the window, the tray and the notices, and presence (below) keeps it
running whenever the desktop can show it. A computer with no display, no web view runtime or no
tray still gets a service, and `status` and `doctor` say why there is no window.

## Local lifecycle

There is one Ghostlight home per OS user, `~/.ghostlight`, and every path derives from it
(ADR-0197 Decision 10): the installed executables `ghostlight` and `ghostlight-service` in `bin`, `run` (the service lock and, on Linux,
the relay socket), the state
directory `state` (audit log, MCP client locations, the setup record `setup.json`, the
last-browser record `last-browser.json`, the agent icon cache `icons`), the policy
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
changes, the edge in `bin` is missing or is not the one the record names, a development install
carries an edge that differs from the installed one (the development edge follows its build), or
`bin` still holds an older layout that kept its connectors there, where the service image had the
edge's name and two connector images sat beside it; the installer stops those exact images and deletes the connector
files. It also stops what runs from each 1.x `bin/v<version>` directory, moves the audit log the
directory holds into `state` under a name that says which version it came from, and removes the
directory (`install/version_directories.rs`); a log that cannot be moved keeps its directory.
Every install, migrating or not, also removes the folders 1.x left that nothing reads: its policy
folder (`XDG_STATE_HOME/ghostlight` on Linux, `%LOCALAPPDATA%\Ghostlight` on Windows) once it is
empty, and the 1.x window's web view data (`XDG_DATA_HOME/org.sylin.ghostlight` on Linux,
`%LOCALAPPDATA%\org.sylin.ghostlight` on Windows, where WebView2 keeps it in `EBWebView`) when it
holds nothing but web view data; a folder that holds anything else is left. An install inside a
Windows app container leaves both for the next install outside it, because its removals below
`%LOCALAPPDATA%` would reach only the container's private view.
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
`uninstall` removes those two again, each only when it is provably this home's: the shortcut when it
starts a program in `bin` (or, when no target can be read from it, carries the AppUserModelID), the
key when its `UninstallString` or `InstallLocation` points into the home. An entry that names anything else is left and named, and a
second uninstall finds both gone (ADR-0199 Decision 20).
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
command that finishes them from a regular terminal, `<home>/bin/ghostlight.exe install`; `doctor` says it cannot see what other apps see.
Inspection there offers no Install, Fix or Uninstall for a client whose configuration the container
keeps private, and its detail says why, so neither the setup view nor Integrations offers a change
that would be refused, and Set up everything leaves that client out. A dry run or a removal there
names each such client whose entry it leaves in place, with the command that changes it from a
regular terminal. An install
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
a control status with an instance other than the one it replaced. A start that fails says why: a
service whose start fails writes the reason to `run/start-failure` (its own output goes nowhere)
and to diagnostics, and removes the record once it serves; the installer stops waiting as soon as
no process runs the service image, and names the spawn error, the recorded reason, or the 30 s
timeout. The quit is a control `quit` for
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
lock held touches nothing and waits for it: an `update` start up to 30 s, so the service it
replaces can finish leaving, and any other start up to 5 s, so it outlasts a service that is
leaving on idle, which keeps the lock and the endpoint until its teardown ends and tells no one.
When the wait runs out the start exits 0, because the service that holds the lock answers the
starter's own connection. A service that is not accepting openings within 30 s of taking the lock exits
with a nonzero status and a diagnostics event, so a stalled start never blocks the next one. The
service binds the endpoint as soon as it has started, with the session listener and the quit signal
already installed (ADR-0199 Decision 5). The timed discovery of ADR-0166, which waited for a
desktop to be ready before the endpoint existed, is gone with the desktop.

A person's Quit (the tray, the workbench, or control `quit` with reason `quit`) writes the latch
`state/quit` before the service finishes in-flight work and exits. While it exists a `browser` or
`recover` start exits 0 with a diagnostics event before it takes the lock, and again if the
latch was set while it waited for the lock; any other start deletes it after taking the lock and
serves. So after Quit a browser restart does not bring
Ghostlight back; the next agent request, a command that needs it, or a person does. Before it
closes its streams the service sends a goodbye on each: `quit`, `replace` (the installer's quit)
or `uninstall`.

A service end decides its sessions' tabs once (ADR-0203 Decision 3). After the drain, the quit
takes over every session before the first goodbye, and from then on a closing session releases
nothing itself. Under one deadline (`END_LIMIT`, 5 s from the end of the drain) it then hands each
MCP stream its resume token, sends the MCP and shell goodbyes, releases the workspaces it ends,
and sends the browser goodbye last. A workspace with a call still running has its queue stopped
for the quit first, and is released only if that call finishes inside the deadline. Which
workspaces end depends on the reason. On `replace`, an MCP workspace whose connection is still
live stays bound for its edge's resume, and one whose client left during the drain is released.
On `quit`, `uninstall` or a reason this build does not know, every MCP workspace is released and
its token carries no tabs, so a resumed session reads honest stale handles. A command-line
workspace is released at every end, because nothing can resume it. Release follows preserve-tabs,
as at any release. The end of the sign-in session and the idle exit send the browser nothing:
they forget every binding and the released pool in memory, because the session end has a 2 s
fallback exit and an idle exit has no browser stream open.

The MCP relay answers its outstanding requests as cut off on every goodbye,
except that after `replace` it sends a cut-off handshake or catalog listing to the next service.
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
OpenSSH session). The control `status` answer carries the reason as `window`. A service that
reaches no display, such as one an agent started over SSH, can still get a window from the person:
the Applications entry starts the shell in their session, and so does `ghostlight open` when the
service answers `no_display` and the command's own session names a display. With no display there
either, `open` says why at once.

The service owns the tray model and the shell draws it. The model is a state (ready, working, paused
or attention, from the same readiness the band shows; a service with no browser connected looks
ready), a tooltip (the band's word for the state, then the sentence that counts connections, and "No
browser connected" when none is) and four entries: Open and Quit always, Pause while work is active,
and a fourth entry that is enabled as Resume while work is paused and as Start a new session once
the session ended, and disabled while work is active. Each entry is enabled exactly when the
runtime state offers its act (`governance::offers`), the table the band and Settings read too. The first workbench frame carries it and a `tray_model`
frame follows each change. The shell draws the state as a small dot at run time. On a dark taskbar
the dot goes over the original icon bytes. On a light Windows taskbar it goes over the light-taskbar
mascot in `crates/desktop/assets`: the same art without its sparkles, on a soft outline in the
window's night colour, with the face darkened at 16 pixels so it still reads. The ring around the
dot is drawn in the taskbar's ground. On Windows the shell draws the 16 pixel art at the 16 pixel
small-icon size and the 32 pixel art above it; a Linux StatusNotifier host picks its own size, so it
gets the 32 pixel art.

The tray and the service do not depend on a permanent window. Closing the window destroys only the
disposable workbench view in the shell, which stays with its icon; the shell tells the service
(`closed`), and the first time, the service explains the icon in one notice, once per home
(`state/tray-hint`). Windows Open focuses or restores the existing window and rebuilds it when
absent. Wayland cannot report or unset a client's minimized state, so Linux Open coalesces,
destroys any existing view, waits for the destroyed event, and rebuilds from the canonical
configuration. On Linux, abnormal WebKit renderer termination discards that exact window after the
signal callback; the next explicit Open creates a fresh WebView without an automatic crash loop.
A person's Quit and the end of the session are not the only ways out. The service counts the
open streams of every role (`mcp`, `browser`, `control` and `desktop`), and an attached shell is
one. When the count reaches zero it arms an idle limit of 10 minutes, `idle::IDLE_EXIT`, and any
accepted opening disarms it; a control call such as `status` is an opening, so it restarts the
span. If the limit passes with no stream open and the executor holds no work, the service stops
through the quit path without a goodbye, since no stream is open, and without writing the latch:
the next edge, command or person starts it again for its own reason, and a browser start after an
idle exit is not refused. If the executor still holds work, the limit is armed again for another
10 minutes. The idle stop is the service's own reason, not relay vocabulary; it is recorded as the
`idle_exit` process-diagnostics event. Presence launches nothing for it, because a quit has begun.
With presence a shell is attached on every desktop that can show one, so the idle exit fires only
on a headless service, such as one an agent started over SSH or in CI. Pause and Stop are service
state the shell shows; they do not affect it. A service that has streams ends only by a person's
Quit (which holds, see above), the end of the session, a replacement or an uninstall.
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
refuse and nothing is spawned; browser recovery launches no browser and an install shows no window.

On the final signal a process arms a 2 s fallback exit first. The service then says
`goodbye{session_end}` on every stream, closes the shell's streams and signals its own quit,
without waiting for work in flight and without setting the quit latch. The shell ends on that
goodbye. An edge, which may find the service already gone, exits 0 on its own signal and on a
`session_end` goodbye, and a stream that drops while the session is ending never leads to a
start. A one-shot command leaves at once.

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
the peer checks and backend availability separately from the adapter protocol. The edge holds the
extension's first frame, the adapter hello, and sends it first once a service accepts the stream.
When no service accepts the stream it writes `backend_unavailable` to the browser once and drops what
the browser sends. When a service answers again it exits (`Ended::ServiceReturned`), so the
extension reconnects on a new port with a fresh hello. It never interprets or replays a physical
operation.

The adapter is a policy-free browser execution engine, not merely a collection of API calls. It
owns browser-local operation state, safe mechanism retries, duplicate-operation suppression,
physical observation, factual receipts, adapter resynchronization, and fail-safe local human
control. It does not own model-facing tools, workspace authority, governance, product recovery, or
journey composition. A physical effect is repeated only when the browser engine can prove that it
was not dispatched or that the physical sub-step is idempotent. Insufficient evidence produces an
unknown disposition.

Compatibility uses three axes, all additive (ADR-0199 Decision 8): the relay protocol, the
adapter protocol, and the MCP revision, which only the service negotiates. The shell has no axis
of its own. On the adapter axis the hello names the adapter protocol major, and that is the whole
negotiation: no capability list crosses, and nothing parses an implementation version. Protocol 4
stays compatible by staying additive. Within the major nothing is removed or renamed, every reader
ignores a field it does not know, each closed code the service reads has a fallback arm, and an
extension asked for a primitive it does not know answers `unsupported` without dispatching it.

Each adapter has one closed executable handler table. Platform is identity and presentation
information, never a dispatch key. A vendor protocol such as CDP is private adapter technique, not
a second wire vocabulary.

The browser adapter is a service component. `browser/adapter/mod.rs` names a closed `Family`
(Chromium) and re-exports the service's internal browser work vocabulary from
`browser/adapter/work.rs`: `BrowserCommand` in, `BrowserOutcome` back, `BrowserEvent` between
calls. Chromium speaks one wire, adapter protocol 4 (`adapter/chromium/protocol4`, specified in
`docs/design/adapter-protocol-4.md`). Its translator turns each unit of work into frozen
primitives and back. Any first frame that is not a protocol-4 hello hears `adapter_incompatible`
with a sentence telling the person to update the extension, and its socket closes. An extension
that hears it shows that sentence under "Update Ghostlight in Browser." in its popup, options and
setup pages, and no longer reconnects the moment the native host exits: it keeps only its delayed
3 s retry until a hello is accepted, so a service put back to a compatible build heals the link
with no one acting.

Page-local observation and presentation ship as one service-owned source assembled in
`adapter/chromium/protocol4/assets.rs` from `assets/world.js` and the files its include marker
names. The process assembles and hashes it once and sends it in every `hello_accepted`; the
extension installs it into the isolated world `ghostlight` of every frame. Node tests for those
files sit under `assets/tests` and run with `node --test`. The golden fixtures in
`tests/golden/protocol4` pin the asset id, the wire and the world messages for both the service
and the extension.

## Stable seams

The MCP edge passes JSON-RPC lines between the client and the service unchanged
(`crates/edge/src/forward.rs`: the edge authors no words). It looks inside a line only to track
outstanding request ids per direction, to answer `ping`, and to answer outstanding requests with
the failure templates the service handed over when the service is cut off. A lone handshake or
catalog listing cut off by a lost service is sent to the next service once instead, and a line
whose write failed is held, because neither can have taken effect. The MCP session engine
lives in the service (`intake/mcp.rs`): it negotiates the revision, serves `tools/list` and
`tools/call`, and renders each result as the lean model view in `structuredContent`, with the
authored summary and safe next steps in the ordinary text block; a session on an MCP revision
before 2025-06-18 gets the same lean JSON in that block too. The view only omits fields at their
default. It never branches on a tool name, and the canonical envelope stays in the service for
audit, history and flow references. This keeps the edge free of product vocabulary and of MCP
revision logic. Content-free readiness inspection is a `control` opening that returns before
workspace creation and never demand-starts the authority or writes audit.

One agent gets one set of tools (`service/toolset.rs`, ADR-0203, amendment of 2026-10-19). An
agent can reach Ghostlight through two server entries at once, such as Claude Code with both the
plugin and the installer's entry, and a client never removes a tool it was offered. So an MCP
connection whose source is the running app (process id and start time) of the agent its client
claims to be, when a connection of that same process and agent already offers the tools, stands
by: its `initialize` instructions say Ghostlight is already connected to this agent, and its
catalog is empty. Its calls are still served. When the connection that offers the tools ends, and
the service is not ending, the first that stands by is promoted and sends
`notifications/tools/list_changed`. Two processes, a source that could not be read, a source that
is not the claimed agent's own app (an editor whose terminal runs the agent) and the command line
never stand by. A session a resume token continues keeps the role its client saw, whatever order
the edges reconnect in: the token says whether it stood by, and a promotion writes a new one.

The browser side is adapter protocol 4, carried opaquely by the browser relay. Its messages are
hello, primitive request, receipt, browser event, cancel, and presentation. It carries a closed
primitive vocabulary but no model-facing tool names, product defaults, governance decisions,
recovery text, or result envelopes.

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
never borrows a current session's identity. A connection's attribution is minted when the service
answers its opening, so its time is the accept time. Once per connection, right after that answer,
the service reads the ancestors of its peer process from the operating system and keeps the app it
saw start the connection: the first ancestor that lies in a catalog agent's installed app, else the
first that is not a shell, interpreter or other wrapper, as an agent id and an executable file name
(ADR-0161, amendment of 2026-10-18; ADR-0202 Decision 3). The opening's answer never waits on the
read; the `initialize` answer does, because the read decides the session's instructions (ADR-0203,
amendment of 2026-10-19). Nothing a client sends takes part. The window sets the client's claim
beside that source. Receipts keep the source and, when the claim names a catalog agent, that
agent's catalog id as a labelled claim, never the client's words. One function in
`language::provenance` names the agent and composes the explanation, live and from history alike:
the claim names the agent, else the source's catalog agent, else Unknown agent, and "X, in Y." is
said only when the catalog knows X runs inside Y (ADR-0202 Decisions 14 and 14a). A record read
back from an earlier run that kept neither a claimed agent nor a source reads "Earlier session",
and its hover says only that no agent name was kept. A client's own words are quoted only where they say more than the agent's name. Provenance
grants no authority. Its channel decides one thing: a policy's channel switch refuses every request
on a channel it turns off (ADR-0202 Decision 22a). Audit lines with
extra attribution fields still read; the extra fields are ignored.

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

The browser hello also carries a persistent opaque adapter installation id, an optional life, an
optional bounded product name, the platform, engine and adapter versions, and a reported attention
fact. They identify and describe one local extension installation for diagnostics, compatibility,
reconnect continuity, and routing. The installation id is the browser's identity: it never grants
authority and never replaces workspace ownership, but it is the key the service routes on and the
opaque handle a model may name. The life (`life_` and 32 hex digits) is minted by the worker into
`chrome.storage.session` when none is there. Chrome keeps that storage across a worker restart and
a native-host restart and clears it on a browser restart and on an extension reload, update or
disable, so one life names one browser run, in which tab ids are never reused. The port keeps the
life each connection's hello named, and a connection that ends or is replaced reports it in its
`Disconnected` event. A hello with no readable life is accepted and names none, so a different or
missing life proves nothing (ADR-0203 Decision 1). Human toolbar actions
cross the adapter protocol as a closed control-intent vocabulary. The service applies them to its
authoritative runtime controls and publishes the resulting content-free control state back to the
adapter. Ended state is terminal for hold and resume; only explicit start-new-session intent creates
a new active runtime session.

Browsers are plural (ADR-0114). The service holds one connection per browser identity, so several
browsers are connected and worked in at once. A hello whose identity is already registered is the
same adapter arriving twice: it replaces that entry and closes the replaced stream, so a duplicate
connection collapses rather than lingering as a socket that silently discards work. Before the
service registers the new connection it reports `Disconnected` for the one it replaces, whose own
end is never seen, so the replaced connection's bindings go pending (see Workspace). Each workspace
binds to one browser, physical tab ids resolve as `(browser, physical_id)`, and every adapter event
names the browser that produced it. The binding is taken when the workspace's first tab binds: an
open, an adopt, or a resumed tab the browser confirmed (ADR-0203 Decision 2.2). A crossing that
binds no tab never binds a browser, so `browser_tabs` list and `browser_record` status, stop and
discard leave the choice open, and a list in a session that holds no tab answers at once from what
the service knows, without asking, waiting for or starting a browser. A bound workspace is never
rebound; a tab of another browser is refused as `browser_pinned`. Until a tab binds, one call keeps
the first browser it chose for all its crossings. A crossing with no binding takes an explicit
selection, then reported attention, then the sole connected browser, and otherwise refuses while
naming the candidates rather than choosing between two signed-in contexts. Attention is reported by
adapters that advertise it, never inferred from connection order, and a bound browser that
disconnects is waited for rather than replaced.

Waiting is bounded and lives at the one browser seam (ADR-0196 Decision 12). The port answers
`await_available` for one browser with available, not answering (a registered connection still
installing its page runtime, or one whose liveness lapsed), or absent. A bound workspace waits up
to `RETURN_GRACE`, about seven seconds and capped by the invocation deadline less the adapter
receipt reserve. Recovery never chooses a browser for it: that workspace's browser is already
chosen. A workspace with no binding waits within the
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

Under `on_demand`, recovery starts the only installed browser, or, among several, the browser
that last connected to Ghostlight (ADR-0149 amendment, 2026-10-17). The port learns that browser
when it accepts a hello whose reported name maps to exactly one supported browser, and writes its
native-host id, and nothing else, to `state/last-browser.json`. With several installed browsers
and no usable memory, recovery repairs the stale registrations it owns and leaves startup to the
person. `manual` never launches.

A bound workspace whose browser stays gone gets its own browser started again, under `on_demand`
only (ADR-0196 Decision 12 amendment, 2026-10-17). The port keeps, in memory for the service's
life, the installed browser each browser identity reported when it connected
(`installed_browser_of`). When the bound browser maps to one, its wait ends as soon as no browser
is evidently returning (`await_returning`), and `BrowserRecovery::request_browser` runs a flight
scoped to that installed browser: registration checks, owned repair, one launch and the handshake
wait. It answers ready when a connection of that browser arrives, or at once when one is already
connected in another profile, since starting it again would only open a window there. The
executor then waits within the call's deadline for the bound identity itself, since another
profile may connect first, and refuses with `browser_wrong_profile` only when the deadline ends
without it. With no mapping, as after a service restart, the bound workspace waits the whole
grace and keeps its refusals. Every flight, unbound or scoped, shares one launch guard per
installed browser: while a start Ghostlight made is younger than `LAUNCH_HANDSHAKE_WINDOW` (30
seconds, the longest call timeout) and no connection of that browser has arrived since, no flight
starts it again, and each waits for that start's connection as if it had made it.

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

A request has one id, an `InvocationId` (`invocation_` and 32 hex digits). The MCP intake mints
it for each tool call and keeps the client's own id beside it, so a cancel names the same id. The
session queue, the domain events, the result the client receives, the audit line and the
diagnostics all carry that id, so a result joins its audit line.

A request arrives as one value, a `PreparedInvocation`, built once when the session reads the
call. It owns its id, tool, workspace, the evidence of the connection that sent it, its
cancellation, when it was received, its deadline and its decoded operation. The session queue
holds it, and the executor takes it whole with how the queue admitted it: queued, or refused for
capacity, which completes through the same path without running. Its deadline and its recorded
span count from when the service received it, and a live row starts at that time, so a call that
waited shows the same span live as in history.

A request's cancellation keeps who cancelled it beside its flag, set once: the agent (its
`notifications/cancelled`), the end of its session, or Ghostlight's quit, which the session's
close tells apart. A cancelled call's sentence names that cause, and a cancellation the browser
reported without one keeps the plain words. A call that arrives once Ghostlight began to end, or
after the end stopped its session's queue, is answered on the thread that reads its stream through
`execute_prepared`, as cancelled before it started by the quit, with an audit line; it takes no
lease. The quit's cause (`QuitCause`) carries why Ghostlight is ending: after a quit, a replace, an
idle exit or a reason this build does not know, the edge reconnects, so the call's step is to try
again in a moment; after an uninstall or the end of the sign-in session there is no step.

What a frame learns lives with the frame and is read once by its completion. A frame is the
request's top-level operation or one flow step, each built by its own constructor from its own
operation. Its findings are what the page showed, the browser that received its commands, its
governance checks, its document coverage and the recovery a stale-target refusal offers. They sit
on the frame's stack, never on the executor and never keyed by an id. A step shares its flow's
id, workspace, caller, deadline, cancellation and authority snapshot, and records only its own
checks, coverage and browser. The flow keeps its own admission check and takes one fact from each
step, the browser it reached, so the flow's record names the last browser a step reached. A
request that never decoded records no capabilities.

### Workspace

Owns MCP sessions, controlled tabs, opaque tab and target handles, document generations,
view handles and viewport transforms, selection, ownership, leases, the released-tab pool, stale
detection, and release on disconnect.

A connection binds to one workspace when it is admitted and keeps it until it closes. Nothing
moves a session into another workspace, and a handle from another session is refused before any
browser work (ADR-0196 Decision 8).

A workspace is named for the connection that opened it (ADR-0202 Decision 15). It keeps that
connection's evidence as its opener for its whole life, and the session card, the connection
counts, Integrations in use, the tab-group title, the ownership refusal and released-tab reuse all
read the opener. When the workspace opens, the opener's agent is read once, as
`language::provenance` names it for the window, together with its reuse key (ADR-0203 Decision 9).
The group title is `Ghostlight - ` and that agent's name, such as `Ghostlight - Claude Code`,
`Ghostlight - Command line`, or `Ghostlight - Unknown agent` for an opener nothing names. A
command-line workspace outlives its connections (ADR-0106), so it cannot be named from the current
ones. A later connection to the same workspace is listed beside the others and renames nothing.
The client's claim is one bounded value (`provenance::bounded_claim`: control and direction text
stripped, at most 100 characters, an empty claim is none), read from `initialize`, discovery and
a resume token.

A tab binding has one lifecycle (ADR-0203 Decision 2). Forgetting a tab removes its targets,
views, snapshots and images, whether Ghostlight closed it, the person closed it, or the browser
did not confirm it, and a handle bound again at generation 0 carries nothing from before. A tab a
refused landing left held still refuses reading and acting, but `browser_navigate` to an allowed
address and `browser_tabs` close may select it, each authorized on its own and never on the page
the tab shows, and a governed landing clears the hold. One executor helper takes every hold and
emits `HoldEntered`, the read path's included.

A physical tab id names one tab only within one browser run, and Chrome hands ids out again after a
restart, so a binding the service keeps across a gap in what it knows about a browser is a hint
until that browser confirms it (ADR-0203 Decision 1). A gap begins when a browser stream ends, when
a hello registers that browser id again, and when a resume token brings bindings back after a
service restart. The workspace aggregate states the rule once, and the released pool's reuse check
is the same predicate. A pending binding is kept when the live browser has a tab with its id and
either the binding was made in the life the browser reports now, or the tab shows the URL the
workspace last knew. In the same life the binding takes the live URL, title and readiness, and its
generation moves on when the URL is not the one it remembered, so a target or capture of the
earlier page reads as navigated. Otherwise the binding is forgotten as closed, and the agent hears
that the handle names no open tab. A pending binding is inert: it takes no commit, readiness or
dialog event, a `TabClosed` forgets it, it does not count as owned when another session asks, an
`OpenTab` or `AdoptTab` result that carries its id in its browser forgets it, and it is never
released through the browser, because the extension closes any id it is sent when preserve-tabs is
off. Release, quit, session end, reap and a failed confirmation forget it without a command. It
stays in the one tabs map, so no handle is minted twice. The first call of a workspace that holds
pending bindings confirms those of every connected browser with one `ListTabs` each, before any tab
selection or browser targeting, and `browser_tabs` list does the same. A call that selects a
pending tab of a browser not yet connected, or that names no tab while every pending binding is in
one absent browser, waits for that browser as a bound session does, and forgets the bindings only
when the browser did not come back. When the workspace binds another browser, the other browsers'
pending bindings are forgotten and the result names their handles in `forgotten_tabs`. A browser
can also restart between two crossings of one call, so every command addressed to one tab reads how
the workspace holds that tab before it crosses, confirms it again if it went pending, and answers a
tab the browser does not confirm as no longer open with nothing sent.

An MCP session's resume token is written by the service and opaque to the edge, which holds it in
memory and never reads it. It carries the session's revision, its client claim, and each tab with
its own browser and a digest of the tab's URL (the SHA-256 of `TabState.url` bounded to 8192
characters, as the adapter bounds a tab's URL), so the edge holds no page text. It carries the life
of each kept tab's browser, when one life covers them, and whether the session stood by. It is
published with the `initialize` answer, after each call, on a promotion and on the session watch's
one-second tick, so a close or a commit outside a call reaches it, and it is never written past
64 KiB (`MAX_RESUME_TOKEN_BYTES`, what the reader accepts): a token too large keeps its first tabs,
and a tab left out reads as no longer open after a restart. A resumed session's tabs enter as
pending bindings, each with its own browser. A token written before digests resumes with every tab
stale, and a token that does not decode still resumes an initialized session at the oldest supported
revision with no claim and no tabs, with a diagnostics line. The token stays unsigned: it is a hint
and never authority. A same-user program that writes one can bind only a tab whose id shows the URL
it names, or, carrying the browser's current life, a tab of the current run; that program can
already act as the user (ADR-0199 Decision 5, ADR-0203).

Physical tab ids are unique inside one browser only. Ownership checks compare a tab id within the
browser its workspace works in, so Chrome's tab 5 and Edge's tab 5 never refuse each other.

When a workspace is released, the tabs it still holds enter a released-tab pool keyed by browser,
with the governed URL each held at release and the releasing workspace's reuse key. A held tab, one
that never landed, or a tab of a workspace with no reuse key does not enter; it is still released
and closed as preserve-tabs says. Release cleanup then asks the browser to close each tab. A tab
becomes reusable only when the person's preserve-tabs setting refuses that close: the extension has
then already dropped the dead workspace's binding and debugger hold, and the tab stays open. Until
the close answers, no session can claim the tab, so a cleanup close never acts on a tab another
session took (ADR-0196 Decision 8). A close that worked, failed, or did not answer in time removes
the tab, so with preserve-tabs off nothing is reused. A tab also leaves the pool when it closes,
when any page commits in it, and when a workspace takes it. When its browser disconnects, its
entries go pending as bindings do, because a restarted browser hands out tab ids again: after the
browser returns, an entry is reused only where that browser still shows the tab at its release
address, and pool entries record no life, so the URL decides. A close or commit that arrives while
cleanup runs keeps the tab out. Default reuse (`reuse:"domain"`) adopts only from this pool
(ADR-0196 Decision 9). The executor claims the best same-host candidate released under the caller's
reuse key, under the aggregate lock, exact URL first and then lowest id, checks it against the live
tab list, adds it to the workspace, and sends `AdoptTab` with that exact tab id and the workspace's
group title. The kept tab left its group when its session ended (ADR-0201), so the adopt puts it
back in `Ghostlight - <agent name>` as it loads the page, unless the person put it in a group of
their own. The reuse key is the agent the opener's claim names on its channel (`Agent::of(claim,
channel).id`): every command-line session shares the command line's, a claim the catalog knows gives
that agent's catalog id, and any other claim an unknown agent under its claimed name. A claim that
names no agent gives no key, so that session reuses no kept tab and no one reuses its tabs. The
source Ghostlight saw takes no part, because the source decides nothing (ADR-0202 Decision 1). So an
agent reuses only the tabs its own sessions kept, as a courtesy between clients: a tab another agent
released is left in the pool and the open creates a fresh tab. The key only narrows reuse and never
grants authority. A client that claims another client's name can adopt that client's kept tabs; the
adopt loads the page in the kept tab, so the origin's sessionStorage and the tab's history cross
with it, and that is the accepted same-user risk (ADR-0199 Decision 5, ADR-0203). The pool is in
memory and does not survive a service restart. A person can still type into a preserved released tab
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

Owns authority loading, immutable snapshots, capabilities, scheme boundaries,
policy-protected hosts, the channel switches, admission, landing decisions, runtime controls, holds,
and content-minimized audit intent. Operation handlers see only the governance facade.

The channel switches, `channels.mcp.enabled` and `channels.cli.enabled`, are on when unset. The
executor asks the snapshot about the request's channel, read from its connection's evidence, before
any browser work, and a channel that any layer turned off refuses the request through the one
completion path. The connection stays open, so the agent hears why. The channel is declared by
Ghostlight's own peer, and any program running as the same user could declare another, so a switch
is a guardrail for well-behaved agents, not a barrier against hostile software on the same account.

The snapshot gives one answer to each question (ADR-0203 Decision 7). Whether a requirement set
can be used anywhere is decided through the snapshot's full landing decision, never-touch hosts and
the window's own origin included, over a finite set of probe hosts: every exact pattern, one fresh
label under each suffix wildcard, and one host that matches no suffix. The catalog lists a tool
only when that answer is not unavailable (`could_admit`), and the Policy page's reach lines read
the same answer, so layers whose grants share no host advertise nothing and get their own
sentence. Recorded work is replayed by one function, `AuthoritySnapshot::replay(channel,
requirements, host)`, which checks the channel first, reading a record that names none as MCP
work, then the host; `ghostlight policy simulate` and the window's preview both call it. One
organization identity is resolved per publication, the signed presentation's name, rationale and
contacts first and the manifest's block for what it lacks, and the snapshot, the window, the
channel refusal and the passport read it.

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
last snapshot outlives that connection. The controls part carries the acts the runtime state
offers, and the connections part carries the counts the service computes, the ones the tray's
tooltip and `status` say; the band, Settings and About draw those and count nothing of their own.

Harness registration is an orchestrator-owned local-human capability. Each supported harness has
one explicit config resolver and schema. The resolver applies Windows or Linux environment
precedence before its documented fallback, including `CODEX_HOME` for Codex. On Windows a
configuration below `%APPDATA%` or `%LOCALAPPDATA%` is also looked for, on every look, in each
installed Store package's private copy of that root, `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming`
or `...\Local` (`install/packaged.rs`): `Packages` is listed once and only the client's own settings
folder is tested in each package, so a Store edition is found even before it writes its file. A
copy in the agent's own package (named by its packaged app or Store icon) beats the plain path,
except that while that copy has no file the plain file it shows wins; another package's copy beats
the plain path only when the plain file and its folder are both absent; within a rank the copy
written last wins. Inspection names the chosen file, says when it is the agent's own Store copy
and names another copy that exists; Install and Fix write only the chosen file, and Uninstall,
available while any copy holds Ghostlight's own entry, removes it from every copy that holds one
(ADR-0197 amendment of 2026-10-17). Check is read-only.
Install and uninstall are serialized, merge only the `ghostlight` entry, keep unrelated siblings,
preserve JSONC and TOML comments, create a backup, and refuse malformed, unreadable, or
foreign-owned configuration rather than guessing. A separate confirmed Fix action may replace one
parseable foreign entry under Ghostlight's own key. The writer re-checks that state, preserves the
same surrounding document, and uses the same backup and atomic replacement seam. Fix is never an
automatic or aggregate action. The UI exposes no generic filesystem or process operation.

An agent wears its app's own icon when one is installed. The catalog (`install::ROSTER`) names,
per host, where that icon may be found: a Windows program (on `PATH`, through the program a
launcher script starts, or below a known folder), a Store package, or Linux desktop entries, and
the agent whose icon stands for one that has none (Claude Code wears Claude Desktop's). One
resolver, `install/icons.rs`, runs off the request path, at the service's start and on Re-check,
bounded in time. It keeps the first icon found in `state/icons` with a manifest, reads a source
again only when its file changed, and on Linux takes PNG only, never SVG. On Windows a program's
icon is its icon resource, copied as a `.ico` file by `crates/win-peer`, which maps the program as
data and runs none of it; a Store app's icon is the PNG logo its package manifest names, in the
variant that suits the window, found through its family's folder below `%LOCALAPPDATA%\Packages`
and the install folder Windows names for that family. The manifest also names the agent whose app
gave the icon and the kind of place it was read from.

Every catalog agent has one image address, `agent-icons/<agent id>`, and the service decides what
it serves there (`crate::agent::image`), in the owner's order: the app's own icon from the cache,
then a curated vendor mark (none is packaged yet), then the agent's packaged monogram tile. An
app's icon is served as a finished tile: the service decodes the cached copy, and when the icon is
not its own plate it sets the icon at its own size in the middle of a square of a dark gray or a
warm off-white, whichever more of the icon's ink reads on, and encodes it as an opaque PNG. An
icon is its own plate when its drawn area spans at least three quarters of it both ways and is
opaque along the middle of each edge, as an app icon's plate set inside a transparent margin is.
An icon with its own opaque plate, or one it cannot decode, is served as cached. Each tile is made
once per cached copy and kept in memory, but the copy is read and checked against its hash each
time the symbol or the route asks, so a copy that is gone or changed gives the monogram. The monogram
tiles are Ghostlight's own SVGs in `crates/orchestrator/src/agent/monograms/`, embedded per catalog
row and served only at that address, never as page files. The agent's symbol, built where
`crate::agent` builds the agent, is `{ "icon": "agent-icons/<id>?v=<version>" }`: the version is
the first 12 hex characters of the SHA-256 of the image served there now (an app icon's tile, not
the cached copy), so the address changes
exactly when the image does. It also carries `"about": "Icon from the installed <app> app."` only
when that image is an app's own icon, and the Integrations pane shows that line. The command line
and an unknown agent keep their glyphs. The image is the agent's image only: whether the agent is
connected or working is the page's dot and words, never part of the image. The window answers GET
and HEAD at the address for catalog ids only (any other id is not found), as `image/png`,
`image/x-icon` or, for a monogram, `image/svg+xml`, with `nosniff`, `default-src 'none'; sandbox`
and an ETag of the version. Which image it serves says what is installed, so the address follows
the data routes: it answers the window and a watcher, and while "Others can watch" is off it
answers that watching is off, whatever the id. An image request carries no bearer, so the page
draws every agent image the same way: it leaves the image without a source, fetches each address
once with its bearer (`ui/lib/icons.js`) and fills in every image that names it. Until then the
image shows a quiet placeholder, and an image that cannot be had keeps it and says nothing. Every
image the service serves is a whole tile, so the page draws an app's icon and a monogram the same
way, on no ground of its own.
`ghostlight doctor` reads the cache, never the apps, and says per agent whether an icon was found,
from which agent's app and which kind of place.

Linux desktop integration is another installer-owned local-human capability. A per-user install
owns one XDG desktop entry and one byte-identical icon. The entry runs the shell,
`bin/ghostlight-desktop`, from the one fixed `bin` directory, and the shell starts the service when
none runs; update and uninstall touch only explicitly owned files. Ghostlight ships no Debian
package. Browser package provenance
is read-only and typed separately from native-host registration, so a current manifest cannot make
a sandboxed or absent browser look usable.

Setup is a part of the workbench, `setup`, projected for the window only: a watcher's snapshot
omits it and shows the ordinary workbench. The words of the setup view, of the actions it offers
and of the install summary are `language/setup.rs`'s; `workbench/setup.rs` gathers the facts and
words nothing. The facts are the browsers connected now, the live sessions, the cached client
roster and, only while setup is incomplete, the native-host report and a read of each installed
Chromium browser's `Local State` and `Extensions` folder (`install/browser_profiles.rs`), which
tells an added extension in a closed browser from a missing one. Nothing that read finds is
stored, logged or sent. Setup is complete when a model client's first top-level request ran in a
browser and succeeded: the audit projection writes `state/setup.json` once at that moment, and
the break migration writes it for a home already in use. A `ghostlight call` never completes
setup. A complete setup reads nothing from the machine.

The setup view's browser steps are service acts. `POST /api/setup/act` (window only) carries out
`add_in_browser`, which starts the named browser at the store listing, and `open_browser`, which
starts it in the profile that holds the extension, after checking the step against what the
computer shows now. Any other step is refused. The Get Google Chrome link, the Connect and Update
act of the integrations tab, Copy setup and the Review link need no setup route. An agent row offers
Connect or Update only where the client roster says Ghostlight can install, and reads Connected
only while the agent is in use: a live session of it with that row's entry in place, the one rule
the Integrations page reads from the connections part (`harnesses_in_use`). The window serves the extension's
own `extension/kit.css` and mascot at `/extension/kit.css` and `/extension/ghostlight-mascot.png`
from the closed asset table in `workbench/web/assets.rs`, so the setup view and the extension's
pages share one stylesheet with no copy.

A window that opens while setup is incomplete opens on the `setup` address. That covers the
interactive install, which asks the running service to show the window through the control
operation `show`, and the tray's Open. The browser can ask for the same through the protocol-4
event `show_requested`, which has no fields and which the service answers by revealing the window
at the address it would choose anyway. An install run with `--dry-run`, `--no-open`, in CI or in
an app container shows nothing.

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

Every direct call and flow enters `ApplicationExecutor::execute_prepared`. One invocation owns
one unit of work until exactly one completion is committed.

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
positive grant identities for each evaluated layer. It carries no caller restriction: a call that
sends `restrict_hosts` or `restrict_capabilities` is refused at decoding, before dispatch.
`language/history.rs` owns the readable explanations and each history row's one closed outcome
class, computed from the typed record (`OutcomeClass::recorded`): `succeeded`, `refused`, `kept`,
`needs_you`, `failed`, `unknown` or `cancelled`. A credential refusal is `needs_you`, the
preserve-tabs refusal `kept`, and a step that never started `failed`. The window draws each row
from its class and search finds a row by its class's word; composition's flow causes derive from
the same class. `workbench/history.rs` projects and restores
whole groups, at most 500 with at most 20 children each. Missing receipts stay unconfirmed; only
parent evidence establishes a never-run suffix. A composition-change projection updates the
existing feed without settling the parent or sending extra completion notifications. Expanded
view state remains disposable. Policy previews consider recorded children, not empty wrappers
(ADR-0156).

## Closed domain events

The in-process vocabulary is:

- `WorkWaiting`
- `WorkStarted`
- `TabCreated`
- `DocumentCommitted`
- `TargetIndicated`
- `WorkPhaseStarted`
- `HoldEntered`
- `WorkBlocked`
- `WorkCompleted`

`WorkWaiting` and `WorkStarted` carry the details of the connection that sent the request and
the time the service received it. Every other event joins its request by invocation id.

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

Ordinary product features compose these primitives only in the orchestrator. A relay or adapter
change requires a new physical browser capability or an adapter protocol requirement.

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
then sends bounded names, media types, and bytes through the adapter protocol. The extension creates
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
carries bytes out, and only that destination is bounded by the transfer ceiling. A save to
Downloads carries effect, so a pause refuses it before it is sent and a lost receipt reads as
after dispatch, its effect unknown. Encoding runs in
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
applies cross-origin authority plus final filtering before any model disclosure. A read sends the
extension a document binding only when the authority watches documents, which it does when its
policy does not admit every document. Every tab's ring starts fresh at a top-frame navigation,
bound or not, and a bound tab's also at a changed binding; a navigation inside an embedded document
never clears it. Reads are
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
suffix wildcards, which outrank `*`; an exact tie denies. Managed and local policy intersect. Sacred
destinations compose by union, and each layer keeps its own list: a refusal of a never-touch host is
that layer's policy decision, reason `protected_host` and rule `sacred_host`, credited to the first
layer whose list matches, with its tier and a denial id taken from that layer and the rule alone, so
the id names no host (ADR-0203 Decision 7.2). Only non-HTTP(S) schemes and Ghostlight's own control
page stay Ghostlight's own ceiling, credited to no authored layer. False tab-close and target-name
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
visible and the result reports that compensation did not occur, and the tab is held. A denied
landing in an existing tab enters hold and returns a committed, non-repeat-safe blocked outcome.
The hold keeps the landing's decision, the snapshot that made it and the refused address, in
memory only. A later call on the held tab is refused as `tab_held` with that landing's denial, its
reason, rule, denial id and tier, decided again under a republished authority; it names the
refused host and the two calls that leave it. Redirects receive the same check.

Audit records include time, invocation id, workspace id, tool, complete RAWX requirement set,
authority version, managed sequence, mode, deciding tier, grant, rule, denial id, decision,
status, effect class, and reason code. They exclude request and page payloads except the governed
host and optional bounded target name described by the active privacy setting.

Every terminal supplies a language-owned `AuditProjection` alongside its client result. Audit
accepts this projection rather than a raw summary or result JSON. One `retained()` in
`language/audit.rs` gives what history may keep of an outcome or a refusal: it strips page-authored
text and client claims (fill read-backs, dialog messages, another session's owner, a credential
control's name and handle, the control page's origin and the browser's own words) and recurses
into an outcome wrapped by an opened dialog or an unconfirmed check, and `Outcome::audit` and
`Refusal::audit` render from it with no fallback to the agent's sentence (ADR-0203 Decision 4.1).
A flow step that never started is written by `AuditRecord::not_started`, the one writer of that
line, which carries the policy sequence as every other line does. Refusal metadata is a closed
type; arbitrary browser error text stays in the permitted client result. The workbench renders
the retained sentence. Historical failure payloads are discarded when read, without editing the
historical file or claiming to sanitize its old summary text (ADR-0103 H1 amendment).

## Failure and recovery

Disconnect before a physical request has no effect. Disconnect after dispatch without a decisive
receipt yields `unknown`, never failure or success. Cancellation follows the same boundary, and
before dispatch it is reported as a cancellation, never as a disconnection. The other
before-dispatch facts keep their own names: a browser that is attached but not answering, a
browser that is not connected, a bound browser that is gone while a different one is connected,
and a connection busy with other work. An adapter contract failure has its own name too, but it
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

The extension also ships a bundled setup page, `setup.html`, and one stylesheet for its pages,
`extension/kit.css`, whose tokens and rules apply only under the `.gl-page` class. The page opens
once when the first connection finds the service absent, and from the setup buttons of the popup
and the options page. It loads nothing from the network, holds the setup command and the words
for this computer's step, and offers Open Ghostlight, which sends `show_requested`. The words of
the extension's pages live in the extension and ship in its package. The packaged set is recorded
in `tests/golden/extension-2.0.0.files.sha256`, and `tests/guards/extension-freeze.test.mjs`
fails when a packaged file is added, removed or changed against that record. An extension change
regenerates the record in the same commit. Once a store submission is staged, changing a packaged
file means resubmitting, which is the owner's call (ADR-0200 Decision 16).

Workspace tab grouping is browser mechanism. `OpenTab` carries the governed URL, the owning
workspace already present on every browser request, and the presentation-only exact title
`Ghostlight - <agent name>`, the name of the agent that opened the workspace as the window names it
(see Workspace). The adapter
resolves placement before opening: it reuses one browser-wide exact-title group wherever that group
currently resides, otherwise reuses a window containing another Ghostlight group, otherwise creates
a dedicated normal window containing the requested URL. Destination resolution is serialized so
concurrent opens cannot create duplicate groups. The first tab for a workspace brings the Ghostlight
window into view; later tabs do not repeatedly steal window focus. The adapter colors the group
blue, records the opaque workspace association, and opens without an externally visible blank-tab
step. Group ids are restart-local hints and exact-title discovery repairs stale hints. The label
never routes or grants authority. A newly opened tab is not adopted through its opener: that
relationship cannot distinguish an agent popup from a human opening a link. Explicit executor work
establishes ownership. Moving a group or tab between windows does not change ownership or trigger
automatic regrouping.

The adapter never chooses a tab to reuse. The executor's `OpenTab` carries no reuse request, so a
plain open always creates. A reused tab arrives as `AdoptTab`, the adapter's open with
`adopt_tab_id` naming the exact tab the service chose. A tab the person keeps after its session ends
leaves its Ghostlight group (ADR-0201): Chromium saves a group that is still open when its window
closes or the browser exits, and no extension API can find, reopen or delete a saved group, so a
group left open between sessions would come back as one more same-title group after every browser
restart. The adopt puts the tab back in the group, moving it to the group's window when it is
elsewhere, and leaves a tab the person put in a group of their own where it is.

Document-commit events silently update cached destinations and invalidate agent references. They
never authorize browsing, hold tabs, write action history, or produce presentation. Even a
correlated event is not proof of agent authorship. The executor governs its requested operations
and returned landings at the existing boundaries. ADR-0164 owns this distinction.

Controlled debugger attachments emulate a focused page while the connected, compatible service
reports active runtime control (ADR-0168). This keeps focus-triggered page refreshes from
interrupting automation between calls. The existing debugger lifecycle owns per-tab serialization,
retention, and restoration. Pause and disconnection remove the override; stop and local
debugger release also detach. A released workspace loses debugger retention even when its tab is
preserved. Custody follows the service that granted it (ADR-0203 Decision 3, amending ADR-0186):
the extension keeps the service epoch in `chrome.storage.session`, and a `hello_accepted` with a
new epoch releases every managed tab through `controlledTabs.release` before Active applies, so
the debugger is unretained, focus emulation and the glow go off, and the tab stays released until
its next top-level commit. The service takes a tab back with its next call that addresses it
(ADR-0168), and a worker restart against the same service keeps custody. Passive browser events
never acquire this custody. The scope can outlive a model turn,
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

Model-driven close is dual gated. The orchestrator first admits the action capability on the tab's
host and then the monotonic tab-close policy constraint. Closing a tab a refused landing left held,
like undoing Ghostlight's own denied landing, checks the runtime controls and the tab-close setting
only, because it takes the tab away from a page no policy admitted. The extension then checks its
default-on preserve-tabs preference immediately before `chrome.tabs.remove`. A local refusal uses
the existing content-free adapter error channel and is rendered by the orchestrator as a blocked
no-effect result. The local preference is not advertised as a physical capability, does not
renegotiate the adapter, and cannot override service authority. Direct human closure through
Chromium remains outside this model-driven path.

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
and controls. The one hold a refusal leaves is on a tab a refused landing left on the refused page:
that tab refuses reading and acting until a navigation to an allowed address or a close takes it
away, and every other tab and session is unaffected.

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
mechanism binds frame routing to document identity. Neither relay interprets policy. A locator
whose embedded document now holds another document, or whose frame is gone from a complete
inventory, is a stale target that navigated, decided where the adapter describes the documents,
and a `document_changed` answer to a call that names one locator is the same; an incomplete
inventory still reads as unverified access. A read targeted below the top document reports the
tab's top URL and no title, so the landing check, the observed host and the `url` fact are the
tab's, and an embedded host never enters the durable record (ADR-0203 Decisions 5.4 and 6.1).

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
