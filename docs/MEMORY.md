# Project memory

Durable, model-agnostic memory for whoever works on Ghostlight next, read alongside
[`AGENTS.md`](../AGENTS.md).

One rule keeps this file short: **if the tree can tell you, this file does not.** State lives in
[`STATUS.md`](STATUS.md), decisions in [`adr/`](adr/README.md), contracts in [`1.0/`](1.0/), and
machine-local facts under ignored `local/`. What is left here is what none of those can say: what
the owner wants, and what this project learned the hard way.

## Standing owner directives

- **Preserve history.** Docs, ADRs, licenses, research, trust and legal material, task ledgers, and
  product identity survive internal rewrites. Reconcile active documents; never erase evidence.
- **Clean-room is not a resource wipe.** Old implementation code is not authority for new
  internals, but tests, fixtures, platform findings, CI, packaging, release evidence, and
  publication knowledge remain project assets. Inventory and translate them before deleting a
  working predecessor.
- **Release work must earn its place.** Keep the checks that prevent a real failure, prove a user
  promise, or make recovery safer. Do not turn restamping, duplicate checklists, optional directory
  submissions, or one giant conductor into release gates.
- **The artifact trust model is checksums plus keyless GitHub build-provenance attestations.**
  Ghostlight has no Windows code-signing certificate (the SignPath Foundation application is
  pending). Chrome API credentials are optional automation; manual Developer Dashboard submission
  is a supported release path. Do not invent either as a readiness gate without a new owner
  decision.
- **Preserve product identity; redesign internals deliberately.** The name, the original icon bytes,
  the visual language, the motion character, and user expectations are identity. Model-facing tools
  and descriptions are mechanisms the orchestrator owns and may redesign.
- **Prefer the root fix.** No wrapper, alternate id, guarded installer, or parallel protocol added
  to route around the abstraction that should own the change.
- **Understand the architecture before fixing.** Several failures in one capability means stop
  implementing. Map ownership, lifecycle, authority, state, and delivery; find the single owning
  seam; change it there. Restore a green checkpoint before proposing the change.
- **Fewest meaningful moving parts.** A logical boundary does not earn a process, crate, service,
  event bus, actor system, workflow engine, CQRS split, or registry.
- **Refactor authority is standing.** When work touches a weird-shaped seam, the owner has
  authorized refactoring toward a lean domain-driven monolith: fewest but most meaningful moving
  parts, each kind of complexity isolated at exactly one seam. Capture facts where they are
  learned, compose words where they are owned, and never let a boundary drop information a later
  surface must guess about.
- **Invisible when healthy, legible on demand.** Installation and ordinary browser work should
  succeed without a workbench ritual. Safe recovery is automatic and bounded at its owning seam;
  the workbench, controls, preferences, diagnostics, and CLI depth appear progressively. Behavior
  that can unexpectedly change the user's environment or attention, such as opening a browser or
  drawing on a page, has one small closed preference.
- **Manual browser recovery speaks to the model.** When browser auto-open is off, never make the
  model translate a person-facing error. Tell it to ask the user to open any eligible installed
  browser Ghostlight can name, with the extension installed, then repeat the call.
- **Plural evidence asks; unique evidence acts.** A person who cares which browser is used has
  one open already or names one, so Ghostlight never presents a browser choice and never spends
  a refusal saying it declined to choose (the owner's verdict on the ambiguity refusal,
  ADR-0149). Name every connectable browser, repair silently what Ghostlight already owns, and
  reserve launching for a unique candidate.
- **Diagnostics are a product surface, not a developer afterthought.** Process logs must be
  findable, readable, correlatable, and factual: one command to one chronological story,
  operation ids that follow one call across processes, terse dense lines carrying the numbers
  that matter, bounded retention, and honest gaps. A raw dump that needs manual stitching is a
  defect, not a v1 -- and so is a chatty one: the person chasing an error, human or agent,
  wants information, not prose (ADR-0145, the process-diagnostics batch).
