# Driving Ghostlight from a script

Ghostlight does not need an MCP client to do browser work. `ghostlight call` invokes the same tools,
through the same governance, and writes the same audit record. It exists for the jobs that have no
model in them: a deploy check, a smoke test, or a report run by your own scheduler. Ghostlight
adds no scheduler or workflow engine.

## One call

```sh
ghostlight call browser_tabs '{"action":"list"}'
ghostlight call browser_navigate '{"url":"https://example.com"}'
```

The command prints Ghostlight's sentence for what happened, and exits with the terminal status:

| Exit | Meaning |
| --- | --- |
| 0 | Succeeded |
| 1 | The call could not be made: bad arguments, bad JSON, or no reachable service |
| 2 | Blocked by authority |
| 3 | Attention required in the visible browser |
| 4 | Failed |
| 5 | Cancelled |
| 6 | The effect cannot be determined |

**Six is not a failure and not a success.** An uncertain effect may or may not have happened, so a
script must not retry it blindly. That is why it has its own code rather than sharing with 4.

`--json` prints the whole terminal result instead of the sentence, for a script that wants the
facts:

```sh
ghostlight call browser_read '{"tab":"tab_a1b2"}' --json | jq -r .facts.text
```

`--output <file>` writes bounded content, which today means a screenshot's image bytes. Without it
a capture reports that its content was omitted, since a megabyte of base64 in a terminal helps
nobody. In a batch, later captures gain an index rather than overwriting the first.

`--catalog` lists the tools this build offers. Their full contract is in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

## Background work and repeat runs

The operator's `browser.attention` preference applies equally to CLI calls, MCP, and flows.
Background is the default: direct work leaves the person's active tab alone, never adopts an unowned
same-host tab, and does not merge or move existing groups. A new work window may appear unfocused.
Opening prefers an unfocused window containing only owned tabs. Native keyboard or pointer
input can select the controlled tab in that window without foregrounding it. Any key/mouse
surface in a focused or shared window refuses with `attention_refusal:native_input` before input. Work in
one physical window is coordinated and checks target, placement, and focus before input packets.
Human focus or tab movement can interrupt later input; after an effect may have landed, inspect
partial or unknown receipts rather than retrying or reselecting. Show tab is for human review.
Return focus elsewhere or use new background work and reobserve before continuing native input.
When no browser is connected, Ghostlight asks the person to open one rather than risking a
foreground launch. An older adapter without background enforcement refuses before dispatch.
The Workbench shows the effective preference and offers `Show tab` for an exact owned tab.
Reveal grants no permission and does not Resume, retry, or roll back work.
Each result's `facts.browser_attention` contains `value`, `decided_by`, and
`organization_ceiling`, matching the effective policy view. A blocked focus request has no
effect but is not repeat-safe advice; use explicit operator reveal instead.
Background form fill uses exact control selection, one native whole-value replacement, and
validated blur commit with batch retention checks. It does not claim every key was emitted.
Whole-value edits do not select the tab and may edit an inactive owned target in a shared or
focused window. They refuse an active target in the focused window and retain the same event
fences and input checks. Cleanup can release held input after takeover, without replaying edits.

Keep that preference under operator control. For a recipe that must create a fresh tab even if
the operator later chooses foreground, state the navigation intent explicitly:

```sh
ghostlight call browser_navigate '{"url":"https://example.com","new_tab":true,"reuse":"never"}' --json
```

Keep the returned tab handle for later calls in the same session. A new run in a released session
can leave a new preserved tab; the preserve-tabs interlock remains independent. Foreground
permits the earlier unbound same-host reuse when `reuse` remains `domain`.
`new_tab:true` makes the fresh-tab request. `reuse:"never"` alone prevents unowned-tab adoption
when an open is needed; it still navigates an existing unambiguous controlled tab in the session.

## Several calls, one session

**The session is your terminal.** Every `ghostlight call` you type in one shell reaches the same
tabs, so you can work a step at a time:

```sh
ghostlight call browser_navigate '{"url":"https://example.com"}'   # returns tab_a1b2
ghostlight call browser_read '{"tab":"tab_a1b2"}'
ghostlight call browser_tabs '{"action":"close","tab":"tab_a1b2"}'
```

Ghostlight keys the session on the process that called it, identified by its process id and start
time. A program that spawns `ghostlight call` repeatedly gets the same treatment: one session for as
long as that program runs.

Tabs live as long as their session, and a tab you open in one shell is not visible from another.
When the terminal exits, Ghostlight releases its tabs and asks the browser to close them -- but the
extension's **Preserve controlled tabs** setting is on by default and refuses that, so in practice
the tabs stop being controlled and stay visible for you to deal with. Turn that setting off if you
want a terminal to clean up after itself.

If your program shells out *through* a shell, the parent is a throwaway `cmd.exe` that differs on
every call, and you would get a new session each time. Set `GHOSTLIGHT_SESSION` to any string once,
and every descendant lands in the same session no matter how many shells deep:

```powershell
$env:GHOSTLIGHT_SESSION = "acme-deploy-$PID"
```

That key is a convenience, not a credential: it identifies a session, never a permission.

For a fixed list of calls, `--stdin` still takes one `<tool> <json>` per line:

```sh
printf '%s\n' \
  'browser_navigate {"url":"https://example.com"}' \
  'browser_tabs {"action":"list"}' \
  | ghostlight call --stdin --json
```

