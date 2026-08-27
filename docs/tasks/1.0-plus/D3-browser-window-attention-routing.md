# D3 -- browser-window attention routing

The STATUS "Owed" item: "ADR-0084's complete browser-window attention routing remains deferred;
only the narrow Chromium slice is implemented."

## Current-tree facts (as of authoring, 2026-08-26)

- ADR-0084 defined attention as the workspace-level "stop and wait for the person" state with
  per-window routing. The 1.0 tree implements the narrow Chromium slice: the extension's
  attention overlay (`buildAttention` in `extension/lib/presentation.js`) renders in pages, the
  runtime attention hold exists in the orchestrator's runtime controls, and the S5 directives
  (pause refuses; stop is terminal with its pinned sentence) are pinned by the
  reference-experience batch.
- What the narrow slice does not do: route attention to the right browser window when the person
  has several windows across several displays, coordinate the overlay with window
  focus/minimization, or present one coherent story when multiple workspaces hold attention at
  once across plural browsers (ADR-0114).

## Behavior

First deliverable is a decision, not code: an ADR that maps the full scope of ADR-0084 onto the
current architecture and decides, per row, implemented / superseded / still owed. The full
ADR-0084 inventory predates ADR-0114's plural browsers and ADR-0126's human-control directives;
several of its rows may be satisfied by means it did not predict.

Code follows only for the rows the ADR marks still-owed, at the owning seam (the orchestrator
decides; the extension renders; the connectors relay). The overlay, the runtime hold, and the
workbench surface must tell one story: one attention state per workspace, visible where the
person is, never competing with a person's own window management.

## STOP preconditions

- If plural-browser attention already covers a row the ADR was thought to owe, record it as
  satisfied-by-current-means instead of building a second mechanism.
- No window management decisions in the extension: routing decisions belong to the orchestrator
  (fringe-stability rule). If the ADR cannot place a decision at the orchestrator, STOP.

## Verification

- New ADR recorded in `docs/adr/` and indexed; rows dispositioned.
- Any code change: full gate per BOOTSTRAP, plus a live proof on the dev authority with two
  windows (one attended workspace raises attention; the other window's work is untouched).
