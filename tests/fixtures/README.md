# Browser regression fixtures

`contenteditable.html` is a synthetic controlled editor. It discards direct DOM replacement and
generic synthetic input, while accepting native editing transactions. Its retained values and
submission counter test the user-visible result independently of Ghostlight's receipt.

`sylin-iframe.json` snapshots the owner's public Sylin iframe demo on 2026-10-16, the composed
stage. It carries the exact two HTML responses, nine referenced CSS/JavaScript/image responses,
their source locations, content types, and SHA-256 digests. Unicode is JSON-escaped and binary
responses are base64 so the fixture remains ASCII. Decoding restores the original bytes; the
browser journey verifies hashes. No authenticated page, entered form value, or machine-local data
is included.

The stage builds its form iframe inside the open shadow root of a `<composed-iframe-stage>`
element, from a `<template>`, with `demo-iframe.js`. That shadow root also holds a
`<frame-proof role="button">` control labelled "Verify composed frame", which reads "Composed
frame verified" once pressed. The frame holds the application form, whose completion notice sits in
a second open shadow root. The frame journey drives all of it: the form inside the frame inside the
shadow root, and the control in the shadow root (find, click, label change).

The frame journey serves these responses on its disposable local server, adjusts the HTML base
and child origin, and adds one parent input. The original form's local-only submission script is
preserved. Browser geometry and form behavior therefore do not depend on the public site's uptime
or future edits. This is captured Sylin content, not a claim about the current live website.

Set `GHOSTLIGHT_TEST_LIVE_SYLIN=1` to fetch current HTML and additionally navigate the public demo.
That optional lane requires network access. It serves every site file the live HTML references by
fetching it from the public site; the default suite uses the checked-in snapshot and its assets,
and refuses to run when the snapshot lacks a file its own HTML references.

Refresh the fixture with `node scripts/capture-sylin-iframe.mjs` when the public demo changes. It
fetches the two pages and every file they reference, and rewrites `sylin-iframe.json`. Review the
changed bytes and repeat both browser lanes before recording new evidence.

`agent-ux/rerender-list.html` is a song library that re-renders the way a React list does: named
rows that each hold their own `/song/<id>` link and a "More options" button whose id changes on
every render, plus buttons for an in-app tab switch at the same address, a delete, a recycle that
reuses a row node for another link, and a view switch that renames the region. It carries no
network resource. `tests/agent-ux-rerender-journey.mjs` serves it to a live browser and checks
which handles follow their control and which refuse (ADR-0198 Decision 17).

`workbench/agent-icon.png` is a 48 px PNG drawn for the window's tests: a blue disc with a dark
ring and a yellow diamond on a transparent ground, no app's artwork. The preview server
(`tests/workbench-preview-server.mjs`) serves it as Codex's own icon at `agent-icons/codex`, the
way the service serves an icon it found in an installed app, so the window shows one agent's icon
beside the monograms.
