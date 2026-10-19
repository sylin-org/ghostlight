# arch-1: docs ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

D0 and D1 have landed. Next: D2, which writes the contract lines for each slice after its code
lands. D1's lines that wait on the owner's two questions are listed under Notes.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| D0 | docs(adr): one owner for each fact | candidate | none (commit 2: ux-2 closes) | - | landed (3729f9a1 and this commit) |
| D1 | docs(release): the release candidate names the plugin, and LANGUAGE names where a selector works | candidate | D0; ux-2 closes (`docs/1.0/LANGUAGE.md`) | see Notes | landed (this commit) |
| D2 | docs(contracts): the contracts state what landed | as the code it follows | each landed slice; ux-2 closes (`docs/1.0/`) | - | todo |

## Notes

- D1, question 2: the owner has not answered when the candidate is tagged, so RELEASE.md and the
  store disclosure keep option A's one candidate, the ADR's recommendation. If the owner picks
  option B, a later docs commit makes both say the cutover comes from the last rc.
- D1, question 1: unanswered, so the notes carry the decline path's line only: plugin users
  disable the plugin until the cutover (the 2.0 notes: until npm serves 2.0.0). I11 adds "the
  plugin or `ghostlight install`, not both" under option A, or the skill-only plugin under option B.
- D1, the plugin step: the ADR places it "before cutover step 2". It sits under Candidate gates,
  before the candidate commit is chosen, because nothing lands between the rc tag and `v2.0.0`;
  cutover step 2 says it publishes the plugin at that version. Not a changed decision.
- D1, for the coordinator: the 2.0 notes copied inside `notes-v2.0.0-rc.1.md` still list the
  channel enable switches as removed, while `notes-v2.0.0.md` says a policy can turn a channel off
  (974a5ed2 kept the rc.1 copy "as history", but rc.1 is not published). D1 leaves it as is.
- D1, for the install lane: `packaging/plugin/ghostlight/README.md` still states the precedence
  claim the ADR-0144 amendment withdraws. I9a removes it there.
