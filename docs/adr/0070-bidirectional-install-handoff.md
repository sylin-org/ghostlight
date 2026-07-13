# ADR-0070: Bidirectional, state-aware installation handoff

- Status: Accepted
- Date: 2026-07-12
- Amends: ADR-0015 (self-registering installer) and ADR-0067 (client installation)
- Builds on: ADR-0065 (one stack)

## Context

Ghostlight has two deliberately separate installation halves:

1. the service, relay, native-messaging host, and MCP-client registration; and
2. the user-visible Chromium extension.

Chromium does not allow the CLI to silently install the extension. That security boundary is good,
but it leaves users responsible for discovering the missing half. The existing extension already
handles one direction: on its first installation it opens a Ghostlight walkthrough that explains
how to install the service. The reverse direction is weaker: `ghostlight install` prints a generic
instruction to load an unpacked extension.

The intended experience is that whichever half the user encounters first leads directly to the
other. Installation should feel like one product crossing an intentional browser boundary, not two
unrelated packages.

## Decision

### 1. Both halves lead to the other

- Extension first: the extension's existing one-time `onInstalled` page explains and runs through
  service and client registration.
- Service first: a successful explicit `ghostlight install` opens the canonical Ghostlight
  extension-install page.

The canonical page is controlled by Ghostlight rather than hard-coding one store. It can present
the Chrome Web Store listing when live, an honest manual fallback while publication is pending, and
the correct path for another supported Chromium browser later.

### 2. Opening is state-aware and respectful

The installer opens the page only when all of these are true:

- this is a real install, not `--dry-run`;
- the process is not running under `CI`;
- registration did not wholly fail;
- the handoff has not already been completed for this installation; and
- the user did not pass `--no-open`.

An idempotent reinstall does not repeatedly open a browser. `--no-open` is documented for scripts,
managed deployment, and users who prefer the printed URL. CI is quiet automatically. An agent-run
shell command may have piped output but is still an explicit install, so it opens the handoff unless
the caller opts out. Failure to launch the system browser is nonfatal and leaves the exact URL on
screen.

### 3. The handoff page owns changing distribution details

The binary carries one stable URL under `sylin.org`:
`https://sylin.org/ghostlight/service/post-install/`. Store identifiers, publication status,
browser-specific choices, screenshots, and current instructions live on that page. This avoids a
binary release merely to change store guidance.

The page must be public, ungated, lightweight, and useful without JavaScript. It must say candidly
when a store listing is not yet available.

### 4. Completion ends with one clear instruction

After both halves are installed, the user is told to restart MCP clients so they reload their tool
directory. The completion page points to `ghostlight doctor` only as recovery, not as another
mandatory ceremony.

### 5. Installation remains local and non-governing

Opening a user-facing install page after an explicit install is not telemetry. No
installation identifier, query parameter, license state, client list, or machine data is attached.
The extension keeps its existing first-install-only behavior and contains no policy logic.

## Implementation

- Add a small installer handoff module with the stable URL, automation gate, one-time marker, and
  per-platform default-browser launch.
- Add `--no-open` to `ghostlight install`.
- Preserve a printed fallback for every path.
- Unit-test the pure decision gate and command selection without opening a real browser.
- Publish the canonical service-first post-install page before enabling the handoff in a release.

## Consequences

- The browser's required user gesture remains visible and understandable.
- npm, binary, package-manager, and extension-first discovery converge on one journey.
- Re-running the idempotent installer stays quiet.
- Managed and scripted deployment retains full control through `--no-open`; CI is quiet by default.
- The website becomes part of installation continuity. The printed URL and manual documentation
  remain sufficient if it is unavailable; installed software never depends on the site to run.
