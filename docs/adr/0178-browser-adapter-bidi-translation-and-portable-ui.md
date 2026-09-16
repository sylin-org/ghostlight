# ADR-0178: Browser Adapter BiDi Translation and Portable UI

Date: 2026-09-16. Status: Accepted.

Builds on ADR-0054, ADR-0112, and Dossier 19.

## Context

Ghostlight's core architectural premise relies heavily on the `chrome.debugger` API (which proxies the proprietary Chrome DevTools Protocol) to achieve high-fidelity automation in the active user session. As we move to support other browsers (like Firefox and emerging engines), we face two hard roadblocks:
1. No other browser family exposes an extension-bound debugger API.
2. The industry has decisively standardized on W3C WebDriver BiDi as the cross-browser automation protocol (with Firefox retiring CDP entirely in 2025).

Simultaneously, the visual layer of Ghostlight (the "Glass"—narrations, effects, borders, overlays) is tightly coupled to the Chromium extension's `content.js`, making it non-portable.

We need an architecture that decouples the product's semantic intent from the vendor's browser engine, while preserving Ghostlight's fundamental user promise: **zero-configuration automation within the interactive user's established session.**

## Decision

1. **Standardize on WebDriver BiDi as the Internal Rosetta Stone**
   - The Orchestrator will no longer invent proprietary internal `BrowserOperation` payloads. Instead, it will use the standardized W3C WebDriver BiDi JSON-RPC schema to represent semantic browser intent (e.g., `input.performActions`, `script.evaluate`).
   - By adopting BiDi internally, the Rust backend remains engine-agnostic and future-proof.

2. **The Translation Layer for Chromium (Protecting the User Experience)**
   - To use WebDriver BiDi natively on Chromium, the browser *must* be launched with a `--remote-debugging-port`. This directly violates ADR-0054 (zero elevation / seamless install) because it requires Ghostlight to hijack the browser launch sequence.
   - **Decision:** The Chromium extension will *not* be stripped of `chrome.debugger`. Instead, `crates/bridge` (or a dedicated `bidi-to-cdp` module) will act as a real-time translation layer. 
   - The Orchestrator emits BiDi JSON-RPC -> The Bridge translates BiDi to CDP -> The payload travels over Native Messaging -> The Chromium extension executes the CDP via `chrome.debugger`.
   - This ensures full BiDi compliance for the core Ghostlight logic while preserving the magical "just install the extension" onboarding flow for Chromium users.

3. **Portable "Glass" via Runtime Injection**
   - The Ghostlight visual overlay will be extracted from Chromium-specific content scripts into framework-agnostic Web Components (e.g., `<ghostlight-overlay>`).
   - Instead of packaging this UI in the browser extension, the JavaScript will be compiled into the Rust binary.
   - The Rust Orchestrator will use BiDi's `script.addPreloadScript` (or CDP's `Page.addScriptToEvaluateOnNewDocument`) to inject the Web Components directly into the browser at runtime.
   - **Impact:** The extension is reduced to a "dumb shell" (just a background service worker routing Native Messaging). All UI updates, animations, and bug fixes deploy instantly with the Rust binary, entirely bypassing the Chrome Web Store review cycle. Visual presentation commands become simple `script.evaluate` calls against the injected components.

## Consequences

*   **Positive:** We gain a future-proof, W3C-standardized internal protocol. Firefox (Gecko) integration becomes a straightforward matter of routing BiDi commands to its `--marionette` port, fulfilling the "Hybrid" model outlined in Dossier 19.
*   **Positive:** The visual UI code is written once and shared flawlessly across any browser engine via standard DOM/CSS components.
*   **Negative/Cost:** We take on the significant engineering burden of mapping BiDi commands to CDP commands within our Rust codebase for the Chromium translation layer. We cannot use a raw BiDi socket for Chromium without breaking our core onboarding premise.
