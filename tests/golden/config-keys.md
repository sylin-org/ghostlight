# Configuration key reference

Generated from the typed key registry in src/policy/mod.rs by `ghostlight config docs`.
Do not edit by hand; change the registry and regenerate.

Layer resolution: org-mandatory > user > org-recommended > preset default > built-in
Minimal. The built-in Minimal defaults equal the `safe` preset.

## `engine.connection.first_call_wait_ms`

Upper bound on the first-call wait for the extension handshake.

- Type: uint
- Constraints: integer between 0 and 60000
- Default (fully_open): 5000
- Default (safe, = built-in Minimal): 5000
- Default (restricted): 5000

## `engine.script.budget_ms`

Total wall-clock budget for one script tool call, in milliseconds.

- Type: uint
- Constraints: integer between 1000 and 480000
- Default (fully_open): 120000
- Default (safe, = built-in Minimal): 120000
- Default (restricted): 120000

## `content.security.secrets.redact`

Redact values of secret fields (password/OTP/payment) in read_page output.

- Type: bool
- Constraints: none
- Default (fully_open): false
- Default (safe, = built-in Minimal): true
- Default (restricted): true

## `content.security.sacred_domains`

Domains the agent must never touch: any tool call on a tab showing one of these domains, and any navigation targeting one, is denied. Always enforced.

- Type: string list
- Constraints: unique string elements; each a valid domain pattern
- Default (fully_open): []
- Default (safe, = built-in Minimal): []
- Default (restricted): []

## `audit.enabled`

Record one audit line per tool call (the flight recorder).

- Type: bool
- Constraints: none
- Default (fully_open): true
- Default (safe, = built-in Minimal): true
- Default (restricted): true

## `audit.destination`

Where audit records are written.

- Type: enum
- Constraints: one of: file, stderr, syslog, none
- Default (fully_open): "file"
- Default (safe, = built-in Minimal): "file"
- Default (restricted): "file"

## `audit.file.path`

Audit file path; empty means the platform default location.

- Type: string
- Constraints: empty string, or an absolute path
- Default (fully_open): ""
- Default (safe, = built-in Minimal): ""
- Default (restricted): ""

## `audit.syslog.address`

UDP target for the syslog audit destination, as host:port.

- Type: string
- Constraints: none
- Default (fully_open): "127.0.0.1:514"
- Default (safe, = built-in Minimal): "127.0.0.1:514"
- Default (restricted): "127.0.0.1:514"

## `governance.mode`

Default enforcement mode when the active manifest does not set one: observe records shadow denials, enforce blocks.

- Type: enum
- Constraints: one of: observe, enforce
- Default (fully_open): "observe"
- Default (safe, = built-in Minimal): "enforce"
- Default (restricted): "enforce"

## `inbound.web.from`

Sources allowed to connect to the local inbound.web adapter (the HTTP/WS ingestion listener). "localhost" only, unless opened to "*" or specific hosts.

- Type: string list
- Constraints: unique string elements
- Default (fully_open): ["localhost"]
- Default (safe, = built-in Minimal): ["localhost"]
- Default (restricted): ["localhost"]

## `inbound.web.enabled`

Whether the inbound.web adapter admits web (WS) tool sessions. Off by default: web ingestion is opt-in. An org-mandatory false denies the web adapter.

- Type: bool
- Constraints: none
- Default (fully_open): false
- Default (safe, = built-in Minimal): false
- Default (restricted): false

## `inbound.pipe.enabled`

Whether the inbound.pipe adapter (the named-pipe/UDS listener thin MCP adapters dial into) binds.

- Type: bool
- Constraints: none
- Default (fully_open): true
- Default (safe, = built-in Minimal): true
- Default (restricted): true

## `outbound.browser.enabled`

Whether the outbound.browser executor participates.

- Type: bool
- Constraints: none
- Default (fully_open): true
- Default (safe, = built-in Minimal): true
- Default (restricted): true

## `manage.web.enabled`

Whether the management-plane HTTP UI binds. An org-mandatory false takes the management UI off-line without affecting tool ingestion.

- Type: bool
- Constraints: none
- Default (fully_open): true
- Default (safe, = built-in Minimal): true
- Default (restricted): true

## `manage.web.from`

Sources allowed to reach the management-plane HTTP UI. Permanently loopback; cannot be widened.

- Type: string list
- Constraints: unique string elements
- Default (fully_open): ["localhost"]
- Default (safe, = built-in Minimal): ["localhost"]
- Default (restricted): ["localhost"]
