# ADR-0127: Integration switches and foreign-entry evidence

- Status: Accepted
- Date: 2026-08-16
- Supersedes: ADR-0125 Decision 2 (the product-card roster and its status taxonomy). ADR-0125's
  registry, ownership, safety, artwork, and Locate decisions stand unchanged.
- Builds on: ADR-0126 Decision 9 (lead with the answer)

## Context

The MCP integrations destination rendered eighteen product cards in a four-category taxonomy:
Ready, Available, Needs Attention, Not Detected. Two rounds of compaction reduced its height without
changing what it was, and it kept reading as an inventory rather than an answer.

Three things were wrong underneath the layout.

The screen was built for a visit that rarely happens. `ghostlight install` already registers every
detected client, so on the common visit the honest content is that everything is connected and there
is nothing to do. Eighteen cards said that at length.

The status taxonomy invented a distinction people do not have. Connected and available are not two
kinds of thing; they are one thing in two positions. Making them separate categories also meant a
row moved to another part of the page when someone acted on it, on a surface that re-renders from
sequenced snapshots.

A blocked target asserted a conclusion with no evidence. "The configuration is malformed or has a
foreign ghostlight entry" told a person that something was wrong under Ghostlight's own key and gave
them nothing to look at, nothing to compare, and no way to judge whether to intervene. ADR-0125
Decision 2 and the reference-experience epic both required showing what Ghostlight found, what it
owns, and what it would change. A card had nowhere to put it, so it was never built.

## Decision

### 1. One switch per client

The operation is binary: this agent can drive the browser, or it cannot. The control is a switch and
the switch position is the status, so no status word is printed beside it and the four-category
taxonomy is gone.

The list is alphabetical over every client present on the machine. It does not reorder when someone
acts, which removes a class of disorientation the status grouping created rather than mitigating it.

A switch is a real `role="switch"` control with `aria-checked` and an accessible name. Its state is
carried by text as well as position and color, so it survives high contrast and does not rest on
color alone.

### 2. The page leads with what Ghostlight already did

The first line answers the question the destination exists for, in the shape ADR-0126 Decision 9
established for the front door: who needs you when something does, and otherwise how many agents can
drive the browser. The connected clients' own artwork appears beside it. Finding and wiring every
agent on a machine is the product's work, and the page says so before it lists anything.

### 3. Two states are not switches, and get their own shapes

A blocked target cannot be switched on, so it is a card carrying evidence. A product that is not
installed on this computer cannot be switched at all, so it folds away as discovery rather than
occupying the status surface.

Because those are the only exceptions, a card on this page now means something.

### 4. A foreign entry shows its evidence, and is still never overwritten

When something under Ghostlight's key was written by something else, the surface shows the command
it found, the command Ghostlight would write instead, the exact file, and a plain statement that
Ghostlight changed nothing.

The registration inspection carries that found command through as bounded, whitespace-normalized
text, capped at 200 characters. Nothing else from the document travels: the rest of the file is the
owner's business, and the entry under Ghostlight's own key is the narrowest disclosure that lets a
person decide.

The available actions are the ones that write nothing: open the file, or copy what Ghostlight would
have written. An automatic overwrite is offered only where the installer already permits it, which
for a foreign entry it does not. ADR-0125's ownership rule is unchanged and is precisely why showing
the evidence is safe.

### 5. An updatable client keeps an explicit update

A client registered through an older installation reads as on, because it is registered. The switch
must not hide that it points at a stale executable, so the row also carries a note and an explicit
Update beside the switch.

## Consequences

The per-card Install, Locate, Copy setup, and Copy MCP command row is gone. Locate and Copy setup
belong to the two exception shapes that need them; the connector path is one string for every client
and is stated once for the page.

The switch leans on a guarantee the red `Remove` button only implied: that removal is
ownership-checked and byte-identical on repeat. That guarantee is already proven by the installer
tests, but those tests are now load-bearing for the interface as well as for the installer.

What ADR-0125 decided about substance is untouched and still governs: one fixed registry of 18
products and 21 concrete targets, ownership checks before any write, preservation of unrelated
configuration, packaged offline artwork, the bounded native picker, and the refusal to expose a
generic file editor or command runner.
