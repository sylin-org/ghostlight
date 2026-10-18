# Project memory

Durable, model-agnostic memory for whoever works on Ghostlight next, read alongside
[`AGENTS.md`](../AGENTS.md).

One rule keeps this file short: **if the tree can tell you, this file does not.** State lives in
[`STATUS.md`](STATUS.md), decisions in [`adr/`](adr/README.md), contracts in [`1.0/`](1.0/), and
machine-local facts under ignored `local/`. What is left here is what none of those can say: what
the owner wants, and what this project learned the hard way.

## Durable release learnings

- **npm acceptance, metadata, and tarball delivery can become visible separately.** A successful
  publish can enter asynchronous processing; version and latest metadata can then appear while
  the ordinary tarball URL still serves a cached 404. Keep the accepted publication, wait for
  the public tarball, verify its candidate hash, and run a fresh consumer before calling delivery
  complete. Do not republish the immutable version to repair propagation. The 1.3.12 custody
  record preserves this observed sequence.

- **Guard exact public versions and derive catalog documentation from source.** A minor-line check
  accepted README 1.3.6 while the public service had reached 1.3.10, and the implemented
  `browser_workspace` tool (since removed by ADR-0196) raised the catalog to 24 while active
  language documents still said 23. Public surfaces must match docs/public-status.json exactly
  (scripts/check-public-surfaces.ps1); LANGUAGE.md headings and the README count derive from
  the catalog golden docs/1.0/catalog.json (scripts/check-repository-integrity.ps1).

## Standing owner directives

- **Only main and dev.** The owner directed this on 2026-10-05. Main contains the
  published product; every new change and release candidate comes from dev. No
  feature, agent or release branches. Archive tags retain earlier branch heads,
  and stashes retain pending edits. Do not merge unfinished trials into main.


- **The browser offering is Chromium-only.** Chrome, Edge, Brave, and Chromium are the
  supported browsers. Do not describe a Firefox adapter as paused, planned, or
  awaiting parity in current product contracts or public guidance (ADR-0184). Preserve
  the old records and the owned-registration migration cleanup.

- **Release credentials stay repo-local and gitignored.** The owner requires npm credentials in
  `local/.npmrc`, not the shared home-directory npm configuration. Run login and release commands
  with that explicit user config; preserve owner-private file permissions. The existing Chrome/MCP
  release credentials remain in `local/.ghostlight-release.env`. Never publish values. The owner
  authorized durable Chrome/MCP release OAuth credentials in `local/.ghostlight-release.env` for
  release work; commands, saved-identity lookup and recovery are in [the release
  procedure](RELEASE.md#chrome-api-procedure), and the release scripts default to that file.
  Having credentials does not authorize a publication: publish only under owner authorization for
  that specific release, and reuse such authorization already given in the conversation instead of
  asking again.

- **Ghostlight usage MUST be delightful.** Deployment-induced split authorities are a product
  defect, not browser troubleshooting for the user. One production installation per OS user;
  installing or invoking a package on a development machine must use the selected development
  authority. ADR-0167 records the mandate and supersedes installation-local production election.
  The owner's 2026-10-02 realignment defines delight as quiet confident success, not reasonable
  error messages. Judge complete journeys by first-attempt completion, unnecessary calls, human
  interruptions, reliable confirmation, and cleanup. ADR-0192 records the bounded local candidate;
  its fixture evidence does not replace independent native acceptance.

- **Use fixed install paths and the existing injector.** The owner rejected per-release
  installation directories and an additional deployment framework on 2026-10-02.
  Package downloads replace one permanent sibling set. Ordinary service updates carry
  the current injected page runtime through the existing extension, without extension
  restamping or reload. When privileged extension plumbing changes, its visible release
  number can align with the service; equality is not an ordinary update prerequisite.
  ADR-0193 records the implementation boundary.

- **Package updates must prevent split-version authority states (npx update / install).** When a user
  upgrades Ghostlight via package install or npx while an existing service and client connectors are
  running, older MCP connectors can remain connected to the old service while the browser connects to
  the newly spawned adapter. Upgrades must cleanly quiesce/retire lingering predecessor processes and
  redirect running clients.

- **One checkout, two branches.** Work in the ordinary Ghostlight repository on `dev`. Do not
  create fleet folders, extra worktrees or per-agent branches (owner, 2026-09-09 and 2026-10-05;
  see AGENTS.md Branches). Put scratch output in the existing ignored folders (`.tmp/`, isolated
  target directories). Leave the existing preserved worktrees
  (`.tmp/fleet-acceptance/integration-worktree`, the Codex release-1-3-13 worktree) in place
  unless the owner asks for them to be removed.

- **Dedicated test machines.** test-01, test-02, test-03 and leo-desktop-02 are owner-granted test
  resources. Agents may install prerequisites, interact with the desktop and browser, package,
  install, upgrade, restart, reboot and recover on them for project testing without asking each
  time. Preserve the baseline first, and record which candidate is installed before and after so
  evidence never comes from stale binaries. This never authorizes public releases or
  founder-private access. The browser's own data (profiles, saved tab groups, bookmarks, settings)
  stays the owner's: ask before changing it, even in a cleanup the owner asked for (owner,
  2026-10-18). How the 2026-09 campaign ran (lanes, escalation, evidence format) is in
  [the fleet campaign](testing/fleet-acceptance-2026-09.md).
