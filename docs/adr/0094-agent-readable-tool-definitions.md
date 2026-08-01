# ADR-0094: Agent-readable tool definitions

Status: Accepted

Date: 2026-08-01

Amends: ADR-0007, ADR-0031 Decisions 3 and 5, ADR-0034 Decision 7, and ADR-0050 Decision 6

## Context

Ghostlight's tool definitions are both a compatibility surface and the instructions an agent uses
to choose its next action. Earlier decisions treated the complete trained definition, including
description prose, as byte-frozen. ADR-0034 later deprecated that freeze and ADR-0050 explicitly
allowed description-only improvements, but the agent guide and fidelity tests still enforced the
older rule.

That stale rule preserved avoidable defects. Descriptions named `tabs_context` and `tabs_create`
even though the callable tools are `tabs_context_mcp` and `tabs_create_mcp`. Several overlapping
tools described their own mechanics without saying when to choose their sibling. The broad
`computer` tool spent most of its description on click placement while leaving its side effects
and relationship to semantic tools implicit.

MCP also defines standard tool annotations for display title, read-only behavior, destructive
behavior, idempotency, and open-world interaction. Ghostlight already has stronger per-action
classification and enforcement, but did not publish these client-facing hints.

## Decision

### 1. Preserve identity; improve guidance

The compatibility boundary is each trained tool's name, parameter names, parameter types, enum
values, and ordering. Those stay stable. Optional growth remains additive.

Tool and parameter descriptions are guidance. They may change deliberately when the new text is
more accurate, more concise, or helps an agent choose and recover correctly. Guidance names the
actual callable tool, including the `_mcp` suffix. The fidelity suite remains a regression
snapshot, not a prohibition on better prose.

This pass makes four focused distinctions:

- `computer` is the low-level coordinate and screenshot tool; `act_on` is the semantic one-target
  tool, and form tools own field entry.
- `script` is for dependent steps that consume earlier structured results; `browser_batch` is for
  steps whose inputs are known before the call.
- `form_fill` matches several fields by meaning; `form_input` sets one already-resolved ref.
- `file_upload` accepts client-supplied bytes; `upload_image` uses a screenshot already captured by
  Ghostlight.

`tabs_create_mcp` also states that it is the explicit recovery action for an unavailable workspace,
matching ADR-0090's runtime behavior.

### 2. Publish standard MCP annotations from the registry

`ToolDescriptor` owns a typed annotation record next to the description and schemas. The
`tools/list` renderer emits `annotations` for every tool. The hints are advisory metadata only.
They never replace or influence Ghostlight's per-action capability classification, authorization,
or audit.

The mapping is conservative:

- `readOnlyHint` is true only when every call observes without changing browser or page state.
- `destructiveHint` is true when any supported call may overwrite, discard, close, submit, upload,
  execute, or compose such an action.
- `idempotentHint` is true only when repeating the same call adds no further effect.
- `openWorldHint` is true for page content, page interaction, and tab metadata derived from open
  pages; blank-tab creation, local presentation, and policy explanation are closed-world.

Three details are intentionally explicit:

- `tabs_context_mcp` is not read-only because `createIfEmpty:true` may open a tab. It is idempotent
  because repeated calls do not create additional tabs once the workspace exists. Both tab tools
  are open-world because their results can carry titles and URLs from existing pages.
- `navigate` is potentially destructive because `force:true` can discard unsaved page state.
- `computer` spans read-only screenshots and mutating input. MCP applies `readOnlyHint:false` and
  `destructiveHint:true` when those fields are omitted, so Ghostlight publishes those conservative
  whole-tool values explicitly. Per-action governance still distinguishes screenshots from input.
- `update_plan` is closed-world and read-only. The compatibility handler only echoes an
  informational plan; it does not request approval or change domain permissions.

All titles and hint values are pinned in `tests/tool_schema_fidelity.rs`.

### 3. Do not optimize runtime responses for a registry score

Static registry analysis primarily sees `tools/list`. Runtime responses continue to follow
Ghostlight's existing contract: report the outcome, preserve structured results where declared,
and give one concrete next step on recoverable failure. Response changes require a product reason,
not a scoring reason.

External scores are diagnostics, not a product contract. No tool is renamed, duplicated, hidden,
or split to improve a directory grade. Tool count and historical naming are accepted when they
represent the honest product surface.

## Consequences

- Agents see shorter, more discriminating guidance and the exact names they can call.
- MCP clients can use standard annotations for display, confirmation, retry, and trust-boundary UX.
- The weakest broad tool describes both its preferred uses and its mutation risk without changing
  its trained input shape.
- Older clients can ignore the additive `annotations` object just as they ignore Ghostlight's
  additive `example` field.
- Registry grades may improve after the change reaches the default branch and is rescanned, but no
  grade is promised or used as a release gate.
