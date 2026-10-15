# ADR-0199: One service, thin shells

Date: 2026-10-14. Status: Accepted by owner direction (2026-10-15).

The owner approved the lean blueprint's three decisions on 2026-10-15, and the ruling that typed
commands run as one-shot service-image processes in 2.0 and move inside the running service in 2.x
with no edge change. Each decision below says where it lands: in 2.0 (frozen); with the desktop
shell, which may land before the 2.0 release when it is ready but is not part of the frozen set;
or in 2.x.

Builds on ADR-0196, ADR-0197 and ADR-0180, and keeps ADR-0197 Decisions 2, 4 to 9, 12 and 16 to 24
with its 2026-10-14 amendments, except where stated here. Each supersession lands with its
decision:

- ADR-0197 Decisions 1 and 3 are superseded now (Decisions 1 to 5 here).
- ADR-0197 Decision 15, and the development directory and marker of Decision 13, are superseded
  when Decision 18 lands (2.x).
- ADR-0127 Decisions 1 and 3 and ADR-0166's desktop-ready discovery (its section "Discovery means
  the desktop interaction route is ready", which ADR-0197 Decision 10 kept) are superseded when
  the shell lands. ADR-0102 Decisions 1 and 6 and ADR-0119 Decisions 2 and 3 change when the shell
  lands; ADR-0119's Quit writes a latch now.
- ADR-0127 Decision 2 is amended now: `serve --reason R` is the service's one launch, and
  `service` and `--headless` stay removed.
- ADR-0197 Decisions 10, 11, 14 and 25, and its Consequence on the Windows command line, are
  amended now.
- ADR-0180 is extended to every process now.
- ADR-0070 Decision 1 is amended by Decision 23.

ADR-0127's rule that nothing starts Ghostlight at sign-in stays.

## Context

On 2026-10-14 the owner asked for the process and binary architecture as it would be built from
scratch, with clean separation and easy maintenance, on one premise: exactly one service exists,
and no other instance of it is ever up. Four proposals and three independent judgments agreed on a
service with no UI toolkit, frozen edges and one local endpoint. The owner then ruled on the open
questions, and on 2026-10-15 narrowed what 2.0 waits for to the frozen set.

The tree shows why:

- The service hosts Tauri, so every Linux start is tied to GTK. Four musl connectors with no
  desktop made 32 transient authorities in four seconds (ADR-0166), and a client that filtered the
  display started an authority with no desktop (MEMORY, the Bluefin evidence). ADR-0127 Decision 3
  ends the authority on any desktop event-loop fault.
- The release `ghostlight.exe` is GUI subsystem, as its PE header confirms (assessment finding
  ONB-1). Typed commands print nothing on Windows, and a shell does not wait for them.
- Discovery is a port, a token and a runtime file under a lease (relay protocol 3). Each can go
  stale, and every client reads them before it connects.
- A development build lives in a second directory chosen by a marker that three components check.
- After Quit, a browser launch starts the service again (ADR-0197 Decision 11).
- ADR-0180 exists because Ghostlight once made Windows show "This app is preventing shutdown". Its
  2026-09-25 amendment gives every Windows process the session listener. On dev only the service's
  desktop registers it; the connectors rebuilt on `relay` register none.

Live evidence, 2026-10-14: a named pipe connected in both directions between a process inside the
Claude desktop MSIX app container and one outside it, the main risk the review flagged.

Nothing that speaks relay protocol 3 has shipped, so this is the cheapest moment to change the seam.

## Decision

### Components

1. **Two executables in 2.0, one job each; the shell's name is reserved.**
   - `ghostlight-service`: every product decision and all state. MCP, language, governance, work,
     workspaces, the Chromium adapter and page assets, audit, history, diagnostics, every typed
     command, install and registrations, the tray model, the workbench pages and notification
     text. Once the shell lands it links no UI toolkit (owner ruling); until then ADR-0127's
     desktop authority runs inside it.
   - `ghostlight`, the edge: the MCP relay (ADR-0197 Decision 2), the browser relay (Decision 4)
     and the front door for typed commands. It parses no payload, authors no sentence, stores
     nothing, compares no versions and knows no verb.
   - `ghostlight-desktop` is reserved for the shell: a thin native window around the OS webview
     (WebView2, WKWebView, WebKitGTK), plus the tray and notifications. It ships in the service's
     build, with the same version in the same release, and is not frozen (owner ruling
     2026-10-15). It renders only service-owned HTML served through a private custom scheme, opens
     no network port and decides nothing.
   - The Chromium extension: adapter protocol 4, frozen at 2.0.