- **One product across every machine.** The same words, controls, and truth on each computer a
  person uses, shaped to the desktop they are on: a tray where the shell has one, an Applications
  entry everywhere, a notification area on Windows, and never a single one of those as the only
  route. Platform behavior is a table with a row per platform, so macOS is a later row and some
  evidence rather than a rewrite. macOS is deferred for want of test hardware, not abandoned.
- **Ghostlight owns mechanism, not the user's larger intent.** It understands canonical browser
  operations, authority, lifecycle, observable browser state, and effect truth. It does not infer
  that a generic click or write means booking, buying, sending, or another task-level consequence.
- **Human runtime control is authoritative.** This is the reference-experience epic's intent, not
  current behavior: today a hold refuses at the final boundary rather than holding a caller, and the
  stop directive does not exist in the tree. The intended contract is that pause prevents the next
  browser effect, and that stop is terminal and tells the controller
  `The user asked to interrupt the process. Wait for further instructions.` The exact value is
  pinned in `docs/tasks/reference-experience/PINS.md` and owned by that epic's S5.
- **One desktop authority startup.** Connectors, CLI demand-start, and direct execution all launch
  the same no-argument desktop authority. It creates a tray where the desktop offers one and starts
  the workbench backgrounded: minimized on Windows and hidden on Linux. A session without a tray
  keeps the Applications entry and `ghostlight open`; there is no service-only launch mode.
- **Keep the fringes stable.** Product and journey change belongs in the orchestrator. The
  connectors negotiate and relay. The extension owns Chromium, the page, and the drawing, and makes
  no product, workspace, authority, or model-language decision.
- **Thin means nothing bleeds through the extension, not that the extension does little.** A
  capability that is physically the browser's belongs at the browser layer, because that is the only
  layer that has it. Counting responsibilities is the wrong test and has moved browser capabilities
  to the wrong side before (ADR-0053, corrected by ADR-0109).
- **Plural by design.** Sessions, workspaces, operations, browser instances, and future browser
  families are collections. Never build a singleton assumption into a new contract.
- **Keep browser work visible and user-placed.** Reuse the same-name Ghostlight group wherever the
  user put it; create a dedicated window only when none exists; never reclaim an unrelated one.
- **Keep visual evidence.** Model-driven close needs both orchestrator authority and the extension's
  preserve-tabs setting. Either refusal keeps the tab visible, and manual closure stays the user's.
- **Never phone home.** No telemetry, activation, update ping, audit upload, or hidden vendor
  dependency. An administrator-configured signed policy fetch from the organization's own file or
  HTTPS source is the explicit opt-in exception; it never contacts Ghostlight or another vendor
  endpoint.
- **Outward changes wait for the owner.** Local edits, tests, and commits are normal. Pushes,
  merges, tags, releases, store actions, and anything public are not.
- **Full integration means the active live graph.** An isolated target and every automated gate can
  be green while an older authority is still serving real connectors. Do not call that a full
  integration test. Identify exact live image paths, deploy and restart only the changed component,
  prove its existing shores renegotiate, and name any physical lane that still did not run.
- **A representative fixture does not prove a fixed roster is complete.** When a surface promises
  every supported product, assert the exact target ids and product cardinality, then verify the
  deployed live projection. A small fixture may prove layout, but it cannot prove completeness.
- **A vendor rename can be a configuration migration, not an executable alias.** Keep independently
  installed generations as concrete targets under one product identity when their paths differ.
  Otherwise detection can make setup write a valid file that the detected client never reads.
- **Persist before handoff.** Update STATUS, the relevant ADR or task evidence, and this file when a
  durable fact changes, and commit before writing a restart prompt.
- **Live swaps only through `scripts/dev-loop.ps1`.** Never hand-copy a binary over
  `target/release`, never kill-and-restart the authority by hand. The script's `deploy.lock`
  suppresses demand-start while it works; bypassing it makes the connector respawn the authority
  mid-swap, which produced two live instances and two workbench windows at once (2026-08-24,
  foundry press_key diagnosis). If the service seems stuck, deploy again through the script; do
  not improvise around it. Convergence after a version change also means hunting the superseded
  install's own long-lived connectors: an orphan connector demand-starts its exact sibling by
  path, so stopping a stale orchestrator alone just gets it respawned (2026-08-27, installed 1.0.0
  MCP connector kept reviving its orchestrator beside the deployed 1.1.0 authority). Stop the
  connector by exact path first; the parent harness then reconnects through its configured,
  current-path command.
