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

Run `rustup target add x86_64-unknown-linux-gnu` once before the first Linux compile check. The
coordinator's host has only the Windows target and CI runs only after the owner pushes.

Must not edit: `CHANGELOG.md` and `docs/STATUS.md` (the coordinator owns them);
`crates/desktop/**` and any spike directory the desktop shell track names; `server.json` (it
tracks the published release); `coordination/**`; `dist/**`; `site/**`; `open-spec/**`;
`reference/**` (clean-room study material, read-only); `local/`, `private/` and `saps/` (never
read them either); the untracked notes `docs/testing/independent-user-delight-2026-10-02.md` and
`docs/testing/quiet-success-local-deployment-2026-10-02.md` (never stage them).

Rules:

1. Before each slice read `AGENTS.md`, the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md), this
   ledger and every ADR the slice names. ADR-0199 wins over any blueprint wording a spec quotes.
2. Never run any Ghostlight binary, journey, install, carrier, `deployment install` or
   `scripts/dev-loop.ps1`. The coordinator runs every live check a spec names; record each under
   Owed live checks. `cargo test` may start ordinary OS programs (`ping`, `sleep`) where a spec
   says so. Read-only check scripts (`scripts/adapter-compatibility.ps1`,
   `scripts/check-public-surfaces.ps1`) may run.
3. One slice is one commit unless the spec names a short run, with the conventional-commit subject
   given. Stage and commit only your own paths: `git add -- <paths>`, then `git commit -s -m
   '<subject>' -m '<body>' -m 'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>' --
   <paths>`, so another lane's staged files stay out. Retry while `.git/index.lock` exists. Never
   `git add -A`, `git add .`, `git commit -a`, stash, reset, `checkout --`, clean, rebase, push or
   tag.
4. Update this ledger in the same commit: a Landed row, RESUME HERE (the next slice key), owed
   live checks, and every place the tree differed from the spec.
5. Line numbers in specs drift: re-find code by symbol. When the tree contradicts a spec, follow
   the tree, keep the spec's intent and record the difference here. When the intent cannot be
   kept, stop and record a blocker here instead of improvising.
6. Code style per `AGENTS.md`: ASCII only; a module doc comment on every module and a doc comment
   on every public item; no magic strings (closed vocabularies are enums or named consts beside
   their use); every model-facing or person-facing sentence lives in
   `crates/orchestrator/src/language/`; no comments that narrate history; lint suppressions are
   `#[expect(lint, reason = "...")]`; prefer the root fix.
7. Tests are behavioral: they drive real functions with fakes or temp homes and never pin source
   text. New unit tests of `crates/relay` and `crates/edge` go in `src/<module>/tests.rs` files
   (declared `#[cfg(test)] mod tests;`) or under `crates/<crate>/tests/`, never inline in a src
   file, so the frozen source lock (slice f12a) can exclude them. Existing inline test modules
   move when their module is rewritten, and slice f12a moves the rest. A test that needs a home
   uses `GHOSTLIGHT_TEST_HOME` or a temp directory, never the real `~/.ghostlight`.
8. Until slice f12a2-asset-pin drops the pin, a page-asset change refreshes
   `tests/golden/protocol4/assets.sha256` with the id the failing `assets/tests/world.test.js`
   prints.
9. Commit `Cargo.lock` with every dependency change. When a relay, connector or edge dependency
   changes, regenerate `tests/golden/cargo-tree` with `GHOSTLIGHT_GUARD_UPDATE=1 node --test
   tests/guards/` and say why in the commit body.
10. Deleting a tracked file listed in `docs/repository-integrity-baseline.json` also removes its
    entry.
11. Product identity is inherited: never change icons, the native host name
    `org.sylin.ghostlight`, the home `~/.ghostlight` or public wording beyond what a spec says.
    Docs follow `docs/MEMORY.md` ("Delight is clarity").
12. Root manifest rule: keep `crates/desktop` in `[workspace]` members and the `ghostlight-desktop`
    `[[package]]` block in `Cargo.lock`. Never remove any of its dependencies: `crates/desktop`
    belongs to the shell-spike lane, and its manifest declares crates it uses only in later spike
    slices.
13. The shell-spike lane runs in parallel in the same checkout. A gate failure inside
    `crates/desktop/**` or `docs/tasks/shell-spike/**` (fmt, clippy, tests, the guards, or the
    integrity script's local links) is that lane's work in progress: wait two minutes and retry;
    after 15 minutes record a blocker here and stop. Never fix, format or stage it.
14. Every slice that changes `crates/relay` or `crates/edge` records under Owed live checks:
    `cargo test -p ghostlight-relay` (and, from f08a on, `cargo test -p ghostlight` for the edge)
    on a Linux test machine.
15. Owner question Q1 (the install-time page) is answered in Notes below; f06e applies the
    recorded branch.

Hard order: shell-spike-1 commits before f01a starts (f00 is docs only and may run at the same
time). f01a's precondition checks it.

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