- **Coordinated machines may commit and push as the owner.** The owner authorized provisioning
  the local Ghostlight Git credentials to the four dedicated machines. Use the owner's verified
  identity and secure credential helpers; never put tokens or private keys in task messages,
  logs, command arguments, source, Git URLs, or evidence. Keep each agent's pushes on its own
  campaign branch for integration. This does not grant force-push, merge, or release authority.
- **Ghostlight manages only its own actions.** Human browsing creates no Ghostlight activity,
  audit, policy check, tab hold, automatic child-tab adoption, or regrouping. A controlled opener
  and an in-flight command do not prove that Ghostlight caused a browser event. Silently stale
  agent references when the page changes; govern the next actual agent request (ADR-0164).

- **Delight through sane defaults.** Anticipate the caller's intent with safe defaults instead of
  boilerplate configuration; bounded settlement across page tools is the model case (see
  'Settlement is a timing preference' below, ADR-0174/ADR-0195).

- **The agent decides agent-facing experience.** On 2026-10-14 the owner delegated every decision
  about what agents see to the agent, as the audience: tool result content and size, matching
  rules, refusal wording and next steps, and catalog descriptions. Use the agent's own experience
  as evidence and record each decision in an ADR. Governance, truthfulness, privacy and the trust
  claims are unchanged and stay the owner's. The test: an agent never needs an extra call to learn
  what a control accepts or what its action changed, and a mistake costs one call.
- **In an MCP, the user to delight is the model.** The owner said so on 2026-10-17, while the Linux
  batch fixed what it found. Judge every MCP-facing behavior, recovery included, by the agent's
  experience: a request that cannot have taken effect succeeds instead of failing (the edge sends a
  cut-off listing again, ADR-0197 amendment of 2026-10-17), and a failure says what happened and
  the next safe step. The person's surfaces (window, CLI, installer) keep "Delight is clarity".

- **Resilience through leniency is a product-wide rule.** Tolerate harmless differences, unknown
  optional information, and unavailable nonessential components. Use safe defaults and bounded
  automatic recovery; keep unaffected work available instead of rejecting the whole operation or
  installation. Users should not have to repair routine incompatibilities. Be strict where
  continuing would misinterpret an action, lose data, or bypass authority; do not silently guess
  those semantics. Versioned history is one application, not the scope of this directive
  (ADR-0163). Prove recovery at real boundaries, preserve evidence, and never replay uncertain work.
  Be adaptive as well: discover where things really are on every look (where an app keeps its
  settings, a Store app's private copy of them) instead of trusting a fixed table of locations or
  package names (owner, 2026-10-17: "our process must be resilient and adaptive").
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
  Ghostlight's About must keep its complete TCG-like guardian card: lantern pixel art, softly lit
  dark frame, cyan divider/diamond, circular version medallion, name/title, ability icons, and
  flavor line, with the adjacent real local facts and help links. Normal-screen refinement must
  not flatten or replace that identity.
- **Prefer the root fix.** No wrapper, alternate id, guarded installer, or parallel protocol added
  to route around the abstraction that should own the change. If a spot fix is genuinely
  unavoidable, say so in the commit message so the debt is visible. When a root fix lands, retire
  the temporary workaround it replaces in the same change; a leftover 6s fill delay taxed every
  form fill until ADR-0169.
- **Understand the architecture before fixing.** Several failures in one capability means stop
  implementing. Map ownership, lifecycle, authority, state, and delivery; find the single owning
  seam; change it there. Restore a green checkpoint before proposing the change.
- **Fewest meaningful moving parts.** A logical boundary does not earn a process, crate, service,
  event bus, actor system, workflow engine, CQRS split, or registry.
- **Keep releases lean.** A release carries only what must change together; additive work ships
  in the next minor. The 2.0 break grew by accretion (agent experience, rules, process surgery,
  desktop shell, install journey) while 1.x users stayed exposed to known hazards, and the owner
  had asked for lean (2026-10-14). Before adding work to a release in flight, ask whether it must
  freeze with it; if not, it waits.
- **Show working software, and ask design questions as questions.** Every milestone ends with
  something the owner can see or try, live, before the next one starts; summaries do not
  replace it. Reuse what exists (adapting it) before building anew, and never build throwaways.
  A choice that changes how something works (a transport, a seam, a security trade) goes to the
  owner as an explicit choice with its trade-off, never folded into a description (owner,
  2026-10-15, after a shell spike showed test content instead of the existing workbench).
- **Test locally before anything leaves the machine.** No push, tag, release, publication or
  other outward-facing step until the build has been deployed in place on the owner's machine
  and tested there live. CI then confirms what local testing already showed; it does not replace
  it (owner, 2026-10-16).
- **Delight is the purpose; dependable boundaries support it.** Ghostlight makes integrated
  tooling useful and natural for end users. Governance lets individuals and organizations choose
  reliable boundaries that work consistently whenever work passes through Ghostlight. Ordinary
  permitted work should stay easy, and limits and outcomes should be understandable when they
  matter. This is not a claim of host containment.
- **Host restrictions belong to policy.** The owner removed the built-in localhost, loopback, and
  link-local ban. All-open includes local HTTP(S) browser work; do not reintroduce a hard-coded
  address block, a local-access toggle, or an exception flow (ADR-0155).
  The one exception is the workbench's own loopback origin, which agents can never open (owner,
  2026-10-15; ADR-0199 amendment).
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
- **Delight is clarity.** Say only what the person needs to know or do. Never narrate the
  interface's own behavior ("this page updates on its own", "you can close this"); let the change
  itself be the message. Fallback actions appear only when they become needed, never up front.
  Filler such as "That is it." goes.