- **A generic configuration root is not product detection evidence.** A config file directly under
  home, the platform config root, or roaming must exist itself, or the product needs independent
  executable evidence. Treating the generic parent as detection makes absent clients actionable
  and causes aggregate setup to overreach.

## Durable lessons

Every one of these cost something to learn.

- **A launcher channel re-verifies its download on every launch, so a hand-staged binary at its
  versioned cache path never runs.** `npx -y ghostlight` checksum-validates
  `~/.ghostlight/bin/<version>/` against the published manifest before each spawn and
  re-downloads on mismatch (2026-09-02, during the ADR-0150 verification): deploying to the npx
  stage means publishing a release, and verifying an unreleased connector belongs in a
  directory the launcher does not checksum. Routing, meanwhile, is configuration: the
  `GHOSTLIGHT_RUNTIME_FILE` override (ADR-0150) steers a floating launcher entry at the
  machine's real authority without touching its checksums.

- **Cross-tree registration adoption requires a deliberate install.** Silent registration
  repair once adopted the machine toward whichever tree crossed the no-browser seam, which let
  an un-isolated scratch build rewrite the real browser registration (2026-08-30 incident);
  ADR-0149's amendment narrows silent repair to same-tree stale details and reports every other
  installation's registration as `owned_elsewhere` with the owning directory named. Journeys
  isolate the registration surface behind `GHOSTLIGHT_NATIVE_HOST_DIR` regardless, and the
  runtime file lives beside each executable, so N installed trees legitimately mean N single
  authorities.
- **A journey that runs against the real machine pins the contract, not the inventory.** The CLI
  journey pinned the one no-browser sentence a single-registered-browser machine produces; every
  continuous-integration image carries two unregistered browsers and answered with a different
  honest refusal. Assert the closed set of honest answers with the exact sentence and facts for
  each reason, and keep machine-shaped exactness in unit tests over a controlled inventory.
- **A capability split across a boundary grows two implementations of one policy**, and they
  diverge. Recording thinning lived in the extension and in Rust at once; the Rust copy dropped each
  discarded frame's duration, so a thinned replay played back faster than the work it recorded.
  Fixed by moving the whole capability to the side that physically has it (ADR-0109), not by
  syncing the copies.
- **Put a rule in the shape, not in a reviewer's memory.** "Bytes never cross" survives as a
  variant that has nowhere to put them; a field everyone agrees not to fill does not. Where a
  budget differs by path, say so per path: one number for every path is a contradiction waiting to
  be discovered, and raising it only moves the contradiction.
- **Trade fidelity, never coverage.** A bounded recorder that stops at its limit produces a replay
  that silently omits everything after. Whoever drops a frame folds its time into the frame before
  it, or the artifact misreports how long the work took.
- **Say what a person would say.** A replay is "30 seconds of page changes", not "17 of 65 frames
  as 3804453 bytes". Mechanism belongs in the facts; the sentence is for a reader.
- **Model-facing names describe authority, not implementation APIs.** CDP calls the primitive
  `Runtime.evaluate`, but page JavaScript can mutate and navigate, so the tool is
  `browser_execute`. Keep physical vocabulary behind the language boundary.
- **Correctness kept by memory rots.** A hand-maintained list that each new case must join will
  eventually miss one. Derive it from a registry, or observe at the one seam every case already
  crosses.
- **A guard that parses nothing passes everything.** Check every source-scraping test against a
  negative control. A guard can also go stale in the same commit that makes it stale: once a
  rendered string carries a fact, asserting that the fact appears *separately* stops protecting
  anything and starts pinning duplication in place.
- **Replace only what changed**, decided from the per-crate source diff rather than the build
  output. A binary that merely recompiled is not a changed fringe, and swapping it costs a killed
  native host, an extension reload, and a browser reconnect for nothing.
- **A delegated batch spec must say what a change makes redundant**, not only what it adds, or the
  executor is correct and the surface is repetitive.
