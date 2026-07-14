# BOOTSTRAP: experience-closure batch

Implements ADR-0079 and the actionable parts of the July 2026 non-author experience review. The
batch also closes the developer-first entry work and refreshes the agent-browser comparison.

## Authority

1. Live tree and tests.
2. ADR-0079, plus ADR-0072 and ADR-0073 as amended by this batch.
3. `docs/design/non-author-experience-review-2026-07.md` and its linked design/research notes.
4. This batch ledger.

## Invariants

- The 13 trained schemas stay byte-stable.
- The extension remains policy-free. It renders service-owned state and relays human choices.
- Attention state is per MCP session. Never reuse the global take-the-wheel hold.
- Quieting presentation never changes an authorization result.
- No page content, full URL, screenshot, or form value enters attention audit records.
- No telemetry, account requirement, remote browser, headless browser, or persistent attention
  state is introduced.
- ASCII only. New governance source uses `LicenseRef-Ghostlight-Commercial`; other new source uses
  `Apache-2.0 OR MIT`.

## Work sequence

| Work | Outcome |
|---|---|
| E1 | Decision record, attention state machine, transport gate, audit, and orchestration |
| E2 | Sticker, pause overlay, popup recovery, narration, screenshot, and recording feedback |
| E3 | Developer-first README and installation path |
| E4 | Current agent-browser capability map and strategic recommendations |
| E5 | Full gates, live-test debt record, project memory/status, commits, and push |

## Gates

Use an isolated `CARGO_TARGET_DIR` for every Rust gate:

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `node --test` under `extension/`
5. `node --check` for every changed extension JavaScript file
6. Architecture, schema-fidelity, and documentation-link checks remain green.

Live Linux and browser verification is recorded as external debt until the owner supplies the
machine and SSH access. Store publication, product-site publication, and funding activation remain
owner-controlled external actions.
