# ADR-0194: Binary screenshot permission

- Status: Accepted; source and Windows Chromium component verification pass
- Date: 2026-10-02
- Supersedes: ADR-0158 Decision 6
- Builds on: ADR-0131, ADR-0162, and ADR-0181

## Decision

The owner chose ordinary screenshots in full-open mode and one binary policy allowance.
Screenshot permission authorizes the complete rendered image, including embedded documents.
It does not mean that Ghostlight has independently admitted semantic access to every frame.

1. `browser.screenshots.enabled` is a registered boolean, enabled by default. Any configured
   authority layer authoring `false` refuses screenshot capture. A lower layer cannot restore
   it. Invalid authority still refuses work. Existing Read and controlled-tab checks remain.
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
on every platform. The Windows background stall needs verification with the mask path removed.

## Verification

Prove unrestricted and policy-permitted capture without document inventories, binary refusal
for every capture branch before dispatch, organization precedence, original embedded pixels in
real Chrome images, and preservation of separate semantic and coordinate-action restrictions.

[The verification record](../testing/binary-screenshot-policy-2026-10-02.md) records passing
source gates and all 70 real Chrome component checks. Linux installed behavior is not claimed.
