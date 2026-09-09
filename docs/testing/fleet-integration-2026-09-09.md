# Fleet integration, 2026-09-09

The four acceptance branches are combined on `codex/fleet-integration`, based on
coordinator commit `ca8227c7`. The owner checkout remains on `dev`; no shared branch
or release is updated by this integration. Service/package remains source candidate
1.3.5 and the adapter remains source candidate 1.1.2.

## Included work

| Source branch and head | Changes and evidence |
| --- | --- |
| `codex/fleet-test-01`, `9da0bbf7` | Document-bound targeted typing, closed result schema repair, Debian inspection pipeline fix; [CachyOS report](fleet-test-01-2026-09-09.md). |
| `codex/fleet-test-03`, `5205c1c3` | Independent schema repair, honest recovery failure reporting, native-musl evidence; [Alpine report](alpine-musl-fleet-2026-09-09.md). |
| `codex/fleet-test-02`, `f51130d0` | Linux Codex desktop environment forwarding, preservation of custom and malformed configuration; [Bluefin report](bluefin-fleet-2026-09-09.md). |
| `codex/fleet-leo-desktop-02`, `21d4e3ed` | Windows TCP table ABI correction and exact-installation NSIS process shutdown; [peer](windows-fleet-peer-2026-09-09.md) and [packaging](windows-fleet-packaging-2026-09-09.md) reports. |

Original commits and reports remain in the integration history. The two independently
discovered schema fixes are reconciled into one property: Alpine's typed storage enum
supplies its values, and CachyOS's canonical-receipt test checks required fields, types
and enum membership across every tool and both storage states. The narrower duplicate
test is not retained. The language contract now documents the existing receipt field.

Each merge is checked before its integration commit. Conflict resolution preserves
the current campaign state, standing owner directions and platform learnings. No
policy or product decisions are moved into a connector or the extension.

## Central verification

The combined Windows build passes formatting, warnings-denied workspace Clippy,
all 533 Rust tests (448 orchestrator library), all 222 extension tests, changed
JavaScript syntax and staged diff checks. All eight browser-harness and recovery
reporting tests passed after Alpine integration. The final process and provenance
journeys against freshly built sibling executables passed, including 12 real provenance
receipts from three OS-observed executable images. The workbench surface also passed.
`GHOSTLIGHT_BIN_DIR` explicitly named the fresh `.target-dev-loop/debug` siblings.

Ignored coordinator logs are under `.tmp/fleet-acceptance/` in the owner checkout:
`integration-cachyos-gates.log`, `integration-alpine-gates.log`,
`integration-bluefin-gates.log`, `integration-windows-gates.log` and
`integration-process-gates.log`. These are central source/isolated-boundary checks,
not a new installed-browser run or a rebuilt native package acceptance claim.

The bounded Linux check of exact combined source db57b9bf is now complete on Bluefin:
542 Rust tests, including the Linux-only Codex configuration cases, 222 extension
tests and the source gates pass. The host controller replaced only the authority;
the actual Codex cold-start call created one authority and returned policy_explain
with succeeded/effect none/history_storage saved. Connectors and saved client and
native registration bytes are unchanged. The [Bluefin integration report](bluefin-fleet-integration-2026-09-09.md)
records exact before/after hashes and scope. Existing installed-browser evidence
retains the candidate and artifact identity recorded by each machine; no native
browser/GNOME, reboot or broad campaign checks were repeated.

## Remaining limits and credential state

CachyOS and Bluefin verified existing owner authentication through libsecret.
Windows imported the destination-encrypted handoff directly into GCM Windows secure
storage and verified owner account, repository push access and authenticated Git
push dry-run. Alpine's branch push is observed, but its account/helper verification
and post-reboot report await reconnection. No plaintext credential or private key
is included in source, reports or task messages.

The final Windows uninstall/refusal test was rejected by automatic approval review
with only `blocked by policy`; no alternate-route retry was made. Windows still
needs an adapter loaded through an allowed route. Bluefin still lacks native visible
browser/GNOME acceptance. Alpine packaging remains a native-musl debug prototype,
and the published GNU installer is not a working Alpine route. Store, pristine
Windows, full reboot and release-provenance claims remain limited as recorded in
the individual reports. None of these limits is converted into a passing result.

The scheduled coordinator continues until the combined Linux check, remaining
credential verification and consolidated delivery are accounted for. It must then
delete the campaign automation, as requested by the owner.
