# Project memory (cross-agent)

Durable, model-agnostic memory for any agent or human working here (Claude Code, Codex, Cursor,
...). Read it on session start, alongside [AGENTS.md](../AGENTS.md).

**Scope discipline.** This file holds only three things: **standing preferences**, **durable
learnings**, and a **pointer index**. It is NOT current state (that is
[docs/STATUS.md](STATUS.md)), NOT decisions (those are [ADRs](adr/README.md)), and NOT
machine-local or sensitive facts (those are `local/`). When any of those disagree with this file,
**they win** -- this file points at them, it does not duplicate them. Keep entries terse; prune
what goes stale. A model's own private memory (e.g. Claude Code's auto-memory) is a secondary
cache of this file and must never become a competing source of truth.

## Standing preferences (owner directives)

Collaboration and process -- this file is their canonical home:

- **Memory is project-level.** The owner runs several agents/LLMs against this repo and delegates
  tasks between them. Durable memory lives here + STATUS + ADRs + `local/`, never in one model's
  private store. When you learn something durable, write it to the right scope here, not to a
  model-private memory.
- **Outward-facing content is draft-then-confirm.** Draft anything that leaves the repo (npm,
  store listings, social posts, website copy, comments on external repos) and WAIT for the owner
  before posting. Local commits are normal autonomous work; pushes, merges, releases, and external
  changes are the owner's call.
- **Prefer the root fix over the spot fix.** If a spot fix is genuinely unavoidable, say so
  explicitly in the commit message so the debt stays visible.
- **Documentation and product history survive internal rewrites.** Root documentation, the full
  `docs/` tree, ADRs, licenses, public identity, research, trust material, and task records are
  inherited product knowledge. Reconcile them for a new implementation; never replace them with a
  narrow clean-room substitute or quarantine them with the old code.
- **Use the fewest meaningful moving parts.** A logical boundary does not automatically earn a
  process, service, crate, trait framework, or new identity. Add one only for a real lifecycle,
  trust, version, or correctness need. Preserve product capability and safety, not incidental
  compatibility. A break-and-rebuild is welcome when it produces a smaller, clearer, better
  system (ADR-0096).
- **Preserve product identity; redesign internal tools deliberately.** The Ghostlight name, icons,
  visual language, animation, public character, legal identity, and user expectations survive an
  internal rewrite. Model-facing tools and descriptions are mechanisms, not identity. The
  orchestrator owns them and may make them clearer, smaller, and more semantic.
- **The browser product stays in the local user's context.** Ghostlight is for visible work in the
  user's existing authenticated browser. Chromium is the current adapter; the domain must support
  plural browser instances and future browser families without singleton assumptions. Headless,
  isolated-profile, cloud, and remote browser execution remain product exclusions.
- **Browser placement belongs to the user.** Reuse the same-name Ghostlight tab group across open
  browser windows and place new operation tabs there. Create a separate browser window only when no
  suitable Ghostlight group exists and avoid disrupting the user's active non-Ghostlight window. A
  tab group is visible organization, not a user-facing security boundary; never move tabs or groups
  back after the user places them elsewhere.
- **A stale workspace recovers only through explicit tab creation.** Known Ghostlight tab ids stay
  authoritative. `tabs_create_mcp` may replace a conclusively dead window pin after safely creating
  a fresh blank tab; other calls never switch workspaces automatically (ADR-0090).
- **A Ghostlight test is a live test.** Test Ghostlight by calling its MCP tools from the active
  client against the real local engine, extension, and user's visible authenticated browser. For
  cross-platform proof, run that same live path on each target OS, including Windows and Linux.
  Do not substitute Playwright, a disposable browser profile, or an emulated harness unless the
  explicit subject is that harness or CI boundary.
- **Public content is warm, practitioner-first, and delight-led.** Developers should see the
  product, fit, installation, first useful task, and recovery before organization procurement
  depth. Lead with useful work, visibility, and control; do not make personal use sound incomplete
  or governance sound punitive. A front door should invite the reader into the experience, not
  become a qualification checklist, apology, excuse catalog, or directory of competing products.
  Product pages, decision aids, comparison guides, and the Trust Center carry deeper material at
  the point where a reader asks for it (ADR-0100).
