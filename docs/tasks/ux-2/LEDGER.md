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
| e1 extension-share | extension | j-33, n-d-02 (popup eyebrow), j-08 (ended step), n-d-06, j-34 (kit) | todo |
| o1 a-failed-check-leads-with-the-failure | outcome | n-d-07 | todo |
| k1 brand-reference-for-2-0 | docs | j-37 | landed (this commit) |

Held: n-d-09 (the taskbar app icon on a light taskbar) waits for the owner's light-mode look.

## Notes

- k1 (j-37): the 2.0 vocabulary table lists everything presentation.js draws today, not only the
  four effects the item named. The renderer still draws the cursor, ripples, trail, field splash
  and shimmer, scroll chevrons, read scan, navigate pill, screenshot and zoom frames, and five
  medallion kinds, so a table of four would have under-described 2.0. The navigate pill's
  chevron is described as u6 draws it, an SVG chevron. The dictionary page is labelled as the 1.x
  vocabulary rather than reskinned. The click section's emit site now points at
  work/pointer.rs, and docs/design/README.md names the dictionary as 1.x.
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
- u0 (j-38): the preview's head comment lists every query parameter. One part of the original
  j-38 was already true on the tree: Policy reads the policy the page script was handed, so
  nothing threw. The diagnostics folder path was left as it is: the agreed change does not name
  it, and the live layout is machine state. The default page's second operation now waits
  instead of being held while the runtime runs, which the service never shows; held work is on
  `?readiness=paused`. `?readiness=channel_off` says Ready on the band because the service has
  no channel state yet; u4 (n-s-02) changes the answer, and the fixture's mirrored words follow
  it there.
