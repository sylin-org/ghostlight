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
  before posting. Committing to `dev` is normal autonomous work; `dev -> main` merges and release
  tags are the owner's call.
- **Prefer the root fix over the spot fix.** If a spot fix is genuinely unavoidable, say so
  explicitly in the commit message so the debt stays visible.
- **The browser product stays in the local user's context.** Ghostlight is for visible work in the
  user's existing authenticated Chromium profile. Headless, isolated-profile, cloud, and remote
  browser execution are product exclusions, not missing parity work.
- **Browser placement belongs to the user.** Reuse the last-focused eligible normal window for new
  work and pin the MCP session there. Create a browser window only when none is eligible. A tab
  group is visible organization, not a user-facing security boundary; never move tabs or groups
  back after the user places them elsewhere (ADR-0085).
- **A Ghostlight test is a live test.** Test Ghostlight by calling its MCP tools from the active
  client against the real local engine, extension, and user's visible authenticated browser. For
  cross-platform proof, run that same live path on each target OS, including Windows and Linux.
  Do not substitute Playwright, a disposable browser profile, or an emulated harness unless the
  explicit subject is that harness or CI boundary.
- **Repository content is practitioner-first.** Developers should see the product, installation,
  first useful task, and exact no-account/free-core facts before organization procurement depth.
  Product pages and the Trust Center carry the buyer-focused material.
- **Persist before context loss.** On a "prep for compaction" / "handoff" / "save state" request,
  first update memory + durable docs (this file, STATUS, ADRs/LEDGERs) and commit, THEN emit a
  self-contained continuation prompt -- persist first, answer second.

Code and writing conventions are canonical in [AGENTS.md](../AGENTS.md) ("Code style" and
"Writing conventions"): ASCII only everywhere; no magic strings (namespaced constants module);
named event/state vocabularies as dedicated domain modules; docs human-plain with no AI-isms. This
file does not restate them -- follow AGENTS.md.

## Durable learnings (cross-cutting facts, not decisions)

- **Build/test in an isolated `CARGO_TARGET_DIR`**. Lightbox creates its own isolated process build
  by default. Live MCP
  clients continuously respawn `ghostlight-relay.exe` and a running service holds `target/*.exe`
  against the linker, so a plain `cargo build`/`test` can relink-fail (Windows os error 5) and
  silently leave a STALE binary.
- **The extension's tab-group membership gate -- not only the service -- keeps the agent out of the
  user's OWN tabs.** The service first-touch-adopts any unowned tabId (`claim_tab_live`), so the
  extension's "is this tab one we manage?" check is load-bearing scoping, not just defense in
  depth. Any change there must widen to "tabs we manage", never "any tab" (ADR-0066 context).
- **Distribution is automated and credential-gated** in `scripts/release.ps1`; the MCP registry
  publish is DNS-authed on the sylin.org apex; canonical URLs are `sylin.org` (the github.io site
  is retired, redirect-stubbed). Off-tree/secret change history is in `local/AUDIT-LOG.md`.
- **Public release and platform truth is product-owned.** `docs/public-status.json` is the
  canonical machine-readable release fallback, live-platform statement, and extension-store
  statement. The README must contain its exact public claims; the website consumes a synchronized
  fallback through `scripts/publish-website.ps1`. Run `scripts/check-public-surfaces.ps1` locally
  and with `-Online` after deployment instead of repairing either surface independently.
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
  merely because the native host reconnected; require the exact terminal command, tab destruction,
  or a changed browser-process generation.
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
| Decisions (one per file), authoritative and immutable | [docs/adr/](adr/README.md) |
| Deep design rationale (superseded by ADRs where they differ) | [docs/SPEC.md](SPEC.md) |
| Build / run / deploy on a dev machine | [docs/DEV-LOOP.md](DEV-LOOP.md) |
| Larger work: task batches (BOOTSTRAP + LEDGER) | `docs/tasks/<batch>/` |
| Machine-local state: which engine runs, install | `local/MACHINE-STATE.md` |
| Sensitive/working notes, credential *locations*, handoffs | `local/NOTES.md` |
| Founder legal / entity / financial planning (agents do NOT read) | `/private/` |
