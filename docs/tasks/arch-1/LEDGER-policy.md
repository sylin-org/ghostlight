# arch-1: policy ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. The lane starts after ux-2 closes, with P10 and P1.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| P10 | feat(governance): the runtime state names the acts it accepts | candidate | ux-2 closes | - | todo |
| P1 | fix(governance): one evaluator says whether a capability can be used anywhere | candidate | ux-2 closes | - | todo |
| P2 | fix(governance): a never-touch refusal names the layer that wrote it | candidate | ux-2 closes | - | todo |
| P3 | fix(governance): one replay of recorded work | candidate | ux-2 closes | - | todo |
| P4 | fix(governance): one organization identity per publication | candidate | ux-2 closes | - | todo |
| P9 | fix(governance): an owned user policy moved at load stays owner-only | candidate | ux-2 closes | - | todo |
| P5 | refactor(governance): one publication is read whole | none | ux-2 closes | - | todo |
| P6 | refactor(governance): the unsigned managed file path goes | none | ux-2 closes | cross-lane core and surface call sites | todo |
| P7 | refactor(governance): one settings vocabulary | none | ux-2 closes | - | todo |
| P8 | refactor(language): the policy sentences live in language/policy.rs | none | ux-2 closes | cross-lane `language/mod.rs` | todo |
