# Editor incident and recovery, 2026-09-07

The owner reported a refused unsent Reddit draft, then requested a live reproduction with
`browser_fill_form` on a specific thread, without activating Send or Comment.

## Evidence before the fix

- Receipt `invocation_97a374b79b4e49c2b24fe4ad6e6f9789` records `browser_fill_form`, requirements
  `read + write`, no configured policy layers, and a request-restriction capability denial.
  Its summary incorrectly describes session permissions. Three matching refusals trigger session
  attention; a later `policy_explain` call is then refused by runtime attention.
- Fresh installed CLI policy explanation confirms all four capabilities available and zero layers.
  No owner policy is changed to reproduce or repair the issue. Request restrictions are allowlists,
  independent of whether the user's larger task is a draft or submission.
- After deploying `00b44646`, inspection first reports an old adapter. The owner reloads the
  unpacked extension. Inspection of the already-open test tab then reports no reachable documents.
  Refreshing only that disposable page restores content-script receivers. Do not automatically
  refresh user pages: they can contain unsaved drafts.
- Inspection advertises collapsed and CSS-hidden editor controls without a hidden state. These
  targets correctly fail physical visibility checks, but their metadata makes selection misleading.
- After opening Reply, receipt `invocation_01c721ca97a448d58192b53dd0606ee1` reports one field filled
  and `submitted:false`. The screenshot and read-only inspection of the visible editor show it
  empty. Direct `textContent` assignment followed by generic synthetic input is discarded by the
  controlled rich editor. Git blame locates that implementation in `bf4f4724a` from August 10;
  it predates the hardening epic.

## Repair

The extension replaces rich-editor text through Chromium's editing transaction, selecting only
the named editor's contents. Fill, targeted clear, and focused clear share that mechanism. Native
editing emits the appropriate input event; no duplicate generic input/change follows it. Ordinary
inputs, selects, credentials, read-only and disabled checks retain their existing boundaries.
Inspection reuses the existing composed-visibility predicate to mark hidden controls accurately.

The orchestrator names caller restrictions in refusals and reports the complete operation
capability requirements. Form-fill descriptions explain `read + write` even for drafts. Configured
policy denials remain distinct. No limits are relaxed and no request is corrected or replayed
automatically. Policy explanation remains available for diagnosing attention and human holds,
without browser access or clearing those controls; ADR-0136 records that amendment.

Targeted and focused typing also checked Write at the landing even though their declared
requirement is Action. That could refuse after typing was already applied. Both paths now retain
their canonical operation requirements through landing checks. Regression cases retain real
destination denials and their applied-effect truth, while permitting Action-only typing correctly.

## Validation

All required gates pass: formatting, workspace Clippy with warnings denied, 513 Rust tests,
195 extension tests, changed JavaScript syntax, ASCII/whitespace checks, and fresh-build process
and CLI journeys. The adapter's real Chromium fixture first
proves that the previous fill is discarded, then checks replacement, multiline text, empty clearing,
and ordinary/open-shadow editors. It verifies no sibling editor change or submission. The fixture
runs through the real MCP/service/relay/MV3 journey with test native-port discovery, distinct from
the installed native-host Reddit test. Thirty-one Chromium checks and 195 extension tests pass.

After the owner reloaded the repaired unpacked adapter, the installed native-host Reddit test
passed. Inspection now distinguished hidden controls from the one visible reply editor. Actual
`browser_fill_form` receipt `invocation_15e7726ffef944268b4db02b7e171ca5` records one field filled
with `submitted:false`. A separate read-only page-script observation returned the exact synthetic
draft from the visible editor; a screenshot independently confirmed it. No Send/Comment control
was activated. The fake reply remains unsent in the disposable test tab for the owner's review.
This is installed browser evidence, not only the isolated discovery-shim fixture.

The hotfix orchestrator was then replaced through the dev-loop with both connectors left running.
The installed executable matches the isolated release build SHA-256
`f03440c7beb28323d68bb8b3b8a5803d5a8db7b73225bffc235682f09676f243`, and the deploy lock is removed.
A fresh installed MCP connector produced three deliberate capability refusals in its own test
session, each naming `restrict_capabilities` and required Read. The third raised session attention.
`policy_explain` then succeeded (`invocation_072831daec7b4da4b215fcf931cb996e`), and a subsequent
browser request still required attention. No browser effects or global control changes occurred
in that verification. The incident fixes accompany this commit; nothing was pushed or published.

The original installed deployment's orchestrator SHA-256 is
`7cd382c31b43dad321ee1b7b1115be03363d3e8c2189703b512bea879fa681a0`;
MCP connector SHA-256 is `1271afb9604d0148ef3eb17219cc2d0ac1bc5692d98825fa8a520375b7fb7a6b`.
The unchanged browser connector is
`6c8e85e8d54cfd40d3423bba2bf1d49969ef0290961426eec0bfed064721580a`.

## Remaining limits

Extension reload does not bootstrap content scripts into existing documents. A future recovery
fix needs idempotent, document-bound bootstrap before work; it must not replay a possibly delivered
mutation or reload pages silently. The current document-unavailable recovery wording also needs
typed receiver-failure evidence to avoid suggesting another inspection when receivers are absent.
Full Linux desktop/browser verification and the interrupted installed Sylin matrix remain owed.
