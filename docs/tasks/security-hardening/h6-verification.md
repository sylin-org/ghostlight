# H6 implementation and verification

Date: 2026-09-07. Decision: [ADR-0158](../../adr/0158-document-access-and-coverage.md).
Implemented locally; not deployed or published. The ledger owns cycle progress.

## Implemented behavior

The common executor browser boundary obtains document identities and routing metadata before
content extraction. The immutable authority snapshot admits each supported document for the
operation's RAWX requirements. The adapter applies the supplied document scope using Chrome's
document-targeted messaging. Locators carry document identity internally; frame-id reuse cannot
revive them. The connectors remain generic and the extension contains no policy evaluator.

`content.frames.handling` accepts `permitted_content`, `complete_operation`, or `complete_page`.
`content.frames.notice` accepts `on_demand`, `when_affected`, or `when_excluded`. Defaults are
permitted content and notice when affected. The policy editor shows both choices and their
effective author. Organization floors cannot be weakened locally.

Every scoped completion carries content-free coverage; retained audit carries the same bounded
facts. Counts are observed maxima across an operation's physical preparation and execution, not
sums of repeated reads. Flow and sequence children retain their own coverage. Human details keep
bounded excluded host names in volatile workbench state only. They are lost on service restart;
durable coverage and authored qualifications remain. No excluded URL, path, label, value, or
locator is copied from document admission into model results or audit.

Screenshots hide excluded embeds before capture and show an authored exclusion mask. They verify
document identity and mask state before and after capture, discard unverifiable images, and
restore the embed's prior styles. Full-page, viewport, target, and magnified captures share this
mechanism. Coordinate input checks the actual document at the point; a mask is not authority.

Unrestricted scripts refuse when document exclusions or unavailable document evidence prevent
bounded admission. All-open scripts keep their browser behavior, including pages with opaque
embeds. This does not sandbox scripts or filter website network requests.

Recordings with host restrictions stop when their admitted document set changes, including an
otherwise permitted navigation. This is deliberately conservative: the adapter cannot grant a
new document authority. Start a new recording after fresh admission to continue. Unrestricted
recordings can follow navigation. Prior frames remain in memory. Export to the client, a file,
or a page input rechecks all recorded sources under current authority. Source evidence has a
32-URL bound; overflow or unsupported sources marks it incomplete instead of forgetting earlier
sources. Incomplete provenance cannot be exported under host-restricted authority. Diagnostic
buffers are cleared across document navigation and revalidated before being read.

## Evidence

- Workspace: 478 Rust tests; formatting and Clippy with warnings denied.
- Extension: 192 tests, including document identity, exclusion before extraction, unavailable
  versus empty observations, point routing, after-dispatch uncertainty, and replay source bounds.
- Real process journey: fresh orchestrator, browser connector, and MCP connector; synthetic
  browser receipts updated to describe and wrap documents. Existing H1-H5 assertions pass.
- Existing script browser journey: 19 real Chromium evaluator cases still pass.
- Workbench surface and isolated Chromium history journeys pass, including the two policy
  choices, organization floors, history expansion, incremental focus/scroll, and scoped recovery.
- `tests/frame-browser-journey.mjs`: 29 checks with Chrome for Testing 152.0.7977.82 on Windows.
  Uses the shipped MV3 worker, content scripts, webNavigation, document-targeted tabs messaging,
  debugger input/capture, recorder, and GIF export. Only native-port discovery is replaced by a
  test pipe on loopback into the real browser connector; real orchestrator and MCP processes
  handle the results. This is not proof of the installed native-host registration or Tauri UI.

The browser journey reads the actual public
[Sylin iframe demo](https://sylin.org/ghostlight/demo/iframe/) and its
[embedded form](https://sylin.org/ghostlight/demo/iframe/form/). Both use sylin.org, so an additional
copy of those responses is served on localhost and 127.0.0.1 to permit independent host rules.
The copy changes asset resolution and the embed address and adds a parent text field for batch
preflight assertions. It retains the form's local simulation; nothing is submitted remotely.

The 29 checks cover all nine handling/notice combinations, permitted and excluded reads, negative
search and waits, size limits, mixed Read/Write grants, batch preflight, permitted parent writes,
stale embedded targets, every screenshot shape, a mask mutation that discards output and restores
styles, blocked coordinates, unavailable documents, script refusal, recording interruption and
prior-frame export, source reauthorization on replay export, successful embedded form fill and
local submission, and actual audit origin/value/selector minimization. The masked full-page image
was visually inspected: the form region shows `Excluded by policy`; surrounding Sylin content
remains legible. Human-host disclosure is also checked through the Rust workbench facade.

The journey emits `.tmp/h6-browser-evidence.json` with source response SHA-256 hashes and named
checks, and `.tmp/h6-masked-sylin.jpg` for visual review. It creates and removes its own profile and
test processes. Set `GHOSTLIGHT_TEST_BROWSER` to Chrome for Testing and `GHOSTLIGHT_BIN_DIR` to the
fresh build if using a different target directory. No existing browser profile or registration
is modified. Other platforms, installed native-host transport, and exhaustive browser race
coverage remain untested. Matching redirected or ambiguous embed geometry may refuse capture.
