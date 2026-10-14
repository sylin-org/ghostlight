# ADR-0197: Contained seams and the 2.0 break

Date: 2026-10-13. Status: Accepted by owner direction (the owner authorized the redesign and its
recommended decisions; each decision below is reversible on its own).

Builds on ADR-0196 and keeps its Decisions 1-6 and 8-13. Supersedes ADR-0096 Decisions 1-3 (MCP
negotiation moves into the service), ADR-0181 (replaced by adapter protocol 4), ADR-0196
Decision 7 (no transition level: the break is strict), ADR-0165 (Flatpak route), ADR-0167's installation selection,
ADR-0193 Decisions 2-3 (deployment seam), ADR-0161 stage 2 (observed peer identity) and the
ADR-0145 Decision 2 popup diagnostics route. Amends ADR-0119 (Quit), ADR-0121 (managed policy
and observe mode), ADR-0126 (the Attention state), ADR-0158 (frame handling and notice
options) and ADR-0166 (a stalled start reaps itself).

## Context

ADR-0196 set the direction: one pipeline, a service-owned browser adapter, a small frozen
extension, contained sessions, protocol-versioned connectors, no downgrades. The owner then
accepted one compatibility break in exchange for stability afterwards, asked for a surgical
redesign that concentrates complexity at the seams and removes what nothing needs, and asked
that the rebuilt tree read pristine: no leftover or dead code.

An architecture assessment on 2026-10-13 inventoried nine areas. The pipeline core holds. The
accidental complexity sits around it:

- Binaries live in at least seven places, so a five-field installation record with four
  selections, a deploy lock in two places, a startup admission file and four outside lease
  probes exist to decide which copy serves. Connectors write that record.
- Connectors decode closed product types and embed the product version, so every release
  rebuilds and stops them. Each connector links 35 crates on Windows and 85 on Linux.
- MCP negotiation and result rendering live in the connector, which therefore changed in 24
  commits since 2026-08-01.
- The browser connector replays a cached hello and queues stale frames across service
  restarts. Connectors respawn the service every 500 ms, so Quit does not hold.
- Governance re-reads policy on every call and runs an HTTPS fetch under the same lock. Several
  knobs (observe mode, channel switches, the Attention state) have no promise behind them.
- Seven packaging channels each need their own lifecycle scripts.

## Decision

### Components

1. **Three executables, one extension, one job each.**
   - `ghostlight`: every product decision. MCP, language, governance, work, workspaces, the
     Chromium adapter and its page assets, audit, diagnostics, tray and workbench, CLI and
     install. Tauri stays in process.
   - `ghostlight-mcp-connector`: keeps one MCP stdio stream alive across service restarts.
   - `ghostlight-browser-connector`: carries one Chrome native port to one service connection.
   - The Chromium extension: the browser's native side, adapter protocol 4 only.

   The connectors are built on one small frozen crate, `relay` (framing, a tolerant runtime
   reader, the opening frame, spawn with backoff, the parent watchdog, an append-only log
   writer). The `bridge` crate dissolves: what only the service uses moves into it; the
   installation record, admission and desktop activation are deleted.

### Seams

2. **MCP client to MCP connector.** The connector forwards every line unchanged, including
   JSON-RPC batch arrays. It tracks outstanding request ids per direction and echoes each id
   with its JSON type. It answers `ping` itself. While the service is away it holds lines in
   order, up to 4 MiB and 30 s, and starts the service with backoff (0.5 s doubling to 30 s).
   When an outstanding request is cut off, or a held request expires, it answers with a failure
   template the service handed it earlier. The connector authors no words.
3. **Connectors and CLI to the service: relay protocol 3.** One loopback listener. A runtime
   file `{relay, port, token, service_version}` beside one lease, both under the fixed home.
   Readers ignore unknown fields and fields are never removed. The opening is
   `{relay, token, stream}` where the stream is `mcp`, `browser` or `control`. Control
   operations are `activate_workbench`, `readiness` and `quit`. Liveness means an accepted
   opening. Tokens are compared in constant time and unauthenticated openings are bounded.
4. **Chrome to browser connector.** Hold the first frame until the first service connection,
   then pump both ways without parsing. When the service goes away, tell the extension
   `backend_unavailable` and never start the service while waiting. When a service accepts
   again, exit, so the extension reconnects with a fresh hello. The native port recycles on
   every service return. Correlation ids are logged by the service and the extension, not by
   the connector.
