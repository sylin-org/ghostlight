# Ghostlight -- agent guide

This is the canonical onboarding document for any coding agent (or human) working in this
repository, regardless of which tool or model you are. `CLAUDE.md` is a thin pointer here.
Assume you have no memory of prior sessions: everything you need is in this file and the
documents it points to.

## Read this first (in order)

1. [docs/MEMORY.md](docs/MEMORY.md) -- the cross-agent project memory: the owner's standing
   working preferences, durable cross-cutting learnings, and a pointer index to everything below.
   Read it first; it tells you where each kind of durable fact lives. Its sensitive/machine-local
   counterpart is `local/NOTES.md` (gitignored).
2. [docs/STATUS.md](docs/STATUS.md) -- where the project stands right now: version state,
   open PRs, in-flight work, and the owed-items list. Read it before starting anything.
3. [docs/1.0/INTENT.md](docs/1.0/INTENT.md),
   [docs/1.0/LANGUAGE.md](docs/1.0/LANGUAGE.md),
   [docs/1.0/ARCHITECTURE.md](docs/1.0/ARCHITECTURE.md), and
   [docs/1.0/ACCEPTANCE.md](docs/1.0/ACCEPTANCE.md) -- the current implementation contract.
4. [docs/adr/README.md](docs/adr/README.md) -- the ADR index. **Before touching a subsystem,
   read its ADR(s).** ADRs are the authoritative record of every design decision; do not
   re-litigate a decided question, and do not silently contradict one. To change a decision,
   write a new ADR (or a marked amendment), never rewrite history.
5. [docs/SPEC.md](docs/SPEC.md) -- the original design specification. Still the best deep
   explanation of the governance model, but ADRs supersede it where they differ.
6. [docs/DEV-LOOP.md](docs/DEV-LOOP.md) -- read before any build/run/deploy work on a dev
   machine. It starts with a "when code changes, do this" table.
7. `local/MACHINE-STATE.md` and `local/NOTES.md` -- if present (both gitignored): machine-local
   truth (which engine is running, install state, local gotchas) and sensitive/working memory
   (owner context, credential *locations*, session handoffs). Do not read them without explicit
   owner authorization. See [local/README.md](local/README.md).
8. [CONTRIBUTING.md](CONTRIBUTING.md) -- test tiers, PR expectations, and the whole-repo
   Apache-2.0 OR MIT / DCO contribution terms.

Larger work is organized as task batches under `docs/tasks/<batch>/`, each with a
`BOOTSTRAP.md` (ground rules) and a `LEDGER.md` (durable progress, one task = one commit,
a RESUME HERE section). If you are executing a batch, the ledger is the source of truth
for what is done. [docs/tasks/README.md](docs/tasks/README.md) indexes all of them and says which
predate the 1.0 internals rebuild, which is all but one: never take a file path or code excerpt
from an old batch as current.

## Authority and historical continuity

The complete documentation corpus is project memory. Root documentation, `docs/`, ADRs,
research, trust material, legal material, public assets, and task records must not be discarded
or quarantined during an implementation rewrite.

- The active request, this file, the current source and tests, and the four `docs/1.0/` contracts
  govern implementation work.
- Historical ADRs remain immutable evidence of why the product evolved. A new decision supersedes
  an old one; it does not erase or silently rewrite it.
- Older product documents remain required context. Where they describe superseded implementation
  details, the current source and `docs/1.0/` contract win. Update the active document instead of
  deleting the history.
- Source from older implementation commits and branches is not implementation authority for the
  1.0 clean-room rewrite. Preserve its Git history, but do not copy it into the new internals.
- Clean-room does not make tests, fixtures, CI, packaging knowledge, platform findings, release
  evidence, or publication history disposable. Inventory and translate those assets onto current
  seams before retiring a working predecessor. The active 0.8 harvest is indexed from
  `docs/0.8/HARVEST.md`.
- Product identity is inherited. Names, icons, visual language, animation, public character, legal
  identity, and user expectations survive the rewrite unless the owner explicitly changes them.
- Internal tools and model-facing descriptions are mechanisms, not product identity. The
  orchestrator owns them and may redesign them deliberately.

## Cross-session coordination

When the owner says `execute coordination/CHAT.md`, first read and follow
`coordination/INSTRUCTIONS.md`. The tracked chat carries only messages between active Codex
sessions; `coordination/RESULTS.md` carries the latest result.

