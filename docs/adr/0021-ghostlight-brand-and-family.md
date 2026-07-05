# 0021. Ghostlight brand and product family

- Status: Accepted
- Date: 2026-07

## Context

The working name "Browser MCP" collides with an established product
(browsermcp.io) and at least two other repositories, so discovery and identity
suffer (see docs/research/13). A rename was needed before any public artifact
accrues traction to the collided name. The project is also the first of a
possible family of governance-friendly, premium-UX MCP tools, so the name had
to work as a family umbrella, not a single-tool label, and it had to lead with
delight rather than reading as a security product.

Two research passes (verify workflows recorded in the session) generated and
collision-checked candidates against the publisher's org conventions
(sylin-org). The org runs a dual naming register: products with identity get
evocative names with a load-bearing metaphor explained in the README (koan,
koi, zen-garden, hokora, agyo, koan-cottage), while plain descriptive names are
reserved for plumbing and forks. The prefix "os-" is already a claimed org
brand ("Operational Symmetry", the os-tools library crates), so "os-mcp" was
rejected as misleading.

## Decision

The family brand is **Ghostlight**. The metaphor is the single lamp a theater
leaves burning on an empty stage: the ghost is the phantom cursor and the light
is the agent-active glow (delight, named directly), and the light exists so
nobody falls off the dark stage edge when no one is watching (governance:
audit, sacred never-touch lists, take-the-wheel, all discoverable in the same
image). It is thematically consonant with the org's motif of a guardian
presence in a bounded sacred space (hokora, kekkai), which the README states in
one line.

- First product: **Ghostlight Browser** (this project), shipped initially with
  a Chromium extension as its adapter. Other browser-family adapters may follow
  as they implement equivalent automation APIs; the engine and governance layer
  are adapter-agnostic.
- Family scheme: Ghostlight Browser, then further surfaces as separate products
  under the same brand, each a thin adapter over the shared governed engine.
- Repository and crates: at rename time the repo becomes a Cargo workspace
  under sylin-org named for the brand, following org convention (brand-named
  workspace, `<brand>-<component>` sub-crates, dual Apache-2.0 OR MIT, shared
  workspace version). Shared crates (core, policy, audit) are extracted only
  when the second product begins (rule of two, no premature abstraction).
- Binary and MCP server id use the brand; a short CLI alias mitigates the
  10-letter length.

Runner-up reserve: **Genkan** (best on-palette governance metaphor; its
crates.io name is free and should be claimed defensively). Fallback trigger:
if trademark counsel finds Ghostlight Ltd (UK video-game publisher, class 9)
forecloses developer-tool use, or a spoken "gaslight" mishearing test fails,
switch to Genkan.

## Consequences

- Positive: a delight-first family umbrella with a clean namespace
  (crates.io/npm free, ghostlight.io and .sh registrable at decision time),
  distinct from the crowded "browser MCP" identity.
- Positive: the brand carries both delight and governance in one metaphor, so
  positioning does not have to choose between them.
- Negative: an English theatrical name sits slightly outside the org's
  Japanese/Zen register; mitigated by a one-line README bridge.
- Action required before public launch: trademark counsel opinion on Ghostlight
  Ltd; register ghostlight.io, ghostlight.sh, crates.io/ghostlight,
  npm/ghostlight; claim crates.io/genkan as the reserve; run a say-it-aloud
  mishearing check.
- Follow-up: the rename touches package names, installer ids, extension name,
  and docs; sequence it as its own change after the current doc and prompt work
  settles, and update the sacred-schema fidelity test only where names are
  cosmetic (tool schemas themselves never change).

## Amendment (2026-07-04): product name simplified to "Ghostlight"

Status: Accepted. Supersedes the product-naming portion of the Decision above;
the family brand, metaphor, crate/binary/server-id, and namespace choices are
unchanged.

The first product's name is simplified from "Ghostlight Browser" to just
**Ghostlight**. "Ghostlight Browser" read like a standalone web browser (the
Brave / Arc / Tor class), which this is not: it is a governed automation engine
plus a thin browser adapter, driven by an MCP client. Collapsing the product
name into the family name removes that misread and matches the `ghostlight`
crate, binary, and MCP server id already in use.

The browser-side extension keeps a distinct, descriptive name, **"Ghostlight in
Browser"**. It echoes "Claude in Chrome" (the product this is a clean-room
rewrite of) and reads as "Ghostlight, in the browser" rather than as the name of
a browser. That string is the extension's manifest `name` and its Chrome Web
Store listing title; it is not the product or project name.

Scope of this amendment:
- Product and project name in prose (README, CLAUDE.md, script synopses) becomes
  "Ghostlight".
- The extension manifest `name` and the store listing stay "Ghostlight in
  Browser".
- The crate, binary, MCP server id (`ghostlight`), org namespace, and file paths
  are unchanged, so there is no code, installer-id, or schema change.
- Historical records that quote the old product name (this ADR's Decision
  section above, prior dated log entries) are left as-is; they document what was
  decided when.
