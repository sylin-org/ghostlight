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
| u1 teal-means-live | window | n-d-02, j-27, j-24, j-34 (window) | landed (this commit) |
| u2 only-running-work-glows | window | n-d-01, j-25, window-10, window-9 (time format), owner-1 | landed (this commit) |
| u3 band-and-rows-at-every-width | window | j-14, window-11, n-s-04, j-26, window-9 (Settings breakpoint), n-d-08 | landed (this commit) |
| u4 a-switched-off-channel-is-said | window | n-s-02, n-d-05 | landed (this commit) |
| u5 policy-reads-plainly | window | n-s-01, n-d-03, j-15, n-s-07, j-16, j-18 | landed (this commit) |
| u6 one-word-per-act | window | j-08, j-28, j-29, j-32, window-9 (Notifications), window-7, window-3 | landed (this commit); j-32 landed in the integration pass, see Notes |
| u7 search-finds-rows | window | n-d-04, j-30, j-31 | landed (this commit) |
| n1 light-taskbar-tray-glyph | native | j-11 | landed (this commit) |
| n2 the-tray-keeps-the-mascot | native | j-11 (owner) | landed (this commit) |
| e1 extension-share | extension | j-33, n-d-02 (popup eyebrow), j-08 (ended step), n-d-06, j-34 (kit), j-24 (options switches) | landed (6b8f1d56, this commit); j-33 needs the window lane's breakpoint change, see Notes |
| o1 a-failed-check-leads-with-the-failure | outcome | n-d-07 | landed (this commit) |
| k1 brand-reference-for-2-0 | docs | j-37 | landed (this commit) |
| t1 page-targets-for-fill-upload-scroll | coordinator | k1 follow-up (owner) | landed (this commit) |

Held: n-d-09 (the taskbar app icon on a light taskbar) waits for the owner's light-mode look.

## Notes

- k1 (j-37): the 2.0 vocabulary table lists everything presentation.js draws today, not only the
  four effects the item named. The renderer still draws the cursor, ripples, trail, field splash
  and shimmer, scroll chevrons, read scan, navigate pill, screenshot and zoom frames, and five
  medallion kinds, so a table of four would have under-described 2.0. The item called the
  navigate pill's glyph a chevron; the tree (u6, navigationArrow) draws the kit's right arrow,
  shaft and head, in SVG, and the table says so. The dictionary page is labelled as the
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
- k1 (j-37), what reaches the page: the service emits TargetIndicated only from perform_click,
  perform_hover, perform_drag and perform_type_text, and WorkCompleted carries no locator. So
  the renderer's fill and upload field splash, its scroll-target chevrons and glow, and its zoom
  region never run in 2.0: browser_fill_form and browser_upload show only the caption, scroll
  shows viewport-centre chevrons on start, and zoom converges on the middle of the viewport. The
  table says that. Code follow-up for the owner, outside this lane: perform_fill, upload_files
  and perform_scroll would emit TargetIndicated for their resolved targets; if that lands, the
  table's rows return.
- t1 (k1 follow-up): the owner asked for it on 2026-10-19 ("Do this task here"). A fill now
  names each field once the page has set it, an upload names its input once the files are
  attached, and a scroll names its target once the page has scrolled to it, all through one
  executor helper that the click, hover, drag and type sites now share. Naming them after the
  browser acts keeps the cue where the control now is (a scroll centres its target, a fill
  scrolls field by field) and off work that did not happen. A targeted scroll binds its tab
  quietly before the command, so it draws its chevrons and glow on the element only, not a
  second set at the viewport centre. A drag now names its source after authorization, so a
  denied drag draws no trail. The vocabulary table's field splash, scroll cue and target glow
  rows say so. An image dropped at a point and the zoom frame still name no element. Review:
  three reviewers found the first version cued a targeted scroll before it scrolled; fixed.
