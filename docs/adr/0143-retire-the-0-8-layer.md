# ADR-0143: Retire the 0.8 layer

- Status: Accepted
- Date: 2026-08-27
- Amends: ADR-0026, ADR-0050, ADR-0093
- Builds on: ADR-0140, ADR-0142

## Context

Ghostlight's 1.0 clean-room rewrite carried the published 0.8 line as protected input: a harvest
inventory and artifact-relationship ledgers covering 809 historical paths, a behavior-group
recovery matrix, a test inventory, and CI gates that checked all of it on every push. The
machinery existed to prove that no published 0.8 behavior was lost and that no 0.8 artifact was
silently mutated while 1.0 was being rebuilt.

1.0 is published, 1.1.0 is held in custody, and every catalog tool has been exercised live. The
recovery proof the machinery existed for is complete: R1-R9 restored the genuine capability
contractions through current seams, and the live integration pass closed on 2026-08-26. What
remains is bookkeeping that taxes ordinary work: every byte change to a long-inherited file
drifts a ledger row, and CI goes red until the ledgers are regenerated.

On 2026-08-27 the owner directed that the project carry no 0.8-related files or content anymore:
everything is 1.0 and later.

## Decision

- The 0.8 layer is retired and removed from the working tree: `docs/0.8/` (harvest, artifact and
  test inventories, recovery matrix, publication state), the four harvest and check scripts, the
  CI steps that ran them, the 0.8-named business, design, research, and task-batch records, and
  the pre-1.0 adapter rows in `compatibility.json`.
- The compatibility map speaks only 1.0 and later. Adapter 1.0.0 covering service 1.0.0-1.1.0
  is the entire live map; older rows were historical support-matrix entries.
- Git history and the `archive/0.9-pre-1.0` tag preserve every removed byte. Retirement is not
  erasure: nothing was rewritten, only unloaded.
- The preservation directives that obligated the project to carry the 0.8 material as active
  input are retired with it. The general laws stand: ADRs are immutable, dated records are not
  falsified, and product identity survives. ADRs and dated evidence that mention 0.8 keep doing
  so as history.
- The runtime migration that retires recognized pre-1.0 supervisor artifacts stays. It is
  current product behavior serving real upgrades from installed 0.8 systems, not 0.8
  documentation or release bureaucracy; removing it would strand real users.

## Consequences

- Ordinary releases stop paying the ledger tax. Editing `server.json`, trust pages, README, or
  any other long-inherited file no longer requires ledger regeneration, and the release-truth
  CI steps that enforced it are gone.
- The proof obligation the layer carried is discharged by the record, not by a standing gate:
  ADR-0133 and the capability-restoration batch ledger remain the evidence that no published
  behavior was lost, and the dated custody and live-integration records carry the rest.
- A future version archaeology question starts from `git log` and the archive tag instead of a
  harvested inventory.
