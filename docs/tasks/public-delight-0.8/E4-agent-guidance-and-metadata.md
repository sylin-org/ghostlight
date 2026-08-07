# E4: Agent guidance and metadata

## Outcome

Improve how agents and static analyzers understand every Ghostlight tool while proving that the
trained compatibility signature did not change.

## Required work

1. Inventory all 25 `ToolDescriptor` entries in `crates/core/src/browser/directory.rs`.
2. Review each advertised description for:
   - the job it solves;
   - when to choose it over the nearest alternative;
   - material side effects;
   - the exact callable names it references; and
   - one useful recovery action where the tool has a common recoverable failure.
3. Review parameter descriptions, display titles, standard MCP annotations, examples, expected
   results, output-field descriptions, and shared workflow guidance. Improve them where the added
   words change agent understanding, not merely tone.
4. Keep mixed-tool annotations conservative. In particular, `computer` remains a mixed read/action
   tool with its existing official name, actions, parameters, types, enums, order, and structural
   schema.
5. Use `computer`'s external B grade as a review prompt only. Do not split the tool, rename it,
   remove actions, change the action order, add required parameters, or weaken side-effect truth.
6. Update the fidelity snapshot intentionally for approved prose and metadata changes.
7. Add or strengthen a test that compares the pre-pass trained identity projection with the final
   declarations if the existing tests do not already prove names, parameters, types, enums,
   ordering, requiredness, and structural schema stability.
8. Recheck the rendered `tools/list` output for both MCP revisions. Revision projections may add
   protocol metadata but must not fork the canonical guidance without a protocol requirement.

## Acceptance

- All 25 tools receive an explicit review disposition in the ledger or commit notes.
- Every description is concise enough for an agent context and specific enough to guide choice.
- The trained identity projection is unchanged.
- No runtime result or browser behavior changed for a registry score.
- `cargo fmt --check`, strict workspace Clippy, and the complete workspace test suite pass in an
  isolated target directory.
- Both revision-specific tool-list tests and `tests/tool_schema_fidelity.rs` pass.

## Boundaries

Description-only changes still deserve review because they influence agent behavior. Do not make a
mechanical rewrite across every string. If a description is already excellent, record `keep`.
