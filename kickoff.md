# Kickoff: Phase 0 — Reference Study + Project Bootstrap

Read CLAUDE.md and docs/SPEC.md fully before doing anything else. These are your authoritative sources.

Then execute the following:

## 1. Clone the reference implementation

```
mkdir -p reference
git clone https://github.com/noemica-io/open-claude-in-chrome.git reference/open-claude-in-chrome
```

## 2. Study the reference codebase

Read every file in `reference/open-claude-in-chrome/`. The codebase is ~2,200 lines across 6 files. Focus on:

- `host/mcp-server.js` — the MCP server. Extract the complete tool schemas (every tool name, parameter, type, description, enum). These schemas are sacred and must be preserved verbatim in our implementation.
- `host/native-host.js` — the native messaging bridge. Understand the 4-byte LE protocol and the TCP relay (which we're eliminating).
- `extension/service-worker.js` — the extension. Understand every CDP call, the tab group management, the keepalive alarm, the state recovery pattern, the coordinate normalization (`Emulation.setDeviceMetricsOverride`), and the screenshot pipeline.
- `extension/manifest.json` — permissions, native messaging host declaration.
- `install.sh` — native messaging host registration across browsers.

## 3. Write `reference/ANALYSIS.md`

Document your findings in a structured analysis file:

### Section 1: Tool Schemas
For each of the 18 tools, record:
- Tool name (exact string)
- Input schema (exact JSON schema object)
- Description (exact string)
- Classification: Observe / Mutate / Manage / Excluded (per our spec §3)

### Section 2: CDP Commands
For each tool, list the CDP methods it invokes and their parameters. This is the mapping from MCP tool call to CDP command(s).

### Section 3: Native Messaging Protocol
Document the message format between the native host and the extension. Include: handshake, command dispatch, response routing, error handling, reconnection.

### Section 4: Extension Lifecycle
Document: service worker keepalive strategy, tab group state recovery on restart, debugger attachment lifecycle, console/network event buffering.

### Section 5: Screenshot Pipeline
Document: format selection, quality settings, coordinate normalization (deviceScaleFactor override), compression fallback, when screenshots are returned vs not.

### Section 6: Shadow DOM Handling
Document: the `form_input` shadow DOM traversal pattern for web components (the Reddit search bar fix).

### Section 7: Known Issues and Fixes
Document all 6 production bugs from the build story and their solutions, mapped to our architecture (which ones we inherit, which ones we avoid by design).

## 4. Initialize the Rust project

```
cargo init --name browser-mcp
```

Set up the directory structure from CLAUDE.md. Create empty module files with doc comments explaining each module's purpose. Set up Cargo.toml with the specified dependencies. Ensure `cargo build` succeeds (even though modules are empty stubs).

## 5. Extract tool schemas

From the analysis, create a `src/mcp/schemas/` directory (or similar) containing the raw JSON tool schemas extracted verbatim from the reference. These will be used as const literals in tool registration.

## 6. Verify

- `reference/ANALYSIS.md` exists and is complete.
- `cargo build` succeeds.
- `cargo clippy` passes.
- The tool schemas are captured and ready for Phase 1.

Report what you found, what surprised you, and what decisions from the spec are validated or challenged by the reference code.
