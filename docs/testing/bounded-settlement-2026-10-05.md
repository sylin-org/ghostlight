# Bounded settlement verification

Date: 2026-10-05. Status: Source verified; deployed; installed-browser acceptance passed.

## Result

The owner authorized bounded best-effort settlement across similar page tools, followed
by live verification. The implementation uses the existing Work, Language, document
discovery, observer and input owners. ADR-0195 records the changed contract.

- Read, inspect, find, screenshot, click, scroll, hover, fill, typing, keys, drag,
  upload and script execution accept `visual_settle`, default true.
- One maximum 1000 ms preparation budget is shared across discovery, useful-content
  collection and execution. False skips implicit waiting. Flow children have their
  own preference inside the parent's original deadline.
- Geometry preparation precedes current document/subject discovery. Binding a coordinate
  subject before waiting would unnecessarily stale it during ordinary iframe navigation.
- Targeted validation follows required documents and ancestry. Unrelated active-frame
  changes do not stop that work. Pointer and focus subjects are checked at each packet;
  moving excluded content into the destination cannot exploit a cached earlier subject.
- Composite waits preserve a met primary condition when optional visual settling expires.
  Explicit visual/layout waits retain their requested condition. Broad restricted
  document checks, masks, human controls and no-replay behavior remain.
- The document-scope capability is revision 2. Connectors retain their opaque relay roles.

## Evidence

The pre-change module counterproof rejected an unchanged top-document action when an
unrelated active child changed, even with every document allowed and change watching off.
The new tests prove both pre-action and between-packet continuation, while exact required
document replacement still stops input. Separate cases protect current point and focus
authority when an excluded document becomes the destination.

Worker tests cover busy-page execution across input families, early quiet completion,
opt-out without an observer, cancellation before an action handler, and coordinate
subjects discovered after preparation. Catalog/decoder checks cover every similar tool;
executor checks prove a shared lookup/action budget and opt-out on both phases.

Final checks on Windows:

- `cargo fmt --all -- --check`: passed.
- Workspace Clippy with `-D warnings`: passed.
- `cargo test --workspace`: 628 passed.
- `npm test --prefix extension`: 414 passed.
- Changed extension, page-runtime and acceptance JavaScript syntax: passed.
- Fresh `.target-dev-loop/debug` process journey: passed, including reconnect, scoped
  work, screenshots, recording, audit failure and recovery.
- CLI journey and workbench surface: passed. These are narrower checks and do not
  establish installed Chrome behavior.
- Optimized authority built in `.target-dev-loop/release`: SHA-256
  `028bbfb5e837b81f35a815289e0cf5b8a07e2a6ba3b45979a5d76016e90da0e3`.

Generated source evidence is under ignored `.tmp/settlement-*.log`. The release and
debug outputs are build artifacts; no second serving installation was selected.

## Installed acceptance

The owner reloaded the unpacked adapter. The authority was replaced through the
supported narrow swap; neither connector binary was replaced:

```powershell
pwsh -NoProfile -File scripts/dev-loop.ps1 -Action Deploy -Component orchestrator
node tests/settlement-live-journey.mjs
```

The live driver used the normal installed MCP connector, service, registered native
host and already-running browser. Its owned loopback fixture keeps a finite animation
running and repeatedly navigates an auxiliary iframe. It checks all thirteen page
tools with and without settlement, compared timings, and independently counted click,
key, drag, drop, upload and script handlers and retained text. It also distinguishes
composite wait success from an explicit visual wait timeout. Two complete runs passed.

The deployed authority was PID 82716, started at 14:57:13 local time, with the same
SHA-256 as the staged authority above. The reloaded browser relay was PID 81348,
started at 14:56:42. Acceptance used the existing authenticated Chrome session.

The final run recorded 40 calls. Both rounds completed while the finite animation
remained running and the auxiliary document navigated 141 times. Each round added
exactly one actual click, key, hover, drag, drop, upload and script effect. The draft
retained `typed`; the uploaded file retained its expected bytes.

| Tool | Default ms | Opt-out ms |
| --- | ---: | ---: |
| Read | 1035 | 20 |
| Inspect | 1031 | 45 |
| Find | 1029 | 33 |
| Screenshot | 1139 | 96 |
| Click | 1325 | 328 |
| Scroll | 1017 | 25 |
| Hover | 1043 | 45 |
| Fill form | 2125 | 1120 |
| Type text | 1079 | 58 |
| Press key | 1041 | 47 |
| Drag | 1088 | 71 |
| Upload | 1178 | 23 |
| Execute | 1150 | 134 |

These are full command times, not settlement durations. Fill retains its separate
post-edit retention check. Every opt-out removed roughly one second of preparation.
A composite load-ready wait succeeded in 1045 ms despite unsettled visuals; opt-out
succeeded in 24 ms. An explicit visual-settle wait correctly failed at its 150 ms budget.

The first run stopped at hover with typed background native-input protection after
click/fill/type/key succeeded. The receipt does not identify the precise focus or
placement transition, so this report does not attribute that refusal to a person or
settlement. It remains recorded, rather than being relabeled as success. Two fresh
runs completed without changing attention, grants, preserve-tabs or other safeguards.

Final evidence: `.tmp/settlement-live/3bbd6ebf-df9d-481d-b1b0-28ed019b800e/evidence.json`
and `fixture.jpg`. The prior complete run is
`.tmp/settlement-live/b414929c-0415-46a0-8271-dcabeeaca9e9/evidence.json`; the refused
run is `.tmp/settlement-live/9f54b6d2-7846-460f-9ad9-b1cb135fc8c0/evidence.json`.
The final screenshot and browser accessibility view showed the retained draft, file
and click count 2. All three exact owned fixture tabs were then closed through browser
control. Loopback servers and the driver's MCP processes ended. Human tabs were not
selected for cleanup.

The original SharePoint page was no longer open during diagnosis. The recorded failures
were permitted requests followed by unavailable semantic content, an uncertain adapter
result and a no-effect document-verification refusal. Neither the retained logs nor
these source tests establish that incident's exact routing trigger. Repeat the original
page journey before claiming that specific regression fixed.

Pre-existing installation, launcher, UI and documentation changes remain preserved in
the working tree. The local authority build includes existing source candidates;
their broader acceptance claims remain separate. This work changes no public version
and performs no publication.
