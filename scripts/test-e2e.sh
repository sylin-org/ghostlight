#!/usr/bin/env bash
# ADR-0051 Phase 1: run the test suite reliably on a developer machine.
#
# The spawn-based integration tests build and launch the real `ghostlight` binaries. On a dev box
# two things otherwise make a local run flaky (neither happens in CI, which has no live service and
# a closed stdin):
#   1. A running `ghostlight service` and Chrome's respawned native host hold `target/debug/*.exe`,
#      so the incremental linker cannot replace them mid-build (Windows especially).
#   2. The real-stdio relay test (`hub_identity::relay_adapter_*`) inherits the interactive
#      terminal's stdin, which never signals EOF, so it hangs.
#
# This script removes both without disturbing a running dev session: it builds into an ISOLATED
# CARGO_TARGET_DIR the live service never touches, and closes stdin so the relay tests see EOF.
#
# ADR-0051 Phase 4 moved the incidentally-end-to-end wiring tests in-process (support::inproc), and
# marked the ~27 genuinely-spawn tests `#[ignore = "e2e: ..."]` so a plain `cargo test` is fast and
# spawn-free (the CI `test` job). This runner is the FULL local pass: `--include-ignored` runs BOTH
# tiers, so it exercises exactly what the CI `test` + `e2e` jobs cover together.
# Extra args pass through as libtest args (e.g. `--test-threads=1` for a serial run).
set -euo pipefail
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${TMPDIR:-/tmp}/ghostlight-e2e-target}"
echo "test-e2e: isolated CARGO_TARGET_DIR=$CARGO_TARGET_DIR (a live dev service will not lock it)"
# The spawn tests launch ghostlight-relay as a SIBLING binary (tests/support::relay_bin), but no
# CARGO_BIN_EXE_ dependency forces cargo to build another package's bin before the root package's
# tests run -- build it explicitly so the first spawn never races the build plan.
cargo build --locked -p ghostlight-relay
cargo test --locked --no-fail-fast --workspace -- --include-ignored "$@" < /dev/null