Because it reads a line at a time, a caller can read a handle out of one result and write the next
line using it. `browser_flow` batches ordinary tool calls in one invocation. Step IDs are optional
for a fixed batch; name steps when later arguments use `flow_ref` to read their results. A flow
executes its steps and stops on the first failure by default. Policy remains configured by the
person or organization; calls do not accept host or capability restriction fields.

`--stdin` keeps processing later lines after a nonzero result and returns the last nonzero exit
code. It is not fail-fast. Use separate calls and check each exit code before dependent work, or
use a flow's default `on_error:stop`. Explicit `on_error:continue` can run later steps after an
unknown effect or connection loss; it does not prove those steps are independent or safe.

Read `status`, `effect`, `repeat_safe`, and the per-step receipts before repeating a script.
After an unknown or partially applied write, reobserve the actual page state and determine
what remains. Do not replay the whole batch or assume a lost reply means no effect. Human Pause
blocks future dispatch and Resume allows new requests without replay. Stop remains terminal.

Two habits keep scripts out of rework:

- Prefer typed semantic selectors over stashed target handles. A selector (`name`, optional
  `role`, optional `exact`) is accepted by click, type_text, per-field fill, and
  `selector_present` waits, and it survives navigations because it names what a control is
  called rather than where it was in an older document.
- Tab handles are durable. Navigating by a handle whose tab has closed recreates that tab
  under the same handle (and says so in its summary), so a script can keep addressing
  "its" tab across closes without re-listing.

## What governance sees

A scripted call is governed exactly like an agent's call. The same capability classes apply, the
same host rules, the same human runtime controls, the same per-request acknowledgement of explicit
user-authorized credential input, the same browser-attention preference, and the same tab-close
interlock. Background protects direct Ghostlight mechanisms; explicit scripts or page actions
can still cause browser-originated popups or focus. It is not OS containment.
There is no scripting bypass, because the command line is an edge and the orchestrator is the only
thing that executes.

Every record says which intake the work arrived on:

```json
{ "tool": "browser_navigate", "channel": "cli", "capabilities": ["read"], "allowed": true }
```

The workbench shows it too, and scripted tabs group under their own name in the browser, so a script
and an agent working at the same time stay visually distinct.

One caveat worth stating plainly: on a machine where a person can already run programs, the command
line grants nothing they did not already have. Anyone who can run `ghostlight call` could also start
the MCP connector by hand. The channel is recorded so that you can *see* what happened; it is not a
security boundary, and Ghostlight does not pretend it is.

## Turning it off

An organization that wants agent work but not scripted work closes the channel in policy:

```json
{
  "schema": 3,
  "name": "MCP only",
  "version": "1",
  "grants": [],
  "config": [
    {"key": "channels.cli.enabled", "value": false, "level": "mandatory"}
  ]
}
```

`ghostlight call` then exits non-zero with `channel_denied` before any session opens, and nothing is
invoked or audited. The full rules, including how layers compose, are in
[governance-configuration.md](governance-configuration.md#turning-an-intake-channel-off).

## Worked examples

[`scripts/browser-journey.ps1`](../../scripts/browser-journey.ps1) and
[`scripts/browser-journey.sh`](../../scripts/browser-journey.sh) are the same complete journey for
PowerShell and POSIX shell: open a page, list tabs, read it, capture it to a file, and close the
tab. Every step is a plain `ghostlight call` in its own process, and they share a session because
they share a shell, which is how the last step closes exactly the tab the first one opened. The
shell scripts require `jq` for typo-safe JSON construction and result decoding.

Both recipes request `new_tab:true` with `reuse:"never"` and stop at the first nonzero exit or
non-success receipt.
Later reads, capture, and close do not run after an earlier refusal or uncertain effect. They
retain Ghostlight's actual exit code, including 6 for uncertainty, instead of replacing it with
a later result. On a default install the final close is refused by the browser's preserve-tabs
setting and the script exits 2. Enable close only when you intend that cleanup.

The optional `demo-brief` scripts are a longer form journey with adjustable pacing:

```powershell
./scripts/demo-brief.ps1 -Beat 0.4 -CompletionHold 5
```

```sh
./scripts/demo-brief.sh --beat 0.4 --completion-hold 5
```

The optional Card Foundry scripts exercise the same public CLI:

```sh
./scripts/demo-foundry.sh --beat 0.6
```

Ten steps, three capability classes, one session, and no typed value anywhere in the audit.

```
STEP         STATUS     WHAT HAPPENED
----         ------     -------------
open         succeeded  Opened example.com.
list         succeeded  Listed 1 controlled tab.
read         succeeded  Read 9 words from example.com.
screenshot   succeeded  Captured the viewport at 1280x720.
close        succeeded  Closed the controlled tab.
```

## If nothing is running

`ghostlight call` starts the local service the same way a client does, so a script does not need to
launch anything first. A fresh deployment lock suppresses that, so a call during an upgrade fails
cleanly rather than racing the installer.

## Writing an integration rather than a script

If you are building a program rather than a shell script, it can speak the local service bridge
directly instead of shelling out: authenticated loopback TCP, line-delimited JSON,
`Hello` / `Catalog` / `Invoke` / `Cancel`, discovered through the runtime file. That is the same
contract `ghostlight call` uses, and it is the caller an organization can hold to a signature (see
[ADR-0105](../adr/0105-scripted-intake-channels.md)).
