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
- **Persist before context loss.** On a "prep for compaction" / "handoff" / "save state" request,
  first update memory + durable docs (this file, STATUS, ADRs/LEDGERs) and commit, THEN emit a
  self-contained continuation prompt -- persist first, answer second.

Code and writing conventions are canonical in [AGENTS.md](../AGENTS.md) ("Code style" and
"Writing conventions"): ASCII only everywhere; no magic strings (namespaced constants module);
named event/state vocabularies as dedicated domain modules; docs human-plain with no AI-isms. This
file does not restate them -- follow AGENTS.md.

## Durable learnings (cross-cutting facts, not decisions)

- **Build/test in an isolated `CARGO_TARGET_DIR`** (or use `scripts/test-e2e.ps1`). Live MCP
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
