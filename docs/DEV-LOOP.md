# The Ghostlight dev loop

Ghostlight runs ONE stack (ADR-0065/0096): one native host
(`org.sylin.ghostlight`, allowing both the Web Store extension and the unpacked dev extension), one
installed service identity, one typed MCP-edge endpoint, one browser endpoint, and one
`ghostlight` MCP entry in your editor. The "engine" is whichever persistent `ghostlight` service
currently owns the endpoints -- the installed release, or the build you made thirty seconds ago.

The two shores reconnect independently. `ghostlight-mcp-connector` keeps MCP JSON-RPC and exact revision
state at the client shore, reconnects its typed bridge for future calls, and never replays an
in-flight browser effect. `ghostlight-browser-connector` reconnects the browser shore and replays the
extension's identity frame (ADR-0062). `deploy.lock` (ADR-0063) keeps either shore's self-heal from
respawning the old engine mid-swap.

There is no separate dev install, no `-dev` host, no second MCP entry, and no separate dev browser.
A source developer performs [Path B](guides/installation.md#path-b-build-from-source)'s ordinary
one-stack registration once, so the one client entry and native-host manifest point at the
repo-built shores. Your real, authenticated browser and real editors then ride the service under
test. The symmetric cost: while a broken build owns the service endpoints, real use is broken until
you swap back (`-Restore`) or land a fix.

## 1. When code changes: what to do

The service, MCP edge, browser relay, and JavaScript extension live in different processes and
refresh by different mechanisms. Pick the row that matches what you edited.

| You changed | Do this | What must respawn |
| --- | --- | --- |
| Rust: service or core code (usual) | `.\scripts\dev-loop.ps1` | Nothing; both shores reconnect |
| Rust: `crates/mcp-connector/` | `.\scripts\dev-loop.ps1`, then reopen the client's MCP connection | `ghostlight-mcp-connector` |
| Rust: `crates/browser-connector/` | `.\scripts\dev-loop.ps1`, then Reload at `chrome://extensions` | `ghostlight-browser-connector` |
| Extension JS or CSS | Reload at `chrome://extensions` (no Rust rebuild) | Extension worker and browser relay |
| Service/core plus extension | `.\scripts\dev-loop.ps1`, then Reload the extension | Extension worker and browser relay |
| Revert to the installed release | `.\scripts\dev-loop.ps1 -Restore` | Nothing; both shores reconnect |

**Rust: service or core code** -- the everyday case. `dev-loop.ps1` swaps which service owns the
endpoints (see the mechanics below). The editor's `ghostlight-mcp-connector` process and the
browser's `ghostlight-browser-connector` stay alive and reconnect on their own. You do not restart
the editor and do not
touch the browser. A pending call ends truthfully if the bridge breaks; it is never replayed. The
next call uses the new service code.

**Rust: MCP edge code** -- run the swap so `ghostlight-mcp-connector` is rebuilt, then reopen the client's
MCP connection (or restart the client) so it launches that fresh executable. A running edge can
reconnect to a new service, but it cannot replace its own loaded image. This is also the boundary
where changes to `mcp_2025_11_25` or `mcp_2026_07_28` take effect.

If the client reports `Transport closed`, stop using the cached Ghostlight tools and reopen that
client's Ghostlight connection. Do not launch `ghostlight-mcp-connector` by hand as a substitute:
the standalone process does not repair the client's stdio and may create a different workspace.
Inspect browser state before retrying an effectful call because the earlier call may have started.

**Extension JS or CSS** -- no rebuild, no `dev-loop.ps1`. Click Reload at `chrome://extensions`.
The reload tears down the service worker and its native port; the extension reconnects, re-reads
its stored `browserId`, and the engine re-attaches it to the same slot. Chrome caches aggressively
(plausibly V8 bytecode keyed by the pinned extension id), so the explicit Reload is mandatory -- a
stale worker has survived even a fresh profile. Never trust a "still broken" observation until
after a Reload (section 3).

**Both** -- swap the engine first, wait for `ghostlight doctor` to report healthy, then Reload the
extension. Ordering it engine-first means the extension reconnects to a live endpoint instead of a
down one (it would buffer and retry either way, but this avoids a needless reconnect churn).

**Rust: browser-connector code** -- `ghostlight-browser-connector` is browser-only and changes rarely.
`dev-loop.ps1` rebuilds it, but an already-loaded relay keeps the old image until Chromium respawns
it. Reload the extension: the old native port closes and Chromium launches the fresh relay from the
native-host manifest. No editor restart is involved.

### What `dev-loop.ps1` does

In order: writes `deploy.lock` into every candidate engine directory (the repo `target\` dir and
each versioned dir under `~\.ghostlight\bin`) so no shore self-heals the old image mid-swap; stops
SERVICE processes only (identified by executable path, never a bare taskkill); renames running
`ghostlight-mcp-connector` and `ghostlight-browser-connector` images aside so Windows can write the
new files; builds `ghostlight-mcp-connector` + `ghostlight` + `ghostlight-browser-connector` +
`lightbox`; starts the fresh service
(`--debug service --keep-warm`); waits for `ghostlight doctor` to report healthy; removes the
locks; and runs one offline `fake-browser` smoke check. Existing shore processes stay alive until
their own client or Chromium respawns them.

`--keep-warm` disables the idle-grace shutdown so the engine stays up between actions. Add
`-Manifest examples\dev-live-test.json` when you want the engine started under a restrictive test
policy (default is none: the engine serves real use with the real config).

When no dev build is running and either shore finds its service endpoint down, self-heal launches
the sibling engine from that shore's directory. `-Restore` does it deterministically: it stops the
repo-built engine and starts the newest installed release that has all three ADR-0096 executables
and meets the established one-stack reconnect floor. If no installed release qualifies,
`-Restore` refuses and leaves the repo build serving.

### Restore eligibility and older installations

`-Restore` requires all three product executables. It refuses a two-executable installation even
when that release already understands the one-stack swap, because its directory has no MCP edge
for the current client-launch contract. Separately, a release older than v0.5.5 predates the
browser-relay reconnect (ADR-0062) and `deploy.lock` quiesce (ADR-0063), which can produce two
concrete failures, both observed live:

- **The swap does not hold.** The old release's relays cannot see `deploy.lock`, so during the
  brief endpoint-down window of a swap they self-heal the OLD engine back, and it wins the pipe
  race; your fresh build exits. The swap appears to work (doctor shows the new version for a few
  seconds) and then silently reverts.
- **The browser cannot attach.** The old engine cannot parse the current extension's identity
  frame (ADR-0061 `browser_hello`), so doctor reports `extension not connected` even in steady
  state.

The fix is a one-time upgrade of the machine: run `ghostlight install` from a three-executable
build (it repoints the host manifest, client entries, and the auto-start supervisor), then stop any
still-running processes of the old release. Identify them by executable path under
`~\.ghostlight\bin\<old-version>`, never by bare name. After that, `dev-loop.ps1` swaps hold cleanly
and both shores reconnect on their own. Deleting obsolete release directories removes the last way
those images can come back.

## 2. Who is serving right now?

```
ghostlight doctor
```

Doctor names the service endpoint state, the attached browsers, and the live workspaces. There is
one installed service identity, so the question is "which service owns it?", and doctor answers
that. Every attach/detach/focus/reject decision (both sides: the service's own and, when the
extension's "Developer diagnostics" option is on, the extension's `connect_attempt`/
`connect_disconnect` notes) lands in the structured event ring `debug-state-<pid>.json` carries --
look there before reasoning about timing from raw process logs.

## 3. Extensions

Both extension builds talk to the same host, and the host manifest allows both ids:

- The **unpacked dev extension** (chrome://extensions, Load unpacked, `extension/`; its id is
  pinned by the committed manifest `key`, ADR-0016). Load it in whatever browser you actually use.
- The **Web Store extension**, once released.

Do not run both builds in the SAME browser profile -- they would each open a native port and appear
as two browsers (harmless to the service, ADR-0061 gives each a slot, but confusing to you). One
browser, one build. After editing extension JS, reload the extension from chrome://extensions --
Chrome caches aggressively (plausibly V8 bytecode keyed by the pinned extension id), and a stale
worker has survived even a fresh profile. Never trust a "still broken" observation until after an
explicit reload.

Version skew is a normal condition here: right after an engine swap, the loaded extension is one
build older than the engine until you reload it (and a released extension may be older still).
Wire-protocol changes must stay additive and tolerant -- unknown fields ignored, absent fields
defaulted (ADR-0065 Decision 6).

## 4. Offline iteration (no browser at all)

For wire-protocol changes (routing, tabId encoding, focus, notifications) you do not need a real
browser:

```
.\target\release\lightbox.exe fake-browser --auto-reply
```

`fake-browser` dials the engine exactly as the real relay does, prints every frame it receives,
and (with `--auto-reply`) answers `tabs_context_mcp`/`tabs_create_mcp` with a DELIBERATELY
billion-scale tab id -- the same magnitude a real browser produces -- so a tabId-encoding
regression is caught on the first offline round trip. Commands at its prompt: `focus`, `kill`,
`reply <id> <json-result>`, `quit`.

Tests and the e2e harness never touch the real endpoint: they run ephemeral NAMED instances
(`--instance <name>` / `GHOSTLIGHT_INSTANCE`, ADR-0044) as a pure isolation seam. That is the only
remaining use of named instances -- no user- or dev-facing workflow installs or pins one
(ADR-0065 Decision 5).

## 5. Live-testing a browser-visible feature end-to-end

For anything you actually need to SEE (FX, notifications, layout) rather than wire-protocol
correctness, `fake-browser` is not enough -- it never renders a page.

### 5.1 Check who is attached first

```
ghostlight doctor
```

Look for `extension connected (live)` and a `Browsers:` line naming your browser. Because your
tool calls land in the user's real browser, know what is attached before driving it.

### 5.2 Drive the browser with your own tool calls

```
tabs_context_mcp(createIfEmpty: true)   # note the huge composite tabId -- (slot << 32) | native_tab_id, expected
navigate(tabId, url)
computer(action: "screenshot", tabId)
```

Three gotchas:

- **`chrome://newtab/` and other `chrome://` pages cannot host a content script.** Anything that
  renders via `agent-visual-indicator.js` or `content.js` (FX, denial notifications) needs a real
  `http(s)` page loaded in the tab first. Navigate somewhere real (with `-Manifest
  examples\dev-live-test.json`, the committed fixture grants `example.org`) before triggering the
  thing you want to see.
- **A screenshot NEVER shows FX, a denial sticker, or an attention overlay in the captured pixels,
  by design** --
  every effect (cursor, ripples, the notification layer) is hidden for the duration of the
  capture so the agent's own screenshot stays clean, then restored after. Do not read a clean
  screenshot as "it didn't render" or "it got dismissed" -- it means neither on its own. An
  isolated denial sticker replaces an older sticker and expires after three seconds. A denial
  burst can open a blocking attention overlay that stays until the user chooses a disposition.
  To see whether something is still there, ask the user to look at their own screen (the fastest
  path in practice), or capture out-of-band over the browser's own devtools websocket
  (`Page.captureScreenshot` via `--remote-debugging-port`, launched fresh and separately from the
  attach you are trying to observe).
- **After editing extension JS, reload the extension explicitly** (section 3) before trusting any
  observation.

### 5.3 The `notify` tool: iterating on notifications without a denial

`notify` is an UNLISTED tool: a direct entry point onto `Browser::notify()` -- the same primitive
governance denials call to draw the on-screen sticker. It takes `tabId`, `class`
(`error`/`warn`/`info`/`debug`), optional `icon` (`lock` or anything else -> shield), `title`, and
optional `description`, and renders the sticker immediately, bypassing governance (it IS the channel
governance speaks through). It is deliberately absent from `tools/list` and NOT registered in
`browser/directory.rs` -- the sticker is a governance-authority signal, not something the trained
model should emit -- so it exists only as the first branch of `run_tool_call` in
`crates/core/src/tool/pipeline.rs`. Look there, not in the directory, when auditing what tools
exist.

For notification-design work this is the fast path: swap the engine ONCE (to pick up the tool),
reload the extension ONCE (to pick up any renderer CSS), then fire every severity/icon combination
as plain `notify` calls -- no rebuild per variant.

Two caveats when driving it:
- Because it is unlisted, an MCP client's own tool list will not contain it. Send a raw JSON-RPC
  `tools/call` (name `notify`) through `ghostlight-mcp-connector` rather than through a client's
  advertised-tool surface.
- The neutral pipeline's workspace ownership guard runs before dispatch and refuses a `tools/call`
  naming a `tabId` another live workspace owns (returns "unknown tab"). The notify call must come
  from a workspace that owns the tab: have that same MCP path create its own tab
  (`tabs_create_mcp`) and navigate it before calling `notify`. The internal denial path is
  unaffected -- it calls `Browser::notify()` directly, never through an incoming `tools/call`.

## 6. Clean up

Kill only processes whose executable path is under this repo's own `target\` directory or under
`~\.ghostlight\bin` -- the same rule `dev-loop.ps1` itself follows. Never a bare
`taskkill /IM ghostlight.exe` or `/IM chrome.exe`. Prefer `.\scripts\dev-loop.ps1 -Restore` over
manual killing: it hands the endpoint back to the installed release cleanly.
