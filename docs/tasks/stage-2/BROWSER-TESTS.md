# Stage 2 browser tests

Deferred live-browser verification for stage-2 governance. The unattended executor CANNOT drive a real
browser, so every check that needs one is written here instead of run. A human runs these against a
live browser after the code lands (as in release-1). Accumulate entries as tasks land; do not delete
them.

## Format

One entry per check:

```
## <task-id>-<n>: <one-line purpose>
Changed: <what code changed and why a browser is needed to verify it>
Steps: <exact, ordered steps a human runs (tools, URLs, inputs)>
Expect: <the precise observable result that means PASS>
```

Keep steps concrete and self-contained (name the tool, the URL, the manifest/config used). Prefer
checks that are unambiguous to eyeball. Note when a check depends on a specific manifest or config
posture (all-open vs a restrictive manifest vs observe/shadow mode).

## Live verification log

Entries here record checks actually run against a live browser, with the date, the posture,
and the outcome, so a later human knows what is already covered. This does NOT replace
re-running them after further code changes; it records what was observed at a point in time.

### 2026-07-02: all-open engine + g08 sacred domains -- PASS (live Chrome + Claude Code)
Posture: no manifest (all-open); built-in Minimal config; extension connected on a fresh
mcp-server started after an IPC-pipe reset cleared a stale two-instance split (an old server
owned the pipe while the live client was bound to a pipe-less one; killed all browser-mcp
processes, reconnected the MCP server, reloaded the extension).

Covered live:
- All-open engine end to end (the all-open half of g13-3): tabs_context, navigate (with
  scheme defaulting), computer screenshot (JPEG, ADR-0010 downscale/compression), read_page
  (accessibility tree + element refs + viewport), get_page_text, computer left_click
  (returned a TEXT confirmation, not a screenshot -- the mutate screenshot-behavior
  optimization), and cross-domain navigation via a clicked link (example.com -> iana.org).
  No denial ever appeared.