2. **The edge is a carrier check and three rules,** applied in order:

   ```
   this file is not <home>/bin/ghostlight    run the service image beside it with the same arguments (a carrier)
   mcp                                       MCP relay
   chrome-extension://<id>/                  browser relay
   anything else, or nothing                 run <home>/bin/ghostlight-service with the same arguments
   ```

   The edge knows no verb. The service image's verb table decides what a verb does, so Decision 6
   needs no edge change. Paths are compared with links resolved, so a WinGet alias finds its
   package. On Unix a hand-over is an exec. On Windows the child shares the console, and the edge
   swallows Ctrl-C and Ctrl-Break, waits and returns the child's exit code.
3. **Crates in 2.0.**
   - `relay`: frozen. The endpoint, the peer checks, opening, framing, goodbye, start reasons,
     backoff, and an `os` module with the session listener, the detached start and the
     AppUserModelID call the shell needs. It depends on no workspace crate.
   - `edge`: frozen. It depends on no workspace crate but `relay`.
   - `desktop`: arrives with the shell, depends on no workspace crate but `relay` plus the webview
     toolkit, and is not frozen.
   - `orchestrator` builds `ghostlight-service`. Tauri and webkit2gtk leave it when the shell
     lands.
   - `win-peer` keeps only the service's FFI. Both connector crates are deleted in 2.0. `bridge`
     keeps only the service's diagnostics until 2.x dissolves it.
4. **Commands are commands on every Windows version** (owner ruling). In 2.0 the edge is console
   subsystem and embeds an application manifest whose `consoleAllocationPolicy` is `detached`, so
   on Windows 11 24H2 and later a stray double-click shows no console; Windows 10 flashes one
   briefly. Typed commands wait, print and return exit codes in every shell through the edge's
   console. Until the shell names every shortcut, the service image stays GUI subsystem; a typed
   command reaches the person through the edge's console, with a parent-console attach in the
   service image only if live checks need it. Once the shell lands, the service image becomes
   console subsystem with the same manifest, any attach is deleted, `ghostlight-desktop.exe` is
   GUI subsystem, and shortcuts and pins name the shell, which shares the AppUserModelID
   `Sylin.Ghostlight` with its shortcut. The service's children start through one launcher with
   `CREATE_NO_WINDOW`.

### Seams

5. **Relay protocol 4** (supersedes ADR-0197 Decision 3). Lands in 2.0, frozen.
   - Endpoint: on Windows the named pipe `\\.\pipe\ghostlight-<user SID>-<8 hex of a hash of the
     home path>`, with `PIPE_REJECT_REMOTE_CLIENTS` and a DACL for the user only; on macOS and Linux
     `~/.ghostlight/run/relay.sock` in a 0700 directory. The name never derives from session
     variables that MCP clients filter, and the home hash keeps a test home apart. If named pipes
     fail inside the Codex app container, the fallback is loopback with a token behind the same
     relay API.
   - Each side checks the other before any payload byte. The service requires the same user, and on
     Windows at least medium integrity, which keeps sandboxed and AppContainer processes out. The
     client requires a server running as the same user, so an account that took the name first
     receives nothing.
   - Framing is relay 3's. The opening is `{relay: 4, role, edge, reason?, resume?, origin?}`, plus
     the role's own optional fields: `channel` and `session` for `mcp` (the command line's session
     uses them), and `control` with its `op` for `control`. The roles are `mcp`, `browser`,
     `control`, `desktop` and `command`. The reply is `{accepted: true, service, instance,
     templates}` or `{accepted: false, code, say}`, and the client prints `say` as is. `instance`
     changes on every start. Liveness is an accepted opening. Control is `readiness`, `status`,
     `show` and `quit{reason}`.
   - Before it closes streams the service sends `goodbye` with `quit`, `replace`, `uninstall` or
     `session_end`. On quit, MCP relays answer held lines, browser relays wait and the shell exits.
     On replace, MCP relays hold and reconnect, browser relays recycle and the shell shows Updating.
     On uninstall and session end, everything exits.
   - Every role (`mcp`, `browser`, `control`, `desktop`, `command`), every start reason (`agent`,
     `browser`, `command`, `person`, `recover`, `update`) and every goodbye reason (`quit`,
     `replace`, `uninstall`, `session_end`) exists from relay 4's first revision. A 2.0 service
     refuses the `desktop` and `command` roles as unsupported until the shell and Decision 6
     land. While the desktop runs inside the service, the service binds the endpoint once its
     window and tray are ready, as ADR-0166's timed discovery does; a headless service binds at
     start. The runtime file, port, token, templates file and lease are deleted.
