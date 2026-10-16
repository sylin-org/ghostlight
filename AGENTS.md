# Ghostlight -- agent guide

This is the canonical onboarding document for any coding agent (or human) working in this
repository, regardless of which tool or model you are. `CLAUDE.md` is a thin pointer here.
Assume you have no memory of prior sessions: everything you need is in this file and the
documents it points to.

## Branches

There are only two branches: `main` and `dev`, locally and on the remote.
`main` contains the published product. All implementation, experiments, dependency
updates and release preparation happen on `dev`. Do not create feature, agent or
release branches. Advance `main` from `dev` only after the release is publicly
delivered with its compatible adapter. Preserve historical work in archive tags
and pending edits in Git stashes; never discard it to clean up branch names.

## Read this first (in order)

1. [docs/MEMORY.md](docs/MEMORY.md) -- standing owner directives and durable learnings, with a
   pointer to the documentation map.
2. The current-state sections at the top of [docs/STATUS.md](docs/STATUS.md): in-flight work,
   branch state, and any release in progress. Older dated sections are history; read them only
   when relevant.
3. Before changing a subsystem, read its ADRs (index: [docs/adr/README.md](docs/adr/README.md))
   and the docs/1.0 contract that covers it ([INTENT](docs/1.0/INTENT.md),
   [LANGUAGE](docs/1.0/LANGUAGE.md), [ARCHITECTURE](docs/1.0/ARCHITECTURE.md),
   [ACCEPTANCE](docs/1.0/ACCEPTANCE.md)). ADRs are authoritative. Do not re-litigate or silently
   contradict one. To change a decision, write a new ADR or a marked amendment, never rewrite
   history.

Read on demand: [docs/SPEC.md](docs/SPEC.md) (deep governance rationale; ADRs win),
[docs/DEV-LOOP.md](docs/DEV-LOOP.md) (required before any build/run/deploy on a dev machine),
[CONTRIBUTING.md](CONTRIBUTING.md) (test tiers and PR expectations).

Work split across concurrent agents or lanes, or run unattended across many sessions, may use a
task batch under `docs/tasks/<batch>/`: a `BOOTSTRAP.md` (ground rules for that batch) and either
one `LEDGER.md` or one ledger file per lane. The ledger is the record of what is done. Read its
RESUME HERE block before you start, and update it in the same commit as the slice it advances.
Process rules in an older batch's BOOTSTRAP bind only that batch. Other work tracks its state in
`docs/STATUS.md`. [docs/tasks/README.md](docs/tasks/README.md) indexes the batches. Batch files
describe the tree as it was when written; check the current tree before trusting any path or
code excerpt in them. Tree facts and line numbers in any task file, including the active batch,
are as-of-authoring hints: re-read the live tree and find code by name, signature or anchor text
before editing.

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
  seams before retiring a working predecessor. The retired 0.8 layer lives only in git history
  and the `archive/0.9-pre-1.0` tag.
- Product identity is inherited. Names, icons, visual language, animation, public character, legal
  identity, and user expectations survive the rewrite unless the owner explicitly changes them.
- Internal tools and model-facing descriptions are mechanisms, not product identity. The
  orchestrator owns them and may redesign them deliberately.

## Cross-session coordination

When the owner says `execute coordination/CHAT.md`, first read and follow
`coordination/INSTRUCTIONS.md`. The tracked chat carries only messages between active Codex
sessions; `coordination/RESULTS.md` carries the latest result.

## What this project is

Ghostlight is a governed browser automation MCP server: a protocol-versioned edge that is the MCP
relay and the browser relay, a persistent Rust service, and a thin Chromium extension. Together
they give any MCP client (Claude Code, Cursor, Zed, Cline, ...) controlled access to the user's authenticated
browser session, with identity-bound access control, per-action capability classification (read /
action / write / execute), and structured audit logging. The unconstrained engine is first-class
("all-open stays first-class"); governance is an overlay, never a tax on the ungoverned.

