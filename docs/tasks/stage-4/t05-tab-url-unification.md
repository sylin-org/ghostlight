# T05: one tab-URL resolution per call

## Goal

Implement ADR-0024 Decision 4: the sacred check and the grant path share ONE lazily
resolved, memoized tab URL per call, obtained via the existing `tab_url_request` frame
(`Browser::tab_url`). Delete `resolve_tab_host` (the internal `tabs_context_mcp` call
plus its result-shape parsing inside the ingest layer). Per-stage failure semantics and
the zero-frames guarantees are preserved exactly; the only observable change is the
FRAME TRAFFIC (at most one tab-URL probe per call, and no synthesized `tabs_context_mcp`
tool_request), which is a test-visible but user-invisible change sanctioned by the ADR.

## Authority

ADR-0024 Decision 4 is normative. Where this prompt and the ADR disagree, THE ADR WINS.

## Depends on

t04 landed (the pipeline module exists and owns both call sites). STOP if
`src/transport/mcp/pipeline.rs` does not exist or `resolve_tab_host` is not in it.

## Current behavior (verified 2026-07-03 against `b4b2faf`, as moved by t04; re-read)

- `pipeline.rs::sacred_check` STEP B: for any call carrying a numeric `tabId`, resolves
  the tab's current host via `resolve_tab_host`, which issues an INTERNAL
  `browser.call("tabs_context_mcp", {"createIfEmpty": false})` and parses that tool's
  result shape (content[0].text as JSON, tabs array, tabId/url fields). Unresolvable ->
  no sacred match possible (the check finds no host; the call proceeds to the next
  stage).
- The grant path resolves the SAME tab via `Browser::tab_url(tab_id)` (the dedicated
  `tab_url_request` frame): unknown/closed tab or channel failure -> `Indeterminate`
  (fail closed). The navigate landing re-check makes its own post-dispatch `tab_url`
  probe (that one is a DIFFERENT moment in time -- post-navigation -- and stays).
- Consequences today: up to two extension round-trips per governed tab-scoped call to
  answer "what URL is tab N on", by two different mechanisms; the sacred g08 inline
  tests register fake `tabs_context_mcp` responses and their `seen` frame logs show
  that internal tool_request.

## Required behavior

### 1. The shared resolution

In `pipeline.rs`, introduce one per-call memoized resolution (a small private struct or
`Option`-cell local to `handle_tools_call`): the FIRST stage needing the tab's URL
triggers exactly one `browser.tab_url(tab_id)` await; the result (an
`Option<String>` url, `None` for unknown/closed/channel-failure) is reused by any later
stage in the same call. No stage triggers it when it does not need it:

- sacred STEP B needs it iff the sacred list is non-empty AND the call carries a numeric
  `tabId`;
- the grant path needs it iff governed AND requires is non-empty AND the descriptor is
  `TabScoped`;
- nothing else needs it (the landing re-check's post-dispatch probe is separate and
  unchanged).

Laziness is load-bearing: all-open with empty sacred list issues ZERO probes (the
existing pinned zero-frames tests prove it); the free-action no-probe test keeps
passing.

### 2. Consumers

- sacred STEP B: derive the match host from the shared url via the existing
  `pattern::host_for_matching` path exactly as `resolve_tab_host` did; `None` url ->
  no sacred host check (byte-identical outcome semantics).
- grant path: `None` url -> `Indeterminate` (fail closed), `Some(url)` ->
  `resource::resolved_url_resource(&url)`; byte-identical outcomes.
- DELETE `resolve_tab_host` and its result-shape parsing, plus any now-orphaned
  fake-extension helper that existed only to answer its internal `tabs_context_mcp`
  probe (clippy `-D warnings` will flag it as dead code; delete rather than allow).
  `rg -n "resolve_tab_host" src/` -> nothing. The `audit domain` can no longer disagree
  between the two stages (one source); existing pinned domain assertions must still
  pass unchanged.

### 3. Test ripple (frame sequences only)

The g08 sacred inline tests that registered fake `tabs_context_mcp` responses re-wire to
the `attach_fake_extension_with_tab_urls` table (already used by the landing tests), and
their `seen` expectations change from the internal tool_request to `tab_url_request:N`.
Every OTHER assertion in those tests (denial text, denial id, audit bytes, which calls
are denied) stays byte-identical -- transcribe, never weaken. Any test that asserted the
COUNT of probes updates to the unified single probe. Document each `seen`-vector edit in
the ledger as the sanctioned ADR-0024 Decision 4 change.

## Constraints

1. One commit: `refactor(architecture): t05 one tab-url resolution per call`.
2. User-visible behavior byte-identical: denial texts/ids, audit records, tool results,
   ordering. Black-box suites (`tool_enforcement`, `shadow_mode`, `mcp_protocol`,
   `all_open_golden`, `audit_recorder`) pass with ZERO expectation edits (they never see
   frame internals).
3. tools.json/fidelity untouched; architecture test green; ASCII; no new deps.
4. The extension is untouched: `tab_url_request` already exists.

## Tests (minimum)

1. `one_probe_serves_sacred_and_grants` (NEW, pipeline.rs): non-empty sacred list +
   governed manifest + a TabScoped call on a clean tab: the `seen` log contains exactly
   ONE `tab_url_request:N` before the dispatched tool frame (today it would show the
   tabs_context call plus a tab_url_request).
2. `unresolvable_tab_still_fails_closed_for_grants_and_skips_sacred` (NEW): tab_urls
   table answering `None`: with a sacred list, the call is NOT sacred-denied; governed,
   it IS denied with the current Indeterminate/unmatched wording (transcribe).
3. Every reworked g08 test green with unchanged denial/audit assertions.
4. The free-action and all-open zero-frame tests green unchanged.

## Verification

fmt/clippy/test green; `rg -n "resolve_tab_host" src/` empty; ASCII scan; ledger entry
listing each seen-vector edit; RESUME HERE -> t06; commit. Append ONE deferred live
check:

    ## t05-1: single tab-URL probe live
    Changed: t05 unified sacred and grant tab resolution onto tab_url_request (one probe
    per call).
    Steps: with a sacred list configured and a governed manifest active, run read_page on
    a granted tab while watching the extension service-worker console (frame logging on).
    Expect: exactly one tab_url_request per read_page call; sacred and grant outcomes
    unchanged from s-live-1/g08 expectations.

## Out of scope

- The landing re-check's own post-dispatch probe (different moment; stays).
- Hot-reload (t06), deletions (t07), docs (t08). Any caching ACROSS calls (per-call
  memoization only; a tab's URL changes between calls).