6. **Typed commands move inside the running service in 2.x, with no edge change.** The `command`
   opening then carries the arguments, the working directory, which standard streams are
   terminals, and the terminal width. No environment variable crosses. The process that does this
   is the one-shot service-image process the edge hands over to (Decision 2), connecting with the
   `command` role. It pumps stdin, stdout and stderr as bytes, turns Ctrl-C, Ctrl-Break and a
   closed terminal into `interrupt`, prints `relay`'s one fallback sentence and returns a fixed
   code if the stream drops first, and otherwise returns the exit code the service sends. The edge
   still only hands over, so no edge change follows.

   In 2.0 every typed command runs as a one-shot process of the service image (owner ruling
   2026-10-15). `status`, `doctor`, `help` and `--version` never start the service; `call` starts
   it with reason `command`; `open` and a bare launch start it with reason `person`. Nothing
   serves in the foreground.

   This answers the owner's premise. One service exists and no other instance is ever up, because
   a one-shot process takes no lock, binds no pipe and can never serve.
7. **The shell and the service share a build, not a protocol** (owner decision 2, 2026-10-15). The
   `desktop` role carries a same-build contract between the service and the shell that changes
   freely: the shell's hello names its build, the service sends the UI bundle by hash, the tray
   model, window commands, notices and page messages, and the shell sends page messages, clicks
   and native results. The webview's only bridge is post JSON and receive JSON, under a fixed
   content policy that allows nothing from the network. The shell's own words are four states:
   Starting, Updating, Restarting, and "Ghostlight stopped unexpectedly" with Start and
   Diagnostics. `WorkbenchFacade` stays in the service, and today's allowlisted Tauri commands
   become messages to it. Only the shell's name `ghostlight-desktop` and the AppUserModelID
   `Sylin.Ghostlight` stay fixed. An update closes and reopens the window. Lands with the shell.
8. **Three version axes, all additive:** relay protocol, adapter protocol, and the MCP revision,
   which only the service negotiates. Unknown fields are ignored, fields are never removed or
   repurposed, and new behavior gets a new name. The service accepts every relay revision from 4
   and uses only what each extension advertises. Frozen in 2.0: `relay`, the edge, the extension's
   mechanisms, the endpoint names, the host name `org.sylin.ghostlight` and the home layout. Each
   changes only with its protocol. The shell is not frozen and has no axis of its own.

### State, start and stop

9. **One service, by construction** (amends ADR-0197 Decision 10). `ghostlight-service serve
   --reason R` does this, in order. Lands in 2.0:

   ```
   1  refuse unless this image is <home>/bin/ghostlight-service
        (or the marked development build, until Decision 18 lands)
   2  refuse when elevated on Windows
   3  apply the quit latch and the ending state (Decisions 10 and 13)
   4  take run/service.lock, held until exit
        held by another: an update start waits up to 30 s; any other start exits
   5  bind the first pipe instance (FILE_FLAG_FIRST_PIPE_INSTANCE), or on Unix
        unlink a stale socket as the lock holder and bind
   6  open state and accept openings; not ready within 30 s: exit
   ```

   The lock precedes any state, and the pipe flag is a second refusal; of ten racing starters, nine
   exit before they bind. Binding waits for the desktop as Decision 5 says. Lands in 2.x:
   `state/version` (a release older than it is refused, a release raises it to itself),
   `run/service.json` with the pid, process start time, version and instance, and a hard stop by
   that pid and start time after checking the image path. Nothing stops a process by image name.
   A watchdog asking the service's own endpoint for readiness lands only after a hang is seen.
   Development builds neither raise nor check `state/version`.
