# ADR-0129: The integration roster returns to product cards

- Status: Accepted; Decision 1 amended 2026-10-17 (see the amendment at the end)
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

## Amendment (2026-10-17): the 2.0 window shows the roster as master and detail

The owner reviewed the 2.0 window on 2026-10-17, saw six working directions for this destination,
and chose master and detail with five grafts (the decision is recorded as decision 5 of
[docs/tasks/break-2.0/design.md](../tasks/break-2.0/design.md)). Decision 1 of this record no
longer governs the 2.0 window; Decisions 2 to 4 stand as history.

- A rail lists every agent, grouped by state (Needs a look, Connected, Found, not connected, Found,
  set up by hand, Not on this computer), with a state dot per agent and a second line only for what
  is that agent's own. Ghostlight never reads a copy-only agent's file, so one found here is
  connected only while one of its sessions is live, and otherwise is set up by hand: the page
  claims nothing about its file and leaves it out of the headline's count.
  The agents not on this computer fold into one row that opens in place and that search reaches.
- The pane shows the one agent picked: one sentence for it, each button with its effect written
  beside it, the settings file, Ghostlight's entry shown whole, the service's evidence and numbered
  fix steps for an agent that needs attention, and Remove last as a quiet link that asks first and
  names the live sessions. The page opens on the most relevant agent and keeps the pick by id, so
  an agent that changes group when someone acts on it stays in the pane, which answers the moving
  row named in Decision 3.
- One header row holds the computed headline, Re-check, Set up everything with a caption that says
  beforehand what it will do, and one Copy MCP command with the command shown.
- The foreign-entry evidence named in Decision 4 is rendered verbatim (ADR-0135), and the roster
  says which agents are copy-only (`copy_only`) so the page does not guess it.
