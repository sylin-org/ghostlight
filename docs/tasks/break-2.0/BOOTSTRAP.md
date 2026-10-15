# Batch break-2.0: ground rules

This batch builds the ADR-0197 break set on `dev`: frozen relay connectors, MCP in the service,
adapter protocol 4, one install root, governance schema 4, the removals, and a pristine tree. The
target is [ADR-0197](../../adr/0197-contained-seams-and-the-2-0-break.md); it wins over anything
here. Protocol 4 is specified in [adapter-protocol-4](../../design/adapter-protocol-4.md).

Three lanes work in one checkout at the same time: service, extension and connectors. A switch
phase follows, run by one agent after the lanes finish. Each lane keeps its own ledger in this
directory.

## Ownership

Status: the service and extension lanes are closed. The switch owns every file (switch.md),
including bridge, win-peer, the manifests and Cargo.lock. Any remaining connectors slice runs as a
switch slice, one at a time, not as a parallel lane. The table records how the lanes split the
work.

No file is edited by two lanes.

| Lane | Owns |
| --- | --- |
| service | `crates/orchestrator/src/**` except the page assets under `browser/adapter/chromium/level4/assets/` and `chromium/assets/`; its ledger. The browser vocabulary it extends is `chromium/level3/mod.rs` |
| extension | New files under `extension/` only; `tests/golden/protocol4/**`; the level 4 page assets; `scripts/lint-js*.mjs`; its ledger |
| connectors | `crates/relay/**`, both connector crates, root `Cargo.toml` and `Cargo.lock`, `tests/golden/relay3/**`, `tests/golden/cargo-tree/**`, `tests/guards/connectors.test.mjs`, `tests/lib/cdp-pipe*.mjs`, `tests/installed-journey.mjs`, `scripts/dev-loop.ps1`, `scripts/get.*`, `packaging/**`, retired-channel scripts and tests, `.github/workflows/release.yml`, `scripts/check-public-surfaces.ps1`, `scripts/release-preflight.ps1`, ADR header notes and README status cells, this file, its ledger |
| nobody until the switch | `crates/bridge/**`, `crates/win-peer/**`, `crates/orchestrator/{Cargo.toml, tauri.conf.json, packaging/**, ui/**}`, `.github/workflows/ci.yml`, `AGENTS.md`, `CONTRIBUTING.md`, other docs, every existing `extension/` file, the `tests/*.mjs` journeys |

Ownership also covers compilation:

- Only the service lane builds `ghostlight`.
- The connectors lane builds only crates already moved onto `ghostlight-relay`.
- The extension lane builds nothing in Rust.
- Nobody edits `win-peer` or `bridge`.
- Only the connectors lane changes manifests. The service lane never passes `--locked` and never
  stages `Cargo.lock`.
- An error inside another lane's file is that lane's work in progress. Wait and retry. Never fix it.

## Shared contracts

| Contract | Owner | Read by others |
| --- | --- | --- |
| Adapter protocol 4 fixtures, `tests/golden/protocol4/**` | extension | the service, in switch-2 |
| Rust wire types, `chromium/level4/wire.rs` and `world.rs` | service | bound to the fixtures in switch-2 |
| Relay protocol 3: `crates/relay` and `tests/golden/relay3/**` | connectors | the service uses the crate itself in switch-1 |
| Failure templates file `~/.ghostlight/relay-templates.json` | the service writes it; the shape is pinned by `tests/golden/relay3/templates.json` | connectors read it at failure time |
| `GHOSTLIGHT_TEST_HOME` and the file names under `~/.ghostlight` | connectors (relay `home.rs`) | the service adopts `relay::home()` in the switch |
| Page asset include marker and order | extension | the Rust builder in switch-2 |

## Commit protocol

- BASE is `git rev-parse HEAD` recorded before the lanes start. Read pre-break sources with
  `git show BASE:<path>`.
- Stage your own paths only: `git add -- <paths>`. Commit only those paths with
  `git commit -s -m "<type>(<scope>): <subject>" -m "<body>" -m "Co-Authored-By: ..." -- <paths>`.
- Never run `git add -A`, `git add .`, `git commit -a`, stash, reset, `checkout --`, clean, rebase
  or push. While `.git/index.lock` exists, retry after a few seconds. (The AGENTS.md note about
  pending edits in stashes records how the 2026-10-05 branch cleanup preserved existing work. It
  does not permit creating stashes here.)
- An error inside a file another agent is changing is that agent's work in progress: wait and
  retry, never fix it.
- Build into your own `CARGO_TARGET_DIR`.
- Conventional commits, one logical change each. Every commit passes its lane gate.
- Update the lane ledger in the same commit as the slice.
- Never commit a partial slice. Record a blocker in the lane ledger and move to work that does not
  depend on it.

## Rules

- AGENTS.md code style and writing conventions apply. No history-narrating comments (ADR-0197
  Decision 24). Lint suppressions are `#[expect(lint, reason = "...")]`.
- New service items stay `pub` along a pub path from `lib.rs` until they are wired, so
  `-D warnings` sees no dead code. The switch narrows visibility.
- The 2.0 tree reads pristine (ADR-0197 Decision 24): no shims, dead code or transition paths.
- Nothing leaves the machine. Never read `local/`, `private/` or `saps/`. Binaries and journeys
  may run under `GHOSTLIGHT_TEST_HOME` with your own `CARGO_TARGET_DIR`. A 2.0 install on a real
  1.x home is a one-way break migration (83aa61f0: the 1.x service fell to all-open mid-move), so
  first preserve a copy of that home, then run `pwsh scripts/dev-loop.ps1 -Plan` before the real
  run. The install replaces the live service every session on this machine uses.

## Gates

Gate: the switch gate in switch.md.