## What this project is

Ghostlight is a governed browser automation MCP server: a protocol-versioned MCP edge, a
persistent Rust service, a browser-only relay, and a thin Chromium extension. Together they give
any MCP client (Claude Code, Cursor, Zed, Cline, ...) controlled access to the user's authenticated
browser session, with identity-bound access control, per-action capability classification (read /
action / write / execute), and structured audit logging. The unconstrained engine is first-class
("all-open stays first-class"); governance is an overlay, never a tax on the ungoverned.

```
MCP Client <--stdio--> ghostlight-mcp-connector <--typed IPC--> ghostlight orchestrator
    <--browser IPC--> ghostlight-browser-connector <--native messaging--> Extension <--CDP--> Browser
```

It is a clean-room Rust build. The sole external reference is Anthropic's official Claude
in Chrome extension: we harvest its observable interface and technique, never its code
(ADR-0050 Decision 1; the clone under `reference/` is read-only study material).

## Repository layout (current truth)

The tree is a Rust workspace with four process/trust concerns:

- `crates/orchestrator/` -- the product authority: language, workspace aggregate, governance,
  browser coordination, presentation decisions, execution, and completion.
- `crates/bridge/` -- small typed service and browser relay contracts plus framing, and the one
  shared local service-lifecycle seam both connectors use to demand-start the sibling authority.
- `crates/mcp-connector/` -- the stable MCP stdio edge. It owns protocol lifecycle and generic
  rendering, never product decisions.
- `crates/browser-connector/` -- the stable native-messaging/browser relay executable.
- `extension/` -- the Manifest V3 browser adapter. It owns Chromium APIs, DOM-local observation,
  browser-specific durability, and content-free presentation, but no policy or product journeys.
- `docs/` -- the complete product history and current 1.0 contracts: SPEC, ADRs, guides, trust
  center, research, design records, task ledgers, and public material.
- `open-spec/`, `site/`, `examples/`, and root documents -- inherited public, product, legal, and
  historical surfaces. Keep them and reconcile active claims with the current implementation.

If an older document describes a prior source layout, treat the drawing as historical and use the
live tree plus `docs/1.0/ARCHITECTURE.md` for current placement.

## Product and architecture invariants

- The orchestrator is the sole product mutation point and owns model-facing language. Both
  directions of that language live in `crates/orchestrator/src/language/`: the catalog and decoding
  for what Ghostlight accepts, and `outcome.rs` for what Ghostlight says happened. A completed
  action's sentence, its safe next steps, and its content-free measurements come from one typed
  `Outcome`. Do not write a completed-action sentence as a literal at a call site.
- The MCP bridge owns protocol lifecycle, framing, correlation, cancellation forwarding, catalog
  retrieval, and generic invocation/result rendering only.
- The browser bridge owns typed framing, correlation, connection lifecycle, and relay only.
- The extension is policy-free. Chrome APIs, page-local DOM access, browser-specific recovery, and
  visual rendering terminate there without making workspace-authority or journey decisions.
- Relays and adapters must remain stable as service capabilities evolve. Prefer negotiated closed
  mechanisms and orchestrator-owned semantics over feature-specific fringe changes.
- All model-requested work crosses one executor, one workspace aggregate, one governance facade,
  one browser port, and one completion path.
- Sessions, operations, browser instances, and harness connections are plural domain collections.
  Do not build singleton assumptions into new contracts.
- Use a small closed domain-event vocabulary. Do not add a generic event bus, actors, workflows,
  CQRS, event sourcing, reflection registries, or microservices.
- Never phone home. No telemetry, activation service, automatic update ping, or hidden network
  dependency is allowed.
- Keep the design simple. Add architecture only when a concrete invariant requires it.
- Keep release work equally simple. A gate must prevent a demonstrated failure, prove a user
  promise, or make recovery safer. Optional directories, repeated restamping, and one giant release
  conductor are not candidate gates.

## Building and testing

Live clients and Chromium may hold release executables open. Use an isolated target directory when
a live deployment would otherwise contend with the build under test.

