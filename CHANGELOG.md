# Changelog

All notable changes to Ghostlight are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Alpine and other musl systems install Ghostlight from the release. It carries
  `x86_64-unknown-linux-musl` images: the edge and the service static, the window built on Alpine
  against WebKitGTK 4.1. `get.sh` and the npm launcher pick them on a musl system, and `get.sh`
  says what the window needs when its libraries are missing. The release workflow builds them in
  Alpine and installs the candidate on Alpine with Alpine's Chromium.

### Changed

- `browser_dialog` status says what a prompt's field holds, and that accept answers with it, as OK
  does, so an agent can accept a prompt without asking. The text is never audited. An extension
  older than 2.0.1 does not report it; the next step still says what accept answers with.
- Every `browser_tabs` and `browser_window` action says what it does: focus, close and resize
  had no description.

### Fixed

- A client that fills in a tool's advertised defaults before it sends a call, as Claude Code began
  to, is no longer refused for opening a new tab, going back or forward, or scrolling to a target. A
  field sent with its default means the same as leaving it out, and a self-check proves every
  example and known-good call is accepted with its defaults filled in.
- On Windows, Ghostlight no longer stops a shutdown, restart or sign-out. A hidden window that the
  window toolkit (Tao) creates answered Windows' question whether the session may end with no, so
  Windows listed `ghostlight-desktop.exe` as preventing shutdown. Tao 0.36.0 answers yes.

### Security

- TLS certificate checks take rustls-webpki 0.103.16 and webpki-roots 1.0.10, for RUSTSEC-2026-0382
  and RUSTSEC-2026-0381 (a distrusted root). The organization-policy fetch moves to ureq 3, which
  uses those roots directly, so no advisory is ignored.

## [2.0.0] - 2026-10-19

Ghostlight 2.0.0 is a breaking release. Update the service and the Ghostlight in Browser extension
together, because neither works with the other's 1.x version. A 1.x install moves to the 2.0
layout once, when you run the install again. The full release notes are in
[docs/release/notes-v2.0.0.md](docs/release/notes-v2.0.0.md).

### Added

- The Ghostlight window, `ghostlight-desktop`. It opens on the setup steps and then shows the
  workbench: activity, history, policy and Settings. `ghostlight open`, the Start menu entry on
  Windows and the Applications entry on Linux bring it back. The tray icon is there whenever the
  desktop can show one, and it shows ready, working, paused or needs attention (ADR-0199).
- The setup view, in three steps: This computer, Your browser and Your agent. It moves to the next
  step on its own. The Browser step lists the Chromium browsers installed on the computer: Add opens
  the Chrome Web Store listing in that browser, and Open starts a browser that already holds the
  extension (ADR-0200 Decisions 1 and 8).
- The install ends by saying the version, where Ghostlight lives, which agents have it and one next
  step, then opens the window on the setup steps. `--no-open` leaves the window closed
  (ADR-0200 Decision 12).
- One-line installers that need no Node.js: `irm https://sylin.org/ghostlight/get.ps1 | iex` on
  Windows and `curl -fsSL https://sylin.org/ghostlight/get.sh | sh` on Linux. With
  `GHOSTLIGHT_VERSION` set they install exactly that release, a prerelease included (ADR-0200).
- The `ghostlight` command on your PATH. On Windows the install adds `~/.ghostlight/bin` to your
  user PATH (never the machine PATH), so terminals opened afterwards find it. On Linux it links
  `~/.local/bin/ghostlight` and writes a manual page and bash, zsh and fish completions under
  `~/.local/share`; when `~/.local/bin` is not on the PATH the install ran with, it names the full
  path to run instead. Uninstall removes them.
- The service serves the workbench on loopback HTTP, on 127.0.0.1 only, to the window and to
  browser tabs on the same computer. "Others can watch" is on by default: anyone on this computer
  can open that address in a browser tab, see activity with hosts, client names and policy rules,
  and pause agents; every other change needs the window. Settings > Sharing turns watching off,
  and `ghostlight status` shows the address (ADR-0199 amendment of 2026-10-15).
- Each installed agent shows its own app icon in the window, on a tile the service picks so the
  icon reads on the dark page, and otherwise a vendor mark or a monogram. The icons are read from
  the agents' app files on this computer and kept in `state/icons`; nothing is fetched.
- Each agent connection names the app that started it, read from the operating system and never
  from what the client says: the window and the history say, for example, "Started by Claude
  Desktop." beside the name the client gives, and audit lines carry `provenance.source`. It is
  never used to decide what an agent may do (ADR-0161 amendment of 2026-10-18).
- Settings > This screen offers Text size, Standard or Large.
- `ghostlight policy migrate <file>` moves a schema-3 policy file to schema 4 only where the meaning
  stays the same. It keeps a `<file>.schema3.bak` copy and refuses anything it cannot move without
  changing what the policy does (ADR-0197 Decision 19).
- A policy can turn a channel off. `channels.mcp.enabled` and `channels.cli.enabled` set to `false`
  refuse every request from MCP agents or from the command line before any browser work, and the
  refusal says which switch is off and where. Unset means on. `policy migrate` carries both keys, and
  the window's Policy page shows them with the layer that set them. The channel is declared by
  Ghostlight's own peer, so a switch is a guardrail for well-behaved agents, not a barrier against
  software running as the same user (ADR-0202 Decision 22a).
- Handles follow their control. A handle survives a re-render that the page proves is the same
  control, and the action reports `rebound: true`. A stale handle says why: `removed`, `navigated`,
  `tab_closed` or `unknown`. When a role and name repeat, `browser_find` and `browser_inspect` items
  carry `context`, their row or group, and `nth` when that repeats too (ADR-0198 amendment "handles
  follow their control").
- Every tool accepts `timeout_ms`, from 100 to 30000 milliseconds (ADR-0197 Decision 21).
- Form controls say what they accept. `browser_inspect` and `browser_find` items carry a control's
  value and, for a native select, its options. A fill matches a choice exactly, then by one
  normalized match, and says when it did; a multiple select takes a list. A fill that cannot take
  its values refuses before any edit and lists what would succeed. A fill says what the page now
  holds, and fields the page changed back fail as reverted (ADR-0198 Decisions 7 to 11).
- An action that completes with a dialog open says so, carries a `dialog_opened` fact and points to
  `browser_dialog` (ADR-0197 Decision 21).
- The install scripts install from a local copy of a release. With `GHOSTLIGHT_FROM` set to an
  unpacked release folder, `get.sh` and `get.ps1` check every file against its `SHA256SUMS` and
  fetch nothing from GitHub.
- Search in the window (Ctrl+K) finds activity by its words, its host and its agent, and reaches
  history beyond the 199 rows the list shows.
- The page shows fills, uploads and scrolls on their element: a splash on each field a fill set
  and on an upload's input once the files are attached, and a scroll to a target draws its cue on
  the element once the page has scrolled to it.

### Changed

- One service per OS user, `ghostlight-service`, owns the MCP server, the tools and their words,
  governance, the audit trail and every state change (ADR-0197 Decision 17, ADR-0199 Decision 9).
- `ghostlight` is the edge: the one executable an MCP client or a browser starts. It relays to the
  service and starts it when it is missing, and decides nothing. The edge and the relay are version
  4.0.0, the relay revision, and change only with their protocol (ADR-0199 Decisions 5 and 8).
- Everything lives under `~/.ghostlight`. Its `bin` folder holds `ghostlight`,
  `ghostlight-service` and `ghostlight-desktop`, and an update replaces them as one tested set
  (ADR-0197 Decisions 10 and 13).
- Launching a registered MCP client or browser starts Ghostlight when it is not running. After you
  Quit, only an agent, a command or opening Ghostlight starts it again (ADR-0199 Decision 10).
- A service with nothing connected to it exits after 10 minutes. The window counts as connected, so
  this ends a service on a computer with no desktop, such as one an agent started over SSH
  (ADR-0199 Decision 12).
- The service and the extension speak adapter protocol 4. A 2.0 service tells a 1.x extension to
  update and closes the connection, and a 2.0 extension tells you to update Ghostlight when it meets
  an older service (ADR-0197 Decision 5, ADR-0200 Decision 16).
- The extension is version 2.0.0. It carries no policy and no page code:
  the service gives it the page code when it connects, so a service update needs no extension
  update. The page code lives in the service's browser adapter, under
  `crates/orchestrator/src/browser/adapter/chromium/protocol4/assets` (ADR-0196 Decision 3,
  ADR-0197 Decision 5).
- The popup shows Ready, Paused, Session ended, Not connected or Update needed. Paused and Session
  ended show the held lantern, as the tray and the window do, so neither looks like Ready. While
  connected, a quiet Open Ghostlight link in the popup and on the options page brings the window
  forward. Captions no longer depend on the effects setting, and the setup page's install command
  never wraps. The extension opens its setup page only when the service is absent (ADR-0200
  Decision 14 and its amendment of 2026-10-18, ADR-0199 Decision 23).
- The window, the tray and `ghostlight status` say the same thing in the same words: one count of
  agents and browsers ("Ghostlight is running; 3 agents and 1 browser connected."), Pause agents
  and Resume agents, and a Status page that answers Ghostlight, Browser, Rules and History in plain
  words. A pause or an ended session outranks having no browser, and a browser that starts on
  request reads Ready ("Starts Google Chrome when an agent asks."). The tray's tooltip names the
  state and says when no browser is connected (ADR-0199 and ADR-0187 amendments of 2026-10-18).
- The window is easier to read and to follow. Every word meets the kit's contrast, labels use one
  eyebrow and one data tag, and a screen reader hears each finished action once. A failed or
  unconfirmed action says Failed or Unknown in words. An activity row names the most consequential
  capability it used, and a flow takes its steps'. The window fits half a laptop screen: it opens
  no taller than the screen, can be as narrow as 720 px, and has a dark title bar in light mode.
- With several tabs in a session, a call that names no tab is refused before any browser work
  with `ambiguous_tab`, and its `tabs` fact lists each tab with its handle, title and url. A call
  without a tab no longer lands in the tab another subagent sharing the same connection opened or
  focused last. A session with one tab, and `browser_tabs` list, need no tab, so the server
  instructions tell an agent that may share the session to open its own tab with `browser_navigate`
  and `new_tab`, and to pass that tab on every call (ADR-0198 amendment of 2026-10-18).
- A control the page refuses, because it is hidden, disabled, read-only or takes no text, is
  refused in Ghostlight's words: what was refused, that nothing was clicked or typed, and one step
  that works. The audit keeps `page_refused` with the page's code (ADR-0198 amendment of
  2026-10-18).
- `ghostlight uninstall` stops the running Ghostlight and ends by saying what it removed and what
  it kept: the home `~/.ghostlight` stays, and deleting it removes the programs, the audit history
  and the settings; Ghostlight in Browser stays until you remove it from the browser.
  `--dry-run` lists every browser connection and the command entry it would remove (ADR-0199
  Decision 20).
- Ghostlight finds an agent's settings wherever the agent keeps them, including a Microsoft Store
  app's own copy below `%LOCALAPPDATA%\Packages`, on every look (ADR-0197 amendment of
  2026-10-17).
- Run from a Store app's terminal, such as Claude Desktop's or Codex's, `doctor` and the install
  say the browsers' registrations cannot be checked from there and to run `ghostlight doctor` from
  a regular terminal, instead of calling working browsers broken. A service started inside such an
  app leaves starting the browser to you, because a browser started there would run inside the
  app (ADR-0197 and ADR-0149 amendments of 2026-10-18).
- Policy files use schema 4. Ghostlight moves its own user policy the same way `policy migrate`
  does. Policy files an administrator owns are never rewritten; they fail closed in schema 3 until
  their owner moves them (ADR-0197 Decision 19).
- The managed policy keeps its HTTPS fetch, signatures and rollback protection. The fetch runs on
  the refresher thread, never on the path of a call (ADR-0197 owner review amendment).
- Audit lines are version 2 and frozen at 2.0. A reader ignores fields it does not know. The 1.x
  audit logs are moved aside under `state` and are not displayed (ADR-0197 Decision 22).
