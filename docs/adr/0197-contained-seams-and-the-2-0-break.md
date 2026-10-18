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
- **Container rule.** A packaged ancestor alone does not make a container: Windows Terminal,
  Store PowerShell and other packaged apps run in the open. The parent chain (walked to 16
  levels through `win-peer`) names the packaged apps the process descends from, meaning images
  directly below a package directory of `<drive>:\Program Files\WindowsApps` (or
  `<drive>:\WindowsApps`); the per-user `%LOCALAPPDATA%\Microsoft\WindowsApps` alias directory
  does not count. The process is inside the container of the first such app for which the effect
  is confirmed: the install writes a probe file below `%LOCALAPPDATA%`, removes it afterwards,
  and finds it also in `%LOCALAPPDATA%\Packages\<family>\LocalCache\Local`, where `<family>`
  is the package name and publisher id of the package directory. Inside one, the install does
  everything else but leaves each registration that needs a redirected write unwritten: a
  browser's `HKCU` value, an MCP client configuration under `%APPDATA%` or `%LOCALAPPDATA%`,
  the Start-menu shortcut and the installed-apps key. A client that is the container's own app
  is the exception, because the private view is the one it reads: Claude Desktop's
  configuration is written normally inside Claude's container and left private inside any
  other. Its report names the browser, client and Start-menu registrations that were out of
  date and left, and the one command that finishes them from a regular terminal,
  `<home>/bin/ghostlight.exe install`; this holds for `ghostlight native-host install` too.
  `doctor` there says it cannot see what other apps see, instead of reporting registered. An
  uninstall inside a container is refused with the command to run, because it could only
  change the private view.
- **Migration.** Outside a container, an existing manifest in
  `%LOCALAPPDATA%\Ghostlight\NativeMessagingHosts` is rewritten into `manifests/`, the registry
  values are repointed, the old manifest is removed once no browser names it, and the old
  directory is removed when empty.
- **Development.** An agent that runs inside Claude desktop or Codex runs the install once from a
  regular terminal; later development loops then work from inside the container.

## Packaged clients seen from outside (2026-10-17)

Live finding on the owner's Windows machine: Claude Desktop from the Microsoft Store (package
family `Claude_pzs8sxrjxfjjc`) keeps its settings in
`%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude\claude_desktop_config.json`.
The service runs outside any container, read only `%APPDATA%\Claude`, and said Claude Desktop was
not installed, so no Store user could set it up from the window. The Windows app container
amendment above decides what an install does from inside a container. This amendment decides how
Ghostlight, outside, finds a packaged client's settings. The owner asked for a process that is
"resilient and adaptive", so there is no table of package families.

- **Discovery on every look.** For a client whose configuration sits below `%APPDATA%` or
  `%LOCALAPPDATA%` (relative path `<rest>`), the candidates are the plain path and the same
  relative path in each installed package's private copy of that root:
  `%LOCALAPPDATA%\Packages\<family>\LocalCache\Roaming\<rest>`, or `LocalCache\Local` for a
  `%LOCALAPPDATA%` path. The evidence is the client's own settings folder, not only its file, so a
  fresh Store install that has not written the file yet is found and set up. A look lists
  `Packages` once, keeps the packages that have such a private copy, and tests only the settings
  folder the catalog names for each client. There is no recursive search and no registry read.
  The next look finds a Store edition installed or removed since.
- **Choosing a copy.** Ghostlight uses the copy the agent reads, ranked by ownership. A copy in a
  package of the agent's own app beats the plain path, because that app reads its private copy
  first. The agent's own app is a package whose family matches the catalog's packaged app or Store
  icon names for that agent. A copy in any other package, such as one an install run inside
  another app's container left in that app's view, beats the plain path only when neither the
  plain settings file nor its folder exists, so a Store edition the catalog does not name is still
  found. Among copies of the same rank, the one written most recently wins (its file, or its
  folder while it has no file). When the chosen copy goes away, the next look falls back cleanly.
- **A Store folder without its file.** Windows shows a packaged app a merged view: while its
  private folder holds no settings file, the app reads the plain file. So when the agent's own
  folder has no settings file and the plain file exists, the plain file is the one Ghostlight
  reads and writes. Inspection reports what the app loads, and the person's other servers stay
  visible. Ghostlight never creates a private file holding only its own entry over a plain file.
- **What Ghostlight says.** The Integrations pane and `doctor` name the chosen file and say it is
  the Store edition's own copy only when it is in the agent's own package. When another copy of
  the settings exists, they say in one sentence where Ghostlight's entry goes and where the other
  copy is. Another package's copy is never called the Store edition's. It is never a refusal.
