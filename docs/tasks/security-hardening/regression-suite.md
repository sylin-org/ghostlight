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
document/editor/capture/recording behavior, and real Chromium history interaction. Windows also
runs the native desktop lifecycle journey, including concurrent startup Open requests. It runs all
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
| Draft editing and form preflight | Native transactions retain ordinary/open-shadow controlled drafts, replacement, multiline text, and clearing; fixture drafts remain unsent and Ghostlight invokes submission only when requested; known invalid fields anywhere in a batch prevent earlier edits. Standalone editors support both selectors and handles. | Extension content/worker tests; real MV3 editor and cross-frame cases; installed live journey. |
| Page feedback and script progress | Read feedback follows the resolved tab; overlapping operations cannot animate an unrelated page or clear each other's progress; every terminal path removes its own script spinner. | Orchestrator presentation/dispatch tests, extension routing/renderer tests, and real Chromium visual checks. |
| One native workbench | Concurrent startup Open calls create one responsive native window; minimize/restore preserves one window; close leaves the authority alive; concurrent reopen creates one replacement. Counts include hidden windows. | Windows native desktop journey in the full/process runner and Windows process CI. |

The [process and human-history inventory](regression-process-coverage.md) names individual checks
for H1/H2/H4/H5/H7/H8/C1. Browser and executor additions are retained beside their existing fixtures,
so they run under ordinary test discovery as well as this suite.

The catalog-authority tests exercise 338 cases across all 24 advertised tools and 38 authority
variants, with an independent expected-capability table. They check omitted/exact/missing request
allowlists, minimal configured grants, direct and flow-child execution, and split recording source
and destination grants. Read on a recording source and Write on its destination remain independent.

The matrix includes selector and postcondition variants. Named control lookup and explicit
postconditions declare their Read requirement before work begins; handle/focused input without a
postcondition retains its original capability set. Cross-host typing tests keep Action-only
destinations usable after lookup at a Read + Action source. A refused later observation preserves
the acknowledged action without holding the tab. Standalone file-input tests verify actual file
names, counts, and contents through both selector and handle attachment.

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

The lane writes `.tmp/installed-hardening-evidence.json` with installed executable hashes, named
checks, a `passed` boolean, and every tool invocation, plus a masked screenshot. It invalidates
earlier success before startup and retains failure evidence. Run it after the changed binaries and extension are
actually active. An isolated MV3/native-port shim cannot establish native-host installation, and
an installed live check cannot replace deterministic audit-failure or control-race tests.

## Native workbench regression follow-up (2026-09-07)

The owner subsequently reported two open Ghostlight copies. Native enumeration found two
responsive Tauri workbench windows inside one installed service process. The first Windows
journey reproduced that race on release `57f4e314`: service activation and startup could both
construct a window before Tauri registered either label. The previous process tests did not count
native windows and therefore did not establish this promise.

The desktop now publishes activation only after startup construction and backgrounding. Open
allows a separate 15-second native startup wait; expiration invites retry without diagnosing the
authority or telling the person to stop it. A real authenticated service regression first failed
on the old one-second wait, then passed with presentation attached after two seconds and exactly
one reveal. A short unavailable deadline also remains bounded.

`tests/windows-desktop-journey.ps1` launches the ordinary no-argument authority and bursts eight
authenticated Open requests during startup and after close. It counts real native Tauri windows,
including hidden ones, checks responsiveness, verifies minimize/Open keeps the same native window,
and closes the view while proving the authority remains alive. Three rounds provide 18 checks,
including verification of each actual WebView2 profile under the test's evidence directory.
Runtime, policy, native-host, diagnostics, and WebView state are isolated; cleanup addresses only
owned callers and authorities. This gate runs in the Windows full/process lanes and Windows CI.

Final Windows evidence is
`.tmp/hardening-suite/2026-09-08T01-12-37-972Z-31076/results.json`: all 17 gates pass,
`source_unchanged:true`, source fingerprint
`b6ea519480f040c75c7488739813f0eac5353d3eb3b0addcb1b192928fd09754`.
It includes 526 Rust tests, 207 extension tests, six browser-harness checks, 23 script cases,
68 MV3 cases, and the 18 native checks at
`.tmp/native-desktop-9fc18bd7eeaf4cdda5c1ba89ac9d5508/results.json`.
The native duplicate-window failure was retained at
`.tmp/native-desktop-efc7390e47574ea3956609cd6c6f86ba`.

After the dev-loop swap, `.tmp/installed-native-desktop-evidence.json` verifies the installed
release hash against the isolated build, Ready service, eight concurrent Open calls, and one
responsive restored native window across 20 samples. Existing connector processes and binaries
survived. A fresh installed MCP connector passes policy explanation and live browser tab listing
through the existing native host and extension. The thread's cached MCP transport returned
`Transport closed`; this record makes no successful-reconnection claim for that transport.
STATUS owns the deployed release hash. No extension reload or browser-page mutation was needed.

