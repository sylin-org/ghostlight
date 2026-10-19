# arch-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

Opened by D0. Nothing has started. I1, I7, I9a and I9b can start at once. I3, I4, I6 and I10 wait
for ux-2 (`install/mod.rs`). I11 is held for the owner's question 1 (what the plugin registers) and
the live check on a test machine.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| I1 | fix(install): one owner-private creator for secrets and records | candidate | none | - | todo |
| I3 | fix(install): Ghostlight owns the command, and the client keeps its own keys | candidate | ux-2 closes (`install/mod.rs`) | - | todo |
| I7 | fix(deployment): a same-version install restores a missing shell | candidate | none | - | todo |
| I9a | docs(plugin): the plugin README withdraws the precedence claim | candidate | none | - | todo |
| I9b | fix(plugin): the skill says only what the catalog cannot | candidate | none | - | todo |
| I11 | fix(plugin): the plugin registers what question 1 decides | candidate | owner question 1 (the plugin) and the live check | held for the owner | todo (held for the owner's question 1) |
| I4 | fix(install): one rule says Ghostlight has an entry to remove | none | ux-2 closes (`install/mod.rs`) | - | todo |
| I5 | fix(install): Ghostlight's npm launcher entry is Ghostlight's | none | none | - | todo |
| I2 | fix(install): one atomic replace and one same-location rule | none | I1 | - | todo |
| I6 | fix(install): the roster cache follows the acts, and a command reads it once | none | ux-2 closes (`install/mod.rs`) | - | todo |
| I8 | fix(packaging): the carriers agree with the service's rule and words | none | none | cross-lane the installer journeys | todo |
| I10 | refactor(install): install words in language/deployment, and the browser list composed once | none | ux-2 closes (`install/mod.rs`) | cross-lane `language/outcome.rs` | todo |
| I12 | refactor(diagnostics): one spelling of the edge's logs, and no name nobody writes | none | none | - | todo |
| I13 | refactor(win-peer): one token module for its two callers | none | none | - | todo |
