# S2: the second machine

## Objective

When the extension arrives on a computer where Ghostlight is not installed, say so and show the way
back. Today that state is indistinguishable from a service that is momentarily down, and the
walkthrough that explains it is reachable only once and only online.

This stage changes the extension only.

## Read first

- [BOOTSTRAP.md](BOOTSTRAP.md) and [PINS.md](PINS.md).
- ADR-0070 (bidirectional, state-aware installation handoff). Its Decision 1 already owns the
  one-time walkthrough; you are completing the other half of the same idea, not replacing it.
- ADR-0091 (store-only end-user extension installation).
- `extension/service-worker.js`, `extension/popup.js`, `extension/popup.html`,
  `extension/options.js`, `extension/options.html`.

## Why this matters

Chrome syncs extensions across a signed-in profile. A person who installs Ghostlight on one computer
and then signs into Chrome on another gets the extension automatically and the native host not at
all. ADR-0070 already opens a walkthrough on that first install. What is missing is the state after
that tab is closed, and any route back when the walkthrough host is unreachable.

## Verified facts as of authoring

Confirmed at `2f24943f`. Re-read before relying on any of them.

- `extension/service-worker.js:103` already builds `last_error` on the snapshot: the raw message
  under the diagnostics preference, otherwise the generic `The local Ghostlight service is
  unavailable.`
- `extension/service-worker.js:158-160` captures Chrome's disconnect reason into that field.
- No surface renders `last_error`. `popup.js`, `options.js`, `content.js`, and `lib/` contain zero
  references to it.
- `extension/popup.js:62-64` prints `Waiting for the Ghostlight service...` for every disconnected
  state.
- `extension/popup.html` has no links. `extension/options.html:74` links only to GitHub.
- The walkthrough URL is remote only, at `extension/service-worker.js:6`.

## Required behavior

1. **Distinguish the state at the seam that knows it.** The service worker classifies the connection
   as connected, unreachable, or host-absent, using the detection rule in `PINS.md`. The
   classification lives in the connection state, next to `last_error`, and is a closed value. No
   surface re-derives it from message text.
2. **Both surfaces render the distinction, in the pinned words.** The popup and the options
   Connection card show the host-absent sentences from `PINS.md` when that state holds, and keep
   today's `Waiting for the Ghostlight service...` for the ordinary unreachable case.
3. **A permanent route back.** Both surfaces expose a control whose accessible name is the pinned
   `Set up Ghostlight` string, shown only in the host-absent state. It opens the walkthrough.
4. **The route survives an unreachable walkthrough.** Add one bundled page inside the extension that
   states what to install and how, with no network dependency, and fall back to it when the remote
   page cannot be opened. The bundled page is instructions only: no product state, no policy, no
   controls, no page content.
5. **An unrecognized disconnect reason falls back to today's behavior.** A wrong claim is worse than
   a vague one.
6. **Nothing else changes.** No new permission, no manifest capability, no change to the hello
   handshake, the reconnect alarm, or the one-time `onInstalled` behavior ADR-0070 owns.

## Tests to add

Create `extension/tests/onboarding.test.js`, following the conventions in `extension/tests/`
(`node:test`, `node:assert/strict`). Add these tests by name:

- `"a missing native host is classified as host-absent, not merely unreachable"`
- `"an unrecognized disconnect reason falls back to the unreachable state"`
- `"a connected snapshot is never classified as host-absent"`
- `"the popup names the host-absent state with the pinned sentence"`
- `"the popup offers the setup route only in the host-absent state"`
- `"the options connection card renders the same state in the same words"`
- `"the bundled setup page needs no network and contains no product state"`

Assert the pinned strings from `PINS.md` literally.

## Verification

    npm test --prefix extension
    node --check extension/service-worker.js
    node --check extension/popup.js
    node --check extension/options.js
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

Then load the unpacked extension in a Chromium profile with no native host registered and confirm by
eye that the popup states the host-absent sentence and offers the route. Record that observation in
the ledger.

## Out of scope

WSL detection, which is S3. Any Rust change. Any change to `doctor`. Any change to the remote
walkthrough page, which lives outside this repository. Any new preference. Any in-page presence.

## STOP preconditions

- `last_error` is no longer populated on the snapshot, or the surfaces already render it.
- Distinguishing the state would require a new extension permission.
- The classification cannot be made without parsing message text at a surface.
- Chrome no longer reports a disconnect reason that the pinned rule can recognize, and no other
  local evidence distinguishes an absent host.
