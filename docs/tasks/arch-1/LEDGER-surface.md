# arch-1: surface ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

S1 landed. Next: S2, which waits for C12a; S8 and S7 can start now.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | fix(tray): every surface offers the acts the runtime state accepts | candidate | P10 | - | landed (this commit) |
| S2 | fix(workbench): history rows and counts say what the service decided | candidate | C12a | - | todo |
| S4a | fix(workbench): the Policy page previews and names what governance decides | candidate | P3, P4 | - | todo |
| S8 | fix(window): a window that cannot be built says so | none | ux-2 closes | - | todo |
| S6 | fix(setup): setup starts browsers the way recovery does, and marks the right host in use | none | C3c, C13 | - | todo |
| S3 | refactor(workbench): every part comes from one gather | none | P5, I6, C2 | - | todo |
| S4b | refactor(workbench): the Policy page's words come from governance | none | P7, P8 | - | todo |
| S5 | refactor(workbench): the window's words come from language/ and are pinned | none | C18, C12b | - | todo |
| S7 | test(desktop): the shell decodes and re-encodes every desktop golden | none | ux-2 closes | - | todo |

## Notes

- S1: ux-2's 77840ccc had already relabeled the tray's Resume entry "Start a new session" for an
  ended session, with the StartSession intent, and corrected the doc the ADR names at
  `intake/desktop.rs:383-384`. So the entry stays enabled once the session ended: what it asks for
  is offered. `TrayModel::of` now enables a runtime entry exactly when `governance::offers` names
  its act, so Resume is enabled only while paused. The test the ADR names replaces
  `an_ended_tray_offers_a_new_session_but_not_pause`.
- S1: the controls part carries `runtime.offers`. Settings shows a runtime button when its act is
  offered and the context may do it; the band's wheel does the first offered act. Before any
  controls part has arrived, the band shows neither the wheel nor the Continue line.
- S1, cross-lane: the fake controls parts in core's `tests/workbench-surface.mjs` and
  `tests/workbench-preview-server.mjs` gain `offers`, and the status-push check in
  `workbench-surface.mjs` also pushes the controls part, as the service does. Nothing else in
  those files changes.
