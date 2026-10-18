# Ghostlight relay protocol 4

The frozen contract between the service and every program that connects to it: the edge in each
role, the one-shot command process and, later, the desktop shell. The decisions are
[ADR-0199](../adr/0199-one-service-thin-shells.md) Decision 5 (this protocol), Decision 8
(versioning) and Decision 10 (start reasons). The vocabulary is `ghostlight_relay::protocol`, the
transport is `ghostlight_relay::endpoint`, and the executable form of this document is
[`tests/golden/relay4`](../../tests/golden/relay4), bound by `crates/relay/tests/golden4.rs`.
When this text and the goldens disagree, the goldens are right.

The wire between the extension and the service is a separate protocol,
[adapter-protocol-4](adapter-protocol-4.md). A `browser` stream carries it untouched.

## 1. Endpoint

There is no port, token, runtime file or discovery file. The endpoint derives from the user and the
home only, never from a session variable that an MCP client may filter.

| Platform | Endpoint |
|---|---|
| Windows | The named pipe `\\.\pipe\ghostlight-<user SID>-<home hash>` |
| Linux and macOS | The Unix socket `<home>/run/relay.sock` |

The home is `~/.ghostlight`, or the directory named by `GHOSTLIGHT_TEST_HOME` in tests.

The home hash is the FNV-1a 32-bit hash (offset basis 2166136261, prime 16777619) of the home path
with ASCII letters lowercased, written as eight lowercase hexadecimal digits. It keeps a test home
apart from the real one. A worked name, from `vocabulary.json`:

```
user SID    S-1-5-21-1004336348-1177238915-682003330-1001
home        C:\Users\Ada\.ghostlight
hash input  c:\users\ada\.ghostlight   (FNV-1a 32 = 2981217699)
home hash   b1b1c5a3
pipe        \\.\pipe\ghostlight-S-1-5-21-1004336348-1177238915-682003330-1001-b1b1c5a3
```

The Windows pipe rejects remote clients, is created with the first-instance flag and carries a
DACL for the user only. The Linux directory `<home>/run` is mode 0700 and the socket is mode 0600.
Starting a service takes `run/service.lock`; the lock, the pipe flag and a probe of the socket
file make a second service impossible.

## 2. Peer checks, both ways

Each side checks the other before any payload byte moves.

- The service admits a peer that runs as the same user. On Windows it must also run at medium
  integrity or higher, which keeps sandboxed and AppContainer processes out. On Linux the check is
  the peer's user id from the socket. A peer that fails is disconnected without a reply.
- The client requires a server that runs as the same user. On Windows it reads the pipe server's
  token; on Linux it requires `run` to belong to the user with no group or other permission. An
  account that took the name first receives nothing.

## 3. Framing

A frame is a `u32` little-endian byte count followed by that many bytes of UTF-8. The largest frame
in either direction is 8 MiB. A declared length above that is a framing error. Every frame except
the adapter protocol on a `browser` stream and the browser goodbye (section 6) is one JSON object.

## 4. Opening and reply

The peer sends one opening. The service answers with one reply. Both are single frames.

```
{"relay": 4, "role": "mcp", "edge": {"program": "edge", "revision": 4, "build": "2.0.0"},
 "reason": "agent", "channel": "mcp", "resume": "..."}
```

| Field | Meaning |
|---|---|
| `relay` | The relay revision the peer speaks: 4. |
| `role` | `mcp`, `browser`, `control`, `desktop` or `command`. |
| `edge` | The connecting program: `program` is `edge`, `service` (a process of the service image, such as a one-shot command) or `desktop`; `revision` is that program's own revision, which moves only when its protocol does; `build` is its version string. |
| `reason` | Why the peer started the service, when it had to (section 7). |
| `resume` | The resume token the service last handed this session. |
| `origin` | For `browser`, the origin of the extension that opened the port. |
| `channel` | For `mcp`, how the client reached the edge: `mcp` for the stdio edge, `cli` for the command line. |
| `session` | For `mcp`, an opaque marker the peer uses to identify its session, such as its process. |
| `control` | For `control`, the operation (section 5). |