- t1, open: click, hover and type still name their target before the command, and their
  commands scroll the element into view first, so a cue for a control below the fold is drawn
  off-screen. Naming them afterwards would lose a click's ripple when the click navigates, so it
  needs its own decision; it is carried to the architecture sweep's ADR.
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
- u1 (n-d-02): an agent wears the accent only while one of its sessions runs work
  (`active_operations` above zero), not whenever the service names it in use. The rail line of a
  working agent ends in "working" and the pane's state word says "Working", so working and
  connected differ in words. The pane's in-use box is now `.agent-use`, neutral, and `.agent-use.live`
  only while work runs; the item called it `.agent-live`. "Working" is composed by a helper
  (agentStateWord) beside AGENT_STATE_WORDS, because that table is held to the roster's states by
  a Rust test.
- u1 (n-d-02), Apply: every `.primary-button` is a ghost while it is disabled, so Apply is a
  ghost until there is a draft and Set up everything is a ghost when nothing can be set up. Only
  an enabled primary wears the accent. Re-check in the needs-attention steps is always a ghost.
- u1 (n-d-02), one more resting teal: the sacred-hosts readback (`.readback`) was set in the
  accent at rest; it is now ink (--ink-2). The EDIT hint also turns teal on keyboard focus, not
  only on hover. The stylesheet test lists every selector allowed the accent, by reason: work
  running now, focus, the pointer on a control, and an enabled primary. The About card is
  artwork and is left out.
- u1 (j-27): the agent offers (Found, not connected and Needs an update) use a new --offer
  token, ink-3, in place of --cap-action blue, on the dot, the wire, the state word and the rail
  line. The never-settled duration is set in --ink-2 rather than amber.
