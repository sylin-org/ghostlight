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

`--catalog` lists the tools this build offers. Their full contract is in
[`../1.0/LANGUAGE.md`](../1.0/LANGUAGE.md).

## Several calls, one session

Tab, target, and view handles belong to a session, and a session lasts as long as the process. Two
separate `ghostlight call` commands are two sessions, so a handle from the first will not resolve in
the second.

For multi-step work, use `--stdin` and give one `<tool> <json>` per line:

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
