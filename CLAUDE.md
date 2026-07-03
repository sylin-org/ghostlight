# Browser MCP: Governed Browser Automation

## Project Identity

This project builds a governed browser automation MCP server: a single Rust binary + thin Chromium extension that gives any MCP client (Claude Code, Cursor, Zed, Cline, and others) controlled access to the user's authenticated browser session, with identity-bound access control, per-action capability classification (read, action, write, execute), and structured audit logging.

The authoritative design specification is `docs/SPEC.md`. Read it fully before writing any code. Every implementation decision should trace back to a section in the spec.

## Origin

This is a clean-room Rust rewrite informed by [open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome), a Node.js reimplementation of Anthropic's Claude in Chrome extension. The reference repo is cloned into `reference/open-claude-in-chrome/` for study. We are not forking it. We are understanding what it does and rebuilding the concept in Rust with a fundamentally different architecture (governance-first, single-binary, no Node.js dependency).

**Critical constraint:** Preserve the exact MCP tool names, parameter signatures, and description strings from the reference implementation's tool schemas. Claude was trained against these schemas. The 13 trained tool schemas must stay byte-identical to what the official Claude in Chrome extension advertises; exactly one additive, argument-less governance tool named `explain` is sanctioned on top (ADR-0022 Decision 7). No other addition, removal, or edit is sanctioned. Our governance layer shapes which tools are visible and when they execute, but the trained schemas themselves are sacred.

## Architecture (from spec §2)

```
MCP Client <--stdio--> Binary <--native messaging--> Extension <--CDP--> Browser
```

Three processes, two protocol boundaries.

- **Binary (Rust):** MCP server on stdin/stdout. Native messaging host for the extension. Policy enforcement. Audit. Screenshot compression. Single portable executable, zero runtime dependencies.
- **Extension (JS, Manifest V3):** Thin CDP executor. No policy logic. Recovers from service worker death.
- **Browser:** User's Chromium browser. Untouched.

## Repository Structure

```
browser-mcp/
├── CLAUDE.md                    # This file
├── Cargo.toml                   # Workspace root
├── docs/
│   └── SPEC.md                  # Authoritative specification
├── src/                         # Rust binary source
│   ├── main.rs                  # Entry point, CLI args, startup orchestration
│   ├── mcp/                     # MCP protocol layer
│   │   ├── mod.rs
│   │   ├── server.rs            # JSON-RPC 2.0 over stdio
│   │   ├── types.rs             # MCP message types
│   │   └── tools.rs             # Tool definitions and schemas
│   ├── native/                  # Native messaging layer
│   │   ├── mod.rs
│   │   ├── host.rs              # 4-byte LE length prefix protocol
│   │   └── messages.rs          # Extension command/response types
│   ├── policy/                  # Governance layer
│   │   ├── mod.rs
│   │   ├── manifest.rs          # Manifest parsing and validation
│   │   ├── grants.rs            # Grant resolution and domain matching
│   │   └── enforcement.rs       # Per-call policy checks
│   ├── tools/                   # Tool implementations
│   │   ├── mod.rs
│   │   ├── navigate.rs          # Navigation with domain enforcement
│   │   ├── computer.rs          # Screenshot, click, type, etc.
│   │   ├── read_page.rs         # Accessibility tree
│   │   ├── form_input.rs        # Form interaction
│   │   ├── javascript.rs        # JS execution
│   │   ├── find.rs              # Element search
│   │   ├── page_text.rs         # Text extraction
│   │   ├── tabs.rs              # Tab management
│   │   ├── network.rs           # Console/network event retrieval
│   │   └── manage.rs            # resize_window, update_plan
│   └── audit/                   # Audit subsystem
│       ├── mod.rs
│       ├── record.rs            # Audit record types
│       └── destinations.rs      # File, syslog, http, stderr
├── extension/                   # Chromium extension (Manifest V3)
│   ├── manifest.json
│   ├── service-worker.js        # CDP execution, native messaging, keepalive
│   └── native-messaging.json    # Host manifest template
├── reference/                   # Reference implementation (read-only study material)
│   └── open-claude-in-chrome/   # Cloned repo
├── tests/                       # Integration tests
│   ├── manifest_validation.rs
│   ├── grant_resolution.rs
│   ├── domain_matching.rs
│   └── tool_enforcement.rs
├── examples/                    # Example manifests
│   ├── enterprise-healthcare.json
│   ├── developer-unrestricted.json
│   └── qa-staging.json
└── scripts/
    ├── install.sh               # Native messaging host registration
    └── install.ps1              # Windows equivalent
```

