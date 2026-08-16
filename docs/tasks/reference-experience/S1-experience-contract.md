# S1: experience contract

## Objective

Decide the vocabulary, defaults, and measures this epic depends on, record them as one ADR, and
append the decided values to [PINS.md](PINS.md). Change no product behavior. Every later stage cites
this stage's output instead of re-deciding it.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- `docs/1.0/INTENT.md`, `LANGUAGE.md`, `ARCHITECTURE.md`, `ACCEPTANCE.md`.
- ADR-0028 (never phone home), ADR-0070 (bidirectional install handoff), ADR-0082 (Linux user
  session discovery), ADR-0102 (integrated desktop workbench), ADR-0103 (language-owned outcome
  voice), ADR-0104 (demand start and workbench activation), ADR-0112 (one minimized desktop
  startup), ADR-0113 (browser adapter liveness), ADR-0114 (plural browser adapters), ADR-0116
  (Windows and Linux platform scope), ADR-0118 (recoverable Linux workbench startup), ADR-0119
  (durable authority, disposable workbench), ADR-0122 (readable policy and authored user layer),
  ADR-0123 (lean Linux install and visible activation), ADR-0125 (plural Linux harness
  integrations).
- `docs/research/25-delightful-linux-experience-2026-08.md`.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- The live runtime-control vocabulary is `Active`, `Held`, `Attention`, `Ended`, with intents
  `ToggleHold`, `Hold`, `Resume`, `EndSession`, `StartSession`. A hold denies at the final boundary;
  it does not hold a caller. See `PINS.md` for exact locations.
- `docs/1.0/INTENT.md:76` says three destinations reach every workbench surface. The window has four
  tabs today: Monitor, Status, Policy, About. This drift is yours to correct.
- The policy manifest already owns registered settings with organization ceilings, and
  `policy.user.enabled` is the precedent for an operational control living there.
- No man pages, completions, PATH entry, WSL detection, or autostart behavior exists.

## Required output

1. One ADR, next free number, titled for the reference experience contract. It records:
   - the five audiences and the rule that platform behavior is a table with a row per platform, so
     macOS is a later row rather than a rewrite;
   - progressive reveal ordered as returned sentence, then CLI, then workbench;
   - adaptive familiarity as a rule, with the closed desktop set from `PINS.md`;
   - the mapping between the live `Active`/`Held`/`Attention`/`Ended` vocabulary and the
     user-visible words, including what happens to `StartSession`;
   - whether a hold keeps the caller pending or continues to refuse, what a caller timeout means,
     and how a held operation interacts with the ADR-0113 deadline and quarantine;
   - whether a held state survives workbench close, browser reconnect, harness reconnect, and
     orchestrator restart;
   - the browser-startup preference: its name, owner, default, and whether it is a registered policy
     setting or a separate closed choice, decided per platform;
   - whether the per-user install owns `~/.local/bin/ghostlight` or reports the absolute path;
   - whether At a glance replaces Monitor or becomes an additional destination;
   - acceptance thresholds for first use, automatic recovery, truthful stop, diagnostic usefulness,
     keyboard use, and comprehension.
2. Corrections to `docs/1.0/INTENT.md` and any other active 1.0 contract statement that no longer
   matches the tree. Historical ADRs stay immutable.
3. Appended values in `PINS.md`, under a new heading, covering every item listed as not pinned
   there. Append only. Do not edit an existing pin.
4. A list in `LEDGER.md` of presentation this epic will make redundant, so later stages remove it
   rather than leaving two of everything.

## Verification

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension

No behavior changes, so these confirm the tree is still green rather than proving new work. Also
confirm every relative link in changed documents resolves.

## Out of scope

Any source change beyond documentation. Any new preference implementation. Any UI work. Renaming a
destination. Touching `docs/trust/` or `docs/legal/`.

## STOP preconditions

- A proposed state requires Ghostlight to infer the user's task-level intent.
- A proposed preference cannot be expressed as a small closed choice with one owner.
- Pause or stop cannot be defined consistently across plural sessions and operations.
- A decision would settle protocol or lifecycle behavior that an ADR you have not read owns.
- The live vocabulary in the tree no longer matches what `PINS.md` records.
