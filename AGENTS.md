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
3. [docs/adr/README.md](docs/adr/README.md) -- the ADR index. **Before touching a subsystem,
   read its ADR(s).** ADRs are the authoritative record of every design decision; do not
   re-litigate a decided question, and do not silently contradict one. To change a decision,
   write a new ADR (or a marked amendment), never rewrite history.
4. [docs/SPEC.md](docs/SPEC.md) -- the original design specification. Still the best deep
   explanation of the governance model, but ADRs supersede it where they differ.
5. [docs/DEV-LOOP.md](docs/DEV-LOOP.md) -- read before any build/run/deploy work on a dev
   machine. It starts with a "when code changes, do this" table.
6. `local/MACHINE-STATE.md` and `local/NOTES.md` -- if present (both gitignored): machine-local
   truth (which engine is running, install state, local gotchas) and sensitive/working memory
   (owner context, credential *locations*, session handoffs). See [local/README.md](local/README.md).
7. [CONTRIBUTING.md](CONTRIBUTING.md) -- test tiers, PR expectations, licensing boundary.

Larger work is organized as task batches under `docs/tasks/<batch>/`, each with a
`BOOTSTRAP.md` (ground rules) and a `LEDGER.md` (durable progress, one task = one commit,
a RESUME HERE section). If you are executing a batch, the ledger is the source of truth
for what is done.

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
MCP Client <--stdio--> ghostlight-mcp-connector <--typed IPC--> ghostlight service
    <--browser IPC--> ghostlight-browser-connector <--native messaging--> Extension <--CDP--> Browser
```

It is a clean-room Rust build. The sole external reference is Anthropic's official Claude
in Chrome extension: we harvest its observable interface and technique, never its code
(ADR-0050 Decision 1; the clone under `reference/` is read-only study material).

## Repository layout (current truth)

The tree is a Cargo workspace (ADR-0044/0046, ADR-0051 P3):

- `crates/core/` -- the protocol-neutral engine and governance: `governance/`, `browser/`,
  `tool/`, and `hub/` modules. `crates/core/src/governance/` is the commercially licensed module
  (ADR-0027); everything else is Apache-2.0 OR MIT.
- `crates/transport/` -- typed owner-only service bridge, browser IPC, instance identity, and
  observability. Its bridge vocabulary carries product work, never MCP JSON-RPC.
- `crates/mcp-connector/` -- the `ghostlight-mcp-connector` stdio edge. Exact revision state machines live in
  `mcp_2025_11_25` and `mcp_2026_07_28`; this crate does not depend on core, governance, or browser
  execution.
- `crates/browser-connector/` -- the browser-only native-messaging pass-through binary
  `ghostlight-browser-connector`. It has no MCP-client role.
- `crates/lightbox/` -- dev-only governance harness (ADR-0056); publish=false, never shipped.
- `src/` -- the `ghostlight` binary crate (CLI + persistent service) re-exporting the crates.
- `extension/` -- the Manifest V3 extension. Policy-free and thin: Chrome-API mechanism
  only, no policy logic, no heavy processing (ADR-0053).
- `docs/` -- SPEC, ADRs, guides, trust center (`docs/trust/`), task batches, design notes.
- `scripts/` -- dev loop, e2e runner, release pipeline (`release.ps1`), install helpers.

If an older document draws a single-binary `src/` tree, trust the live tree over the drawing.

## The one inviolable constraint

**Ghostlight 1.0 is a clean-slate, orchestrator-owned product** (ADR-0103). Public tag `v0.8.0`
is the immutable working prototype. The current 0.9 worktree is an architecture experiment to
archive, not a release candidate or a base to keep repairing. Version 1.0 harvests observed intent
and lessons from those versions, not their production code, compatibility layers, or test mass.

- Do not extend the 0.9 operation pipeline. Capture it before reset, then implement 1.0 from the
  accepted bill of intent in `docs/1.0/`.
- Ghostlight has one model-facing language. Do not add vendor-, model-, or client-specific tool
  dialects without a new accepted ADR and a measured journey win.
- The service owns the product catalog, schemas, defaults, typed operations, state machines,
  governance, results, and recovery. The MCP connector owns protocol lifecycle and generic
  rendering only.
- The service is a domain-driven modular monolith. Organize it by bounded product contexts, keep
  all model-requested mutation behind one application executor and unit of work, and use explicit
  chokepoints for workspace state, governance, browser effects, and completion. Cross-context
  reactions use a small typed in-process event vocabulary. Do not introduce a generic event bus,
  actor system, workflow engine, CQRS split, event store, or microservice without a new accepted
  decision backed by a concrete need.
- The browser connector remains a frame relay. The policy-free extension implements typed physical
  browser primitives and content-free presentation, never model-facing tools or Ghostlight results.
- Keep schemas flat, typo-closed, omission-tolerant where safe, and aligned with executable
  defaults. Require only intent or authority that cannot be inferred safely.
- A feature composed from existing browser primitives changes only the orchestrator. A genuinely
  new Chrome capability may add one browser primitive; it does not move product orchestration into
  the adapter.
- Tests and process must earn their cost. Protect each distinct contract, safety invariant, failure
  branch, or real user journey once at its narrowest meaningful seam. Do not carry forward old test
  mass, duplicate the same proof across layers, or require ledgers, checkpoints, approval rounds,
  exhaustive matrices, and live-stack runs for routine implementation. Use ADRs for durable
  architecture and trust decisions; run broad and live gates at integration and release milestones.

Two more standing product constraints: **never phone home** (no telemetry, activation
servers, or update pings -- ADR-0028, the Continuity Promise), and **the extension stays
policy-free** (all policy, classification, and audit live in the binary).

Standing technical decisions (each has an ADR or spec section; do not re-litigate):

- The MCP protocol is hand-rolled JSON-RPC 2.0 over stdio in `crates/mcp-connector/`. Do NOT introduce an
  MCP SDK crate (dependency risk, and it must match the preserved schema format exactly). Protocol
  dates, lifecycle, metadata, request ids, and response envelopes never enter the service core.
- Runtime roles are structural: separate executable entry points and dependency direction replace
  the old process-global role marker. Browser work routes only by `WorkspaceId` in compatibility
  `guid`; a human client label is presentation/audit data, never routing or authority.
- Screenshots return only from `browser_take_screenshot`; other operations return bounded text and
  structured canonical facts.
  JPEG quality 55 falling back to 30; coordinate model per ADR-0010 (probe the CSS
  viewport + DPR, downscale to the token budget, rescale model coordinates back).
- Native messaging is the Chromium 4-byte little-endian length-prefix framing.

## Building and testing

**Load-bearing gotcha:** on a dev machine, live MCP clients continuously respawn
`ghostlight-mcp-connector.exe`, Chromium keeps `ghostlight-browser-connector.exe` alive, and a running service holds
`ghostlight.exe` against the linker. A plain `cargo build`/`cargo test` can fail with lock errors
or leave stale binaries. Build and test in an isolated target dir (`CARGO_TARGET_DIR`). Lightbox
manages its own isolated process build unless `--reuse-cache` is explicitly used on a clean CI
worker.

Two test tiers (ADR-0032, ADR-0051):

- **Fast, in-process**: plain `cargo test --workspace`. No processes spawned; the everyday
  gate. In-process fixtures live in `tests/support/` (note: tools that orchestrate internal
  sub-calls need `#[tokio::test(flavor = "multi_thread")]` -- documented in the fixture).
