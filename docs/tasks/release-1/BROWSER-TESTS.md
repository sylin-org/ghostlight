# Deferred browser verification checklist

The unattended run cannot touch a live browser, so every verification step
that needs one accumulates here. Run this file top to bottom when you return.

## Before you start (once)

1. Close and restart the MCP client (Claude Code) so it launches the rebuilt
   binary from the release-1-hardening branch build.
   Note: if you want to test WITHOUT merging, build/install from the branch
   first; the registered binary path is what the client launches.
2. Reload the extension at chrome://extensions (Reload button on the dev
   extension).
3. Confirm basic liveness: ask the agent for a screenshot of any page. If
   that fails, run `browser-mcp doctor` (T07) and check --debug state files
   before proceeding.

## Format for entries (agent: follow exactly)

```
## T<NN>-<n>: <one-line purpose>
Changed: <what behavior changed, one sentence>
Steps:
1. <exact instruction, with URL and element>
2. ...
Expect: <the observable result that means PASS, per step where needed>
```

Entries below are appended by the unattended agent, in task order.

---

## T04-1: Fresh-session first-call warmup succeeds instead of racing the handshake
Changed: the binary now starts watching the extension channel at MCP `initialize`, and
`tools/call` waits up to 5s for the channel before failing (previously it failed instantly if
the handshake had not finished). A successful call that had to wait appends a trailing text
block: "(waited N.Ns for browser extension handshake)". This is a binary-only change (no
extension file touched); restarting the MCP client is required, no extension reload.
Steps:
1. Fully close Chrome (all windows), then close and relaunch the MCP client (Claude Code) so it
   starts a fresh mcp-server process.
2. Immediately (within a second or two of the client starting) launch Chrome with the extension
   enabled, and as soon as the MCP client is usable, issue a tool call, e.g. ask it to navigate
   to https://example.com.
Expect: the call succeeds (does not fail with "not connected"). If the handshake was still
settling when the call arrived, the tool result's last content block reads exactly like
"(waited 1.2s for browser extension handshake)" (digits vary). If the handshake had already
finished before the call, there is no such trailing note (the wait was 0, so `waited` stays
`None`).

## T04-2: Chrome fully closed -> exact bounded-timeout error text
Changed: same as T04-1; this exercises the failure path and its exact wording.
UPDATED by T06 (hop-attributed error reporting): the exact wording below supersedes the
original T04-2 text -- T06 replaced the ad hoc timeout message with the hop-attributed
`ToolError` contract; see T06-1 below for the fuller context.
Steps:
1. Fully close Chrome (all windows, ensure no background Chrome process is running the
   extension).
2. With Chrome still closed, start a fresh MCP client session and issue any tool call, e.g.
   navigate to https://example.com.
Expect: the call takes about 5 seconds, then returns an error result whose text is exactly:
"[hop: extension] Browser extension not connected. Next step: check chrome://extensions and
that Chrome is running."
(No extra "Error: " prefix -- errorness is carried by isError, not by a text prefix.)

## T06-1: Every tool-call failure names the hop that broke (binary only, no extension reload)
Changed: every tool-call failure text is now exactly
"[hop: <hop>] <message>. Next step: <next step>." where `<hop>` is one of invalid-request,
binary, ipc, extension, cdp, page. This replaces the old "Error: native messaging error: ..."
wrapper. This step needs only an MCP client restart (binary-only change).
Steps:
1. Close Chrome entirely (or otherwise ensure no extension is connected).
2. Restart the MCP client so it launches the rebuilt binary.
3. Call any tool, e.g. navigate to https://example.com.
Expect: after about 5s, the result text is exactly:
"[hop: extension] Browser extension not connected. Next step: check chrome://extensions and
that Chrome is running."
4. With the MCP client still running and Chrome still closed, call `tools/call` with a bogus
   tool name if your client lets you construct raw calls (otherwise skip this step; it is also
   covered by the automated test `unknown_tool_name_is_rejected_before_dispatch`).
Expect: an immediate (not ~5s) error result reading
"[hop: invalid-request] Unknown tool: <name>. Next step: call tools/list and use one of the
advertised tool names."