The reply is one object. An accepted reply is `{"accepted": true, "service", "instance",
"templates"}`: the service's version, a value that changes on every service start (so a client can
tell a restart from a blip), and the failure templates (section 8). A refused reply is
`{"accepted": false, "code", "say"}`. The client prints `say` as it is. The codes are
`unsupported` (the relay revision, role or operation is not one this service speaks), `capacity`,
`shutting_down` and `malformed`. After a refusal the service closes the connection.

Liveness is an accepted opening. There is no separate ping.

A 2.0 service refuses the `command` role as `unsupported`, and serves `desktop` only to a shell of
its own build (ADR-0199 amendment of 2026-10-15). The roles exist so a later shell or command
stream needs no new relay revision.

## 5. The frames of each role

**`mcp`.** After an accepted opening both sides exchange frames tagged by `kind`:

| Kind | Direction | Fields |
|---|---|---|
| `message` | both | `line`: one MCP line, exactly as the client or the service wrote it, without its newline. A JSON-RPC batch is one line. |
| `resume` | service to edge | `token`: the opaque token that resumes this session after a service restart. |
| `goodbye` | service to edge | `reason`: a goodbye reason (section 6). |

The edge parses no MCP. It forwards lines, answers outstanding requests from the failure templates
when the service goes away, and holds new lines for a bounded time while it starts the service. A
line whose write failed is held, since it never reached the service. A lone `initialize`,
`tools/list`, `prompts/list`, `resources/list` or `resources/templates/list` cut off by a lost
service or a `replace` goodbye changes nothing, so it is held and sent to the next service once
instead of being answered (ADR-0197 amendment of 2026-10-17).

**`browser`.** The stream carries the adapter protocol untouched in both directions. The edge
holds the extension's first frame, opens the stream, sends that frame and then pumps bytes. When
no service answers, it starts one once, as a `browser` start, and keeps connecting. The only frame
that is not the adapter's is the goodbye (section 6). While the service is away the edge
writes the adapter's `backend_unavailable` frame, with `service` set to `absent`, to the extension
and never starts the service while it waits. It writes the same frame when no service has accepted
within five seconds of its own start, which is below the extension's hello timeout.

**`control`.** The opening carries `control`, an object tagged by `op`:

| `op` | Fields | Does |
|---|---|---|
| `readiness` | none | Reports whether the service is ready, without changing anything. |
| `status` | none | Reports the service's state. |
| `show` | none | Brings the window forward. |
| `quit` | `reason`: `quit`, `replace` or `uninstall` | Finishes in-flight work and exits, sending the matching goodbye first. |

An accepted control opening is answered by one frame, `{"kind": "result", "value": ...}`, and the
stream closes. The value is the service's to define. The 2.0 service answers `readiness` with its
readiness summary, `status` with `{service, instance, pid, started_by}` (`started_by` is the start
reason the service was started with) and, when the service has them, `connections`
(`{mcp_clients, command_line, browsers}`, how many of each are connected) and `workbench`
(`{url, usual_port, watching}`, where the workbench is served and whether other screens on this
computer may watch; the bearer that opens the window is never part of it), `show` with `{shown}`
(whether a window came forward) and `quit` with `{quitting: true}`. The `status` answer also
carries `window` when the service has something to say: why there is, or is not, a Ghostlight
window (`shown`, `tray`, `no_display`, `no_interactive_desktop`, `no_shell_file`, `no_webview`,
`no_tray` or `did_not_stay`). That word is service-defined status content, added after the
revision's first form and unknown to the frozen edge, which never reads it. A reader treats
`connections`, `workbench` and `window` as optional: an older service answers without them. An operation this revision
does not know is refused as `unsupported`.

**`desktop`** carries the same-build shell contract of ADR-0199 Decision 7 and its 2026-10-15
amendment. **`command`** is reserved; its stream, when it lands, is defined by the command contract
of ADR-0199 Decision 6. Neither changes this protocol.

## 6. Goodbye

Before the service closes streams on purpose it sends a goodbye with one of four reasons: `quit`,
`replace`, `uninstall` or `session_end`.

