# Screenshot-only 1.3.14 -- 2026-10-02

The owner authorized publication and selected the screenshot fix on published stable 1.3.13.
The earlier browser/workbench trial remains outside this release. Production extension files are
byte-identical to 1.3.13 and adapter 1.3.12. The existing injector carries the current service runtime.

The service owns default-enabled binary screenshot permission and dispatches complete captures
without document inventory or masks. Tab Read checks, text/action authority, coordinate admission,
recording source restrictions, fixed install paths, and connector contracts remain.

## Verification

The narrowed source passes formatting, strict workspace Clippy, all 570 Rust tests, all 295
extension tests, 11 npm launcher tests and four MCPB tests. JavaScript syntax, repository integrity
and offline public-surface checks pass. Fresh workspace binaries were built in the explicitly
selected `.target-dev-loop/debug` directory.

Pinned Windows Chrome 152.0.7977.82 passes all 70 frame/MV3 checks through the byte-identical public
1.3.12 adapter, including original embedded pixels, complete captures under every handling mode,
binary screenshot refusal without dispatch, permitted target/viewport/full-page/magnified capture,
and separate frame coordinate-action refusal. The fresh-binary process journey passes reconnect,
capture and region chaining, recording and audit recovery. This is component evidence with a test
native pipe; the installed ordinary-browser check above used the prior trial source.

The candidate workflow runs the same frame journey on Linux before native package builds. Local
`cargo audit` is unavailable on this machine; dependency audit and license/source checks use the
existing CI policy tools. Native candidate, provenance and public delivery verification are in progress.
Initial candidate run 37082954269 passed dependency policy and Linux source/process checks but
stopped at an obsolete PowerShell CLI assertion that both read and capture use document admission.
The CLI capture succeeded and delivered the expected JPEG bytes. The corrected test preserves
document admission for read and requires direct dispatch for capture. That failed candidate is
not publication evidence; a fresh candidate must pass the corrected exact source.
The corrected PowerShell CLI journey passes locally. The dedicated installed Linux governance
fixture also replaces its predecessor mask expectation with original embedded pixels and binary
refusal/restoration; its syntax is checked, but that installed lane was not run on this Windows host.
The earlier trial-source component and local installation evidence is preserved separately and
does not attest this narrowed release. No public 1.3.14 delivery is claimed yet.

## Installed stable-baseline service

The existing development loop built the orchestrator from this release branch and replaced only
the selected service at `E:/repo/github/sylin-org/ghostlight/target/release/ghostlight.exe`.
The installed command reports 1.3.14. Chrome PID 20572 and browser/MCP connector PIDs 9540/30964
survive; the new service PID is 24076. Selection, policy and history directories are preserved.
The ordinary browser reconnects without a browser restart or extension reload.

Actual MCP navigation opens a synthetic localhost fixture containing a separate-origin magenta
frame. Viewport and full-page capture succeed at 1424x1558 and visually include its original
pixels. Magnification succeeds at 2400x960 using the current full-page view. An earlier attempt
used the superseded viewport view and correctly returned `stale_view`; that is not capture
failure with a current handle. The preserve-tabs interlock refuses fixture closure and is
respected; the temporary HTTP server is stopped. No owner policy is edited for this smoke.
Binary false and all four capture modes are covered by the separate 70-check component suites.

Candidate run 37083577255 passes source, dependency policy, both native package builds and both
Linux package lifecycles, but final assembly refuses the unchanged 1.3.13 MCPB source manifest.
The bundle manifest and Linux manual version headers are corrected to 1.3.14. The existing
public-surface source check now also compares the npm and MCPB manifest versions with Cargo,
so this demonstrated packaging mismatch fails before native builds. That failed run is preserved
as failed evidence and cannot be published. A new candidate will bind the corrected source.
