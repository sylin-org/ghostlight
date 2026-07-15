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

Ghostlight is a governed browser automation MCP server: a Rust service plus a thin
Chromium extension that gives any MCP client (Claude Code, Cursor, Zed, Cline, ...)
controlled access to the user's authenticated browser session, with identity-bound access
control, per-action capability classification (read / action / write / execute), and
structured audit logging. The unconstrained engine is first-class ("all-open stays
first-class"); governance is an overlay, never a tax on the ungoverned.

```
MCP Client <--stdio--> Relay <--IPC--> Service <--native messaging--> Extension <--CDP--> Browser
```

It is a clean-room Rust build. The sole external reference is Anthropic's official Claude
in Chrome extension: we harvest its observable interface and technique, never its code
(ADR-0050 Decision 1; the clone under `reference/` is read-only study material).

## Repository layout (current truth)

The tree is a Cargo workspace (ADR-0044/0046, ADR-0051 P3):

- `crates/core/` -- the engine and governance: `governance/`, `browser/`, `mcp/`, `hub/`
  modules. `crates/core/src/governance/` is the commercially licensed module (ADR-0027);
  everything else is Apache-2.0 OR MIT.
- `crates/transport/` -- shared IPC, instance identity, observability.
- `crates/relay/` -- the thin pass-through binary `ghostlight-relay` (one exe, two roles:
  `--role agent` MCP stdio, browser role auto-detected from the Chrome extension origin).
- `crates/lightbox/` -- dev-only governance harness (ADR-0056); publish=false, never shipped.
- `src/` -- the `ghostlight` binary crate (CLI + persistent service) re-exporting the crates.
- `extension/` -- the Manifest V3 extension. Policy-free and thin: Chrome-API mechanism
  only, no policy logic, no heavy processing (ADR-0053).
- `docs/` -- SPEC, ADRs, guides, trust center (`docs/trust/`), task batches, design notes.
- `scripts/` -- dev loop, e2e runner, release pipeline (`release.ps1`), install helpers.

If an older document draws a single-binary `src/` tree, trust the live tree over the drawing.

## The one inviolable constraint

**The trained tool-schema surface is sacred** (ADR-0007, amended by ADR-0034 Decision 7).
The 13 tool schemas harvested from the reference extension -- names, parameter names,
types, description strings, enums -- stay byte-stable. Models were trained against them.

- Never rename, remove, paraphrase, or reorder anything a trained model relies on.
- Growth is additive only: new tools join via the capability registry (`explain`, `script`,
  `form_fill`, `wait_for` are sanctioned precedents), and new OPTIONAL parameters may be
  added to existing tools.
- Schemas live as const string literals (raw JSON) in the code, not built programmatically.
- `tests/tool_schema_fidelity.rs` is the regression snapshot over the declared surface.

Two more standing product constraints: **never phone home** (no telemetry, activation
servers, or update pings -- ADR-0028, the Continuity Promise), and **the extension stays
policy-free** (all policy, classification, and audit live in the binary).

Standing technical decisions (each has an ADR or spec section; do not re-litigate):

- The MCP protocol is hand-rolled JSON-RPC 2.0 over stdio. Do NOT introduce an MCP SDK
  crate (dependency risk, and it must match the preserved schema format exactly).
- Screenshots return only on `computer` actions that produce one (`screenshot`, `scroll`,
  `zoom`); everything else returns a text confirmation (roughly 10x context savings).
  JPEG quality 55 falling back to 30; coordinate model per ADR-0010 (probe the CSS
  viewport + DPR, downscale to the token budget, rescale model coordinates back).
- Native messaging is the Chromium 4-byte little-endian length-prefix framing.

## Building and testing

**Load-bearing gotcha:** on a dev machine, live MCP clients continuously respawn
`ghostlight-relay.exe` and a running service holds `target/*.exe` against the linker, so a
plain `cargo build`/`cargo test` can fail with lock errors or leave stale binaries. Build
and test in an isolated target dir (`CARGO_TARGET_DIR`). Lightbox manages its own isolated
process build unless `--reuse-cache` is explicitly used on a clean CI worker.

Two test tiers (ADR-0032, ADR-0051):

- **Fast, in-process**: plain `cargo test --workspace`. No processes spawned; the everyday
  gate. In-process fixtures live in `tests/support/` (note: tools that orchestrate internal
  sub-calls need `#[tokio::test(flavor = "multi_thread")]` -- documented in the fixture).
- **End-to-end (spawn)**: `cargo run -p ghostlight-lightbox -- run --all` launches real binaries
  over the IPC boundary from an isolated target dir and runs the named parity scenarios.

Gate before every commit: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
fast-tier tests green. Extension JS: `node --test` under `extension/`, plus `node --check`
on changed files.

The dev loop for seeing changes live (engine swap via `scripts/dev-loop.ps1`, extension
reload at `chrome://extensions`) is in [docs/DEV-LOOP.md](docs/DEV-LOOP.md). One stack
(ADR-0065): one native host, one endpoint, one `ghostlight` MCP entry; the engine is
whichever service holds the endpoint. Named instances (`--instance`) are a test-isolation
seam only, not a user or dev workflow.

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
