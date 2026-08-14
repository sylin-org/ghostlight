# Maturity-1: shared design and pinned semantics

Normative companion to the m01-m06 task prompts in this directory. Prompts cite
this file instead of restating semantics; if a prompt and this file disagree,
STOP and record a BLOCKED entry (do not resolve the conflict by judgment).

## Provenance (decided questions; do not re-litigate)

- ADR-0026 (release maturity sequencing) and ADR-0027 (open-core licensing) are
  the controlling decisions. The founder confirmed in-session on 2026-07-03:
  Windows-and-Linux CI matrix plus tag-triggered release artifacts; full SPEC rewrite
  (excluded from this batch, see below); syslog + none audit destinations now,
  http deferred; managed:// decided but excluded from this batch (see below);
  extension pure-logic extraction plus a headless smoke now.
- The license files (LICENSE, LICENSE-APACHE, LICENSE-MIT, LICENSE-GOVERNANCE,
  LICENSING.md) already exist at the repo root (commit 56ee80e). No task in
  this batch creates or edits license files.
- The engine/governance license boundary is the src/governance/ directory
  (ADR-0027 Decision 4). SPDX ids: `Apache-2.0 OR MIT` (engine) and
  `LicenseRef-Ghostlight-Commercial` (governance).
- Stage-4 t-live-1 WAS live-verified (commit 44db1f3; recorded at
  docs/tasks/stage-2/BROWSER-TESTS.md line 100). The stage-4 LEDGER closing
  statement predates that pass and is corrected by m01 via an APPENDED note
  (ledgers are append-only).

## Excluded from this batch (do not attempt)

- docs/SPEC.md full rewrite (ADR-0026 Decision 3): authoring normative
  semantics is not executor work; a frontier session owns it.
- managed:// implementation (ADR-0026 Decision 5): requires a platform-fact
  verification spike (OS policy-store paths) and an extension-id decision
  (pinned dev id vs store id) that only the founder can make. The decision to
  build it stands; its execution package comes later.
- http audit destination (deferred by ADR-0026 Decision 4 with a trigger).
- Extraction of the DUCKABLE content.js units (accessibility-tree measure/emit,
  shadow-DOM innerInput, find): they are closure-scoped in an IIFE and walk DOM
  surfaces; extracting them safely needs oracles a frontier session must pin
  first. m05 extracts only the CLEAN service-worker units. The m06 smoke covers
  read_page and form_input end to end in compensation.
- Trademark filing, CWS listing type, pricing: founder actions, not repo tasks.

## Pinned semantics

### SPDX headers (m02)

Header lines, exactly:

- Engine .rs and extension .js files: `// SPDX-License-Identifier: Apache-2.0 OR MIT`
- src/governance/ .rs files: `// SPDX-License-Identifier: LicenseRef-Ghostlight-Commercial`

The header is line 1 of the file, followed by the file's existing first line
(module doc comments `//!` may follow a `//` line). Scope: all .rs under src/
and tests/ EXCEPT tests/tool_schema_fidelity.rs (never-touch), plus the four
.js files under extension/. No .ps1, .json, .md, or .html files. Every NEW file
created by m03-m06 carries the appropriate header from birth (workflows YAML:
none; .rs/.js/.mjs: yes, engine id).

### CI (m03)

Two workflows. ci.yml: a fmt job (ubuntu, `cargo fmt --check`) and a test job
matrixed over ubuntu-latest and windows-latest running
`cargo clippy --all-targets -- -D warnings` then `cargo test`. release.yml: on
tags `v*`, builds for x86_64-pc-windows-msvc and x86_64-unknown-linux-gnu on native runners and
uploads the binary as an artifact. The literal YAML is pinned in m03; transcribe it.

### Audit destinations (m04)

- `audit.destination` enum variants become exactly `["file", "stderr",
  "syslog", "none"]` (order pinned; goldens regenerate from it). Defaults stay
  "file" in all three presets.
- New key `audit.syslog.address`, type Str, default `"127.0.0.1:514"` in all
  three presets, description "UDP target for the syslog audit destination, as
  host:port." No new constraint kind.