10. **Starts carry a reason, and Quit holds** (amends ADR-0197 Decision 11; owner ruling). Lands in
    2.0, frozen. Every start goes through `relay::start_service(reason)`, which starts `serve`
    detached and windowless and passes it no handles at all: on Windows `CreateProcessW` with
    handle inheritance off, so no pipe, file or socket the starter happened to hold can keep a
    caller waiting (2026-10-14: a deploy piped through `grep` never ended, because the service
    held the pipe it inherited from the installer, which had inherited it from PowerShell); on
    Unix every descriptor above 2 is close-on-exec and 0-2 are `/dev/null`. A test starts the
    service from a parent that holds an inheritable pipe and asserts the pipe reaches end of file
    when the parent exits. There is no sign-in entry; the service starts as soon as an edge comes
    up:

    | Starter | When | Reason | After Quit |
    |---|---|---|---|
    | MCP relay | at its own start, and for each held request | agent | starts it |
    | Browser relay | once, at its own start; never while waiting | browser | refused |
    | `call`, when no service answers | the command needs the service | command | starts it |
    | `open` or a bare launch | no service answers | person | starts it |
    | The shell, after a loss without goodbye (when the shell lands) | shared backoff | recover | refused |
    | The shell, launched by a person, when no service answers (when the shell lands) | no service answers | person | starts it |
    | The installer | after a swap | update | starts it |

    `status`, `doctor`, `help` and `--version` never start it. Starting with the first edge lets
    the service load, fetch policy and finish the extension handshake and page assets before the
    first request, so the person never waits. Quit writes `state/quit`. While it exists a browser
    or recover start is refused, and any other start deletes it. After Quit, a Chrome restart does
    not bring Ghostlight back; the next agent session, a command that needs it, or a person does.
11. **Presence** (supersedes ADR-0127 Decisions 1 and 3; amends ADR-0119). Lands with the shell,
    because idle exit would remove an in-process tray; until then ADR-0127's desktop authority
    startup stands. Whatever started it, the service brings up the shell when a display is
    reachable and none attaches within 2 s. It relaunches a shell that vanished without goodbye at
    most three times in two minutes, and a shell whose build is out of date. The promise is
    "visible whenever the desktop can show it": with no display or webview runtime the service
    still serves, and `status` and `doctor` say why there is no window. No option makes a
    tray-capable desktop tray-less while Ghostlight serves.
    - One shell runs at a time, by `run/desktop.lock`; a second launch asks the service to `show`.
    - On a goodbye the shell acts as Decision 5 says; after 60 s on Updating it shows the stopped
      state with Start. With no goodbye it shows Restarting and calls `start_service(recover)` on
      the shared backoff, three tries, then shows the stopped-unexpectedly state.
    - Window behavior moves into the shell unchanged: close destroys, Open creates or focuses, and
      renderer loss discards only the failed view (ADR-0119 Decisions 2 and 3, ADR-0120).
    - On Linux the service launches the shell from its own environment. If a client filtered the
      display, the Applications entry, launched by the desktop session, attaches a shell instead.
12. **Idle, crash, Pause and Stop.** Idle exit lands with the shell: after 10 minutes with no
    stream of any role, the service exits, and an attached shell is a stream. Pause and Stop are
    service state the shell only shows (ADR-0126). Crash handling holds in 2.0: MCP relays answer
    cut-off calls with "may have happened; check before repeating" and resume by token (ADR-0197
    Decision 12), and browser relays exit on the next accepted opening, so the extension
    reconnects with a fresh hello.

### Every process honors the operating system

These rules bind the service, the edge in each role, the shell and every one-shot process (owner
ruling). They land in 2.0 in the edge and the service, and in the shell when it lands.