- u1 (j-24): every button (ghost, danger, primary and the band's wheel) is 32px, weight 500,
  radius 6, 12.5px, ink on an .18 hairline; the 28px settings-row override is gone. "Details" in
  the glance pill is the window's 12px sans disclosure with its chevron. Radii snap to
  4/6/8/12/pill and sizes to 10/11.5/12/12.5/13/14.5/17/22/25. The item's scale left out 12px,
  but the item itself sets the disclosure at 12px and 33 rules use it, so 12 stays a step. A
  test holds both scales; the trading card, monograms, icon tiles, scrollbars and the agent wire
  keep their own geometry.
- u1 (j-34): the About card's halo breathes at --beat-breathe (4000ms), the band's tempo.
- e1 (j-33): neither of the item's two fixes was enough on the tree. The Linux get script line
  is longer than the Windows one (420px at 13.5px in Cascadia Code, about 430px in the monospace
  fonts Linux has), so at 12.5px it still clipped up to about 930px, and one column below 920px
  still clipped from 920 to 961px and, with Copy beside it, from 1000 to 1043px. The kit now goes
  to one column below 980px and keeps Copy under the command below 1080px, the setup page's full
  width, at the 13.5px size. Measured in the preview at 600 to 1400px: neither install command
  nor the window's install and agent prompt panels scroll. Below about 490px the panel still
  scrolls, as the kit intends for a narrow panel. The window's setup view takes the same
  breakpoints from the same bytes.
- e1 (j-34): --beat-glow is deleted rather than set to 4000ms beside --beat-breathe. The glow,
  the lit lantern and the celebration loops now use --beat-breathe, so the kit has one breath.
- e1 (j-24): the options page's switches copy the window's .toggle values exactly (32x18 track,
  14px ink-3 thumb; on is an ink thumb on a .22 track with a .32 border; focus wears the accent).
- e1 (n-d-06): the popup is lit only while its activity list has an entry, which is one per
  running invocation. Idle Ready shows the waiting lantern, the same one as Not connected; the
  words tell them apart.
- e1: the freeze guard's head comment, its rule text and the kit's header now say what AGENTS.md
  says: an extension change regenerates the package record, and once a submission is staged,
  changing a packaged file means resubmitting, which is the owner's call.
- e1 review, open dependency for the window lane (j-33): the kit's one-column layout now runs
  below 980px, but crates/orchestrator/ui/styles.css still lifts the setup view's companion with
  `@media (min-width: 761px) { #setup-root .gl-companion { ... transform: translateY(-50%) } }`,
  written for the old 760px breakpoint. From 761 to 979px the view is one column and the
  companion rides up onto the column above it (measured at 880x800: 121px over the last agent
  row and Skip on ?setup=agent, 76px on ?setup=computer). The kit cannot override it, because
  the id selector outranks anything the kit can scope to. The window lane changes that rule to
  `(min-width: 980px)` and the pinned `(min-width: 761px)` in tests/workbench-settings.test.mjs
  ("every width rule asks the shell ..."); better, that test reads the breakpoint from
  extension/kit.css so the two cannot drift again. Then it checks the setup view at 761 to 979px
  for overlap, not only for scrolling. The extension lane did not edit either file: both belong
  to the window lane, which has them open.
- e1 review: docs/1.0/ARCHITECTURE.md's extension setup-page paragraph now says what AGENTS.md
  and the freeze guard say. It had said the pages are frozen and change only with an owner
  decision to withdraw the submission.
- u2 (n-d-01): isRunning now means phase running only; isPending covers waiting and held, and
  isUnsettled both, which the store uses for ordering and Clear view. A pending row's duration
  cell holds the word alone ("Waiting" in ink-3, "Held" in amber): the 62px column has no room
  for a word and a clock, so the clock beside the word is on the running card only, in ink-3.
  The band says "0 running, 2 waiting", and while paused with nothing running "2 held". The
  phase words and the count rule live in words.js (PHASE, PHASE_WORD, workCounts).
- u2 (n-d-01), integration pass: the band's state and the tray icon that follows it count only
  running work too. live.rs gave readiness `working` for any unsettled operation, so a call that
  only waited showed a teal "Working" beside "0 running, 1 waiting". `working` is now true only
  while an operation runs, the contract `ReadinessFacts.working` already stated.
- u2 (j-25): the dwell is 600ms (SWAP_DWELL_MS in view.js). A burst skips both the card's swap
  and the row's landing drop, and a plain settle holds the live look for the dwell; a refusal or
  a failure shows at once.
- u2 (window-10): after five minutes the settled card rests: a "Last action" eyebrow with its
  age, no glow, the medallion and clock in ink. A failure keeps its red edge and its red word on
  the card. BrowserPrimitive's kind word comes from the call's gesture (PageGesture::could_not
  in language/page_refusal.rs, applied by AuditProjection::for_gesture), so history says "The
  browser could not type into that field." The one line that applies it is in work/mod.rs,
  outside this lane's list; language/outcome.rs is untouched.
- u2 (window-9, time format): one format for every clock, duration() in words.js: "5.6s",
  "1m 05s", then "1:05:09". stopwatch() is deleted.
- u2 (owner-1): the owner, on Integrations: "I think we're misusing the mascot icon here. Maybe
  only have the icon and the text - no green line, no mascot? we already have the green dot to
  indicate connection". The pane header is now the agent's icon beside its name, state line and
  icon line, at every width. The wire, the mascot, MASCOT, their rules and the mascot test are
  deleted; the mascot route stays because setup still shows the mascot. This supersedes j-12's
  50px mascot and the wire part of n-d-02.
- u3 (j-14): the band's facts are four wholes, each with its separator inside it. "recorded"
  gives way below 1280px and the work in flight below 1160px (the item said "then running"; at
  1160 to 1180px "2 running, 1 waiting" still fits), and all of them below 1000px as before. As a
  guarantee beyond the width rules, the facts box is one line tall and wraps, so a fact with no
  room (a long count, Large text) falls onto a line the band never shows instead of being cut.
  Working keeps its facts below 1280px like Ready: its word is short and the running count is
  the fact worth reading then. Every other state hides them below 1280px. The history journey
  now measures the band at 720 to 1400px in Standard and Large text.
- u3 (window-11): the window wears "Shared" while others can watch and no chip while watching
  is off; the context word is `WebContext::word(watching)` and is null in that case.
  WORD_CONTROL is renamed WORD_SHARED. workbench/context.rs and workbench/parts.rs carry the
  change, outside the lane's file list; no other lane owns them.
- u3 (n-s-04): below 1040px only the browser gives way; the agent keeps a 96px column and the
  tool's column narrows to 140px, so the activity keeps about 180px at 821px. At 820px and below,
  the agent and the age sit on a second line under the activity, the command line's mark before
  them, in 11.5px rather than the item's 11px, because 11px is not on the window's type scale.
  The button says ", Codex, 4m ago" for a screen reader, so the agent and age cells and the
  second line are hidden from it. The ages on the second line and on the button tick with the
  age column.
- u3 (j-26): the item's ".cap-board" rule needed to follow the base rule in source order to win;
  a stylesheet test now reads the last applying rule, as the cascade does.
- u3 (n-d-08): the step line shows wherever the setup view is one column, which is below the
  kit's 980px breakpoint since e1, not below 761px. With it, the window lane's half of e1's open
  dependency (j-33) is done: the companion's sticky centering starts at 980px, measured with no
  overlap at 880 and 720px, and the settings test reads the breakpoint from extension/kit.css.
- u3 (window-9): the Settings rail stacks below 720px of shell width, so at the 720 minimum in
  Standard text the rail sits beside the section; Large text stacks it from about 828px.
- u4 (n-s-02): the held state's word is "Turned off". It comes right after Pause and before Not
  connected, the order the executor checks (the channel first), so a person is not sent to open a
  browser that would not help. Both switches off hold whether or not an agent is connected; one
  switch off with no connected agent stays Ready and says "MCP agents are turned off;
  command-line agents can work.". The tray takes the held icon from the tone, and its tooltip the
  word. The switches are read through `AuthoritySnapshot::channels_off`, which calls the
  executor's own `channel_refusal`; the words are in the new `language/channel_switch.rs`.
- u4 (n-s-02), Right now: with one channel off every capability something can still use keeps
  its badge (what the rules allow) and adds "Only command-line work may use it: MCP agents are
  turned off."; a capability the rules refuse says only why. With both off every line is Not
  available, naming whose policy did it. The policy tab's hover says the headline, so it names the
  switch too.
- u4 (n-d-05): a history record keeps no organization name, so an organization's channel refusal
  says "Your organization's policy turns off ..." rather than "Northwind's"; the refusal line
  under it names the organization from the passport. The band, Policy and Status read the live
  policy and do say "Northwind's policy".
- u4 (n-d-05): the .hero-reason line is gone for every refusal, not only where the refusal line
  says who decided: every refusal sentence the window shows already says why, so the line only
  ever repeated it as a code. A refusal with no sentence says "Blocked". "Permission details"
  says every reason no policy layer decided in words from a closed table in
  language/history.rs ("Refused: you paused Ghostlight."), and a channel refusal in the row's
  own words ("Refused. Your policy turns off work from MCP agents.").
- u4 (n-d-05): a refusal is a record whose decision refused it and whose status says blocked. The
  service records an invalid call as failed, so it now draws as a failure. The preview had called
  it blocked; its fixture now says failed, as the service does.
- u4 (n-d-05), the later part: the same mechanism gives the pause and stop directives a person's
  sentence in the window: "You paused Ghostlight." and "You ended the session.", followed by
  whatever the model's sentence went on to say about the action. The model keeps its directives.
- u4: the preview gains `?readiness=turned_off` (both switches off), and `?readiness=channel_off`
  now shows the Ready line, the Policy headline and capability lines, and the Status rules line
  that the service gives. governance/mod.rs (the snapshot's switches and the tier word),
  workbench/live.rs and the evidence tests are outside the lane's file list; no other lane owns
  them. INTENT, ACCEPTANCE item 4 and LANGUAGE say what the window shows.
- u5 (n-s-01): every Policy switch is `role="switch"`, named by its row's name and described by
  its detail; Closing tabs is also described by its Preserve Ghostlight tabs line, and the two
  channel switches by the limit note. A view test reads every name and description back.
- u5 (n-d-03): the channel limit leads Who may work, under its heading, at 72ch. The item's
  `margin: 8px 2px 0` is kept for the note under Restrictions in force; as the group's lead it
  sits `0 2px 8px`, so it is spaced from the switches below it rather than from the heading. The
  agreed sentence began "Each agent's program says ...", but the channel is declared by
  Ghostlight's own program (the trust page, ARCHITECTURE and ACCEPTANCE all say so), so it reads
  "Ghostlight's own program says whether an agent connected through MCP or the command line, and
  another program running as you could claim otherwise." The second sentence is as agreed. It
  claims what the trust page claims, a guardrail for agents that follow the rules and no barrier
  to other software on the account, so security-overview.md is unchanged.
- u5 (j-15): the history choice's label is "Stop browser work"; `require_audit` and its detail
  stay. No golden held the label. The governance guide names the window's two labels.
- u5 (n-s-07): on a watching page every switch, both choices and the never-touch field are
  disabled, Add a rule is hidden, and a person's own rules open read-only, as an organization's
  do. The line at the top is "Changes are made in the Ghostlight window." with the Continue link,
  the shape every Settings section uses. Apply is hidden there rather than reading "Apply in the
  Ghostlight window": with nothing editable there is nothing to apply, and it would point at the
  window a second time. APPLY_IN_WINDOW_LABEL is deleted. Integrations shows the Continue link
  alone. The purity test in workbench/surface_tests.rs read the sentence's "window." as the page
  reaching for `window`; it now looks for a member access, and checks that a sentence passes.
- u5 (j-16): a destination that cannot be read wears `load-failed`, and Try again stands in for
  its Refresh (Re-check on Integrations, which is the same look). A Policy never drawn also wears
  `unread`, which hides Right now, Always off limits and Rules; a Policy drawn before keeps what
  it showed under the failure line.
- u5 (j-18): Settings > This screen names the browser's two settings as lines with the "Set in
  the browser" chip, from a new `browser` group of the controls part worded in
  language/workbench.rs: "The cursor, click ripples and scan-lines on a page an agent works in.
  They start on." and "A caption at the bottom of the page names each action. They start off."
  They are labelled "Agent activity effects" and "Action captions", the names the extension's
  settings page shows, rather than the item's "Page effects", so a person finds them there. A
  screen that shows nothing hides them.
- u6 (j-08): the band's wheel says "Start a new session". The tray's Resume entry says it too
  once the session ended, and its click now asks for a new session: the authority ignores Resume
  on an ended session, so the old "Resume agents" there did nothing. The words and the intent
  come from `TrayItemId::label` and `TrayItemId::intent`, both by the runtime state; the shell's
  four entry ids are unchanged, so crates/desktop needs nothing. The desktop goldens were
  regenerated and did not change: their trays are Ready and Paused.
- u6 (j-32): the tray tooltip says the band's word as its own sentence, "Ready. Ghostlight is
  running; ...". `tray_tooltip` in language/workbench.rs joins them with a period in both arms,
  the desktop goldens are regenerated, and `each_golden_decodes_to_its_frame` in
  crates/desktop/src/link.rs pins the new tooltip. This landed in the integration pass. Others
  can watch reads "On: anyone on this computer can open this view" and "Off: only this window
  shows Ghostlight", the re-walk's colon form. The navigate pill's arrow is the kit's arrow path as an SVG, and
  tests/golden/protocol4/assets.sha256 is regenerated.
- u6 (j-28): Set up everything and its caption are hidden whenever nothing is left for it to do,
  in the window as on a watching page; once a press leaves nothing to do, focus moves to Re-check.
  The caption reads "Connects Cursor and updates Kiro. Junie needs you." in left-aligned sans;
  agentPlan says nothing when there is nothing to set up, so "Nothing to set up." is deleted. The
  service's evidence keeps its words, and the page sets each backticked program or path as code.
- u6 (j-29): the hero meta says the capability alone ("read + write"). The browser chip's hover is
  "Ghostlight in Browser 2.0.0".
- u6 (window-9): the controls part's notifications group gains `state`, "On", which the rail shows
  (workbench/parts.rs, outside the lane's list and owned by no other lane).
- u6 (window-7): the earlier session is a reserved agent (`Agent::earlier`, id `earlier`) with a
  hover the service words, `Agent.note`; the page shows any agent's note on hover and never a
  workspace id. It names a record read back from an earlier run that kept neither a claimed agent
  nor a source. A record whose client claimed a name the catalog does not know, and whose source
  could not be read, looks the same as one written before names were kept, so the agreed hover
  "Recorded before Ghostlight kept agent names." would be false for it. The hover says only what
  the record shows: "Recorded in an earlier session. Ghostlight kept no agent name for it." Telling
  the two apart would take a new receipt field for one hover, which no person needs. A record with
  a source, or with a claimed id the catalog no longer lists, stays "Unknown agent", as does a live
  client that sent no name. An agent nothing named gets no caveat, so its explanation is empty.
  docs/1.0/ACCEPTANCE.md's connection provenance bullets say these card rules.
- u6 fixup: the preview's scripted work settles again. The page's fixture borrowed a server-side
  helper; the server now composes each session's connection details into the fixture
  (`__GHOSTLIGHT_DETAILS__`), and tests/workbench-preview.test.mjs runs the working page's
  fixture until one action settles with no error.
- u6 (window-3): the card names the agent once, in its meta line. The connection disclosure holds
  the service's sentence, "Came through the command line." for the command line, and Copy id;
  Reported application, Intake and the printed id are gone, and a connection with nothing to say
  draws no disclosure. "Show the reported name only when it differs" is decided in
  language/provenance.rs: a live client's own words are quoted only where they read differently
  from the agent's name once case and punctuation are set aside ("codex-mcp-client" for Codex, not
  "claude-code" for Claude Code). An agent a claim named keeps the caveat, now "The agent's name is
  its own claim. Ghostlight does not verify it.", which no longer says "application" or "MCP
  client"; where a source stands apart from the claim, the caveat comes before "Started by ...".
  Permission checks the service explained in the same words are one line, under every check they
  cover. The meta line drops "authority", the raw tool name (the tool's hover keeps it) and
  "succeeded"; a row's details still say failed, unknown or kept, which nothing else on them says
  in words, and the running card says it beside its clock. A flow's steps no longer repeat the
  flow's connection. docs/1.0/ARCHITECTURE.md names the earlier session and the quoting rule.
- u7 (n-d-04): the service words every hit in language/workbench.rs and matches on those words,
  never on an id. A recorded action's title is its tool in words and its row's sentence
  ("navigate: Opened sylin.org."); it is found by its tool, sentence, host, agent, browser and
  outcome, newest first. Its second line is the agent and the browser from the service, and the
  window adds the age with the row's own clock, so the age agrees with the list. Failed, Unknown,
  Kept or Blocked comes as a wire word (`outcome`) the window words with its row's OUTCOME_WORD,
  at the hit's right, in the row's colour. The kind chip reads Page, Work, Session, Action,
  Browser, Check, Setting or Agent instead of the raw kind; work in flight says its phase on the
  second line, so its chip claims none. Beyond the item: Policy and Settings are offered as pages,
  the browser-work hit opens Settings > Browser work (a new `browser_work` destination, which the
  window's SEARCH_ADDRESSES maps to `settings/browser-work`) instead of Policy, a browser hit says "Connected, 2 tabs" instead of "Connected browser adapter 2.0.0", and
  a hit for an action the list holds opens that row's details and gives it focus. The session line
  and the agent-state words are composed in Rust for the hits and in words.js for the cards; tests
  pin both to the same words.
- u7 (j-30): the recorded count dropped from 410 to 200 at the first settle, because the store
  trimmed the service's history to the list's length. It now keeps up to HISTORY_HELD (500, the
  service's HISTORY_LIMIT, pinned by a Rust test), so the band and "Search all 410 (Ctrl+K)" say
  what search reaches. The end line shows only while the list is full and the service holds more
  than it shows. It is moved back to the end only after a rebuild, so a keyboard user on it keeps
  focus while rows arrive above it.
- u7 (j-31): the page's 48-unit gear and 40-unit lens are scaled onto the window's 24-unit box
  (ring r6, hub r1.6, eight spokes; lens r6.3 with a 5.4-unit handle), so the item's r12 and 9px
  are the page's own units. The lens moves as the page's does while running (--beat-lens 1250ms);
  reduced motion stills it with everything else. The particle delays were at styles.css
  .hero.live .particle, not :397-398. The browser pane could not open the preview in this lane, so
  the look was checked by the stylesheet guards, the surface journeys and the headless history
  journey, not by eye.