```
MCP Client <--stdio--> ghostlight mcp <--relay 4--> ghostlight-service
    <--relay 4--> ghostlight chrome-extension://... <--native messaging--> Browser adapter
```

It is a clean-room Rust build. The sole external reference is Anthropic's official Claude
in Chrome extension: we harvest its observable interface and technique, never its code
(ADR-0050 Decision 1; the clone under `reference/` is read-only study material).

## Repository layout (current truth)

The tree is a Rust workspace with these process/trust concerns:

- `crates/orchestrator/` -- the product authority, built as `ghostlight-service`: language,
  workspace aggregate, governance, browser coordination, presentation decisions, execution, and
  completion. Its
  `src/browser/adapter/` is the browser adapter component: one module per browser family
  (`chromium/`), holding the wire levels the extension can speak and every page asset, JavaScript
  and CSS, that runs in a page.
- `crates/relay/` -- the frozen relay protocol 4: endpoint, framing, start reasons, the session
  listener and the route an edge invocation takes.
- `crates/edge/` -- the frozen `ghostlight` executable. It is the MCP relay (`ghostlight mcp`),
  the browser relay (a `chrome-extension://` argument) and the front door that hands every other
  command to `ghostlight-service`. It owns protocol lifecycle and generic rendering, never
  product decisions, and knows no verb.
- `crates/bridge/` -- the service's typed diagnostics contracts.
- `extension/` -- the Manifest V3 Chromium adapter. It owns Chromium APIs, DOM-local observation,
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
  visual rendering terminate there without making workspace-authority or journey decisions. Thin
  means nothing bleeds through the extension, not that it does little: a capability that is
  physically the browser's belongs there, because only that layer has it (ADR-0109 corrects
  ADR-0053).
- Relays and adapters must remain stable as service capabilities evolve. Prefer negotiated closed
  mechanisms and orchestrator-owned semantics over feature-specific fringe changes.
- All model-requested work crosses one executor, one workspace aggregate, one governance facade,
  one browser port, and one completion path.
- Sessions, workspaces, operations, browser instances, browser families, and harness connections
  are plural domain collections. Do not build singleton assumptions into new contracts; a single
  adapter slot once made a duplicate connection silently unrecoverable (ADR-0114). Bounded
  per-owner bindings, such as one workspace per connection (ADR-0196) or one browser per
  workspace, are fine.
- Keep the design simple: add architecture only when a concrete invariant needs it. A logical
  boundary does not by itself earn a process, crate, or service. Keep the small closed DomainEvent
  vocabulary, and add no generic event bus, actors, workflows, CQRS, event sourcing, reflection
  registries, microservices, or extra process or crate (ADR-0096, ADR-0176,
  docs/1.0/INTENT.md). Domain registries such as the workspace and tool registries are not what
  this rules out.
- Never phone home. No telemetry, activation service, automatic update ping, or hidden network
  dependency is allowed.
- Keep release work equally simple. A gate must prevent a demonstrated failure, prove a user
  promise, or make recovery safer. Optional directories, repeated restamping, and one giant release
  conductor are not candidate gates.

## Building and testing

Before committing, run the checks for what you changed. CI runs the full set on every push.

- Any Rust, `Cargo.toml`, `Cargo.lock` or `rust-toolchain.toml` change: `cargo fmt --all -- --check`
  and `cargo clippy --workspace --all-targets -- -D warnings` (always workspace-wide: a manifest
  change can break clippy in untouched code), then `cargo test` for the touched crates, or
  `cargo test --workspace` if bridge, relay, win-peer or a manifest changed.
- Extension: `npm test` in `extension/`.
- Page assets: `node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`,
  plus `cargo test -p ghostlight-service` and `npm test` in `extension/`, which also read the
  expanded bundle.
- `crates/relay`, `crates/edge`, any `Cargo.toml` or `Cargo.lock`, or `tests/golden/`:
  `node --test tests/guards/connectors.test.mjs`.
