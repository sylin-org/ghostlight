# arch-1: package ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). The specification of each slice is its entry in
[ADR-0203](../../adr/0203-one-owner-for-each-fact.md)'s Rebuild plan. Statuses: `todo`, `landed`
(with commits), `blocked` (with the reason).

## RESUME HERE

B2 has landed. B1 is next; it lands in the same candidate as C21, or neither does.

## Slices

In the ADR's order. "After" names what a slice waits for, including the ux-2-first rule.

| Slice | Subject | Gate | After | Note | Status |
| --- | --- | --- | --- | --- | --- |
| B2 | fix(extension): custody follows the service that granted it, and an outdated extension says so | candidate | ux-2 closes | package items 2, 3, 4 and 7; see Notes | landed (this commit) |
| B1 | feat(extension): the browser names its life | candidate | ux-2 closes | package item 1; lands with C21 | todo |

## Notes

- B2, cross-lane: the extension no longer reports `opener_tab_id`, so the slice also removes the
  field from `protocol4/wire.rs`'s `Tab` and from `tests/golden/protocol4/receipts/list_tabs.ok.json`
  (adapter lane). Nothing in the service read it. `docs/design/adapter-protocol-4.md` still names
  the field; D2 takes it out. A `list_tabs` that names tabs still lists the tabs they opened, as the
  wire's doc says; only the reported opener goes.
- B2, `omitted_count`: `allowNetworkUrl` goes, and the receipt keeps the field at 0, because the
  service adds what its own policy withholds to it.
- B2, the diagnostics ring: the reset at a navigation now needs the top frame. The reset at a
  changed binding stays, because the binding is the authority's say (A2 sends one only when it
  watches documents).
- B2, the snapshot fields nothing read: `adapter_version`, `browser_id`, `service_version`,
  `unseen_denials`, `last_error` and `service_outdated` leave the popup snapshot, and with
  `last_error` the worker's own "Update Ghostlight on this computer." words. The snapshot gains
  `adapter_refusal`, the sentence the service sent with `adapter_incompatible`. The popup and the
  options page show it under "Update Ghostlight in Browser.", and the setup page shows it as the
  lead, so the update sentence keeps one owner, `language::adapter::ADAPTER_INCOMPATIBLE`.
  With `last_error` gone nothing shows a raw connection error, so the options page now says only
  that Developer diagnostics keeps connection details for this browser (review fixup).
- B2, the retry: Decision 3 says the extension stops restarting the native host until a hello is
  accepted; the Rebuild plan says it takes the delayed retry. The slice follows the plan: after a
  refusal the relay's exit no longer reconnects at once, and the 3 s retry keeps trying, so a
  service that is put back to a compatible build heals the link with no person involved.
- B2, extras: the connection log gains an `adapter_refused` event, so an exported diagnostics file
  shows the refusal. `state.js` gains the module doc comment it lacked. The controlled-tab hook loses
  `managedIds`, which only the opener computation called.
- B2, the error golden: `tests/golden/protocol4/frames/error.json` still carries an older sentence
  than `language::adapter::ADAPTER_INCOMPATIBLE`. It is the adapter lane's file and no test compares
  the two, so the slice leaves it.
- B2, journeys for the coordinator: none was written or run here. On a test machine, a service
  crash or restart while an agent holds tabs should leave no debugging bar on them once the new
  service answers, and a worker restart against the same service should keep them bound.
  `tests/golden/agents/native/linux.txt` is unchanged.
