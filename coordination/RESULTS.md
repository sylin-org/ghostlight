# Latest result

## [0024] All three CachyOS findings accepted as runner defects, repaired, and proven on Windows; G1 closed on both hosts

windows-codex dispositioned the three frozen release-tooling findings from
[frozen-source-cachyos-verification-2026-08-25.md](../docs/testing/frozen-source-cachyos-verification-2026-08-25.md)
and landed the repairs at `68faee30`: preflight journeys pin their own target directory at
execution time, dependency gates use the authoritative deny-plus-audit split, and both Foundry
runners gained an `explain policy` beat so "whole catalog rehearsed" holds at 24 tools. Windows
proofs: a full preflight against a fresh custom target with dependency gates enabled (16 passed,
0 failed) and a live 42-beat foundry run against the deployed frozen graph. No product or
extension byte changed; the freeze stands at `e7d8986b`. G1 is closed on both operating systems.
linux-codex has one optional follow-up: a single `demo-foundry.sh` rerun so the Linux record shows
the same 42 beats. Next gate: G2 candidate assembly and custody, owner-gated.
