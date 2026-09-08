# Browser regression fixtures

`contenteditable.html` is a synthetic controlled editor. It discards direct DOM replacement and
generic synthetic input, while accepting native editing transactions. Its retained values and
submission counter test the user-visible result independently of Ghostlight's receipt.

`sylin-iframe.json` snapshots the owner's public Sylin iframe demo on 2026-09-07. It carries the
exact two HTML responses, eight referenced CSS/JavaScript/image responses, their source locations,
content types, and SHA-256 digests. Unicode is JSON-escaped and binary responses are base64 so the
fixture remains ASCII. Decoding restores the original bytes; the browser journey verifies hashes.
No authenticated page, entered form value, or machine-local data is included.

The frame journey serves these responses on its disposable local server, adjusts the HTML base
and child origin, and adds one parent input. The original form's local-only submission script is
preserved. Browser geometry and form behavior therefore do not depend on the public site's uptime
or future edits. This is captured Sylin content, not a claim about the current live website.

Set `GHOSTLIGHT_TEST_LIVE_SYLIN=1` to fetch current HTML and additionally navigate the public demo.
That optional lane requires network access. The default suite uses the checked-in snapshot and
its assets. Refresh this fixture deliberately when the public demo changes, review the changed
bytes, and repeat both browser lanes before recording new evidence.
