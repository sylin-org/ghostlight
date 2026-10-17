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

d02-activity-names has landed. Next: d04-integrations; d03-call-help runs in its own lane.

## Landed

| Slice | What | Commits |
|---|---|---|
| d00-agent-names | The service names each session's agent as a person knows it. `install::agent_name` is one closed table beside the harness registry (`AGENT_NAMES`): each product name as the install summary says it, with its harness and product ids and the client names it is known to send (`codex-mcp-client`, `claude-ai`, `cursor-vscode`), matched ignoring case and surrounding space; a test keeps it in step with every registry row. `language::provenance::agent_name` adds the command line (`COMMAND_LINE_AGENT`, "Command line") and shows an unknown name as reported, trimmed. `SessionSummary.agent` and `ConnectionDetails.agent` carry it on sessions, operations and history items; `client_label` and `reported_application` keep the reported name for the trust note. Durable history names only the command line, from its channel. The setup view matches a session to an agent by that name, so a Codex session turns the Codex row Connected, and names the finishing agent by product name. The window's chips, activity client column, hero and search name the agent. | see `git log` |
| d01-glance-summary | At a glance opens on one summary pill instead of a chip per client and browser plus Connection details. The pill shows a face per agent (initials, the prompt glyph for the command line, teal while working), "N agents", an arrow, a dot and "N browsers", and Details; it is a `<details>` disclosure kept open across repaints like the page's other disclosures, remembered as a page disclosure apart from the activity feed's, so a feed rebuild (a queued call, Clear view, a stream reconnect) never closes it. Details opens two shelves: agents (spark or prompt glyph, the d00 name, a count past one session, a working agent's glyph breathing in teal and still under reduced motion) and browsers (window glyph, name, tab count, adapter version on hover). With no browser the shelf shows the service's line in a dashed chip: "Starts <browser> on request" only when `BrowserRecovery::starts_on_request` says a request would start one (`on_demand` and the remembered or only installed browser with a usable registration), otherwise "No browser connected" (`language::workbench::no_browser`). An agent chip opens its card: one line per session in words (tabs and browser, working or idle, "from the command line" for the command line), at most 4 and "and N more", each with Copy id for its connection ids, and the trust note once. A live MCP connection's explanation now names the claim (`"codex-mcp-client" is what the agent calls itself; Ghostlight does not verify it.`, `language::provenance::claimed_name_explanation`). The service adds `tab_count` to each browser and `no_browser` to the connections part, which a watcher is sent too. The per-connection list, its CSS and the chip-dot rules nothing used are gone. | see `git log` |
| d02-activity-names | Activity rows and the hero say what a person needs. `toolWords` in `ui/lib/words.js` is the one rule for a tool's displayed name: the shared `browser_` start left out and the rest in words ("navigate", "fill form", "press key", "policy explain"); the exact name is the row's and the hero title's hover and a fact in the detail card's meta line. The client column and the hero name the agent (d00), or "Unknown agent" with the workspace id only as a hover. The intake cell is empty for MCP and holds the prompt glyph for the command line, with "Command line" as its hover and screen-reader words; the hero no longer adds "via cli". A settled row names only the browser its record names and never borrows its session's. The record names the browser the call reached, not the session's pin: the one browser seam remembers the browser each invocation's commands were sent to (`ApplicationExecutor::send`), unless the command failed before any of it was sent (`BrowserError::before_dispatch`), and the receipt names only that browser. A refusal before the call starts (invalid input, capacity, cancelled or past its deadline, an unknown workspace, an audit preflight refusal), a governance refusal before dispatch, `policy_explain`, and a call whose session's browser is gone record no browser; a flow step names the browser only when that step reached it. A session pinned to a browser that is gone carries none (`workbench/live.rs`). Both guessed "Chromium" before; `BrowserSummary::display_name` keeps the one family fallback for a connected browser that reports no name. docs/MEMORY.md's "Activity lists lead with tool identity" says the window drops the prefix. | see `git log` |

## Owed live checks

The coordinator deploys the batch in place on the owner's machine and checks each view with the
owner before anything is pushed.

- d00-agent-names: with Codex and Claude Code connected and a `ghostlight call` run, the At a
  glance chips read "Codex", "Claude Code" and "Command line", and the Activity client column and
  the hero name the same agents (never `codex-mcp-client` or `ghostlight call`). Connection details
  still show the reported name ("Reported application: codex-mcp-client"). With setup incomplete,
  a Codex session turns the setup view's Codex row Connected. A restored history row of an MCP
  agent still shows its short workspace id until d02 says "Unknown agent".
- d01-glance-summary: in the window, At a glance shows one pill ("3 agents", "N browsers",
  Details). Details opens the agent and browser shelves; a working agent's glyph breathes teal
  (still with reduced motion on); each browser chip shows its tab count. Pressing an agent opens
  its card with its sessions in words, Copy id per line (the toast says "Connection id copied.")
  and the trust note once. With every browser closed and auto-open on (the Windows default), the
  Browsers shelf says "Starts Google Chrome on request" (or the remembered browser); with
  `browser.startup` set to `manual` it says "No browser connected". A watching browser tab shows
  the same pill, shelves and card. The open pill and card survive a new session arriving, a call
  queued behind a running one, Clear view and a stream reconnect.

- d02-activity-names: in the window's Activity list, rows read "navigate", "fill form", "press
  key" with the icon first, and hovering a tool shows its exact name (`browser_fill_form`), which
  the opened detail card also lists among its facts. A `ghostlight call` row shows the prompt glyph
  in the intake column (hover: "Command line") and an MCP row shows nothing there; the hero of a
  `ghostlight call` says "Command line" once, with no "via cli". A restored row of an MCP agent
  reads "Unknown agent" (hover shows the workspace id). With the session's browser closed and auto
  open off, the refused row ("The browser this session works in is not connected. Nothing was
  sent.") shows no browser, and the agent's card line for that session names no browser either.
  With the browser connected, a `policy_explain` row and a row refused for invalid input show no
  browser, while a `browser_tabs` row of the same session names it.

## Notes

- d00-agent-names: the mapping matches harness ids, product ids, product names and known client
  aliases, not the host-qualified labels such as "Cline (CLI)": no client reports a label, and the
  setup view now matches on the product name instead. A Cline session therefore turns every Cline
  row of the setup view Connected, because the reported name does not say the host. The browser's
  tab group title ("Ghostlight - <reported name>") and released-tab reuse still use the reported
  name; the slice changes only what the window shows. The hero still adds "via cli" beside
  "Command line"; d02 drops it. The aliases are the ones the tree or the spec names plus
  `claude-ai` (Claude Desktop) and `cursor-vscode` (Cursor); add others only when a client is seen
  sending them.
- d01-glance-summary: differences from the spec. (1) The no-browser fact is a line the service
  writes (`ConnectionsPart.no_browser`, present only while no browser is connected), not a flag;
  the page shows it as given. Browser recovery answers it with its own decision (`decide`), so it
  cannot disagree with what a request does. It reads the installed browsers once per stretch with
  no browser connected and posture, never under `manual`, and never through the setup machine,
  which a snapshot must not read. A browser installed during such a stretch shows on the next
  stretch. (2) The browser chip's tab count comes from a new `BrowserInstanceSummary.tab_count`
  (the tabs of the sessions pinned to that browser), because the page cannot tell two profiles of
  one browser apart by name. (3) The trust note reuses each connection's `explanation`: a live MCP
  connection's sentence now names the reported claim, which the activity detail card of a live
  operation also shows; history keeps the nameless sentence because it keeps no name. (4) The pill
  says "No agents" and "no browser" for zero; a one-word agent's initials are its first two
  letters ("Co" for Codex, not the mockup's "CX"). (5) Faces use the window's neutral ink, teal
  when working, not the mockup's per-agent hues. (6) "from the command line" appears on every line
  of the Command line card, as the spec says, although the card's title already says it. (7) The
  preview server shows the no-browser state at `?browsers=none` and gives Codex six sessions so its
  card says "and 2 more".
- d02-activity-names: differences from the spec. (1) The command-line intake is a mark, the
  prompt glyph of the At a glance shelves, with "Command line" as its hover and screen-reader
  words, not the words printed: the intake column is 34 px wide, and the client column of the same
  row already says "Command line". (2) The exact tool name sits in the detail card's meta line
  (the hero's facts), because the row's detail card is the hero's markup. (3) The service fix is
  the root of the guessed "Chromium": the receipt named the session's pinned browser, and
  "Chromium" when that browser was gone, whatever the call did. The record now names the browser
  the call's commands reached, read at the browser seam before sending, so a browser that
  disconnects after taking a command is still named, and a call that never reached one names
  none. The session summary names only a connected browser, so a session whose browser closed
  shows no browser on its card line too. A connected browser that reports no product name still
  reads "Chromium", the adapter's family. (4) A flow's steps share the flow's invocation: each
  step's record names the browser that step reached, and the flow's own record names the browser
  any step reached. (5) The detail card's Connection fact keeps its
  shortened connection id, and a refusal keeps its denial id, because both are handles a person
  quotes when asking; no workspace id is printed. (6) A composition step with no receipt names its
  tool in words too. (7) The preview server's history shows each case: a refusal before dispatch
  with no browser, a command-line row, and a restored row that reads "Unknown agent".
