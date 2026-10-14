# Batch break-2.0: ground rules

This batch builds the ADR-0197 break set on `dev`: frozen relay connectors, MCP in the service,
adapter protocol 4, one install root, governance schema 4, the removals, and a pristine tree. The
target is [ADR-0197](../../adr/0197-contained-seams-and-the-2-0-break.md); it wins over anything
here. Protocol 4 is specified in [adapter-protocol-4](../../design/adapter-protocol-4.md).

Three lanes work in one checkout at the same time: service, extension and connectors. A switch
phase follows, run by one agent after the lanes finish. Each lane keeps its own ledger in this
directory.

## Ownership

No file is edited by two lanes.

| Lane | Owns |
| --- | --- |
| service | `crates/orchestrator/src/**` except the page assets under `browser/adapter/chromium/level4/assets/` and `chromium/assets/`; its ledger. The browser vocabulary it extends is `chromium/level3/mod.rs` |
| extension | New files under `extension/` only; `tests/golden/protocol4/**`; the level 4 page assets; `scripts/lint-js*.mjs`; its ledger |
| connectors | `crates/relay/**`, both connector crates, root `Cargo.toml` and `Cargo.lock`, `tests/golden/relay3/**`, `tests/golden/cargo-tree/**`, `tests/golden/frozen-hashes.json`, `tests/guards/connectors.test.mjs`, `tests/lib/cdp-pipe*.mjs`, `tests/installed-journey.mjs`, `scripts/dev-loop.ps1`, `scripts/get.*`, `packaging/**`, retired-channel scripts and tests, `.github/workflows/release.yml`, `scripts/check-public-surfaces.ps1`, `scripts/release-preflight.ps1`, ADR header notes and README status cells, this file, its ledger |
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
  or push. While `.git/index.lock` exists, retry after a few seconds.
- Conventional commits, one logical change each. Every commit passes its lane gate.
- Update the lane ledger in the same commit as the slice.
- If a slice cannot go green in about 60 minutes, commit nothing partial, record the blocker, and
  continue with the next slice that does not depend on it.

## Rules

- ASCII only. A doc comment on every module and every public function. No history-narrating
  comments. Lint suppressions only as `#[expect(lint, reason = "...")]`. Repeated literals become
  named constants.
- New service items stay `pub` along a pub path from `lib.rs` until they are wired, so
  `-D warnings` sees no dead code. The switch narrows visibility.
- The 2.0 tree reads pristine (ADR-0197 Decision 24): no shims, dead code or transition paths.
- Nothing runs or leaves the machine. No Ghostlight binary, journey, service, dev loop,
  installation or browser. Never read `local/`, `private/` or `saps/`.

## Gates

- Connectors: with `CARGO_TARGET_DIR=.target-connectors`, `cargo fmt -p <crates> --check`,
  `cargo clippy -p <crates> --all-targets -- -D warnings` and `cargo test -p <crates>`, where
  `<crates>` is only what is already built on `ghostlight-relay`. Script slices add a parse check,
  `sh -n`, `npm test --prefix packaging/npm` or `node --test packaging/mcpb/test/launcher.test.js`
  as they apply.
- Docs slices: `pwsh -NoProfile -File scripts/check-repository-integrity.ps1`.
- Service and extension lanes: their own gates, named in their ledgers.