- **Audit is metadata-only.** Never persist page content, results, screenshots, form values,
  scripts, paths, or file bytes. Governed attempted or landed hosts and normalized bounded names of
  action targets are the deliberate exceptions: they answer where the agent went and which visible
  control it used. Target names default on for useful history and can be removed monotonically by
  governance. Paths, query, fragment, selectors, handles, and entered values stay out.
- **Observe the action subject at the effect boundary.** The browser already resolves the physical
  element. Return its role and accessible name in that same receipt. A cached inspect name or a
  second describe call is both less truthful and more expensive.
- **Reconnection is not availability, and attachment is not availability.** Put one idempotent
  recovery action at the failed-connection seam, then let a service-held lifetime lease decide
  authority before discovery or presentation state exists. At the browser shore, only an
  end-to-end acknowledgement proves that the extension consumes the attached relay stream;
  operation silence cannot prove the opposite because healthy browser work may be quiet.
- **Async connection entry points must be single-flight.** A guard checked only before an `await`
  does not establish exclusive ownership. Startup, installation, and retry signals can all pass it,
  create competing native hosts, and leave two structurally live pipes with only one active
  listener. Give connection creation one shared in-progress attempt and recheck ownership after
  every asynchronous initialization boundary.
- **Consume callback-scoped platform errors before any ownership guard can return.** Chrome reports
  a native-port exit only inside `onDisconnect`. A stale-port guard that returns before reading
  `runtime.lastError` turns an expected replacement into an unchecked extension error even when the
  current port is healthy.
- **A cached MCP catalog is not a live transport.** Reconnect through the owning client, then look
  at the visible browser before retrying an effectful call.
- **Standards-valid MCP is not the same as current-harness compatible.** Real clients in one Linux
  roster requested four initialized revisions, used the newer discovery fallback, and rejected
  root-level JSON Schema composition in a downstream model API. Test each admitted harness through
  its real process and model path, advertise only revisions the connector actually serves, and keep
  portability fixes generic rather than branching on a client name.
- **A native-port or service-worker restart is not a browser restart.** Hold uncertain resource
  state until an exact generation or terminal evidence resolves it.
- **A loaded document is not mounted presentation.** That takes a ready handshake, exact document
  acknowledgement, and packaged reinjection.
- **Chrome native messaging has directional size limits.** Generic corruption ceilings and browser
  chunking are different contracts.
- **A clean screenshot is not evidence that feedback failed.** Capture deliberately suppresses the
  extension's visual layer, so verify visuals externally.
- **Persistent scope and transient activity are different visual promises.** The border says what is
  controlled; cursors, scans, ripples, frames, and captions say what is happening now.
- **Isolate live stacks when testing.** Build into a separate target directory, and stop processes
  only by exact executable path, never by image name.
- **Persistent package-test overlays need candidate-scoped user state.** A retained runtime file can
  satisfy a file-exists wait before the new authority publishes, producing a truthful result about
  the wrong candidate. Namespace the test home by candidate, and pass the effective Cargo target
  directory explicitly when packaging so a host profile cannot redirect the binaries under test.
- **An internal rewrite does not reset the user experience.** Preserve observable commands,
  launchers, packages, identity, tests, and accumulated platform evidence unless the owner changes
  them explicitly. The npm launcher is a mandatory Ghostlight entry point, and 1.0 may not ship
  with a user-experience regression from 0.8.
- **A Windows GUI-subsystem executable is not a normal PowerShell pipeline child.** Ghostlight keeps
  its console-free desktop launch, but scripts that invoke the binary directly must use an explicit
  waited process with redirected stdout and stderr. `$LASTEXITCODE` and `&` are not a reliable CLI
  boundary for that executable shape.
- **Probe the intended native window, not a process-level main-window guess.** Tray helpers,
  event-loop helpers, and console hosts can make `MainWindowHandle` look healthy after the actual
  Tauri workbench has died. Windows desktop acceptance identifies the exact `Ghostlight` / `Tauri
  Window` HWND and checks its visible, minimized, Close, activation, and recreation states.