- "syslog" wire format: one RFC 5424 datagram per record over UDP, payload
  exactly `<134>1 {ts} - ghostlight {pid} - - {line}` where 134 = facility 16
  (local0) * 8 + severity 6 (info), `{ts}` is UTC RFC 3339 with millisecond
  precision and trailing Z (chrono `to_rfc3339_opts(SecondsFormat::Millis,
  true)` at send time), `{pid}` is `std::process::id()`, and `{line}` is the
  serialized JSONL record unchanged. HOSTNAME, MSGID, and STRUCTURED-DATA are
  the RFC NILVALUE `-`.
- Transport: `std::net::UdpSocket::bind("0.0.0.0:0")` then `send_to`, one
  socket per record (mirrors the open-per-record file destination). Send
  errors: `tracing::warn!` and swallow (mirror the file arm; audit failures
  never break a tool call).
- Address resolution: at `resolve_inner` time via `std::net::ToSocketAddrs`,
  first result wins. Unresolvable/invalid address: `tracing::warn!` and resolve
  to None (audit disabled), mirroring the existing no-data-directory behavior.
- "none" maps to None inner in `resolve_inner` (records intentionally
  discarded; `is_enabled()` reports false). No new Inner variant for it.
- The existing defensive `_ =>` fallback-to-file arm stays last.
- Golden regeneration is the sanctioned two commands run under GIT BASH (never
  PowerShell redirection, which writes UTF-16/CRLF):
  `cargo run --quiet -- config schema > tests/golden/config-schema.json` and
  `cargo run --quiet -- config docs > tests/golden/config-keys.md`, then hand
  review of the diff.

### Extension lib extraction (m05)

- New directory extension/lib/ with two classic scripts loaded by the service
  worker via `importScripts("lib/geometry.js", "lib/keys.js")` as the first
  executable statement of service-worker.js (after its top comment block).
  content.js and the manifest are NOT touched (the extracted units live only in
  the service worker).
- Dual-environment export footer, exactly this pattern in each lib file:

      const <Name> = { ...exports... };
      if (typeof module !== "undefined" && module.exports) {
        module.exports = <Name>;
      } else {
        self.<Name> = <Name>;
      }

  with `<Name>` = `GhostlightGeometry` and `GhostlightKeys`.
- Function bodies move VERBATIM (byte-identical logic; only the wrapper
  changes). rescaleCoord's pure core becomes `rescaleCtxCoord(ctx, x, y)`
  taking the context record (or null); the service worker keeps a thin
  `rescaleCoord(tabId, x, y)` delegating with `screenshotCtx.get(tabId)`.
- Unit tests live OUTSIDE extension/ (so the store zip stays clean) at
  tests/extension/*.test.js, run with `node --test tests/extension/`. No
  package.json, no dependencies.

### Headless smoke (m06)

- Lives in tests/e2e/ (own package.json, private, devDependency playwright).
  CI runs it on ubuntu only (a separate job m06 adds to ci.yml). The full
  browser run cannot be verified on the Windows executor machine; m06's local
  verification is syntax plus dry-run, and the ledger records that the live CI
  run is deferred to the first push.
- Architecture: build the binary; serve the pinned fixture page over local
  http; create a temp Chromium user-data-dir; write the native-messaging host
  manifest (name org.sylin.ghostlight, allowed origin
  chrome-extension://cjcmhepmagomefjggkcohdbfemacojoa/) into
  <user-data-dir>/NativeMessagingHosts/, pointing at a generated wrapper shell
  script that exports a unique GHOSTLIGHT_ENDPOINT and execs the binary; launch
  Chromium via Playwright launchPersistentContext in the NEW headless mode
  (channel "chromium", headless true -- the old headless shell does not load
  extensions) with --disable-extensions-except/--load-extension, xvfb-headed
  fallback; spawn the same binary as the
  MCP server (stdio JSON-RPC) with the same GHOSTLIGHT_ENDPOINT; drive
  initialize, tools/list, navigate, read_page, computer screenshot, form_input,
  computer click by ref, read_page again; assert the pinned markers.

## Global rules for all tasks

- ASCII only in code; the ghost glyph exists only as the `\u{1F47B}` escape.
- No em-dashes or smart quotes in any file this batch writes.
- New .rs/.js/.mjs files carry the SPDX header for their side of the boundary.
- The sacred tool surface (src/transport/mcp/schemas/tools.json,
  tests/tool_schema_fidelity.rs) is never touched by any task.
- The extension stays policy-free: no task adds any access decision to
  extension code.
