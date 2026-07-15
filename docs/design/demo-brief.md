# Ghostlight launch-brief demo

## Purpose

`ghostlight demo-brief` is the short recording story for the GitHub README hero. It is not a
capability inventory. It gives one small job a clear beginning, visible middle, and satisfying end
in roughly 10 to 12 seconds of active browser work.

The story runs through the same path as any MCP client:

```text
demo-brief -> ghostlight-relay -> local service -> extension -> visible Chromium tab
```

The public stage is `https://sylin.org/ghostlight/demo/brief/`. Its source lives in the adjacent
`sylin-org/website` repository. The stage sends and stores nothing.

## Story

1. Read the page once. Ghostlight's page-scan effect establishes that the agent understands the
   surface.
2. Fill Project, Owner, and Summary as three exact, separately paced ref writes so every touched
   control receives a readable visual beat.
3. Select Include screenshots and Keep data local as two more exact ref writes.
4. Click Create brief.
5. Hold the same-page completion state: `Moonlight Notes is ready for review.`

The exact fictional values are stable:

- Project: `Moonlight Notes`
- Owner: `Maya Chen`
- Summary: `Turn field observations into a shared release brief.`

## Visual contract

The page is warm-dark and restrained so Ghostlight's sky-blue language remains the only blue in
the recording. Its core colors are:

- canvas `#171918`;
- card `#202321`;
- inputs `#292d2a`;
- border `#3a403b`;
- primary text `#f0ede6`;
- secondary text `#a6ada6`;
- clay accent `#b59375`;
- completion surface `#26352d`;
- completion sage `#9db8a3`.

The page has no ambient animation, progress indicator, confetti, glass treatment, or native blue.
Its only authored movement is the short crossfade from form to completion. The Ghostlight-managed
tab border remains visible for the whole story. Read, field, click, and submit effects belong to
the extension.

The composition fits inside a browser window recorded at 1024 x 800 with Chrome chrome visible.
It also remains usable at narrow widths and honors reduced motion.

## Timing

The defaults are tuned for a short capture:

- loaded-page setup hold: 2.0 seconds;
- page-scan hold: 1.6 seconds;
- between-action beat: 0.25 seconds;
- completed-state hold: 3.0 seconds.

Relay, tool, and page-settlement time naturally provide the remaining beats. All three operator
rhythms are CLI options. The setup hold is preparation time; editors may trim it from the loop.

## Reliability boundary

- The command is an ordinary MCP client and never calls extension internals.
- The existing tighten-only demo policy grants only the Sylin stage and explicit loopback preview.
- One interactive `read_page` call inventories every later click target. Refs are reused while the
  document remains stable.
- The command inventories exact refs once, then uses top-level `form_input` writes so each visible
  field phrase has a unique command identity and deliberate beat. Agents using the same page should
  still prefer one compact `form_fill` call when they do not need capture pacing.
- Submission stays separate so its intent remains visible to the viewer.
- The command waits for the exact completion sentence before holding the final frame.
- No screenshot or GIF recorder runs inside the command. Record the visible Chrome window with the
  chosen desktop capture tool so browser chrome and extension cues remain truthful.
- The existing `ghostlight demo` Foundry story is unchanged.

## Capture recipe

1. Use a 1024 x 800 Chrome window with the tab strip, address bar, and Ghostlight tab group visible.
2. Start desktop capture.
3. Run `ghostlight demo-brief`.
4. Keep the opening page scan, all three field touches, both checkbox clicks, submit click, and the
   complete state.
5. Trim the setup hold as needed and loop the finished asset without inserting synthetic effects.
6. Export `docs/assets/demo.gif` below the README's existing size budget, then enable its hero slot.

## Acceptance checks

- `ghostlight --help` lists `demo-brief` and `ghostlight demo-brief --help` documents all pacing
  controls.
- The public stage exposes every exact accessible name used by the command.
- Site clean-build checks pin the route, controls, local-only disclosure, and route-specific assets.
- Rust formatting, strict clippy, and workspace tests pass in an isolated target directory.
- A live run shows the persistent border, page scan, field feedback, two click cues, submit cue, and
  the exact completion sentence without narration or a recording overlay.
- No trained schema, extension policy boundary, or runtime network boundary changes.