- **Diff the public surfaces by name, not just the source tree.** The clean-room inventory compared
  implementation and tests and still dropped the README's hero GIF, badge row, and onboarding
  spine, the Homebrew formula template, the website publish path, the icon generator, and the
  store-justification length guard -- a file-level "missing on the new branch" list reads as
  intentional rewrite when it is really identity and release machinery. Root documents,
  `packaging/`, `scripts/`, and legal guards need an explicit main-vs-branch reconciliation before
  any rewrite is called complete (restored 2026-08-25).
- **Two frame-id vocabularies do not translate; the parent's DOM is the truth.** CDP names frames
  with strings, `chrome.webNavigation` with numbers, and the tab-level debugger session cannot see
  out-of-process frames at all, so any bridge built on CDP identity silently covers only
  same-process frames. The parent document's own DOM answers honestly: match the embed element by
  URL, take its content-box origin, and compose offsets recursively (ADR-0138). One mechanism for
  same-origin and cross-origin beats a fast path plus a fallback that fails exactly where the
  fallback was for.
- **An anchored effect without a live box renders nowhere.** A zero-size or hidden target resolves
  to the frame origin, and inside an embedded frame the frame boundary clips the effect in half.
  Suppression is the honest rendering of "nothing to point at."
- **A panic after the effect is the worst failure shape; prove every success arm's expect.** Focused
  typing discarded the describe step's observation and then `.expect`ed a fallback subject that did
  not exist. The typing landed, the operation task died, and the workbench showed "Typing" forever
  -- no error, no deadline, no recovery, across reconnects. When a success arm asserts a fallback,
  the fallback must be produced on that same path (ADR-0138's describe now feeds the receipt), and
  a test must walk the full success path, not just the refusals around it.
- **Per-host reproducibility is not determinism; pin every platform-derived byte.** The extension
  ZIP was "deterministic" (two runs byte-identical) on each host and still differed across hosts:
  `ConvertTo-Json` writes the platform newline into rewritten JSON, and .NET's `ZipArchive` stamps
  the central directory's host-system marker and Unix mode bits from the running OS. Identical
  sources, two archives, 99 differing bytes of pure metadata -- and only one of them was the
  reviewed Chrome Web Store artifact. A packager used for publication must pin serialization and
  container fields explicitly to the reviewed artifact's exact shape, and a release must prove
  determinism across operating systems, not just across runs on one machine.
- **A READY card covers only the roster.** Registration state is per-client, read from that
  client's own configuration; a client with no registry row is invisible no matter how broken
  its hand-written entry is, and one client's green card never contradicts another's failure
  (2026-08-27: Zed READY while ZCode failed on an orchestrator-binary command). Adding client
  support means a registry row with the client's real config dialect, pinned against what the
  client itself writes -- never the assumption that a known sibling binary doubles as a stdio
  server.
- **An npm maintainership move needs a one-time code that a passkey-only account cannot
  produce.** The website has no organization-as-maintainer flow (the invite box resolves
  users only), and npm's 2026 restriction bars bypass-2FA tokens from account changes, so
  the release token cannot run `npm owner add` either. The one org grant the website can
  make is team package access (org settings, developers team, add existing package), which
  does not change the public maintainers list. Finishing a transfer takes a recovery code
  or an npm support request; pin the maintainer assertion in the online truth check only
  after the flip is observable.

## Where to look

| Need | Source |
| --- | --- |
| How to work here, and the boundaries | [`AGENTS.md`](../AGENTS.md) |
| What is true right now | [`STATUS.md`](STATUS.md) |
| A map of this documentation tree | [`README.md`](README.md) |
| Intent, language, architecture, acceptance | [`1.0/`](1.0/) |
| Every decision, and why | [`adr/`](adr/README.md) |
| Build, restart, deploy, validate | [`DEV-LOOP.md`](DEV-LOOP.md) |
| Task-oriented guides for people | [`guides/README.md`](guides/README.md) |
| Design notes, living and dated | [`design/README.md`](design/README.md) |
| What each task batch was, and where it stopped | [`tasks/README.md`](tasks/README.md) |
| The source licensing boundary | [`../LICENSING.md`](../LICENSING.md) |
