# Current document lifecycle recovery -- 2026-10-01

Source-only correction on `codex/in-service-outcome-ux`, following deployed `7af79ed4`.
No adapter reload or further installed action occurred after the owner's 06:16 UTC report.
The foreground lane is released for parent-coordinated independent review of this frozen patch.

## Evidence and attribution

Installed synthetic `browser_execute` and `browser_read` returned `document_unavailable`,
effect `none`, before dispatch. Chrome's own worker API returned active HTTP root frame 0 plus
prerender HTTPS root frame 42 in the same fixture tab. The active MAIN dispatch function and
runtime marker were installed. `.tmp/local-deploy-frame-inventory.json` preserves schemes,
lifecycles and API shapes without URLs or page content.

Independent historical replay reproduces the inventory refusal at `22d27bd1`, `c491a264` and
`7af79ed4`. Their document verifier has the identical Git blob `d87f19d0d8d509ab2e8b2d84fc9d4a4affe4fd7a`.
The separately confirmed MAIN-world diagnostics dependency fixed by `7af79ed4` is not attributed
as the cause of this inventory refusal. Neither result attributes every owner failure.

The audit was preserved at `.tmp/local-access-incident-2026-10-01T06-22-35Z/` before correction.
Redacted exact-invocation classification identifies six of nine recent verification failures as
saved synthetic attempts; three remain unclassified. All nine recorded effect `none`.
No raw audit, private host, credential, or page content is included in this report.

## Frozen correction and source verification

`extension/lib/documents.js` filters inactive lifecycle trees before both inventory and routing.
Active root, unique identities, connected ancestry, optional parent-document identity, bounds,
HTTP/error support, exact targeting, allowed scope, and active-scope-change checks remain strict.
Whole legacy metadata-free snapshots retain validated compatibility; ambiguous modern metadata
refuses. No permission, protocol, policy, or refusal-summary change accompanies the correction.
[ADR-0189](../adr/0189-current-document-trees-exclude-inactive-browser-pages.md) owns the decision.

Passed aggregate gates: `cargo fmt --check`, strict workspace/all-target Clippy, 594 Rust tests,
365 extension tests, 101 executable Workbench assertions, both changed JavaScript syntax checks,
and `git diff --check`. Logs: `.tmp/document-lifecycle-{clippy,rust,extension,surface}.log`.
New coverage includes consecutive scoped read/fill/read with active 0 plus prerender 42,
inactive-tree addition and removal, stale cached locators, point/focus routing, all three
inactive lifecycles, malformed active trees, exclusion and post-dispatch uncertainty.
Independent Sol Max source review also replayed historical and adversarial cases.

## Installed acceptance still required

This candidate has not been explicitly reloaded. Freeze source before the parent's independent
review. Then reload only the existing Ghostlight adapter; native binaries need no rebuild.
Verify consecutive reads and fills in the ordinary installed browser under the reproduced frame
condition, followed by the bounded human Pause/Show/Resume/Stop journey. Retain failing receipts.
Do not suppress verification refusals globally or relax access guards to get a green result.

The installed native Workbench renders correctly in `.tmp/live-native-corrected-current.png`.
Its visible-only At a glance test locator failed while another application occluded the window;
that harness result does not show missing navigation. Previous outcome and full About captures
are `.tmp/live-native-printwindow.png` and `.tmp/live-native-about.png`.
The actual local UI entry remains `target/release/ghostlight.exe open`; there is no HTTP UI address.

Rollback copies exist for the 04:43 and 05:56 native swaps. Reverting those binaries alone does
not remove this historically reproduced adapter defect. Selection, policy, history, registrations
and browser grants must remain intact throughout restoration. No remote publication occurred.