13. **Never block or delay the end of a session, and start nothing once it is ending.**
    - Every process answers `WM_QUERYENDSESSION` with TRUE at once. Nothing calls
      `ShutdownBlockReasonCreate` or takes a logout inhibitor. On the final signal
      (`WM_ENDSESSION` with TRUE, `CTRL_LOGOFF_EVENT`, `CTRL_SHUTDOWN_EVENT`, SIGTERM, or the
      session manager's end), a process arms its fallback exit first (ADR-0180's 2026-09-25
      amendment), then stops, and is gone in under 2 s. Every service write is crash-safe, so only
      writes in flight finish. Restart Manager's `ENDSESSION_CLOSEAPP` asks the same of one process.
    - The ending state begins at the first signal, the query included, and lifts only if the OS
      cancels (`WM_ENDSESSION` with FALSE). While it holds, `start_service` refuses, presence
      relaunch and recover are off, and the service starts no child.
    - Session end is the fourth goodbye reason: the service sends `goodbye{session_end}` on every
      stream before it exits. Each edge and the shell also listen themselves, because the service
      may already be gone, and on either one they exit without restarting anything.
    - The rule: once the session is ending, nothing Ghostlight owns starts a process.
14. **One listener, shared by every binary,** in `relay`'s `os` module.
    - Windows: a hidden top-level window on its own thread, never a message-only window, which
      gets no broadcasts, plus `SetConsoleCtrlHandler`. A console process that loads user32 or
      gdi32, as the listener does, receives `WM_QUERYENDSESSION` and `WM_ENDSESSION` instead of
      `CTRL_LOGOFF_EVENT` and `CTRL_SHUTDOWN_EVENT`; the handler still carries `CTRL_CLOSE_EVENT`
      and Ctrl-C. It returns FALSE after close, logoff and shutdown, so the default exit is never
      suppressed. Only the edge claims Ctrl-C and Ctrl-Break, so that in a hand-over it outlives
      the child, which shares the console and gets the same signal, and returns the child's exit
      code. In the shell, the toolkit drops exit retention once the session is ending (ADR-0180
      Decision 2).
    - macOS: SIGTERM for the service and the edges; the shell answers `applicationShouldTerminate`
      with `NSTerminateNow`.
    - Linux: SIGTERM and SIGHUP for every process; the shell also honors its session manager.
15. **Sleep and resume are not shutdown.** Every process survives them. On resume, relays reconnect
    on the shared backoff (0.5 s doubling to 30 s), and nothing spins.
16. **The rest the OS asks for.** Notifications go through the OS notification system, so Do Not
    Disturb and Focus apply. The shell's webview honors reduced motion and forced colors.

### Install and update

17. **Install** (amends ADR-0197 Decision 14). Lands in 2.x. In 2.0, `ghostlight deployment
    install` stays, with the new names and today's rename-swap; the exact-image stop covers 1.x
    images and earlier 2.0 development layouts inside the break migration. In 2.x
    `ghostlight install` is the only writer of `bin`, and `deployment install` folds into it.
    `bin/install.json` records the version, the channel, the edge's relay revision, the shell's
    build and file hashes.

    ```
    take run/install.lock; read bin/install.json
      older payload: refuse (a carrier then runs the installed edge)
      same version, intact: reconcile registrations only
      development installed: refuse unless --development or --restore
    stage and verify into bin/.staging
    write bin/swap.json; rename each changed file aside into .retired, then the new one in
      the service always; the edge when its relay revision changes; the shell when its build does
    reconcile registrations (read first; inside a container, skip them and print the command)
    send quit{replace}; at 10 s, stop by pid and start time after checking the image path
    start_service(update); wait up to 30 s for an accepted opening
    write install.json; delete swap.json; release the lock; delete unmapped .retired files
    ```

    Registrations come before anything stops, because a browser relaunches its host the moment the
    old one exits. If power fails between two renames, `swap.json` remains and `start_service`
    starts the staged image, which finishes the swap. There is no rollback. The service reconciles
    registrations whenever it starts outside an app container, so a half-finished container install
    heals.