- A session works only in its own tabs. Ghostlight does not adopt a tab on its own: `browser_tabs`
  lists tabs opened from a tab the session owns, and the model may adopt one explicitly under the
  same governance as navigation (ADR-0196, ADR-0197 Decision 20).
- Handles are short: a prefix and eight random base32 characters, checked against the workspace that
  owns them (ADR-0197 Decision 21).
- A session holds one recording. `browser_record` takes no recording id, and a second start is
  refused until the first is saved or discarded (ADR-0197 Decision 21).
- `browser_inspect` and `browser_find` answer once the page has been quiet for 300 ms, within the
  settle budget (ADR-0198 Decision 31).
- The result a client reads leaves out fields at their default, and a reader treats an absent
  field as its default (ADR-0198 Decision 21).
- Results carry the envelope once, in structured content. The text block is the summary and next
  steps, and tool definitions carry no `outputSchema` (ADR-0197 Decision 17). Clients on an MCP
  revision before 2025-06-18 still get the result JSON at the end of the text block
  (ADR-0198 Decision 24).
- WinGet stays as a supported route (ADR-0197 Decision 16, owner review). Its Windows archive is
  built by `scripts/package-winget-archive.ps1` and is a carrier for `ghostlight install`, which
  installs into `~/.ghostlight/bin`; after `winget upgrade` the next install carries the newer
  version there, and an older copy never replaces a newer one. It is not offered as a separate
  portable download. WinGet carries 2.0 once its 2.0.0 manifest is accepted into winget-pkgs;
  until then `winget install Sylin.Ghostlight` installs a 1.x release.
- With several browsers installed and none open, an agent request starts the browser that last
  connected to Ghostlight, instead of asking you to open one. Ghostlight remembers only which
  browser that was, in `state/last-browser.json`. With no such memory it still names the
  browsers you can open, and `Auto-open browser on request` off still never starts one
  (ADR-0149 amendment, 2026-10-17). The same holds for an agent session whose browser you
  closed: its next request starts that browser again, never another, and continues there once
  it is back (ADR-0196 Decision 12 amendment, 2026-10-17). Agents asking at once, or retrying
  while that start is still opening, get one browser window, not one each.
- The Chromium page-asset tests run with `node --test` and need no `npm install`. CI runs them
  beside the extension tests.
- The window rests in ink. Teal marks only work that is running, the focus and the one main action
  of a view: the Ready light is grey at rest, held and waiting work no longer look live, a row says
  Waiting or Held, and the band counts only running work.
