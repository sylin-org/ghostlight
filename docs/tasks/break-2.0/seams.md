# Break 2.0: seams ledger

The seams lane carries the 2.0 decisions of [ADR-0199](../../adr/0199-one-service-thin-shells.md)
in its Migration order: the removals, the catalog golden, policy schema 4, audit v2, the
refresher, private files on Linux, the protocol-4 trims, relay protocol 4, the edge and the
carriers, the journeys, the pristine pass, the guards and the version, and the current-state
documents. The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) plus the gate
below. The switch ledger's remaining slices ([switch.md](switch.md), switch-8 onward) continue
here. The wire is [adapter-protocol-4](../../design/adapter-protocol-4.md); ADR-0199 wins over
ADR-0197 where they differ.

Gate, from the repo root on `dev`, in Git Bash, with `CARGO_TARGET_DIR=.target-lane-seams`:

- `cargo fmt --check` (fix only with `cargo fmt -p <package>` for each package a slice changed,
  never a bare `cargo fmt`), `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- A Linux compile check of frozen code: `cargo clippy -p ghostlight-relay --target
  x86_64-unknown-linux-gnu --all-targets -- -D warnings` when `crates/relay` changed and, from
  slice f08a on, the same for `-p ghostlight` when `crates/edge` changed.
- `npm test` in `extension/` when `extension/` or `tests/golden/` changed.
- `node --test "crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/tests/*.test.js"`
  when page assets changed.
- `node --test tests/guards/` when `crates/relay`, `crates/edge`, a connector crate, any
  `Cargo.toml`, `Cargo.lock` or `tests/golden` changed.
- `npm test --prefix packaging/npm` and `node --test packaging/mcpb/test/launcher.test.js` when
  `packaging/` changed.
- `sh -n` on every changed `scripts/*.sh`, a parse-only check of every changed `.ps1`, and
  `node --check` on every changed `.js` and `.mjs` file, journeys included.
- `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.

## RESUME HERE

Next: f01a-flatpak. It starts only after shell-spike-1 has committed.

## Landed

| Slice | What | Commit |
|---|---|---|
| f00-adrs | ADR-0199 is filed and indexed with Decisions 1 to 23 and the owner's 2026-10-15 rulings (2.0 waits for the frozen set only, the shell ships in the service's build and is not frozen, the extension opens its setup page only when the service is absent, one-shot commands in 2.0). ADR-0198 gains its dispositions amendment. The ADR index gains the 0199 row and the status notes of the ADRs 0199 touches. This ledger starts, and switch.md points here. | see `git log` |

## Owed live checks

None yet. The coordinator runs every live check a slice names and records it here; implementers
never run them.

## Notes

- Owner question Q1 (the install-time page), answered 2026-10-15 by the coordinator under the
  owner-approved summary of blueprint decision 3: the frozen extension opens its own bundled
  `setup.html` (`chrome.runtime.getURL('setup.html')`), not the sylin.org post-install page, when
  its first connection finds the service absent. sylin.org stays for documentation and the setup
  page links to it. ADR-0199 Decision 23 records the bundled branch only; f06e applies it.
- Owner question Q2, answered the same day: the setup page shows the get-script one-liners
  (Windows: `irm <main>/scripts/get.ps1 | iex`; Linux: the `get.sh` line), with no Node.js
  requirement. The install journey's ADR records it.
- Where ADR-0199's draft differed from the approved text, the ADR as filed wins: the edge has a
  carrier check and three rules and knows no verb, and 2.0.0 waits for the frozen set only.
