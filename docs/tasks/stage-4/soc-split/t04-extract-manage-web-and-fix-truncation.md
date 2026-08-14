# t04: Extract `manage/web.rs` as its own bounded context; rename assets; fix the truncation bug

Cites: ADR-0033 Decision 2, Decision 3, Decision 7. Needs t03 DONE (the halves are split; this
task completes the management-plane extraction).

## What this task is

Two things land together because they are the same change:

1. **The management plane becomes its own bounded context.** `manage/web.rs` gets its own module,
   its own routing context, its own capability decision (`manage.web.enabled` / `manage.web.from`,
   from t02), and the assets rename (`console.*` → `manage.*`). The shared listener stays
   (Decision 7), but the two routing branches now run in separate modules with separate gates —
   `inbound::web` for the WS-upgrade path, `manage::web` for the operator UI routes.

2. **The response-flush / truncation bug gets fixed** in the management-plane's response writer.
   This is the CI failure that surfaced the whole SoC issue: a request received `200 OK` + headers
   + the early body (`/console.css` in `<head>`) but not the tail (`/console.js` before `</body>`),
   because the server closed the connection before the full body drained. The fix lives here, in
   the management plane's writer, where it belongs — not bolted onto the ingestion path.

## Why fourth

Depends on t03's file split. The truncation fix lands here (not t03) because it touches only the
management-plane response writer, which is only cleanly separable once the split exists.

## Current-tree facts (re-verify)

- After t03, the Console routes (`route_console_request`, `write_config_response`,
  `write_sessions_response`, `write_enable_remote_response`, `write_asset`, `write_plain_error`,
  `record_config_changed`) live in a temporary home (likely a `manage/web.rs` stub or still in the
  remains of the old router). The pure builders (`config_payload`, `sessions_payload`) are
  reusable.
- Assets today: `src/hub/console/index.html`, `console.css`, `console.js`, loaded via
  `src/hub/console_assets.rs` (`INDEX_HTML`, `CONSOLE_CSS`, `CONSOLE_JS` constants).
- The static-route tests in `tests/console_static_routes.rs` reference `/`, `/console.css`,
  `/console.js`, `/api/v1/*`.
- The flush bug: `write_asset` and the JSON writers do `stream.write_all(response.as_bytes())`
  and return; the `TcpStream` is then dropped at the end of the spawned task. On Windows
  specifically, a socket closing with data still pending in the send buffer can RST instead of
  draining, so a slow client read sees a truncated body.

## What changes

1. **`src/hub/manage/mod.rs`** declares the zone + its permanently-loopback posture (Decision 3).
   Doc comment states: this plane NEVER flows through `serve_session`; it reads `ConfigStore` /
   audit / state directly; `from` is locked to localhost; an org layer can disable it but cannot
   widen it.
2. **`src/hub/manage/web.rs`** holds all the operator-UI routes, moved from their t03 stub.
   Owns: the routing function for non-WS requests on the shared listener; the `manage.web.enabled`
   + `manage.web.from` gate (consulted fresh per request, like the inbound side's `from`); the
   config/sessions/enable-remote handlers; the static-asset writers; the loopback hard-check
   (defense-in-depth on top of t02's validator — `manage.web.from` is already constrained to
   localhost, but the router additionally rejects any non-loopback peer before routing).
3. **Assets rename**: `src/hub/console/` → `src/hub/manage/assets/`; files `index.html` (path
   stays `/`), `console.css` → `manage.css`, `console.js` → `manage.js`; routes `/console.css` →
   `/manage.css`, `/console.js` → `/manage.js`; `console_assets.rs` → `manage/assets.rs` (or
   inlined into `manage/web.rs`). The HTML's own `<link>`/`<script>` references update.
4. **The flush fix**: every response writer in `manage/web.rs` calls `stream.flush().await?` (and
   ideally `stream.shutdown(Shutdown::Write)` to drain cleanly) before returning, so the full
   body reaches the client before the socket closes. The WS side (`inbound/web.rs`) does NOT need
   this — its framed protocol drains per-frame via `poll_flush` already. This is exactly the
   "fix the truncation in the management-plane writer, where it belongs" move.
5. **`inbound/web.rs`'s classifier** (from t03) now delegates non-WS requests to
   `manage::web::route(...)` instead of an inline router. The two zones are now genuinely
   separate: a request is classified at the seam, then each side owns its routing + gate + writers.

## Tests

- `tests/console_static_routes.rs` → rename to `tests/manage_web_routes.rs`; update route paths
  (`/console.css` → `/manage.css`, `/console.js` → `/manage.js`); keep the WS-upgrade-unchanged
  assertion (it now crosses the seam into `inbound::web`). The truncation failure that surfaced
  this (`console_index_page_is_served_over_a_real_http_get` getting CSS but not JS) now passes
  reliably because of the flush fix — that is the regression sentinel.
- `tests/console_config_api.rs` → `tests/manage_web_config_api.rs`; routes unchanged (`/api/v1/
  config`); the `manage.web.from` 403 path now consults the management-plane gate (a denied Origin
  on the management plane is independent of the inbound-web gate).
- `tests/console_enable_remote.rs` → `tests/manage_web_enable_remote.rs`; the literal key strings
  updated in t01; the route path stays (`/api/v1/config/webapi-enable-remote` — or rename to
  `/api/v1/config/inbound-web-enable-remote` for consistency; pick one and pin it).
- `tests/console_sessions_api.rs` → `tests/manage_web_sessions_api.rs`.
- A new test asserting the loopback hard-lock on `manage.web`: a simulated non-loopback peer
  (constructor injection or a bound remote address in a test harness) is rejected at the router
  regardless of the (locked) `from` value. This is Decision 3's acceptance test.

## Verification

- All four gates green, including the previously-flaky `console_index_page_*` (now
  `manage_web_routes.rs::*`) on both supported operating-system jobs.
- `grep -rn "console" src/hub/` returns nothing (assets fully renamed; only historical ADR/task
  text mentions "Console").
- The management plane and the web ingestion adapter are now separately denyable: a test with
  `inbound.web.enabled = false, manage.web.enabled = true` confirms the management UI still
  serves while the WS adapter does not bind, and vice versa.
- All-open golden + tool fidelity + architecture test all pass unchanged.

## Out of scope

- `manage/cli.rs` (doctor/status) — phase 5.
- `manage/instrumentation.rs` (debug sink relocation) — phase 5.
- A second TCP port for the management plane — explicitly NOT taken (Decision 7).
- The recursive `inbound`/`manage` grant grammar — deferred.
