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
- **One normal desktop startup.** Installed Ghostlight always creates its tray and starts the
  workbench minimized. Connectors launch that same executable with no mode flag. Only explicit
  `--headless` omits desktop presentation.
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
- **Never phone home.** No telemetry, activation, update ping, remote policy fetch, or audit upload.
- **Outward changes wait for the owner.** Local edits, tests, and commits are normal. Pushes,
  merges, tags, releases, store actions, and anything public are not.
- **Persist before handoff.** Update STATUS, the relevant ADR or task evidence, and this file when a
  durable fact changes, and commit before writing a restart prompt.

## Durable lessons

Every one of these cost something to learn.

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
- **Reconnection is not availability.** Put one idempotent recovery action at the failed-connection
  seam, then let a service-held lifetime lease decide authority before discovery or presentation
  state exists.
- **A cached MCP catalog is not a live transport.** Reconnect through the owning client, then look
  at the visible browser before retrying an effectful call.
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
- **The 0.8 distribution and trust records are history, not a working 1.0 pipeline.** Rebuild
  package and release automation from current boundaries before claiming a 1.0 artifact exists.

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