## T06-2: Stale `ref` on click / scroll_to / form_input is reported truthfully, not masked
Changed: previously a stale element `ref` (the page changed since `find`/`read_page` produced
it) either reported a misleading "coordinate or ref is required." success-shaped text, silently
substituted [0, 0] for `scroll`, reported a false "Scrolled to target." for `scroll_to`, or
returned form_input's content-script error as a SUCCESS text block prefixed "Error: ...". All
four now surface as a genuine `isError: true` result: "[hop: page] Element <ref> not found; the
page may have changed since it was read." (form_input instead echoes the content script's own
message verbatim, no added wording). Requires reloading the extension at chrome://extensions
AND restarting the MCP client.
Steps:
1. Reload the extension at chrome://extensions, then restart the MCP client.
2. Navigate a grouped tab to a simple static page (e.g. https://example.com) and call `find`
   with a query that matches the page heading; note the returned `ref` (e.g. `ref_1`).
3. Navigate the SAME tab away to a different URL (e.g. https://example.org) so the DOM the ref
   pointed at is gone.
4. Call `computer` with action `left_click` and the stale `ref` from step 2.
Expect: an `isError: true` result reading
"[hop: page] Element ref_1 not found; the page may have changed since it was read. Next step:
take a screenshot or call read_page to re-locate the element, then retry." (ref number varies).
5. Repeat steps 2-3, then call `computer` action `scroll_to` with the stale `ref`.
Expect: the same "[hop: page] Element ref_N not found; ..." error, NOT the previous "Scrolled to
target." success text.
6. Repeat steps 2-3 on a page with a form input, then call `form_input` with the stale `ref` and
   any `value`.
Expect: an `isError: true` result whose text is the content script's own message (e.g.
"Element ref_5 not found or was garbage-collected") with the hop prefix and next step appended,
NOT a "Error: ..." SUCCESS-shaped text block.

## T06-3: chrome:// page blocks content-script injection -> named page-hop failure
Changed: `read_page` (and other content-script-backed tools) on a page where script injection is
blocked (e.g. chrome:// pages) now fails with a named hop instead of an untagged rejection.
Requires reloading the extension AND restarting the MCP client.
Steps:
1. Reload the extension at chrome://extensions, then restart the MCP client.
2. Navigate a grouped tab to chrome://version.
3. Call `read_page` on that tab.
Expect: an `isError: true` result starting with either
"[hop: page] content script unavailable on this page (script injection blocked). Next step:
take a screenshot or call read_page to re-locate the element, then retry." or, if the debugger
attach itself is refused first, "[hop: cdp] debugger attach failed: ...". Either way the text
names a hop (page or cdp), never an untagged/opaque message.

## T06-4: Normal navigate + screenshot flow is unchanged
Changed: nothing on the success path; this is a regression check that hop-attributed error
plumbing did not disturb any success-text wording.
Steps:
1. With the extension reloaded and the MCP client restarted, navigate a grouped tab to
   https://example.com, then call `computer` action `screenshot` on that tab.
Expect: `navigate` returns "Navigated to https://example.com/." (or similar, unchanged wording);
`screenshot` returns the usual "Screenshot captured (jpeg)." text plus an image block, with no
"[hop: ...]" text anywhere and no `isError`.

## T04-3: Server stays responsive to `ping` while a tools/call is waiting
Changed: `tools/call` now runs on its own spawned task and no longer blocks the read loop, so
other protocol traffic (initialize, ping, subsequent calls) keeps flowing while one call is
waiting on the bounded 5s window.
Steps:
1. Start the mcp-server with `--debug` (or `BROWSER_MCP_DEBUG=1`) so the event log is available,
   with Chrome fully closed (so any call will hit the full 5s wait).
2. Pipeline two requests over stdio close together: a `tools/call` (which will wait ~5s), then a
   `ping`.
   (If your MCP client does not expose raw pipelining, this can also be checked by running
   `browser-mcp` directly and piping newline-delimited JSON-RPC requests into stdin by hand;
   see the requests shape used in tests/mcp_protocol.rs.)
Expect: the `ping` response arrives promptly (well under 5s), not only after the `tools/call`
response. Cross-check with `browser-mcp status --json` or the debug event log: the mcp_request
for `ping` is recorded and answered before the delayed `tools/call` response is written.

## T07-1: `browser-mcp doctor` with no MCP session running reports the no-server problem, exit 1
Changed: `doctor` is now a fused, one-shot diagnosis (Binary / Browsers / MCP clients / IPC
endpoint / Debug sessions / Verdict sections) instead of registration-state-only output, and it
now returns a truthful exit code (0 healthy, 1 any problem found). Binary-only change; rebuild
the binary first (rename `target/debug/browser-mcp.exe` aside if a running session holds it
locked, then rebuild). No extension reload needed for this step (no MCP client needs to be
running at all).
Steps:
1. Ensure no MCP client / mcp-server process is running (close the MCP client, or otherwise make
   sure nothing owns the `org.sylin.browser_mcp.v1` IPC endpoint).
2. Run `browser-mcp doctor` from a shell.
3. Check the exit code (`echo $?` in bash, `$LASTEXITCODE` in PowerShell).
Expect: the report shows all six sections in order (Binary, Browsers, MCP clients, IPC endpoint,
Debug sessions, Verdict). The IPC endpoint `state` line reads
"absent (no mcp-server currently owns it)". The Verdict section has at least one
"  problem: no mcp-server is running (the IPC endpoint does not exist): ..." line. The exit code
is 1.

## T07-2: `browser-mcp doctor` during a healthy debug session reports OK, exit 0
Changed: same fusion as T07-1; this exercises the healthy path, including the new clientInfo
capture (Part B) and the extension-connected signal. Requires the dev install to register the
server with `BROWSER_MCP_DEBUG=1` (or manually restart the MCP client with `BROWSER_MCP_DEBUG=1`
set in its environment) and the extension reloaded/attached at least once.
Steps:
1. Restart the MCP client so it launches the rebuilt binary with debug mode on (`--debug` or
   `BROWSER_MCP_DEBUG=1`).
2. Reload the extension at chrome://extensions if it was not already loaded, and make one tool
   call (e.g. navigate to https://example.com) so the extension attaches.
3. Run `browser-mcp doctor` from a shell.
4. Check the exit code.
Expect: IPC endpoint `state` reads
"accepts connections (doctor made one brief probe connection)". Under "Debug sessions", the
newest `mcp-server` row shows `client <name> <version>` where `<name>`/`<version>` match what the
MCP client reports in its `initialize` request (e.g. "claude-code" and its version), NOT
"(not recorded)", and `extension connected` (not "not connected"). The Verdict section is exactly
one line: "  OK: mcp-server (pid <pid>) is running, the extension is connected, and the IPC
endpoint accepts connections." Exit code is 0.

## T07-3: `browser-mcp doctor` catches a disconnected extension, then recovers
Changed: same fusion; this exercises Verdict rule 6 (extension disconnected from a live
mcp-server). No rebuild needed beyond T07-2's if already done.
Steps:
1. With the debug session from T07-2 still running (mcp-server up, extension was connected),
   disable the extension at chrome://extensions (or otherwise stop its service worker) and wait a
   few seconds for the mcp-server to observe the disconnect.
2. Run `browser-mcp doctor`.
3. Re-enable the extension at chrome://extensions, make one more tool call so it reattaches, then
   run `browser-mcp doctor` again.
Expect step 2: a Verdict problem line naming the mcp-server's pid, either
"the extension is disconnected from the mcp-server (pid <pid>; it connected <n> time(s) earlier
in this session): ..." (if it had connected before) -- exit code 1.
Expect step 3: doctor returns to the single "OK: ..." Verdict line and exit code 0.

## T07-4: `browser-mcp doctor --verbose` shows every session with its counters
Changed: `--verbose` (previously ignored by the installer's doctor) now lifts the 6-row display
cap on the Debug sessions section and prints a `counters:` line under every row.
Steps:
1. With at least one debug session on record (from T07-2/T07-3), run
   `browser-mcp doctor --verbose`.
Expect: every session row (not just the newest 6) is shown, with no
"(and N older; use --verbose to show all)" line, and each session row is immediately followed by
a line reading
"      counters: requests=<n> tools=<n> errors=<n> frames_out=<n> frames_in=<n> connects=<n>
disconnects=<n>" with real (non-placeholder) numbers.

## T07-5: `browser-mcp status` still works during a debug session (role filtering regression check)
Changed: `status_report()` is now role-aware (only reports mcp-server sessions); this confirms
that filtering did not silently break the existing `status` command.
Steps:
1. With the debug session from T07-2 running, run `browser-mcp status` (no flags).
Expect: the usual formatted report (pid, uptime, extension connected/not, counters, in-flight,
recent events) renders exactly as before this change -- no "no mcp-server debug state" message
while a real session is live.

## T07-6 (optional): native-host debug state file and the extension-last-seen line
Changed: the native-host role now writes its own `debug-state-<pid>.json` / `debug-events-<pid>.jsonl`
files, but only when Chrome itself was launched with `BROWSER_MCP_DEBUG=1` set in its environment
(Chrome does not pass `--debug` to the process it spawns, so this is opt-in and its absence is
normal -- do not treat a missing native-host row as a problem).
Steps:
1. Fully close Chrome.
2. Launch Chrome from a shell with `BROWSER_MCP_DEBUG=1` set in that shell's environment (so the
   native-host process Chrome spawns inherits it), with the extension enabled.
3. Make one tool call from the MCP client so the extension attaches.
4. Run `browser-mcp doctor` (with the mcp-server also in debug mode, per T07-2, for the fullest
   picture).
Expect: the Debug sessions section includes a `native-host` row
("  native-host   pid <pid>  started <S> ago  active <A> ago", no client/extension fields on that
row), and, after the session rows, a line reading
"  extension last seen <A> ago (native-host pid <pid>)". Separately, confirm launching Chrome
WITHOUT `BROWSER_MCP_DEBUG=1` (the normal case) produces no native-host row and no problem line
about its absence.

## T01-1: Small page still renders byte-identical read_page output
Changed: `accessibilityTree` in extension/content.js was rewritten from a serialize-as-you-walk
design to a two-pass measure/emit design (structural pagination). When output fits the character
budget the intent is byte-identical output to before this change (same lines, same order, same
refs, no markers, no summary line). Extension-only change: requires reloading the extension at
chrome://extensions; no MCP client restart needed.
Steps:
1. Reload the extension at chrome://extensions.
2. Navigate a grouped tab to https://example.com.
3. Call `read_page` with only `tabId` set (all other args default: filter="all", depth=15,
   max_chars=50000).
Expect: a short accessibility tree (heading, paragraph, link, etc.), each shown line ending in
`[ref_N]`, no lines containing "[subtree collapsed:", no line starting with "[element cap
reached:" or "[showing", no "... (truncated)" anywhere, and the output ends with a blank line
then "Viewport: WxH". If you have a pre-change capture of this exact call, diff them: they should
be identical.

## T01-2: Large page triggers structural pagination with collapse markers and a summary line
Changed: same as T01-1; this exercises the over-budget path.
Steps:
1. With the extension reloaded, navigate a grouped tab to
   https://en.wikipedia.org/wiki/Web_browser.
2. Call `read_page` with `max_chars: 2000` (defaults for everything else).
Expect: only complete lines (no line is cut mid-word, no "... (truncated)" string anywhere), one
or more lines matching exactly
"<indent>  [subtree collapsed: <N> elements; call read_page with ref_id=ref_<M> to expand]"
(N and M vary), followed near the end by a line matching exactly
"[showing <M> of <T> elements; expand a collapsed subtree with ref_id, or narrow with
filter=\"interactive\" or a smaller depth]" (M <= T, both plausible integers), then the usual
"Viewport: WxH" trailer as the last line.

## T01-3: Expanding a collapsed subtree via ref_id gets a fresh budget rooted there
Changed: same as T01-1/T01-2; exercises re-rooting the walk at a collapsed subtree's ref.
Steps:
1. Take a `ref_<M>` value from a collapse marker line produced in T01-2 (same tab, same page,
   same session -- refs are WeakRef-backed and only valid while the page/tab is unchanged).
2. Call `read_page` on the same tab with `ref_id: "ref_<M>"` and default `max_chars`.
Expect: the output is rooted at that element's subtree (its own lines and descendants, own fresh
"[showing ...]" or unmarked output depending on its own size), not the whole page again.

## T01-4: filter="interactive" and depth still shrink output as before
Changed: none to this behavior; regression check that pagination did not disturb filter/depth
handling (both were already honored and are explicitly out of scope for structural changes).
Steps:
1. On https://en.wikipedia.org/wiki/Web_browser, call `read_page` with `filter: "interactive"`
   and default depth/max_chars.
2. Separately, call `read_page` with `depth: 3` and default filter/max_chars.
Expect: step 1 shows substantially fewer lines than the "all" filter, only interactive elements
(links, buttons, inputs, etc.) and their containers. Step 2 shows a shallower tree (no lines more
than 3 levels of indent below the root). Neither call should be required to trigger a collapse
marker unless the shrunk output still exceeds max_chars (50000 default; unlikely at depth 3 or
filter=interactive on this page, but not a failure if it does -- markers are correct behavior at
any size).

## T01-5 (synthetic): the 10000-element cap fires and reports an exact count
Changed: new hard backstop (`MAX_ELEMENTS = 10000`) with a dedicated cap line ahead of the
summary line when it fires.
Steps:
1. Navigate a grouped tab to any simple page (e.g. https://example.com).
2. Call `javascript_tool` on that tab with the expression:
   `document.body.innerHTML = Array.from({length: 12000}, (_, i) => "<span>item " + i + "</span>").join(""); "ok"`
3. Call `read_page` on the same tab with `max_chars: 2000000` (large enough that the character
   budget is not the limiting factor).
Expect: exactly 10000 `span "item <n>" [ref_N]`-shaped lines, then a line reading exactly
"[element cap reached: output stopped after 10000 elements; use filter=\"interactive\", a ref_id
subtree, or a smaller depth]", then a line reading exactly
"[showing 10000 of 12000 elements; expand a collapsed subtree with ref_id, or narrow with
filter=\"interactive\" or a smaller depth]", then the "Viewport: WxH" trailer.

## T01-6: Stale ref_id still returns the unchanged error string
Changed: nothing (regression check); the stale-ref error path was explicitly preserved verbatim.
Steps:
1. On any grouped tab, call `read_page` with `ref_id: "ref_99999"` (a ref number that was never
   assigned in this page session).
Expect: the result text is exactly
`Error: ref_id "ref_99999" not found or was garbage-collected.`
(no markers, no summary line, no viewport trailer -- this is a plain string return, unchanged from
before this task).

## T02-1: filter=interactive only shows on-screen elements, with the Note line
Changed: `read_page` with `filter: "interactive"` now culls elements whose bounding rect does not
intersect the current viewport (via `getBoundingClientRect`), and appends one extra trailer line
when culling removed anything.
Steps:
1. With the extension reloaded, navigate a grouped tab to
   https://en.wikipedia.org/wiki/Web_browser (a long page, scrolled to the top).
2. Call `read_page` with `filter: "interactive"` (defaults for everything else).
Expect: every emitted element line corresponds to something currently visible on screen (no
off-screen links/buttons from far down the article). The very last line of the output is exactly
"Note: interactive results are limited to the current viewport; scroll or use filter=all for the
full document." (this line comes after the "Viewport: WxH" line).

## T02-2: Scrolling changes which interactive elements appear
Changed: same as T02-1; exercises that culling is scroll-position-relative, not a one-time compute.
Steps:
1. Same tab as T02-1, already scrolled to the top with a prior `filter: "interactive"` result
   recorded.
2. Call `computer` with action `scroll` to scroll down several screens (e.g. scroll down by a
   large amount, or use `scroll_to` on a ref far down the page from a `filter: "all"` call).
3. Call `read_page` with `filter: "interactive"` again on the same tab.
Expect: the set of `ref_N` interactive elements returned in step 3 differs from the set returned in
step 2's precursor (T02-1) -- new links/buttons that are now on screen appear, and elements that
were on screen before but have scrolled off no longer appear. The trailing Note line is still
present (still a long page with more off-screen content).

## T02-3: filter=all is unaffected -- no Note line, full document
Changed: nothing observable for `filter=all`; regression check that culling never applies there.
Steps:
1. Same tab as T02-1/T02-2 (any scroll position).
2. Call `read_page` with `filter: "all"` (or omit `filter` entirely -- "all" is the default).
Expect: the output includes off-screen elements from elsewhere in the document (not just what is
currently on screen), and the output ends with the "Viewport: WxH" line with nothing after it --
no "Note: interactive results are limited..." line, regardless of scroll position.

## T02-4: A short page that fits the viewport produces no Note line
Changed: same mechanism as T02-1; exercises the "nothing was culled" branch of the new note logic.
Steps:
1. Navigate a grouped tab to a short page whose interactive elements (if any) all fit within one
   screen without scrolling, for example https://example.com (it has exactly one link, "More
   information...", near the top).
2. Call `read_page` with `filter: "interactive"`.
Expect: the output ends with the "Viewport: WxH" line and nothing after it -- no Note line, since
nothing was off-screen to cull.

## T03-1: get_page_text picks the largest-innerText candidate, with the Source element header
Changed: `get_page_text` no longer picks the first matching selector or reads `textContent` off a
cloned node; it now scans every element matching any of twelve candidate selectors, picks the one
with the strictly largest `innerText`, and prefixes the output with "Source element: <selector>".
Paragraph breaks now survive (innerText preserves layout line breaks; the old textContent path
collapsed all whitespace to single spaces). Extension-only change: requires reloading the
extension at chrome://extensions; no MCP client restart needed.
Steps:
1. Reload the extension at chrome://extensions.
2. Navigate a grouped tab to a text-heavy Wikipedia article, for example
   https://en.wikipedia.org/wiki/Web_browser.
3. Call `get_page_text` with only `tabId` set (no `max_chars`).
Expect: the output starts with "Source element: " followed by one of the twelve candidate
selectors (for example "main" or ".content"), and the body text below it is broken into multiple
paragraphs separated by blank lines (not one giant single-spaced line). No "Title:" or "URL:"
line appears anywhere.

## T03-2: max_chars truncates with the exact bracketed notice
Changed: `max_chars` (previously ignored end to end) now bounds the normalized body text; the
service worker forwards it unchanged and the content script floors/validates it, defaulting to
50000 for anything absent or invalid.
Steps:
1. On the same tab as T03-1 (extension already reloaded), call `get_page_text` with
   `max_chars: 500`.
Expect: the output is "Source element: <selector>", a blank line, roughly 500 characters of body
text, a blank line, then a line reading exactly
"[Truncated at 500 characters. Retry with a larger max_chars, or use read_page to get a
structured view with element refs.]" (the number matches the `max_chars` you passed).

## T03-3: No readable text produces the actionable no-content message
Changed: previously an empty/near-empty page silently returned "Title: ...\nURL: ...\n\n" with no
text; now it returns a single actionable line naming the source element and suggesting
`read_page`.
Steps:
1. Navigate a grouped tab to about:blank.
2. Call `get_page_text` with only `tabId` set.
Expect: the output is exactly one line:
"No readable text content found (source element: body). The page may be mostly visual or may
render text dynamically. Use read_page to inspect the page structure instead."
(No "Source element:" header, no blank body.)

## T03-4: Hidden text (display:none) is excluded, unlike the old textContent implementation
Changed: switching from `textContent` on a cloned node to `innerText` means CSS-hidden text (for
example `display:none` banners or collapsed sections) is no longer included in the output. This
is a direct regression check against the old behavior.
Steps:
1. Navigate a grouped tab to a simple page, for example https://example.com.
2. Call `javascript_tool` on that tab with the expression:
   `const d = document.createElement("div"); d.style.display = "none"; d.textContent =
   "HIDDEN_MARKER_XYZ"; document.body.appendChild(d); "ok"`
3. Call `get_page_text` on the same tab with only `tabId` set.
Expect: the output does NOT contain the string "HIDDEN_MARKER_XYZ" anywhere. (The pre-T03
`textContent`-based implementation would have included it; this confirms the switch to
`innerText` actually excludes CSS-hidden content.)

## T12-1: Cross-domain navigation clears network requests
Changed: the network buffer is now keyed to the tab's current hostname; navigating a tab to a
different hostname replaces its buffer with a fresh empty one owned by the new hostname, so a
read after a cross-domain navigation never returns the old domain's requests. Extension-only
change: requires reloading the extension at chrome://extensions; no MCP client restart needed.
Steps:
1. Reload the extension at chrome://extensions.
2. Create a tab in the group and navigate it to https://example.com/.
3. Call `read_network_requests` on that tab (this enables Network tracking for the first time).
4. Navigate the same tab to https://example.com/ again (reload) to capture some traffic.
5. Call `read_network_requests` again.
Expect (step 5): the output contains example.com request lines (URLs starting with
"https://example.com/" or "http://example.com/").
6. Navigate the SAME tab to a different domain, for example https://www.iana.org/.
7. Call `read_network_requests`.
Expect (step 7): the output contains no example.com URLs anywhere. It is fine (and expected per
the accepted CDP-race limitation) if the very first iana.org document request is missing or
appears as a response-only "? https://www.iana.org/ -> 200" style line; seeing any example.com
traffic here is a failure.

## T12-2: Same-hostname navigation retains earlier requests
Changed: same as T12-1; this checks the non-reset side of the same rule (an unchanged hostname
must NOT reset the buffer), including SPA-style same-hostname URL changes.
Steps:
1. Continuing from T12-1 (tab currently on https://www.iana.org/, buffer already has iana.org
   entries from step 7 above), navigate the same tab to https://www.iana.org/domains (a
   different path, same hostname).