## Implementation Phases

### Phase 0: Reference Study
- Clone `https://github.com/noemica-io/open-claude-in-chrome` into `reference/`.
- Read and understand every file. The codebase is ~2,200 lines across 6 files.
- Document the following in a `reference/ANALYSIS.md`:
  - The exact tool schemas (names, parameters, descriptions): these must be preserved verbatim.
  - The CDP commands used by each tool.
  - The native messaging protocol (message format, handshake, keepalive).
  - The extension's service worker lifecycle management (keepalive alarm, state recovery).
  - The screenshot pipeline (format, quality, coordinate normalization).
  - The shadow DOM traversal for `form_input`.
  - Known bugs and their fixes (all 6 from the build story).
- This phase produces no Rust code. It produces understanding.

### Phase 1: Skeleton
- Initialize the Rust workspace with `cargo init`.
- Implement the MCP server on stdio: handle `initialize`, `tools/list`, and return hardcoded tool schemas extracted from Phase 0.
- Implement native messaging host protocol (4-byte LE framing).
- Verify: Claude Code can launch the binary, see the tools, and send a call (which returns a stub response).
- No extension yet. No policy yet. Just the protocol plumbing.

### Phase 2: Extension + CDP
- Build the Manifest V3 extension based on the reference implementation's patterns.
- Implement the native messaging connection between binary and extension.
- Implement the core tools: `navigate`, `computer` (screenshot only), `read_page`, `get_page_text`, `tabs_context`, `tabs_create`, `resize_window`.
- Implement coordinate normalization (`Emulation.setDeviceMetricsOverride`).
- Implement screenshot compression (JPEG, quality fallback).
- Implement service worker resilience (keepalive alarm, tab group state recovery).
- Verify: Claude Code can navigate to a URL, take a screenshot, and read the page.

### Phase 3: Full Tool Set
- Implement remaining tools: `computer` (all actions), `form_input` (with shadow DOM traversal), `javascript_tool`, `find`, `read_console_messages`, `read_network_requests`, `update_plan`.
- Verify parity with the reference implementation using the test prompt from `reference/open-claude-in-chrome/test-prompt.md`.

### Phase 4: Policy Engine
- Implement manifest parsing and validation (`policy/manifest.rs`).
- Implement domain pattern matching with wildcard support (`policy/grants.rs`).
- Implement per-call enforcement at all five enforcement points from spec §5 (`policy/enforcement.rs`).
- Implement tool advertisement filtering based on manifest grants.
- Implement `computer` sub-action classification (per-action capability requirements: read, action, write, execute).
- Implement denial response formatting.
- Implement manifest source loading: `file://`, `env://`, and default (no manifest = unrestricted).
- Write thorough unit tests for grant resolution and domain matching.
- Verify: with a restrictive manifest, Claude Code only sees permitted tools and gets clear denials on unauthorized actions.

### Phase 5: Audit
- Implement audit record generation on every tool call.
- Implement audit destinations: `file` (JSON lines), `stderr`, `syslog` (defer `http` to later).
- Implement sensitive field handling (parameter/screenshot omission based on manifest config).
- Verify: audit log captures permitted calls, denied calls, identity, domain, grant ID, and timing.

### Phase 6: Platform + Packaging
- Cross-compile for Linux x86_64, macOS x86_64/aarch64, Windows x86_64.
- Write install scripts for native messaging host registration (bash + PowerShell).
- Write example manifests (enterprise healthcare, developer unrestricted, QA staging; from spec Appendix A).
- Write README with installation instructions for both enterprise and personal deployment.
- Verify: the binary works on all three platforms with no runtime dependencies.

## Key Technical Decisions

### Rust Crates
- **`tokio`**: Async runtime. MCP stdio and native messaging are concurrent I/O streams that must be multiplexed.
- **`serde` / `serde_json`**: JSON serialization for MCP messages, native messaging, manifests, audit records.
- **`clap`**: CLI argument parsing (`--manifest`, `--version`, `--help`).
- **`tracing` / `tracing-subscriber`**: Structured logging (separate from audit; this is debug/operational logging).
- **`uuid`**: Audit event IDs.
- **`chrono`**: Timestamps in audit records.
- **`glob` or hand-rolled**: Domain wildcard matching (simple enough to not need a crate).
- **Do NOT use** an MCP SDK crate. The MCP protocol is simple JSON-RPC 2.0 over stdio. Hand-roll it. External MCP crates add dependency risk and may not match the exact tool schema format we need to preserve.

