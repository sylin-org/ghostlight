# ux-2: ledger

Ground rules: [BOOTSTRAP.md](BOOTSTRAP.md). Statuses: `todo`, `landed` (with commits), `blocked`
(with the reason).

## RESUME HERE

Opened 2026-10-18. The re-walk's agreed list is split into the slices below. The window slices
run in order, u0 to u7. The native, extension, outcome and docs lanes run beside them. Next: the
lanes, then the coordinator's integration gate, the in-place deploy, the owner's validation and
the push.

## Slices

| Slice | Lane | Items | Status |
| --- | --- | --- | --- |
| u0 preview-shows-every-state | window | j-38 | landed (this commit) |
| u1 teal-means-live | window | n-d-02, j-27, j-24, j-34 (window) | todo |
| u2 only-running-work-glows | window | n-d-01, j-25, window-10, window-9 (time format) | todo |
| u3 band-and-rows-at-every-width | window | j-14, window-11, n-s-04, j-26, window-9 (Settings breakpoint), n-d-08 | todo |
| u4 a-switched-off-channel-is-said | window | n-s-02, n-d-05 | todo |
| u5 policy-reads-plainly | window | n-s-01, n-d-03, j-15, n-s-07, j-16, j-18 | todo |
| u6 one-word-per-act | window | j-08, j-28, j-29, j-32, window-9 (Notifications), window-7, window-3 | todo |
| u7 search-finds-rows | window | n-d-04, j-30, j-31 | todo |
| n1 light-taskbar-tray-glyph | native | j-11 | landed (this commit) |
| n2 the-tray-keeps-the-mascot | native | j-11 (owner) | landed (this commit) |
| e1 extension-share | extension | j-33, n-d-02 (popup eyebrow), j-08 (ended step), n-d-06, j-34 (kit) | todo |
| o1 a-failed-check-leads-with-the-failure | outcome | n-d-07 | landed (this commit) |
| k1 brand-reference-for-2-0 | docs | j-37 | landed (this commit) |

Held: n-d-09 (the taskbar app icon on a light taskbar) waits for the owner's light-mode look.

## Notes

- k1 (j-37): the 2.0 vocabulary table lists everything presentation.js draws today, not only the
  four effects the item named. The renderer still draws the cursor, ripples, trail, field splash
  and shimmer, scroll chevrons, read scan, navigate pill, screenshot and zoom frames, and five
  medallion kinds, so a table of four would have under-described 2.0. The navigate pill's
  chevron is described as u6 draws it, an SVG chevron. The dictionary page is labelled as the
  pre-1.0 vocabulary rather than reskinned. The click section's emit site now points at
  work/pointer.rs, and docs/design/README.md names the dictionary as pre-1.0.
- k1 (j-37), corrected label: the item called the old rows "1.x". The tree says they are the
  pre-1.0 (0.x) vocabulary. agent-visual-indicator.js and lib/presentation-broker.js belong to
  the 0.9 extension (archive/0.9-pre-1.0) and the 1.0 rebuild deleted them on 2026-08-10. Every
  1.x release, v1.0.0 to v1.3.15, drew from extension/lib/presentation.js with the browser_*
  tools, which is the 2.0 set plus an attention notice and a typed nav arrow. The history
  heading, the dictionary page and the design index now say pre-1.0, and visual-language.md
  says what 1.x drew. The review said 1.x drew from the orchestrator's presentation.js; that
  file only exists from 2026-10-14, after v1.3.15, so the note names extension/lib/presentation.js.
- k1 (j-37), neighbouring references: option (a). visual-feedback.md and tool-visual-signatures.md
  get a preface saying they record the pre-1.0 vocabulary and pointing at the 2.0 table; nothing
  in them is deleted or rewritten. docs/design/README.md moves both out of Living references into
  the prior-architecture table. For 2.0, the "Fires on" column of the vocabulary table is the
  coverage record, and "Adding a new effect" says so instead of pointing at the old ledger.
- n1 (j-11): both light-taskbar frames, 16 and 32 pixels, are now the monochrome tray glyph
  in crates/desktop/assets (tray-glyph-16.png drawn by hand, tray-glyph-32.png smooth): one
  silhouette in the night colour with its eyes cut out and no sparkles. Since no light frame is
  recoloured any more, ink() and BODY_LIGHTNESS are deleted rather than given the alpha-128
  skip the item named; the grey specks go with them. The dark taskbar keeps the product icon.
  The glyph is a plain ghost without the lantern, because the state dot sits where the lantern
  would. The owner approves it, and it is checked in the light-mode session at 100% and 150%.