Gate before every commit:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm test` from `extension/`
- `node --check` on changed extension JavaScript

The process and live browser journeys in `tests/` exercise the real executable boundaries. Run
them when changes touch process startup, relay reconnect, installation state, or live browser
behavior.

`tests/process-journey.mjs` resolves executables from `.target-ghostlight-1.0/debug` unless
`GHOSTLIGHT_BIN_DIR` says otherwise. If you built into any other target directory, pass it
(`GHOSTLIGHT_BIN_DIR=.target-yours/debug node tests/process-journey.mjs`) or the journey will
quietly pass against stale binaries and tell you nothing about your change.

## Code style

- Rust 2021. `thiserror` for typed library errors, `anyhow` in main/integration code.
- Explicit types in public APIs; doc comment on every public function; module-level doc
  comment on every module explaining its role in the architecture.
- Tests: integration in `tests/`, unit inline `#[cfg(test)]`.
- No `unsafe` unless absolutely required (documented why).
- `rustfmt` formatted; `clippy` clean with `-D warnings`.
- No magic strings: repeated string/enum-like literals belong beside their usage in a named
  module.
- Named event/state vocabularies (wire message types, lifecycle events, FX names) belong in
  a dedicated domain module (struct/enum plus rendering), not scattered inline literals --
  even when there is only one caller today.
- Prefer the root fix over the spot fix. If a spot fix is genuinely unavoidable, say so
  explicitly in the commit message so the debt is visible.

## Writing conventions (code AND docs)

- **ASCII only, everywhere, docs included.** No em-dashes, no arrows, no curly quotes, no
  decorative unicode. Use `--` for a dash, `->` only inside code blocks.
- Docs are written human-plain: no AI-isms, no filler enthusiasm, no "delve"/"leverage"
  prose. Short sentences beat clause chains.
- Commit messages: conventional commits, `<type>(<scope>): <description>` (scope optional).
  Types: feat, fix, refactor, docs, test, chore, perf, ci.
- One logical change per commit; every commit leaves a green tree.

## Boundaries -- never do these

- Never copy code from `reference/` (clean-room rule; interface and technique only).
- Never add phone-home behavior of any kind (ADR-0028 is normative and permanent).
- Never put policy logic, classification, or audit in the extension.
- Never discard or quarantine product documentation, ADRs, licenses, public identity, or historical
  records as part of an internal rewrite.
- Never publish or post anything outward-facing (npm, store listings, social posts, comments
  on external repos, anything leaving this repo) without explicit owner confirmation. Draft it,
  then wait. Local commits are normal; pushes, merges, releases, and external changes are the
  owner's call.
- Never read or publish the contents of `/private/` or `saps/` (gitignored founder-personal
  material) into anything shared.
- Never read machine-local files under `local/` without explicit owner authorization.
- Never weaken an over-claim guard in `docs/trust/`: every public claim there was red-teamed
  against the tree; keep claims and code in lockstep (change the code first, or soften the
  claim).

## Scope discipline

Historical ADRs include implemented, superseded, proposed, and deferred capabilities. Their
presence does not prove that the current 1.0 tree implements them. Check current source and tests
before making a product claim, then either implement the missing journey properly or mark the
active documentation honestly.

## Personal and machine-local data

- `/private/` (gitignored) -- founder-personal stash (legal, entity, financial planning). Not for
  agents; do not read or publish it.
- `local/` (gitignored except its README) -- machine-local dev state and working notes. Do not read
  or update its contents without explicit owner authorization. See [local/README.md](local/README.md).
- **Memory is project-level, not model-private** (the owner delegates across several agents/LLMs).
  Durable memory lives in the repo: standing preferences + learnings + index in
  [docs/MEMORY.md](docs/MEMORY.md), current state in `docs/STATUS.md` (or a batch LEDGER), decisions
  in ADRs, machine/sensitive facts in `local/`. A model's own memory system (e.g. Claude Code's
  auto-memory) is a secondary cache of these and must never diverge from or compete with them.

## Keeping this system honest

When you finish significant work: update `docs/STATUS.md` (and the batch LEDGER if you are
in one), record new decisions as ADRs, capture any durable cross-cutting learning or standing
preference in [docs/MEMORY.md](docs/MEMORY.md) (session handoffs and sensitive/working notes go in
`local/NOTES.md`), and keep this file pointing at reality. Write durable facts to these project
scopes, never to a model-private memory store (that is only a cache, and it must not diverge from
these). Trust the tree and `git log` over any prose that disagrees with them.