## Earlier execution record

The earlier Windows run on September 7 passed all 16 gates on unchanged source:

- 525 Rust tests, including the 338 catalog/authority case iterations.
- 207 extension tests and six Chromium-harness fault-injection tests.
- 23 real-engine script cases and 68 real MV3 cases, including five visual regressions.
- Process/reconnect, continuity, provenance, both CLI journeys, policy grammar, workbench surface,
  and real Chromium history interaction.
- Required formatting, Clippy with warnings denied, fresh workspace build, and changed JavaScript
  syntax. The direct `npm test --prefix extension` gate also passed.

The local report is `.tmp/hardening-suite/2026-09-08T00-41-14-186Z-19796/results.json` (UTC path).
It records `passed:true`, `source_unchanged:true`, and source fingerprint
`09b043076efc600b24ccb82286f08baa2df82876770a83fefe308170f2639a1c`.
The browser is Chrome for Testing 152.0.7977.82. `.tmp/h6-browser-evidence.json` records all named
MV3 checks and snapshot hashes. The running/completed visual PNGs were also inspected: the actual
wheel is present during execution and absent after completion.
The earlier 520-test/65-case full pass is retained at
`.tmp/hardening-suite/2026-09-08T00-00-20-637Z-36092/results.json`.

The first complete run failed rather than hiding its defects: an outdated PowerShell synthetic
adapter lacked the H6 document contract, and Windows temporarily locked Chrome's endpoint/profile
files. The PowerShell fixture now proves actual admitted read/capture and byte-exact delivery.
The shared Chromium harness waits for complete readable startup data and bounded owned-process
cleanup; unexpected errors or persistent locks still fail. The final full run passes those lanes.

The installed continuation exposed a pre-existing selector restriction: standalone controls
accepted handles but semantic fills/uploads silently required an HTML form ancestor. The original
fixture remains unchanged, and the corrected shared resolver passes its installed and MV3 checks.
Review also corrected incomplete selector/postcondition capability declarations and exposed the
already-supported postcondition in tool schemas. Tests prove early missing-permission refusals,
retained credentials/submission checks, Action-only landings, and truthful later observation failure.

The final orchestrator was deployed through the dev-loop. Installed and isolated release SHA-256
match `57f4e3147023d606e60ebb4c78d17b719c0b2f78b999b077e9a4482153f88435`; the exact-path process
is Ready, the deploy lock is absent, and both connectors remain unchanged. The owner confirmed the
final extension reload. Later corrections affected the service and tests only.

Installed acceptance passed at `2026-09-08T00:40:45.469Z`, with eight named groups and 50 tool
invocations in `.tmp/installed-hardening-evidence.json`. It verifies retained ordinary/shadow
drafts, effect-free batch preflight, Action-only typing, script/flow execution counts, permitted
parent fields, excluded child content, delivered masked pixels, script refusal, and the public
Sylin framed form's local-only simulation and captures. The masked JPEG was visually inspected.
The fixture now serves its captured image locally and uses an explicit awaited return to inspect
pixels within the existing script-size limit; earlier harness failures were not counted as passes.
The final test tab `tab_fb44d16040ab4df5bb5e1eb644d60257` remains under preserve-tabs. The earlier
Reddit draft was not touched or submitted.

Installed acceptance uses the real MCP connector, service, registered native host, and adapter.
The five visual regressions retain separate actual-render evidence in isolated MV3 Chromium;
human-control races and audit-storage faults remain in their deterministic process/UI lanes.

Windows/Linux CI now includes the missing continuity and all three browser lanes, but remote CI
has not run and no push is authorized. Full Linux desktop/browser verification remains unexecuted
here; the earlier WSL loopback/environment failures remain documented in STATUS. This report
does not close deferred C1 verification, C2/C3 admission, or extension reload bootstrap recovery.

## Tool consolidation follow-up (2026-09-07)

ADR-0162 removes caller restrictions and flow dry-run and retires sequence into the ordinary flow
executor. The catalog has 23 tools. Flow IDs are optional, missing argument objects default to `{}`,
and a parent tab defaults only tab-scoped children. A three-step regression across two controlled
pages proves inherited tab, explicit override, and return to the parent tab. Named references,
Stop/Continue, partial effects, audit storage failures, and human control still use the same seams.

The authority matrix now uses real isolated policy files for admitted and missing-capability cases.
It retains every current catalog variant and equivalent flow child, credentials, draft/submission
distinctions, upload, typing, recording source/destination authority, and H6 coverage. New negative
tests reject obsolete inputs before browser effects, even when supplied in a later child. Legacy
request and sequence receipts remain readable, and the rendered workbench does not blame the
person's rules for historical request denials.

Final full Windows run:

