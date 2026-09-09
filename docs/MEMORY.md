# Project memory

Durable, model-agnostic memory for whoever works on Ghostlight next, read alongside
[`AGENTS.md`](../AGENTS.md).

One rule keeps this file short: **if the tree can tell you, this file does not.** State lives in
[`STATUS.md`](STATUS.md), decisions in [`adr/`](adr/README.md), contracts in [`1.0/`](1.0/), and
machine-local facts under ignored `local/`. What is left here is what none of those can say: what
the owner wants, and what this project learned the hard way.

## Standing owner directives

- **The four remote machines are dedicated test resources.** The owner grants the coordinator
  and each machine's local agents full control of test-01, test-02, test-03, and leo-desktop-02
  for project testing: prerequisites, desktop/browser interaction, installation, packaging,
  and controlled restart, reboot, upgrade, and recovery work. Preserve the baseline first and
  use that authorization without repeatedly asking. This does not authorize public releases or
  access to founder-private material. The [fleet campaign](testing/fleet-acceptance-2026-09.md)
  tracks execution and evidence.
- **Ghostlight manages only its own actions.** Human browsing creates no Ghostlight activity,
  audit, policy check, tab hold, automatic child-tab adoption, or regrouping. A controlled opener
  and an in-flight command do not prove that Ghostlight caused a browser event. Silently stale
  agent references when the page changes; govern the next actual agent request (ADR-0164).

- **Resilience through leniency is a product-wide rule.** Tolerate harmless differences, unknown
  optional information, and unavailable nonessential components. Use safe defaults and bounded
  automatic recovery; keep unaffected work available instead of rejecting the whole operation or
  installation. Users should not have to repair routine incompatibilities. Be strict where
  continuing would misinterpret an action, lose data, or bypass authority; do not silently guess
  those semantics. Versioned history is one application, not the scope of this directive
  (ADR-0163). Prove recovery at real boundaries, preserve evidence, and never replay uncertain work.
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
  Ghostlight has no Windows code-signing certificate. Chrome API credentials are optional
  automation; manual Developer Dashboard submission
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
- **Delight is the purpose; dependable boundaries support it.** Ghostlight makes integrated
  tooling useful and natural for end users. Governance lets individuals and organizations choose
  reliable boundaries that work consistently whenever work passes through Ghostlight. Ordinary
  permitted work should stay easy, and limits and outcomes should be understandable when they
  matter. This purpose guides the security-hardening epic; it is not a claim of host containment.
  For that epic, the owner requires an ideation session before implementing a package with
  unresolved product decisions. Preserve accepted decisions and proceed with independent agreed
  work; the epic's IDEATION and LEDGER separate the two.
- **Host restrictions belong to policy.** The owner removed the built-in localhost, loopback, and
  link-local ban. All-open includes local HTTP(S) browser work; do not reintroduce a hard-coded
  address block, a local-access toggle, or an exception flow (ADR-0155).
- **Model calls describe work; people configure authority.** The owner removed caller-supplied
  host/capability restrictions and flow dry-run, and consolidated sequence into ordinary flow
  steps. Do not recreate a model-authored policy layer or simulation mode under another name.
  Configured governance and historical evidence remain intact (ADR-0162).
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
- **Absorb ordinary bursts.** The owner rejected a refusal-led local-resilience experience. Keep
  permitted work flowing within its original deadline, make sustained waiting legible, contain
  stalled connections, preserve other sessions and human controls, and recover without replay.
  Capacity bounds serve continuity; people should not have to tune service machinery (ADR-0160).
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
- **Human runtime control is authoritative.** ADR-0126 decides that pause refuses the next effect
  instead of suspending a caller, and stop is terminal with a pinned directive. Both directives
  already live in `crates/orchestrator/src/language/outcome.rs`; the reference-experience S5
  records their implementation. Preserve those decisions when testing effect-boundary timing
  and scope. Do not rebuild missing language from an older memory entry.
- **Automatic attention belongs to its session.** Global human controls stay independent.
  Explicit session recovery uses the exact incident, permits new requests without replay, and
  changes no permission. A composition that triggered attention stops even if recovery races its
  completion. Dispatch checks happen after writer wait; a refused follow-up observation cannot
  erase an acknowledged action (ADR-0157).
- **Client provenance has a bounded purpose.** Ghostlight governs its invocation route and assumes
  host integrity; it does not contain independent desktop automation. Record claimed and observed
  identity separately. Optional signer/hash admission needs a concrete verifiable peer; proving
  Ghostlight's connector does not prove its upstream MCP application. ADR-0105's September 6
  amendment records the agreed staging; the security-hardening ledger owns progress.
  Evidence belongs to the connection that submitted each action, independently of shared workspace
  continuity (ADR-0161). A socket-owner test needs different processes at its endpoints: a
  same-process loopback test cannot reveal an observer that selects its own socket owner.
