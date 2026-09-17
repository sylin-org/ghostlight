# ADR-0179: Hub-and-Spoke Browser Adapters and Firefox Registration

Date: 2026-09-16. Status: Accepted.

Builds on ADR-0050, ADR-0061, ADR-0084, ADR-0114, ADR-0178, and Dossier 19.

## Context

ADR-0178 established W3C WebDriver BiDi as Ghostlight's internal Rosetta Stone and extracted the presentation layer ("Glass") into runtime-injected Web Components, turning browser extensions into dumb shells.

To support Firefox (Gecko) alongside Chromium browsers in a unified, vendor-agnostic architecture, the orchestrator needs to treat browser adapters polymorphically in a hub-and-spoke model. 

1. **Protocol Uniformity at the Native Boundary:** Chromium and Firefox native messaging protocols are wire-identical: both communicate via 32-bit native-endian length-prefixed JSON frames over `stdin`/`stdout`. The same `ghostlight-browser-connector` relay binary can serve both browser families without modification.
2. **Platform Discrimination:** When an adapter connects, the orchestrator needs to know which engine family it is addressing so it can bind the connection to the correct engine adapter and translation pipeline.
3. **Host Registration:** Firefox uses different native messaging host directories and manifest requirements than Chromium:
   - On Windows, Firefox reads `HKCU\Software\Mozilla\NativeMessagingHosts\org.sylin.ghostlight`.
   - On Linux, Firefox reads `~/.mozilla/native-messaging-hosts/org.sylin.ghostlight.json`.
   - Firefox requires an `allowed_extensions` array in the manifest containing WebExtension IDs (e.g. `ghostlight@sylin.org`), whereas Chromium requires `allowed_origins`.

## Decision

1. **Typed Platform Discriminator in Browser Handshake**
   - Add a `platform` field to `BrowserFrame::Hello` using a typed `BrowserPlatform` enum (`ghostlight/chromium`, `ghostlight/gecko`).
   - If `platform` is omitted (older extensions), it defaults to `BrowserPlatform::Chromium` for complete backward compatibility.

2. **Hub-and-Spoke Browser Adapter Model in Orchestrator**
   - The Orchestrator core (governance, policy, workspace aggregate, execution loop in `crates/orchestrator/src/work/`) remains vendor-agnostic, issuing semantic commands and WebDriver BiDi operations.
   - Incoming browser connections are paired with an engine-specific adapter based on the reported `BrowserPlatform`.
   - The `ChromiumAdapter` translates BiDi commands to CDP and dispatches them over Native Messaging to the Chromium extension.
   - The `GeckoAdapter` passes BiDi commands natively to Firefox's WebDriver BiDi endpoint and utilizes the Firefox dumb shell for window/tab identity and presentation.

3. **Unified Native Host Registration for Firefox**
   - Extend `NativeHostRegistry` in `crates/orchestrator/src/install/native_host.rs` to include Firefox as a first-class supported browser alongside Chrome, Edge, Brave, and Chromium.
   - Support Firefox on Windows via `Software\Mozilla\NativeMessagingHosts` pointing to the Ghostlight manifest.
   - Support Firefox on Linux via `~/.mozilla/native-messaging-hosts/org.sylin.ghostlight.json`.
   - Update `HostManifest` to include both `allowed_origins` (for Chromium) and `allowed_extensions` (`["ghostlight@sylin.org", "ghostlight-dev@sylin.org"]` for Firefox), allowing a single unified manifest on Windows and valid manifests across all platforms.
   - Add native, Snap, and Flatpak package detection for Firefox (`org.mozilla.firefox`).

## Consequences

* **Positive:** A single running Ghostlight service instance can simultaneously drive workspaces in Chromium and Firefox without separate builds or conflicting configurations.
* **Positive:** The native messaging bridge executable (`ghostlight-browser-connector`) remains universal and requires no browser-specific fork.
* **Positive:** The installation CLI (`ghostlight install`) automatically discovers and registers Firefox alongside Chromium browsers.
* **Positive:** Existing Chromium extensions remain 100 percent compatible with the updated handshake via default fallback.
