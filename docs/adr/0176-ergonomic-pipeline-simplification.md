# ADR-0176: Ergonomic Pipeline Simplification, Invariant Centralization, and Settle Policy Unification

Date: 2026-09-15. Status: Accepted by owner direction.

Builds on ADR-0169, ADR-0170, ADR-0173, ADR-0174, and ADR-0175.

## Context

Following the implementation of ADR-0169 through ADR-0175, an architectural evaluation of the
modular monolith identified three recurring sources of friction in internal feature evolution:

1. **Magic Number Drift in Catalog Invariants**: The catalog size (24 tools) was asserted as a literal
   across five distinct locations in Rust and JavaScript test harnesses. Adding or modifying a tool
   required manual restamping across disparate files.
2. **Scattered Settle Options in Bridge Contracts**: Bounded visual settlement (`visual_settle: Option<bool>`)
   was introduced on several independent commands (screenshot, region screenshot, wait) with duplicated
   boolean fields and option unpacking.
3. **Repetitive Execution Ceremony**: Operation handlers in `crates/orchestrator/src/work/` repeatedly
   implemented 15--20 lines of identical authorization checks, decision validation, blocked terminal
   generation, and tab/target resolution.

## Decision

1. **Centralize Catalog Count Invariants**:
   - `crates/orchestrator/src/language/catalog.rs` exports `pub const CATALOG_TOOL_COUNT: usize = EXPECTED_TOOL_NAMES.len();`.
   - `crates/orchestrator/src/language/mod.rs` re-exports `CATALOG_TOOL_COUNT`.
   - Desktop and integration test suites import or derive tool counts from this single canonical
     source of truth, eliminating magic number drift.

2. **Unified Settle Policy Value Object**:
   - Introduce `SettlePolicy` in `crates/bridge/src/browser.rs` as a reusable value object for
     settle and stabilization configuration:
     ```rust
     #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
     pub struct SettlePolicy {
         #[serde(default = "default_settle_true")]
         pub visual_settle: bool,
     }
     ```
   - Provide seamless conversions `From<bool>` and `From<Option<bool>>` to preserve wire-format
     backwards compatibility while enabling clean domain composition.

3. **Standardized Execution Helpers**:
   - Introduce higher-order execution helper methods on `ApplicationExecutor`:
     - `with_authorized_tab`: Encapsulates authorization, permission failure handling, tab lookup,
       and error terminal generation before delegating to the tab-bound action.
     - `with_authorized_tab_and_target`: Encapsulates authorization, tab and semantic target
       resolution, and failure handling before delegating to the targeted pointer/input action.
   - Refactor reading, pointer, and navigation handlers to use these templates, removing repetitive
     ceremony while strictly preserving the existing governance and completion gate invariants.

4. **Preserved Boundaries and Non-Goals**:
   - The 4-process topology (Client, MCP Connector, Orchestrator, Browser Connector, Extension)
     remains strictly intact.
   - The extension remains 100% policy-free; no tool definitions or permission logic are moved to JS.
   - The closed `DomainEvent` enum is preserved; no generic dynamic event buses or reflection
     mechanisms are introduced.