- **Saving history and doing browser work have separate outcomes.** Keep working is the default;
  optional Require audit stops new browser work during known storage failure. Recovery is bounded,
  replays neither actions nor receipts, and leaves gaps explicit. A saved parent does not confirm
  its children. Human controls remain independent (ADR-0159).
- **Embedded authority follows the document.** H6/ADR-0158 bind access to Chrome document identity,
  not reusable frame ids. Handling and notice preferences are separate. Excluded hosts belong
  only in volatile human details; model/audit coverage stays content-free. Screenshots mask before
  capture, unverifiable captures refuse, restricted recordings stop at document-set changes, and
  replay export checks every captured source. Scripts have no inferred containment mechanism.
- **One desktop authority startup.** Connectors, CLI demand-start, and direct execution all launch
  the same no-argument desktop authority. It creates a tray where the desktop offers one and starts
  the workbench backgrounded: minimized on Windows and hidden on Linux. A session without a tray
  keeps the Applications entry and `ghostlight open`; there is no service-only launch mode.
- **One process does not prove one workbench.** A startup/Open race created two responsive native
  windows inside one authority. Publish activation only after startup construction finishes;
  Tauri's label lookup does not reserve a window under construction. Native lifecycle acceptance
  counts actual windows, including hidden ones, through startup and close/reopen bursts. A mock
  activation port or process count cannot prove this promise (ADR-0119's September 7 amendment).
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
  The owner's default is acceptance on the installation in place with a normally launched browser.
  Do not switch to a separate installation, profile, runtime, or agent-launched browser to obtain
  a pass. Isolated checks can supplement that evidence but cannot establish that installation,
  startup, or recovery works for the user. A separate build directory avoids locked files; it is
  not authorization to run a second desktop installation.
- **Live debugging uses the installation in place.** The owner does not want a second Ghostlight
  authority, an alternate diagnostic build running beside it, startup helpers, or diagnostic
  shortcuts scattered around the OS. Build away from locked files, deploy through the dev loop,
  and exercise only the normal installed graph. Keep diagnostic capability in the product's
  existing toggles and log locations. Ask before running a separate desktop test installation
  during an active live-debugging session.
- **Installation must connect without restarting the browser.** Installing the service and extension
  must make an already-running browser usable. A complete Chrome shutdown is not an acceptable
  setup requirement. Installation acceptance uses the actual browser, native-host registration,
  extension, and installed executables; a replacement native pipe cannot prove that promise.
- **The installer's filesystem view is not the browser's view.** Packaged desktop callers can
  redirect AppData writes while their checks still see the logical path. Register the physical
  manifest path and test against a browser launched independently. A successful browser launch
  from the same caller can hide the failure; elevation and matching user identities do not prove
  an unredirected filesystem view (ADR-0115's September 8 amendment).
- **Preserve the failure before changing its environment.** Capture persistent product diagnostics,
  exact process identities, and the failing consumer's actual OS operations when local checks
  disagree with it. A restart that makes a failure disappear is evidence, not a root-cause fix.
  Record every changed condition, prove the correction on the running installed stack, and repeat
  the original trigger before closing the incident. The Windows boot case required both recovery
  in the unchanged Chrome process and a successful ordinary reboot.
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
- **Foreign-entry safety distinguishes automatic discovery from reviewed repair.** Setup, update,
  remove, and aggregate setup must not overwrite an uncertain command. A person who sees the
  bounded evidence and confirms Fix may authorize replacement of that one parseable entry under
  Ghostlight's own key. Re-check at the writer, preserve siblings, and back up first; malformed
  whole documents remain manual (ADR-0154).

## Durable lessons

- **Warm MCP success does not prove cold desktop demand-start.** A client can
  filter DISPLAY, Wayland, XDG runtime and session-bus variables from its stdio
  child. Test from no authority through the real client, inspect only the named
  environment boundary, and forward variable names through the client's supported
  configuration instead of capturing machine-specific values. See the
  [Bluefin evidence](testing/bluefin-fleet-2026-09-09.md).

- **Release identity precedes asset transport.** GitHub redirects release assets to a signed CDN
  URL that does not identify the release tag. Resolve the latest release page first, validate the
  stable tag, then pin checksums and every sibling download to that one release. The actual Linux
  one-line installation and `tests/installer-shell.mjs` caught the old final-asset-URL assumption.
- **Fixed archive timestamps do not guarantee reproducible bytes.** .NET's Pax writer also puts
  the packager PID into extended-header names. The portable package's fixed short roster uses
  Ustar now; test it in separate packager processes, since two calls in one process miss this bug.
  Compression-runtime identity remains part of the reproduction environment.

Every one of these cost something to learn.

- **A structured result must satisfy its advertised output schema.** Adding
  `history_storage` to the serialized receipt without updating the closed schema
  made both MCP Inspector and OpenCode reject every tool result (test-03 Alpine
  fleet, 2026-09-09). A raw JSON-RPC journey did not expose that client validation.
  Compare actual serialized fields against every catalog output schema and retain
  at least one real client that validates structured output.


- **A filled DOM is not proof that a controlled editor retained the draft.** Reddit discarded
  `textContent` plus generic synthetic input while Ghostlight reported success. Native browser
  editing preserves the editor's transaction; regression fixtures must reject the old mechanism,
  and installed verification must read the retained editor value or inspect a screenshot.
- **Guardrail tests must prove permitted work too.** A caller supplying exactly the advertised
  capability set must complete the operation, including its landing and export checks. Assert
  actual retained values, effect counts, and captured pixels independently of receipts. Form-batch
  validation must cover every frame before the first edit; checking each field only when its turn
  arrives can change an earlier field before discovering a known invalid later one. The full
  hardening suite and its feature map live in `docs/tasks/security-hardening/regression-suite.md`.
  A tool name alone is not a complete input matrix: handles, selectors, and postconditions can
  require different capabilities. Advertise and admit the complete request before effects, while
  keeping the action's landing requirement separate from earlier lookup and later observation.

- **Mocked debugger replies do not prove JavaScript completion semantics.** H3's real Chromium
  lane found that `awaitPromise` plus `replMode` returned an async wrapper's promise as `{}`;
  explicitly awaiting the selected wrapper produced its intended value. Preserve real engine
  evidence when changing parsing, execution, or effect classification (ADR-0133's H3 follow-up).

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
- **History helps people resume work.** Readable history is the default, with detail on demand
  and policy controlling retention. State what happened plainly; explain uncertainty when it
  changes the next step. Displaying more existing detail never authorizes capturing more content.
  Additional retention profiles and richer diagnostic capture need a concrete user benefit
  and a separate decision (ADR-0103 H1 amendment).
- **Composition history is one expandable account.** Retain safe receipts as children finish,
  with the parent's snapshot and positional correlation. Missing completion is missing evidence,
  never proof that work did not run. Explain allowance from the actual evaluated grants as well
  as refusal. Expanded details stay open without extra alerts; wrappers never repeat action or
  denial counts. Automatic resumption needs a separate decision (ADR-0156).
- **Completed means succeeded.** Flow and sequence keep unsuccessful and unreached work distinct.
  Continue permits later work, never a false overall success. Known partial progress stays known;
  recovery respects confirmed changes and never proposes replaying the entire composition.
  Human controls and invocation limits take precedence (ADR-0133 H2b amendment).
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
- **Capability coverage does not transfer between sibling observation paths.** Shadow-aware form
  inspection did not make page reading shadow-aware, and explicit visible reads did not make the
  shortest read use that path. Trace the default call through decoding, dispatch, negotiation, and
  the page-local collector. Chromium `innerText` stops at shadow boundaries; a composed read needs
  an explicit open-root and slot traversal, plus a capability revision so an older adapter cannot
  answer with narrower semantics (ADR-0151).
- **A composed page is a shared semantic and geometry boundary.** Fixing full-page text still left
  waits, find, document trees, point receipts, target capture, and drops on narrower DOM or frame
  paths. Put open-root traversal, visibility, frame aggregation, and point routing at shared
  adapter seams, then revision every affected capability family so old adapters refuse instead of
  returning plausible partial truth (ADR-0152 and ADR-0153).
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
  The runtime matters too: on 2026-09-08, PowerShell 7.5.2 and 7.6.5 produced identical extracted
  adapter files but different compressed ZIP bytes. Only 7.6.5 reproduced the submitted 1.1.2
  hash. Record the packaging runtime with artifact custody; fixed ZIP metadata alone is insufficient.
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
| Dated security assessment and evidence | [`design/security-assessment-2026-09-06.md`](design/security-assessment-2026-09-06.md) |
| Security-hardening epic, decisions, dependencies, and acceptance | [`tasks/security-hardening/EPIC.md`](tasks/security-hardening/EPIC.md) |
| Unresolved hardening choices for ideation before implementation | [`tasks/security-hardening/IDEATION.md`](tasks/security-hardening/IDEATION.md) |
| Agreed security-hardening scope and current progress | [`tasks/security-hardening/LEDGER.md`](tasks/security-hardening/LEDGER.md) |
| What each task batch was, and where it stopped | [`tasks/README.md`](tasks/README.md) |
| The source licensing boundary | [`../LICENSING.md`](../LICENSING.md) |
