# Driving Ghostlight from a script

Ghostlight does not need an MCP client to do browser work. `ghostlight call` invokes the same tools,
through the same governance, and writes the same audit record. It exists for the jobs that have no
model in them: a deploy check, a smoke test, a scheduled report.

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

`--json` prints the result as the model sees it: a field at its default is left out
(`history_storage` `saved`, `readiness` `not_applicable`, `next_steps` empty). Read an absent field
as its default. An absent `facts.coverage` means nothing limited the result; when a policy exclusion,
an unavailable document or a size ceiling did, the full block is present, and a `browser_flow` row
carries it even when the step's result was cut for size. An item's `state` (empty) and
`credential_class` (false) are left out at their default, as are a document-tree node's `label`
(empty) and `children` (empty); an item's `value` of `""` stays, since an empty field is a fact:

```sh
ghostlight call browser_navigate '{"url":"https://example.com"}' --json \
  | jq -r '.history_storage // "saved"'
```

`--output <file>` writes bounded content, which today means a screenshot's image bytes. Without it
a capture reports that its content was omitted, since a megabyte of base64 in a terminal helps
nobody. In a batch, later captures gain an index rather than overwriting the first.

`--catalog` lists the tools this build offers. Their full contract is in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md). `ghostlight call --help` (or `-h`) prints the usage, the
exit codes and the session variable on one screen, and reaches no service.

## Several calls, one session

**The session is your terminal.** Every `ghostlight call` you type in one shell reaches the same
tabs, so you can work a step at a time:

```sh
ghostlight call browser_navigate '{"url":"https://example.com"}'   # returns tab_a1b2
ghostlight call browser_read '{"tab":"tab_a1b2"}'
ghostlight call browser_tabs '{"action":"close","tab":"tab_a1b2"}'
```

Ghostlight keys the session on the process that called it, identified by its process id and start
time. A command can be forwarded through other Ghostlight executables on its way to the service;
those hops are skipped, and the owner is the first process above them that is not Ghostlight. A run
of consecutive ancestors with the same program name counts as one terminal, and its outermost member
is the owner. That is what makes Git Bash work: MSYS starts every command through a short-lived stub
of `bash.exe`, so the stub folds into the shell above it. A shell nested in the same shell is one
terminal too. A program that spawns `ghostlight call` repeatedly usually gets one session for as long as it
runs, but a program that shares its parent's program name (a bash script run from bash, Python
multiprocessing workers, a venv `python` launcher pair) joins the outer process's session, and
parallel workers with the same name share tabs. Such callers should set `GHOSTLIGHT_SESSION`.

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

The same applies to `npx`: each `npx ghostlight` call starts a fresh Node process that runs
Ghostlight as its child, so consecutive calls get different sessions. For continuity across them, set
`GHOSTLIGHT_SESSION`, or call the installed `ghostlight` command directly.

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
user-authorized credential input, and the same tab-close interlock.
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
the MCP relay by hand (`ghostlight mcp`). The channel is recorded so that you can *see* what happened; it is not a
security boundary, and Ghostlight does not pretend it is.

## Worked examples

[`scripts/browser-journey.ps1`](../../scripts/browser-journey.ps1) and
[`scripts/browser-journey.sh`](../../scripts/browser-journey.sh) are the same complete journey for
PowerShell and POSIX shell: open a page, list tabs, read it, capture it to a file, and close the
tab. Every step is a plain `ghostlight call` in its own process, and they share a session because
they share a shell, which is how the last step closes exactly the tab the first one opened. The
shell scripts require `jq` for typo-safe JSON construction and result decoding.

It exits with Ghostlight's own code rather than inventing one, so a refusal stays distinguishable
from a breakage. On a default install the close step is refused by the browser's preserve-tabs
setting and the script exits 2, which is governance working rather than the journey failing.

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
directly instead of shelling out: the per-user relay endpoint, an `mcp` opening on the `cli`
channel, then MCP lines, as specified in [relay protocol 4](../design/relay-protocol-4.md). That is
the same contract `ghostlight call` uses, and it is the caller an organization can hold to a signature (see
[ADR-0105](../adr/0105-scripted-intake-channels.md)).
