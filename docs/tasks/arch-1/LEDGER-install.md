# arch-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

I1 and I3 have landed. ux-2 has closed, so I7, I9a, I9b, I4, I5, I6, I8, I10, I12 and I13 can start,
and I2 can start now that I1 is in. I11 is held for the owner's question 1 (what the plugin
registers) and the live check on a test machine.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| I1 | fix(install): one owner-private creator for secrets and records | candidate | none | see Notes | landed (this commit) |
| I3 | fix(install): Ghostlight owns the command, and the client keeps its own keys | candidate | ux-2 closes (`install/mod.rs`) | see Notes | landed (this commit) |
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

## Notes

- I1, harness locations: they are written through `record::atomic_replace`, not the client-config
  writer, so every write is owner-only and the file gets no `.ghostlight-backup` copy beside it.
  On Windows that copy would take the directory's access. A locations file written before this
  commit becomes owner-only at its next write, as `install.json` does; nothing is re-protected at
  start. Not a changed decision.
- I1, the Windows proof: the orchestrator forbids `unsafe`, so the tests read the access list with
  the system's `icacls /save` and the user's SID with `whoami /user`. win-peer gains no new call.
- I1, for C19b: `install::record::create_private` is now public, for the audit sink's new log.
- I1 changes no golden and needs no journey. `linux.txt` is unaffected.
- I3, the owned `type`: the one current predicate checks the dialect's `type` with the command
  and its arguments, so an entry with the right command and no `type` (or another one) is now
  updatable where it was installed, and an install sets only that key. A `type` in a dialect that
  needs none is the client's. Not a changed decision: Decision 10.1 names `type` an owned key.
- I3, the written bytes: an owned entry is updated key by key, so only owned keys that differ are
  set and missing owned keys and defaults are appended. A one-line stale entry stays on one line;
  a one-line entry that gains a key (`type`, `env`, `enabled`) is laid out one key per line, in its
  own order, not the alphabetical order of a whole new entry. The writer golden pins each case. A
  current entry spelled another way (case, `.` or a link to the same edge) is now left alone,
  because the install reads inspection's path rule.
- I3, the Codex writer already edited its entry in place; only its equality no-op went.
- I3, an extra proof: `a_dialect_that_needs_a_type_is_updatable_without_it_and_the_update_keeps_the_rest`.
- I3 changes no native golden: they seed no Ghostlight entry, so `linux.txt` is unaffected. It
  needs no journey.
