# Project memory

Durable, model-agnostic memory for people and agents working on Ghostlight. Read this with
[`AGENTS.md`](../AGENTS.md). Current mutable evidence belongs in [`STATUS.md`](STATUS.md), decisions
belong in [`adr/`](adr/README.md), and machine-local facts belong under ignored `local/` material.
Those sources win when they disagree with this file.

## Standing owner directives

- **Preserve project history.** Root documentation, the full docs tree, ADRs, licenses, research,
  trust evidence, legal copy, business records, task ledgers, and product identity survive internal
  rewrites. Reconcile current guides; do not erase historical evidence.
- **Preserve product identity, redesign internals deliberately.** Ghostlight's name, original icon,
  visual language, motion character, browser controls, and user expectations are identity.
  Model-facing tools and descriptions are mechanisms owned by the orchestrator.
- **Prefer root architecture to spot fixes.** Use meaningful semantic implementations. Do not add
  a wrapper, alternate id, guarded installer, recovery path, or protocol only to bypass the owning
  abstraction.
- **Use the fewest meaningful moving parts.** Logical boundaries do not automatically earn a
  process, crate, service, event bus, actor system, workflow engine, CQRS split, registry, or
  container.
- **Keep the fringes stable.** Product and journey changes belong in the orchestrator. MCP and
  browser connectors negotiate with their consumers and relay typed facts. The extension owns
  Chromium, page-local DOM, observation, and visual rendering, but no product, workspace,
  authority, recovery, or model-language decisions.
- **Keep browser work visible and user-placed.** Reuse the same-name Ghostlight group wherever the
  user placed it. Create a dedicated normal window only when no group exists. Never disrupt or
  reclaim an unrelated active window.
- **Keep visual evidence.** Model-driven close requires both orchestrator authority and the
  extension's local preserve-tabs setting. Either denial keeps the tab visible; manual browser
  closure remains the user's action.
- **No phone home.** No telemetry, activation, update ping, remote policy retrieval, audit upload,
  or vendor runtime dependency.
- **Outward changes require approval.** Local edits, tests, and commits are normal. Pushes, merges,
  tags, releases, store actions, external posts, and service mutations wait for the owner.
- **Persist before handoff.** Update STATUS, relevant ADR/task evidence, and this memory when a
  durable fact changes; commit before producing a restart prompt.

## Current 1.0 architecture facts

- The canonical product contracts are [`1.0/INTENT.md`](1.0/INTENT.md),
  [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md), [`1.0/ARCHITECTURE.md`](1.0/ARCHITECTURE.md), and
  [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md).
- One Rust workspace builds the shared typed bridge, `ghostlight` orchestrator,
  `ghostlight-mcp-connector`, and `ghostlight-browser-connector`.
- The orchestrator owns the complete 24-tool catalog, workspaces, immutable authority snapshots,
  runtime controls, one executor, one completion path, browser port, content-free presentation,
  and payload-free audit.
- The MCP connector owns local stdio protocol lifecycle, catalog retrieval, correlation,
  cancellation forwarding, and generic invocation rendering only.
- The browser connector owns native-message framing, typed relay correlation, and durable
  connection lifecycle only.
- The extension is policy-free and free of model-facing results. Its physical-capability hello and
  durable adapter identity support negotiation without moving product evolution to the fringe.
- In-flight effects interrupted after dispatch become unknown and are never replayed. Both relay
  processes can remain alive while the orchestrator restarts and renegotiate for later work.
- Browser state is plural by design: MCP sessions, workspaces, operations, browser instances, and
  future browser families must not be modeled as global singletons.
- The Tauri 2 workbench is an integrated presentation adapter in the modular monolith, behind a
  typed `WorkbenchFacade`. It owns no durable product state or browser primitive. A recoverable
  shell failure leaves the orchestrator headless.
- Workbench harness management is explicit and orchestrator-owned. It supports nine named
  harnesses, preserves JSONC/TOML comments and unrelated configuration, backs up changes, and
  refuses malformed, unreadable, or foreign-owned entries.
- The original extension icon bytes are reused by the desktop, tray, bundle, and workbench. Do not
  redraw, recolor, rename, or substitute the product identity.

## Durable implementation lessons

- Build and process tests in an isolated target directory when a live installed Ghostlight stack
  may hold Windows executables open. Never kill processes by image name; verify exact executable
  paths and stop only test-owned processes.
- A cached MCP catalog is not a transport-liveness signal. Reconnect through the owning MCP client,
  then inspect visible browser state before retrying an effectful call.
- Chrome native messaging has directional size limits. Generic corruption ceilings and browser
  chunking are different contracts.
- A native-port or service-worker restart is not a browser restart. Preserve uncertain resource
  state until exact generation or terminal evidence resolves it.
- A completed document load is not proof that extension presentation is mounted. Presentation uses
  a ready handshake, exact document acknowledgement, and packaged reinjection.
- Persistent controlled scope and transient activity are different visual promises. The border
  identifies controlled scope; cursor, scans, highlights, ripples, frames, ribbons, and captions
  explain current work.
- Browser screenshots intentionally suppress the extension's visual layer. Visual QA must observe
  externally; a clean captured page is not evidence that feedback failed.
- Debugging and audit remain metadata-only. Never persist MCP bodies, page content, results,
  screenshots, form values, scripts, paths, or file bytes.
- The public 0.8 distribution and trust records are historical truth, not a working 1.0 release
  pipeline. Rebuild package and release automation from the current boundaries before claiming a
  1.0 artifact exists.

## Pointer index

| Need | Source |
| --- | --- |
| Repository authority and implementation rules | [`AGENTS.md`](../AGENTS.md) |
| Mutable implementation status | [`STATUS.md`](STATUS.md) |
| Current 1.0 contracts | [`1.0/`](1.0/) |
| Architecture history | [`adr/`](adr/README.md) |
| Integrated desktop decision | [`adr/0102-integrated-desktop-workbench.md`](adr/0102-integrated-desktop-workbench.md) |
| Build, restart, and live validation | [`DEV-LOOP.md`](DEV-LOOP.md) |
| Planned release procedure | [`RELEASE.md`](RELEASE.md) |
| Historical deep design | [`SPEC.md`](SPEC.md) |
| Source licensing boundary | [`../LICENSING.md`](../LICENSING.md) |
