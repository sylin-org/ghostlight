# ADR-0160: Local service continuity and bounded exchanges

- Status: Accepted
- Date: 2026-09-07
- Amends: ADR-0096 service admission, ADR-0105 Windows FFI scope, ADR-0113 transport failure
  handling, ADR-0157 completion notices
- Preserves: ADR-0106 caller-owned lifetime, ADR-0157 human controls, ADR-0159 audit/effect truth

## Decision

The owner approved H8 after rejecting a refusal-led experience. Ordinary bursts wait and complete
without intervention. A stalled client is cleaned up; another session and human controls remain
usable. Recovery admits future work without repeating an uncertain browser action. Waiting counts
against the original request deadline. Healthy idle connections remain connected.

Use the existing service, executor, completion, and workbench seams. No scheduler framework,
policy knobs, new process, automatic application retry, or protocol revision is introduced.

### Admission and user experience

Each authenticated connection has two fixed workers and small FIFO queues. Ordinary work uses one;
operations already independent of the workspace lease use the other. The language/executor owns
that distinction. Status and recording controls retain their existing lease semantics. Cancellation
stays on the reader; cancelled or expired queued work advances to completion without waiting for
the active browser operation. Duplicate request IDs never replace an admitted cancellation token.
IDs and capacity remain reserved until response delivery finishes or fails.

Decode once on intake, preserve the deadline while waiting, and use the ordinary executor and audit
completion for every accepted invocation and capacity refusal. Queued work acquires its authority
snapshot at the same existing seam, after workspace admission. It never receives an extended
deadline or skips current control checks because it waited.

Waits shorter than 500 ms are invisible. Sustained waits appear as "Waiting for earlier browser
work" in the existing workbench operation list, below running work. The same invocation becomes
running and then history. Waiting creates no popup or browser-page presentation. Capacity refusal
is exceptional and reports no effect and safe repetition only because work did not start. History
explains the failed request; the existing session and diagnostics surfaces remain available.

Draining requests after an explicit human Pause or Stop also produces quiet terminal history.
It preserves the fixed human directive and blocked/no-effect outcome without a guardrail popup for
each queued request. Actual policy denials and session review keep their existing notice behavior.

### Concrete implementation bounds

These are fixed implementation bounds, not authored governance:

| Resource | Bound |
| --- | --- |
| Unfinished handshakes | 16 per listener |
| Authenticated service connections | 64 |
| Registered browser identities | 16; replacement retires the old connection |
| Ordinary admitted requests | 32 per connection, including the active request |
| Independent admitted requests | 8 per connection, including the active request |
| Retained encoded request charge | 16 MiB per connection lane; 64 MiB ordinary and 8 MiB independent across the service |
| Message bytes | Existing 8 MiB frame limit |
| Authentication or incomplete frame | 5 seconds total; progress cannot restart the clock |
| Writer acquisition and delivery | 2 seconds total, also bounded by the invocation deadline |
| Human control publication | One 500 ms budget across the currently connected browsers |

The memory charge measures serialized input and retained identifiers plus a small metadata charge;
it is not an exact process heap limit. Frame parsing, results, browser assets, history, and
caller-owned workspaces have their own existing lifetimes and bounds. These controls do not claim
to prevent host-level denial of service or guarantee fairness against arbitrary connection churn.

Incomplete-frame time begins with its first byte, including a byte already prefetched while reading
the preceding frame. Authentication begins immediately. A healthy idle connection has no expiry.
Readers observe service shutdown; writer cleanup uses a separate socket handle and cannot deadlock
behind serialization. Writer failure closes the affected connection rather than appending another
message after a potentially torn frame. A caller's deadline while merely waiting for serialization
does not disconnect the operation already holding that writer. Retired heartbeat workers wake on
close rather than accumulating until the next periodic probe.

Browser controls still change authoritative state before publication. Failure to publish to a
stalled adapter interrupts that connection. Work already dispatched remains uncertain where its
receipt is missing. Connectors retain their existing automatic reconnection for future requests.

### Private runtime publication

Inspection found inherited broad-user access rules on this machine's installed Windows discovery
file. The token must be private at creation, not written and then protected. Windows now creates a
new file with an explicit current-user owner and a protected current-user/SYSTEM DACL. Linux uses
exclusive creation with mode 0600. Both publish from a fresh sibling temporary file; they never
reuse an existing temporary path or its permissions. Discovery reads have a 64 KiB ceiling.

The existing `ghostlight-win-peer` crate remains the only audited unsafe boundary. One small file
creation helper joins its socket-peer observation functions. Bridge runtime discovery calls this
helper on Windows; no other crate gains unsafe permission. The standard Rust file API does not
provide creation-time security attributes, so this uses the same documented Win32 FFI approach
already chosen in ADR-0105, with ownership guards and safety notes.

References: [Microsoft file security](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights),
[security descriptor conversion](https://learn.microsoft.com/en-us/windows/win32/api/sddl/nf-sddl-convertstringsecuritydescriptortosecuritydescriptorw),
[Rust Windows file options](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html).

This protects the discovery file's contents from inherited broad read permissions. It is not a
claim that an installation directory writable by another user is trustworthy, that an administrator
cannot take ownership, or that a same-user process is isolated from its own account. Installed file
replacement occurs only on an authorized service deployment/restart.

## Evidence

The [H8 record](../tasks/security-hardening/h8-local-continuity.md) owns source, process, UI, and
platform evidence. The installed runtime and publication state are recorded separately in STATUS.
