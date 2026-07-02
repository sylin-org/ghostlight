# 0001. Single portable Rust binary + thin Chromium extension

- Status: Accepted
- Date: 2026-07

## Context

AI coding agents (Claude Code, Cursor, etc.) need governed access to the user's own
authenticated Chromium session. The job spans two things at once: translating MCP tool
calls into CDP actions, and governing them (policy, tool r/w classification, audit,
screenshot compression). The prior-art reference (open-claude-in-chrome) does this in
Node.js across five processes, so every install inherits a Node/npx runtime dependency
and its attendant class of install failures (SPEC Appendix B: reference runtime deps are
Node.js + Chrome, process count 5).

The design question is where each responsibility lives, and in how many artifacts.

## Decision

One zero-dependency Rust binary is the whole engine. It hosts the MCP server on stdio
(hand-rolled JSON-RPC 2.0), is the browser's native-messaging host, and owns policy
enforcement, audit generation, and screenshot compression (SPEC 2.1, binary
responsibilities row; CLAUDE.md). Tool schemas are embedded verbatim in the binary as a
JSON fixture guarded by a fidelity test, so the sacred external contract cannot drift
(commit cd9e6d4).

The Manifest V3 extension is a thin CDP executor: it executes CDP commands, buffers
console/network events, manages the "MCP" tab group, and runs keepalive/state recovery.
It carries mechanism but no policy (SPEC 2.4; commit 9be07e5). The division of runtime
instances is refined in ADR-0002; the extension's policy-free invariant in ADR-0005.

## Consequences

- No Node/npx: the install-failure class that plagues Node-based browser MCPs does not
  exist; a single portable executable ships with zero runtime dependencies.
- All engine logic (policy, audit, compression, MCP framing) lives in Rust and is unit-
  and integration-testable; the extension holds only mechanism.
- Three processes and two protocol boundaries instead of the reference's five and four
  (SPEC Appendix B; README architecture section).
- Trade-off: the split does not reduce the trust surface of the code. The extension
  still has full CDP access to the session (SPEC 9.1); it constrains the usage surface,
  not the code trust.
- Follow-up: per-platform native build and packaging replace the "just run npx" story.