18. **Development is a channel** (supersedes ADR-0197 Decision 15 and Decision 13's development
    directory and marker). Lands in 2.x. It is not a frozen concern, because
    `relay::start_service` starts only `bin`. 2.0 keeps ADR-0197 Decisions 13 and 15 for
    development. In 2.x `scripts/dev-loop.ps1` builds in an isolated target and runs
    `<target>/ghostlight install --development`, which installs into `bin` and records the channel.
    Release carriers then run that build and never install over it. `install --restore` is the one
    way back and the one allowed move down. `~/.ghostlight/dev`, the marker and the forwarding rule
    are deleted.
19. **Carriers and containers.** Lands in 2.0 with the new names. ADR-0197 Decisions 13 and 16 and
    both 2026-10-14 amendments stand. A carrier installs when the installed set is absent or
    older, then runs the installed edge, and never serves. The npm launcher maps no arguments to
    `mcp`. The WinGet alias is a carrier edge. MCPB's final release carries 2.0.0. Inside an app
    container, `install` writes the home, skips every redirected registration and prints
    `%USERPROFILE%\.ghostlight\bin\ghostlight.exe install` for a regular terminal (the container
    rule of the ADR-0197 amendment); there is no Explorer hand-off, WMI, Task Scheduler or COM
    activation. A service started inside a container ends when that app quits. When the shell
    lands, only the shell needs WebKitGTK, nothing refuses to install without it, and the WebView2
    data folder is `~/.ghostlight/state/webview`.
20. **Uninstall** stays as today: `ghostlight uninstall` removes the registrations Ghostlight wrote
    and leaves the home. It is refused inside a container, as today.

23. **The frozen extension opens a page only when the service is absent** (owner decision 3,
    2026-10-15; amends the extension-first half of ADR-0070 Decision 1). The extension opens a page
    only when its first connection after an install finds the service absent (Chrome reports the
    native host as not found). It never opens one on an update, when a connection works, or on any
    other failure. Chrome reports a synced arrival as an install too, so a synced profile on a
    computer that already has Ghostlight opens nothing.

    Owner answer of 2026-10-15: the page is the extension's own setup page,
    `chrome.runtime.getURL('setup.html')`, which loads nothing from the network. The trust pages say
    that no vendor page opens. The setup page links to the documentation on sylin.org, which opens
    only when the person follows the link.

    The choice holds for 2.x, because the extension freezes with 2.0. The install journey's ADR
    cites this decision and does not amend ADR-0070 Decision 1 again.

### Guarantees

21. **Guards and tests for 2.0** (amends ADR-0197 Decision 25).
    - The frozen source lock covers `relay`, the edge and the extension's shipped files. Unit tests
      of the frozen crates live outside the locked bytes.
    - Dependency direction: `relay` and the edge depend on no other workspace crate. When the
      shell lands, `orchestrator` depends on no UI toolkit.
    - The network-crate ban holds on the edge and `relay`. Once the shell lands it holds on the
      desktop's tree too, with the single exception of `http` (request and response types for the
      webview's custom-scheme and IPC callbacks; no I/O and no socket).
    - Unsafe code appears only in `win-peer` and `relay`'s `os` module.
    - A shutdown-message test: a Windows test sends `WM_QUERYENDSESSION` and `WM_ENDSESSION` to
      each binary's hidden window and requires TRUE, an exit within the budget and no child
      started; SIGTERM tests do the same on Unix. A unit test proves the ending state suppresses
      `start_service`.
    - There is no source scan for "only `start_service` starts the service": ADR-0197 Decision 24
      bans source-text tests.
    - After 2.0 is published, CI runs the published edge and extension against the new service.
      Peer checks on Windows 10, Windows 11 and inside both app containers are live checks of the
      release bar.

### Timing