- A switched-off channel shows everywhere: the band says Turned off, the tray, Status and Policy
  name the switch, and the refusal says it in your words ("Your policy turns off work from MCP
  agents.").
- The window keeps its meaning at every width. The band drops whole facts instead of cutting
  words, rows keep the agent's name at half-screen widths, and the Full control chip is gone; it
  reads Shared while watching is on.
- One word per act: "Start a new session" in the band, the tray and the popup. Detail cards say
  what happened in plain words, and rows recorded before agents were named say "Earlier session".
- Integrations shows each agent with its app icon and its words, without a connection drawing.
- On a light Windows taskbar the tray shows the mascot with a soft outline, keeping its face at 16
  pixels.
- On the Policy page every switch has an accessible name, "Require audit" is now "Stop browser
  work", a watching page cannot edit, and page effects and captions show as set in the browser.
- The popup rests in grey and shows the waiting lantern while no agent is working. The setup
  page's install command fits at every width, and the options page's switches look like the
  window's.
- After a browser restart or an extension reload, a tab a session remembers is confirmed by the
  browser before it is used. A tab that now shows another page reads as no longer open, and the
  agent opens it again. An agent's tab survives a Ghostlight update (ADR-0203 Decision 1).
- When Ghostlight quits, it releases every session's tabs, command-line sessions included, so none
  keeps the debugging bar, and the extension releases the tabs a crashed service held when the
  next one connects. An extension too old for the service says to update Ghostlight in Browser
  (ADR-0203 Decision 3).
- Closing a tab needs Action on its host. `browser_tabs list` lists a tab on a host the policy
  refuses to read without its title or address and names that host, and in a session with no tab
  it answers at once and starts no browser (ADR-0203 Decisions 2 and 7).
- Refusals say what is true: a handle that names another of the session's tabs, a fill across two
  tabs, a point outside a screenshot, a held tab (which names the refused host and how to leave
  it), and a duration wait cut short, which sent nothing (ADR-0203 Decision 5).
- The Policy page, the catalog and enforcement give one answer about where a capability can be
  used, the preview honors the channel switches, and a never-touch refusal names the layer that
  set it (ADR-0203 Decision 7).
- History says "needs you" for a credential refusal, and the tray offers Resume only while paused.
- Updating Ghostlight keeps an agent's own settings in its configuration file: disabled,
  approvals, tool filters and environment (ADR-0203 Decision 10).
- When an action opens a dialog, the agent is told to read it with `browser_dialog` status before
  it answers; status gives the dialog's kind and message.
- After a Ghostlight update, a running agent is told to list its tools again, so a tool a newer
  service or a changed policy adds reaches it.
- The Claude Code plugin and Ghostlight's own entry together give an agent one set of tools: a
  second connection from the same agent waits with none and takes over if the first ends. The 2.0
  plugin asks npm for Ghostlight 2.
- The install finds Claude Code's settings where `CLAUDE_CONFIG_DIR` points.

### Removed

- The `browser_workspace` tool. The catalog has 23 tools. A session can no longer list other
  sessions or switch into another session's workspace. A call to the old tool fails before any
  browser work and points to `browser_tabs` list (ADR-0196).
- The two connector executables, replaced by the edge (ADR-0199).
- The Windows setup program (NSIS), the Debian package, Scoop, the separate portable archive and the
  Homebrew template. The routes are `npx -y ghostlight install`, the one-line installers and WinGet
  (ADR-0197 Decision 16, owner review).
- The Flatpak route. The install removes the Flatpak connection a 1.x install left in your home and
  says what it did (ADR-0197 Decision 22).
- MCPB as a channel after this release. The Claude Desktop bundle in this release is the last one,
  and it carries 2.0.0.
- Observe mode, the user-policy enable switch and the frame options (ADR-0197 Decision 19).
- The `layout_stable` wait condition. A call that names it is refused with a pointer to
  `visual_settle` (ADR-0197 Decision 21).
- The popup's diagnostics toggle, the form-reset trace and the YAML editor for goose and Continue,
  which get a Copy setup action instead (ADR-0197 Decision 22).
- The folders 1.x left that 2.0 reads nothing from. Every install removes the 1.x window's web
  view data when it holds nothing else and the 1.x policy folder once it is empty
  (`%LOCALAPPDATA%\org.sylin.ghostlight` and `%LOCALAPPDATA%\Ghostlight` on Windows,
  `~/.local/share/org.sylin.ghostlight` and `~/.local/state/ghostlight` on Linux), and says each
  one it removed (ADR-0197 amendment of 2026-10-18).

### Fixed

- `browser_dialog` `accept` on a prompt answers with the text its field starts with, as OK does.
  It answered with an empty string, so a page's default answer was lost. `respond` still sends its
  text as given, empty included (ADR-0200, amendment of 2026-10-19).
- A captured screenshot handed back to a page (`browser_upload` with `source_image`, attached to a
  file input or dropped at a point) no longer fails with "file payload size mismatch". The stated
  size ignored base64 padding, so about two images in three were refused; this has been wrong since
  1.0.
- Ordinary page tools spend at most one second on optional preparation and then go ahead on a busy
  page; `visual_settle: false` skips it. A composite wait keeps a condition that held when optional
  settlement runs out and reports `visual_settled` apart (ADR-0195, prepared for 1.3.15).
- The service supplies the page code, so page-code fixes reach the installed extension at once.
  Choosing a select option by its visible label is now confirmed as retained (LU-2).
  `browser_read` masks every credential-class field, not only `type=password`, reports checkboxes
  and radios as checked or unchecked instead of their value attribute, and never shows a
  placeholder as a value (EXT-7, HM-7). `browser_find` matches visible text in `div` containers
  and in `[draggable]`, `[aria-label]` and `[tabindex]` elements, and prefers the deepest match
  (LU-6). Only the returned matches get handles.
- A tab handle from another session now points the model to its own `browser_tabs` list
  instead of suggesting a switch.
- A refusal for another session's handle names that session only by its client label. When the
  client reported no usable name, the refusal is the generic one instead of showing the other
  session's internal workspace id.
- Each connection keeps one workspace for its life. A switch could leave the original
  workspace and its tabs open until the service restarted; that path is gone.
- Default reuse no longer takes over a tab you opened. A plain open reuses only a tab Ghostlight
  opened on that host and a released session of the same client left open, after checking the
  live browser, and the browser never picks a tab itself. A tab another client released is not
  reused. A released tab is reusable only after the preserve-tabs setting kept it open, so the old
  session's cleanup can never touch the new session's tab (ADR-0196).
- Saved tab groups no longer pile up in the bookmarks bar. Chromium saves a tab group that is still
  open when its window closes or the browser exits. A tab that preserve-tabs keeps open after its
  session ends now leaves the `Ghostlight - <client>` group, so Chromium deletes the idle group
  instead of saving it, and when the same client reuses the tab it goes back into that client's
  group. A tab you moved into a group of your own stays there. A session still open when its
  window closes or the browser exits leaves one saved group, which no extension can reach;
  Ghostlight never touches saved groups (ADR-0201).
- The Windows one-line installer reads a real release's `SHA256SUMS`. GitHub serves that file as
  bytes, and the script split them into one-byte lines and refused every install before
  downloading anything. Run with `irm | iex`, it now leaves your PowerShell session as it was.
- An agent's configuration write leaves no temporary file beside the agent's settings when it
  fails, and an install removes such files an earlier failure left.
- `ghostlight doctor` lists the window program, `ghostlight-desktop`, with the other two.
- An install reads the browsers' profile lists only while setup is incomplete, as the trust pages
  say.
- Chrome and Edge tabs with the same id no longer refuse each other.
- A refusal found after the browser already opened, navigated, focused, or closed a tab reports
  the applied effect and is no longer marked safe to repeat.
- Anchor links and `history.pushState` routes no longer expire target handles. The tab follows
  the new address, and landing governance still checks it.
- A call no longer fails while its browser is briefly away. When the browser is reconnecting,
  still starting up, or the service has just restarted, the call waits up to about seven seconds
  and then goes ahead. A session's own browser reconnecting is no longer reported as "a browser
  profile that is not connected", and a service update no longer opens a new browser window
  (ADR-0196). A browser closed longer ago than that is not waited for, so Ghostlight still starts
  it at once where browser startup is allowed.
- One slow answer no longer cuts off a healthy browser. A short deadline, such as the 500 ms
  delivery of a control signal, no longer marks the browser unavailable or fails other calls in
  flight.
- Browser refusals say what is true: the browser stopped answering, is not connected, is not
  connected while a different browser is, or the connection was busy. "Reconnect the Ghostlight
  browser adapter" is gone, and a bridge contract failure no longer says "disconnected".
- Process diagnostics log when a connected browser stops answering and when it answers again
  (`heartbeat_lost`, `heartbeat_resumed`).
- A call made while Ghostlight quits gets an answer instead of a closed connection.
- A disabled copy of the extension no longer counts as installed, so the advice names the step
  that helps.
- A process diagnostics record is never cut off when the service exits.
- An install of the same version puts back a missing or changed window program.

### Security

- The workbench server binds 127.0.0.1 only and answers only when the `Host` header is its own
  address. Control needs a key made at each service start that only the window holds, and
  governance refuses agent navigation to the workbench's own address whatever the policy says
  (ADR-0199 amendment of 2026-10-15).
- On Linux, the audit log, the policy files and the other state files are created readable by
  their owner only (mode 0600, folders 0700).
- The audit keeps only what may be retained. A fill whose submit raises a dialog, or whose
  expectation falls short, writes no field value, and a refusal never writes another client's name
  (ADR-0203 Decision 4).
- On Windows, the managed-policy signing seeds, a new audit log, the agent locations file and an
  owned user policy are created readable by their owner and SYSTEM only (ADR-0203 Decision 11).

## [1.3.15] - Withdrawn

Prepared and submitted for staged Chrome review on 2026-10-05, and withdrawn on 2026-10-16 without a
public release, in line with no 1.x patch releases. Its bounded preparation, document validation and
wait changes (ADR-0195) ship in 2.0.0; its adapter compatibility with service 1.3.14 does not.

### Fixed

- Give read, inspect, find, screenshot, click, scroll, hover, fill, typing, key, drag,
  upload and script tools one bounded preparation wait. Busy pages remain usable after
  at most one second. `visual_settle: false` skips optional preparation.
- Validate targeted work against its required documents and ancestry. Unrelated frame
  navigation and same-document routes no longer interrupt permitted actions. Required
  document replacement and excluded input still stop execution.
- Preserve a satisfied wait condition when optional visual settlement expires, with
  `visual_settled` reporting that separate result. Explicit visual waits retain their timeout.
- Ship this fix on the published 1.3.14 baseline. The unpublished browser/workbench trial
  remains separate. Adapter 1.3.15 retains compatibility with service 1.3.14 during rollout.

## [1.3.14] - 2026-10-02

### Changed

- Capture complete screenshots without document inventories or privacy masks, including visible
  embedded content. Full-open capture goes directly to the existing browser primitive.
- Add binary `browser.screenshots.enabled` policy permission for viewport, full-page, target, and
  magnified screenshots. A false value in either authority layer prevents capture and returns no
  image. Tab Read permission and separate frame text/action restrictions still apply (ADR-0194).
- Keep the public 1.3.12 browser adapter compatible. Its existing injector installs the current
  service-owned page runtime; no extension update or reload is required.

This release contains the screenshot fix on stable 1.3.13. The browser/workbench trial remains
outside this release. Recording source restrictions are unchanged.

## [1.3.13] - 2026-10-02

### Fixed

- Install npm and downloaded packages at permanent `~/.ghostlight/bin` paths.
  Verify all three binaries before replacing changed components. Preserve running
  unchanged connectors, history, policy and selected development installations.
- Migrate owned native-host and MCP registrations from the former version directory.
  The existing 1.3.12 browser injector continues loading the installed service's
  current page runtime. Ordinary service updates require no extension update.

This release contains the deployment repair on the published 1.3.12 baseline.
Unpublished browser and workbench changes are outside this release.

## [1.3.12] - 2026-09-30

This version includes the 1.3.11 source changes recorded below; service 1.3.11 was not published.
Compared with service 1.3.10, it also introduces the acknowledged page runtime and adapter protocol
3, one budgeted form-fill transaction, Chromium-only support, and the `rustls` security update.
Use the matching 1.3.12 service and Chromium adapter; service 1.3.10 and
adapter 1.3.8 do not support the new protocol. Chromium 125 or newer is required.

### Changed

- Keep browser feedback passive. Messages and animations remain, while interactive runtime
  controls belong in the Tauri workbench (ADR-0185).
- Return credential guidance for the current request without holding the session. Fill and
  typing accept `user_authorized_credentials: true` when the user explicitly authorized entry,
  including in an earlier instruction. The acknowledgement applies only to that request;
  configured policy and explicit human Pause/Stop still apply.
- Allow explicitly requested uploads and recording attachments to file controls with
  credential-like names.

### Removed

- Remove automatic session pauses after repeated policy refusals and the resulting review/resume
  workflow. Later permitted work can proceed immediately.
- Remove the injected attention dialog, its browser-page buttons, and the obsolete quiet-resume
  action.

### Fixed

- Keep page feedback from taking focus or intercepting pointer input even when its stylesheet is
  unavailable, and remove predecessor presentation roots during runtime replacement.
- Carry credential authorization through grouped fill, targeted typing, and focused typing,
  including final browser checks, without placing input values in receipts or diagnostics.

## [1.3.11] - 2026-09-26

### Added

- Install one immutable, SHA-256-bound page runtime through an acknowledged browser command before
  publishing a Chromium adapter connection. Existing and future out-of-process frames receive the
  same runtime through one debugger-lifecycle seam (ADR-0181).
- Derive Chromium capability negotiation from its executable handler inventory so the adapter
  cannot advertise an unimplemented mechanism.

### Changed

- Align the service and Chromium adapter at version 1.3.11 and adapter protocol major 3 for the
  breaking closed-command cleanup.
- Execute form filling as one deadline-bound physical transaction: all-field preflight, native
  input, debugger release, and one batch retention check before optional submission (ADR-0182).
- Require Chromium 125 or newer for immediate page-runtime installation in existing documents.

### Removed

- Remove the unused raw BiDi/CDP bridge surface and Chromium raw-CDP command path.
- Remove fire-and-forget preload injection and every runnable headless browser-test mechanism.
- Remove the partial Firefox adapter, native-host registration, packaging, CI, publisher, and
  support claims from the Chromium-only browser offering (ADRs 0183 and 0184).
- Remove a retired Ghostlight-owned Firefox native-host registration on the next install or
  uninstall while leaving foreign registration state untouched.

### Fixed

- Update `rustls` to 0.23.45 for RUSTSEC-2026-0285.
- Keep the adapter's form-fill work inside the orchestrator's remaining invocation budget instead
  of multiplying fixed stability waits by field count.
- Install the page runtime in out-of-process iframes before resuming their debugger targets.
- Make installed acceptance invoke every advertised tool, including live workspace discovery.

## [1.3.10] - 2026-09-25

### Fixed

- Honor Windows shutdown and logoff across the desktop authority, MCP connector, and browser
  connector with mandatory hidden top-level session listeners.
- Arm the independent 1.5-second exit guard before asking Tauri to exit, so a destroyed Tao event
  target cannot prevent system shutdown.
- Preserve Windows' default console-process termination and keep connector shutdown notification
  non-blocking when another exit reason is already pending (ADR-0180 amendment).

## [1.3.9] - 2026-09-18

### Changed

- `browser_read` now includes values from form controls (`input`, `textarea`, `select`) and `contenteditable` node text, aligning page text with visual presentation (ADR-0151 amended). Password fields remain strictly masked (`********`).

## [1.3.8] - 2026-09-17

Unified service release in version lockstep with Chrome adapter 1.3.8. Introduces
the Hub-and-Spoke browser adapter architecture and Mozilla Firefox integration
(ADR-0179). Chrome adapter 1.3.8 is published in the Chrome Web Store.

### Added

- Hub-and-Spoke browser adapter architecture supporting multiple browser engines
  through typed platform negotiation (`ghostlight/gecko` and `ghostlight/chromium`, ADR-0179).
- Mozilla Firefox WebExtension dumb shell (`extension-firefox/`) with Manifest V3,
  background event page, native messaging relay, tabs, scripting, capture, and window management.
- Multi-browser native host registration across Chrome, Edge, Brave, Chromium, and
  Mozilla Firefox (`ghostlight install`, `ghostlight check`, `ghostlight uninstall`).
- Desktop workbench UI indicators for multi-browser status and connected adapter state.

### Changed

- Unified service and Chrome browser adapter versions in strict lockstep (v1.3.8).

### Fixed

- Prevent CDP input dispatch freeze on native modal dialogs (`window.prompt`, `window.alert`,
  `window.confirm`) by racing click dispatch against dialog opening events.
- Repair desktop workbench UI grid track media query so `.row-browser` stays hidden on narrow displays (<1040px).

## [1.3.7] - 2026-09-15

Prepared candidate for service release v1.3.7. Compatible with Chrome adapter 1.1.4
through 1.1.8. Chrome adapter 1.1.7 is published in the Chrome Web Store; adapter 1.1.8
is in review.

### Added

- Add 24th MCP catalog tool `browser_workspace` with `list` and `switch` actions, enabling
  zero-argument discovery of admitted workspaces and dynamic live session rebinding (ADR-0175).
- Add autonomous visual and layout settlement defaults (`visual_settle: true`) to
  `browser_screenshot` and composite `browser_wait` conditions (ADR-0174).
- Add cross-workspace tab discovery attribution (`owner_workspace`) and actionable guidance
  when a tab handle belongs to another admitted workspace (ADR-0172).
- Add agnostic visual settlement heuristic in `GhostlightSensor` observing CSS transitions,
  bounding box geometry, and scroll dimensions with continuous spinner bypass (ADR-0173).
- Add typed `SettlePolicy` value object in bridge contracts for wire serialization and
  settle behavior control (ADR-0176).

### Changed

- Decompose monolithic orchestrator entry point `crates/orchestrator/src/main.rs` into five
  modular, single-responsibility CLI modules under `ghostlight::cli::`: `parse`, `setup`,
  `doctor`, `status`, and `desktop` with unified intent dispatch (ADR-0177).
- Introduce universal execution templates `with_authorized_tab`, `with_authorized_target`, and
  `with_authorized_optional_target` on `ApplicationExecutor`, standardizing target resolution
  and governance checks across all operation modules (ADR-0176, ADR-0177).
- Centralize catalog invariants and tool count (`CATALOG_TOOL_COUNT`) across language definitions
  and test suites, eliminating magic numbers (ADR-0176).

### Fixed

- Verify multiline contenteditable form input reliably in adapter content scripts.
- Ensure strict ASCII encoding and format validation across all repository and test surfaces.

## Chrome adapter [1.3.8] - 2026-09-16

Aligned Chrome Web Store extension version in lockstep with service release
(v1.3.8). Prepared for Chrome Web Store submission with modal dialog input race
resolution and native dialog event announcement.

### Fixed

- Resolve `dispatchClick` timeout when clicking targets that trigger synchronous
  JavaScript modals (`prompt`, `alert`, `confirm`).
- Race CDP mouse input against `Page.javascriptDialogOpening` so input dispatch
  completes cleanly upon dialog arrival without waiting on unanswerable CDP
  `mouseReleased` replies.

## Chrome adapter [1.1.8] - 2026-09-15

Prepared for Chrome Web Store submission with multiline contenteditable value
verification, visual and layout settlement sensor heuristics, and background
quiet-window stability. Service versions and browser permissions are unchanged;
the public adapter is 1.1.7.

### Fixed

- Verify multiline contenteditable element values reliably by inspecting innerText
  and textContent fallback, normalizing CRLF and lone CR to LF.
- Preserve form focus custody and keyboard descriptors during rapid sequential input
  across tab switches.

### Added

- Add load-sensitive and visual settlement sensor heuristics (GhostlightSensor) for
  layout stability, finite CSS transition awareness, and quiet-window convergence.

## Chrome adapter [1.1.7] - 2026-09-13

Replaced the pending 1.1.6 Chrome submission with the multiline correction. Google
review is pending with automatic publication after approval. Service versions and
browser permissions are unchanged; the public adapter remains 1.1.4.

### Fixed

- Preserve textarea line breaks during native form filling. Enter now carries the
  character payload Chromium requires.
- Normalize Windows and lone-carriage-return line endings consistently during
  textarea input and value verification.

## Chrome adapter [1.1.6] - 2026-09-13

Submitted to the Chrome Web Store, then cancelled and replaced by 1.1.7 after the
multiline defect was reproduced. Adapter 1.1.5 was the local investigation version;
1.1.6 carried the verified focus correction. Service versions are unchanged.

### Fixed

- Preserve controlled form drafts across tab switches while Ghostlight control is
  active, by emulating focus on retained tabs. Restore ordinary focus behavior on
  pause, stop, disconnect, and release.
- Recover controlled-tab ownership on explicit agent work after extension reload.
- Use native browser input for form replacement and semantic clicks, with complete
  keyboard descriptors and readback. Improve opt-in form diagnostics.

No new browser permissions. Compatibility remains service 1.3.4-1.3.6. Sites can
still reset unsaved drafts after Ghostlight control ends.

## [1.3.6] - 2026-09-12

Published as GitHub release `v1.3.6`, npm package `ghostlight@1.3.6`, and MCP Registry version
`org.sylin/ghostlight 1.3.6`. Chrome adapter 1.1.4 is unchanged and remains public in the Chrome
Web Store.

### Fixed

- Use one serving Ghostlight installation per OS user. Package and development
  launches share authority instead of splitting the connected browser from MCP.
- Preserve development selection through package setup and cold startup. Inactive
  package removal leaves the active browser registration intact.
- Retain history, diagnostics and physical policy paths across binary selection.
  Preserve the Linux system package's existing user history during migration.
- Recover CLI calls from stale runtime discovery without hiding startup failures.

### Changed

- Add durable development/release selection and selected-authority diagnostics.
- Adopt all three native binaries on the first development deployment, and support
  deliberate release restore through the development loop.
- Keep Chrome adapter 1.1.4 unchanged; it remains compatible with service 1.3.6.
- Refresh the bundled browser-control skill for the current 23-tool catalog and publish
  plugin metadata 1.0.1.

## [1.3.5] - 2026-09-11

Published as GitHub release `v1.3.5`, npm package `ghostlight@1.3.5`, and MCP Registry version
`org.sylin/ghostlight 1.3.5`. Chrome adapter 1.1.4 is public in the Chrome Web Store. Adapter
1.1.2 was submitted for Google review on 2026-09-08 and observed public on 2026-09-10. The owner
then rolled it back to 1.1.1 code, republished by Google as 1.1.3. Adapter 1.1.4 supersedes the
unsubmitted local 1.1.3 ZIP and was observed public on 2026-09-11.

### Fixed

- Keep human navigation and tab creation outside Ghostlight control, policy and activity;
  stale agent references silently and govern the next actual agent request (adapter 1.1.4).
- Preserve atomic document-bound target typing and name the destination of blocked agent work.
- Bound concurrent desktop startup, reap failed owned children, and publish discovery only
  after desktop readiness. Preserve retained-client recovery during executable replacement.
- Repair Linux portable extraction and retained-client package upgrades, and Windows installer
  native-host paths and deployment-lock ownership.
- Record real JavaScript dialog-opening events before optional before-unload handling, so the
  adapter reports and retains prompts until they are answered or dismissed (adapter 1.1.2).
- Preserve truthful script/flow effects, document-specific coverage, bounded diagnostics and
  audit, local process continuity, and human controls across the security-hardening journeys.
- Exercise npm's actual installed platform command instead of requiring symlink privileges on
  Windows. Repair the Foundry scripts' retired catalog inputs and malformed shell payload.
- End abandoned connector sessions when their exact spawning process exits while allowing healthy
  MCP clients to remain idle without a time limit.

### Changed

- Add explicit Flatpak Chromium activation for home-resident installations on default data
  roots, with one named bus permission and owned registration/removal. System-package
  Flatpak integration remains deferred; setup never restarts the user's browser.
- Forward live desktop-context variable names in owned Linux Codex configuration.
- Configure host and capability authority through policy; retire caller-supplied restrictions
  and flow dry-run. Use `browser_flow` in place of the retired `browser_sequence` tool (ADR-0162).
- Show grouped incremental action history and expandable action details from At a glance.
- Add real installed Windows native-host recovery, retained MCP/native connection checks, and
  all-catalog browser acceptance. Define the outstanding clean-package and visible Linux gates.

## [1.3.4] - 2026-09-05

Published as GitHub release `v1.3.4`, npm package `ghostlight@1.3.4`, and MCP Registry version
`org.sylin/ghostlight 1.3.4`. Chrome adapter 1.1.1 is public in the Chrome Web Store.

This release also includes the composed-page behavior and integration repair documented under
1.3.3, whose candidate remained unpublished. Chrome adapter 1.1.1 is unchanged and covers both
service versions.

### Fixed

- **Local browser destinations follow policy (ADR-0155).** Removed the built-in localhost,
  loopback, and link-local restrictions, including IPv4-embedded IPv6 addresses. Agents can test
  local HTTP(S) development servers without a special setting or exception. Host grants, RAWX
  capability checks, request restrictions, observe/enforce behavior, and policy-defined
  never-touch destinations apply normally. Non-HTTP(S) schemes retain their existing boundary.

## [1.3.3] - 2026-09-04

Unpublished candidate, superseded by 1.3.4. Its frozen tag and draft retain the original artifacts.

The Chrome adapter advances to 1.1.1 for the composed-page capability revisions. Adapter 1.1.1
covers service 1.3.3; the earlier adapters retain their existing compatibility through service
1.3.2.

### Added

- **Explicit integration repair (ADR-0154).** A parseable foreign command occupying Ghostlight's
  key now offers a confirmed per-target `Fix` action in MCP integrations. Fix re-checks the file,
  creates a byte-exact backup, replaces only the `ghostlight` entry, preserves unrelated content,
  and refuses stale, repeated, malformed, unreadable, or non-lossless cases. Automatic and
  aggregate setup still never overwrite foreign entries.

### Changed

- **Full-page reading follows the composed page (ADR-0151).** The shortest `browser_read` call now
  reads visible text across the top document, open shadow roots, assigned slots, and injected
  HTTP(S) frames under one global character ceiling. Explicit article reading remains available
  and falls back to the same full-page behavior when no useful article exists.
- **Semantic observation and geometry follow the same boundaries (ADR-0152 and ADR-0153).** Find,
  document inspection, text waits, accessible-name fallback, iframe geometry, point receipts,
  coordinate drops, and related target operations now cross observable shadow and frame
  boundaries consistently. Negotiated capability revisions make older adapters refuse these
  stronger meanings instead of returning incomplete results.

## [1.3.2] - 2026-09-02

### Fixed

- **Demand-start identity follows the runtime override (ADR-0150).** With
  `GHOSTLIGHT_RUNTIME_FILE` set, its directory now elects the whole authority: runtime
  discovery, the service lease, the deploy lock, and the demand-started `ghostlight` sibling
  all resolve there. A floating launcher entry on a development machine (for example Cline's
  `npx -y ghostlight`) can route at the machine's real authority instead of starting a second
  one, and an elected directory without an authority fails loudly instead of letting a foreign
  binary spawn into the slot. Routing writes and adopts no browser registration; without the
  override, per-installation election beside each executable is unchanged.

## [1.3.1] - 2026-08-31

Published 2026-08-31 as the GitHub release `v1.3.1` and the npm package `ghostlight@1.3.1`,
from candidate run 33355735166 at revision `0d7b7759`. The Chrome Web Store adapter stays at
1.1.0, whose approved bytes are identical to this candidate's; adapters 1.1.0 and 1.0.0 both
remain compatible with this service line (ADR-0142).

### Fixed

- **Cross-tree registration adoption requires a deliberate install (ADR-0149 amendment).**
  1.3.0's silent repair adopted any stale Ghostlight-owned registration toward whichever tree
  crossed the no-browser seam, so a scratch build's browser call could rewrite the machine's
  real registration. Silent repair now applies only to stale details within the running
  installation's own directory; a registration owned by another installation is reported, never
  adopted, and only `ghostlight install` takes ownership.

### Added

- **The `owned_elsewhere` diagnosis.** Recovery answers `Another Ghostlight installation owns
  the browser registration.` with the `native_host_owned_elsewhere` fact and one next step
  teaching `ghostlight install` from the installation that should own the browsers. `doctor`
  names the owning installation's directory and marks it removed when that installation no
  longer exists, so a deleted tree becomes a visible row instead of silent breakage.
- **`GHOSTLIGHT_NATIVE_HOST_DIR` isolation for test tooling.** Setting it roots every
  native-host manifest and Windows registry key below one directory, so journeys and scratch
  builds exercise the real install and recovery seams without touching the machine's
  registration. Every shipped journey sets it, the CLI journey asserts the machine's real
  registration is byte-identical across its run, and the release preflight gained
  snapshot-and-guard stages that fail on any machine-state leak or leftover process.

## [1.3.0] - 2026-08-30

Published 2026-08-30 as the GitHub release `v1.3.0` and the npm package `ghostlight@1.3.0`,
from candidate run 33333813230 at revision `7b925625`. The Chrome Web Store adapter stays at
1.1.0, whose approved bytes are identical to this candidate's; adapters 1.1.0 and 1.0.0 both
remain compatible with this service line (ADR-0142).

### Added

- **One-step harness setup (ADR-0146).** The workbench Integrations destination gains
  `Set up everything`: one action configures every detected target and updates every
  Ghostlight-owned stale one through the existing serialized, backup-preserving writers. It
  skips current and absent products, leaves foreign or malformed entries untouched and
  visible, and continues across independent environmental failures with a bounded result per
  target. A generic home, configuration, or roaming root is no longer detection evidence, so
  an absent client can no longer look actionable.
- **Current Windsurf continuity (ADR-0148).** Windsurf's official Linux distribution is now
  named Devin and moved its user MCP configuration. A concrete `devin` target joins the fixed
  registry under the existing `windsurf` product card: it reads
  `<config root>/devin/mcp_config.json` and recognizes the `devin`, `devin-desktop`, and
  `surf` executables. The historical `~/.codeium/windsurf` target stays independent, so
  either installed generation is maintained without guessing one path from the other. The
  complete workbench roster is pinned at 23 targets across 19 products, restoring the ZCode
  row the preview fixture had omitted.

### Changed

- **Manual browser startup speaks to the model (ADR-0147).** The `browser.startup` setting is
  presented as `Auto-open browser on request` with `On` and `Off`. With it off, recovery
  launches nothing and a refused browser call tells the MCP model to ask the user to open one
  of the eligible installed browsers it names, with the Ghostlight extension installed, then
  repeat the call. Structured facts carry the plural `browsers` list (the singular `browser`
  fact remains for one choice), and tab listing crosses the same recovery seam instead of a
  separate generic refusal.
- **Recovery never presents a browser choice (ADR-0149).** With several installed browsers
  and none connected, both postures return the model-directed ask naming every browser whose
  native-host registration is current; stale Ghostlight-owned registrations among them are
  repaired silently first; when nothing usable is registered the answer is one named,
  choice-free remedy; and if two adapters arrive inside the same launch wait, the workspace
  binds the first arrival under the ordinary pinned-session rules. A launch happens only for
  a unique candidate under `on_demand`.

### Removed

- The `browser_recovery_ambiguous` failure and its "Ghostlight did not choose one" sentence
  leave the closed vocabularies; no browser refusal ever says Ghostlight declined to choose.

## [1.2.0] - 2026-08-30

Published 2026-08-30 as the GitHub release `v1.2.0` and the npm package `ghostlight@1.2.0`,
from candidate run 33284810442 at revision `765df478`. The Chrome Web Store adapter 1.1.0
(submitted separately) adds the popup toggle; adapter 1.0.0 remains compatible with this
service line (ADR-0142).

### Added

- **Shared process diagnostics log (ADR-0145).** All three executables append bounded,
  content-free operational records -- connection lifecycle, demand-start outcomes,
  negotiation, and operation boundaries -- into one local folder. Activation is layered and
  eventual: `GHOSTLIGHT_DIAGNOSTICS_DIR` pins a process on, or a `diagnostics.on` marker
  beside the runtime file is applied live by an OS watch with a 2-second safety-net re-check,
  whichever fires first. Logs default to a `logs` folder beside the runtime file, records
  carry no page content or payloads, retention is automatic, and the record schema is pinned
  by test.
- **Diagnostics surfaces.** `ghostlight diagnostics path|show|prune|on|off` merges every
  component into one chronological timeline (`--last`, `--component`, `--op`, `--json`);
  `doctor` reports the active layer, folder, and size; the workbench Status card toggles and
  opens the log folder; the Chrome adapter popup toggles through the runtime-control path,
  gated on the service's advertised state (adapter 1.1.0).
- **Diagnostics state on the wire.** `hello_accepted` and `control_state` carry the
  activation layer as an optional additive field, and the toggle event is sent only to
  services that advertise it, so an older service never decodes an unknown variant.

## [1.1.0] - 2026-08-27

Published 2026-08-27 as the GitHub release `v1.1.0` and the npm package `ghostlight@1.1.0`,
from candidate run 33097568009 at revision `655e2078`. The Chrome Web Store adapter stays at
1.0.0; adapter 1.0.0 covers this service line (ADR-0142).

### Added

- ZCode joins the supported harness registry (ADR-0141): install, uninstall, doctor, and the
  workbench manage `~/.zcode/cli/config.json` under its `mcp.servers` dialect, and a
  hand-registered orchestrator command stays pinned foreign evidence.

## [1.0.0] - 2026-08-26

Published 2026-08-26 as the GitHub release `v1.0.0`, the npm package `ghostlight@1.0.0`, the
Chrome Web Store adapter `1.0.0`, and the MCP Registry record `org.sylin/ghostlight 1.0.0`,
from candidate build run 33020313866 at revision `b2c27993`.

Ghostlight 1.0 is the clean-room rebuild around one orchestrator that owns every product decision,
a stable protocol-versioned MCP edge, a browser-only relay, and a policy-free Chromium adapter.
The catalog grows to 24 tools, governance becomes a policy a person can read, and the whole
model-facing surface is revoiced to teach.

### Added

- **Ghostlight 1.0 contracts.** `docs/1.0/` defines the current intent, language, architecture,
  and acceptance boundary without replacing the inherited project documentation.
- **Integrated desktop workbench (ADR-0102).** The orchestrator hosts a Tauri 2 tray
  workbench with at-a-glance plural activity, payload-free history, checkup, runtime controls,
  global search, supported-harness management, and high-signal native notifications.
- **Supported-harness management.** Codex, Claude Code, Claude Desktop, Cursor, Visual Studio Code,
  Windsurf, Zed, OpenCode, and Crush registrations can be checked, installed, or removed through
  explicit ownership-checked operations that preserve JSONC and TOML comments. The roster grows to
  18 products and 21 concrete targets with offline visual marks, status-sorted cards, and a
  lossless YAML seam (ADR-0125).
- **A live monitor.** The workbench's landing surface carries the current action in full with its
  elapsed time, then settles it into a newest-first queue as the next one rises. The orchestrator
  publishes sequenced changes rather than being polled, and a surface that misses one resynchronizes
  from a snapshot instead of trusting its cache.
- **Rows that say what happened.** Every record carries a Ghostlight-authored sentence and a
  measured duration: "Opened example.com.", "Read 1,240 words.", "Filled 3 fields and submitted the
  form.", "Stopped at step 3 of 5."
- **Per-action observation (ADR-0103).** Audit records gained a closed, content-free `observed`
  projection: the host an action landed on, the readiness the browser reported, a count, and a
  capture size. Never a path, query, or fragment; never page text. See
  [docs/guides/siem-integration.md](docs/guides/siem-integration.md) for the record shape.
- **Demand-start and one local authority (ADRs 0104, 0112, and 0127).** Launching a connected MCP
  client or Chromium starts the local authority when it is absent. A lifetime lease admits exactly
  one authority, so concurrent launches converge on one engine and one tray instead of racing.
  Launching Ghostlight starts the one desktop authority with its workbench backgrounded; launching
  it again focuses the running workbench.
- **Behavioral capability restoration (ADR-0133).** Every published 0.8 browser job is reachable
  again through current seams: REPL-mode `browser_execute` with promise waiting, user gesture, and
  by-value returns; modified, repeated, focused, timed, and stroke-sequence input; duration waits
  and coordinate wheel scrolling; semantic selectors as alternatives on click, typing, and
  per-field fill; boolean and numeric form values with contained-form submit verification;
  optional postconditions that report truthfully when an expectation fails; article-first reading
  with a visible-text mode and bounded diffs; document-scope inspect trees with snapshot handles
  and structural diffs; uploads from absolute paths, bounded inline files, or one captured image;
  and `browser_flow`, the twenty-third tool, composing one to twenty steps whose arguments may
  reference earlier results through bounded JSON Pointers.
- **Guarded beforeunload navigation.** `browser_navigate` accepts `beforeunload:discard`, which
  accepts only that navigation's own beforeunload prompt and then follows the ordinary commit and
  landing path; the default still stops and reports a blocking prompt.
- **The policy a person can read and author (ADR-0122).** Policy is the workbench's fourth
  destination: one situation sentence, one plain line per capability naming the layer that decided
  it, the rules behind those lines, and the permanent ceilings in every situation including
  all-open. The workbench authors one user policy through validated, atomic writes; organization
  ceilings disable the controls they govern and name who set them.
- **RAWX and managed policy (ADR-0121).** The action directory is an independent set that drives
  enforcement, audit, catalog projection, explanation, and simulation. Schema-3 ordered grants,
  host polarity and specificity, layer intersection, sacred destinations, stable denial ids, and
  grant attribution are live. An organization can deliver signed monotonic bundles from its own
  file or HTTPS source, with required Ed25519 and optional mandatory-both ML-DSA-65, verified
  cache, bounded retry, and rollback refusal. The workbench shows a Policy Passport and a lamp
  band distinguishing all-open, applied policy, retained-policy warning, and fail-closed states.
- **Reference experience (ADR-0126).** One product across every machine a person uses: a
  second-machine extension state with one route back, a closed per-platform environment table
  shared by install and `doctor`, a first-class CLI (`doctor --json`, manual pages, shell
  completions, plain-word states), pinned human-control directives, and a policy attention hold
  separate from a person's pause.
- **Lean Linux installation (ADR-0123).** One XDG Applications entry, `ghostlight open`
  activation, native-package provenance with Snap and Flatpak refusal, and the release Linux
  artifact built on Ubuntu 22.04 with Debian 12 and Ubuntu 24.04 lifecycle smokes.
- **Tab and group reuse with self-healing (ADR-0137).** Duplicate same-title groups merge into
  the canonical one, a plain open adopts the nearest unbound same-host tab, and the summary says
  "Reused the ... tab." when it happens. A refused release-close unbinds the tab so it becomes
  adoptable, and workspace release tells the extension to forget released tab ids.

### Changed

- **Fully open-source licensing (ADR-0140).** The whole product, including the governance module,
  is now Apache-2.0 OR MIT. Every paid option -- the tier table, founding program, prices,
  commercial license text, `PRICING.md` itself, and the governance-module CLA -- was withdrawn.
  The trust center's support, MSA, DPA, and tiers
  pages were revised or retired to match; packaging ships only Apache-2.0 and MIT texts.
  The runtime never enforced licensing before and nothing changed there: the Continuity Promise
  holds unchanged.
- **1.0 implementation rebaseline.** The active implementation is the clean-room orchestrator,
  shared typed bridge, stable MCP and browser connectors, and policy-free browser adapter. The
  branch now descends from the complete post-0.8 project history rather than an orphan root.
- **Documentation continuity.** Root documentation, licenses, ADRs through ADR-0101, research,
  trust material, design records, task ledgers, public surfaces, and product identity are retained.
- **Fringe stability.** The complete workbench and harness-management feature is implemented
  without changes to the MCP connector, browser connector, shared bridge, or extension. The later
  demand-start work changes the bridge and both connectors at their connection-lifetime seam, which
  is the one reason those fringes are allowed to change; the extension remains untouched.
- **One place authors what Ghostlight says (ADR-0103).** Completed-action sentences, safe next
  steps, and content-free measurements now come from one typed outcome in the language module
  instead of string literals spread through the executor. A sentence and the measurement beside it
  can no longer disagree, because they are read from the same value.
- **A language that teaches.** Every validation message, tool description, refusal, and result
  guidance was revoiced: validators name the allowed set and the received value, refusals lead
  with the recovery action, and invalid-input results carry the specific expectation as their
  next step instead of a circular instruction.
- **Dialogs are probed, not trusted.** `browser_dialog` attempts `Page.handleJavaScriptDialog`
  directly and reports a typed `dialog_absent` outcome, so a dialog that opened while the debugger
  was detached is handled instead of being invisible forever.
- **Per-client MCP revisions.** The edge negotiates the revision each harness needs, including
  `2025-03-26` for Junie and the `2026-07-28` discovery fallback for Antigravity, and serves
  portable top-level schemas where a downstream model API rejects composed ones.

### Fixed

- **Release frees tabs for reuse.** Workspace release notifies the extension to forget released
  tab ids, so the reuse ladder works end to end instead of leaving reaped tabs bound in topology.
- **Drag keeps its held-button lane.** The drag path restored ADR-0088's bounded two-lane seam:
  explicit held-button packets, action-scoped native interception, opaque drag-data replay, and
  cleanup on every terminal path.
- **Packaged Windows desktop regressions.** Release launches expose no console, and the workbench
  is created when the native event loop is ready instead of being lost just after startup.
- **Extension reload no longer leaves false errors.** The disconnect handler consumes Chrome's
  callback-scoped `runtime.lastError` before its stale-port ownership guard, so an expected
  replacement cannot leave a false extension error.
- **Harness roster compatibility.** Zed's `zeditor` launcher on CachyOS/Arch is detected beside
  `zed`, and Kiro/Bedrock receives portable schemas the typed decoder still validates exactly.

### Removed

- **Superseded implementation code.** The old root binary, core, transport, lightbox, extension
  mechanisms, test suites, release scripts, and packaging launchers remain in Git history but no
  longer coexist with the 1.0 implementation.
- **Removed command forms.** `ghostlight service` and `ghostlight --headless` are gone; every
  supported orchestrator start initializes the desktop authority.
- **0.8 behaviors deliberately not restored (ADR-0133).** Old tool names, narration prose,
  destructive diagnostics, client plan mutation, and direct UDP syslog audit stay retired.

## [0.8.0]

The v0.8.0 candidate makes the MCP edge explicitly protocol-versioned, leaves the service kernel
protocol-neutral, and makes browser tab continuity match what users see in Chrome.

### Added

- **MCP 2026-07-28 support (ADR-0096).** The local stdio edge implements the 2026 discovery and
  request-metadata lifecycle alongside MCP 2025-11-25. The protocol-neutral service does not parse
  MCP JSON-RPC or retain MCP request ids.
- **Local MCPB packaging (ADR-0095).** Release assembly now produces a self-contained Claude
  Desktop package with the same local-only service and Chrome Web Store adapter boundary.
- **Browser-created tab continuity (ADR-0099).** Opted-in browser results report a bounded tab
  delta. The service validates and claims a still-open child before returning its composite tab id,
  so an agent can use that id immediately without a separate context refresh.

### Changed

- **Three explicit executable roles (ADR-0096).** MCP clients launch
  `ghostlight-mcp-connector`, the persistent protocol-neutral service remains `ghostlight`, and
  Chromium launches `ghostlight-browser-connector`. The old dual-role `ghostlight-relay` path is
  removed rather than retained as a compatibility alias.
- **Extension-owned Chrome topology (ADR-0098).** The extension owns native Chrome window, group,
  and placement mechanism. The service sends only logical workspace authority and presentation
  intent, so user-moved tabs and groups stay where the user put them.
- **Major/minor adapter contract (ADR-0093).** Chrome adapter v0.8.0 covers every Ghostlight
  service v0.8.x patch. Compatible adapter and service patches can ship independently; a contract
  change advances the minor for both components.
- Exact-title tab groups are reused for presentation without merging workspace authority or tab
  inventories.

### Fixed

- A browser-created child with one exact managed opener is passively adopted into that workspace,
  while guessed, ambiguous, and cross-workspace tabs remain inaccessible.
- Closed MCP transports now tell the agent to reconnect through its current client instead of
  spawning a standalone connector that creates a different workspace and duplicate browser tabs.
- Moving a managed tab or whole group into another Chrome window no longer causes context lookup
  or addressed navigation to create replacement tabs or groups.

## [0.7.3] - 2026-08-01

### Added

- **Agent-readable MCP definitions (ADR-0094).** All 25 tools now publish standard display titles
  and conservative read-only, destructive, idempotent, and open-world hints. Focused descriptions
  distinguish overlapping tools and name the actual callable tab helpers.

### Changed

- `update_plan` is now a truthful service-local informational echo. It changes no permissions and
  requires no Chrome adapter update.
- **Independent Chrome adapter versions (ADR-0093).** `compatibility.json` now states which
  Ghostlight service versions each Chrome adapter covers. Service-only releases extend that map
  without changing the extension manifest or restarting store review. Release artifacts, store
  tooling, public status, and preflight checks now use the adapter version explicitly.
- **Conventional license discovery.** The repository root now carries the standard Apache-2.0
  license text, while `LICENSING.md` and `docs/licenses/` retain the exact open-core boundary and
  alternative license texts.

## [0.7.2] - 2026-08-01

### Fixed

- **Immediate upgrade activation (ADR-0092).** Installing a new release now replaces a verified
  managed Windows service under deploy locks and confirms that the selected executable owns the
  endpoint. External repository/dev engines stay in place. Linux explicitly restarts its updated
  user service.
- `tabs_create_mcp` now reports one composite tab ID consistently in its leading prose, embedded
  JSON inventory, and structured result.

## [0.7.1] - 2026-07-31

### Fixed

- **Stale workspace recovery (ADR-0090).** When a selected Chrome workspace disappears,
  `tabs_create_mcp` can open a fresh tab in another eligible normal window and replace the stale
  session pin. Other calls stay put, and the short error points agents directly to tab creation.

## [0.7.0] - 2026-07-20

Window-aware browser placement, a cohesive visible action language, and browser input that behaves
like real user input. The trained tool schemas remain unchanged.

### Added

- **Short launch-brief demo.** `ghostlight demo-brief` drives a public, dark-mode form through the
  ordinary MCP relay in a compact read, fill, select, and submit story for the README hero. Exact
  ref writes pace each visible field without model rediscovery. Shared demo-client support keeps
  relay framing, stable ref inventory, and page-provenance handling in one place while the existing
  Card Foundry story remains unchanged.
- **Unified action signatures (ADR-0083).** Non-spatial work now uses one signal-aware medallion
  language: a workwheel for JavaScript, a glowing keyboard for typing, calm dots for waits, and a
  camera confirmation for screenshots. The controlled-tab border remains visible while a tab is
  under Ghostlight control.
- **Ranked find presentation (ADR-0086).** Find highlights every visible match, gives the strongest
  match more definition, and uses top or bottom horizon glows when more matches sit outside the
  viewport. Not-found feedback stays distinct without exposing the query through presentation
  messages.
- **Destination-aware spatial cues (ADR-0089).** Ref-based scrolls settle chevrons into the exact
  destination halo. Coordinate image placement settles a fixed, content-free photo tile into the
  drop target while the tool result separately reports whether the page signaled handling.

### Changed

- **Window-placed workspaces (ADR-0085).** First-touch work reuses the last-focused eligible normal
  Chrome window and pins that window for the MCP session. A new browser window is created only when
  none is eligible, so prepared desktop layouts and recording positions stay intact.
- **Developer-facing front door.** The README now leads with the live browser story, the short hero
  capture, practitioner setup, and the decision aid. Public readiness material adds structured
  support routes, an OpenSSF self-assessment, greenfield first-success criteria, and a reusable
  open-source publication guide.
- `ghostlight install --help` now names every supported MCP client, including Codex, VS Code,
  OpenCode, Zed, Windsurf, and Crush.

### Fixed

- **Browser input fidelity (ADRs 0087 and 0088).** Printable key actions insert characters,
  protected fields retain content-free key feedback, named keys preserve their identity, and typed
  character counts follow Unicode code points after line-ending normalization.
- Pointer-only drags now keep complete held-button packets, while native HTML drag and drop uses a
  bounded per-tab interception and replay lane that preserves page-authored drag data without
  exposing it to the service.
- Coordinate image placement now distinguishes successful dispatch from page-signaled handling,
  and empty scroll targets fail instead of producing an ambiguous no-op.
- Branch CI no longer duplicates the same validation on a pull request while required security and
  release gates remain enforced on the PR.

## [0.6.0] - 2026-07-15

Closed-loop browser actions, calmer visible feedback, local-only transport hardening, and
resource-scoped command reliability.

This release establishes Ghostlight's greenfield browser boundary: browser control is local to the
interactive user's authenticated Chromium session. It does not preserve or replace the removed
browser-control web transport.

### Added

- **Closed-loop browser core (ADR-0078).** The additive tool surface now includes semantic
  `act_on`, explicit JavaScript dialog control, and exact owned-tab focus, reload, and close.
  Bounded interaction receipts and ref-linked target resolution reduce read-act-read roundtrips.
- **Resource-scoped scheduling (ADR-0080).** Browser work uses bounded fair queues per tab,
  client topology, and browser. Same-tab commands serialize, different tabs remain parallel,
  and single-surface compound operations retain a reentrant lease with a bounded yield quantum.
- **Extension surface executor.** A bounded per-surface FIFO adds command deduplication,
  acceptance and terminal acknowledgements, payload erasure, and exact unknown-outcome recovery.
- **Document-aware Presentation Broker (ADR-0081).** Page signage now uses explicit content-script
  readiness, exact document/revision acknowledgements, bounded state and event channels, on-demand
  packaged-renderer activation, navigation replay, and browser-session-only active state.
- **Linux user-session discovery (ADR-0082).** A relay launched without desktop environment
  variables securely discovers the owning `/run/user/<uid>` session, restores the user bus, and
  reaches the same local service endpoint as Chromium's native host.

### Removed

- **Browser-control web ingress (ADR-0077).** The `inbound.web` HTTP/WebSocket transport, its
  source policy, configuration keys, remote-enable route, and related scaffolding are removed.
  Browser control is local-only through the same-user OS pipe. The Console remains a separate,
  read-only loopback HTTP listener and rejects WebSocket upgrades.

### Changed

- **Least-privilege release publication.** A read-only assembly job now generates the pinned
  CycloneDX SBOM, packages the extension, creates a canonical `SHA256SUMS` manifest, and uploads
  one release bundle. The privileged publisher only downloads, verifies, attests, and releases
  those existing files.
- **Developer-first entry.** The README and install journey lead with practitioner setup,
  no-account and free-core facts, a read-only first proof, and the pre-release extension path.
- **Calmer visible feedback (ADR-0079).** Isolated denials use a short non-modal sticker. Repeated
  denials pause only the producing MCP session and offer explicit recovery controls. Narration,
  screenshot, and recording cues share a quieter visual language.
- **Persistent controlled-tab disclosure (ADR-0081 amendment).** Every Ghostlight-managed tab now
  keeps a gently breathing sky border across idle time, navigation, detachment, and extension
  worker restarts. Capture hides it only long enough to keep the returned image clean.
- **Atomic execution authority.** Configuration and policy publish as one epoch-bound snapshot;
  URL probes, authorization, dispatch, landing verification, post-processing, and audit use one
  admitted execution context.
- **Complete JavaScript release gates.** The operating-system matrix discovers every extension
  test, parses every extension JavaScript file as a whole, and exercises the npm launcher's host,
  hash, and platform-selection boundaries.

### Fixed

- Native-port and extension-worker reconnects no longer clear an uncertain tab. Recovery requires
  the exact terminal command, confirmed tab destruction, or a changed browser-process generation.
- Extension reload on an unchanged page no longer silently disables narration, denial notices, or
  action feedback. The broker reinjects the packaged renderer into managed tabs and verifies the
  current document accepted the presentation.
- Retained compound intents no longer suppress their own later extension subrequests.
- Tool completions and auxiliary asynchronous replies remain bound to the native connection that
  accepted them, even when a replacement connection reuses the same numeric request id.
- JavaScript dialogs block ref resolution, geometry reads, scroll probes, cursor movement, and
  direct fallbacks before any page-dependent preparation can stall behind the dialog.
- The Foundry demo explicitly negotiates current or legacy provenance through `tools/list`,
  validates the full service-authored boundary, and fails closed on an unnegotiated contract.

## [0.5.7] - 2026-07-13

Reliable memory-only GIF recording, bounded large-payload delivery, agent narration, a broader
installer matrix, and the cohesive Sylin Card Foundry demo.

### Added
- **Agent narration (ADR-0072).** The additive `narrate` tool shows one temporary, responsive
  workflow ribbon in the controlled tab. It is domainless RAWX none, audited through the normal
  chokepoint, usable in scripts and batches, capture-hidden, memory-only, and visually distinct
  from authoritative governance notifications.
- **More first-class MCP clients.** Codex, Windsurf, Zed, OpenCode, and Crush join Claude,
  Cursor, and VS Code. Codex TOML is merged losslessly; strict JSON is updated idempotently;
  commented JSONC is preserved and receives copyable manual configuration.
- **Sylin Card Foundry demo.** `ghostlight demo` now tells one simulated foil-card QA story with
  3D card inspection, defect review, drag disposition, structured form completion, screenshot and
  GIF evidence, and a real off-domain policy denial. The public stage is
  `sylin.org/ghostlight/demo/foundry/`.
- **Bidirectional install handoff.** A successful first install opens the stable browser-extension
  walkthrough once, while dry-run, CI, failed, repeated, and `--no-open` paths stay quiet.

### Changed
- **Recording is ephemeral and lease-bound (ADR-0073).** Capture is session, surface, and
  generation-owned; memory-only; byte-bounded; transactionally started and finalized; refreshed
  by recordable tool activity; and capped by independent 30-second idle and 120-second hard
  deadlines. Export auto-finalizes, and every session/policy/panic/retention failure path erases
  captured bytes.
- **GIF encoding is bounded.** Frames are encoded in two passes and processed one at a time rather
  than retaining all decoded frames at once.
- **One tab group per client (ADR-0066).** Repeated tab creation reuses the session's managed group
  instead of producing a new group for every tab.
- Public documentation and install URLs now consistently use `sylin.org`; Chrome Web Store privacy
  and permission disclosures were tightened against the implemented data flows.

### Fixed
- **Large browser-bound payload delivery (ADR-0074).** Screenshot and GIF uploads use negotiated,
  SHA-256-verified, memory-only chunks, avoiding Chromium's native-messaging size disconnect. Old
  extensions fail fast before an oversized write, and uncertain delivery is reported as
  `outcome_unknown` with `retry_safe: false`.
- Debug observability no longer persists MCP or tool payload bodies that may contain captured page
  data.
- Restored the intended governance badge bleed after the responsive narration pass.

## [0.5.6] - 2026-07-12

Session-policy governance and a scripted demo, root-fixed browser identity and reconnect
resilience, the one-stack dev model, an on-screen governance ribbon, the field-splash effect
with a codified visual language, and the SAPS security-hardening pass. (Includes everything
listed under 0.5.5 below, which was prepared but never published.)

### Changed
- **One stack: the engine is whoever holds the endpoint (ADR-0065).** There is one native host
  (`org.sylin.ghostlight`, allowing both the Web Store and the unpacked dev extension ids), one
  service endpoint, and one `ghostlight` MCP entry -- no dev install, no `-dev` host, no second
  tool namespace. The dev loop becomes an engine swap (`scripts/dev-loop.ps1`: deploy-lock
  quiesce, stop the engine, rebuild, start the fresh build; `-Restore` hands the endpoint back to
  the installed release), ridden through transparently by the relay reconnects (ADR-0045/0062).
  Amends ADR-0064: its dev-stack-as-workflow half (extension identity host selection,
  instance-aware `allowed_origins`, the installed dev instance) is retired; its structural
  simplifications survive. Named instances remain only as a test-isolation seam;
  `scripts/dev-browser.ps1` is removed.
- **The pre-release two-stack development design was retired (ADR-0065).** ADR-0064's explicit dev
  isolation replaced the earlier auto-shadow during development, but the final v0.5.6 release
  removed the dev host, dev install, and duplicate MCP entry in favor of the one-stack engine swap
  above. Named instances remain only as a test-isolation seam.
- **Safe-by-default posture.** The audit flight recorder now defaults ON in every preset,
  all-open included (a session always leaves a trail), and `inbound.web.enabled` now defaults
  OFF in every preset (driving the browser over a local TCP port is opt-in; the pipe path remains
  the default). `ghostlight doctor` shouts `UNGOVERNED (all-open)` and prints an audit-health line.
- **README/registry re-lead:** the front door leads with the durable axis (governance + audit +
  governed composition) and the Continuity Promise; pricing is discoverable-when-sought.
- Softened the security/support SLAs to explicit best-effort targets (solo project).

### Added
- **On-screen denial notifications (SAPS PRES-HIGH-01).** When governance refuses an action, the
  browser shows a persistent top ribbon -- a neutral near-black band with a severity-colored icon
  medallion (error/warn/info/debug) -- explaining the denial in plain language, on the page where
  it happened. It persists until the next genuine page action or an explicit close click;
  read-only actions (screenshot, read, zoom, wait) deliberately never dismiss it.
- **Session policy overlays and the policy composition model (ADR-0060).** A client can declare a
  tighten-only policy at `initialize` (`_meta.ghostlightSessionPolicy`, a schema-3 manifest):
  grants intersect, sacred domains union, the stricter mode wins -- the AWS-session-policy shape,
  escalation-safe by construction. An overlay tightens even an all-open service.
- **`ghostlight demo`:** a scripted, watchable tour of the public demo stage driven through the
  real MCP surface under a session overlay, ending with a governance refusal on screen. Exits
  non-zero on any failure, so it doubles as an end-to-end smoke test.
- **Multi-browser sessions (ADR-0058/0061).** Every attached browser gets its own identity-keyed
  session and tab-id slot, with focus-chain routing for slot-less calls; `ghostlight doctor` lists
  every connected browser. Browser identity is a persistent UUID the extension itself mints and
  stores (stable across relay reconnects AND service-worker restarts), replacing the relay's
  guessed parent pid -- the root fix for tab ids mis-routing under pid 0.
- **Browser-relay restart resilience (ADR-0062).** A service restart or upgrade no longer drops
  Chrome's native-messaging port: the browser relay reconnects and replays the extension's
  identity frame, keeping the same session and slot with no extension reload.
- **Deploy-quiesce lock (ADR-0063):** a `deploy.lock` beside the service executable holds off
  relay self-heal while a binary is being replaced, so a swap never races a relaunch of the old
  image (stale locks self-expire after 30 minutes).
- **Developer instrumentation (ADR-0059):** `lightbox fake-browser` (an offline wire-protocol
  driver -- no Chrome needed, with a deliberately billion-scale canned tab id so encoding bugs
  surface offline), structured native-host attach/reject diagnostics in the debug state ring, and
  the one-command `scripts/dev-loop.ps1`.
- **Field splash effect + a codified visual language.** Every form write (`form_input`,
  `form_fill`, `file_upload`, `upload_image`) now draws a sky-blue splash hugging the field it
  just set -- the click ripple's language applied to the field's own rectangle -- so a watcher
  sees exactly which field the agent touched. The whole effect vocabulary (foundations,
  invariants, the effect dictionary, and the add-an-effect checklist) is now codified in
  `docs/design/visual-language.md` as the normative reference.
- **`ghostlight demo` pacing controls:** `--setup-pause` (default 10s, a hold right after the
  demo tab opens so you can resize/position the window) and `--section-pause` (default 5s
  between the tour's sections), alongside the existing per-step `--pause`.
- **`ghostlight doctor` confirms the extension link without `--debug`** via an in-band
  control-plane `status` request over the ADAPTER/CONTROL endpoint (activates the reserved
  `control` role). Renders a real connected/disconnected verdict.
- **`navigate` auto-creates the Ghostlight tab group** when none exists, instead of failing cold.
- Supply-chain gate: `deny.toml` + a blocking `cargo-deny` CI job; pinned toolchain
  (`rust-toolchain.toml`).
- Bounded audit file (single-generation rotation at 50 MiB).
- `MAINTENANCE.md` (continuity / bus-factor) and `CODE_OF_CONDUCT.md`.

### Fixed
- **Management-plane write path:** the `enable-remote` action requires an `X-Ghostlight-Intent`
  consent header (CSRF hard-stop); the management router enforces a loopback `Host` header; a
  present-but-unparseable `Origin` is no longer treated as the loopback peer.
- **Owner-only pipe DACL fails closed** on Windows instead of falling back to the default DACL.
- **One-liner installers verify integrity** (SHA-256 + best-effort `gh attestation verify`) before
  trusting a downloaded binary.
- **Group-gated the tab-URL probe** so it cannot enumerate the user's non-Ghostlight tabs.
- **`tabs_create` prose now carries the composite tab id**, matching `structuredContent`, instead
  of leaking the raw native id a follow-up call could mis-route on.
- Retired the perpetually-hanging `e2e-smoke` CI job; corrected stale docstrings and the governance
  license path.

## [0.5.5] - 2026-07-11 (prepared but not published; first shipped as part of 0.5.6)

Central policy distribution (ADR-0055): an organization can now host, sign, and roll out a
governance policy from one place instead of hand-copying a manifest file to every machine. Plus a
dev-only integration harness for the whole governance/managed stack (ADR-0056), and the Open Trust
Center -- a public, evidence-linked set of procurement and security-review documents.

### Added
- **managed:// central policy distribution** (ADR-0055): a signed, versioned policy bundle an
  organization publishes once and every device polls. `ghostlight policy sign` produces the bundle
  (the same Ed25519 + ML-DSA-65 composite signature as the license engine); a device bootstraps from
  a local file or a network fetch (`--no-default-features` opts out of the network path entirely),
  keeps a last-known-good cache that is never allowed to fail open, and re-polls live with no
  restart. `ghostlight doctor` and `explain` surface the active policy's provenance -- source,
  freshness, signer -- from a single `ManagedStatus` sidecar; every audited tool call now carries a
  `policy_seq` so the audit trail can prove which policy version was in effect at decision time; a
  denial under a managed policy names the organization's contact line.
- **Lightbox** (ADR-0056): a dev-only integration harness that exercises the real governance/managed
  code path -- no mocks -- against injected time and a local fake-org endpoint, covering passport
  freshness and multi-instance policy propagation. Not part of any shipped release artifact.
- **The Open Trust Center** ([docs/trust/](docs/trust/)): public, evidence-linked procurement
  documentation -- an FAQ, security overview, data-flow diagrams, sub-processor and supply-chain
  disclosures, continuity commitments, a CAIQ-shaped controls questionnaire, the support policy, and
  MSA/DPA drafts on Common Paper standard forms. Every non-obvious claim links the ADR, test, or
  attestation that backs it; nothing here is gated behind a sales call.

## [0.5.4] - 2026-07-10

### Fixed
- Windows binaries no longer require the Microsoft Visual C++ Redistributable. The MSVC build now
  links the C runtime statically, so `ghostlight.exe` and `ghostlight-relay.exe` start on a clean
  Windows machine with nothing extra to install. Previously they failed with "VCRUNTIME140.dll was
  not found" wherever the redistributable was absent -- a latent issue in every Windows build since
  v0.1.0, surfaced by the winget clean-room validation. This is a generic Rust-on-MSVC behavior
  (Rust's own `std` links the VC runtime dynamically by default), not a dependency of any crate, and
  the static-linking fix covers every Windows channel at once (npm, scoop, winget, direct download).
  Linux is unchanged.

## [0.5.3] - 2026-07-10

The offline licensing engine (ADR-0028), plus how-to guides. Licensing is purely observational --
it never gates behavior, never phones home, and is dormant unless an org-deployed governance policy
is operating -- so this release changes nothing about how Ghostlight runs.

### Added
- Offline license verification: `ghostlight license status` / `install` (a file, an armored
  `-----BEGIN GHOSTLIGHT LICENSE-----` block, or stdin), plus a read-only `License:` section in
  `ghostlight doctor`. A license is verified locally with a composite Ed25519 + ML-DSA-65 (FIPS 204,
  post-quantum) signature; there is no activation server and no network call in the license path.
- Founder-only offline authoring (`license sign` / `pubkey`, gated behind the `license-admin`
  build feature; never in a release build).
- Guides: [docs/guides/licensing.md](docs/guides/licensing.md) (installing/refreshing a key) and
  [docs/business/issuing-licenses.md](docs/business/issuing-licenses.md) (the founder's signing
  runbook).

### Notes
- The audit `license` marker appears only while governance is operationally in effect via an
  org-deployed policy; the all-open audit stream is byte-identical to before.
- Production key generations are embedded by the founder offline; until then a public evaluation
  key allows frictionless self-signed evaluation.

## [0.5.2] - 2026-07-10

npm launcher supply-chain hardening (prompted by a Socket.dev supply-chain finding on the 0.5.1
package). No changes to the ghostlight binaries themselves.

### Security
- The npm launcher now verifies every downloaded binary before it is made executable or run. Each
  release binary's sha256 is pinned in a `checksums.json` that ships inside the npm package (an
  immutable, independently hosted manifest), and the launcher aborts on any mismatch -- a tampered
  release asset, a corrupted transfer, or a hijacked redirect can no longer lead to running an
  unverified binary.
- Downloads and their redirects are constrained to GitHub hosts (`github.com` and
  `*.githubusercontent.com`), so a rogue `Location` header cannot redirect the fetch off-platform.
- The release pipeline (`scripts/release.ps1`) generates the pinned manifest from the published
  `.sha256` assets and refuses to publish a launcher whose manifest is missing or incomplete.

## [0.5.1] - 2026-07-10

Windows first-run fix, found live while validating the Cline marketplace submission (issue #17).

### Fixed
- Windows auto-start actually works without elevation now (ADR-0054). The installer previously
  registered a Task Scheduler logon task, but Windows denies creating logon-trigger tasks from a
  non-admin shell -- so every unelevated install silently registered nothing, and a fresh user's
  first tool call failed until they ran `ghostlight service` by hand. The installer now writes the
  per-user registry Run key (always writable, visible in Task Manager's Startup page) and starts
  the service once, detached; the relay's self-heal likewise spawns the sibling service executable
  detached (null stdio, its own process group) instead of asking a scheduled task that may not
  exist. Old scheduled tasks from elevated installs are cleaned up on install and uninstall.
- Linux systemd user-service behavior is unchanged.

## [0.5.0] - 2026-07-09

The tool-surface release. Four new tools, harvested from the official Claude-in-Chrome v1.0.80 (now
the sole reference), grow the advertised surface from 17 to 21; the 13 trained tools are re-baselined
against that reference; and the role executables collapse from three to two.

### Added
- `file_upload` tool: upload base64-encoded file bytes to a located file `<input>` on the page via
  an in-page DataTransfer. It never reads the host filesystem; the caller supplies the bytes.
  (ADR-0050)
- `browser_batch` tool: a trained front door that runs a sequence of browser actions in one call over
  the same engine as `script`, returning each step's result with images preserved. (ADR-0050)
- `upload_image` tool: upload a previously captured screenshot (by image id) to a file `<input>`, or
  drag-drop it onto a page element at a coordinate. (ADR-0050)
- `gif_creator` tool: record a browser-automation session and export it as an animated GIF, either
  downloaded or drag-dropped onto a page element. Capture is change-driven (a frame only when the
  page visually changes, with real per-frame timing); frames carry visual overlays (click cues,
  action labels, a progress bar, and a Ghostlight watermark) and use an adaptive NeuQuant color
  palette for faithful screenshots. The encoding pipeline runs in the binary; the extension only
  relays captured frames. (ADR-0050, ADR-0052, ADR-0053)

### Changed
- The extension is thinner: it contains only what must touch a Chrome API (the thin-extension rule,
  ADR-0053); pure computation and durable state -- including the whole GIF pipeline -- live in the
  binary, where fixes ship with a release instead of a web-store review.
- The 13 trained tool schemas are re-baselined against the official Claude-in-Chrome v1.0.80. Only
  three description strings changed for accuracy (`form_input`, `get_page_text`, `read_page`); no
  trained tool name, parameter, or enum value changed. (ADR-0050)
- The two thin pass-through adapters (`ghostlight-adapter-agent`, `ghostlight-adapter-browser`) are
  merged into a single `ghostlight-relay` binary, role-selected at launch (`--role agent`; the
  browser role auto-detected from the Chrome extension origin, since a native-messaging manifest
  gives Chrome a bare path). Releases now ship two executables (`ghostlight` + `ghostlight-relay`)
  instead of three. (ADR-0051)

## [0.4.1] - 2026-07-09

A distribution-plumbing patch: it makes Ghostlight publishable to the official MCP Registry and
fills out the package-manager manifests. No runtime behavior changes.

### Added
- npm `mcpName` marker (`org.sylin/ghostlight`) in the launcher package, the ownership proof the
  MCP Registry requires to link the `org.sylin/ghostlight` server entry to its npm package.

### Changed
- `server.json` targets the current registry schema (`2025-12-11`) and tracks this release.
- The winget and scoop manifests carry the real per-artifact sha256 sums for this release instead
  of placeholders.

## [0.4.0] - 2026-07-09

The multi-instance, resilience, and conformance release. The single binary splits into three role
executables; named instances run fully isolated stacks; a live `dev` build shadows the release
install for unpinned clients; the adapter rides through a service restart, rebuild, or crash with no
client reload; and an MCP protocol-conformance pass brings the handshake up to the current spec.

### Added
- Named instances (ADR-0044): `--instance <name>` runs a fully isolated Ghostlight stack (a `dev`
  alongside the default deploy) with its own identity, directories, host registration, and
  supervisor; `--keep-warm` keeps a service up between actions instead of idle-exiting.
- The development override (ADR-0048): an MCP client or browser registered WITHOUT an explicit
  instance now resolves at connect time, preferring a live `dev` instance and falling back to
  the default -- run a dev service and every unpinned client routes to it; stop it and they
  return to the release install on their next connect.
- One browser surface: the native-host manifest always allows both the Web Store and the pinned
  unpacked-dev extension ids, so `ghostlight install` needs no --extension-id and one
  registration serves a store install and a dev checkout at once.
- `ghostlight doctor` reports whether a live dev instance is currently shadowing the default for
  unpinned clients.
- MCP `protocolVersion` negotiation (ADR-0049): `initialize` echoes the client's requested revision
  when supported and offers the latest (`2025-11-25`) otherwise, instead of a hardcoded
  `2024-11-05`.
- `--no-supervisor` install flag (ADR-0046): skip registering the OS auto-start service (the dev
  loop runs the service in a terminal instead), documented in docs/DEV-LOOP.md.

### Changed
- Three role executables (ADR-0046): the single binary is now `ghostlight` (the CLI + persistent
  service) plus two thin pass-throughs, `ghostlight-adapter-agent` (the MCP-client side) and
  `ghostlight-adapter-browser` (the Chrome native-messaging side). A service rebuild no longer
  relinks the adapters, and `install` places all three side by side.
- Resilient reconnecting adapter (ADR-0045): a service restart, rebuild, upgrade, or crash no
  longer forces an MCP-client reload -- the adapter reconnects (a patient window, up to 120s) and
  replays the captured MCP handshake, so the client rides through transparently.
- Session identity is stable across reconnects: the agent adapter re-presents one guid per
  process, so tab ownership and the session's Chrome tab group survive a service restart
  (ADR-0047 D2).
- New tabs are born directly in the calling session's tab group (no more about:blank bootstrap
  litter), and tabs_context_mcp reports that session's group (ADR-0047 D3).
- Tab groups are titled by the MCP client's name (for example, the ghost glyph followed by
  "Claude Code"), deduped across sessions, instead of a truncated session id (ADR-0047 D4).
- A tab owned by a session that is no longer connected can be adopted by a live session, and
  dead group-map entries are pruned on service-worker restart (ADR-0047 D5).
- `--instance dev install` is now thin (ADR-0048 D6): it registers only the pinned
  `ghostlight-dev` MCP-client entries; browser traffic rides the unified default host.
- The extension always connects to the `org.sylin.ghostlight` host; the installType-based
  dev-host selection is superseded by adapter-side resolution (ADR-0048 D5).
- MCP conformance (ADR-0049): `initialize` advertises `tools.listChanged` (the server does emit it
  on manifest hot-reload); a malformed JSON-RPC frame gets an addressable `-32700` instead of a
  silent drop; and a JSON-RPC batch (array frame, removed from MCP in 2025-06-18) is rejected with
  a message pointing at the `script` tool.

### Fixed
- Tab tools no longer refuse tabs that sit in a per-session Ghostlight group: the extension's
  gate now recognizes every Ghostlight-managed group (ADR-0047 D1; the e2e F4 desync).
- A service-side read error in the agent adapter reconnects instead of exiting, so an abrupt
  service death never forces an MCP-client reload (ADR-0047 D6).
- The anti-squat hub-key is now per-user, not per-instance, so an unpinned adapter (default
  identity) can verify a live `dev` service's proof -- the development override no longer fails the
  "not the one this user installed" refusal (ADR-0048 amendment).

## [0.3.0] - 2026-07-07

The composition batch (ADR-0035 through ADR-0038): sequential multi-step scripting, semantic form
filling, page-state awareness, and structured results -- the tools that collapse multi-round-trip
browser workflows into a single call. Plus the distribution push: one-line installers, the npm
launcher, the landing/install pages, and the extension's first-run walkthrough tab.

### Distribution (2026-07-07 session)

- One-line installers `scripts/get.sh` / `scripts/get.ps1` (download latest release, run the
  idempotent `ghostlight install`). Release assets now include RAW per-target binaries
  (version-less names) so `releases/latest/download/...` works with no API parsing.
- `ghostlight` npm launcher (`npx -y ghostlight`): fetches the version-matched binary on first
  run; stderr-only chatter so MCP stdio stays clean.
- cargo-binstall metadata and winget/scoop manifest templates under `packaging/`.
- Landing + install pages under `site/` (GitHub Pages); the extension opens the install
  walkthrough on first install (reason "install" only, no state, no tracking).
- README quick-install block with Cursor / VS Code one-click deeplinks and the npx snippet;
  `server.json` for the official MCP registry.

### Fixed (distribution session)

- Cross-platform test-profile compile errors (`proc.rs` cfg-gated `Stdio` import,
  `supervisor.rs` cfg-gated test helper) that failed the Linux CI gate.
- Console index truncation on hosted Windows runners: the management web server now performs a
  bounded lingering close (drain to client EOF) after `flush` + `shutdown`.
- The quarantined `e2e-smoke` CI job is capped at 15 minutes (it previously hung to the
  6-hour runner ceiling on every push).
- CI now runs `tests/extension/grouping.test.js` (it existed but was not in the test line).

### Added

- **`script` tool** ([ADR-0035](docs/adr/0035-script-tool.md)): run up to 20 tool calls sequentially
  in one request. Steps execute in order through the same governance chokepoint every individual call
  enters; each step is independently authorized, audited, and post-processed. Step arguments may
  reference a prior step's structured result (`$prev.field`, `$N.field`); a `dry_run: true` flag
  returns per-step governance verdicts (`would_allow` / `would_deny`) without dispatching, so the
  model sees the pre-flight map before committing. A `budget_ms` argument bounds the whole call.
- **`form_fill` tool** ([ADR-0036](docs/adr/0036-form-fill-tool.md)): fill a form by field labels in
  one call. Matches keys against label, placeholder, name, and aria-label with specificity-ordered
  tiering; ambiguous keys are returned unmatched with candidates instead of guessed. Optional
  `submit: true` clicks the form's own submit control after filling.
- **`wait_for` tool** ([ADR-0037](docs/adr/0037-page-state-awareness.md)): wait until a page condition
  holds and the page has settled. An adaptive settle detector (mutation-rate decay, floored at 3)
  gates on the page's own pace; returns elapsed_ms, settle diagnostics, and the matched element's
  ref for direct chaining.
- **Structured results** ([ADR-0038](docs/adr/0038-structured-results.md)): tools with a declared
  result vocabulary (`find`, `tabs_context_mcp`, `tabs_create_mcp`, `navigate`, `wait_for`,
  `script`, `form_fill`) carry a `structuredContent` field alongside text and advertise an
  `outputSchema`. This is the substrate `script`'s references resolve against.
- **Consequence digests** ([ADR-0037](docs/adr/0037-page-state-awareness.md) Decision 2): every mutating
  action's confirmation gains an `observation:` block reporting what changed (URL, title, DOM
  mutations, focus movement, alerts, dialogs).
- **`read_page` diff mode** ([ADR-0037](docs/adr/0037-page-state-awareness.md) Decision 3): the optional
  `diff: true` argument returns only changes since the previous read on that tab. Stale-ref errors
  now name the re-render and the fix.
- **`engine.script.budget_ms` config key**: total wall-clock budget for one `script` call (default
  120000ms, range 1000..480000).

### Changed

- `dry_run` is a pipeline-level parameter on `run_tool_call`, not a script-internal evaluator: it
  runs the real governance decision (registry, schema, sacred, authorize) and returns the verdict at
  the dispatch boundary without dispatching. It is honored by every tool at the pipeline layer but
  advertised only on `script`'s inputSchema (the 13 trained schemas are byte-pinned).
- [ADR-0035](docs/adr/0035-script-tool.md) Decision 9 (an `idempotency_key` cache on `script` /
  `form_fill`) was not taken in v1; it is superseded by [ADR-0040](docs/adr/0040-pipeline-idempotency-gate.md)
  (Proposed), which relocates retry-safety to a pipeline-level gate covering every tool call.

## [0.2.0] -- 2026-07-05

The Ghostlight Hub release. The single-session model is replaced by a persistent
background service that owns the one browser link and multiplexes every client through a
single governance chokepoint, plus a local Console for seeing what the service is doing.

### Added

- **The Ghostlight Hub (ADR-0030).** A persistent, standalone `ghostlight service` now
  owns the browser link and the client endpoint for its whole life. Every MCP client runs
  as a thin adapter that connects to it, so any number of clients (Claude Code, Cursor,
  and others) can be connected at once, each multiplexed as its own session through the
  single governance chokepoint. This repeals the previous one-session-at-a-time limit
  (ADR-0004). The service is kept warm by a per-user OS supervisor (Windows Task Scheduler or
  Linux systemd --user), self-heal-started on first use if it is down, and shuts down only after
  an idle-grace window with no live sessions and no
  browser link.
- **The Console (ADR-0030 Decision 9).** A local, loopback-pinned web page served by the
  service at its web-API address. It shows live sessions (with truncated session ids), a
  provenance-aware view of the layered configuration (value, source layer, and lock state
  per key), and a single "enable remote connections" control. It is never a manifest
  editor and never a remote control plane.
- **Local web API.** An opt-in TCP JSON-RPC endpoint that acts as a second session source
  alongside the stdio adapters, gated by the new `channels.webapi.from` policy key
  (loopback-only by default).
- **Per-session browser tab groups.** Each session's tabs are grouped in the browser so
  concurrent sessions stay visually distinct.
- **Cross-session isolation and admission control.** Binary-authoritative tab ownership so
  one session cannot drive another's tabs; adapter-minted session ids bound to the
  connecting client's OS credential; per-client session and mint quotas (never a single
  global cap); and an anti-squat proof on the client endpoint.
