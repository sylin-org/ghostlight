# Current document lifecycle recovery -- 2026-10-01

Correction `1485b8a4033a754c7741a56fcebbcc941fcab075` on `codex/in-service-outcome-ux`, following
deployed native source `7af79ed4`. Further live actions paused at the owner's 06:16 UTC report.
The parent independently accepted the frozen patch before the existing adapter was reloaded.
Bounded installed acceptance passed. The foreground lane is now released for final independent
live review; no concurrent GUI, browser, service or adapter changes are planned.

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

## Installed acceptance

The parent independently replayed 17 repo tests and five separate probes. Frozen SHA-256 values
matched: documents.js `99d9a457144b4747d5360d2e664044221e25b27a83e168c416e9b5d421f472bd`;
documents.test.js `740d2c7395b186552f1b3357b74c23a2fcd4b447c1a6bb3487c0c75ad2f02972`.
Only the existing adapter Reload control was invoked. A generic Chrome main-window helper first
refused the wrong window; exact internal details-page identity resolved that harness limitation.

The first post-reload attempt used a stale saved tool handle and returned `tab_unavailable`,
effect none. It is retained in `.tmp/local-deploy-lifecycle-stale-handle-failure.json`.
A fresh owned localhost fixture passed two consecutive read/fill/read rounds, with exact filled
value observations. `.tmp/local-deploy-lifecycle-accept.json` contains every receipt.
Chrome simultaneously reported active HTTP root 0, HTTPS prerender root 46 and HTTP prerender
root 47. The new adapter projected only the active document. Sanitized actual inventory:
`.tmp/local-deploy-lifecycle-final-inventory.json`. No private URL or page content is included.

The installed human-control journey passed: thrown script retained unknown effect and recovery;
Pause blocked fill with no effect; Show tab left Pause unchanged; Resume preserved one script
effect and original input value, replaying neither request; End session blocked reads; Start
session restored Ready. `.tmp/local-deploy-live-ux.json` and its timestamped copy retain receipts.
Actual final native screenshots: `live-native-uncertain.png`, `live-native-paused.png`,
`live-native-paused-after-show.png`, `live-native-ended.png`, `live-native-final-about.png`, and
`live-native-final-ready.png`, all under `.tmp/`. The complete guardian card is unchanged.

The installed native Workbench renders correctly in `.tmp/live-native-corrected-current.png`.
Its visible-only At a glance test locator failed while another application occluded the window;
that harness result does not show missing navigation. Previous outcome and full About captures
are `.tmp/live-native-printwindow.png` and `.tmp/live-native-about.png`.
The actual local UI entry remains `target/release/ghostlight.exe open`; there is no HTTP UI address.

Rollback copies exist for the 04:43 and 05:56 native swaps. Reverting those binaries alone does
not remove this historically reproduced adapter defect. Selection, policy, history, registrations
and browser grants must remain intact throughout restoration. No remote publication occurred.

Remaining limits: the parent owns final independent live acceptance. Disconnected Pause,
legacy history, child recovery, narrow/enlarged text and Tab/Shift-Tab were validated in the prior
isolated native acceptance; this final installed run did not repeat them. Enter activation
remains inconclusive from the earlier guarded attempt. Form-diagnostics worker suspension and
delivery races have executable source coverage; actual worker suspension was not forced in this
run. Synthetic fixture tabs and the own worker inspector remain available for independent review.