- The model surface changes on purpose: regenerate the catalog golden with
  `GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p ghostlight-service the_catalog_matches`, and the
  result goldens with the same variable on `work::tests::golden`
  (`GHOSTLIGHT_CATALOG_UPDATE=1 cargo test -p ghostlight-service work::tests::golden`).
- Changed runtime JavaScript that no test runs: `node --check` on it.
- Docs or other text: `pwsh -NoProfile -File scripts/check-repository-integrity.ps1` (ASCII and links).

The process and live browser journeys in `tests/` exercise the real executable boundaries. Run
them when changes touch process startup, relay reconnect, installation state, or live browser
behavior.

Journeys read `.target-ghostlight-1.0/debug` unless `GHOSTLIGHT_BIN_DIR` says otherwise. Build
there or point `GHOSTLIGHT_BIN_DIR` at your fresh build, or a journey quietly passes against stale
binaries. The live service runs from copies in `~/.ghostlight`, so it never locks a repo build.
`scripts/dev-loop.ps1` builds its deploy in `.target-dev-loop`. When another agent may be building
or running journeys in this checkout, use `.target-lane-<name>` and pass `GHOSTLIGHT_BIN_DIR` to
every journey.

Live installed journeys (`tests/live-journey.mjs`, `tests/settlement-live-journey.mjs`) use the
home's `<home>/bin` and refuse any other `GHOSTLIGHT_BIN_DIR`.

## Code style

- Edition and lints come from the workspace `Cargo.toml`, and fmt and clippy `-D warnings` are in
  the commit gate. `unsafe` is forbidden everywhere except `crates/win-peer` and the `os` module of
  `crates/relay` (enforced by the lints and the governance unsafe-confinement test), and every
  unsafe call site there carries a `// SAFETY:` note. Otherwise match the surrounding code:
  `thiserror` for typed library errors, `anyhow` at binary and integration edges, unit tests
  inline and process tests in `tests/`.
- Every module opens with a doc comment stating its role in the architecture. Document a public
  item when its name and types do not state its contract: lifecycle and pairing obligations
  (acquire/release), invariants, failure codes, side effects.
- Repeated literals become a named constant beside their use. A vocabulary that leaves its module
  lives in one domain module (enum plus rendering), even with one caller. That covers vocabularies
  that cross a process or language boundary (wire message types, refusal codes, lifecycle events,
  FX names), that the model reads (tool and field names in hints, outcome and refusal codes), that
  are persisted or audited (audit event names, capability classes, policy and config keys), or that
  gate a trust boundary (destinations, permissions). Single-use literals that stay inside one
  module need neither.
- Prefer the root fix over the spot fix. If a spot fix is genuinely unavoidable, say so
  explicitly in the commit message so the debt is visible.

## Writing conventions (code AND docs)

- **ASCII only, everywhere, docs included.** No em-dashes, no arrows, no curly quotes, no
  decorative unicode. Use `--` for a dash, `->` only inside code blocks.
  Write non-ASCII test input as an escape, never a literal byte.
- Docs are written human-plain: no AI-isms, no filler enthusiasm, no "delve"/"leverage"
  prose. Short sentences beat clause chains.
- Commit messages: conventional commits, `<type>(<scope>): <description>` (scope optional).
  Types: feat, fix, refactor, docs, test, chore, perf, ci.
- Sign off every commit (`git commit -s`); contributions are under the DCO and Apache-2.0 OR MIT
  (CONTRIBUTING.md, ADR-0140).
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

When you finish significant work, update docs/STATUS.md. If you are executing a batch, record
progress in its LEDGER, and touch STATUS only to say the batch started, changed scope or closed.
Record a new decision as an ADR. Add to [docs/MEMORY.md](docs/MEMORY.md) only a new standing
owner preference or a cross-cutting learning. Keep this file pointing at reality, and trust the
tree and `git log` over prose that disagrees.