2. Call `read_network_requests`.
Expect: the output contains BOTH the request(s) captured on the earlier "/" page from T12-1 step
7 AND the new requests from /domains -- nothing was dropped by the path-only navigation.

## T12-3: Console messages are domain-scoped the same way
Changed: the console buffer follows the identical per-hostname ownership rule as the network
buffer.
Steps:
1. Navigate a grouped tab to https://example.com/.
2. Call `read_console_messages` on that tab once (enables Runtime tracking for the first time;
   ignore its output).
3. Call `javascript_tool` on the same tab with the expression: `console.log("marker-A"); "ok"`.
4. Call `read_console_messages` on the same tab.
Expect (step 4): the output contains the line "[log] marker-A".
5. Navigate the same tab to a different domain, for example https://www.iana.org/.
6. Call `read_console_messages` on the same tab.
Expect (step 6): the output does NOT contain "marker-A" (either "No console messages matching the
pattern." if nothing else was logged yet, or only iana.org-originated messages if the page itself
logs something).
7. Call `javascript_tool` on the same tab with the expression: `console.log("marker-B"); "ok"`.
8. Call `read_console_messages` on the same tab.
Expect (step 8): the output contains "[log] marker-B" and still does NOT contain "marker-A".

## T12-4: clear still works after the per-domain change
Changed: `read_network_requests`'s `clear: true` parameter still empties the buffer as before,
now via the new `{ host, items: [] }` shape.
Steps:
1. On a grouped tab with some captured network traffic (for example continuing from T12-1/T12-2),
   call `read_network_requests` with `clear: true`.