- **One visual language: sylin.org's.** Product surfaces (the setup page, the popup, the
  Ghostlight window) use the site's kit: the night ground, the `#5eead4` accent, system fonts,
  eyebrow labels, the 01/02/03 steps. The trading-card shape (the framed mascot card with its
  version disc) is reserved for presenting Ghostlight as a product: the site, store listings,
  About. Setup and status surfaces show the mascot alone (owner, 2026-10-14).
- **Solve the immediate need, then reveal useful depth.** Lead Ghostlight with private, controlled
  browser access and legible agent activity. Its integrations across MCP clients, models and
  harnesses, and its configurable policies should be understandable optional depth. This is
  product direction, not permission to make unverified privacy or compatibility claims.
- **Activity lists lead with tool identity.** The owner rejected replacing the white tool column
  with repeated Completed/Effects uncertain labels. Keep tool names primary, outcomes secondary,
  and exceptional recovery visible. The window shows each tool without the shared `browser_`
  prefix, in words ("navigate", "fill form"), with the exact name one hover away and in the
  detail card (owner, 2026-10-17); agents, audit and the catalog keep exact names. Evaluate the
  whole mixed list for scanability; successful technical checks or one readable card do not prove
  user comprehension (ADR-0187).
- **Absorb ordinary bursts.** The owner rejected a refusal-led local-resilience experience. Keep
  permitted work flowing within its original deadline, make sustained waiting legible, contain
  stalled connections, preserve other sessions and human controls, and recover without replay.
  Capacity bounds serve continuity; people should not have to tune service machinery (ADR-0160).
- **Manual browser recovery speaks to the model.** When browser auto-open is off, never make the
  model translate a person-facing error. Tell it to ask the user to open any eligible installed
  browser Ghostlight can name, with the extension installed, then repeat the call.
