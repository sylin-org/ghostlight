# arch-1: install ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

I1, I3, I7, I9a and I9b have landed. ux-2 has closed, so I4, I5, I6, I8, I10, I12 and I13 can start,
and I2 can start now that I1 is in. I11 is held for the owner's question 1 (what the plugin
registers) and the live check on a test machine.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| I1 | fix(install): one owner-private creator for secrets and records | candidate | none | see Notes | landed (this commit) |
| I3 | fix(install): Ghostlight owns the command, and the client keeps its own keys | candidate | ux-2 closes (`install/mod.rs`) | see Notes | landed (this commit) |
| I7 | fix(deployment): a same-version install restores a missing shell | candidate | none | see Notes | landed (this commit) |
| I9a | docs(plugin): the plugin README withdraws the precedence claim | candidate | none | see Notes | landed (this commit) |
| I9b | fix(plugin): the skill says only what the catalog cannot | candidate | none | see Notes | landed (this commit) |
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
- I7, the launchers: they check the shell only when the installed record is this package's own
  version. A newer installed set whose shell is missing still runs as it is, because the service
  refuses to install an older payload over it; installing there would turn a missing window into
  a launch that fails. A changed shell is not seen by the launchers, which never hash the
  installed set; the service's same-version install finds it. Not a changed decision.
- I7, the service: `shell_changes` is read once per plan and passed to `place_in_bin`, so a
  same-version plan with a missing or changed shell takes the place-in-bin arm and restarts, as a
  damaged service image does. An extra proof covers the second `Same` arm (a retired development
  directory left to remove): `a_same_version_install_with_a_missing_shell_and_a_migration_restores_the_shell`.
- I7 changes no golden. It needs no new journey: the installer journeys on a test machine cover
  the place-in-bin path it reuses. `linux.txt` is unaffected.
- I9a, the double registration: the README's client note now says only that Claude Code then holds
  two registrations named `ghostlight` and that which one it starts is unverified. I11 replaces
  that line with question 1's answer and the live check. Not a changed decision.
- I9a, the network sentence: "the only network traffic is the binary download" was false too, since
  `npx` asks the npm registry at every launch. The README now says so and points to the npm
  launcher row of `docs/trust/data-flows.md` for when the launcher downloads. Within the slice's
  Changes.
- I9a changes no code, golden or journey. `linux.txt` is unaffected.
- I9b, the false statements: the skill no longer says a document inspect returns a `snapshot_`
  handle (it returns no handle) or that navigating through a closed tab handle recreates the tab,
  and it drops the retired "session hold" and "Pause/Stop" (G4-02): a credential refusal now says
  nothing is held or paused. Its result-field list is gone; it speaks of what a result says
  instead.
- I9b, the hold sentence: the skill says a refused request does not pause the session. It drops
  "later permitted requests remain available", because a tab held after a refused landing refuses
  them until the hold clears (C6). Not a changed decision: Decision 5.5 keeps "do not ask the user
  to resume".
- I9b, the plugin README: its last sentence said the skill is kept in lockstep with LANGUAGE.md. It
  now says the skill restates nothing of the catalog. Within the slice.
- I9b, for C20: the body reads clean through a scratch port of `hint_violations` against
  `docs/1.0/catalog.json`. Tool names are written bare and handle prefixes unquoted, as the catalog
  writes them; the one backticked word is `user_authorized_credentials`. The YAML front matter
  (`name:`, `description:`) is not a hint, so C20 reads the body after it.
- I9b changes no code, golden or journey, and no plugin version (D1's RELEASE.md step bumps it).
  `linux.txt` is unaffected.
