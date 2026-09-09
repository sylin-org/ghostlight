# Bluefin combined-candidate check, 2026-09-09

## Identity and scope

PASS for the requested bounded Linux integration check of exact source
db57b9bf37631ff3029901f7ba43f7edc405fd24. No integration defect was found.
Source was clean at that revision through gates, build, deployment and the real
Codex call. This report is a later documentation-only commit on
codex/fleet-test-02-integration, not a change to the shared integration branch.

Machine: bluefin / test-02, Bluefin 44.20260901, Fedora 44 atomic, GNOME Wayland,
x86_64. Compilation uses the existing Fedora 44 Toolbox ghostlight-fleet-test-02;
the deployment controller, installed authority and actual Codex run on the host.
The isolated source worktree is
/var/home/test/ghostlight-fleet-test-02/integration. The existing installation stays
at /var/home/test/ghostlight-fleet-test-02/source/target/release.

## Source gates

| Check | Result |
| --- | --- |
| cargo fmt --check | PASS |
| cargo clippy --workspace --all-targets -- -D warnings | PASS |
| cargo test --workspace | PASS: 542 tests; 457 orchestrator library |
| Linux Codex configuration coverage | PASS: all six Codex installer tests, including malformed members, custom/explicit values, inline entries and manual/automatic equivalence |
| Canonical receipt/output-schema regression | PASS across every tool and both storage states |
| npm test from extension | PASS: 222 tests |
| Changed extension JavaScript syntax and Debian lifecycle shell syntax | PASS |
| Chromium harness and installed recovery reporter | PASS: eight tests |
| Workbench surface fixture | PASS |
| Locked release authority build | PASS: 1m 23s |

Cargo reused the existing isolated build caches under source/.target-fleet-debug
and source/.target-fleet, not the live target/release directory. Compiler logs name
the integration worktree's crates. An initial auxiliary test command misspelled
the Chromium harness filename and ran only the two reporter cases; the corrected
explicit command ran and passed all eight. It is the latter log that supports the
eight-test claim. No full hardening or installed-browser campaign was rerun.

## Authority-only deployment

The integration dev-loop.ps1 is byte-identical to the already installed worktree's
script, SHA-256 2c310eb12d9d830fe40d70c8c16689df6a68223cf83ae14915a654e849d2b463.
The host invokes that script with Component orchestrator and NoStart. Its Cargo
command is delegated to Toolbox with --manifest-path pointing to the exact
integration worktree. This preserves the controller's repository path guard and
the existing installed path. No script safety check or registration was changed.

The controller found and stopped only the selected installed authority PID 60429,
replaced only ghostlight, removed deploy.lock, and left the authority stopped for
the actual cold-start test. The fresh build and installed authority match exactly.
The source diff leaves both Linux connectors and bridge unchanged; the win-peer
ABI change is Windows-specific. The adapter was not loaded or reloaded.

| Installed sibling | Starting SHA-256 | Final SHA-256 |
| --- | --- | --- |
| ghostlight | b0e6faf96c489c8aff5f7b426f34a76f3d32c37b5a6bd265345c775765ddb101 | 047a14947d70973e54ae34df3a33db1fa1d4ffcd633faf3636e079d3888a88dd |
| ghostlight-mcp-connector | 301d579aa8b014eb05f2d2dd711273cb015de01cac6151a48766258e46f8a9b0 | unchanged |
| ghostlight-browser-connector | 30f3b26e2db7224739b357612e7cab31766b830c25a316035a4ef72dc3121159 | unchanged |

## Actual Codex cold start

The previously passing Codex CLI 0.152.0 driver was reused with a new evidence
directory. It consumes the installed product's generated manual configuration,
forwards environment names only, and uses ephemeral per-invocation configuration.
The desktop variables are recorded only as presence/parent-match booleans.

- No authority was running when the driver began; it stopped no additional PID.
- Exactly one authority appeared: PID 94499, the exact installed executable.
- All six forwarded desktop variables were present and matched the parent.
- Exactly one actual MCP policy_explain({}) call completed.
- Structured outcome: status succeeded, effect none, history_storage saved.
- Codex exited 0. Assertions checked the transcript, result and exact process path.
- Saved Codex configuration, Applications entry and Chromium native-host manifest
  hashes are unchanged before/after. No policy or browser action was requested.

The authority remains running as service 1.3.5, browser relay major 1 and service
bridge major 2, with diagnostics enabled. Browser readiness remains Not connected.
Original main remains clean. Previous portable packages retain their earlier
binary identity; no packages or releases were rebuilt or published in this check.

## Evidence and limits

New raw evidence is under
/var/home/test/ghostlight-fleet-test-02/integration-evidence/: baseline.log,
source-gates.log, javascript-gates.log, harness-reporting-gates.log,
syntax-gates.log, authority-build.log, host-dev-loop.ps1, host-deploy.log,
deployed-hashes.log, registration-before.log, registration-after.log,
check-codex-cold.mjs, codex-cold-generated-desktop.jsonl,
codex-cold-generated-desktop-environment.json and verified-outcome.log.

The original failed cold-start evidence remains untouched in the parent evidence
directory. Its environment JSON SHA-256 is
c957f8b8efb22c18845b309a94d40b9f58f561c97a97bf60d7892e8ba4d6700e;
its JSONL SHA-256 is
6a02bac2038fac53bb54039deb3c08cd518628e79aa65de60624aaa7a92b9208.
The [earlier Bluefin report](bluefin-fleet-2026-09-09.md) retains the original
failure, repair and package evidence.

Native desktop controls, installed-browser/adapter behavior, visible GNOME and
reboot acceptance remain outside this check and retain their previous limitations.
Credential setup was not repeated. The coordinator confirmed Git report retrieval
works; task-message delivery is not retried. No shared integration-branch push,
merge, PR or release is part of this result.