- **Start the browser the person uses; ask only without one.** Ghostlight never presents a
  browser choice and never spends a refusal saying it declined to choose (the owner's verdict on
  the ambiguity refusal, ADR-0149). With no browser connected, it starts the only installed
  browser, or, among several, the one that last connected to it ("Start the last one", the
  owner's ruling of 2026-10-17, ADR-0149 amendment). With several and no such memory, name every
  connectable browser, repair silently what Ghostlight already owns, and leave startup to the
  person. The `manual` posture never launches.
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
  Client-supplied text is generic input. Product and privacy descriptions must follow the actual
  operation and data use, not infer authentication or financial activity from possible text values.
- **Human runtime control is authoritative.** ADR-0126 decides that pause refuses the next effect
  instead of suspending a caller, and stop is terminal with a pinned directive. Both directives
  live in `crates/orchestrator/src/language/outcome.rs`. Preserve them when testing
  effect-boundary timing and scope.
- **Ghostlight does not regulate agent behavior.** Repeated policy denials and credential-input
  guidance never pause a session or create a review/resume ritual. Each request follows configured
  policy and explicit human controls. Dispatch checks happen after writer wait; a refused
  follow-up observation cannot erase an acknowledged action (ADR-0185).
- **Browser feedback is passive.** Messages and animations may appear on the page; buttons,
  focusable controls, and input-intercepting overlays may not. Interactive runtime control belongs
  in the Ghostlight window, including when page styling fails (ADR-0185).
- **Explicit user instructions permit credential input.** Fill and typing accept the per-request
  `user_authorized_credentials` acknowledgement. Prior explicit user authorization is sufficient;
  do not ask again. Without it, return guidance for that request, not a session hold. The caller's
  acknowledgement is not independently verified proof and cannot override configured policy or
  human Pause/Stop. Credential-like file-control names do not block explicitly requested uploads
  or recording saves (ADR-0185).
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
  only in volatile human details; model/audit coverage stays content-free. ADR-0194 replaces screenshot
  masking with binary `browser.screenshots.enabled`: allowed captures include visible embedded
  content without document inventory; a refusal sends no capture command. Separate text, target,
  and action restrictions remain. Restricted recordings stop at document-set changes, and
  replay export checks every captured source. Scripts have no inferred containment mechanism.
- **Presence brings up the shell; the service never needs a display.** Whatever started the
  service, it starts `ghostlight-desktop --tray` 2 s after it is ready when no shell is attached,
  and again 2 s after a shell vanishes without a goodbye, only when a display is reachable (Linux
  `DISPLAY` or `WAYLAND_DISPLAY`; Windows the visible window station). Three launches in two
  minutes latch `did_not_stay` until a person acts. With no display, web view runtime or tray the
  service still serves and `status` says why there is no window (`window` in `status --json`). A
  session without a tray keeps the Applications entry and `ghostlight open`
  (ADR-0199 Decision 11, which superseded ADR-0127's desktop-authority startup).
- **A held lease is not readiness.** Publish discovery only once the interaction route exists,
  reap every child that failed or lost the lease race, and bound concurrent starters' retries, or
  they take turns spawning failures (32 authorities in four seconds, ADR-0166; current mechanics
  in ADR-0197 Decision 10). Readiness never waits for a window: the service links no UI toolkit,
  binds its endpoint at start and serves with no display (ADR-0199 Decisions 1 and 11).
- **One process does not prove one workbench.** A startup/Open race created two responsive native
  windows inside one authority. The service now holds one desktop place and a second shell is sent
  `already_open`, but native lifecycle acceptance still counts actual windows, including hidden
  ones, through startup and close/reopen bursts. A mock activation port or process count cannot
  prove this promise (ADR-0119's September 7 amendment).
- **Keep browser work visible and user-placed.** Reuse the same-name Ghostlight group wherever the
  user put it; create a dedicated window only when none exists; never reclaim an unrelated one.
- **Keep visual evidence.** Model-driven close needs both orchestrator authority and the extension's
  preserve-tabs setting. Either refusal keeps the tab visible, and manual closure stays the user's.
- **Ghostlight is always user-space.** It installs and runs per user, under the user's own
  home. It never depends on system packages, privileged helpers, or tunnels through a browser
  sandbox; a sandboxed browser such as a Flatpak Chromium is named unsupported instead (owner,
  2026-10-14; ADR-0197 Decision 22).
- **Never phone home.** No telemetry, activation, update ping, audit upload, or hidden vendor
  dependency. The owner defines phone home as communication outside the user's or the
  administrator's intent, not HTTP access as such (2026-10-14). An administrator-configured signed
  policy fetch from the organization's own file or HTTPS source is intended communication; it
  never contacts Ghostlight or another vendor endpoint.
- **Outward changes wait for the owner.** Local edits, tests, and commits are normal. Pushes,
  merges, tags, releases, store actions, and anything public are not.
- **Full integration means the installation in place.** Green gates on an isolated target do not
  prove the user's installation: an older authority can still serve real connectors (28295436),
  and a rebuilt connector can stay stranded in bin (c034af69). Identify the exact live image
  paths, deploy only the changed component through the dev loop, confirm those paths now run the
  new build, restart it, prove its existing shores renegotiate, and name any physical lane that
  did not run. Accept on the installed graph with a normally launched browser. Never start a
  second authority, installation, profile, runtime, agent-launched browser, startup helper or OS
  diagnostic shortcut to get a pass. Diagnostics live in the product's existing toggles and log
  locations (owner, 2026-09-08 Windows boot investigation; ADR-0145 amendment). Isolated checks
  supplement this evidence but cannot prove installation, startup or recovery. Ask before running
  a separate desktop test installation. A separate build directory only avoids locked files.
  A sandboxed run (a bubblewrap home, a test home with its own browser profile, Chrome for
  Testing) is a false integration test: acceptance always uses the real installation and the
  system browser the person uses (owner, 2026-10-17, on the Linux batch).
- **One live Ghostlight on the owner's machine, always.** Never run a second service, a demo or
  spike build of any Ghostlight executable, or a journey that starts its own service in a test
  home on the owner's machine: two of anything invites "which service is this adapter talking
  to?" A deploy replaces the live installation in place; if it breaks, the owner restores. Do
  not hedge with parallel deployments. Journeys that start their own service run on the test
  machines or in CI only (owner, 2026-10-15). In the product too, the service is zero or one:
  status reads like "Ghostlight service is up; 2 MCP clients and 2 browsers connected" and never
  counts services. The small processes agents and browsers launch are its connections and are
  named as such; the window is its window. Anything else running from Ghostlight is a fault that
  doctor names with its fix.
- **Browser support means full ordinary-session capability.** Do not ship a partial adapter under a
  browser-support label. Every advertised Ghostlight job must work in the user's visible,
  authenticated, already-running browser without a headless mode, disposable profile, managed
  launch, or restart ritual. A smaller honest capability list is still a poor regular-user
  experience when normal work immediately requires another browser (ADR-0183).
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
- **Persist before handoff.** Before writing a restart, compaction or handoff prompt, write any
  durable fact the next agent needs into the tree and commit it. The prompt points at the tree; it
  does not carry state. Where each kind of fact goes is in AGENTS.md "Keeping this system honest".
- **Live swaps only through `scripts/dev-loop.ps1`.** Never hand-copy a binary into an install or
  kill and restart the authority by hand: the installer (`ghostlight deployment install`) owns
  locking, stopping and copying, and bypassing it once left two live authorities (151aad95,
  2026-08-24). If the service seems stuck, deploy again through the script. An orphan connector
  from a superseded install demand-starts its exact sibling by path (2026-08-27), so stop that
  connector by exact path first. Mechanics: docs/DEV-LOOP.md.
- **A generic configuration root is not product detection evidence.** A config file directly under
  home, the platform config root, or roaming must exist itself, or the product needs independent
  executable evidence. Treating the generic parent as detection makes absent clients actionable
  and causes aggregate setup to overreach.
- **Foreign-entry safety distinguishes automatic discovery from reviewed repair.** Setup, update,
  remove, and aggregate setup must not overwrite an uncertain command. A person who sees the
  bounded evidence and confirms Fix may authorize replacement of that one parseable entry under
  Ghostlight's own key. Re-check at the writer, preserve siblings, and back up first; malformed
  whole documents remain manual (ADR-0154).

- **Settlement is a timing preference, not an action prerequisite.** The owner directs a
  bounded default across similar page tools, then current valid execution even on busy pages.
  `visual_settle:false` skips waiting. Unrelated active-frame churn cannot invalidate a targeted
  operation; explicit waits and configured document authority retain their actual promises
  (ADR-0195).

## Durable lessons

- **A gate on one system never compiles the other system's gated code.** On 2026-10-17 the two
  coordinated sessions broke each other's platform within an hour: a Linux-written test called
  `std::os::unix::fs::symlink` without `#[cfg(unix)]` (Windows CI red), and a Windows-side field
  added to a shared struct missed a `#[cfg(unix)]` test that builds it (Linux CI red). Gate every
  Unix-only or Windows-only call; when a shared type changes, grep the other system's gated code
  for its uses; and treat CI's other-system clippy as the first check of that code, pushing to
  `dev` early rather than calling a one-system gate complete. Cross-compiling the service for the
  other system needs that system's C toolchain (`ring`), which neither host has.
- **Cite unpushed work by its subject, not its hash.** A rebase onto the other session's pushes
  rewrites every local hash, and ledger lines that named them pointed at commits no branch holds.
- **Agents share the owner's machine; their processes must end with their step.** On 2026-10-15 a coding agent ran `find / -maxdepth 6 -name <crate>` in Git Bash to locate a crate's source.
  Git Bash's root reaches every drive, its shell timed out, and the orphaned `find` burned 20
  CPU-minutes until another session noticed. Never search a drive or the filesystem root;
  dependency sources are in the Cargo home's `registry/src`, or ask `cargo metadata`. Give long
  commands a timeout and leave nothing running when a step ends.

- **Execution world is part of browser correctness.** A service-owned MAIN-world runtime cannot
  use extension-context messaging. Another installed extension can expose a webpage messaging API
  with different calling requirements, so an isolated test profile can conceal a startup failure.
  Keep optional diagnostics outside automation initialization, project local rows at each physical
  boundary, and retain exact document custody through worker suspension (ADR-0188).

- **Warm MCP success does not prove cold desktop demand-start.** A client can
  filter DISPLAY, Wayland, XDG runtime and session-bus variables from its stdio
  child. Test from no authority through the real client, inspect only the named
  environment boundary, and forward variable names through the client's supported
  configuration instead of capturing machine-specific values. See the
  [Bluefin evidence](testing/bluefin-fleet-2026-09-09.md) and the
  [Alpine installed repair](testing/alpine-musl-fleet-2026-09-09.md).

- **A deployment lock, an installer exit, or a refreshed tool catalog does not prove replacement**
  (dev loop and the Windows installer alike). Clients respawn connectors during a swap, and a
  running old connector can reconnect while keeping old bytes. Compare every installed sibling to
  its intended package. Under the deployment lock, stop only exact destination image paths, bound to
  process identity rather than file registration. Verify that a separate installation and an
  unrelated file reader survive. Evidence: testing/windows-fleet-packaging-2026-09-09.md.

- **Release identity precedes asset transport.** GitHub redirects release assets to a signed CDN
  URL that does not identify the release tag. Resolve the latest release page first, validate the
  stable tag, then pin checksums and every sibling download to that one release. The actual Linux
  one-line installation and `tests/installer-shell.mjs` caught the old final-asset-URL assumption.
- **Fixed archive timestamps do not guarantee reproducible bytes.** A writer can embed per-process
  data (the retired .NET tar path put the packager PID in Pax headers, 4a7f4dc4). Two packaging
  calls in one process miss this, so prove determinism across separate packager processes, as
  tests/winget-archive.ps1 does. The compression runtime is part of the reproduction environment.

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
- **A dirty indicator is not proof that a form retained the draft.** Verify retained values after a
  later framework render under ordinary tab lifecycle: restore normal focus behavior and check tab
  state against document visibility. Ghostlight itself emulates focus on controlled tabs (ADR-0168),
  so a pass under Ghostlight control alone is not retention evidence. Absence of a DOM reset event
  does not exclude a library reset. Diagnose with an authored real-framework fixture and
  current-state reads; a wrapper on one React props object can log only the first change. Evidence
  and the September 13 correction: [Research
  29](research/29-form-retention-focus-and-frameworks-2026-09.md).
- **Controlled-tab focus has an explicit lifetime.** The owner approved focus emulation while
  controlled debugger tabs have active runtime control (ADR-0168). Keep it between tool calls;
  restore normal behavior on pause, stop, disconnect, or release. A model's final response is
  not a workspace release. Do not claim that unsaved drafts will survive later site refreshes.
- **Extension session storage survives worker suspension, not extension reload.** Chrome clears
  `storage.session` on reload/update/disable and browser restart. The orchestrator can still own
  tabs afterward. Restore the adapter's opaque cache from explicit service work, without using
  group labels, opener relationships, or passive browser events as ownership (ADR-0168).
- **Guardrail tests must prove permitted work too.** A caller supplying exactly the advertised
  capability set must complete the operation, including its landing and export checks. Assert actual
  retained values, effect counts, and captured pixels independently of receipts. For batches,
  include a known-invalid later item and assert the earlier ones were not changed (ADR-0158 records
  the September 7 failure and the preparation rule). The full hardening suite and its feature map
  live in `docs/tasks/security-hardening/regression-suite.md`. A tool name alone is not a complete
  input matrix: handles, selectors, and postconditions can require different capabilities. Advertise
  and admit the complete request before effects, while keeping the action's landing requirement
  separate from earlier lookup and later observation.

- **Mocked debugger replies do not prove JavaScript completion semantics.** H3's real Chromium
  lane found that `awaitPromise` plus `replMode` returned an async wrapper's promise as `{}`;
  explicitly awaiting the selected wrapper produced its intended value. Preserve real engine
  evidence when changing parsing, execution, or effect classification (ADR-0133's H3 follow-up).

- **A launcher channel re-verifies its download on every launch, so a hand-staged binary at its
  versioned cache path never runs.** `npx -y ghostlight` checksum-validates
  `~/.ghostlight/bin/<version>/` against the published manifest before each spawn and re-downloads
  on mismatch (2026-09-02, during the ADR-0150 verification): deploying to the npx stage means
  publishing a release, and verifying an unreleased connector belongs in a directory the launcher
  does not checksum. There is one fixed home per OS user (ADR-0197 Decision 10); journeys isolate
  with the test-only GHOSTLIGHT_TEST_HOME, and an unreleased build is installed into `bin` as the
  development channel, not run from the npx cache.

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
- **Put the rule in the shape; correctness kept by memory rots.** A rule a reviewer must remember,
  or a hand-maintained list each new case must join, eventually misses one: the reduced-motion
  selector list silently skipped new effects, and per-tool fact reporting would have left new tools
  silent (7a33c4f6). Make it a type variant with nowhere to put the forbidden thing ("bytes never
  cross"; a field everyone agrees not to fill does not survive). Otherwise derive the list from a
  registry, or observe at the one seam every case already crosses. Where a budget differs by path,
  state it per path: one number for every path is a contradiction waiting to be found, and raising
  it only moves the contradiction.
- **Trade fidelity, never coverage.** A bounded recorder that stops at its limit produces a replay
  that silently omits everything after. Whoever drops a frame folds its time into the frame before
  it, or the artifact misreports how long the work took.
- **Say what a person would say.** A replay is "30 seconds of page changes", not "17 of 65 frames
  as 3804453 bytes". Mechanism belongs in the facts; the sentence is for a reader.
- **Model-facing names describe authority, not implementation APIs.** CDP calls the primitive
  `Runtime.evaluate`, but page JavaScript can mutate and navigate, so the tool is
  `browser_execute`. Keep physical vocabulary behind the language boundary.
- **A guard that parses nothing passes everything.** Check every source-scraping test against a
  negative control. A guard can also go stale in the same commit that makes it stale: once a
  rendered string carries a fact, asserting that the fact appears *separately* stops protecting
  anything and starts pinning duplication in place.
- **Replace only what changed.** A binary that merely recompiled is not a changed fringe; swapping
  it costs a killed native host, an extension reload and a browser reconnect for nothing. The
  installer replaces connectors only when the relay revision changes (ADR-0197 Decision 14); a
  connector source change under one revision reaches an installed home only by a revision bump or
  the forced replacement in DEV-LOOP.md.
- **A delegated batch spec must say what a change makes redundant**, not only what it adds, or the
  executor is correct and the surface is repetitive.
- **No bureaucratic rules: every check and fence must serve the person.** Ask first whether a
  check matters to a user or only to the process; the fix can be removing the check. In a slice
  spec, give a golden or a fence its purpose ("no byte the installer writes changes"), never a
  blanket ban ("zero diff"), so the executor lands a justified diff for review instead of stopping.
  A golden pins only its own subject; a field outside it is noise that turns every unrelated
  change into churn (owner, 2026-10-16 and 2026-10-17, after a zero-diff rule blocked the icon
  endpoint over the window's symbol, which the installer neither writes nor says).
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
- **Completed means succeeded.** Flow keeps unsuccessful and unreached work distinct
  (browser_sequence was folded into browser_flow). Continue
  permits later work, never a false overall success. Known partial progress stays known; recovery
  respects confirmed changes and never proposes replaying the entire composition. Human controls and
  invocation limits take precedence (ADR-0133 H2b amendment).
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
  authority before discovery or presentation state exists. At the browser shore, only an end-to-end
  acknowledgement proves that the extension consumes the attached relay stream; operation silence
  cannot prove the opposite because healthy browser work may be quiet. (ADR-0104 D2, ADR-0113,
  amended by ADR-0196 D12; see also the deadline lesson below).
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
- **Stop processes only by exact executable path, never by image name.** This covers test cleanup as
  well as installers: the user's installed stack, browser, and MCP clients may be running. An
  image-name check matched the authority, missed the connectors, and would have stopped a separate
  installation (docs/testing/windows-fleet-packaging-2026-09-09.md). Give each candidate fresh
  state, because leftover runtime files or binaries can make a check pass for the wrong build.
- **An internal rewrite does not reset the user experience.** The 1.0 rewrite dropped the 0.8 entry
  points and 7022c6bd had to restore them. A rewrite keeps observable commands, launchers and
  packages unless the owner changes them (AGENTS.md, Authority and historical continuity). The npm
  launcher (`npx -y ghostlight install`, and a bare npm launch as MCP stdio) is a mandatory entry
  point.
- **A Windows GUI-subsystem executable is not a normal PowerShell pipeline child.** Since the shell
  milestone only `ghostlight-desktop` is GUI subsystem; the edge and the service are console
  programs whose manifest asks for no console on a start with none to join. A script that must
  drive `ghostlight-desktop` directly uses an explicit waited process with redirected stdout and
  stderr, because `$LASTEXITCODE` and `&` are not a reliable boundary for that executable shape.
- **Probe the intended native window, not a process-level main-window guess.** Tray helpers,
  event-loop helpers, and console hosts can make `MainWindowHandle` look healthy after the actual
  workbench window has died. Windows desktop acceptance identifies the exact `Ghostlight` /
  `Ghostlight Window` (window class) HWND in the shell's process and checks its visible, minimized, Close, activation, and recreation states.
- **Diff the public surfaces by name before calling a rewrite complete.** A file-level 'missing'
  list reads as intentional rewrite when it is really identity and release machinery: the 1.0
  clean-room dropped the README hero GIF, badge row and onboarding spine, the Homebrew template, the
  website publish path, the icon generator and the store-justification length guard (restored
  2026-08-25, 14397e9c). Before a break set lands on main, reconcile root documents, packaging/,
  scripts/, store justifications and legal guards against main by name, and confirm each removal
  against its ADR.
- **Two frame-id vocabularies do not translate; the parent DOM is the truth.** CDP frame strings and
  webNavigation numbers do not map onto each other, and the tab-level debugger cannot see
  out-of-process frames, so a bridge built on CDP identity silently covers only same-process frames.
  One mechanism for same- and cross-origin frames (ADR-0138 owns it) beats a fast path plus a
  fallback that fails exactly where the fallback was for.
- **Capability coverage does not transfer between sibling observation paths.** Shadow-aware form
  inspection did not make page reading shadow-aware, and an explicit visible read did not make the
  shortest `browser_read {}` take that path. After the text fix, waits, find, document trees, point
  receipts, target capture and drops were still on narrower DOM or frame paths. Chromium `innerText`
  stops at shadow boundaries, so a composed read needs explicit open-root and slot traversal. Trace
  the default call through decoding, dispatch, negotiation and the page-local collector. Put
  traversal, visibility, frame aggregation and point routing at shared adapter seams. Then revision
  every affected capability family so an older adapter refuses instead of returning plausible
  partial truth (ADR-0151, ADR-0152, ADR-0153).
- **An anchored effect without a live box renders nowhere.** A zero-size or hidden target resolves
  to the frame origin, and inside an embedded frame the frame boundary clips the effect in half.
  Suppression is the honest rendering of "nothing to point at."
- **A panic after the effect is the worst failure shape; prove every success arm's expect.** Focused
  typing discarded the describe step's observation and then `.expect`ed a fallback subject that did
  not exist. The typing landed, the operation task died, and the workbench showed "Typing" forever
  -- no error, no deadline, no recovery, across reconnects. When a success arm asserts a fallback,
  the fallback must be produced on that same path (ADR-0138's describe now feeds the receipt), and
  a test must walk the full success path, not just the refusals around it.
- **Per-host reproducibility is not determinism for a published artifact.** The extension ZIP was
  "deterministic" (two runs byte-identical) on each host and still differed across hosts:
  `ConvertTo-Json` writes the platform newline into rewritten JSON, and .NET's `ZipArchive` stamps
  the central directory's host-system marker and Unix mode bits from the running OS. Identical
  sources, two archives, 99 differing bytes of pure metadata -- and only one of them was the
  reviewed Chrome Web Store artifact. The runtime matters too: on 2026-09-08, PowerShell 7.5.2 and
  7.6.5 produced identical extracted adapter files but different compressed ZIP bytes. Only 7.6.5
  reproduced the submitted 1.1.2 hash. A packager for a published artifact pins serialization and
  container fields to the reviewed artifact's exact shape, proves determinism across operating
  systems, and names its packaging runtime (pwsh version) in the custody note. Source and
  test-fixture hashes are not published artifacts: compute them from committed (LF / git blob)
  bytes, never from working-copy bytes.
- **A READY card covers only the roster.** Registration state is per-client, read from that
  client's own configuration; a client with no registry row is invisible no matter how broken
  its hand-written entry is, and one client's green card never contradicts another's failure
  (2026-08-27: Zed READY while ZCode failed on an orchestrator-binary command). Adding client
  support means a registry row with the client's real config dialect, pinned against what the
  client itself writes -- never the assumption that a known sibling binary doubles as a stdio
  server.
- **The npm maintainer-of-record transfer to sylin-org is owed.** Org team package access (the
  developers team's read-write grant) does not change the public npm maintainers list, so it does
  not count as the transfer. The account is passkey-only, the website has no org-as-maintainer flow,
  and bypass-2FA tokens cannot run `npm owner add`, so finishing needs a recovery code or npm
  support. Until it lands, leave `glama.json` maintainers as they are and keep the maintainer
  assertion out of `check-public-surfaces.ps1 -Online` (docs/STATUS.md, 2026-09-01).
- **Packaged Chrome extensions clamp alarms to 60 seconds.** `chrome.alarms` enforces a minimum 1-minute
  delay in packaged extensions regardless of smaller fractional values like 0.05m (3s). Active background
  workers need in-memory timers (`setTimeout`) for fast sub-minute retry loops, with alarms serving as the
  cold wakeup fallback.
- **Loopback probes unblock blocking TCP accept loops cleanly without sleep-spin polling.** Rust standard
  `std::net::TcpListener` lacks a timeout on `accept()`, tempting short sleep loops with nonblocking sockets.
  Connecting a brief loopback probe (`TcpStream::connect`) during shutdown wakes the blocking listener thread
  immediately with zero idle CPU wakeups during service lifetime.
- **The service, not the adapter, chooses which tab to reuse (ADR-0196 Decision 9).** The adapter's
  own map clears on extension reload and cannot tell a released tab from the person's, which is how
  person-opened tabs were taken over. Reuse adopts only a tab a released session of the same client
  label left open. A released tab is claimable only after its release close comes back refused by
  preserve-tabs, because the extension does not check whose tab a close names (released pool and
  PooledTab docs in crates/orchestrator/src/workspace/mod.rs).
- **One call's deadline is not evidence about the connection.** A probe acknowledgement shares one
  ordered stream with every receipt and can sit behind a large screenshot, so a short deadline
  turned healthy browsers into false disconnects. Evidence that earns a call a wait must lapse and
  be re-read on every poll; evidence that never lapses spent the deadline browser startup needed.
  Policy: ADR-0196 Decision 12. Mechanics (5 s probe verdict, only heartbeat timeout or
  end-of-stream fails pending calls, busy writer skipped for best-effort frames):
  docs/1.0/ARCHITECTURE.md waiting and liveness. Fix: commit 2b59f56c.
- **Windows session shutdown messages require a top-level window, not a message-only window.**
  Windows broadcasts `WM_QUERYENDSESSION` and `WM_ENDSESSION` only to top-level windows (`parent == NULL`),
  not to `HWND_MESSAGE` windows. Applications that run as system-tray background authorities (ADR-0119)
  and connectors that load `user32.dll` must maintain a hidden top-level window (`WS_POPUP`,
  `WS_EX_TOOLWINDOW`) to receive session termination broadcasts. A fallback must be armed before
  calling into a native event loop that shutdown may already have destroyed; a console handler alone
  is insufficient and must not suppress Windows' default process exit (ADR-0180 amendment). The listener is `ghostlight_relay::os::session`, registered by every process (ADR-0199 Decisions 13 and 14).

- **A reply is not yet validated effect evidence.** A reply proves an attempt, not its effect:
  confirm an effect only when the receipt matches the request identity and the admitted document
  scope. A refusal reply is not an effect, and a dispatched effect whose reply is lost or unreadable
  stays uncertain (44ec184f, 989be757). Journal, keyboard-prefix and snapshot ordering rules are in
  ADR-0190; protocol-4 effect classification is in ADR-0197 D5/D7.

- **A timed-out wait is not resource cleanup.** Cancel must stop the service issuing steps and stop
  page loops. A late finally must release only what it acquired, never a newer owner's resources.
  Page effects may outlive release, so cleanup never revises an unknown result or authorizes replay
  (ADR-0191 history; current requirement in ADR-0196 Appendix A). Prove recovery with the first next
  read, taken before any polling, retry or reset, against a long budget and a still-running
  animation; zero active requests does not prove custody release.

- **A Chrome tab can contain several outermost pages.** Build document authority and routing
  from the active tree, excluding prerender, cached and pending-deletion pages before root checks.
  Keep exact active identities and fail closed on ambiguous lifecycle or malformed active graphs.
  Generic verification refusals need per-operation evidence before assigning a cause (ADR-0189).
- **On Linux, tao keeps neither time nor a window's end for you.** `ControlFlow::WaitUntil` is a
  plain wait in its GTK loop, so a deadline fires only when another event comes; and it reports a
  window destroyed only once a later loop turn has wired the window, so a window dropped the
  moment it was built is never reported. A shell with only its tray icon waited 18 s to
  reconnect, and a window replaced at birth left the shell windowless for good (linux-1, L05).
  The shell keeps its own clock and records its own window's end; judge the shell by the windows
  the compositor counts, not by what the process believes.

## Where to look

The documentation map is [docs/README.md](README.md); onboarding order is AGENTS.md 'Read this first'; task batches are in [tasks/README.md](tasks/README.md).