- **End-to-end (spawn)**: `cargo run -p ghostlight-lightbox -- run --all` launches real binaries
  over the IPC boundary from an isolated target dir and runs the named parity scenarios.

For 1.0 routine commits, run formatting, lint the touched targets, and run the focused tests that
protect the changed contract. Run the full workspace, extension, and live-browser gates at
integration milestones and release readiness. Do not turn every local edit into a release rehearsal.
Prototype maintenance may still need its existing compatibility gates until the 0.9 tree is archived.

The dev loop for seeing changes live (engine swap via `scripts/dev-loop.ps1`, shore respawn when
that shore changes, extension reload at `chrome://extensions`) is in
[docs/DEV-LOOP.md](docs/DEV-LOOP.md). One stack (ADR-0065/0096): one native host, one installed
service identity, one typed MCP-edge endpoint, one browser endpoint, and one `ghostlight` MCP
entry. Named instances (`--instance`) are a test-isolation seam only, not a user or dev workflow.

The Chrome adapter and service version independently (ADR-0093). The manifest owns the adapter
version; `compatibility.json` owns its inclusive service range. Do not bump the manifest for a
service-only release.

## Code style

- Rust 2021. `thiserror` for typed library errors, `anyhow` in main/integration code.
- Explicit types in public APIs; doc comment on every public function; module-level doc
  comment on every module explaining its role in the architecture.
- Tests: integration in `tests/`, unit inline `#[cfg(test)]`.
- No `unsafe` unless absolutely required (documented why).
- `rustfmt` formatted; `clippy` clean with `-D warnings`.
- No magic strings: repeated string/enum-like literals belong in a namespaced constants
  module (see `crates/core/src/constants.rs` for the pattern).
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
- Never touch the trained fields of the 13 sacred tool schemas (see above).
- Never add phone-home behavior of any kind (ADR-0028 is normative and permanent).
- Never put policy logic, classification, or audit in the extension.
- Never publish or post anything outward-facing (npm, store listings, social posts, comments
  on external repos, anything leaving this repo) without explicit owner confirmation. Draft
  it, then wait. Committing to `dev` is normal autonomous work; `dev -> main` merges and
  release tags are the owner's call.
- Never read or publish the contents of `/private/` or `saps/` (gitignored founder-personal
  material) into anything shared.
- Never weaken an over-claim guard in `docs/trust/`: every public claim there was red-teamed
  against the tree; keep claims and code in lockstep (change the code first, or soften the
  claim).

## What NOT to build (annotated scope exclusions)

Still excluded: OIDC/SAML/LDAP federation (identity is local-file / env-resolved);
content inspection or DLP (governance decides on capability + domain, never page content);
Firefox support (Chromium Manifest V3 + CDP only).

Superseded with nuance (the ADR is authoritative where they differ):

- Remote policy: `managed://` central policy distribution exists (ADR-0055, signed bundles,
  fail-closed last-known-good cache). The per-user `--manifest` still has no HTTP source.
- Multi-user: the Hub (ADR-0030) multiplexes multiple concurrent sessions, all admitted as
  the same OS user. Multi-session, single-user -- never a shared multi-tenant server.
- Manifest signing: managed:// bundles and commercial licenses carry a hybrid post-quantum
  signature (Ed25519 + ML-DSA-65). A plain per-user manifest file is still unsigned.

## Personal and machine-local data

- `/private/` (gitignored) -- founder-personal stash (legal, entity, financial planning). Not for
  agents; do not read or publish it.
- `local/` (gitignored except its README) -- machine-local dev state and working notes that any
  local agent may read and update: `MACHINE-STATE.md` (which engine is running, install state,
  local gotchas) and `NOTES.md` (owner/working context, credential *locations* -- never values --,
  session handoffs). See [local/README.md](local/README.md).
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