- g08-1 sacred domains, driven entirely by hot-reload (NO client restart, which also proves
  the a5 substrate live in both directions): wrote a user config.json with
  content.security.sacred_domains ["iana.org","*.iana.org"], waited for the watcher, then:
  (A) read_page on a tab showing www.iana.org -> Denied (D-f66bdca0), names www.iana.org
      (matched via *.iana.org).
  (A') computer screenshot on the same tab -> Denied, IDENTICAL D-f66bdca0 (deterministic;
      every tool on a sacred tab is denied).
  (B) navigate that same sacred tab to a CLEAN target (example.org) -> Denied (D-f66bdca0):
      a sacred tab cannot be moved away, not just read.
  (C) navigate a separate CLEAN tab (chrome://newtab) to https://iana.org/ -> Denied
      (D-3905b751), names the TARGET host iana.org (the navigate-target check).
  (D) navigate that clean tab to example.org -> allowed (enforcement is selective, not
      blanket).
  Then deleted config.json; the watcher re-resolved and the once-denied www.iana.org tab
  read succeeded again (bidirectional hot-reload, no restart).
- Audit recorder live (g06/g08 audit path): with audit.enabled and audit.file.path pointed
  at a scratch file (the path change itself hot-reloaded the sink), every call above was
  recorded: the 4 sacred denies each decision=deny, grant_id=null, a stable denial_id,
  duration_ms=0 (blocked before any browser work); the 2 allows with non-zero duration
  (65 ms tabs_create, 190 ms navigate). Covers the audit-shape half of g13-2 for the sacred
  path (stable denial_id, grant_id null).

Observation (NOT a defect; enforcement was correct): for the navigate-TARGET denial (C), the
audit record's `domain` field was null -- it recorded the CURRENT tab context
(chrome://newtab, no host) rather than the sacred target (iana.org) that triggered the
denial. The denial message and the deterministic denial_id both name the target, so it stays
traceable, but consider whether the audit `domain` should carry the target host for a
target-triggered denial.

Still pending (need a human-driven setup this session could not perform):
- g10-1..5, g11-1..4: require clicking Chrome's own extension popup / toolbar and firing a
  keyboard shortcut. The `computer` tool dispatches CDP Input into PAGE content, not Chrome's
  browser chrome, so the extension popup cannot be driven from an MCP session.
- g13-1..3, g15-1..2: require the mcp-server to be started with a restrictive `--manifest`
  (or BROWSER_MCP_MANIFEST) active. Manifest grants are fixed at server startup (the manifest
  watch-slot is not wired to hot-reload), and a session cannot restart its own MCP client, so
  these need a human to relaunch the server with a manifest. The all-open half of g13-3 is
  covered above. UPDATE, same day, later session: most of g13 was subsequently covered live;
  see the next entry.

### 2026-07-02 (later session): g13 grant enforcement live -- PASS, and the finding that produced ADR-0022
Posture: discovered mid-session that the org policy path (`%ProgramData%\browser-mcp\policy.json`)
is auto-loaded at server startup and wins over everything, so a governed restart needs NO client
config change: wrote a restrictive schema-2 manifest there (grants: `example-full` =
example.com/*.example.com access all; `net-readonly` = example.net access read), killed the
running mcp-server, and the next tool call respawned a governed server automatically (extension
re-handshake took 0.4 s, no manual reconnect needed). The policy file was REMOVED after the
session and all-open restored (verified via doctor); nothing machine-wide remains.

Covered live (g13-1 steps 1-3 equivalents, g13-2 audit shapes):
- Unmatched domain: `read_page` on example.org (no grant) -> `Denied (D-04ede48d): no grant
  covers example.org...`; audit line decision=deny, grant=null, denial=D-04ede48d.
- Full grant: navigate + read_page + `computer left_click` on example.com all allowed;
  audit lines carry grant_id=example-full.
- Access rule: `navigate` to example.net (read-only grant) -> `Denied (D-b81ab772): 'navigate'
  needs write access on example.net, and grant 'net-readonly' allows read only...`; audit line
  decision=deny, grant=net-readonly, denial=D-b81ab772.

### 2026-07-03: t-live-1 stage-4 regression pass -- PASS (live Chrome + Claude Code, stage-4 tree)
Posture: fresh binary built from the stage-4 tree (t01-t08 landed, commit range b8225ef..a14ccf8
on branch stage-4). No VS Code reload was needed: `doctor` confirmed no mcp-server process was
running at session start, so the first tool call spawned a brand-new process from the rebuilt
binary automatically. All governed scenarios below were driven by writing/editing/deleting the
real org policy file (`C:\ProgramData\browser-mcp\policy.json`) directly -- the ADR-0023 selection
rule and the ADR-0025 watcher pick up every change with no client restart, so a single
continuously-running mcp-server process (pid 15680, same pid for the entire 5m22s span) carried
every state transition below with zero restarts. Adapted the manifest DELIVERY mechanism for
s-live-1/s-live-2 from the entries' literal `--manifest file://` CLI flag to the org policy path
instead (operationally equivalent for what is being tested -- ADR-0023 makes both origins go
through the same one loader -- and avoids needing to touch the MCP client's own config/reload it).

Covers s-live-1, s-live-2, s-live-4, t01-1, t05-1, t06-1, t06-2 (re-run against the stage-4 tree)
plus s-live-3 (first stage-4 run). All PASS, no regressions found in the pipeline rewrite.

- **t01-1** (org-path policy file loads live): killed the running all-open mcp-server, wrote a
  schema-3 manifest (grant `read-only` on example.com/*.example.com, plus a mandatory
  `audit.enabled`/`audit.destination`/`audit.file.path` config block) to the org path, then made
  the next tool call. The server booted successfully (pre-t01 this was a fatal startup error) with
  `governance mode: enforce` and the correct manifest name/hash per `doctor`.
- **s-live-1** (read grant end to end): `tabs_create_mcp` -> `navigate` example.com -> `read_page`
  + `computer screenshot` all succeeded. `computer left_click` ->
  `Denied (D-39e7ba2d): 'computer (left_click)' needs the 'action' capability on example.com, and
  grant 'read-only' allows read` (verbatim pinned match). `form_input` ->
  `Denied (D-39e7ba2d): 'form_input' needs the 'write' capability on example.com, and grant
  'read-only' allows read` (verbatim pinned match, same denial id as the click).
- **s-live-4** (audit capability field): the scratch audit JSONL showed no `rw` key anywhere;
  `tabs_create_mcp` had `"capability":"none","grant_id":null`; `navigate`/`read_page`/screenshot
  had `"capability":"read","grant_id":"read-only"`; the denied `left_click` had
  `"capability":"action","decision":"deny","duration_ms":0`; the denied `form_input` had
  `"capability":"write"`. Every field matched the entry's pinned expectations exactly.
- **t05-1** (single tab-url probe): read the mcp-server's own debug frame log (a file-based
  substitute for watching the extension service-worker console) and confirmed exactly one
  `tab_url_request` frame per governed call (navigate, read_page, screenshot, the denied
  left_click, the denied form_input each had exactly one), with the two denied calls showing NO
  subsequent dispatch frame at all -- the probe resolves the tab once, sacred and grant checks
  both consume it, and denial short-circuits before any extension dispatch.
- **t06-1** (policy edit applies live, no restart): edited policy.json in place, with NO restart,
  from the read-only manifest to a schema-3 "allow * / deny example.com" manifest (grant
  `everything-but`). Within the hot-reload window, `navigate` to example.com flipped to
  `Denied (D-ca582045): example.com is excluded by grant 'everything-but': your policy denies this
  site explicitly` (this is also s-live-2's exact pinned denial text), while example.org's
  `navigate`/`read_page`/`left_click` all succeeded. The audit file recorded exactly one
  `manifest_reload` event carrying the new manifest's name/hash at the transition. Later, deleting
  policy.json (with the same process still running, no restart) made a previously-denied domain
  navigable again immediately -- confirmed all-open reversion live.
- **t06-2** (broken mid-edit policy never weakens the session): saved policy.json as truncated,
  invalid JSON. A subsequent `navigate` to example.com returned the IDENTICAL denial id
  (`D-ca582045`) as before the edit -- enforcement stayed on the last-good manifest, not reverted
  to all-open and not crashed -- and no new `manifest_reload` event was written. (Could not
  independently confirm the ADR's "ERROR in the server log" line this session, since the
  mcp-server's stderr/tracing output isn't captured anywhere this session could read; this is a
  gap in what this session could observe, not a claim the behavior didn't happen -- the
  keep-last-good behavior itself is solidly confirmed.) Fixing the file (a new manifest denying
  both example.com AND example.org, to make the resumed swap unambiguous) produced a new
  `manifest_reload` event with a new hash, and `navigate` to example.org promptly flipped from
  allow to a fresh, distinct denial id (`D-cd329e99`) -- the swap resumed correctly.
- **s-live-3** (explain tool live): at the all-open baseline (before any policy file existed this
  session), `explain` returned the pinned `Capabilities: read =` block with one line per tool and
  per `computer` action (27 lines: 14 tools + 13 computer actions), and a normal navigate +
  screenshot browsing sequence never invoked `explain` on its own.

Cleanup: removed the scratch audit file and confirmed `C:\ProgramData\browser-mcp\` is empty;
`doctor` reports `Policy manifest: none (all-open)` and a healthy verdict. Nothing machine-wide
remains from this session.

NOT covered live: g13-1 steps 4-5 (hand-navigation drift re-check; redirect parking) -- the
session pivoted to the design finding below before running them. g13-3's governed half (debug
frame-count) and g15-1/2 (mode switch) also remain pending.

FINDING (the reason steps 4-5 were not finished): the access-rule denial above exposed that
`navigate` was classified mutate, so a read-only grant could not navigate to its own granted
domains -- contradicting this file's own g13-1 script (which assumes step-2's navigate succeeds)
and the shipped read-only example grants. Root-caused in design review to a category error
(classifying by unknowable page effect instead of by what the governor can prove; "a navigate
to a logout page is an intent problem, not a write"). Outcome: ADR-0022 (intent-calibrated
capabilities: read/action/write/execute, per-action requirement directory, host polarity),
implemented as the stage-3 task batch (`docs/tasks/stage-3/`). Under ADR-0022 the g13-1 script
becomes correct as written (navigate succeeds on a read grant; the on-page click is what gets
denied). The g13/g15 checks below should be re-run against the stage-3 tree with a schema-3
manifest; the s08 task adds the capability-model live checks.

## Checks

## g08-1: sacred domains deny the agent live, and the audit log records it
Changed: g08 wired the first real enforcement path (ADR-0018 step 2) at the dispatch
chokepoint: a `content.security.sacred_domains` entry now denies any tool call whose
current tab or `navigate` target matches it, before the tool runs. This needs a live
browser and a live MCP client (Claude Code) restart to observe end to end; the automated
suite (`transport::mcp::server::tests::sacred_tab_denies_every_tool_and_never_runs_it`,
`navigate_target_denied_even_when_tab_is_clean`, `empty_list_is_byte_identical`,
`denied_call_writes_one_deny_record`) proves the same code path against a fake extension,
but not real on-screen browser behavior or the real default audit file location.
Steps:
1. Edit the user config file (Windows: `%APPDATA%\browser-mcp\config.json`) to
   `{ "config": { "content.security.sacred_domains": ["example.com", "*.example.com"] } }`.
2. Restart the MCP client (Claude Code) so the new binary/config is picked up.
3. Ask the agent to navigate a tab to `https://example.com/`.
4. Manually navigate a Browser MCP group tab to `https://example.com/` (or reuse the tab
   from step 3), then ask the agent to read or screenshot that tab, and separately ask it
   to navigate that same tab to `https://example.org/`.
5. Ask the agent to navigate to `https://example.org/` (a clean domain).
6. If `audit.enabled` resolves true (the Minimal default), inspect the audit JSONL file
   (default `%LOCALAPPDATA%\browser-mcp\audit.jsonl`) after the above.
Expect: step 3's tool result starts with `Denied (D-` and names `example.com`; the browser
does not actually navigate. Step 4's read/screenshot is denied with the same message
shape (naming `example.com`), and navigating that tab elsewhere is ALSO denied (the
never-touch rule blocks moving the tab away, not just reading it). Step 5 works normally
(the browser navigates, the agent gets real page content). Step 6 shows one
`"decision":"deny"` record per denial above, each with a stable `denial_id` (identical
across repeats of the same denial), `"grant_id":null`, and `"domain"` naming the matched
host; no denial record for the step-5 call.

## g10-1: popup renders hold state and the toggle button works
Changed: g10 added the first extension UI (`popup.html`/`popup.js`), the `action` and
`commands` manifest keys, and the hold request/reply plumbing in `service-worker.js`. This
is the extension's first popup ever; it can only be verified by loading the unpacked
extension in Chrome.
Steps:
1. Reload the unpacked extension at `chrome://extensions` (pick up the new `action`/
   `commands` manifest keys and the new JS).
2. With no MCP session running (browser-mcp binary not started), click the toolbar icon.
3. Start an MCP session (Claude Code connected, extension attached), click the toolbar
   icon again.
4. Click the `Pause agent browsing (take the wheel)` button.
5. Click the resulting `Resume agent browsing` button.
Expect: step 2 shows `No active browsing session.` with the button disabled, within about
1.5 seconds (the `holdRequest` timeout). Step 3 shows `Agent browsing is allowed.` with an
enabled `Pause agent browsing (take the wheel)` button. Step 4 flips the status to
`Agent browsing is PAUSED.`, the button label to `Resume agent browsing`, and the toolbar
badge shows `II`. Step 5 flips back to `Agent browsing is allowed.` and clears the badge.

## g10-2: a paused agent gets the pause text, never reaches the extension
Changed: g10 wired the hold check into `handle_tools_call`, before `governance.decide`,
the sacred check, and any extension traffic. Needs a live Claude Code + extension to
observe the agent's own behavior and confirm no `tool_request` frame reaches the
extension.
Steps: with the extension paused (see g10-1 step 4), ask the agent to take a screenshot.
Expect: the agent receives text starting with `Paused: the user has taken control of the
browser`, naming the `'computer (screenshot)'` call as NOT executed; the tool result is a
normal successful response (not an error) and the agent reports it is waiting for the
user, not retrying. No CDP/tab activity occurs in the browser.

## g10-3: the 2-minute resume hint appears
Changed: `hold_message` appends a second sentence once `held_for >= HOLD_HINT_AFTER`
(2 minutes). Needs a live timing check (or a temporarily lowered `HOLD_HINT_AFTER` in a
local build, restored before committing) since the automated suite only proves the pure
function's threshold logic, not a real elapsed-wall-clock pause.
Steps: pause the extension, wait past 2 minutes (or rebuild locally with a lowered
`HOLD_HINT_AFTER`, verify, then restore and rebuild the real constant), then ask the agent
for any browser tool call.
Expect: the reply carries the base `Paused:` text plus, appended, `This session has been
paused for more than 2 minutes. Only the user can resume it, from the Browser MCP
extension: the popup Pause/Resume button or the toggle keyboard shortcut.`

## g10-4: the hold survives a service-worker restart
Changed: the hold flag lives in `Browser` (the mcp-server process), not the extension;
`route_reply`'s hold-request handling and the flag itself are unaffected by the extension
process dying and Chrome relaunching its service worker. This is exactly the property a
disconnect-driven test cannot fully simulate without a real Chrome service-worker
lifecycle event.
Steps: pause the extension, then in `chrome://extensions` click the service worker's
"service worker" link and terminate it (or use the "Reload" action on the extension while
paused), let it restart, then ask the agent for a browser tool call.
Expect: the tool call is STILL answered with the `Paused:` text -- the binary-side flag
was never touched by the extension restart. Re-opening the popup after the restart shows
`Agent browsing is PAUSED.` (matches the binary's state once the new service worker
reconnects and queries it).

## g10-5: the keyboard shortcut toggles the hold with the popup closed
Changed: `chrome.commands.onCommand` (the `toggle-hold` command, default `Alt+Shift+P`) is
new; a keyboard shortcut can only be exercised via a live Chrome window.
Steps: with the popup closed and an MCP session active, press `Alt+Shift+P` (or whatever
`chrome://extensions/shortcuts` shows if reassigned), then open the popup.
Expect: the toolbar badge toggles (`II` appears or clears) immediately on the keypress;
the popup's rendered state (`Agent browsing is PAUSED.` / `Agent browsing is allowed.`)
matches the badge when opened afterward.

## g11-1: mid-flight kill severs the session in one gesture
Changed: g11 added the panic kill switch: `killSession()` in the service worker (marker,
signal, debugger detach, state clear, port teardown, in that order) and the binary-side
`killed` flag/error in `Browser`. This is the core scenario and needs a live Chrome tab
with the CDP debugger actually attached, and a live MCP client to observe the truthful
error text end to end.
Steps:
1. Start a session from the MCP client: `tabs_create_mcp`, `navigate` to any http(s) page,
   `computer` `screenshot`. Confirm Chrome shows the "is debugging this browser" infobar.
2. Issue a slow call (`computer` `wait` with a ~20s duration) and, while it is in flight,
   open the Browser MCP toolbar popup and click `End session now` once.
3. After the kill, ask the agent for any other browser tool call.
Expect: step 2's in-flight call returns exactly
`[hop: extension] The user ended the browser session (kill switch). Next step: ask the
user to reconnect from the Browser MCP extension popup, then retry.`; the debugger infobar
disappears (the detach happened); the popup now shows
`Session ended. Browser access is severed until you start a new session.` with a
`Start new session` button. Step 3's call returns the SAME error text immediately (no 60s
wait).

## g11-2: the audit log records exactly one session-killed line
Changed: the kill hook writes one `SessionEventRecord` (`event: "session_killed"`) through
the same destination the flight recorder resolves. Needs the real default audit file
location and a live kill to produce it.
Steps: with `audit.enabled` resolving true (the Minimal default), perform a kill (see
g11-1), then open the resolved audit file (default
`%LOCALAPPDATA%\browser-mcp\audit.jsonl` on Windows).
Expect: the last line is a compact JSON object with `"event":"session_killed"`, a
36-char lowercase `event_id`, an RFC 3339 `ts`, and no `tool`, `action`, `rw`, `domain`,
`decision`, `grant_id`, `denial_id`, or `duration_ms` field.

## g11-3: the mid-kill service-worker-restart guarantee
Changed: the `chrome.storage.session` marker is set BEFORE the debugger detach begins,
specifically so a service-worker death mid-kill is completed by startup recovery
(`init()`) rather than leaving a live debugger attachment behind. This exact guarantee can
only be observed against a real Chrome service-worker lifecycle event (a real worker
teardown/restart), not simulated by dropping an in-memory duplex stream.
Steps: kill the session (see g11-1), then force the service worker down from
`chrome://extensions` (the extension's "service worker" link, or wait for MV3 idle
teardown) and wake it (open the popup). Wait at least one keepalive alarm period
(24 seconds) after it wakes.
Expect: the popup still shows the killed view; the debugger infobar does not reappear; a
tool call from the MCP client still fails with the section-7 kill text; the keepalive
alarm firing during the wait does not reconnect the extension.

## g11-4: explicit recovery and kill-with-binary-down
Changed: `RECONNECT_SESSION` (`chrome.storage.session.remove` + `connect()`) is the only
path back to a working session; a kill while no mcp-server is running must still leave the
extension refusing to reconnect until that explicit gesture. Both need a live Chrome
popup and, for the second half, starting/stopping the actual MCP client process.
Steps:
1. Click `Start new session` after a kill (see g11-1). Confirm reconnection, then run
   `tabs_context_mcp` and a fresh `navigate` + `screenshot` from the client.
2. Separately: quit the MCP client entirely (no mcp-server running), click
   `End session now` in the popup, confirm the popup shows the killed view with no error
   surfaced, then restart the MCP client and issue a tool call before clicking
   `Start new session`.
Expect: step 1 reconnects within a few seconds, the fresh flow works end to end, and the
binary no longer reports the kill message. Step 2's tool call after the client restart
fails with the ordinary not-connected error (the extension still refuses to reconnect)
until `Start new session` is clicked.

## g13-1: restrictive-manifest session end to end (grant/access/drift/redirect)
Changed: g13 wires real grant enforcement at all five dispatch points, including a new
`tab_url_request` extension mechanism queried fresh on every tab-scoped call. Everything
here is exercised by unit/integration tests with no extension (or a fake one) connected;
this is the first time it runs against a REAL Chrome tab with the REAL extension in the
loop, which is the only way to see the actual drift-catching and redirect-parking
behavior land correctly.
Manifest (schema 2, `--manifest file://<path>`):
```json
{
  "schema": 2, "name": "g13-manual-check", "version": "1",
  "grants": [
    { "id": "example-full", "domains": ["example.com", "*.example.com"], "access": "all" },
    { "id": "research-read", "domains": ["research.example.org"], "access": "read" }
  ]
}
```
Steps:
1. Start the MCP client with the manifest above active. Ask the agent to `navigate` to
   `https://example.com/`. Confirm it works exactly as it would with no manifest.
2. Ask the agent to `navigate` to `https://research.example.org/`, then issue a `computer`
   `left_click` on that page.
3. On the same `research.example.org` tab, ask for a `computer` `screenshot`.
4. On the `research.example.org` tab, click a link BY HAND to any off-grant domain (or
   type a new URL into the omnibox yourself), then ask the agent to call `read_page` on
   that same tab.
5. Ask the agent to `navigate` to a URL you know redirects off-grant (e.g. a link shortener
   pointing away from `example.com`/`research.example.org`).
Expect: step 1 navigates normally. Step 2's `left_click` returns
`Denied (D-...): 'computer (left_click)' needs write access on research.example.org, and
grant 'research-read' allows read only. ...` and the click visibly does not happen on the
page. Step 3's `screenshot` succeeds normally (observe is permitted). Step 4's `read_page`
returns `Denied (D-...): no grant covers <the off-grant host>. ...` -- proving the
per-call check re-queries the CURRENT tab URL rather than trusting a cached one from
step 1/2. Step 5: the tab visibly lands on `about:blank` and the agent receives a
`Denied (D-...)` text naming the final (redirected-to) host, not the originally-typed
allowed URL.

## g13-2: audit file shows consistent grant/denial ids across the session
Changed: every decision in g13-1 above should have produced exactly one audit record,
with allows carrying the grant id and denials carrying a denial id that repeats
identically for the same rule/grant/manifest combination (ADR-0020). This can only be
confirmed against the real resolved audit file location, populated by the real session
just run.
Steps: with `audit.enabled` resolving true (the Minimal default), run through g13-1's five
steps, then open the resolved audit file (default `%LOCALAPPDATA%\browser-mcp\audit.jsonl`
on Windows) and find the five corresponding lines.
Expect: one line per call (five total, not counting the sacred-domains/tabs_context_mcp
machinery, which writes none). Step 1's line: `decision: "allow"`,
`grant_id: "example-full"`. Step 2's line: `decision: "deny"`, `grant_id: "research-read"`,
a `denial_id` present. Step 3's line: `decision: "allow"`, `grant_id: "research-read"`.
Step 4's line: `decision: "deny"`, `grant_id: null`, `domain` equal to the off-grant host
you clicked to (not `"(unknown)"`). Step 5's line: `decision: "deny"`, `grant_id: null`,
`domain` equal to the FINAL redirected-to host, and `duration_ms` clearly non-zero (the
navigation actually ran before the landing was checked) -- contrast this against every
other deny record's `duration_ms: 0`. Repeat step 2 or step 4 once more and confirm the
new denial's `D-...` id is byte-identical to the first one for the same rule/grant/host.

## g13-3: removing the manifest restores all-open with zero new extension traffic
Changed: `Governance::is_governed()` is meant to gate away every bit of g13's new
extension traffic (the `tab_url_request` query) when no manifest is active, so all-open
stays byte-identical and adds no latency. `--debug` observability is the only way to see
the actual frame count on a live session.
Steps: start the MCP client with NO `--manifest` flag and `--debug` enabled. Run the same
navigate/click/screenshot sequence as g13-1 (there is no manifest now, so everything
should simply work). Inspect the debug state/event log (`browser-mcp status`, or the raw
log file `--debug` writes).
Expect: every call behaves exactly as it did before g13 landed (no `Denied (` text ever
appears). The debug log shows ONLY the familiar `tool_request`/`tool_response` frame
pairs for each call -- no `tab_url_request` frame appears anywhere in the session.

## g15-1: the observe-vs-enforce mode switch against a real page
Changed: g15 adds the mode switch (per-grant > manifest > `governance.mode`) turning a
would-deny into a real block (`enforce`) or a recorded-but-allowed `shadow_deny`
(`observe`). This needs a real page and a real agent to see the shadowed action visibly
execute, and a real audit file to see the two decisions side by side.
Manifest (schema 2, `--manifest file://<path>`), used for BOTH steps below, editing only
the top-level `mode` field between runs:
```json
{
  "schema": 2, "name": "g15-manual-check", "version": "1", "mode": "enforce",
  "grants": [
    { "id": "example-read", "domains": ["example.com", "*.example.com"], "access": "read" }
  ]
}
```
Steps:
1. With `mode: "enforce"` active, navigate to `https://example.com/` and ask the agent for
   a `computer` `left_click` on the page.
2. Edit ONLY the manifest's `mode` to `"observe"`, restart the MCP client, and repeat the
   identical `left_click` on the same page.
Expect: step 1's click returns `Denied (D-...): 'computer (left_click)' needs write
access on example.com...` and the click visibly does NOT happen. Step 2's click visibly
DOES happen on the page (the shadowed action executes normally) and the agent's response
carries no `Denied (` text at all -- it reads exactly like a normal successful click.
Open the audit file (default `%LOCALAPPDATA%\browser-mcp\audit.jsonl` on Windows) and
confirm step 1's line has `decision: "deny"` and `duration_ms: 0`, and step 2's line has
`decision: "shadow_deny"` and a clearly non-zero `duration_ms`; both lines carry
`grant_id: "example-read"`. Do NOT expect the two lines' `denial_id` to match: editing
the manifest's `mode` field changes the manifest's own content hash, which is one of the
denial id's three inputs by design (ADR-0020: a denial id is attributable to the exact
policy version that produced it) -- see the g15 ledger entry's deviation 6 for the full
reasoning and the automated test that instead pins the same-hash case directly.

## g15-2: hold, kill, and sacred-domain checks are unaffected by an active observe-mode manifest
Changed: g15's mode switch must never interfere with mechanisms that are structurally
separate from it -- the take-the-wheel pause (g10), the panic kill switch (g11), and the
sacred-domains carve-out (g08), which the g15 doc requires to stay a REAL `deny` in every
mode. The sacred case is unit/integration-tested already (no browser needed); hold and
kill are browser-facing mechanisms this task never touches but is worth reconfirming
under an active manifest specifically.
Steps: with the g15-1 manifest active and `mode: "observe"`:
1. Engage the take-the-wheel pause from the extension popup, then ask the agent to call
   any tool.
2. Release the pause, then click `End session now` (the panic kill switch) from the popup.
3. Add `www.example.com` (or another domain the grant above covers) to
   `content.security.sacred_domains` in the user config file, navigate the agent's tab
   there, and ask for any tool call on that tab.
Expect: step 1's call returns the ordinary `Paused:` text, exactly as g10 already
guarantees, regardless of the active manifest. Step 2's kill severs the session with the
ordinary kill-switch error text, exactly as g11 already guarantees. Step 3's call
returns `Denied (D-...): ... is on the user's never-touch list`, NOT a `shadow_deny`
outcome and NOT the ordinary tool result, even though the manifest's own mode is
`observe` -- the sacred-domains check runs ahead of and independently from the grant
mode switch.

## s01-1: read-only grant can navigate; acting on the page is still denied
Changed: s01 reclassified navigate from mutate to observe (ADR-0022 Context/Decision 2)
on the stage-2 schema-2 model; only a real browser proves the granted page loads for a
read-only session.
Steps: start the mcp-server with a schema-2 manifest whose only grant is
{"id":"research-read","domains":["example.com","*.example.com"],"access":"read"} and
audit enabled; then (1) navigate to https://example.com/ in an MCP tab, (2) computer
screenshot, (3) computer left_click on the page.
Expect: (1) and (2) succeed with no Denied text; the audit lines carry decision=allow,
grant_id=research-read, and rw=observe for the navigate. (3) returns Denied (D-...)
naming research-read and the read-only wording; its audit line is decision=deny.

## s05-1: schema-3 capability grants end to end (advertisement, enforcement, explain)
Changed: s05 replaced the whole schema-2 domains/access/tools grant model with schema-3
hosts/allowed capability sets (ADR-0022 Decisions 3-6, 8), rewiring enforcement, tool
advertisement, and `policy explain` together. Only a real browser proves a live session
sees the new tool list, the new denial wording, and the new explain sentences agree with
what actually happens on a page.
Steps: start the mcp-server with a schema-3 manifest:
{"schema":3,"name":"s05-manual-check","version":"1","grants":[{"id":"example-read",
"hosts":{"allow":["example.com","*.example.com"]},"allowed":["read"]}]}, audit enabled.
(1) Run `browser-mcp policy explain` on this manifest file and confirm the printed
sentence reads "Allowed on example.com, *.example.com: read pages." (2) Call
`tools/list` (or ask the agent to list tools) and confirm `form_input` and
`javascript_tool` are ABSENT while `navigate`/`computer`/`read_page`/`find`/
`get_page_text`/`tabs_context_mcp`/`tabs_create_mcp`/`resize_window`/`update_plan`/
`read_console_messages`/`read_network_requests` are all PRESENT (this is a deliberate
change from schema-2's read-only set: it no longer excludes `navigate`/`tabs_create_mcp`/
`resize_window`/`update_plan`). (3) Navigate to https://example.com/, take a screenshot,
and scroll. (4) Attempt a `computer` `left_click` on the page. (5) Attempt `form_input`
on the page (it should be advertised-absent, but call it directly by name anyway to
prove per-call enforcement, not just advertisement, blocks it).
Expect: (1) matches verbatim. (2) matches exactly (11 of 13 tools). (3) all three calls
succeed with no Denied text; each audit line carries decision=allow, grant_id=
example-read. (4) returns "Denied (D-...): 'computer (left_click)' needs the 'action'
capability on example.com, and grant 'example-read' allows read. Give this denial id to
your administrator to request 'action' access." and its audit line is decision=deny,
denial_id set, rule capability (verify via the audit file, since the record's rw field
alone does not show the rule). (5) returns the equivalent message naming 'write' and
'form_input'; the extension never receives a form_input tool_request for this call.

## s07-1: the explain tool appears live and does not trigger spuriously during normal browsing
Changed: s07 added the one sanctioned addition to the sacred tool surface, `explain`
(ADR-0022 Decision 7): a server-side, argument-less tool that returns the action
directory and never touches the extension. Only a live MCP client proves it actually
shows up in a real client's tool list, that calling it returns the directory text with
no extension involvement, and that a trained Claude session does not spuriously call it
during ordinary browsing.
Steps: start the mcp-server with no manifest (all-open), extension connected, audit
enabled. (1) In the live MCP client (Claude Code or another client), list the available
browser-mcp tools and confirm `explain` appears (last in the list if the client
preserves server order). (2) Call `explain` directly (or ask the agent to call it) and
read the returned text. (3) With the browser extension's own debug/devtools console
open, confirm no native-messaging frame was sent for the `explain` call (no
`tool_request` for "explain" appears in the extension's logs). (4) Separately, run an
ordinary multi-step browsing session (navigate, screenshot, read_page, click) with a
trained Claude Code session and watch whether it ever calls `explain` unprompted for a
normal "what's on this page" / "explain this" style request.
Expect: (1) `explain` is present in the advertised tool list. (2) the response is one
text block opening with "Capabilities: read = retrieve and observe only; ..." followed
by a blank line and one line per action, ending with "explain: requires nothing. Show
every action available here and the capability each one requires."; no Denied text, no
error. (3) the extension's logs show no `tool_request` entry for `explain` at all,
confirming zero native-messaging frames. (4) the trained session should NOT call
`explain` for ordinary page-content questions (it knows the 13 trained tools already);
if it does spuriously call `explain`, record the exact prompt that triggered it -- per
ADR-0022 Decision 7 this is the signal to consider renaming the tool (e.g.
`tool_capabilities`) in a follow-up decision, not a stage-3 code defect.

## s-live-1: read grant end to end (capability enforcement live)
Changed: stage 3 (s01-s06) replaced observe/mutate with per-action capability requirements
over schema-3 grants; first live run of a read-only grant against real Chrome.
Steps: save this manifest and start the MCP client with `--manifest file://<path>`:
`{ "schema": 3, "name": "s-live-read-check", "version": "1", "grants": [ { "id": "read-only", "hosts": { "allow": ["example.com", "*.example.com"] }, "allowed": ["read"] } ] }`
Then: (1) `tabs_create_mcp`; (2) `navigate` to `https://example.com/`; (3) `read_page` and
a `computer` `screenshot` on that tab; (4) a `computer` `left_click` anywhere on the page;
(5) a `form_input` call on that tab (any element ref; the denial happens before dispatch,
so no matching element needs to exist).
Expect: steps 1-3 succeed normally (`navigate` requires `read` under ADR-0022; a read grant
can navigate, read, and screenshot). Step 4 returns text starting `Denied (D-` containing
`'computer (left_click)' needs the 'action' capability on example.com, and grant 'read-only' allows read`
and the click visibly does not happen. Step 5's denial contains
`'form_input' needs the 'write' capability on example.com, and grant 'read-only' allows read`.

## s-live-2: denied_domain live (allow * with a deny carve-out)
Changed: s04/s05 added host polarity; `hosts.deny` carves holes out of `allow`, producing
the new `denied_domain` rule attributed to the denying grant. First live run.
Steps: start the MCP client with
`{ "schema": 3, "name": "s-live-deny-check", "version": "1", "grants": [ { "id": "everything-but", "hosts": { "allow": ["*"], "deny": ["example.com"] }, "allowed": ["read", "action", "write"] } ] }`
active. (1) `navigate` to `https://example.com/`; (2) `navigate` to `https://example.org/`,
then `read_page` and a `left_click` there.
Expect: step 1 is denied with text starting `Denied (D-` containing
`example.com is excluded by grant 'everything-but': your policy denies this site explicitly`
and the browser does not navigate. Step 2 works end to end (allow `*` covers everywhere
else), including the click.

## s-live-3: the explain tool live (advertised, correct output, no spurious calls)
Changed: s07 added `explain`, the one sanctioned tool-surface addition (ADR-0022 Decision
7); only a live client shows whether a trained model ignores it during normal browsing.
Steps: with any posture (no manifest is fine): (1) list the advertised tools in the client;
(2) ask the agent to call `explain`; (3) run a short normal browsing session (navigate
somewhere, screenshot, then ask the agent to "explain this page").
Expect: step 1 shows `explain` alongside the 13 trained tools. Step 2 returns a single text
block starting `Capabilities: read =` with one requires line per tool and per computer
action. Step 3 answers from page content WITHOUT invoking the `explain` tool; record any
spurious invocation in the live log as a rename signal per ADR-0022 Decision 7.

## s-live-4: audit capability field live
Changed: s06 replaced the audit record's `rw` field with `capability` (ADR-0022 Decision
8); this confirms the real JSONL from a live session.
Steps: with `audit.enabled` true and `audit.file.path` pointed at a scratch file, re-run
the s-live-1 session (same manifest, same calls), then open the audit JSONL file.
Expect: no line contains an `rw` key. The `tabs_create_mcp` line has `"capability":"none"`
and `"grant_id":null`; the `navigate`, `read_page`, and screenshot lines have
`"capability":"read"` and `"grant_id":"read-only"`; the denied left_click line has
`"capability":"action"`, `"decision":"deny"`, `"duration_ms":0`, `"grant_id":"read-only"`;
the denied form_input line has `"capability":"write"`.

## t01-1: org-path policy file loads live (the stage-3 outage fix)
Changed: t01 made parse_manifest the sole loader for the policy file; previously any
org-path policy file was a fatal startup error.
Steps: place a schema-3 manifest with a read-only grant and a mandatory audit.enabled
config entry at the platform org policy path; restart the MCP client; run tabs_context,
a navigate to a granted host, and a computer left_click.
Expect: the server starts; the client's tool list is the governed (filtered) set; the
navigate succeeds; the left_click is denied naming the capability; the audit file
records the calls. Removing the file and restarting restores all-open.

## t05-1: single tab-URL probe live
Changed: t05 unified sacred and grant tab resolution onto tab_url_request (one probe
per call).
Steps: with a sacred list configured and a governed manifest active, run read_page on
a granted tab while watching the extension service-worker console (frame logging on).
Expect: exactly one tab_url_request per read_page call; sacred and grant outcomes
unchanged from s-live-1/g08 expectations.

## t06-1: policy edit applies live (no restart)
Changed: t06 added manifest hot-reload (ADR-0025); grants/mode swap on org-file change.
Steps: with a governed org policy active and a live client session, edit the policy
file to add the "action" capability to the active grant; within a few seconds run a
computer left_click that was previously denied; then delete the policy file and
re-run any denied call.
Expect: the click flips from Denied (capability) to executing, with audit lines
showing the new manifest hash and a manifest_reload session event; after deletion the
session is all-open (14 tools; a client that honors list_changed refreshes its tool
list) and a second manifest_reload event carries manifest null.

## t06-2: broken mid-edit policy never weakens the session
Changed: t06 keep-last-good on reload (fail-closed org matrix extended to grants).
Steps: with a governed session, save the policy file mid-edit as invalid JSON; run a
call outside the grants; then fix the file.
Expect: enforcement continues on the last-good manifest (same denials, same hash) with
an ERROR in the server log and NO manifest_reload event until the fixed save, which
swaps normally.

## t-live-1: stage-4 regression pass (pipeline rewrite)
Changed: stage 4 rewrote the dispatch pipeline (registry-driven, ADR-0024) with
behavior pinned byte-identical by the test wall; only a live pass proves the wall had
no holes.
Steps: re-run the stage-3 backlog s-live-1 through s-live-4 unchanged against the
stage-4 tree, plus t01-1, t05-1, t06-1, and t06-2.
Expect: every expectation in those entries holds unchanged; any divergence is a
stage-4 regression (file it against the pipeline rewrite, not the entry).
