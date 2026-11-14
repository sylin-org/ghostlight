# Office for the web field session and Chrome probes -- 2026-11-13

An agent used Ghostlight for a read-only review of Office documents in a SharePoint Online
library and reported ten difficulties. This record keeps what was reproduced on the installed
stack, what was measured against Chrome directly, and what was not established. It names no
tenant, site, document or person. Screenshots and page content stayed local.

[ADR-0207](../adr/0207-observe-without-changing-the-page.md) owns the decisions that followed.

## Environment

- Windows 11. Google Chrome 157.0.8103.78, the person's own browser and profile.
- Ghostlight in Browser 2.0.1 from the Chrome Web Store. Ghostlight 2.0.2, release channel.
  Later the same day a development build of the working tree was deployed in place through
  `scripts/dev-loop.ps1`; the extension stayed the published 2.0.1.
- No policy: `policy_explain` reported no layers.

## What the report said

In the order of its impact on the task:

1. "Document access could not be verified" on almost every tool on Office pages. Screenshots and
   a window resize kept working. The refusal's own next step failed the same way.
2. The state stayed with a tab across reloads and navigation, including to a page with no frames.
   Only a browser restart cleared it.
3. Waiting about ten seconds on a fresh tab before the first call avoided it.
4. Taking a tab from one Office document to another brought it back.
5. An intermittent "Ghostlight and the browser extension do not agree on this request" on waits.
6. Blank screenshots after wheel scrolling in Word.
7. The page shown squeezed into a strip about 250 px wide, twice, until a window resize.
8. Clicks at computed thumbnail centers that landed between thumbnails.
9. Tabs the page opened were not part of the session.
10. Smaller notes on `repeat`, awaited scripts, `browser_find` limits, ribbon text in reads, and
    coverage facts.

## Reproduced on the installed stack

All of this ran through the installed product and the person's browser.

**The stuck tab (items 1, 2 and 4).** One tab: a library page, then a Word document, then a
PowerPoint deck.

| Step | Result |
| --- | --- |
| Read on the Word document | Succeeded, no wait needed |
| Read on the deck, same tab | "Document access could not be verified. Some document content was unavailable." |
| Ten second wait, read again | Same refusal |
| Same tab to example.com, read | Same refusal, on a page with no frames |
| Fresh tab on example.com, read | Succeeded |
| Fresh tab straight onto the deck, read at once | Succeeded |

The state is per tab and survives navigation. Calling early was not the trigger (item 3 was not
reproduced). After eleven more plain pages in the stuck tab and more than fifteen minutes the
mark was still there, so the back/forward cache does not hold it.

**The version message (item 5).** On a healthy deck tab, a `text_present` wait for text that is
on the page succeeded. The same wait for text that is not on the page, with a 4000 ms timeout,
returned the version-mismatch sentence, every time. On example.com the same kind of wait
correctly returned "Text never appeared".

**Screenshots (items 6 and 7).** Every one of these was a plain viewport capture.

| Tab and page | Capture |
| --- | --- |
| Word in Viewing mode, just after navigation (twice) | Entirely white, while a read returned the ribbon text |
| PowerPoint, fresh tab, first capture | The Office UI in a 300 x 200 box at the top left, white elsewhere |
| Word in Viewing mode, idle (twice) | Correct |

The host page's Office iframe has computed `min-width: 300px` and `min-height: 200px`. A script
run before and after the squeezed capture measured that iframe at 2158 x 1140, the window's
inner size. The page was never squeezed for the person; only the picture was.

**A point below the fold.** On a clean tab with a long plain page and no frames, a full-page
screenshot came back at 1204 x 2400. A wheel scroll at image point (150, 2000) was refused with
the document sentence. The same call at (150, 300), which is on screen, scrolled.

**After the service changes were deployed.** The same tab sequence was repeated against the
development build, with the published extension unchanged.

- The read on the deck in the reused tab succeeded.
- On example.com in that tab every result carried one unavailable document on a page with one
  document. That is the incomplete mark, now tolerated under a policy that admits every document.
- On that tab, read, find, inspect, execute, a settling wait, a page scroll and two PageDown
  presses all succeeded on example.com.
- On Word in Viewing mode in that tab, four PageDown presses moved the status bar from page 1
  of 4 to page 4 of 4, and ten wheel ticks at a view point moved it to page 3 of 4.
- The absent-text wait returned "Text never appeared ... within 4 seconds".
- A `text_absent` expectation could not pass on the marked tab: absence is never confirmed while
  coverage is limited.

**Other observations.**

- The back/forward cache is in use in a controlled tab: after a navigation away and a history
  back, the page's navigation entry still had type `navigate` and no not-restored reasons.