- **A plain copy left behind.** A plain `%APPDATA%` file from an earlier non-Store install is not
  the one the Store edition reads once it has its own file. Inspection reports the private copy and
  Install, Fix and Check do not edit the plain one. Uninstall removes Ghostlight's own entry from
  every copy that holds one, since earlier installs may have written either; a foreign entry stays.
  `ghostlight uninstall` and the product's removal take the agent whenever any copy holds
  Ghostlight's own entry, so they reach an entry left only in the plain copy.
- **Containers are unchanged.** Inside the client's own container, the plain path the private view
  shows and the private copy are one file. It is written normally and is not named as another copy.
  Inside any other container the private copy lies below `%LOCALAPPDATA%` and is left with the one
  command that finishes it from a regular terminal. Ghostlight still uses no WMI, Task Scheduler or other container escape.
- **No change elsewhere.** A client found only at its plain path behaves as before, and Linux has no
  packages to look at.

## Amendment 2026-10-14: connector source lock

Connector and relay source is locked to the last published tag at the same relay revision. The
check is `tests/guards/connectors.test.mjs`, run in CI, not a committed hash file. It diffs
`crates/relay`, `crates/mcp-connector` and `crates/browser-connector` against that tag and fails
until `RELAY` is bumped. Test directories (`crates/*/tests/**`) are excluded; inline unit tests
still count. The `extension/` half of Decision 25 (page-asset hashes) is unchanged by this
amendment.

The per-target `cargo tree` snapshots under `tests/golden/cargo-tree/` pin crate names, not
versions, so a routine `cargo update` needs no refresh. The guard also fails if any connector tree
contains a network crate (hyper, reqwest, ureq, h2, http, rustls, native-tls, openssl, curl,
isahc, tokio, socket2 or mio), which keeps ADR-0028 (never phone home) true by construction. In CI
an unresolvable tree fails; outside CI it is skipped.

## Amendment 2026-10-16: the JavaScript lint is withdrawn

Decision 25's JavaScript lint for unused variables and undefined names is withdrawn. Nothing ran
it, and it would report on frozen extension files that cannot change without a store
resubmission (ADR-0200 Decision 16). The 2.0 guard set is the one ADR-0199 Decision 21 lists.
`node --check` over runtime JavaScript and the page-asset tests stay.

## Amendment 2026-10-17: what the MCP edge sends again

Decision 2 answered every outstanding request as cut off when the service went away. That is
right for a request that may have taken effect, and wrong for two kinds that cannot have. The
owner authorized the change on the Linux batch (linux-1), where a real installation showed the
agent an error for its tool listing after a service crash: the listing had reached the dying
service's socket before the edge saw the stream end.

- A line whose write failed never reached the service, because the service acts only on a whole
  frame. It is held like a line sent while the service was away, keeps its place and its hold
  time, and loses only its answers to the connection that failed.
- A lone `initialize`, `tools/list`, `prompts/list`, `resources/list` or
  `resources/templates/list` changes nothing. When a lost connection or a `replace` goodbye cuts
  it off, it goes back to the front of the hold and is sent to the next service, once. Cut off a
  second time, it is answered as cut off, so a request that brings the service down cannot keep
  doing so. A batch is never sent again.

Every other cut-off request is still answered from the service's template and never sent again.
After `quit`, `uninstall` and `session_end` the listings are answered too, so Quit still holds.
The agent sees its answer instead of an error for the requests it cannot get wrong, and an honest
"may have taken effect" for the ones it can. The rules are in `crates/edge/src/forward.rs` and
`hold.rs`; relay protocol 4 is unpublished, so the edge source lock does not apply yet.

## Amendment 2026-10-18: the folders 1.x left

Decision 16 has the 2.0 migration clean the leftovers of the retired channels. Two folders of 1.x
itself stayed on upgraded machines, and nothing in 2.0 reads either: the policy folder the
migration empties, and the 1.x window's web view data. The 2.0 policy lives under the home, and
the 2.0 window keeps its own web view data there.

- On Linux (linux-1, L10), every install, migrating or not, removes `XDG_STATE_HOME/ghostlight`
  when it is empty and `XDG_DATA_HOME/org.sylin.ghostlight` when it holds nothing but WebKitGTK's
  web view data.
- 2026-10-18, Windows: the same rule removes `%LOCALAPPDATA%\Ghostlight` when it is empty and
  `%LOCALAPPDATA%\org.sylin.ghostlight` when it holds nothing but WebView2's `EBWebView` folder.
  On the owner's machine the window folder held only `EBWebView` (38.8 MB) and the policy folder
  was empty. Inside an app container the install leaves both for the next install outside it,
  because its removals below `%LOCALAPPDATA%` would reach only the container's private view.
  The managed bootstrap under `%PROGRAMDATA%\Ghostlight` is never touched.

A folder that holds anything else is left, a removal that fails is left for the next install, and
the install never fails on a leftover. The install names each folder it removed. The rule is
`migration::retire_leftovers` in `crates/orchestrator/src/install/migration.rs`.