- n1 previews, written by a throwaway program to the coordinator's scratchpad folder
  ux2-tray-preview/: `<light|dark>-taskbar-<16|24|32>px-<ready|working|paused|attention>.png`
  and the same names with `-8x` (48 files, nearest-neighbour 8x), plus sheet-1x.png and
  sheet-8x.png. In the sheets the top half is the light-taskbar art and the bottom half the
  dark-taskbar art; the left column of each half is on a light ground (#f3f3f3), the right on a
  dark one (#202020). The 24 pixel frames are the 32 pixel art scaled down, as Windows does at
  150%.
- n1, contracts: docs/1.0/ARCHITECTURE.md (the tray paragraph) and ACCEPTANCE.md item 12 now
  say the light Windows taskbar shows the shipped tray glyph under the dot, and that the ready
  icon is that art's own bytes. Both had said the light taskbar inked the original icon and that
  no other artwork shipped.
- n2 (j-11, owner): shown n1's glyph, the owner said: "I think it's fine to use the mascot in
  the tray icon, just not in the Integrations page." So the tray shows the mascot on both
  taskbars, and n2 replaces n1's glyph: tray-glyph-16.png and tray-glyph-32.png and their tests
  are deleted. The dark taskbar still shows the product icon's own bytes, and a test pins it.
- n2, what the light frames change: crates/desktop/assets/tray-mascot-light-16.png and -32.png
  are made once from extension/icons/icon16.png and icon32.png. The sparkles and stray pixels
  are removed; the ghost, its lantern and the shadow under it stay. The silhouette is grown by
  one pixel and drawn beneath the art in the night colour at 60 percent, so the head parts from
  the taskbar. At 16 pixels the two eye pixels take the 32 pixel art's darkest eye colour and
  the two mouth pixels a mid grey-blue, by hand, since the source's were nearly as light as the
  head. Everything else keeps the mascot's own colours. The shadow stays at both sizes: at 16
  pixels it sits on the outline as the ghost's base rather than as a stray line. Tests pin
  properties, not bytes: one piece, a night outline bordering the art, every edge pixel dark
  against the taskbar, and eyes clearly darker than the head at 16 pixels.
- n2 previews, in the coordinator's scratchpad folder ux2-tray-preview/ (FILES.txt lists them):
  sheet-1x.png and sheet-8x.png have three bands, the new light-taskbar mascot, the plain
  mascot untreated on a light taskbar for comparison, and the dark taskbar. The throwaway maker
  and sheet scripts are in its n2/ folder. ARCHITECTURE.md and ACCEPTANCE.md item 12 now name
  the light-taskbar mascot instead of the glyph.
- o1 (n-d-07): an unconfirmed check now leads with why it fell short and gives the action's own
  sentence after it: "The expected condition did not hold after the action. Clicked the "Submit"
  button on example.com." The item's wording ("did not hold after clicking ...") would need a
  second, gerund form of every action sentence; leading with the failure and keeping the one
  action sentence says the same in fewer moving parts. Pause and Stop keep their directive first.
- o1, same rule elsewhere in outcome.rs: the "Sent, but ..." sentences now lead with what went
  wrong ("The agent cancelled before the browser confirmed what the sent command did."), as do the
  adapter handshake timeout and the blocked landing whose tab state is unknown. Partial counts
  ("Completed 2 of 5 steps. Step 3 failed.", "Set 2 of 3 fields; ...") stay: their opening
  already says the work was incomplete. docs/1.0/LANGUAGE.md states the new order. The result
  goldens hold none of these sentences, so regenerating them changed nothing.
- u0 (j-38): the preview's head comment lists every query parameter. One part of the original
  j-38 was already true on the tree: Policy reads the policy the page script was handed, so
  nothing threw. The diagnostics folder path was left as it is: the agreed change does not name
  it, and the live layout is machine state. The default page's second operation now waits
  instead of being held while the runtime runs, which the service never shows; held work is on
  `?readiness=paused`. `?readiness=channel_off` says Ready on the band because the service has
  no channel state yet; u4 (n-s-02) changes the answer, and the fixture's mirrored words follow
  it there.
