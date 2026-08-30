# ADR-0146: One action sets up every detected harness

- Status: Accepted
- Date: 2026-08-30
- Amends: ADR-0125 Decision 2 and ADR-0129 Decision 1
- Builds on: ADR-0117 and ADR-0135

## Context

Ghostlight can detect and safely maintain a fixed roster of MCP client registrations. The
workbench exposes that authority one product card at a time. On a machine with many clients, the
person still has to repeat the same safe Set up action across the roster and remember which cards
need it.

The requested job is simpler: make Ghostlight work everywhere the person already codes. This is
not a request to install third-party products, overwrite uncertain configuration, or replace the
card roster. It is one explicit use of Ghostlight's existing per-target ownership rules.

While building the aggregate action, the detector also exposed a false-positive seam. A config
file directly under a generic home or configuration root made that root's existence look like
product evidence. An aggregate action makes such over-detection more consequential, so it must be
closed at detection rather than handled as a special case in the button.

## Decision

### 1. Integrations has one `Set up everything` action

The integrations heading carries one primary `Set up everything` action beside `Re-check`. It is
enabled when at least one detected target is `Available` or `Updatable`.

The card roster, category order, individual actions, evidence, and manual routes remain unchanged.
This amends the per-card-only consequence of ADR-0129 without reopening the rejected roster shapes.

### 2. Aggregate setup uses the existing target writers

One orchestrator application-service call snapshots the current roster under the harness mutation
lock. It applies the existing install operation to every target that was `Available` or
`Updatable`, then refreshes the roster.

The action:

- adds a missing Ghostlight registration only for a detected product;
- updates only an entry already identified as Ghostlight-owned;
- skips current and not-detected targets;
- leaves malformed and foreign entries untouched under the existing ownership rule;
- preserves the existing per-target backup and lossless editing behavior; and
- continues across independent environmental failures, returning a bounded failure per target.

It does not download or install a third-party product.

### 3. The result is one typed aggregate outcome

The orchestrator returns counts for registrations added, owned registrations updated, targets that
still need attention, and independent failures, plus one product-authored summary sentence. The
WebView only invokes the command, refreshes its snapshot, and renders that result.

### 4. Generic roots are not detection evidence

A product-specific config directory remains useful detection evidence. The generic home,
configuration, or roaming root does not. A file at such a root is evidence only when that file
exists, or when Ghostlight finds the product executable through another supported detection path.

## Consequences

A person with several supported clients gets one deliberate setup step and can inspect every
result on the same cards. A blocked target cannot prevent independent safe registrations, but it
remains visible and contributes to the aggregate attention count.

The desktop adapter gains one closed command and no generic filesystem authority. The MCP
connector, browser connector, bridge, and extension do not change.

The workbench journey must prove the button reaches the aggregate orchestrator command. Unit tests
must prove safe add, owned update, blocked-target preservation, idempotence, and that a generic home
directory alone cannot make a product eligible.
