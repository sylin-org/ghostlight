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

## Installed local acceptance

The owner requested local deployment after source commit `d06892f9`. The existing Windows
development loop built that clean worktree's manifest in release mode and replaced only the
orchestrator in the existing `target/release` installation. A process-scoped Cargo wrapper selected
the clean manifest while retaining the original checkout's build and live directories. The original
dirty checkout was not changed. The built and installed executable SHA-256 values match:

```text
CDCFB3272EE60DDB21D42B0A061745E0058E430E1DC9760D21FC34C7A09DBB31
```

The installation selection retained its serving, state, and policy directories. The replacement
service is running; doctor reports the existing native-host registrations current and browser
readiness Ready. The independently launched ordinary Chrome process remained running. Neither
connector nor the installed extension was replaced, and no browser restart or extension reload
was performed.

The current Codex MCP connection opened one disposable loopback fixture in that ordinary browser.
All-open authority was confirmed without changing policy. Background captures returned JPEGs:

| Capture | Image size |
| --- | --- |
| Viewport | 1409 x 1558 |
| Full page | 1409 x 1800 |
| Target | 228 x 56 |
| Magnified iframe region | 2400 x 1200 |

Decoding the delivered viewport JPEG in the fixture returned RGBA `[255,0,254,255]` at the embedded
document's magenta marker. Before and after capture, the iframe had no inline style, visibility
was visible, opacity was 1, and no mask overlays existed. Magnification passed with a fresh view;
an earlier view invalidated by intervening captures was refused as stale. The test tab remains
open because the installed preserve-tabs interlock refused closure. The fixture server was stopped.

Bounded receipts are saved locally in `.tmp/local-capture-evidence.json`. This is installed Windows
acceptance for permitted screenshots. Binary refusal was verified in the real Chromium component
lane above; this installed check did not change the user's full-open policy or attest Linux behavior.

## Delivery

Unpublished source follow-up. Public 1.3.13 artifacts and the Chrome Store adapter remain unchanged.
Any later publication must use a new immutable service version.
