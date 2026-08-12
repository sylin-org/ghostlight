# Driving Ghostlight from a script

Ghostlight does not need an MCP client to do browser work. `ghostlight call` invokes the same tools,
through the same governance, and writes the same audit record. It exists for the jobs that have no
model in them: a deploy check, a smoke test, a scheduled report.

## One call

```sh
ghostlight call browser_list_tabs
ghostlight call browser_open_page '{"url":"https://example.com"}'
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
ghostlight call browser_read_page '{"tab":"tab_a1b2"}' --json | jq -r .facts.text
```

`--output <file>` writes bounded content, which today means a screenshot's image bytes. Without it
a capture reports that its content was omitted, since a megabyte of base64 in a terminal helps
nobody. In a batch, later captures gain an index rather than overwriting the first.

`--catalog` lists the tools this build offers. Their full contract is in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

## Several calls, one session

**The session is your terminal.** Every `ghostlight call` you type in one shell reaches the same
tabs, so you can work a step at a time:

```sh
ghostlight call browser_open_page '{"url":"https://example.com"}'   # returns tab_a1b2
ghostlight call browser_read_page '{"tab":"tab_a1b2"}'
ghostlight call browser_close_tab '{"tab":"tab_a1b2"}'
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

```sh
$env:GHOSTLIGHT_SESSION = "acme-deploy-$PID"
```

That key is a convenience, not a credential: it identifies a session, never a permission.

For a fixed list of calls, `--stdin` still takes one `<tool> <json>` per line:

```sh
printf '%s\n' \
  'browser_open_page {"url":"https://example.com"}' \
  'browser_list_tabs' \
  | ghostlight call --stdin --json
```

Because it reads a line at a time, a caller can read a handle out of one result and write the next
line using it. For a fixed sequence with no handle passing, `browser_run_sequence` does the whole
thing in one call.

## What governance sees

A scripted call is governed exactly like an agent's call. The same capability classes apply, the
same host rules, the same runtime pause, the same credential handoff, the same tab-close interlock.
There is no scripting bypass, because the command line is an edge and the orchestrator is the only
thing that executes.

Every record says which intake the work arrived on:

```json
{ "tool": "browser_open_page", "channel": "cli", "capability": "action", "allowed": true }
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
{ "version": 1, "channels": { "cli": {} } }
```

`ghostlight call` then exits non-zero with `channel_denied` before any session opens, and nothing is
invoked or audited. The full rules, including how layers compose, are in
[governance-configuration.md](governance-configuration.md#turning-an-intake-channel-off).

## A worked example

[`scripts/browser-journey.ps1`](../../scripts/browser-journey.ps1) is a complete PowerShell journey:
open a page, list tabs, read it, capture it to a file, close the tab, and exit non-zero if any step
did not succeed. It is worth reading for one detail in particular -- it keeps a single
`ghostlight call --stdin` process open and writes a line at a time, so each step can use the handle
the previous step returned. That is how it closes exactly the tab it opened and nothing else.

```
STEP         STATUS     WHAT HAPPENED
----         ------     -------------
open         succeeded  Opened example.com.
list         succeeded  Listed 1 controlled tab.
read         succeeded  Read 9 words.
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
