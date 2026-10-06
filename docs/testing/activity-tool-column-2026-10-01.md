# Restore the activity tool column -- 2026-10-01

Candidate on `codex/in-service-outcome-ux`, following clean `801798e0`. The owner clarified
that tool identity belongs in the primary white activity column. Repeating Completed, Request
refused and Effects uncertain labels removed useful distinctions between tools. The original
`22d27bd1` renderer displayed `entry.tool`; the new renderer had replaced it with outcome/activity.

## Before and after

For the quoted record, the primary column changes from `Effects uncertain` to `browser_execute`.
The secondary sentence remains "Sent, but the browser never confirmed what happened." The
unsafe-repeat guidance remains visible below that row. This identifies the known tool without
inventing the purpose of a script or claiming a task completed. Read, fill, navigate, screenshot,
inspection and other rows regain the same exact identity. Live and restored records already have
that fact, so no new context field or payload retention is needed.

Only `crates/orchestrator/ui/lib/view.js` changes product behavior: row detail buttons display the
existing tool and begin their accessible name with the same tool. Expansion keys, focus handling,
secondary outcome text, exceptional recovery, hero, all other columns, exact Show tab checks,
governance, machine facts and the complete guardian About card are preserved. No styling changes.
The earlier broader proposed prose rewrite is superseded by the owner's narrower correction.

## Actual evidence

Fresh installed native capture: `.tmp/tool-column-native-current.png`, PID 25200. It shows the
mixed list of repeated Completed labels alongside uncertainty and human-control outcomes.
Installed image SHA-256 stays
`917c2de645a017f3eaeb91392a14623fa2a8529823432741149b4a8f8d146527`.
The first native locator attempt found no At a glance control; an exact-window PrintWindow capture
showed that destination already selected. This was not treated as a product navigation failure.

Both owner-supplied Library screenshots were resolved using the current Library skill's required
materialization route with consumer-local destinations. Both failed at the Windows metadata helper:
`AttributeError: module 'os' has no attribute 'setxattr'`. No transfer or metadata bypass was used,
and neither supplied screenshot is claimed inspected. The native capture is independent evidence.
Resolved identities: `libfile_d231d58c7bc48191902f9d9b1904b643` and
`libfile_7b3be4055c348191b0526f050bbbda2d`.

Paired component renders use the same closed synthetic fixture, real UI modules and unchanged CSS:

- `.tmp/tool-column-review/before-1400.png`: current repeated outcome labels.
- `.tmp/tool-column-review/after-1400.png`: distinct exact tool names across the entire mixed list.
- `.tmp/tool-column-review/after-700.png`: narrow layout retains those names and recovery.
- `.tmp/tool-column-review/fixture.json` and `capture.json`: fixture and capture identities.

These are disposable headless component previews with no extension, credentials, live service
connection or user tabs. They prove rendering, not installed candidate acceptance. The installed
application has not been replaced or patched in memory. No browser or service was restarted.

## Validation and limits

The executable Workbench test passes 105 assertions. Four new checks cover a complete mixed list
with live, success, refusal, unknown, partial, human-control and legacy rows; visible tool and
accessible name; secondary outcomes and visible exceptional recovery; and live-to-settled identity.
Existing expansion, focus, child receipt, Show tab and uncertainty coverage remains intact.
Aggregate gates pass: `cargo fmt --check`, strict workspace/all-target Clippy, 594 Rust tests,
365 extension tests, both changed JavaScript syntax checks and `git diff --check`.
Logs are `.tmp/tool-column-{clippy,rust,extension,surface}.log`.
Frozen source SHA-256: view.js `ddeebb2ef451bebd8c961e31136c5ce16ea1b39469d8459b6896223d6e39b90f`;
workbench-surface.mjs `6d3ebeed1ebe7e044effeccc6e47fe1c4ddb7faf27d7b597cf3c3b832ebbd374`.
The disposable component profiles have been removed after their processes exited; snapshots and
the reproducible fixture remain. The existing native/browser installation and user tabs remain.

The parent independently evaluated comprehension and safety before installation. The original
failure diagnostics and earlier local deployment evidence are preserved. Public versions and
publication state are unchanged. No generated art, dashboard, data model or new destination.

## Approved installed follow-up

The parent approved exact `696275e66fb69c9412b1a1b56533066a948f82a4` and directed the authorized
local correction. `scripts/dev-loop.ps1 -Action Deploy -Component @('orchestrator')` built in
`.target-dev-loop`, stopped only the selected old authority, replaced only `ghostlight.exe`, and
started the selected authority. Existing adapter and connector files were unchanged; no native-host
registration, browser restart or extension reload occurred. Native PID 28872 remains running.

Installed and fresh-build SHA-256 match:
`8f4bc1ee0bd7bdecee90d2ed5c307dcdf5e0319ab3932f311a8dce6985402a7a`.
Verification at 13:06 UTC confirms Ready; identical selection and policy-file state; prior audit
bytes preserved; both connectors unchanged; and identical adapter source, permissions and grants.
`.tmp/tool-column-installed-verified.json` records the facts. Version remains 1.3.12.

Actual installed screenshots under `.tmp/`:

- `tool-column-installed-mixed.png`: restored tool names in the original mixed history.
- `tool-column-installed-owned.png`: a later successful read with earlier uncertainty retained.
- `tool-column-installed-show.png`: exact owned Show tab control used without changing Ready.
- `tool-column-installed-details.png`: uncertainty details expanded with repeat guidance and Show.
- `tool-column-installed-about.png`: complete unchanged guardian card and adjacent local facts.
- `tool-column-installed-final.png`: Ready, compact mixed list, tool names primary.

One fresh localhost fixture opened, read, ran one intentionally throwing synthetic script, and
read again. The uncertain result retained `repeat_safe: false` and was never repeated. No existing
user tab was navigated or closed. The fixture tab remains available for inspection; its temporary
server and connector exited. Exact receipts: `.tmp/tool-column-installed-ui.json`.

Native UIA initially omitted the Show tab control until the exact window/provider was refreshed.
The row disclosure correctly exposes ExpandCollapsePattern, not InvokePattern. Using that
supported pattern expanded and collapsed it. These harness limits did not trigger another browser
action or change the product. There is no claim about global keyboard activation.

Fresh rollback capture: `.tmp/local-deployment-2026-10-01T12-51-36Z/`. Its guarded,
syntax-checked `rollback-orchestrator.ps1` is bound to that directory and restores only the saved
prior authority, leaving selection, policy, audit, connectors and adapter in place. It was not run.
Open the actual native app with `target/release/ghostlight.exe open`; no HTTP UI address exists.

The 700px component preview has inherited clipping from the unchanged 720px minimum width.
It shows restored identities, not full fit at 700px. That layout issue is outside this targeted
correction. Both uploaded Library screenshots remain uninspected due to the Windows metadata
helper failure recorded above. Actual native captures supply independent evidence.