- On an `mcp` stream the goodbye is the frame `{"kind": "goodbye", "reason": "<reason>"}`.
- On a `browser` stream it is the exact bytes `{"goodbye":"<reason>"}`, with no spaces. A peer looks
  only at frames that start with `{"goodbye":"` and never parses any other frame; adapter frames
  start with `{"kind":`. See `browser.goodbye.txt`.

What each peer does:

| Reason | MCP edge | Browser edge | Shell |
|---|---|---|---|
| `quit` | Answers every outstanding request from the `cut_off` template and holds nothing. The next request starts the service again, as an `agent` start. | Tells the extension the service is away, waits, never starts the service, and exits when a service accepts again. | Exits. |
| `replace` | Answers outstanding requests as cut off, except the handshake and listings it sends again, then holds lines and reconnects without starting the service for up to the hold limit. After that it behaves as after a lost connection. | As for `quit`: it exits when a service accepts again, so the extension reconnects with a fresh hello. | Shows Updating. |
| `uninstall` | Answers outstanding requests as cut off, then exits. | Exits at once. | Exits. |
| `session_end` | Exits. | Exits. | Exits. |

The service sends the goodbye after in-flight work has finished and before it closes the streams.
A stream that ends without a goodbye is a crash: MCP edges answer cut-off requests from the templates, send the
handshake and listings again, and resume by token; browser edges exit on the next accepted opening.

## 7. Start reasons

Every start of the service carries a reason: `agent`, `browser`, `command`, `person`, `recover` or
`update`. A starter passes it as `serve --reason <word>`; `serve` without one of the six is a usage
error. The service writes `state/quit` when a person quits (the tray, the workbench, or control
`quit` with reason `quit`), before it exits. In order, a starting service refuses a wrong image
and an elevated token, then, for a `browser` or `recover` start, exits 0 at once while that file
exists. It then takes `run/service.lock`: a start that finds the lock held waits for it, an
`update` up to 30 s and any other start up to 5 s, and then exits 0. Only after it holds the lock
does any other start delete the file and open state; a `browser` or `recover` start that waited
looks at the file again first. A service that is not accepting openings within 30 s of taking the lock
exits with a nonzero status.

| Reason | Who uses it | After a quit |
|---|---|---|
| `agent` | The MCP edge, at its own start and for each held request. | Starts the service. |
| `browser` | The browser edge, once, at its own start. | Refused. |
| `command` | A typed command that needs the service. | Starts the service. |
| `person` | `open`, a bare launch, or the shell launched by a person. | Starts the service. |
| `recover` | The shell, after a loss without a goodbye. | Refused. |
| `update` | The installer, after a swap. | Starts the service. |

## 8. Failure templates

The reply's `templates` carry the only words an MCP edge may say about a failure. `cut_off` is for
a request outstanding when the service went away, `expired` for a held request that outlived the
hold limit. Each maps a request method to a JSON-RPC response object, with `default` for any
method not named. The edge sets the object's `id` to the request's id, keeping its JSON type. With
no templates at all (before the first connection) it answers a JSON-RPC internal error, code
-32603, with an empty message.

## 9. Tolerance

- Every closed word (role, start reason, goodbye reason, quit reason, program, refusal code, MCP
  frame kind, control operation) decodes to an `Unknown` value when this revision does not know it.
  The service refuses an unknown role or operation as `unsupported`; an edge treats an unknown
  goodbye reason as the end of the stream and an unknown refusal code as a refusal.
- Readers ignore fields they do not know. Fields are never removed or repurposed, and new behavior
  gets a new name.
- The service accepts every relay revision from 4 and uses only what each peer advertises.
- `tests/golden/relay4/tolerance` holds one fixture for each of these rules.

## 10. Timings and bounds

| Name | Value |
|---|---|
| Opening deadline | 5 s from connect to a complete opening; the service drops the connection after that. |
| Largest frame | 8 MiB |
| Hold limit | An MCP edge holds at most 4 MiB for at most 30 s while the service is away. |
| Start backoff | First wait 500 ms, doubling to at most 30 s. |

The goldens' `vocabulary.json` lists every closed word, the endpoint rules with the worked name
above, the run directory, socket and lock names, and these bounds. A change to any of them is a
change to the protocol.
