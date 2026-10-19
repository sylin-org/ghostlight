# arch-1: policy ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

P10 and P1 landed. Next: P2.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| P10 | feat(governance): the runtime state names the acts it accepts | candidate | ux-2 closes | - | landed (570528ad, 4476fda8) |
| P1 | fix(governance): one evaluator says whether a capability can be used anywhere | candidate | ux-2 closes | see Notes | landed (this commit) |
| P2 | fix(governance): a never-touch refusal names the layer that wrote it | candidate | ux-2 closes | - | todo |
| P3 | fix(governance): one replay of recorded work | candidate | ux-2 closes | - | todo |
| P4 | fix(governance): one organization identity per publication | candidate | ux-2 closes | - | todo |
| P9 | fix(governance): an owned user policy moved at load stays owner-only | candidate | ux-2 closes | - | todo |
| P5 | refactor(governance): one publication is read whole | none | ux-2 closes | - | todo |
| P6 | refactor(governance): the unsigned managed file path goes | none | ux-2 closes | cross-lane core and surface call sites | todo |
| P7 | refactor(governance): one settings vocabulary | none | ux-2 closes | - | todo |
| P8 | refactor(language): the policy sentences live in language/policy.rs | none | ux-2 closes | cross-lane `language/mod.rs` | todo |

## Notes

- P10: `offers` names only the four acts. `ToggleHold` is a request, not an act: `apply_intent`
  turns it into Hold while active and Resume otherwise, then the table decides. One behavior
  changes on purpose: a Start session sent while paused no longer resumes, because a paused state
  does not offer it (the window's own table already said so). `apply_intent` now reads and changes
  the state under the observer lock, so a Resume that races an End session cannot undo it.
  `stop_is_terminal_and_idempotent` is renamed `stop_is_still_terminal` and also covers Hold and
  the toggle; `every_state_changes_only_through_an_act_it_offers` pins the whole table. S1 is
  unblocked.
- P1: `AuthoritySnapshot::reach` decides every probe host through `decide_landing`, so never-touch
  hosts and the window's own origin count. The probes are every exact pattern, one fresh label
  under each suffix, and that label alone; the label has no dot and is renumbered until no probe it
  builds is an exact pattern. A probe a ceiling refuses counts neither as admitted nor as narrowed
  by a layer, so a never-touch host stays a ceiling line and does not turn "Available" into "Some
  blocked" (P2 credits its layer). Three departures in shape, none in decision: `could_admit` asks
  the same probes and stops at the first admitted one instead of computing the whole reach, which
  is equal by construction and pinned by the agreement test; an unavailable line now names a layer
  that admits the capability nowhere on its own, not the first decider, which could be a layer
  that allows it somewhere; and the disjoint sentence ("The sites X and you allow do not overlap.")
  sits in `effective.rs` beside the others until P8 moves them. `note()`'s "no effect" check reads
  the snapshot over the rule's hosts, which also stops a wide rule under a narrow organization
  grant being marked inert. Dropping `browser_screenshot` when screenshots are off is C10c's. The
  catalog and result goldens are unchanged.
