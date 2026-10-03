# Binary screenshot permission verification -- 2026-10-02

## Scope

ADR-0194 replaces screenshot redaction with one binary policy allowance.
`browser.screenshots.enabled` defaults to true. False from either authority layer refuses
viewport, full-page, target, and magnified captures before a browser capture command.
Read and controlled-tab admission remain. An allowed image includes visible embedded content;
separate frame text and action grants do not redact pixels or acquire broader action authority.

The source removes mask overlays, iframe style mutation, mask timers, restoration, and
verification. Historical document-wire fields and receipt counts remain decodable. A new adapter
refuses legacy non-null mask scopes. Recording boundaries are unchanged.

## Source checks

- Rust formatting and strict workspace Clippy pass.
- All 629 workspace tests pass, including new default allowance, organization precedence,
  invalid setting, complete capture without document inventory, and refusal for all four branches.
- All 410 extension tests pass. The real worker capture function returns original browser bytes
  without document scope work; failed capture restores presentation and releases its debugger lease.
- Workbench surface checks pass. The policy editor exposes the binary screenshot restriction.
- Changed JavaScript syntax checks pass.
- The process journey passes against freshly built binaries, including reconnect, screenshots,
  region chaining, recording, and audit recovery. Repository integrity and offline public-surface
  checks pass; no public version or compatibility metadata is restamped.

## Real Chromium component evidence

The Windows frame journey uses freshly built workspace executables from the explicitly supplied
`.target-dev-loop/debug` directory, the source MV3 worker, and Chrome for Testing
152.0.7977.82. All 70 checks pass.

The delivered full-page JPEG contains the deliberately marked pixels of the embedded document
whose semantic Read access is excluded. Viewport, target, and magnified captures return complete
images. Disabling screenshot permission returns no image and dispatches no capture primitive.
Restoring permission resumes capture without iframe style mutation. Screenshot coordinates still
cannot perform an action in the excluded document. Existing recording, script, form, cancellation,
and audit boundaries pass in the same journey.

This fixture replaces native-port discovery with a loopback pipe to the real browser connector.
It uses a disposable profile and independently checked Sylin fixture content. It proves Windows
component behavior, including the inactive screenshot cases that previously failed mask expiry.
It is not installed ordinary-profile acceptance or Linux browser evidence. The unrelated Linux
background pointer focus issue remains outside this change.

## Delivery

Unpublished source follow-up. Public 1.3.13 artifacts and the Chrome Store adapter remain unchanged.
Any later publication must use a new immutable service version.
