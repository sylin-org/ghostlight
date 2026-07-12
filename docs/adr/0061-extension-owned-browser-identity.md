# ADR-0061: Extension-owned browser identity and server-assigned tab slots

- Status: Accepted (implemented 2026-07-12)
- Date: 2026-07-12
- Amends: ADR-0058 (per-browser identity and focus routing)

## Context

ADR-0058 gave each connected browser an identity so the Hub can route to it and composite tab ids
carry their owning browser. That identity is the browser's OS pid, which the browser-role relay
derives from `proc::parent()` and sends in the hello as `browserPid`; sessions are keyed by it, and
the composite tab id is `browserPid * 2^32 + native_tab_id`.

Live testing the `ghostlight demo` tour surfaced a real, reproducible failure: `navigate` returned
"the browser that owns this tab is no longer connected", and the minted tab id decoded to
**browser-pid 0**. Root cause, traced in `hub/outbound/browser.rs`:

1. **`browserPid` can be 0.** `attach()` reads `hello.browserPid ... unwrap_or(0)`. When the relay's
   `proc::parent()` cannot resolve the browser process, the session is keyed `0`.
2. **`resolve_target(None)` (used by `tabs_create`, which has no input tab id) falls back to
   `sessions.keys().min()`** when the focus chain has no live entry -- so a lingering pid-0 session
   (a hard-killed relay that never cleanly detached) is the smallest key and gets picked. The new
   tab is minted under pid 0 even though the live browser is a different pid; `navigate` then decodes
   pid 0, routes to the dead session, and fails.

The deeper problem is that identity is **guessed by the relay from an OS artifact** that is indirect,
not guaranteed non-zero, and not the most stable choice. A spot-fix (fall back to `relayPid` when
`browserPid` is 0) removes the immediate symptom but leaves identity sourced from process metadata.
This ADR does the root fix instead.

A related question was raised: should the tab id be a string (`"{pid}:{tabid}"`) rather than a
composite number? Rejected: `tabId` is `"type": "number"` in the trained tool schemas (a frozen
ADR-0034 D7 field, and the type the per-call input validator enforces), so a string would break the
contract and gamble on Claude's round-trip behavior. The design below keeps the number tab id.

## Decision

**Browser identity belongs to the extension, not the relay.** The extension is the one entity that
persists across every relay reconnect and service-worker relaunch, so it owns the identity:

1. **Extension mints a persistent browser id.** On first run the extension generates a UUID and
   stores it in `chrome.storage.local` under `ghostlight_browser_id` (`local`, not `session`, so it
   survives service-worker death). It reads it back on every startup and includes it in every
   native-messaging hello. Always present, never 0, unique per browser profile, stable across relay
   reconnects AND SW relaunches -- strictly more than `browserPid` or `relayPid` gives.
2. **The extension announces `browserId` as its opening frame**, and the relay forwards it
   verbatim. The relay stays a pure byte pipe: it never parses the extension's frames, so identity
   is NOT folded into the relay's `ROLE_BROWSER` hello. Instead the extension posts
   `{"type":"browser_hello","browserId":"<uuid>"}` as its first native message on every connect; the
   service reads it as the second frame on a browser connection, right after the relay's hello. This
   keeps the extension->service identity handshake entirely between those two parties (see the
   implementation plan for why this is preferred over threading the id through the relay). The
   relay's `browserPid` stays purely diagnostic; `proc::parent()` stays ONLY for the browser-role
   parent-death watchdog (what it is actually good at) and is no longer the identity.
3. **The service keys browser sessions by `browserId`.** A reconnect from the same browser (same
   UUID) cleanly REPLACES its session -- the exact ADR-0058 semantics, now hung on a stable,
   reliable, never-zero identity.
4. **The service assigns each browser a small, stable numeric `slot`** (1, 2, 3, ...; never 0),
   mapped from its UUID for the lifetime of the service. The composite tab id stays exactly as
   ADR-0058 designed -- `slot * 2^32 + native_tab_id` -- but `slot` replaces the guessed pid.
   Decoding routes `slot -> UUID -> session`. Because slots are dense, non-zero, and always map to a
   live browser, the `pid=0` and `min()-picks-a-corpse` failures are impossible by construction.

This is the synthesis of both threads: the tab id STAYS a `number` (Claude-safe, no D7 change, no
string round-trip gamble), but its high bits become a reliable server-assigned slot rather than an
unreliable pid; and identity moves to where it belongs. No lookup table for tabs (only a small
slot<->UUID map), no schema change.