- Report: `.tmp/hardening-suite/2026-09-08T02-30-03-979Z-33624/results.json`.
- Source SHA-256: `3e1aa2e7c989bcc1217f173a5f255ba6a91463ba358315a56966db6898273e50`.
- All 17 gates passed; `source_unchanged: true`.
- 530 Rust tests, 207 extension tests, 18 native desktop/profile checks, six browser harness checks,
  23 script-browser cases, and 68 MV3 frame/editor cases passed, plus the process/CLI/history lanes.
- Changed JavaScript passed `node --check`.

Earlier runs exposed stale tool-count/dry-run assertions and an invalid tab-list test input. The
process negative test also distinguished requested child dispatch from an earlier navigation's
independent landing observation. These were corrected; earlier failed runs are not counted as passes.

The dev-loop deployed only the orchestrator. Installed and isolated release SHA-256 match
`8c2675e6a8ca922f5d137fcf0eaa705b5cc3b41ac8b43fb83bf36d21deb61a2e`; the installed service is Ready.
Connector hashes remain `1271afb9604d0148ef3eb17219cc2d0ac1bc5692d98825fa8a520375b7fb7a6b`
(MCP) and `6c8e85e8d54cfd40d3423bba2bf1d49969ef0290961426eec0bfed064721580a` (browser).

Fresh installed MCP verifies the current catalog, rejection of both restriction fields, both
dry-run boolean values, and the retired tool. Policy explanation remains available after those
invalid calls. Evidence: `.tmp/installed-tool-removal-evidence.json`, with the catalog separately
in `.tmp/installed-tool-removal-catalog.json`.

Installed browser acceptance completed at `2026-09-08T02:33:57Z`: eight groups, 46 calls, passed.
It proves unsent ordinary/shadow drafts, unchanged earlier fields on batch preflight refusal,
typing/clearing, script/flow effect counts, unrestricted embedded read/fill/capture/script access,
and live Sylin framed content with local-only simulated submission and captures. Restricted frame
and pixel-mask proof remains in the real MV3 lane under configured policy; the installed all-open
fixture does not rewrite the person's authority. Evidence: `.tmp/installed-hardening-evidence.json`.
The previous installed receipt was preserved as `.tmp/installed-hardening-before-tool-removal.json`.
The disposable test tab `tab_05231c5669234f61967bd8e976cc7abb` remains open; the Reddit draft was
untouched. Cached clients need a catalog refresh/reconnect. No extension reload is needed, nothing
was pushed/published, and full Linux execution remains outstanding.

## Action detail follow-up (2026-09-07)

At a glance action names now toggle inline hero details. Native mouse, Space, and Enter activation,
receipt refresh, newer activity, nested expansion, focus retention, and Pause-before-tab ordering
are covered in the real bundled Chromium UI. At 1280 and 720 pixels, the expanded panel must fit
inside its row with no horizontal overflow. Both screenshots were visually inspected:
`.tmp/h4-action-details-1280.png` and `.tmp/h4-action-details-720.png`.

Checks caught clipped fixed-height rows and a deferred native toggle losing expansion during a
receipt refresh. Both were corrected before deployment. The first full run also exposed a fake
DOM that overwrote earlier click listeners. It now dispatches to all registered listeners, and
the integration Fix confirmation test passes without changing that product behavior.

Final full Windows run:

- Report: `.tmp/hardening-suite/2026-09-08T03-11-26-679Z-12656/results.json`.
- Source SHA-256: `cb65eabb62cacf9fff25a1f96f8668dbf21fcfdf36b34405ea3d1db7284d745d`.
- All 17 gates passed; `source_unchanged: true`.
- 530 Rust tests, 207 extension tests, 18 native desktop/profile checks, and the existing process,
  browser, script, MV3 frame/editor, and history lanes passed. Changed JavaScript passed syntax checks.

The browser interaction evidence uses isolated Chromium with synthetic workbench events. It does
not claim those clicks occurred inside the installed Tauri window. Full Linux execution remains
outstanding; this follow-up does not reopen or complete deferred admission/verification decisions.

## Linux delivery follow-up (2026-09-08)

The full suite now runs the portable archive regression on both platforms and the shell installer
regression on Linux. Portable tests compare separate packager processes and extracted payload,
owner, timestamp, and mode metadata. Shell tests pin one validated release across CDN asset
transport and reject invalid release/version/checksum inputs. The actual public download is a
separate local integration result, not part of the offline regression.

All 20 Linux gates passed on unchanged source at
`.tmp/hardening-suite/2026-09-08T21-31-40-763Z-113207/results.json`, fingerprint
`d5b65ea751a6d6b00c1b9eb1db7f4ea772d3b0846a8910efe473fabbf137162e`.
The [additional Linux evidence](../../testing/linux-local-acceptance-2026-09-08.md) records the
real defects, actual delivery, installed browser/desktop checks, and package-consumer boundaries.
