# ADR-0128: Integration master and detail

- Status: Superseded in full by ADR-0129 on 2026-08-16. Retained as history; does not govern.
- Date: 2026-08-16
- Supersedes: ADR-0127 Decisions 1, 2, 3, and 5 (the switch roster). ADR-0127 Decision 4, the
  foreign-entry evidence, is carried forward unchanged and is the substance of this surface.
- Builds on: ADR-0125 (registry, ownership, artwork, Locate), ADR-0126 Decision 9

## Context

The integrations destination has now been through four shapes: a product-card grid, compact rows,
two-line rows, and one switch per client. Each fixed a real defect in the one before it and each was
rejected by the owner on the same grounds -- the surface read as a dense control strip rather than a
place to understand and manage something.

The switch model in ADR-0127 was the most compressed of the four. It removed the status taxonomy and
the reordering problem, and it was rejected on sight. That is a legitimate answer: a switch makes
every client equally weightless, which is efficient and gives a person nothing to look at.

The evidence work from ADR-0127 Decision 4 was not the problem. It was the only part of that ADR
with something to read, and it had nowhere good to sit in a row of switches.

## Decision

### 1. The destination is a master and a detail

A list of clients on the left, one pane describing the selected client on the right. The list is for
finding, so a row carries identity and a state mark and nothing else. The pane is for understanding
and acting, so every operation lives there beside the facts that justify it.

Below 900 pixels the two columns stack, so a narrow window keeps both.

### 2. The list groups by state; the pane names it in words

List rows carry a state mark, never a word, and the mark is never the only carrier: the pane states
the client's state and each target's state in plain language. Groups are ordered by what a person can
act on, and names are alphabetical inside a group.

### 3. Selection is view state, kept by id

This surface redraws from every sequenced snapshot. Selection therefore lives in the view rather
than in the DOM and is re-applied by product id, so a selection cannot silently reset while someone
is reading. If the selected product disappears from the registry, the selection falls back rather
than emptying the pane.

### 4. The pane opens on what needs a person, and a foreign entry outranks a stale path

With nothing selected, the pane opens on a client with a genuinely foreign entry, then on any client
needing attention, then on the first client. A foreign entry is someone else's file that Ghostlight
refused to touch; a stale path is a version number. They are not equally urgent.

A foreign entry anywhere in a product earns that landing even when another target of the same
product is connected.

### 5. Every target gets a block

A concrete target's block names the target, its state in words, its sentence, its exact file, what
Ghostlight writes there, and the operations available on it. A blocked target additionally shows the
command found under Ghostlight's key and the command Ghostlight would write instead, per ADR-0127
Decision 4, with only the routes that write nothing.

The connector path is one string for every client and is stated once in the pane.

## Consequences

Bulk setup costs a selection per client, which the switch model did not. That is the price of a
surface that shows one thing properly, and it is paid on the rare visit rather than the common one:
`ghostlight install` already registers every detected client, so the common visit changes nothing.

Four rejected shapes are recorded in ADR-0125, ADR-0127, and this file. The pattern across them is
that compaction was never the problem being solved. Each round removed repetition and the surface
still read wrong, because the destination needed somewhere to explain one client rather than a
denser way to list eighteen.