- **Treat public copy as a coordinated release surface.** Bring every mutable listing up to the
  current public release instead of leaving stale copy in place for the next launch, then repeat
  the sweep when the next release ships. Immutable package, registry, and catalog records change
  only with their owning release; do not churn historical versions.
- **End-user extension installation is store-only.** Packaged and public paths point to the Chrome
  Web Store. Only source-development docs may explain loading the repository extension directly for
  immediate local testing (ADR-0091).
- **Do not publish the founder's home address.** If a distribution channel makes a mailing address
  customer-visible, defer that native listing until a legitimate non-home public address exists.
  Use supported store interoperability in the meantime. For Edge, the Chrome Web Store adapter is
  the end-user path.
- **The browser adapter versions independently from the service.** The extension manifest owns the
  adapter version. The 1.0 runtime negotiates protocol majors and capabilities at connection time;
  do not infer compatibility from browser-family strings or silently revive the retired 0.8
  `compatibility.json` mechanism.
- **Persist before context loss.** On a "prep for compaction" / "handoff" / "save state" request,
  first update memory + durable docs (this file, STATUS, ADRs/LEDGERs) and commit, THEN emit a
  self-contained continuation prompt -- persist first, answer second.

Code and writing conventions are canonical in [AGENTS.md](../AGENTS.md) ("Code style" and
"Writing conventions"): ASCII only everywhere; no magic strings (namespaced constants module);
named event/state vocabularies as dedicated domain modules; docs human-plain with no AI-isms. This
file does not restate them -- follow AGENTS.md.

## Durable learnings (cross-cutting facts, not decisions)

- **Build/test in an isolated `CARGO_TARGET_DIR`**. Lightbox creates its own isolated process build
  by default. Live MCP clients continuously respawn `ghostlight-mcp-connector.exe`, Chromium keeps
  `ghostlight-browser-connector.exe` alive, and the running service holds `ghostlight.exe` against
  the linker.
  A plain `cargo build`/`test` can relink-fail (Windows os error 5) and silently leave a STALE
  binary.
- **The two reconnect paths mean different things.** `ghostlight-mcp-connector` retains only the selected
  revision and future-call continuity across a service restart; it fails pending calls truthfully
  and never replays an effect. `ghostlight-browser-connector` separately replays the browser identity frame so
  Chromium can keep its native port and browser slot (ADR-0062/0096).
- **A listed MCP tool is not a transport-liveness signal.** The MCP client owns connector stdio
  and may retain cached Ghostlight declarations after that connector exits. On `Transport closed`,
  reopen Ghostlight through that client; a standalone connector cannot repair the closed stdio and
  may create a different workspace. Inspect state before retrying an effectful call (ADR-0096).
- **Runtime roles are structural, and workspace routing has one identity.** Separate executable
  entry points plus crate dependency direction define the MCP edge, service, and browser relay;
  there is no process-global role marker. Browser frames route only by `WorkspaceId` in the
  compatibility `guid` field. Human client labels are presentation/audit data, never routing,
  scheduling, ownership, or authority keys (ADR-0096).
- **Chrome topology belongs at the browser shore.** The Rust service owns `WorkspaceId` authority,
  exact tab ownership, governance, scheduling, and browser-profile routing, but never native
  Chrome window or group ids. The extension owns live placement and one per-workspace tab/group
  record. It follows user-moved owned tabs and groups in their current windows, never moves an
  existing tab back, and may reuse an exact-title group for presentation without sharing authority
  or tab inventory. A browser-created child is adopted only when its exact opener belongs to one
  unambiguous workspace; adoption preserves Chrome's chosen window, group, and focus (ADR-0098/0099).
- **An upgrade is not active until the selected engine owns the endpoint.** Registering new paths
  and successfully spawning a singleton loser are not enough. Verify the endpoint owner, quiesce
  old self-heal paths, replace only a proven managed predecessor, and preserve an external/dev
  engine (ADR-0092).
- **Tab authority is established at the browser shore, never from an input handle.** An explicit
  `tabId` is verification-only: the current live workspace must already own it, and unknown and
  cross-workspace ids fail identically before any browser frame. Successful, correlated creator
  inventories and strict opted-in `tabDelta` results may atomically add exact declared tab ids to
  a workspace. A passively adopted browser child becomes service-authoritative only when a later
  creator inventory confirms it. The extension's managed-tab gate remains defense-in-depth for
  those owned tabs and still keeps guessed user tabs out (ADR-0066/0096/0099).