- **Reconnect grace and an honest bounded queue (ADR-0030 Decision 3).** A bounded
  reconnect window over transient extension drops, per-client rate limiting, and
  oversize-reply chunking so one session's large payload cannot head-of-line-block
  another's small one.
- **Extension polish.** Official mascot icons, a per-action visual-feedback vocabulary
  (click ripples, drag trail, type shimmer), and an options page plus popup toggle for
  those preferences and action captions.
- **Installer auto-start.** `ghostlight install` now registers and starts the OS
  supervisor so the service is always ready.

### Changed

- Renamed the browser extension to "Ghostlight in Browser" and recorded its Chrome Web
  Store listing.
- Reorganized the internals into a `src/hub` composition root (HubCore / ServiceContext)
  with transport-generic session serving, so the same governance path serves both the
  stdio adapters and the web API.

### Fixed

- **Lifecycle hardening (ADR-0029).** Cross-platform process-liveness primitives, a
  parent-death watchdog so an orphaned session self-terminates when its editor exits, a
  liveness-aware `doctor` with a `--fix` reaper and a startup orphan sweep, and a single
  shutdown coordinator. Idempotent extension library modules so a re-injected content
  script cannot double-register.

## [0.1.0] -- 2026-07-04

First tagged release: the unconstrained browser-automation engine (all-open) with the
governance overlay available as an opt-in capability manifest. Shipped as Windows x86_64 and Linux
x86_64 binaries plus the extension ZIP, with SHA-256 checksums and signed build-provenance
attestations.

[0.2.0]: https://github.com/sylin-org/ghostlight/releases/tag/v0.2.0
[0.1.0]: https://github.com/sylin-org/ghostlight/releases/tag/v0.1.0
