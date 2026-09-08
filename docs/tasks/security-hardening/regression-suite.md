# Session-wide hardening regression suite

The owner requested a full regression suite after the unsent-editor incident. This record covers
every implemented package from `48ef29ec` through the current hardening work: H1, H2a/H2b, H3, H4,
H5, H6, H7, H8, the accepted C1 reporting foundation, and the editor/caller-restriction corrections.
It does not turn deferred signer/hash verification or C2/C3 admission into implemented features.

## Run the suite

Use Rust from `rust-toolchain.toml`, Node 22 or newer, and Chrome for Testing with unpacked-extension
support. Set `GHOSTLIGHT_TEST_BROWSER` to that Chrome executable. The existing local default is
`.tmp/chrome-testing/chrome-win64/chrome.exe`.

```text
node tests/hardening-suite.mjs
```

The runner performs formatting, Clippy with warnings denied, all workspace Rust tests, every
extension test, policy grammar, a fresh workspace build, process/reconnect, local continuity,
provenance, both CLI journeys, the workbench surface, real Chromium script execution, actual MV3
document/editor/capture/recording behavior, and real Chromium history interaction. It runs all
independent gates after a failure and exits nonzero if any fails or a required browser is missing.
It never labels an unavailable lane as passed.

`CARGO_TARGET_DIR` may select another build directory. The runner always builds there, reads Cargo's
emitted executable artifact paths, and passes their directory as `GHOSTLIGHT_BIN_DIR`; an inherited
stale binary selection or cross-build output directory cannot silently win.
It writes logs and `results.json` beneath `.tmp/hardening-suite/<run>/`, including source revision,
source-content fingerprint, executable hashes, platform, named gate results, and timings. A source
change during the run invalidates the result. Documentation-only edits do not change the source
fingerprint. The report is evidence for the named platform and lane, not an installation claim.

`--lane=process` and `--lane=browser` select explicit subsets for separate CI jobs. Both still build
the exact binaries they test. CI includes continuity and all three Chromium journeys on Windows
and Linux, using pinned Chrome for Testing 152.0.7977.82 and retaining browser test artifacts. The
CI-only `GHOSTLIGHT_TEST_NO_SANDBOX=1` option affects only the disposable Linux test browser.

The default frame journey uses a hash-checked snapshot of the owner's public Sylin demo and all
its render/script assets. It needs no public-site fetch. Set `GHOSTLIGHT_TEST_LIVE_SYLIN=1` for the
separate public-content check; do not substitute a live-site outage for a regression result.
Fixture provenance and update instructions are in [the fixture guide](../../../tests/fixtures/README.md).

## Feature coverage

Every row needs permitted-work and refusal/failure evidence, with an independent observation of
effects where the browser can change. Test counts alone do not close a row.

| Feature | Success and boundary/failure checks | Owning executable suite |
| --- | --- | --- |
| H1 readable bounded audit | Useful authored measurements/permission evidence remain; actual JSONL excludes form/script/page/exception sentinels, including failed compositions and historical reconstruction. | Rust language/audit and work tests; process and script-browser journeys. |
| H2a stopping | Stop prevents later dispatch after child decode, reference, policy, and execution failures; Continue permits later independent work. | Rust flow tests; process journey; actual Chromium script flows. |
| H2b aggregate effects | Direct and composed paths preserve successful, partial, unknown, and never-run work; failures cannot become success; omitted payloads retain progress; dry run never executes. | Rust composition/work tests; process and script-browser journeys. |
| H3 single execution and script truth | Expressions, bare return, await, repeated declarations, and resource declarations work; runtime exceptions never cause replay or false no-effect; invalid syntax never evaluates. | Extension evaluator tests; script-browser and installed live journeys. |
| H4 grouped history | Incremental child receipts precede parent completion; reload, retention, missing receipts, permission details, and focused/expanded UI state remain truthful. | Rust history/governance tests; process, workbench-surface, and history-browser journeys. |
| H5 runtime controls and caller limits | Exact capability sets permit each supported operation; missing requirements prevent dispatch; attention stays local; Pause/Stop/cancellation/deadlines stop later effects; diagnostics remain usable without releasing controls. | Complete Rust capability matrix and control tests; process, continuity, and installed live journeys. |
| H6 document authority and coverage | All handling/notice combinations permit fully allowed content; mixed grants omit denied values/handles before extraction; negatives do not claim unseen absence; stale documents cannot regain authority; coverage reasons remain distinct. | Rust governance/frame tests; extension document tests; frame-browser and installed live journeys. |
| H6 captures, scripts, and recordings | Delivered screenshot pixels mask denied regions; tampered masks discard output; ordinary captures/scripts/recordings still work; excluded-document scripts refuse; restricted recordings stop at document changes and every export destination reauthorizes source history. | Rust frame/recording tests; extension capture/document/recording tests; frame-browser and installed live journeys. |
| H7 audit health | Keep working preserves browser results; Require audit prevents new work during failure, including composition children; live receipt storage is independent; repair never replays/backfills or hides historical gaps. | Rust audit/work tests; real process failure/repair/cold-start cases; actual history UI. |
| H8 continuity and local runtime | Bursts wait within original deadlines; duplicate IDs retain cancellation; incomplete/stalled peers expire independently; Pause/Stop drains queues without replay; fresh sessions recover; runtime publication uses exact private permissions. | Rust bridge/service tests; process and local-resilience journeys on each platform. |
| C1 reporting | Distinct processes are observed correctly; queued/composed/refused work keeps its original connection across shared sessions/reconnects; claims remain transient; restored history cannot borrow a new claim. | Rust provenance/audit/history tests; real provenance processes; actual Chromium history rendering/escaping. |
| Draft editing and form preflight | Native transactions retain ordinary/open-shadow controlled drafts, replacement, multiline text, and clearing; no submission occurs without an explicit submit target; known invalid fields anywhere in a batch prevent earlier edits. | Extension content/worker tests; real MV3 editor and cross-frame cases; installed live journey. |
| Page feedback and script progress | Read feedback follows the resolved tab; overlapping operations cannot animate an unrelated page or clear each other's progress; every terminal path removes its own script spinner. | Orchestrator presentation/dispatch tests, extension routing/renderer tests, and real Chromium visual checks. |

