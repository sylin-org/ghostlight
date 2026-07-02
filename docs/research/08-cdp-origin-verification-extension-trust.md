# CDP Origin Verification & the Extension Trust Boundary

**Date:** 2026-07-01 · **Track:** Security · **Source:** research agent (verbatim report)

> The single most load-bearing *technical* finding of the discovery. How to trust "current URL"
> at the CDP layer, why a tampered extension is unmitigable while it's the transport, and the
> per-frame / commit-signal correctness rules any domain enforcement must follow.

## Verdicts up front
- **(a) Most trustworthy CDP origin signal:** the main-frame **`Page.frameNavigated` event's
  `securityOrigin`** (browser-process, computed at navigation commit), cross-checked against
  `Target.getTargetInfo.url`, correlated by `loaderId`/`frameId`. **Never** trust
  `Runtime.evaluate` of `location`/`window.origin` (renderer/page-controlled, spoofable).
- **(b) The tampered-extension problem is NOT solvable while the extension is the CDP transport.**
  `chrome.debugger` puts the extension fully in the trust path: it serializes every CDP
  response, so it can forge a tab's URL. No binary↔extension message signing fixes this, because
  the extension is the *source* of the data. Ground truth requires the binary to open its **own
  CDP channel bypassing the extension** (`--remote-debugging-pipe` best), which forces the binary
  to *launch* Chrome and cannot be bolted onto a user's already-running normal Chrome. The two
  product goals ("attach to the user's existing session via an extension" vs. "extension-
  independent ground truth") are **fundamentally in tension.**
- **(c)** Treat the extension as untrusted; harden per-frame commit-event tracking, host
  canonicalization, and fail-safe containment; accept the residual risk explicitly.

## 1. CDP origin signals ranked by trustworthiness
The Chromium trust boundary is the **browser process**. Browser-process signals computed at
navigation-commit are authoritative; renderer-sourced/page-observable values are spoofable
(`RenderFrameHostImpl::CanCommitOriginAndUrl` kills a renderer that claims an origin it isn't
locked to).

| Rank | Signal | Process | Page-spoofable? | Notes |
|---|---|---|---|---|
| **1** | **`Page.frameNavigated` → `Frame.securityOrigin` (+url, loaderId)** (main frame) | Browser | **No** | Fires only on real cross-document commit. **Primary trust anchor.** |
| 2 | `Target.getTargetInfo`/`getTargets` → url | Browser | Origin: No; path moves w/ same-origin `pushState` | Best for current-state polling; **no `securityOrigin` field** (URL only). |
| 3 | `Page.getNavigationHistory` → current url | Browser | Origin: No | `userTypedURL` = address-bar intent, not committed identity. Don't use for authz. |
| 4 | `Network.responseReceived` → response.url + loaderId | Browser | No | Trustworthy post-redirect "what was fetched"; join via loaderId/frameId. |
| 5 | `Security.visibleSecurityStateChanged` | Browser | No | TLS corroboration only (no origin). |
| 6 | `Page.navigatedWithinDocument` → url | Browser (event) | Path: page-controlled | Same-document (pushState/hash). Origin unchanged by construction; don't re-derive origin from its URL. |
| 7 | `Runtime.evaluate` (isolated world) of location/origin | Renderer | Resists naive spoof; still renderer | Weaker than any browser signal. |
| 8 | `Runtime.evaluate` (main world) of window.location/origin | Renderer | **YES, actively spoofable** (`Object.defineProperty`/Proxy) | **Never use for authorization.** |

- **Adversary (malicious page):** cannot touch signals 1-5; only levers are 6-8 (path via
  pushState is same-origin-locked; JS location spoofing is renderer-only). An origin allowlist
  keyed on browser-process commit signals is immune.
- **Adversary (tampered extension):** can forge **all** signals 1-8 (see §2).

## 2. Is binary-side verification possible without direct CDP? No, not through `chrome.debugger`
The extension receives CDP results in its own JS context; the binary sees only what the extension
re-serializes over native messaging. A tampered extension forges any response. **Message signing
does not help**: the extension is the origin of the data. Confirmed by *Chrowned by an
Extension* (EuroS&P 2023). The only fix: give the binary its own CDP channel that bypasses the
extension.

| Transport | Trust path | Ground truth for binary? | Exposure |
|---|---|---|---|
| `chrome.debugger` (extension-mediated) | **Extension** | **No**, forgeable | Extension only |
| `--remote-debugging-pipe` | Launching parent process only (inherited fd 3/4) | **Yes** | Minimal, no port/network/DNS-rebind. Playwright default. |
| `--remote-debugging-port` | Any local process (post-M110 origin-restricted) | Yes | Broadest: localhost port, historical DNS-rebind→UXSS, cookie-theft magnet. |

Hard constraints: both flags are **launch-time only** (no runtime toggle; launching against a
running same-profile instance silently ignores the flag). **Chrome 136 (Mar 2025):** the flags
are **ignored on the default user-data-dir**: you must pass a non-default `--user-data-dir`,
which means you're **not** on the user's real authenticated session, defeating the premise.
Corporate policy often blocks `--remote-debugging-*`.

**Net:** a trusted oracle is only achievable if the **binary launches Chrome** (pipe mode,
non-default profile). For the installed-browser + extension deployment, the tampered-extension
threat is **inherent and unmitigable at the CDP layer**. Practical mitigation: pin the extension
ID/hash; where possible run a parallel pipe-mode oracle and treat any divergence as a critical
audit event / denial.

## 3. Prior art: what to trust for "current URL"
The recurring ecosystem failure is **trusting the requested URL** instead of the **committed CDP
event**.
- **Playwright (reference model):** `frame._url` is a pure projection of CDP commit events
  (`frameCommittedNewDocumentNavigation` from `Page.frameNavigated`), never from the navigate
  command.
- **Puppeteer:** same, with an explicit "push beats pull" rule (ignores pulled `getFrameTree` in
  favor of the pushed `frameNavigated`). Documented race: under CPU load it can *drop*
  `navigatedWithinDocument` (2.2%→16.6% miss), leaving `page.url()` stale, which argues for using
  browser-side `Target`/`targetInfoChanged` as a re-verification authority for audit-critical
  checks.
- **browser-use (cautionary tale):** `SecurityWatchdog` enforces on the **requested string, not
  the committed URL**, has **no `navigatedWithinDocument` subscription** (SPA/redirect blind),
  checks only the **top frame**, and matches on `urlparse().hostname` (string, not
  `securityOrigin`). Good parts to port: fail-safe containment (navigate offender to
  `about:blank`; close tabs) and robust `_is_ip_address()` (NFKC + IDNA-dot + `inet_aton` to
  defeat decimal/hex/octal/homograph IP encodings).
- **CDP-proxy interception** (`zackiles/cdp-proxy-interceptor`, `henu-wang/chrome-mcp-proxy`):
  block/rewrite specific CDP methods in-flight; the natural place for allowlist enforcement. Our
  binary is exactly this proxy, but over typed native-messaging commands (easier to audit than
  raw CDP JSON).

## 4. SPA / pushState and the origin allowlist
`history.pushState`/`replaceState` are **same-origin-restricted** (cross-origin throws
`SecurityError`); they change path/query/fragment only, **never origin**. CDP: same-document nav
fires `Page.navigatedWithinDocument` (no `securityOrigin`); cross-document fires `frameNavigated`
(with `securityOrigin`, `loaderId`). **Conclusion:** an **origin-keyed** allowlist need NOT
re-check on pushState: origin only changes via a real cross-document navigation. (A **path-keyed**
allowlist *does* need to watch `navigatedWithinDocument.url`, treating that path as page-
controlled.)

## 5. Iframes / cross-origin subframes
Every frame has its own `frameId`/`url`/`securityOrigin`. A subframe navigating to a **disallowed**
origin fires its own `frameNavigated` and **does not change the top-frame URL**, so a
**top-frame-only allowlist misses subframe origins** (browser-use's gap). **Enforce per-frame**,
matched by `frameId`; snapshot with `Page.getFrameTree` on attach. Cross-origin iframes are OOPIFs
but their `frameNavigated` still routes through the parent page target's event stream: one
`Page`-domain listener sees all subframe origin changes. **Opaque/special schemes, key on
`securityOrigin`, never the URL string:** `data:`/sandboxed frames get a fresh opaque origin
(reported `"null"`) → **deny by default**; `about:blank`/`javascript:`/same-origin `blob:` inherit
the embedder's real origin (the reported `securityOrigin` already reflects inheritance).

## Concrete recommendations for browser-mcp
1. **Anchor policy on browser-process commit signals.** Track a per-target, **per-frame**
   committed-origin state machine driven by `Page.frameNavigated.securityOrigin` (+ `loaderId`).
   Cross-check `Target.getTargetInfo.url`. Never authorize on `Runtime.evaluate` of location.
2. **Subscribe to both `Page.frameNavigated` and `Page.navigatedWithinDocument`;** enforce against
   the committed URL, not the `navigate` argument. (Fixes browser-use's redirect/SPA blind spots.)
3. **Enforce at three points:** pre-navigation (the `navigate` tool), post-commit (real
   `frameNavigated`), and new-target creation (`Target.createTarget`/tab-created, a common
   bypass).
4. **Fail-safe containment on violation:** navigate the offending frame to `about:blank` / close
   the tab; never crash the session.
5. **Per-frame enforcement + opaque-origin deny-by-default.** Match on CDP `securityOrigin`, not
   `urlparse().hostname`.
6. **Robust host canonicalization** (NFKC + IDNA-dot + `inet_aton`) and explicit glob-type
   classification (host vs full-URL vs path).
7. **On the tampered-extension threat, be explicit in the design:** unmitigable at the CDP layer
   while the extension is the transport. If the threat model requires defending against a tampered
   extension, the binary must launch Chrome in `--remote-debugging-pipe` with a non-default
   `--user-data-dir` (accepting it no longer rides the user's real session). Otherwise, pin the
   extension ID/hash, treat it as trusted-by-necessity, and document the residual risk. **This
   tension (existing-session attach vs. extension-independent ground truth) is the central
   architectural trade-off; call it out in SPEC §5 and the threat model.**

## Sources
- [Page](https://chromedevtools.github.io/devtools-protocol/tot/Page/) ·
  [Target](https://chromedevtools.github.io/devtools-protocol/tot/Target/) ·
  [Chromium compromised-renderers](https://chromium.googlesource.com/chromium/src/+/master/docs/security/compromised-renderers.md)
- [chrome.debugger](https://developer.chrome.com/docs/extensions/reference/api/debugger) ·
  [remote-debugging security / M136](https://developer.chrome.com/blog/remote-debugging-port) ·
  [Chrowned by an Extension (EuroS&P 2023)](https://arxiv.org/abs/2305.11506)
- [browser-use #3153](https://github.com/browser-use/browser-use/issues/3153) ·
  [puppeteer #10405](https://github.com/puppeteer/puppeteer/issues/10405) ·
  [Playwright Frame](https://playwright.dev/docs/api/class-frame)