5. **Service to extension: adapter protocol 4.** Specified in
   `docs/design/adapter-protocol-4.md`. Assets arrive in `hello_accepted` and install into
   the isolated world `ghostlight` of every frame, called through handles of that world, never
   through numeric context ids. Receipts carry facts and closed failure codes; the service
   classifies effects. Events cover only tabs Ghostlight opened or addressed. An unknown
   primitive answers `unsupported`. New primitives get new names and existing ones never change
   meaning. Protocol 4 is the only adapter protocol: an older extension is told to update, and
   an extension that meets an older service tells the person to update Ghostlight. The
   extension and the service therefore publish together. If the store refuses an extension that runs
   service-supplied code, the fallback is an asset bundle embedded in the package and selected
   by hash; feature changes would then need store releases.
6. **Service to page assets.** Two entry points, call and cancel, answering `{ok, value}` or
   `{ok:false, error:{code, message}}`. Assets and their translator ship in the same build, so
   this seam changes freely. Credential classification lives here.
7. **Executor to browser port.** One typed browser error: cancelled, deadline, not connected,
   not answering, busy, wrong profile, unsupported, failure with facts, protocol. A deadline
   fails only its own call; only a closed socket fails other pending calls. Presentation is a
   one-way notice off the executor's path.
8. **Language.** One table of tools drives the input schemas (flat, no `oneOf`), decoding, the
   capability directory, the flow allowlist and a golden `docs/1.0/catalog.json` with a size
   budget. Next steps are typed: call a tool with arguments, or say something to the person.
9. **Governance.** One refresher publishes an immutable authority snapshot; the invocation path
   does no file I/O. One `admit()` replaces inline authorization calls. Runtime control is
   checked at every effecting step and published to adapters only when it changes.

### State, start and stop

10. **One home, one service per OS user, by construction.** The home is `~/.ghostlight`, derived
    from the user's home directory as today. A test-only override exists for journeys and is
    documented as such. Only `ghostlight serve`, run from the fixed path or the development
    marker's executable, takes the lease. A `serve` that has not published discovery within
    30 s exits, so a stalled start never blocks later starts. ADR-0166's discovery timing stays:
    discovery is published only once the workbench can be activated.
11. **Lazy start; Quit holds.** The MCP connector starts the service at its own start and when a
    request is held. The browser connector starts it only at its own start. Nothing starts it
    while idle, so Quit holds until the next request, `open`, or client or browser launch.
