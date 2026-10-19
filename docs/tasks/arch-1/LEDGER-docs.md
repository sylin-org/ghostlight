# arch-1: docs ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

D0, D1 and D2's first commit have landed. D2's first commit writes the contract lines of every
slice landed by `e2752439`. The slices still open in other lanes (C3b, C3c, C12b, C10c, C16, C17,
C18, C19b, C19c, C20, S3 to S8 and I11) get their lines in a later D2 commit, after their code;
Notes lists the lines each one owes. D1's lines that wait on the owner's two questions are listed
under Notes too.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| D0 | docs(adr): one owner for each fact | candidate | none (commit 2: ux-2 closes) | - | landed (3729f9a1 and this commit) |
| D1 | docs(release): the release candidate names the plugin, and LANGUAGE names where a selector works | candidate | D0; ux-2 closes (`docs/1.0/LANGUAGE.md`) | see Notes | landed (this commit) |
| D2 | docs(contracts): the contracts state what landed | as the code it follows | each landed slice; ux-2 closes (`docs/1.0/`) | first commit covers every slice landed by `e2752439`; see Notes | landed (this commit) |

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
- D2, what the first commit covers: A1 to A4, B1, B2, C1 to C7, C9, C10a, C10b, C11, C12a, C13,
  C19a, C21, C22, I1 to I10, I12, I13, P1 to P10, S1, S2 and S4a. Each claim was read against the
  tree at `e2752439` before it was written: the sentences against `language/outcome.rs` and
  `language/provenance.rs`, the list facts against `work/navigation.rs`, the service end against
  `service/mod.rs`, the hello against `protocol4/wire.rs` and `extension/worker.js`, the audit
  shapes against `tests/golden/audit/`, the never-touch credit against the policy ledger, and the
  Windows file access against `install/record.rs`.
- D2, lines owed by slices still open, written only after their code: ARCHITECTURE's Work
  chokepoints without `CompletionGate`, and the SIEM guide's `allowed` wording checked against the
  doc comments (C19c); the recording binding as a workspace fact (C16); the SIEM guide's optional
  `browser_id`, install-local and random, a new audit log owner-only on Windows and how an
  administrator grants a collector account read access (C19b, which also adds the audit log to
  data-flows' Windows line); the adapter-protocol-4 design's dropped `presentationLabel` (C18); the
  catalog dropping `browser_screenshot` when screenshots are off (C10c); a pinned wait that reads
  its own browser in ARCHITECTURE's waiting paragraph (C3b); the window's `window_failed` (S8).
- D2, beyond the named ACCEPTANCE items: Browser job journeys gain item 26 (a browser restart never
  hands an agent's handle the person's tab, which proves README's promise) and item 27 (a replace
  keeps, a quit releases, a call during a quit gets a tool result), and Extension product gates
  item 3 names the hello's `life`. Each restates a proof the core or package ledger already names.
  Not a changed decision.
- D2, the diagnostics ring: the package fix for unbound tabs (LEDGER-package.md) has landed, so
  ARCHITECTURE and ACCEPTANCE item 20 now say every tab's ring starts fresh at a top-frame
  navigation, bound or not, and an embedded document's navigation never clears it.
- D2, Ghostlight's own ceiling: the docs name non-HTTP(S) schemes and the window's own address,
  not localhost, loopback and link-local as Decision 7.2's text does, because ADR-0155 removed
  those ceilings and P2 did not restore them (LEDGER-policy.md, P2). The trust and governance docs
  already said local destinations follow policy. ADR-0203's amendment of 2026-10-19 now says so
  too, and corrects its ADR-0122 line.
- D2, Windows file access: data-flows names the policy keygen seeds, the MCP client locations file
  and the owned user policy. The spec's "a file that already existed keeps its access" is written
  as "keeps its access until Ghostlight next writes it", because those files are replaced through
  `atomic_replace`, which writes the new file owner-only (LEDGER-install.md, I1). The audit log is
  C19b's and is not named.
- D2, data-flows' correction paragraph: C1 found the ownership leak in 1.3.14 (`v1.3.15` was
  withdrawn unreleased, STATUS), so the paragraph names 1.3.14 and earlier releases, and says the
  fill and dialog leak never shipped in a release.
- D2, also edited: ADR-0186's header gains the ADR-0203 pointer D0 left for when B2 landed;
  `docs/guides/compliance-team.md`'s "Pause/Stop"; the adapter-protocol-4 design loses
  `opener_tab_id` (B2's note) and gains `life` (B1), the epoch release and the outdated-extension
  handling (B2); ARCHITECTURE's older-layout line (I10's note) and its hello sentence, which no
  longer lists capability declarations the protocol-4 hello does not carry.
- D2, "Pause/Stop" kept: `docs/design/security-assessment-2026-09-06.md` and
  `docs/design/tool-surface-review-2026-09-07.md` are dated reviews that quote the vocabulary of
  their day, so they keep it.
- D2, Fringe stability: ARCHITECTURE said adapter behavior is selected by advertised capabilities
  and that hello capability revisions derive from the handler table. The protocol-4 hello carries
  neither (`protocol4/wire.rs` `Hello`, the worker's hello), so the paragraph now says what
  protocol 4 does: the hello names the protocol major, no capability list crosses, and the
  protocol stays compatible by staying additive with tolerant readers and `unsupported` for an
  unknown primitive.

### Follow-up (2026-10-19)

- F7-no-rc1-names, the owner's word that there is no rc.1 tag: the release candidate is the
  release workflow's bundle, and the install is proved on it with `GHOSTLIGHT_FROM` (RELEASE.md's
  cutover steps 1 to 3 already said so, linux-claude, 422e4696). ADR-0200 gains a marked amendment
  that supersedes rule 2 of its prerelease amendment, and the ADR index says so. The store
  disclosure's release order builds and proves the bundle instead of publishing `v2.0.0-rc.1`,
  and its cutover publishes `v2.0.0` with `docs/release/notes-v2.0.0.md`. The rc notes are removed
  rather than renamed: `notes-v2.0.0.md` already is the 2.0 notes and the newer text (D1's note on
  the channel switches), and the preamble only told people how to install the prerelease. No
  active document linked the removed file except the store disclosure and STATUS. STATUS's
  release order and checklist say bundle, not rc.1, and lose the owner-questions line, since
  question 2 is answered here and question 1 by the install lane's F2. ROADMAP says the same.
  RELEASE.md's description of `-Prerelease` keeps the option with a generic example and says 2.0
  publishes none. New owner item in STATUS and the store disclosure: with no prerelease, a store
  reviewer who installs Ghostlight gets 1.x, so the Notes for certification say that and no more
  until the owner decides what a reviewer is offered. Historical records (task ledgers, the
  break-2.0 journey, coordination) keep their rc.1 lines, and code and test fixtures keep
  `2.0.0-rc.1` as a version-format example.