The [process and human-history inventory](regression-process-coverage.md) names individual checks
for H1/H2/H4/H5/H7/H8/C1. Browser and executor additions are retained beside their existing fixtures,
so they run under ordinary test discovery as well as this suite.

The catalog-authority tests exercise 254 cases across all 24 advertised tools and 32 authority
variants, with an independent expected-capability table. They check omitted/exact/missing request
allowlists, minimal configured grants, direct and flow-child execution, and split recording source
and destination grants. Read on a recording source and Write on its destination remain independent.

Visual regressions execute the shipped renderer and worker routing. Actual MV3 tests inspect its
closed shadow DOM while reads/scripts overlap across tabs, then verify terminal cleanup. Raw CDP
screenshots preserve the running and completed wheel states; Ghostlight's own screenshot hides its
feedback and would be the wrong observation. Hidden denials clear their invocation's activity while
keeping the human notice queued. Renderer tests also cover preference changes, stale fallback,
runtime controls, disconnect, and old timers racing a newer invocation.

## Installed acceptance

```text
node tests/live-journey.mjs
```

This is an explicit installed-system test, separate from the default isolated suite. It uses the
actual MCP connector, running service, registered native host, and installed extension. It requires
all-open configured authority and does not change policy or global human controls. It creates its
own disposable tab, tests retained editor values and source-bound behavior there, then exercises
the public Sylin form's local-only simulation and captures. It honors the browser's preserve-tabs
setting, reports the retained tab, and never uses the existing Reddit draft or submits a Reddit
comment. `GHOSTLIGHT_BIN_DIR` selects the installation, defaulting to `target/release`.

The lane writes `.tmp/installed-hardening-evidence.json` with installed executable hashes and
invocation evidence, plus a masked screenshot. Run it after the changed binaries and extension are
actually active. An isolated MV3/native-port shim cannot establish native-host installation, and
an installed live check cannot replace deterministic audit-failure or control-race tests.

## Execution record

The final Windows run on September 7 passed all 16 gates on unchanged source:

- 520 Rust tests, including the 254 catalog/authority case iterations.
- 207 extension tests and six Chromium-harness fault-injection tests.
- 23 real-engine script cases and 65 real MV3 cases, including five visual regressions.
- Process/reconnect, continuity, provenance, both CLI journeys, policy grammar, workbench surface,
  and real Chromium history interaction.
- Required formatting, Clippy with warnings denied, fresh workspace build, and changed JavaScript
  syntax. The direct `npm test --prefix extension` gate also passed.

The local report is `.tmp/hardening-suite/2026-09-08T00-00-20-637Z-36092/results.json` (UTC path).
It records `passed:true`, `source_unchanged:true`, and source fingerprint
`d548a9e7493d0b5e9e2dd8c8f7641116c937e5104669866a625333b64ee48dc5`.
The browser is Chrome for Testing 152.0.7977.82. `.tmp/h6-browser-evidence.json` records all named
MV3 checks and snapshot hashes. The running/completed visual PNGs were also inspected: the actual
wheel is present during execution and absent after completion.

The first complete run failed rather than hiding its defects: an outdated PowerShell synthetic
adapter lacked the H6 document contract, and Windows temporarily locked Chrome's endpoint/profile
files. The PowerShell fixture now proves actual admitted read/capture and byte-exact delivery.
The shared Chromium harness waits for complete readable startup data and bounded owned-process
cleanup; unexpected errors or persistent locks still fail. The final full run passes those lanes.

The final orchestrator was deployed through the dev-loop. Installed and isolated release SHA-256
match `75650ebf6bd5c222f6ada8321c4b625bdac63647e33af3a7184166e2116c1252`; the exact-path process
is Ready, the deploy lock is absent, and both connectors remain unchanged. The final extension
reload and installed live journey are pending the owner's manual action. The source/browser
fixture pass does not assert that the installed adapter has reloaded.

Windows/Linux CI now includes the missing continuity and all three browser lanes, but remote CI
has not run and no push is authorized. Full Linux desktop/browser verification remains unexecuted
here; the earlier WSL loopback/environment failures remain documented in STATUS. This report
does not close deferred C1 verification, C2/C3 admission, or extension reload bootstrap recovery.