Prior art: an application-minted persistent instance id is the standard device/client-identity
pattern (browser fingerprint-free device ids, mobile install ids, WebSocket client tokens) -- the
canonical way to get identity that survives transport churn without relying on process metadata.

## Consequences

- Fixes the pid-0 routing failure at the root; a freshly created tab always carries the slot of the
  actual live browser.
- Identity survives relay reconnects and service-worker relaunches, which `browserPid` did not
  reliably do.
- The number tab id and the composite arithmetic are preserved; Claude stays on trained rails.
- Dead-session hygiene still matters (a slot must be freed when its browser disconnects), but a
  stale slot can no longer be silently minted onto a new tab, because slots map to UUIDs and a new
  tab is minted under the resolving live session's slot.
- Slots are per-service-lifetime (a service restart re-assigns them); outstanding tab ids from
  before a restart are stale, which is already true today (a restart re-groups tabs).

## Implementation plan (three layers)

Note on mechanism (as implemented): the browserId travels as the extension's own opening frame, not
folded into the relay's hello. Two shapes were considered:

- **A (relay carries it):** the relay reads the extension's identity frame and folds the UUID into
  its `ROLE_BROWSER` hello. Rejected: it makes the deliberately-dumb byte-pipe relay parse the
  extension's message semantics -- a coupling the codebase avoids (the relay does not parse
  `tool_request`, `notification`, etc.; it pumps bytes).
- **B (service reads it), chosen:** the relay is unchanged; the extension posts
  `{"type":"browser_hello","browserId"}` first, and the service reads it as the second frame on a
  browser connection. Protocol semantics live in the service, which already parses every frame. The
  only cost is a second, bounded read in `attach` -- localized and testable.

1. **Extension (`extension/lib/identity.js` + `extension/service-worker.js`):** a small
   injected-dependency module, `createBrowserIdentity(storage, generate)` (mirroring
   `lib/debug.js` / `lib/grouping.js`), reads/generates/persists a `crypto.randomUUID()` under
   `ghostlight_browser_id` in `chrome.storage.local`. `connect()` resolves it before opening the
   port and posts `{type:"browser_hello", browserId}` as the very first frame. Unit-tested in
   `tests/extension/identity.test.js`.
2. **Transport (`crates/transport/src/handshake.rs`):** `browser_hello_bytes` is UNCHANGED (the
   relay stays a byte pipe). Add the identity-frame vocabulary (`EXTENSION_IDENTITY_TYPE`,
   `BROWSER_ID_FIELD`) and a `parse_extension_identity(bytes) -> Option<String>` the service uses.
   `BrowserInfo.pid` becomes `BrowserInfo.slot` (`crates/transport/src/ipc.rs`).
3. **Core (`crates/core/src/hub/outbound/browser.rs` + `crates/core/src/constants.rs`):**
   - A slot registry: `slots: HashMap<browser_id (String), slot (u32)>` + a monotonic `next_slot`
     from 1. `slot_for()` gets-or-assigns; `slot_of()` is a pure lookup. The mapping is NEVER
     evicted (a reconnect keeps its slot); only the live `sessions` entry is evicted on detach.
   - `attach()` reads the relay hello, then the identity frame (bounded by `IDENTITY_WINDOW`),
     assigns a slot, keys `sessions` by slot, and seeds the focus chain. A missing/blank identity is
     rejected fail-closed (`AttachOutcome::AlreadyAttached`, `Diagnostic::MissingIdentity`).
   - `tab_id::{encode,decode}` arithmetic unchanged (now `slot * 2^32 + native`; params renamed).
   - `resolve_target(None)` picks the most-recently-active LIVE slot (`focus_front_live`); the
     `min()` corpse fallback is retired (the chain is seeded on attach, so it always covers a live
     session). A composite that decodes to `slot 0` (a plain/un-encoded id or a pre-0061 client) is
     treated as unrouted and resolved by focus, keeping the native tab id.
   - `note_focus`/`focus_chain`/`encode_tab_ids`/`merge_tab_id`/diagnostics all key on slot;
     `ghostlight doctor`'s browser list shows `slot`.
- Tests (all landed + green): a reconnect from the same `browserId` replaces (not duplicates) the
  session and keeps its slot; two distinct `browserId`s get distinct non-zero slots and route
  independently; a `ROLE_BROWSER` hello with no valid identity frame is rejected (no slot minted);
  the `parse_extension_identity` shape/blank/non-JSON cases; the extension module's mint/persist/
  read-back/degraded-storage cases.
