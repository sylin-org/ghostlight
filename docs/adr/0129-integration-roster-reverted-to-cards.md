# ADR-0129: The integration roster returns to product cards

- Status: Accepted
- Date: 2026-08-16
- Supersedes: ADR-0130 and ADR-0128 in full. ADR-0125's card roster is restored and governs again.
- Note: the switch roster was filed as ADR-0130 and renumbered to ADR-0130 on 2026-08-17 to clear a
  duplicate number. Only the references below changed; no decision here was reopened.

## Context

On 2026-08-16 the MCP integrations destination was redesigned five times in one session: product
cards, compact single-line rows, two-line rows, one switch per client, and a master-and-detail
split with two-line list rows. Each iteration removed a real defect the owner had named, and each
result was rejected.

The owner then reverted the destination to the card roster it started from.

That sequence is the reason this record exists. Without it the same five shapes are available to be
rediscovered by anyone who opens the surface and sees repetition.

## Decision

### 1. The card roster stands

`ADR-0125` Decision 2 governs the destination again: one product card per client, the four-category
taxonomy, per-card actions, and the compact status-sorted grid recorded in `STATUS.md`. The surface
files and their journey assertions were restored to that exact state.

### 2. ADR-0130 and ADR-0128 are superseded in full

Both are retained as history and neither governs. That includes ADR-0130 Decision 4, the
foreign-entry evidence: the projection field and its bound were reverted with the interface that
consumed them, because nothing else rendered them.

### 3. What the attempts established, for whoever tries next

Recorded as evidence rather than as an argument for another attempt:

- Repetition was measurable and real. Eighteen cards carried four distinct sentences, the status
  appeared twice per card in two vocabularies, and `Copy MCP command` appeared eighteen times
  copying a string that is identical for every client. Removing that repetition did not make the
  destination feel better, which means repetition was not the thing that made it feel wrong.
- Compression makes the most important row the least readable. On one line, a blocked client's name
  collapsed to an ellipsis while its explanation was cut mid-word.
- Grouping by status makes a row move when someone acts on it, on a surface that redraws from
  sequenced snapshots.
- A switch makes every client equally weightless.
- A master and detail gives a plain connected client with one target the widest space on the screen
  and the least to say in it.

### 4. The one gap that outlived every shape

`ADR-0125` Decision 2 and the reference-experience epic both require showing what Ghostlight found,
what it owns, and what it would change. That is still not implemented. A blocked target still
asserts that a configuration is malformed or foreign and shows no evidence for it.

It was built once, during the attempts above, and reverted with them. Whatever shape this
destination eventually takes, that gap is the substance, and the shape is not.

## Consequences

`STATUS.md` describes the destination as the compact status-sorted card roster, which is again
accurate.

The Rust projection returns to its previous shape. Re-landing the foreign-entry evidence is an
independent change and does not require a particular layout.