12. **Sessions survive a service restart.** After `initialize` and after workspace changes the
    service hands the MCP connector an opaque resume token (the negotiated revision, client
    info, channel, the session's owned tabs). On reconnect the service checks the tabs against
    the live list; missing tabs go stale. Target and view handles go stale as today.

### Install and update

13. **One install root.** Every route installs into `~/.ghostlight/bin`. Any other copy of
    `ghostlight` is a carrier: if the installed service is absent or older, it installs from its
    own directory, then runs the installed binary. `doctor`, `status`, `help` and `--version`
    never install. A development build lives in `~/.ghostlight/dev` and is selected by a marker
    file; it wins over package invocations.
14. **`ghostlight deployment install`.** Take the install lock. Read `install.json`. Refuse an
    older payload; do nothing for the same version. Rename-swap the service image. Replace a
    connector only when its relay revision changes. Ask the old service to quit: it refuses new
    work and finishes in-flight calls within 10 s. If it has not exited by then, stop that exact
    service image (this fallback also covers services from before `quit` existed). Start `serve`,
    and write `install.json` only after it publishes. Then reconcile registrations: native hosts
    (reclaiming any manifest that names another path), owned client entries, the Linux desktop
    entry and man pages, the Windows Start-menu shortcut and Uninstall key. There is no rollback;
    a service that does not start is reported, and the next launch retries the install.
15. **Development.** `scripts/dev-loop.ps1` builds in an isolated target and runs
    `<build>/ghostlight deployment install --development`. Restore is offline and refuses when
    the installed connectors' relay revision does not match the release service.
16. **Channels.** npm (also used by the two client plugins and the MCP Registry) and the
    `get.ps1`/`get.sh` scripts remain. NSIS, Debian packages, MCPB, Scoop, WinGet, portable
    archives and the Homebrew template are retired at 2.0, not in a 1.3.x patch. The 2.0
    migration cleans their leftovers: Start-menu shortcuts and Uninstall keys of an old NSIS
    install, a `/usr/bin` copy that shadows the fixed path (doctor names `apt remove`), and native
    host manifests that name another path. A final MCPB carrying 2.0 is a release requirement,
    because MCPB runs its bundled installer on every Claude Desktop start.

### Product changes

17. **MCP moves into the service** (supersedes ADR-0096 Decisions 1-3). The CLI is an MCP client
    of the same stream. Results carry the envelope once, in structured content; the text block
    is the summary and next steps. `outputSchema` is dropped.
18. **Managed policy is an administrator-owned file** at the known-folder program-data path on
    Windows and `/etc/ghostlight/policy.json` on Linux. The file and its directory must be owned
    by Administrators or SYSTEM (root on Linux) and not writable by the user, or Ghostlight fails
    closed. HTTPS fetch, signatures, rollback protection, the cache, the Passport and the
    authoring commands are removed. Never phone home becomes structural: the binary has no
    network client.
19. **Governance knobs removed:** observe mode, channel enable switches, the user-policy enable
    switch, the frame handling and frame notice options (fixed at today's defaults), and the
    Attention runtime state with its control file. Policy schema 4 with `ghostlight policy
    migrate`; Ghostlight's own user file migrates automatically.
20. **Child tabs.** Ghostlight never adopts a tab on its own (MEMORY "Ghostlight manages only its
    own actions"). `browser_tabs` lists tabs whose opener is a tab this session owns, and the
    model may adopt one explicitly, governed like navigation. INTENT's automatic adoption is
    retired.
21. **Model surface:** every tool accepts `timeout_ms`; `layout_stable` merges into
    `visual_settle`; handles are short; a click that opens a dialog says so, and dialog status
    carries the bounded message; one recording per workspace.
22. **Removed:** the Flatpak route (this retires the Flatpak priority line in MEMORY), observed
    peer identity, the popup diagnostics toggle (the workbench and CLI keep it), the form-reset
    trace (its Options claim was false), the YAML editor for goose and Continue (they get Copy
    setup), migrations from pre-1.0 and versioned-directory layouts (they go through 2.0). The
    pre-break audit log is archived on disk and not displayed.
23. **Version 2.0.0** marks the break.
24. **The 2.0 tree reads pristine.** It carries no transition paths, compatibility shims,
    unused code, unused dependencies, migrations other than the one 2.0 break migration,
    stale journeys or fixtures, tests that pin source text, comments that narrate history, or
    lint suppressions without a stated reason. Historical records (ADRs, research, testing
    evidence) are preserved as history; current-state documents are rewritten to match the
    tree.

### Guarantees

25. ADR-0196 Decision 13, plus: a JavaScript lint for unused variables and undefined names
    over the extension and the page assets; a check that every Rust lint suppression states its
    reason; hashes of `crates/relay`, both connector crates and `extension/`
    are locked to their protocol revisions; per-target `cargo tree` snapshots for the connectors;
    golden protocol-4 fixtures are shared by `cargo test` and `npm test`; CI runs the process
    journey against the last published connectors and extension.

## Migration

Each step is one or more reviewed commits on dev that leave a green tree and a working
installation.

- **M1, service internals, no wire change.** Language table, typed next steps, the browser error
  type, `admit()`, the authority refresher, no probe quarantine, one-way presentation, the
  waiting thread kept, audit tail reload and private file modes, one history bound.
- **M2, the adapter component.** Page assets move to `browser/adapter/chromium/assets`; bridge
  modules no connector uses move into the service; level 3 is today's behaviour.
- **M3, install groundwork that works with today's connectors.** `install.json`, version
  ordering and refusal, npm command classification, `serve` and `quit`, the Windows console
  attach, startup reconcile, Start-menu and Uninstall rows. The exact-image stop stays as the
  fallback.
- **M4, the break set, unreleased.** The `relay` crate and both connectors; MCP in the service
  with resume; the fixed home, entry rule and development marker; the new dev loop; protocol 4
  with its extension, built beside today's path and then switched in one commit that deletes
  the old path on both sides; governance schema 4 and the removals; the pristine pass; the
  guards.
- **M5, release preparation.** Documentation reconciled, version 2.0.0, channel retirement in
  source, store text drafted, the release order (extension and service together) written down.
  Publication stays the owner's decision.
- **After 2.0 has spread,** a separate decision may delete the break migration and the
  exact-image stop fallback.

## Consequences

- About 12,000 production lines leave the tree and about 4,000 move out of frozen components into
  the service. Each connector drops from 35 to about 17 crates on Windows and from 85 to about 16
  on Linux. The service drops from 283 to about 265 crates on Windows.
- After 2.0, an ordinary update replaces only the service. MCP sessions keep their tab handles;
  the browser port recycles once.
- The 2.0 extension and service must be published together. Until both have reached a person,
  Ghostlight tells them which side to update instead of working.
- People who installed through NSIS, apt, WinGet, Scoop or MCPB move to the npm or script route
  at 2.0. The migration cleans up what those routes left behind.
- Enterprises that relied on HTTPS policy fetch or signed policy files deploy the policy file
  with their existing management tools instead.
- The Windows command line still uses the GUI subsystem. Attaching to the parent console makes
  output visible, but an interactive shell does not wait for it; the npm launcher remains the
  route that returns exit codes.
- macOS remains deferred for want of test hardware, as MEMORY records.

## Owner review amendment (2026-10-14)

The owner reviewed the decisions that change promises. Where this section and the text above
differ, this section wins.

- **Decision 16, channels.** WinGet stays. Its portable package works as a carrier: running
  `ghostlight install` through the WinGet alias installs into `~/.ghostlight/bin`, and after
  `winget upgrade` the next launch or install through the alias carries the newer version
  forward under the same no-downgrade rule. NSIS, Debian packages, Scoop, the separate portable
  download and the Homebrew template stay retired. MCPB retires after its final 2.0 release.
- **Decision 18 is withdrawn.** Managed policy keeps its HTTPS fetch, signatures, rollback
  protection, cache, Passport and authoring commands. The fetch runs only on the refresher
  thread, never on the invocation path, and the ownership check on the machine policy file
  stays. The owner's definition: phone home means communication outside the user's or the
  administrator's intent. An administrator-configured policy fetch is intended communication,
  so it is not phone home. The dependency savings in the Consequences shrink accordingly.
- **Decision 22, Flatpak.** Confirmed: the Flatpak route is removed and ADR-0165 is superseded.
  The owner's principle: Ghostlight is always user-space. It installs and runs per user and
  never tunnels through a browser sandbox or depends on system packages or privileged helpers.
- **Confirmed as written:** the governance knob removals (Decision 19), MCP in the service
  with connectors that handshake and start the service when it is not up (Decisions 2, 11 and
  17), Quit, child tabs, session continuity, the strict break and the small removals.

## Windows app container amendment (2026-10-14)

Live finding: a process started under an MSIX-packaged desktop app (Claude desktop, Codex) runs
in that app's container even though it reports no package identity. Its writes to `HKCU`,
`%LOCALAPPDATA%` and `%APPDATA%` go to a private per-app view that Chrome and every other app
never read. The user profile root, so the home, is not redirected. Breakaway attributes do not
leave the container, and starting a process through WMI or the Task Scheduler is an
EDR-flagged technique that Ghostlight does not use. Where this section and the text above
differ, this section wins.

- **Home layout.** `~/.ghostlight/manifests/` holds the native-host manifest on Windows,
  `org.sylin.ghostlight.json`. Every browser's `HKCU` registration value names that fixed path.
  After the first registration made outside a container, any later install, from anywhere, only
  rewrites the manifest file. macOS and Linux keep their browser-owned manifest directories,
  because those browsers read the file itself.
- **Read first.** A registry value is written only when it differs from what is read, so an
  install inside a container that finds the right value registered writes nothing.
- **Container rule.** A process is inside an app container when an ancestor's executable lies
  under a `WindowsApps` directory (the parent chain is walked to 16 levels through `win-peer`).
  Inside one, the install does everything else but leaves each registration that needs a
  redirected write unwritten: a browser's `HKCU` value, an MCP client configuration under
  `%APPDATA%` or `%LOCALAPPDATA%`, the Start-menu shortcut and the installed-apps key. Its report
  names the browser and client registrations it left and the one command that finishes them from
  a regular terminal, `<home>/bin/ghostlight.exe install`. `doctor` there says it cannot see what
  other apps see, instead of reporting registered. An uninstall inside a container is refused
  with the command to run, because it could only change the private view.
- **Migration.** Outside a container, an existing manifest in
  `%LOCALAPPDATA%\Ghostlight\NativeMessagingHosts` is rewritten into `manifests/`, the registry
  values are repointed, the old manifest is removed once no browser names it, and the old
  directory is removed when empty.
- **Development.** An agent that runs inside Claude desktop or Codex runs the install once from a
  regular terminal; later development loops then work from inside the container.
