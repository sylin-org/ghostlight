# Public delight 0.8: BOOTSTRAP

Execution package for ADR-0100. A fresh session can start here with no conversational context.
The goal is one warm, truthful, outcome-led public experience for Ghostlight 0.8 across the product
repository, website, agent guidance, and discovery surfaces.

## Start here

1. Read `AGENTS.md`, `docs/MEMORY.md`, and `docs/STATUS.md` in the product repository.
2. Read `docs/adr/0100-public-documentation-delight.md` completely.
3. Read this file and `LEDGER.md`. Resume only from `RESUME HERE`.
4. Before changing the website repository, read its own `AGENTS.md` and current build instructions.
5. Re-read live files and external surfaces before every pass. Version, review, score, and listing
   state are time-sensitive.

## Authority

Conflicts resolve in this order:

1. The live tree and accepted ADRs.
2. ADR-0100, with ADR-0094 for tool guidance and ADR-0093 for adapter compatibility.
3. `docs/public-status.json`, `CHANGELOG.md`, compatibility manifests, and the live tool registry
   for their owned facts.
4. This bootstrap and the task file for the active pass.
5. The ledger.

Earlier design and research documents are evidence, not current truth. Do not copy a stale version,
topology, store state, or capability claim from them.

## Delight contract

Review every public surface against four questions:

1. Can the right reader recognize the product and its use within roughly 15 seconds?
2. Can the reader decide whether it fits within roughly 2 minutes?
3. Can a supported user reach one safe useful result within roughly 5 minutes?
4. If something fails, do the user and agent receive a concrete next action?

These are editorial targets. Do not add timers, analytics, telemetry, or vendor-bound measurement.

## Invariants

- User delight comes first. Prose is warm, inviting, calm, and specific.
- Lead with useful work in the user's signed-in browser, then visibility and control, then local
  ownership, then optional governance.
- Do not make personal use sound incomplete or governance sound punitive.
- The trained tool compatibility signature is fixed: names, parameter names, types, enums,
  ordering, required fields, and structural contracts do not change in this batch.
- `computer` keeps its Claude-in-Chrome-compatible signature regardless of an external B grade.
- Descriptions, examples, titles, standard annotations, output guidance, and external metadata may
  improve when the change is truthful and helps purpose, choice, side effects, or recovery.
- No runtime behavior change is justified by a directory score. If research finds a real runtime
  defect, record it separately and stop before expanding this batch.
- Keep the extension policy-free and preserve the no-phone-home Continuity Promise.
- Distribution evidence is not reception evidence. Project-authored posts are not testimonials.
- External figures are dated snapshots and always carry their source.
- No invented quote, aggregate user count, active-user claim, certification, or comparative
  superlative.
- Public and store installation remains store-only for end users.
- ASCII only in both repositories.
- Use the fewest meaningful moving parts. Add no CMS, analytics product, capability database,
  badge system, or parallel release summary.
- Preserve unrelated dirty work in both repositories.

## Repository boundaries

The product repository is `sylin-org/ghostlight`. The website is `sylin-org/website`, an Eleventy
site. `scripts/publish-website.ps1` documents the existing synchronization boundary and must remain
the source of truth for release-status and install-guide fallbacks.

Website content work is authorized. Prepare it on a non-publishing branch unless the owner gives a
separate instruction to deploy. Do not push or merge a branch that triggers a public site build
without explicit owner confirmation.

Store resubmission, directory submission or edits, release tags, registry publication, social
posts, discussion posts, and website deployment are also explicit owner gates. Draft and verify
them, update the ledger, then stop.

## Work sequence

| Pass | Outcome |
| --- | --- |
| E1 | Current capability, public-truth, discovery, and reception baseline |
| E2 | One canonical message architecture and evidence-linked copy kit |
| E3 | Concise product-repository front doors and first-success path |
| E4 | Rich guidance and metadata for all 25 tools without signature changes |
| E5 | Warm website experience plus synchronized package and directory drafts |
| E6 | Full reconciliation, release-ready publication packet, and local reception loop |

Each pass has its own task file. Complete and commit one pass before beginning the next. Every
prefix must leave each touched repository coherent. Update `LEDGER.md` after every pass, including
the commit hash and any deviation.

## Shared voice

- Write to one person trying to get useful work done.
- Prefer direct verbs and recognizable outcomes over category jargon.
- Prefer one concrete example over a superlative.
- Explain limitations without apology theater.
- Recommend another approach when it fits better.
- Keep compatibility dates exact: `2025-11-25` and `2026-07-28`.
- Use `Ghostlight MCP` or `Ghostlight browser automation` in search metadata where qualification
  helps discovery. The product name remains Ghostlight.
- Do not use mascot voice, urgency, fake scarcity, or exclamation-heavy copy.

## Shared verification

Run the checks relevant to every changed surface. At minimum:

1. `git diff --check` in each touched repository.
2. Scan changed text for non-ASCII characters.
3. Validate every changed relative link and every external link used as claim evidence.
4. Run `pwsh -File scripts/check-public-surfaces.ps1` after product public-state changes.
5. Run the website repository's own formatting, link, and build checks after site changes.
6. Run the full ADR-0094 tool-definition gates during E4: formatting, strict Clippy, workspace
   tests, and `tests/tool_schema_fidelity.rs` through the normal test suite in an isolated target.
7. Compare the identity projection of every trained schema before and after E4. Only guidance and
   metadata differences are allowed.

Do not run `check-public-surfaces.ps1 -Online` as proof of an unpublished candidate. Use it only
after the owner-authorized deployment step and label its timestamp.

## Stop conditions

Stop and record the exact blocker if:

- current source, public status, and an external surface disagree in a way that cannot be resolved
  from authoritative evidence;
- a proposed copy claim lacks source or live proof;
- a tool improvement would require a signature or runtime change;
- the website's branch or deployment behavior is unclear;
- a store edit would reset the pending 0.8 review without an explicit owner choice;
- a requested external post, push, merge, submission, or publication lacks owner confirmation.
