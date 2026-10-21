# arch-1: ground rules

This batch carries out [ADR-0203](../../adr/0203-one-owner-for-each-fact.md), "One owner for each
fact". The coordinator decided it on 2026-10-19 under the owner's delegation of 2026-10-18
("you're authorized to evaluate the proposals and decide. I want this release to be an
opportunity to streamline architecture aspects like this one.") and the owner's leave of
2026-10-19 ("You're authorized to proceed autonomously."). The ADR's "Rebuild plan" is the
specification: each slice here is one of its slices, and each lane's ledger lists them in the
ADR's order. [findings.md](findings.md) maps every finding of the sweep to its decision and slice.

Ledgers: [docs](LEDGER-docs.md), [core](LEDGER-core.md), [adapter](LEDGER-adapter.md),
[policy](LEDGER-policy.md), [surface](LEDGER-surface.md), [install](LEDGER-install.md) and
[package](LEDGER-package.md).

## Gates

- **candidate**: the slice is in the release candidate the cutover uses. It fixes a harm a person
  or agent meets in 2.0, makes a public claim true, changes a seam whose later change costs
  compatibility, or acts once at the 1.x upgrade.
- **none**: the slice lands on `dev` when ready, before or after the candidate, and never holds it.

The coordinator may re-order, split or re-gate a slice in its ledger under this rule without
amending the ADR. Only a changed decision needs an amendment.

## Lanes

Each file a slice edits belongs to exactly one lane. Paths without a crate are under
`crates/orchestrator/src`.

- **docs**: `docs/**`, `README.md`, `CHANGELOG.md` and the release notes under `docs/release/`.
  Slices D0, D1 and D2.
- **core**: `work/**`, `workspace/**`, `service/**`, `browser/` outside `browser/adapter/`,
  `presentation/**`, `intake/` apart from `intake/desktop.rs`, `audit.rs`, `audit/**`,
  `events.rs`, `provenance.rs`, `diagnostics.rs`, and `language/` apart from the modules other
  lanes own (`policy.rs`, `deployment.rs`, `workbench.rs`, `setup.rs` and `readiness.rs`).
  `language/mod.rs` is core's. Core also owns the catalog, result and audit goldens, every journey
  in `tests/` (process, installer and live browser, `*.mjs` and `*.ps1`), `tests/lib/**` and
  `tests/guards/**`. The C slices.
- **adapter**: `browser/adapter/**`, its page assets, and `tests/golden/protocol4/**`. Slices A1 to
  A4.
- **policy**: `governance/` apart from `governance/managed/cli.rs`, and `language/policy.rs`.
  Slices P1 to P10.
- **surface**: `crates/orchestrator/ui/**`, `workbench/**`, `intake/desktop.rs`,
  `language/workbench.rs`, `language/setup.rs`, `language/readiness.rs`, `crates/desktop/**`,
  `tests/golden/desktop/**` and the window's node tests (`tests/workbench-*.test.mjs`). Slices S1
  to S8.
- **install**: `install/**`, `cli/**`, `agent/**`, `governance/managed/cli.rs`,
  `language/deployment.rs`, `crates/bridge/**`, `crates/win-peer/**`, `packaging/**` (the npm and
  MCPB launchers and the plugin), `scripts/get.ps1`, `scripts/get.sh` and `tests/golden/agents/**`.
  Slices I1 to I13.
- **package**: `extension/**` and `tests/golden/extension-2.0.0.files.sha256`. Slices B1 and B2.

No slice edits `crates/relay` or `crates/edge` (ADR-0203 Decision 13).

Edits the ADR already names as cross-lane: C16 (`protocol4/link.rs`, `workbench/tests.rs`), C17
(`translate/observe.rs`), C18 (`protocol4/golden.rs`, `page-shared.js`, `presentation.js`), C21
(`protocol4/wire.rs` and its fixtures), C22 (`agent/mod.rs`, `workbench/tests.rs`), A4 (work's
`DOCUMENT_TREE_NODE_LIMIT`), P6 (core's `GovernanceFacade::new` call sites, surface's
`owning_user_policy` tests), P8 (`language/mod.rs`), I8 (the installer journeys) and I10
(`language/outcome.rs`).

## Rules

**ux-2 first, by file.** A slice that edits a file any ux-2 commit touched starts after ux-2's
ledger closes (u7, the j-32 tooltip, the integration gate and the push). ux-2 closed on
2026-10-19 at `7cf3b39e`, so this rule is met for every slice. At `0fc62c90` ux-2 had touched:

- core: `events.rs`, `provenance.rs`, `presentation/mod.rs`, `service/mod.rs`,
  `language/audit.rs`, `language/channel_switch.rs`, `language/history.rs`, `language/mod.rs`,
  `language/outcome.rs`, `language/page_refusal.rs`, `language/provenance.rs`, `work/forms.rs`,
  `work/mod.rs`, `work/pointer.rs`, and `work/tests/` `availability.rs`, `control.rs`,
  `execution.rs`, `presentation.rs` and `protocol4.rs`; the journeys
  `tests/workbench-history-browser.mjs`, `tests/workbench-preview-server.mjs`,
  `tests/workbench-setup-surface.mjs` and `tests/workbench-surface.mjs`; the guard
  `tests/guards/extension-freeze.test.mjs`.
- adapter: `protocol4/assets/presentation.js`, `protocol4/assets/presentation-css.js` and
  `tests/golden/protocol4/assets.sha256`.
- policy: `governance/effective.rs`, `governance/evidence.rs`, `governance/mod.rs` and
  `governance/tests.rs`.
- surface: `intake/desktop.rs`, `language/readiness.rs`, `language/setup.rs`,
  `language/workbench.rs`, `workbench/` `context.rs`, `history.rs`, `live.rs`, `mod.rs`,
  `parts.rs`, `parts/tests.rs`, `setup.rs`, `surface_tests.rs` and `tests.rs`; `ui/` `app.js`,
  `index.html`, `kit/kit.css`, `kit/settings.js`, `lib/entries.js`, `lib/setup-view.js`,
  `lib/store.js`, `lib/view.js`, `lib/words.js` and `styles.css`; `crates/desktop/src/tray.rs` and
  its tray assets; `tests/workbench-preview.test.mjs`, `tests/workbench-settings.test.mjs` and
  `tests/workbench-store.test.mjs`.
- install: `install/mod.rs` and `agent/mod.rs`.
- package: `extension/kit.css`, `extension/lib/popup-view.js`, `extension/options.css`,
  `extension/popup.html`, `extension/tests/pages.test.js`, `extension/tests/popup-view.test.js`,
  and `tests/golden/extension-2.0.0.files.sha256`.
- docs: `docs/1.0/ACCEPTANCE.md`, `ARCHITECTURE.md`, `INTENT.md` and `LANGUAGE.md`;
  `docs/design/README.md`, `tool-visual-signatures.md`, `visual-feedback-dictionary.html`,
  `visual-feedback.md` and `visual-language.md`; `docs/guides/governance-configuration.md`;
  `docs/tasks/ux-2/**`.

So core, policy, surface and package wait. Adapter's A1 to A3 start at once, and A4 waits because
it regenerates `assets.sha256`. In docs, D0's ADR and task files start at once; D1 and D2 wait,
because they edit `docs/1.0/`. In install, I3, I4, I6 and I10 wait for `install/mod.rs`; the other
install slices start at once unless a fresh look at the list says otherwise.

**Cross-lane changes.** A slice that removes a field, changes a signature or trait method, or
deletes a name another lane's file uses, first greps the name across `crates/`. It edits those uses
in the same commit, and lands only when that lane has no open slice in those files.

**Journeys.** Lanes run the cargo and node tiers, and a slice writes or extends the journeys it
needs. The coordinator has every journey that starts its own service (process, installer, live
browser) run on test-01, test-02, test-03 or leo-desktop-02, or in CI after the push, from a fresh
build named by `GHOSTLIGHT_BIN_DIR`. On the owner's machine only unit tests, the headless
workbench-history journey (`GHOSTLIGHT_TEST_HEADLESS=1 node tests/workbench-history-browser.mjs`)
and the in-place deploy through `scripts/dev-loop.ps1` run (docs/MEMORY.md, One live Ghostlight).

## One checkout

- AGENTS.md and docs/MEMORY.md bind; the ADR decides the design. Line numbers in the ADR are hints:
  find the code by name.
- Delete what a slice makes redundant, with no shims (ADR-0197 D24). Every slice leaves a green
  tree.
- Each lane builds in its own `CARGO_TARGET_DIR`, `.target-lane-arch-<lane>`.
- Stage by path (`git add -- <paths>`, `git commit -s ... -- <paths>`). Never stash, never amend,
  never `git add -A`.
- The slice's ledger row changes in the slice's own commit.
- Nothing is deployed, released or published by a lane. The coordinator integrates at each lane's
  close, has the journeys run as above, deploys in place, checks live and pushes.

## The second sweep (2026-10-20)

The [ADR-0203 amendment of 2026-10-20](../../adr/0203-one-owner-for-each-fact.md#amendment-2026-10-20-the-second-sweep)
decides it, and [the second sweep](sweep-2026-10-20/README.md) holds its findings, their verdicts
and the defects that go first. The first sweep's open slices are part of it: each lane takes the
ones that share its files before the new findings in those files.

### Lanes and machines

Core is split in three for the second sweep; every other lane keeps its files.

| Lane | Files | Machine | Ledger |
| --- | --- | --- | --- |
| core-language | `language/` apart from `policy.rs`, `deployment.rs`, `workbench.rs`, `setup.rs` and `readiness.rs`; the catalog and result goldens | test-01 (CachyOS) | [LEDGER-core-language.md](LEDGER-core-language.md) |
| core-work | `work/**`, `workspace/**` | test-03 (Alpine) | [LEDGER-core-work.md](LEDGER-core-work.md) |
| core-service | `service/**`, `intake/` apart from `intake/desktop.rs`, `browser/` outside `browser/adapter/`, `presentation/**`, `audit.rs`, `audit/**`, `events.rs`, `provenance.rs`, `diagnostics.rs`, `lib.rs`, `main.rs`, the journeys in `tests/`, `tests/lib/**` and `tests/guards/**` | the coordinator, on the owner's machine | [LEDGER-core-service.md](LEDGER-core-service.md) |
| adapter | as above | test-02 (Bluefin) | [LEDGER-adapter.md](LEDGER-adapter.md) |
| package | as above | test-02, after adapter | [LEDGER-package.md](LEDGER-package.md) |
| policy | as above | test-03, after core-work | [LEDGER-policy.md](LEDGER-policy.md) |
| install | as above, `cli/**` included | leo-desktop-02 | [LEDGER-install.md](LEDGER-install.md) |
| surface | as above | leo-desktop-02, after install | [LEDGER-surface.md](LEDGER-surface.md) |
| docs | as above | the coordinator | [LEDGER-docs.md](LEDGER-docs.md) |

[LEDGER-core.md](LEDGER-core.md) keeps the first sweep's record; its open slices moved to the
three core ledgers.

### Five checkouts

The first sweep ran in one checkout and the coordinator pushed. The second runs on five machines,
each in its own checkout of `dev`, so lanes push their own slices:

- After a slice, run the checks AGENTS.md names for what it changed, commit by path with `-s` and
  the ledger row, `git pull --rebase origin dev`, run the touched crates' tests again when the
  rebase brought commits in them, and push. A rebase conflict in another lane's file: stop and tell
  the coordinator.
- CI runs on every push, and a red run a slice caused is its lane's next task. Linux cannot compile
  Windows-only code, nor Windows the Linux-only code: a slice that edits such code says so and its
  lane watches the CI run, or the code goes to the lane on the matching system.
- Journeys that start a service run on the lane's machine from a fresh build named by
  `GHOSTLIGHT_BIN_DIR`, never on the owner's machine.
- Live checks: a lane deploys its machine's build in place and runs an agent task (the desk bell)
  after each defect fix and at each lane milestone. The coordinator deploys `dev` in place on the
  owner's machine at every lane close and before the candidate, and checks it live. A push to `dev`
  reaches no user, and the candidate is deployed and checked on the owner's machine and on all four
  test machines before anything is released. That is how this batch keeps docs/MEMORY.md's "Test
  locally before anything leaves the machine" with five checkouts.
- Build in `.target-lane-arch-<lane>`. The rest of "One checkout" above still binds: stage by
  path, never stash, amend or `git add -A`, the ledger row in the slice's commit, no shims.

### Order across lanes

- C17 (one wait vocabulary) is core-language's first refactor. core-work and adapter leave the
  wait uses alone until it lands.
- C12b's language half lands in core-language before core-work's half and WORK-2, WORK-3 and
  WORK-4, which extend its constructor.
- LANG-3's closed vocabularies are defined in core-language with their uses edited in the same
  commit, under the cross-lane rule above. BROW-3's adapter half follows in adapter.
- LANG-2's `HandleKind` is defined in core-language. core-work adopts it in `workspace/`.
- SERV-7's poison-tolerant `lock` becomes one crate helper in core-service's first refactor. Each
  lane then replaces its own copies.
- The package lane ends with the freeze golden regenerated and the extension ready for one new
  2.0.1 submission. Cancelling the staged review and submitting are the owner's go.
- Surface's S3 wires INST-15's `resymbol`. Docs takes the lines other lanes owe, as D2 does.
