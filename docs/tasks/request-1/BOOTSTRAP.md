# request-1: ground rules

This batch carries out [ADR-0202](../../adr/0202-what-a-request-carries.md), "What a request
carries", which the coordinator decided on 2026-10-18 under the owner's delegation ("you're
authorized to evaluate the proposals and decide. I want this release to be an opportunity to
streamline architecture aspects like this one."). The ADR's "Rebuild plan" is the specification:
each slice here is one of its slices, S1 to S11. [LEDGER.md](LEDGER.md) is the record.

## Lanes

- **connection**: S1 to S4 (the connection's evidence, the channel word, the agent's name, the
  workspace's opener).
- **request**: S5 to S9 (one request id, the request owns itself, findings per frame, a flow step's
  own facts, the cancel cause). S5 waits for S2.
- **window**: S10 (a watcher's pause says what is true).
- **governance**: S11 (the channel switches). It waits for S6.

## Rules

- AGENTS.md and docs/MEMORY.md bind; the ADR decides the design. Where the ADR and the tree
  disagree on a line number, find the code by name.
- Delete what a slice makes redundant, with no shims (ADR-0197 D24). Every slice leaves a green
  tree and records its row in LEDGER.md in the same commit.
- Each lane builds in its own `CARGO_TARGET_DIR` (`.target-lane-rb-<lane>`) and stages only its own
  paths. Linux code is first compiled by CI's ubuntu clippy: gate system-only calls and read such
  code twice.
- No journey, installer, dev-loop or Ghostlight binary runs in a lane: the owner's machine holds one
  live Ghostlight. The coordinator deploys in place, checks live and pushes.
