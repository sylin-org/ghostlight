# ADR-0194: Binary screenshot permission

- Status: Accepted; implemented, locally deployed, and verified on Windows
- Date: 2026-10-02
- Supersedes: ADR-0158 Decision 6
- Builds on: ADR-0131, ADR-0162, and ADR-0181

## Decision

The owner chose ordinary screenshots in full-open mode and one binary policy allowance.
Screenshot permission authorizes the complete rendered image, including embedded documents.
It does not mean that Ghostlight has independently admitted semantic access to every frame.

1. `browser.screenshots.enabled` is a registered boolean, enabled by default. Any configured
   authority layer authoring `false` refuses screenshot capture. A lower layer cannot restore
   it, including in observe mode. Invalid authority still refuses work. Existing Read and
   controlled-tab checks remain.
2. Viewport, full-page, target, and magnified captures share this permission. The orchestrator
   admits or refuses the complete operation before dispatch. An allowed capture goes directly
   to the existing capture primitive, without document inventory, partial coverage, or masks.
3. Screenshot pixels include visible embedded content even when separate document grants
   exclude its text, targets, or actions. Operators who cannot permit that disclosure disable
   screenshots. Frame handling and notice settings govern semantic work, not image redaction.
4. Remove mask overlays, iframe style mutation, mask timers, restoration, and mask verification.
   Image bounds, current view validation, tab custody, runtime controls, and content-free audit
   remain. Capturing an image grants no additional permission to act at its coordinates.
5. Historical `masked_regions` receipts remain readable. The stable document wire fields remain
   decodable for predecessor compatibility; new service scopes use `mask: null`. New adapters
   explicitly refuse legacy non-null mask scopes rather than silently returning exposed pixels.
6. Recording admission, document-bound retention, and export checks remain their existing
   contract. This decision concerns screenshots only.

## Prior art

Source reviewed on October 2:

- [Playwright MCP capture](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/tools/backend/screenshot.ts)
  calls normal page or locator screenshots without masking.
- [Chrome DevTools MCP capture](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/src/tools/screenshot.ts)
  uses normal Puppeteer capture, clipping, and image bounds without privacy masks.
- [Playwright library masks](https://playwright.dev/docs/api/class-page#page-screenshot-option-mask)
  are optional locator-selected overlays; the MCP screenshot tool does not use them.

This supports a simple capture primitive. It does not prove that every background capture works
on every platform. The Windows checks below verify capture with the mask path removed.

## Verification

Prove unrestricted and policy-permitted capture without document inventories, binary refusal
for every capture branch before dispatch, organization precedence, original embedded pixels in
real Chrome images, and preservation of separate semantic and coordinate-action restrictions.

[The verification record](../testing/binary-screenshot-policy-2026-10-02.md) records passing
source gates, all 70 real Chrome component checks, and installed Windows acceptance for all four
capture modes through the existing ordinary Chrome connection. No browser restart or extension
reload was needed. Binary refusal was verified in the component lane; the installed acceptance
kept the user's full-open policy. Linux installed behavior is not claimed.

## Stable-baseline delivery amendment (1.3.14)

The owner selected the screenshot fix on published 1.3.13, excluding the browser/workbench trial.
This release keeps production adapter 1.3.12 byte-identical. Its legacy mask preparation remains
dormant because new service captures dispatch without document scopes. The injected runtime
removes mask handlers. The adapter cleanup and explicit legacy-mask rejection described above
remain preserved on the trial source branch and are not prerequisites for this service release.
The trial source verification record is historical evidence; the separate 1.3.14 report identifies
the exact narrowed source, unchanged adapter, candidate, and publication results.
