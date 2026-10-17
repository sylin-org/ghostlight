# Break 2.0: window design ledger

The owner reviewed the 2.0 window on 2026-10-17 ("Overall, I really like this") and asked for design
evaluations, point by point. This ledger records the owner's decisions and the slices that carry
them. The ground rules are the commit protocol in [BOOTSTRAP.md](BOOTSTRAP.md) and the gate in
[seams.md](seams.md). The approved mockups were built during the review and live outside the tree
(the coordinator's scratchpad); the decisions below are the record.

## Owner decisions (2026-10-17)

1. At a glance, Connection details: the expanded list is raw and off-style (the same trust sentence
   once per connection, `Intake: mcp` on every row, truncated connection ids, a flat list that
   ignores the per-client grouping, unbounded height). Decision: no separate Connection details
   toggle; an agent's card opens from the glance summary (decision 4) and shows that agent's
   sessions in words (tabs, the browser it works in, busy or idle), the trust note once per card,
   the intake only when unusual (the command line), connection ids behind Copy id, and at most a
   few rows with "and N more".
2. Activity list: drop the shared `browser_` prefix and keep the icon and the meaningful part
   (navigate, fill form); the exact tool name stays on hover and in the detail card, and agents,
   the audit trail and the catalog keep exact names. Same pass: no raw workspace ids in the client
   column (the agent's name, or "Unknown agent" with the id on hover); friendly agent names
   everywhere (Claude Code, Codex, Command line, as the install summary says them), with the
   reported name kept for the trust note; the intake marked only when it is the exception (CLI); no
   browser shown for a call that never reached one (a refusal before dispatch showed "Chromium");
   the hero card says the command line once, not "ghostlight call" and "via cli".
3. `ghostlight call --help` (and `-h`) prints the call usage, points to `--catalog` and to
   docs/guides/scripting.md, and exits 0 (today: "unknown option --help", exit 1).
4. The connectivity bar (the chips at the top of At a glance) does not tell agents from browsers.
   Decision: one summary pill ("3 agents -> 1 browser", small initials for who is connected, a
   Details button), "It provides agency to the user." Details opens two shelves, agents and
   browsers, with a glyph per kind (agent, command line, browser), friendly names, tab counts on the
   browser side, a working agent shown by a teal breathing glyph, and a no-browser state that says
   what happens next ("starts Chrome on request" when auto-open is on). An agent in the shelves
   opens its card (decision 1).
5. Integrations: today's grid of 19 cards repeats itself (status twice per card, the same sentence
   on 7 and on 11 cards, Copy MCP command on every card although it is one command, a loud red
   Remove on every connected card). Six directions were built as working mockups; the owner chose
   master-detail, then approved five grafts: the agents not on this computer fold into one rail
   row ("12 more agents Ghostlight can join") that opens in place and that search reaches; a
   computed headline ("Ghostlight is in all 7 of your agents.") replaces the intro paragraph; one
   header row holds Re-check, Set up everything and Copy MCP command with the command shown small;
   Ghostlight's entry is shown whole with no nested scroll; agents in use pulse teal (still under
   reduced motion). The owner then dropped the live line under the headline, because the band
   already says it. Kept from the concept: the rail grouped by state (Needs a look, Connected,
   Found, Not on this computer), the pane opening on the most relevant agent, each button's effect
   written beside it, Remove as a quiet link that asks first and names live sessions, numbered fix
   steps for an agent that needs attention, and copy-only agents (Continue, goose) said once.

## Slices

1. d00-agent-names: the service names each session's agent the way a person knows it.
2. d01-glance-summary: decisions 4 and 1.
3. d02-activity-names: decision 2.
4. d03-call-help: decision 3 (its own lane; command line only).
5. d04-integrations: decision 5.

## RESUME HERE

The batch starts. Next: d00-agent-names, with d03-call-help in parallel.

## Landed

| Slice | What | Commits |
|---|---|---|

## Owed live checks

The coordinator deploys the batch in place on the owner's machine and checks each view with the
owner before anything is pushed.

## Notes
