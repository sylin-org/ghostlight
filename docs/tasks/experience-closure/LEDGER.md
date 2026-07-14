# Experience closure batch: LEDGER

Durable progress for ADR-0079 and the July 2026 non-author experience review.

## RESUME HERE

E1-E5 are complete in the repository. Push `dev`, then resume only for visible Linux/browser
verification or owner-approved public metadata/funding work.

## Work

### E1: denial attention circuit -- COMPLETE

- Add the per-session service state machine, synchronized browser dispatch gate, explicit MCP
  outcome, attention audit record, extension control wire, and deterministic tests.
- Thresholds are pinned at 3 matching denials in 60 seconds or 5 session denials in 120 seconds.
  The synchronized final send boundary blocks only the producing session. Teardown clears state.
- The popup and closed-shadow page overlay relay keep, resume, resume-and-quiet, and end-session
  dispositions. Audit records contain bounded decision facts and no page payload or session guid.

### E2: cohesive visual feedback -- COMPLETE

- Replace the persistent denial ribbon with a transient sticker and burst overlay.
- Compact narration and remove the progress bar.
- Add truthful screenshot and recording cues.
- Isolated stickers expire after three seconds or replacement. Compact narration uses one
  three-dot cue. Screenshot and REC signals track actual capture lifecycle.

### E3: developer-first entry -- COMPLETE

- Strengthen the README's practitioner opening, installation visualization, no-account story, and
  factual licensing reassurance.
- Added an immediate install anchor, four-stage journey, explicit pre-release extension path,
  read-only proof prompt, and exact core/governance license explanation. The late reviewer note
  naming OpenCode is recorded as evidence for fast repository orientation, not copied blindly.

### E4: agent-browser comparison -- COMPLETE

- Reverify the current upstream surface and publish a 1:1 overlap map with model, user, and
  governance opportunities.
- Rebased against official agent-browser v0.31.2 docs on 2026-07-14. The mutual table separates
  ordinary overlap, deliberate live-context exclusions, specialist testing complements, and two
  measured free-surface candidates: ref-linked screenshot annotations and owned-tab labels.

### E5: closure -- COMPLETE

- Run all gates, update durable project state, record external verification debt, commit logical
  changes, and push `dev`.
- Green so far: `cargo fmt --check`; `cargo test --workspace` (full fast suite);
  `cargo clippy --workspace --all-targets -- -D warnings`; all 93 extension tests; `node --check`
  across extension root and library scripts; `git diff --check`.

## Commits

- `c6f2be5` -- `feat(governance): pause repeated denied browser actions`
- `acae0cd` -- `docs(onboarding): foreground the developer install journey`
- `9d145d8` -- `docs(comparison): map agent-browser capability overlap`
- The final status/ledger commit closes this batch.

## External gates

- Live Linux and browser verification: waiting for the owner-provided machine and SSH access.
- Chrome Web Store and product-site publication: draftable here, publication remains owner gated.
- Donations/funding: requires the owner's recipient, entity, tax, and provider decision.
- GitHub repository description, homepage, and topics: owner-approved next-work item, but changing
  public repository metadata remains a separate outward-facing action.