22. **2.0.0 waits for the frozen set only** (owner decision 1, 2026-10-15; amends the draft of this
    decision, which waited for everything). The frozen set is: relay protocol 4, the edge and the
    names, the start and goodbye reasons, the quit latch, the session listener, adapter protocol 4
    and the 2.0.0 extension, audit line v2, policy schema 4, the model surface
    (`docs/1.0/catalog.json`), the home layout and names, and the one break migration.
    - The 2.0.0 extension includes the install journey's extension pages: the setup page, the
      toolbar popup and the options page with their scripts, `lib/setup-view.js` and
      `lib/popup-view.js`, the badge colours in `lib/state.js`, the worker's show request, and the
      greyed toolbar icon only if the owner approves it. It also carries the one kit stylesheet,
      `extension/kit.css`.
    - Adapter protocol 4 includes the install journey's `show_requested` event.
    - The one break migration includes recording setup as finished for a 1.x home and retiring the
      1.x handoff marker.
    - So the 2.0.0 tag waits for the install journey's frozen half (its slices j02 to j09), which
      lands after the desktop shell track. The shell and the journey's unfrozen half (the window's
      setup view, the install output, the get scripts' output, the documents) may land before the
      release when they are ready. The release follows the owner's quality bar, not a date: nothing
      ships half-baked.
    - The number stays 2.0.0 (ADR-0197 Decision 23): anything pinned to `ghostlight@^1`, or
      resolving the latest 1.x, must not silently receive a break.

## Consequences

- When the shell lands, Tauri and webkit2gtk leave the service, which then starts with no display,
  over SSH, in CI and in WSL. The ADR-0166 failure class goes, with its custody code.
- Commands behave like commands on Windows 10 and 11 from 2.0, through the edge's console, so the
  ADR-0197 Consequence about the GUI-subsystem command line no longer holds.
- Each typed command in 2.0 is a one-shot process and pays one process start. Moving commands
  inside the running service in 2.x changes no edge.
- Shell era: a broken window never stops agent work, and desktops run one more resident process.
  The workbench must be re-proven in the shell: Windows window behavior, Wayland reconstruction
  (ADR-0120), one window through Open bursts (ADR-0119), and shutdown (ADR-0180).
- Every registration is renamed once, inside the break migration. Hand-written MCP entries that
  name `ghostlight-mcp-connector`, as `llms-install.md` suggests today, stop working; `doctor` names
  each, and Fix rewrites it (ADR-0154).
- Quit holds across Chrome restarts. With no sign-in entry, the tray appears with the first edge.
- A service started inside an app container ends with that app. Shell era: on Linux, a service
  started by a client that hid the display shows no window until the person opens the Applications
  entry.
- A SIGTERM sent to the service alone ends its streams as a session end does. Quit is the way to
  stop Ghostlight and keep it startable.
- The 2.0.0 tag waits for the install journey's frozen half, and for nothing in the shell or the
  journey's unfrozen half.
- The frozen extension opens no vendor page, so the trust pages' claim of no vendor-bound traffic
  holds at install as well.
- MEMORY's "One desktop authority startup" and "A held lease is not readiness" are rewritten when
  the tree changes, and ARCHITECTURE.md, DEV-LOOP.md, README and `llms-install.md` follow the
  tree.
- macOS stays deferred. Its row: `Ghostlight.app` (LSUIElement) under `~/.ghostlight/app` holds the
  shell, and signing and notarizing are prerequisites the project does not have yet.

## Migration

The seams lane ([seams ledger](../tasks/break-2.0/seams.md)) lands the 2.0 decisions in this
order, each step one or more reviewed commits on dev that leave a green tree and update the
documents they make stale:

1. the removals;
2. the catalog golden;
3. policy schema 4;
4. audit v2;
5. the refresher;
6. private files on Linux;
7. the protocol-4 trims;
8. relay protocol 4;
9. the edge and the carriers;
10. the journeys;
11. the pristine pass;
12. the guards and the version;
13. the current-state documents.

Then, in order:

- the desktop shell track (Decisions 1, 3, 4, 7, 8, 11 and 12 for the shell);
- the install journey track; the 2.0.0 tag waits for its frozen half (Decision 22);
- the owner's release bar: clean installs on Windows and the Linux test machines, a 1.x upgrade on
  another machine, and a week of daily use with no shutdown block and no second service.

The 2.x items follow the release: commands inside the running service (Decision 6), the install
fold (Decision 17), the development channel (Decision 18), `state/version`, `service.json` and the
hard stop (Decision 9).