### Native Messaging
The Chromium native messaging protocol is:
1. Read 4 bytes (little-endian u32) = message length.
2. Read `length` bytes = UTF-8 JSON message.
3. Write: 4-byte LE length prefix + JSON payload.

The binary is both the native messaging host (extension connects to it) AND the MCP server (Claude Code connects via stdio). These two streams are multiplexed on the tokio runtime. A message from the MCP client triggers a command to the extension; the extension's response is routed back to the MCP client.

### Tool Schema Preservation
The tool schemas must be extracted verbatim from the reference implementation. Every tool name, parameter name, type, description, and enum value must be byte-identical. Do not paraphrase descriptions. Do not rename parameters. Do not reorder fields. The model's trained behavior depends on exact schema matching. The one sanctioned exception is the additive `explain` directory tool (ADR-0022 Decision 7); it is not part of the trained surface and its schema is pinned by `tests/tool_schema_fidelity.rs`.

In the Rust code, define tool schemas as const string literals (the raw JSON) rather than building them programmatically. This prevents accidental drift.

### Screenshot Behavior
Return screenshots only on `computer` actions that produce one: `screenshot`, `scroll`, and `zoom`.
- For all other actions (`left_click`, `type`, `key`, `hover`, `left_click_drag`, `scroll_to`, etc.), return a text confirmation of the action.
- This reduces context consumption ~10x in multi-step workflows.
- JPEG quality 55, falling back to 30 above the size budget.
- Coordinate model: no device-metrics override. Each screenshot probes the CSS viewport + DPR, captures at native resolution, and downscales to a token budget (`ceil(w/28)*ceil(h/28) <= 1568` tokens, longest side `<= 1568`px) via OffscreenCanvas. A per-tab ScreenshotContext records the CSS viewport and final screenshot pixel dims; model-provided coordinates are rescaled back to CSS viewport px via `round(v * viewportDim / screenshotDim)` before Input dispatch (`getBoundingClientRect`-derived coordinates are already CSS px and are not rescaled). See ADR-0010 (and ADR-0009 for the superseded `deviceScaleFactor:1` approach).

### Extension Design Principle
The extension is **policy-free**: it holds mechanism but makes no access decisions. All policy, tool classification, and audit live in the binary.
- The extension executes CDP commands and runs DOM reads (accessibility tree, `find`, `form_input` with shadow-DOM traversal) in a content script, so it is *policy-free*, not necessarily *minimal* (Phase 0 decision; the reference does the same).
- The extension does NOT check domains, classify tools, generate audit records, or make any policy decision.
- The extension DOES manage: debugger attachment, tab group lifecycle, keepalive alarms, console/network event buffering, DOM-read mechanism, and service worker state recovery.

## Code Style
- Rust 2021 edition.
- Use `thiserror` for error types.
- Use `anyhow` in main/integration code, typed errors in library code.
- Prefer explicit types over inference in public APIs.
- Every public function has a doc comment.
- Every module has a module-level doc comment explaining its role in the architecture.
- Tests go in `tests/` for integration tests, inline `#[cfg(test)]` for unit tests.
- No `unsafe` unless absolutely required (and document why).
- Format with `rustfmt`, lint with `clippy` (deny warnings).

## Testing Strategy
- **Unit tests:** Domain matching, grant resolution, manifest validation, tool classification, denial message formatting. These are pure functions: no I/O, no mocking.
- **Integration tests:** Binary launches, handles MCP initialize, advertises correct tools for a given manifest, enforces policy on tool calls. These use the binary as a subprocess with stdio pipes.
- **Manual verification:** After each phase, use the test prompt from the reference repo (adapted) to verify end-to-end behavior with Claude Code.

## What NOT To Build
Read spec §10 carefully. These are explicit exclusions:
- No OIDC/SAML/LDAP integration.
- No remote policy service (HTTP manifest source).
- No multi-user multiplexing.
- No content inspection or DLP.
- No manifest signing.
- No Firefox support.
- No `upload_image` tool.