2. Perform a page action that generates at least one new request (for example a reload).
3. Call `read_network_requests` again (no `clear`).
Expect (step 3): the output shows only requests made after the clear in step 1 -- none of the
pre-clear requests reappear.

## T12-5: Tab close cleanup runs without errors
Changed: the `chrome.tabs.onRemoved` listener now also deletes the tab's `tabHost` entry
alongside the existing buffer/context cleanup.
Steps:
1. With a grouped tab that has an attached debugger and some buffered console/network entries
   (any tab used in T12-1 through T12-4 qualifies), close that tab.
2. Open the extension's service worker console at chrome://extensions (click "service worker"
   under the Browser MCP extension) and check for errors logged around the time of the close.
Expect: no errors appear in the service worker console from the tab-removal cleanup path.

## T13-1: Deferred uncaught exception appears as a console entry with level "exception"
Changed: `chrome.debugger.onEvent` now handles `Runtime.exceptionThrown` (previously silently
dropped) and pushes a synthetic `{ level: "exception", text }` entry into the same per-tab
console buffer `Runtime.consoleAPICalled` writes to. `read_console_messages`'s `onlyErrors`
filter already accepted `"exception"`, so no change was needed there. Order matters below: the
Runtime CDP domain is only enabled by the first `read_console_messages` call for a tab.
Steps:
1. Navigate a tab in the MCP tab group to any page (for example https://example.com).
2. Call `read_console_messages` once for that tab (this enables the Runtime domain; the result
   will likely be "No console messages matching the pattern.", which is fine).
3. Call `javascript_tool` with text:
   `setTimeout(() => { throw new Error("t13 test"); }, 0); "scheduled"`
   (the setTimeout matters: throwing directly inside the evaluated expression surfaces in the
   evaluate response itself and never emits `Runtime.exceptionThrown`; the deferred throw is a
   genuine uncaught page exception).
4. Call `read_console_messages` with `onlyErrors: true`.
Expect (step 4): the output contains a line beginning `[exception] Error: t13 test` followed by
a `(url:line)` location and a compact `[at ...]` stack, for example something like
`[exception] Error: t13 test (https://example.com/:1) [at <anonymous>@https://example.com/:1]`
(exact frame names/URLs depend on the page).

## T13-2: Ordinary console levels are unaffected (no double counting)
Changed: same as T13-1; this confirms the new branch does not disturb the existing
`Runtime.consoleAPICalled` path.
Steps:
1. Continuing from T13-1 (same tab, Runtime domain already enabled), call `javascript_tool`
   with text `console.log("t13 plain")`.
2. Call `read_console_messages` without `onlyErrors`.
Expect: `[log] t13 plain` appears exactly once, alongside the `[exception] Error: t13 test` line
from T13-1 (both present, neither duplicated).

## T13-3: pattern filtering matches the exception text
Changed: same as T13-1; confirms the new entry participates in the existing `pattern` filter
like any other console entry.
Steps:
1. Continuing from T13-1/T13-2, call `read_console_messages` with `pattern: "t13 test"`.
Expect: the output contains only the `[exception] Error: t13 test ...` line and nothing else
(not the `[log] t13 plain` line).

## T14-1: A failed fetch shows a stand-in 503 with the CDP error text instead of staying pending
Changed: `chrome.debugger.onEvent` now handles `Network.loadingFailed` (previously silently
dropped). When the failure matches an existing buffered request (by `requestId`), the entry's
`status` is set to 503, `errorText` is recorded from the CDP event when present, and `canceled`
is recorded as a boolean. `read_network_requests` now appends ` (<errorText>)` after the status
whenever `errorText` is present. Order matters below: the Network CDP domain is only enabled by
the first `read_network_requests` call for a tab.
Steps:
1. Navigate a tab in the MCP tab group to any page (for example https://example.com).
2. Call `read_network_requests` once for that tab (this enables the Network domain; the result
   will likely be "No network requests matching the pattern.", which is fine).
3. Call `javascript_tool` with text:
   `fetch("https://no-such-host-t14.invalid/").catch(() => "failed")`
4. Wait a moment, then call `read_network_requests` again.
Expect (step 4): a line of the form
`GET https://no-such-host-t14.invalid/ -> 503 (net::ERR_NAME_NOT_RESOLVED)`.

## T14-2: Ordinary successful requests are unaffected
Changed: same as T14-1; confirms the new branch does not disturb the existing
`Network.requestWillBeSent` / `Network.responseReceived` rendering.
Steps:
1. Continuing from T14-1 (same tab, Network domain already enabled), call `javascript_tool`
   with text `fetch("https://example.com/").catch(() => "failed")`.
2. Call `read_network_requests` again.
Expect: a line of the form `GET https://example.com/ -> 200`, with no `errorText` suffix, still
present alongside the T14-1 failure line (both present, neither altered).

## T14-3: A client-aborted request renders with net::ERR_ABORTED and canceled recorded
Changed: same as T14-1; confirms the `canceled` flag is captured (not rendered, per the task's
Out of scope, but exercised here to confirm the branch does not throw when `canceled` is true).
Steps:
1. Continuing from T14-1/T14-2, call `javascript_tool` with text:
   `(() => { const c = new AbortController(); fetch("https://example.com/", { signal: c.signal }).catch(() => "aborted"); c.abort(); return "ok"; })()`
2. Wait a moment, then call `read_network_requests` again.
Expect: a new line of the form `GET https://example.com/ -> 503 (net::ERR_ABORTED)` (the URL
requested with the abort signal); the earlier T14-1 and T14-2 lines are unchanged.

## T14-4: A genuinely in-flight request still renders (pending)
Changed: same as T14-1; confirms `Network.loadingFailed` never fires for a request that has not
failed, so unrelated in-flight requests keep the pre-existing `(pending)` text.
Steps:
1. Continuing from T14-1/T14-2/T14-3, call `javascript_tool` with text:
   `fetch("https://httpbin.org/delay/10").catch(() => "failed"); "started"`
2. Immediately (within a second or two) call `read_network_requests` again, before the 10s delay
   endpoint could have responded or failed.
Expect: a line of the form `GET https://httpbin.org/delay/10 (pending)` (no `->`, no status,
unchanged wording), alongside the T14-1/T14-2/T14-3 lines.

## T15-1: Fresh tab, first call to read_console_messages shows the buffer-empty variant
Changed: the zero-result string for `read_console_messages` now distinguishes "buffer never
populated" from "buffer has entries but none matched the filter," and always adds a note
explaining that tracking starts on first use. Extension-only change; reload the extension at
chrome://extensions, no MCP client restart, no binary rebuild.
Steps:
1. Open a brand-new tab and add it to the group (navigate it into scope, e.g. to
   https://example.com), but do not call `read_console_messages` yet.
2. Call `read_console_messages` with only `tabId` set (no `pattern`, no `onlyErrors`).
Expect: the result text is exactly two lines:
   No console messages recorded for this tab.
   Note: console tracking begins when this tool is first used on a tab. Reload the page to capture messages emitted during page load.

## T15-2: Reloading the page after the first call captures load-time console output
Changed: same as T15-1; confirms tracking now active (unchanged buffering behavior from T12/T13),
and non-empty output format is byte-identical to before this task.
Steps:
1. Continuing from T15-1, reload the page (e.g. navigate to a page that logs on load, such as
   https://example.com or any page you control that runs `console.log("hello")` in an inline
   script) and call `read_console_messages` again.
Expect: if the page logged anything during load, the result shows normal `[level] text` lines
(one per message, no note appended); if the page logged nothing, the T15-1 buffer-empty variant
repeats (total is still 0).

## T15-3: Non-matching pattern shows the buffer-has-entries variant with the correct count
Changed: same as T15-1.
Steps:
1. Continuing from T15-2 (buffer now has at least one message; if not, run
   `javascript_tool` with text `console.log("t15 probe"); "logged"` first, then call
   `read_console_messages` once with no pattern to confirm it appears).
2. Call `read_console_messages` with `pattern` set to `"zzz_no_such_pattern"`.
Expect: the result text is exactly two lines:
   N console message(s) recorded for this tab, but none matched your filter.
   Note: console tracking begins when this tool is first used on a tab. Reload the page to capture messages emitted during page load.
   where N is the buffered total from step 1 (a plain integer, no pluralization change to the
   literal `(s)` suffix).

## T15-4: read_network_requests fresh-tab and non-matching-filter variants
Changed: same shape as T15-1/T15-3, network wording.
Steps:
1. Open another brand-new tab into the group and, without calling `read_network_requests` yet,
   call it once with only `tabId` set.
Expect: exactly two lines:
   No network requests recorded for this tab.
   Note: network tracking begins when this tool is first used on a tab. Reload the page to capture requests made during page load, or interact with the page to trigger new requests.
2. Navigate that tab to https://example.com (or reload it) so at least one request is buffered,
   then call `read_network_requests` with `urlPattern` set to a string that cannot match (for
   example `"zzz_no_such_url"`).
Expect: exactly two lines:
   N network request(s) recorded for this tab, but none matched your filter.
   Note: network tracking begins when this tool is first used on a tab. Reload the page to capture requests made during page load, or interact with the page to trigger new requests.
   where N is the buffered total (a plain integer).

## T15-5: clear still empties the buffer on a zero-match call
Changed: confirms `clear` fires even when the filtered result is empty (unchanged clear
mechanics; only the returned text changed).
Steps:
1. Continuing from T15-4 (buffer has at least one network request), call
   `read_network_requests` with `urlPattern` set to a non-matching string AND `clear: true`.
2. Immediately call `read_network_requests` again with only `tabId` (no filter).
Expect: step 1 shows the "N network request(s) recorded... but none matched your filter." variant
(clear happens after the filtered-zero result is computed, so N still reflects the pre-clear
total). Step 2 shows the buffer-empty variant ("No network requests recorded for this tab."),
confirming the buffer was actually emptied by step 1's `clear: true`.

## T15-6: Non-empty results are unchanged
Changed: confirms this task touched no formatting on the non-empty branch of either tool.
Steps:
1. With console messages present (from T15-2/T15-3), call `read_console_messages` with no
   pattern.
2. With network requests present (from T15-4/T15-5), call `read_network_requests` with no
   urlPattern.
Expect: step 1 shows `[level] text` lines exactly as before this task (no note appended, no
change to per-line format). Step 2 shows `<METHOD> <URL> -> <STATUS>` (or `(pending)` /
`-> <STATUS> (<errorText>)` per T14) lines exactly as before this task (no note appended).

## T08-1: type dispatches real keydown/keyup for every printable ASCII character
Changed: the `type` action of `computer` no longer inserts every character via
`Input.insertText` alone; it now dispatches a real `Input.dispatchKeyEvent` keyDown/keyUp pair
per printable ASCII character (with a correct Shift bit for shifted characters), maps a newline
to a real Enter press, and falls back to `Input.insertText` only for characters with no key
mapping (control characters, non-ASCII). Extension-only change; reload the extension at
chrome://extensions, no MCP client restart needed.
Steps:
1. `navigate` to https://example.com.
2. `javascript_tool` to prepare a probe input and event log:
   ```js
   const inp = document.createElement("input");
   inp.id = "t08probe";
   document.body.prepend(inp);
   window.__ev = [];
   for (const t of ["keydown", "keyup", "input"]) {
     inp.addEventListener(t, (e) => window.__ev.push(
       t + "|" + (e.key || "") + "|" + (e.code || "") + "|" + (e.shiftKey ? 1 : 0)));
   }
   inp.focus();
   ```
3. `computer` with `{ "action": "type", "text": "Ab1!;:\n" }` on that tab.
4. `javascript_tool` to read
   `JSON.stringify({ v: document.getElementById("t08probe").value, ev: window.__ev })`.
Expect: `v` is `Ab1!;:` (Enter adds no character to a single-line input). Every typed character
produced a keydown and a keyup entry in `window.__ev`. `A` shows `keydown|A|KeyA|1` (shift bit
set), `b` shows `keydown|b|KeyB|0`, `1` shows `keydown|1|Digit1|0`, `!` shows
`keydown|!|Digit1|1`, `;` shows `keydown|;|Semicolon|0`, `:` shows `keydown|:|Semicolon|1`, and
the final entries are `keydown|Enter|Enter|0` and `keyup|Enter|Enter|0`. The tool result text
reads exactly `Typed 7 character(s).` (the raw length of `"Ab1!;:\n"`).

## T08-2: Non-ASCII characters fall back to Input.insertText, ASCII characters still dispatch
Changed: same rework as T08-1; this exercises the fallback path specifically.
Steps:
1. Continuing on the same tab/probe from T08-1 (or set up a fresh probe input per T08-1 steps
   1-2 if starting fresh), clear `window.__ev = [];` and clear the probe input's value.
2. `computer` with a `text` argument equal to the JSON string `caf` followed by the 6-character
   escape sequence backslash-u-0-0-e-9 (the JSON escape keeps this file ASCII; it decodes to a
   word ending in an accented e, U+00E9).
3. `javascript_tool` to read
   `JSON.stringify({ v: document.getElementById("t08probe").value, ev: window.__ev })`.
Expect: `v` ends with the accented character (the word "cafe" with an accented final e).
`window.__ev` shows keydown/keyup pairs for `c`, `a`, `f`, but only an `input` entry (no keydown,
no keyup) for the accented character, proving the per-character `Input.insertText` fallback
fired for that one character alone.

## T08-3: CRLF collapses to a single Enter press
Changed: same rework as T08-1; this exercises the `\r\n` collapsing rule.
Steps:
1. Continuing on the same probe (clear `window.__ev = [];` and the input's value first).
2. `computer` with `{ "action": "type", "text": "a\r\nb" }`.
3. `javascript_tool` to read `JSON.stringify(window.__ev)`.
Expect: exactly one Enter keydown/keyup pair appears between the events for `a` and `b` (not
two), confirming the `\r` before a `\n` is skipped rather than producing a second Enter press.

## T09-1: double_click on a word selects it (real clickCount 1 then 2)
Changed: `click()` in `extension/service-worker.js` now dispatches N press/release pairs with
clickCount incrementing 1..N instead of one pair carrying clickCount=N; every mouse event also
carries an explicit `buttons` bitmask and `force`. Extension-only change; reload the extension at
chrome://extensions, no MCP client restart needed.
Steps:
1. `navigate` to https://en.wikipedia.org/wiki/Cat (or any text-heavy article page).
2. `javascript_tool` to install a click-detail probe on the page and report a word's coordinates:
   ```js
   window.__clicks = [];
   for (const t of ["mousedown", "mouseup", "click", "dblclick"]) {
     document.addEventListener(t, (e) => window.__clicks.push(
       t + "|detail=" + e.detail + "|button=" + e.button + "|buttons=" + e.buttons), true);
   }
   const p = document.querySelector("#mw-content-text p");
   const r = p.getBoundingClientRect();
   JSON.stringify({ x: Math.round(r.left + 40), y: Math.round(r.top + 10) });
   ```
3. `computer` with `{ "action": "double_click", "coordinate": [x, y] }` using the coordinates
   from step 2 (screenshot first if a fresh screenshot-space coordinate is preferred instead).
4. `javascript_tool` to read `JSON.stringify({ sel: window.getSelection().toString(),
   ev: window.__clicks })`.
Expect: `sel` is a non-empty single word (the word under the click point is selected -- this only
happens when the page observes a real click (detail=1) immediately followed by a second click
(detail=2) on the same target, which is how browsers compute word-select). `ev` shows, in order:
`mousedown|detail=1|...|buttons=1`, `mouseup|detail=1|...|buttons=0`, `click|detail=1|...`,
`mousedown|detail=2|...|buttons=1`, `mouseup|detail=2|...|buttons=0`, `click|detail=2|...`,
`dblclick|detail=2|...`. No entry has detail=0 for a press/release.

## T09-2: triple_click on a paragraph selects the whole line/paragraph
Changed: same rework as T09-1.
Steps:
1. Continuing on the same page/probe as T09-1 (clear `window.__clicks = [];` first, and clear
   the previous selection with `window.getSelection().removeAllRanges()`).
2. `computer` with `{ "action": "triple_click", "coordinate": [x, y] }` on the same paragraph
   coordinates as T09-1.
3. `javascript_tool` to read `JSON.stringify({ sel: window.getSelection().toString().length,
   ev: window.__clicks })`.
Expect: `sel` is much larger than one word (the whole line or paragraph is selected -- this only
happens when the page sees three sequential clicks with detail 1, then 2, then 3 on the same
target). `ev` shows three mousedown/mouseup/click cycles with detail=1, detail=2, detail=3 in
that order (never a pair whose first detail is 2 or 3), and a final `dblclick|detail=2` plus (per
the browser's own triple-click semantics) no separate "tripleclick" DOM event -- what matters is
that the three detail values appear in strict 1, 2, 3 order, not 1, 1, 1 or a single detail=3
pair.

## T09-3: left_click is a single normal activation with no accidental double-click side effects
Changed: same rework as T09-1; this exercises the plain (N=1) path, which now runs through the
same incrementing loop but with a single iteration.
Steps:
1. `navigate` to https://example.com.
2. `javascript_tool` to install a probe on the "More information..." link:
   ```js
   window.__clicks = [];
   const a = document.querySelector("a");
   for (const t of ["mousedown", "mouseup", "click", "dblclick"]) {
     a.addEventListener(t, (e) => window.__clicks.push(
       t + "|detail=" + e.detail + "|buttons=" + e.buttons));
   }
   const r = a.getBoundingClientRect();
   JSON.stringify({ x: Math.round(r.left + 5), y: Math.round(r.top + 5) });
   ```
3. `computer` with `{ "action": "left_click", "coordinate": [x, y] }` using the coordinates from
   step 2.
4. `javascript_tool` to read `JSON.stringify(window.__clicks)` (do this before any navigation
   the click may have triggered finishes, or check history/back if the page already navigated).
Expect: exactly one mousedown/mouseup/click cycle, all with detail=1 (`buttons=1` on mousedown,
`buttons=0` on mouseup/click). No `dblclick` event fires. The link activates normally (a real
navigation occurs, or the click event's default was not prevented).

## T09-4: right_click carries buttons=2 while pressed
Changed: same rework as T09-1; the right-click path now sets `buttons: BUTTON_BITS.right` (2) on
`mousePressed` instead of omitting `buttons` entirely.
Steps:
1. `navigate` to https://example.com.
2. `javascript_tool` to install a probe: `window.__rc = []; document.addEventListener("mousedown",
   (e) => window.__rc.push("button=" + e.button + "|buttons=" + e.buttons)); document
   .addEventListener("contextmenu", (e) => window.__rc.push("contextmenu"));
   JSON.stringify("ok")`.
3. `computer` with `{ "action": "right_click", "coordinate": [100, 100] }`.
4. `javascript_tool` to read `JSON.stringify(window.__rc)`.
Expect: a `mousedown` entry with `button=2|buttons=2`, followed by a `contextmenu` entry (the
page's native context-menu handling still fires, unaffected by this change).

## T09-5: left_click_drag creates a selection and reports buttons=1 throughout
Changed: `left_click_drag` in `extension/service-worker.js` now sets `buttons: 0, force: 0` on
its opening `mouseMoved`, `buttons: BUTTON_BITS.left, force: 0.5` on `mousePressed` and every
interpolated `mouseMoved`, and `buttons: 0, force: 0` on the final `mouseReleased`. No
`clickCount` was added to the press/release events (unchanged from before).
Steps:
1. `navigate` to https://en.wikipedia.org/wiki/Cat (or any text-heavy article page).
2. `javascript_tool` to install a probe and read a paragraph's start/end drag coordinates:
   ```js
   window.__drag = [];
   document.addEventListener("mousemove", (e) => { if (e.buttons) window.__drag.push(e.buttons); });
   const p = document.querySelector("#mw-content-text p");
   const r = p.getBoundingClientRect();
   JSON.stringify({ sx: Math.round(r.left + 5), sy: Math.round(r.top + 10),
     ex: Math.round(r.left + 200), ey: Math.round(r.top + 10) });
   ```
3. `computer` with `{ "action": "left_click_drag", "start_coordinate": [sx, sy],
   "coordinate": [ex, ey] }` using the coordinates from step 2.
4. `javascript_tool` to read `JSON.stringify({ sel: window.getSelection().toString().length,
   drag: window.__drag })`.
Expect: `sel` is greater than 0 (a text selection was created by the drag). Every value recorded
in `drag` is `1` (every `mousemove` observed while dragging reports `buttons=1`, matching a real
held left-button drag); no `0` values appear among the interpolated move samples.

## T10-1: Normal page scroll is verified effective, reports the plain success text
Changed: the `scroll` action in `extension/service-worker.js` now probes window/element scroll
position before and after dispatching the wheel event, and only claims success when it actually
verified movement (or verification was unavailable, same as before this task).
Steps:
1. `navigate` a grouped tab to https://en.wikipedia.org/wiki/Cat (a long article).
2. `computer` with `{ "action": "scroll", "scroll_direction": "down", "scroll_amount": 3,
   "coordinate": [400, 300] }` (a point roughly at the center of the visible page).
Expect: result text is exactly `Scrolled down by 3.` plus a screenshot showing the page content
visibly shifted upward compared to before the call.

## T10-2: Wheel-blocked container triggers the direct-scroll fallback
Changed: when the dispatched wheel event does not move the window or the nearest scrollable
ancestor (checked twice, 200ms apart), the engine now calls `directScrollFallback`, which runs a
direct `scrollBy({ behavior: "instant" })` on that ancestor (or `window`) and reports whether
that moved something.
Steps:
1. Save this file locally and open it in a grouped tab via `navigate` to its `file://` path:
   ```html
   <!DOCTYPE html>
   <html><body style="margin:0">
   <div id="box" style="height:300px;width:400px;overflow-y:scroll;border:1px solid black">
     <div style="height:3000px;background:linear-gradient(red,blue)">tall content</div>
   </div>
   <script>
     document.getElementById("box").addEventListener(
       "wheel", (e) => e.preventDefault(), { passive: false });
   </script>
   </body></html>
   ```
2. `computer` with `{ "action": "scroll", "scroll_direction": "down", "scroll_amount": 3,
   "coordinate": [x, y] }` where `(x, y)` is a point inside the `#box` element (for example near
   the top-left of the page, since the box is the first thing on the page at 0,0-400,300).
Expect: result text is exactly
`Scrolled down by 3 (mouse wheel had no effect; used direct scroll fallback).` and the screenshot
shows the red/blue gradient inside the box has visibly shifted down (the wheel itself was
swallowed by `preventDefault`, but the fallback `scrollBy` moved the box's own scroll position).

## T10-3: Nothing to scroll reports the truthful no-effect text
Changed: when neither the wheel nor the fallback move anything (a short page with nothing to
scroll), the engine no longer claims `Scrolled down by 3.`; it now reports the exact coordinates
and states nothing moved.
Steps:
1. `navigate` a grouped tab to https://example.com (a short page with no scrollable overflow at
   normal window size).
2. `computer` with `{ "action": "scroll", "scroll_direction": "down", "scroll_amount": 3,
   "coordinate": [400, 300] }`.
Expect: result text is exactly
`Scroll down had no effect at (400, 300); the page did not move at that position.` (coordinates
may be rescaled slightly from the resolved point; use the actual `(x, y)` reported), plus a
screenshot of the unchanged page. If the browser window is unusually tall/short and the page
does have overflow at your viewport size, resize the window smaller first so the page truly has
no scroll room, or use a shorter test page.

## T10-4: Regression -- click/type/scroll_to and the screenshot-action contract are unchanged
Changed: nothing in this task touches `left_click`, `type`, or `scroll_to`; this check confirms
the scroll rewrite did not leak into neighboring actions.
Steps:
1. `navigate` a grouped tab to https://example.com.
2. `computer` with `{ "action": "left_click", "coordinate": [100, 100] }`. Expect: a text-only
   result (`left_click at (x, y).`), no image content block.
3. `computer` with `{ "action": "type", "text": "hello" }` on a page with a focused input (or
   just confirm the result is text-only). Expect: text-only result, no image content block.
4. `computer` with `{ "action": "scroll_to", "coordinate": [0, 0] }`. Expect: text-only result
   `Scrolled to target.`, no image content block (scroll_to was explicitly out of scope for this
   task and must still never return a screenshot).
5. `computer` with `{ "action": "scroll", "scroll_direction": "down" }` on any page. Expect: the
   result DOES include an image content block (scroll remains one of the three
   screenshot-returning actions: screenshot, scroll, zoom).

## T11-1: zoom captures only the requested region, magnified
Changed: `zoom` no longer echoes the region and returns a full-viewport screenshot; it now clips
and magnifies the exact requested region via a fresh `Page.captureScreenshot` with `clip`, and
records the region as the tab's new coordinate context.
Steps:
1. `navigate` a grouped tab to https://en.wikipedia.org/wiki/Cat.
2. `computer` with `{ "action": "screenshot" }`. Note the pixel coordinates of a small element in
   the returned image (for example the site logo in the top-left, or an inline citation marker).
3. `computer` with `{ "action": "zoom", "region": [x0, y0, x1, y1] }` where `(x0,y0)-(x1,y1)` is a
   small box around that element, read off the screenshot from step 2.
Expect: the returned image shows ONLY that region, visibly magnified (not the full viewport), and
the result text is `Zoom region (X0, Y0) -> (X1, Y1) captured (jpeg).` with no "clamped" suffix
(assuming the region was fully inside the viewport), where X0/Y0/X1/Y1 are the actual captured
CSS-pixel integers (may differ slightly from the requested region due to rescale rounding).

## T11-2: zoom validation errors are exact text, no image
Changed: `zoom` now validates `region` before ever attempting a capture.
Steps:
1. On any grouped tab, `computer` with `{ "action": "zoom" }` (no `region` field).
   Expect: text-only result, exactly `region [x0, y0, x1, y1] is required for zoom.`, no image.
2. `computer` with `{ "action": "zoom", "region": [200, 200, 100, 300] }` (x1 < x0).
   Expect: text-only result, exactly
   `zoom region is empty: x1 must be greater than x0 and y1 must be greater than y0.`, no image.
3. `computer` with `{ "action": "zoom", "region": [9000, 9000, 9500, 9500] }` (far outside any
   normal page).
   Expect: text-only result, exactly
   `zoom region is empty or entirely outside the visible viewport.`, no image.

## T11-3: zoom region straddling the viewport edge is clamped and reported as such
Changed: a region that partially exceeds the viewport is clamped to `[0, vpW] x [0, vpH]` rather
than failing, and the result text says so.
Steps:
1. `navigate` a grouped tab to https://example.com (or any page smaller than the window).
2. `computer` with `{ "action": "screenshot" }` to establish a coordinate context, then note the
   approximate screenshot pixel dimensions of the viewport (near the max x/y visible).
3. `computer` with `{ "action": "zoom", "region": [x, y, x+400, y+400] }` where `x, y` are chosen
   so that `x+400` or `y+400` maps past the edge of the viewport (for example near the
   bottom-right corner of the screenshot).
Expect: an image is returned (not an error) and the result text ends with exactly
"(jpeg; clamped to the visible viewport)." and the echoed `(X0, Y0) -> (X1, Y1)` coordinates are
within `[0, viewport width]` x `[0, viewport height]`.

## T11-4: coordinates read off a zoomed image map back correctly (offset mapping)
Changed: `rescaleCoord` now adds back the zoomed region's offset, so a click coordinate read off
a zoomed screenshot lands on the correct element, not on the un-offset viewport position.
Steps:
1. `navigate` a grouped tab to https://en.wikipedia.org/wiki/Cat.
2. `computer` `{ "action": "screenshot" }`, then `computer` `{ "action": "zoom", "region": [...] }`
   around a specific link or button visible in that screenshot.
3. Read a coordinate off the ZOOMED image that sits on top of that link/button, then
   `computer` with `{ "action": "left_click", "coordinate": [that x, that y] }`.
Expect: the click lands on the element visible at that point in the zoomed image (for example the
page navigates if it was a link, or a visible focus/selection change happens), not on whatever was
at that raw pixel position in the un-zoomed viewport.

## T11-5: chained zoom (zooming again off a zoomed image) composes correctly
Changed: `zoomScreenshot` rescales the incoming region against the context as it stood BEFORE the
new zoom, so a second zoom issued against a first zoomed image is interpreted correctly.
Steps:
1. Continue from T11-4 (a zoomed screenshot is the tab's current context), or repeat: `screenshot`
   then `zoom` on a moderately large region (for example an entire paragraph).
2. Read a smaller box off THAT zoomed image (for example a single word within the paragraph) and
   call `computer` `{ "action": "zoom", "region": [...] }` with those coordinates.
Expect: the new image is the correct, further-magnified sub-region (the single word visibly
readable and centered), not a mis-mapped or unrelated part of the page.

## T11-6: a subsequent full screenshot resets the zoom offset
Changed: `screenshot()` now writes `offX: 0, offY: 0` (and the full viewport as the region) into
the tab's context, so coordinates after a full screenshot map normally again even after a zoom.
Steps:
1. Perform a `zoom` (as in T11-1), then `computer` `{ "action": "screenshot" }` (full viewport).
2. Read a coordinate off the FULL screenshot and `computer`
   `{ "action": "left_click", "coordinate": [that x, that y] }`.
Expect: the click lands at the normal (un-offset) position corresponding to the full screenshot,
confirming the zoom offset was cleared by the intervening full screenshot.

## T11-7: scrolling before a zoom is reflected correctly (scroll offset in the clip)
Changed: `zoomScreenshot` adds the page's current `scrollX`/`scrollY` to the clip coordinates
(CDP's `clip` is document-relative, not viewport-relative).
Steps:
1. `navigate` a grouped tab to https://en.wikipedia.org/wiki/Cat.
2. `computer` `{ "action": "scroll", "scroll_direction": "down", "scroll_amount": 10,
   "coordinate": [400, 300] }` to scroll well down the page.
3. `computer` `{ "action": "screenshot" }`, then `computer` `{ "action": "zoom", "region": [...] }`
   around an element visible in that (scrolled) screenshot.
Expect: the zoomed image matches what is visible on screen at the scrolled position (not the
top-of-page content at those same raw coordinates).

## T11-8: no console errors during zoom actions
Steps:
1. Open chrome://extensions, find the Browser MCP dev extension, click "Inspect views: service
   worker" to open its console.
2. Repeat T11-1 through T11-7.
Expect: no errors logged in the service worker console during any of the zoom actions above.