- Leaving an ordinary public page with advertising frames did not mark a clean tab.
- Process diagnostics gave these durations on Office pages: read 12 to 55 ms, scroll 81 ms, key
  press 105 ms, execute 123 to 185 ms, inspect 403 ms, screenshot 226 to 396 ms, navigate 1.3
  to 3.7 s.

## Probes of Chrome

Two throwaway scripts asked Chrome directly. Each starts a headless Chrome 157.0.8103.78 with a
temporary profile and a loopback fixture, and speaks raw CDP. No Ghostlight process takes part.
They are kept in [probes/](probes/). They stand in for the extension, so they are evidence about
Chrome and not tests of the product.

The fixture has three sites on one port (`localhost`, `127.0.0.1` and `[::1]`): a top page with a
full-size iframe that has a CSS minimum size of 300 x 200, an app page in it, and an inner page
in a nested iframe.

**Sessions** (`chrome-sessions-and-capture-2026-11-13.mjs sessions`). The probe keeps the registry
the extension keeps: `Target.setAutoAttach` with `waitForDebuggerOnStart`, `flatten` and an iframe
filter on the root and on every child, one entry per `Target.attachedToTarget`, removed on
`Target.detachedFromTarget`. After the top page left, each scenario listed the sessions still
registered and asked each for `Page.getFrameTree`.

| Scenario | Left registered | Its answer |
| --- | --- | --- |
| Nested frames, then a plain page | The nested session | "Session with given id not found." |
| The same with an unload handler | The nested session | The same |
| Nested frames, then another page with nested frames | The first page's nested session | The same |
| Nested frames plus short-lived cross-site frames, then a plain page | The nested session | The same |
| Nested frames, then a cross-site plain page | The nested session | The same |

In all five, Chrome announced the end of the session attached through the root and never
announced the end of the session attached through that child. Eight more plain pages changed
nothing.

**Capture** (`chrome-sessions-and-capture-2026-11-13.mjs capture`). Three ways of asking for the
same viewport, twice each, with the app frame taking 0, 120 or 400 ms to handle a resize.

| Request | Frame shown at full size |
| --- | --- |
| No clip | 6 of 6 |
| Clip only | 6 of 6 |
| Clip with `captureBeyondViewport: true`, as the extension asks | 5 of 6; once exactly 300 x 200 |

**Geometry** (`chrome-capture-geometry-2026-11-13.mjs`), at device scale factors 1, 1.5 and 2.

- A capture with no clip is the window in device pixels: 1184 x 705, 1776 x 1058 and 2368 x 1410
  for an inner size of 1184 x 705.
- The scrollbar strip is inside the image, on the right. It is on the right for a right-to-left
  page too.
- At 1.5 the visual viewport is a fractional number of CSS pixels wide (1168.67).
- A change made to the top document just before the capture was in the picture 6 of 6 times. So
  was a change made inside a cross-process frame.
- `createImageBitmap` with a source rectangle and resize options, `OffscreenCanvas` and
  `convertToBlob` as JPEG work inside a worker.

**Chrome's source** (`content/browser/devtools/protocol/page_handler.cc`, main branch, read the
same day). With a clip, `CaptureScreenshot` sets device emulation parameters and resizes the view
to the clip for the capture. With `capture_beyond_viewport` it also changes web preferences and
first sets an emulated view of 1 x 1. With neither, it resizes nothing.

## What this established

- A dead nested session stays in the extension's registry, its failed read marks the inventory
  incomplete, and the service refused on that mark. The reproduction, the probe and the code
  agree. The reproduction and the probe ran on Chrome 157.
- The refusal is not specific to Office: any page with a cross-site frame inside a cross-site
  frame leaves a dead session when the tab moves on. It was seen live only on Office.
- A capture asked for with a clip and `captureBeyondViewport` re-lays out the page, and a busy
  cross-process frame is caught squeezed to its CSS minimum or blank. A capture with no clip is
  not affected.
- The version-mismatch sentence on a wait was a spent time budget reaching the extension as a
  timeout of zero.
- A view point that is off screen is refused as a document problem before the code that would
  scroll it into view runs.

## Not established

- The cause of the navigation timeouts in the report (25 to 30 s) and of the hang that followed
  one of them.
- Whether the misplaced thumbnail clicks (item 8) and the lost scroll position come from the
  capture's re-layout. It is plausible and was not shown.
- Why two of my explanations were wrong before the probe settled it: the back/forward cache does
  not hold the dead session, and a reading of Chrome's source suggested the detach event is sent.
  The probe is the evidence; the reading was not.
- Whether Chrome for Testing 152, the build CI pins, behaves the same in either respect.
- How a capture with no clip behaves when the browser window is minimized or fully covered.
