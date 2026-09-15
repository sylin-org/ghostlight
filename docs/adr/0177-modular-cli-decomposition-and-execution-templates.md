# ADR-0177: Modular CLI Decomposition and Universal Execution Templates

Date: 2026-09-15. Status: Accepted by owner direction.

Builds on ADR-0105, ADR-0115, ADR-0145, and ADR-0176.

## Context

Following an architecture and code quality assessment of the Ghostlight DDD monolith, two primary
sources of maintenance friction and conceptual mixing were identified:

1. **Monolithic Entry Point Inversion**: `crates/orchestrator/src/main.rs` had grown to 1,693 lines
   containing disparate responsibilities: command-line argument parsing, browser and MCP harness
   installation/uninstallation, diagnostic health checking (`doctor` and `status`), single-instance
   desktop activation and wait loops, Linux D-Bus startup handling, and execution runners for `call`
   and `policy` commands. This accumulated logic in the binary entry point violated single
   responsibility and obscured domain boundaries.
2. **Repetitive Lookup Ceremony for Optional Targets**: ADR-0176 introduced `with_authorized_tab` and
   `with_authorized_tab_and_target` execution templates. However, operations with optional targets
   (`read_page`, `inspect_document`, `scroll_page`, `perform_wait`, `run_script`, `type_focused`,
   and `perform_key`) still duplicated 15--20 lines of authorization validation, tab lookup, and
   optional target resolution before dispatching browser effects.

## Decision

1. **Universal Execution Template (`with_authorized_optional_target`)**:
   - Introduce `with_authorized_optional_target` on `ApplicationExecutor` in
     `crates/orchestrator/src/work/mod.rs`:
     ```rust
     pub(super) fn with_authorized_optional_target<F>(
         &self,
         context: &OperationContext,
         tab: Option<&TabHandle>,
         target: Option<&TargetHandle>,
         action: F,
     ) -> Terminal
     where
         F: FnOnce(&mut WorkspaceLease, &SelectedTab, Option<ResolvedTarget>) -> Terminal;
     ```
   - Refactor `navigation.rs`, `forms.rs`, `reading.rs`, and `pointer.rs` to leverage
     `with_authorized_tab` and `with_authorized_optional_target`.
   - Ensure target resolution failure, permission denial, and missing tab handling remain uniform
     across all browser operations while strictly preserving the existing governance and completion
     gate contracts.

2. **Modular CLI Decomposition**:
   - Decompose `crates/orchestrator/src/main.rs` into focused single-responsibility modules under
     `crates/orchestrator/src/cli/`:
     - `parse.rs`: Command-line intake parsing, `LaunchMode` resolution, `SetupOptions`,
       `NativeHostCommand`, help text rendering, and shell completion compatibility guards.
     - `setup.rs`: Native messaging host registration, browser package detection, MCP harness
       configuration, command path links, desktop application entries, and extension walkthrough handoff.
     - `doctor.rs`: Comprehensive environment inspection, readiness state gathering, diagnostics report
       rendering, and ownership-safe automated repairs.
     - `status.rs`: Local runtime endpoint observation, service port connectivity probing, and
       JSON/text status reporting.
     - `desktop.rs`: Desktop process lifecycle, single-instance workbench activation loops,
       Linux D-Bus start detection, and local execution runners for `call` and `policy`.
   - Add `pub fn dispatch(mode: parse::LaunchMode) -> anyhow::Result<()>` to
     `crates/orchestrator/src/cli/mod.rs` to route parsed intents to their respective module handlers.
   - Refactor `crates/orchestrator/src/main.rs` into a lean 13-line entry point that parses arguments
     and delegates directly to `ghostlight::cli::dispatch`.

3. **Preserved Boundaries and Non-Goals**:
   - Zero change to external CLI syntax, flags, subcommands, exit codes, or JSON schemas.
   - All tests (469 Rust tests, 283 extension tests, browser and process journeys) remain fully intact
     and passing.
   - Strict ASCII documentation and code formatting invariant maintained throughout.