- **The 0.8 distribution records are historical evidence, not an active 1.0 release pipeline.**
  The 1.0 rebaseline intentionally removed the superseded launchers and release scripts. Rebuild
  distribution from current process boundaries before claiming that 1.0 can be packaged or
  published. Canonical public URLs remain `sylin.org`.
- **Release reception stays manual and evidence-labeled.** For 0.8, use
  `docs/research/public-reception-loop-0.8.md` at release, 7 days, and 30 days. Keep voluntary human
  reports separate from project-authored distribution and automation-prone counters. Three
  independent reports of one normalized first-use failure stop broader outreach until its owning
  documentation, doctor, or product path is fixed and verified. Do not add telemetry, tracking
  parameters, automatic review prompts, or a vendor reporting path to fill evidence gaps.
- **The changelog owns release changes.** Keep `CHANGELOG.md` current as work lands. Release
  preflight requires a non-empty version section, and GitHub release assembly inserts that exact
  section under `What's changed`; do not maintain a second hand-written release summary.
- **Remote-code claims distinguish extension logic from page automation.** All extension logic
  ships in the reviewed package, but `javascript_tool` carries an explicit local MCP-client
  instruction to CDP `Runtime.evaluate` in the attached page. Never collapse those two facts into
  the broader claim that every JavaScript string the extension evaluates ships in the package.
- **Chrome native messaging has directional limits.** Extension-to-host input may be large, but a
  single host-to-extension message is capped at 1 MiB. Keep the generic framing corruption ceiling
  separate from the Chrome outbound contract and use ADR-0074's negotiated bounded chunks for
  large browser-bound requests.
- **Debug observability is metadata-only.** MCP bodies and successful tool results can contain page
  text, form values, files, screenshots, or recordings. Never persist them in debug events; keep
  method/tool ids, states, counts, timings, and byte sizes only (ADR-0073).
- **A native-port or extension-worker restart is not a browser restart.** Chrome storage.session
  provides the process-generation proof used by ADR-0080 recovery. Do not clear an uncertain tab
  merely because the native host reconnected; require the exact terminal executor generation,
  command, request, and resource, tab destruction, or a changed browser-process generation.
- **A completed tab load is not proof that the current document can render extension UI.** An
  extension reload can invalidate an unchanged page's content-script receiver without causing
  navigation. Presentation delivery uses ADR-0081's content-script ready handshake plus exact
  Chrome document/revision acknowledgement and packaged on-demand reinjection. Never restore a
  direct fire-and-forget `tabs.sendMessage` path for Ghostlight page signage.
- **Visible scope and visible activity are different promises.** The persistent sky border means a
  tab is agent-reachable under ADR-0066's managed-tab boundary. Pills, scans, camera frames, and
  pointer effects explain transient work inside that boundary. Do not make scope depend on a tool
  happening to run or make an action effect establish reachability.

## Pointer index (where durable things live)

| Need | Look here |
| --- | --- |
| How to work here: conventions, boundaries, architecture | [AGENTS.md](../AGENTS.md) (start here) |
| Current state: version, in-flight work, owed items | [docs/STATUS.md](STATUS.md) |
| Current 1.0 intent, language, architecture, and acceptance | [docs/1.0/](1.0/) |
| Decisions (one per file), authoritative and immutable | [docs/adr/](adr/README.md) |
| Deep design rationale (superseded by ADRs where they differ) | [docs/SPEC.md](SPEC.md) |
| Build / run / deploy on a dev machine | [docs/DEV-LOOP.md](DEV-LOOP.md) |
| Directory submission facts, ready copy, and external gates | [docs/business/DIRECTORY-SUBMISSIONS.md](business/DIRECTORY-SUBMISSIONS.md) |
| Larger work: task batches (BOOTSTRAP + LEDGER) | `docs/tasks/<batch>/` |
| Machine-local state: which engine runs, install | `local/MACHINE-STATE.md` |
| Sensitive/working notes, credential *locations*, handoffs | `local/NOTES.md` |
| Founder legal / entity / financial planning (agents do NOT read) | `/private/` |
