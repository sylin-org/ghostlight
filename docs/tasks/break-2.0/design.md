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
7. One catalog of agents (2026-10-17, after looking at the deployed batch): "Shouldn't we have a
   catalog of agents, including indication of extracted icons, that we simply reuse across the UI
   and the backend?" On the page, a symbol "would simply contain 'what to show'". Decision: the
   install roster is the one catalog of agents, with the command line and an unknown agent as its
   two reserved entries. Every place the window names an agent carries one `{ id, name, symbol }`
   that the service builds, and the page draws the symbol through one renderer and decides nothing
   about an agent's looks. The owner's icon order (the app's own extracted icon, then a curated
   vendor mark, then the neutral monogram) arrives in 2.0.x as symbol sources the service resolves,
   with no change to the page.

8. The catalog holds every per-agent fact (2026-10-17): "Maybe we could use the catalog for much more -
   lists of likely places for the config files to be found, how to set the config files, etc. And we
   just project to consumer modules what they need." A read-only survey mapped every per-agent fact and
   where it is decided (coordinator scratchpad plan11/catalog-design.json). Decision: one two-level
   catalog (an agent, then its hosts: ordered location candidates per platform, a closed file dialect
   that decides each fact once, detection, download, copy-only), each consumer receiving a typed
   projection of only what it needs, and a generated, checked JSON projection for consumers outside
   Rust. The owner answered, the same day: (a) before the release candidate, a golden of every byte the
   installer writes and says (test-only, and the proof of 2.0's freeze of agent entries), and four
   window defects fixed: each Cline host opens its own download page; the setup view never offers
   Connect to a copy-only agent; neither surface offers an act an app container refuses; one session
   never marks every host of its agent Connected; (b) `ghostlight install --client` also accepts an
   agent's catalog id (`--client cline` selects every Cline host), and help and completions list the
   valid ids; (c) the public lists name every agent in the catalog, generated from it; (d) macOS is
   not in scope now, and the catalog and docs say Windows and Linux plainly. The restructure of the
   installer itself (the two-level catalog, locations as data, dialect facts once, words in
   language/, the generated projection) follows the release, each slice proving a zero golden diff.

9. One icon endpoint (2026-10-17, reviewing the icon batch e01 to e03 and its integration fix): "poor
   SoC. wouldn't it be better to have an icon endpoint - and the icon returned is based on the
   status/availability?" Decision: every catalog agent has one image address, `agent-icons/<id>`,
   and the service decides what it serves there, in the owner's order (the app's own icon, then a
   curated vendor mark, then the monogram). The symbol names that address with the served image's
   version, and carries the sentence about where the image came from only for an app's own icon.
   The page asks for each address and draws what comes back, the same way for every agent, and says
   nothing when an image cannot be had. "Status" is read as the image's availability, not the
   agent's connection state, which stays the page's dot and words.

10. Each app icon on its own ground (2026-10-17, looking at the live window). First: "I'd suggest you
    use a mid gray as background for icons, so full-white icons don't disappear" (landed as
    70b88551, a mid gray tile). Then, seeing it live: "mmm, now it looks odd... maybe we should have
    some heuristics to calculate a good bg color for each [icon] (or just a choice of dark gray/
    dirty white)?" Then: "I mean, we could even bundle the icon AND the bg decision on the
    endpoint, and simplify things. the emitted icon containing the choice bg." Decision: the icon
    endpoint serves each app icon as a finished tile, the icon on the ground the service chose for
    it (a dark gray for a mostly light icon, a warm off-white for a mostly dark or colored one, or
    the icon as cached when it has its own opaque plate). Every agent image the page draws is then
    whole like a monogram, and the page decides nothing about grounds.

## Slices

1. d00-agent-names: the service names each session's agent the way a person knows it.
2. d01-glance-summary: decisions 4 and 1.
3. d02-activity-names: decision 2.
4. d03-call-help: decision 3 (its own lane; command line only).
5. d04-integrations: decision 5.
6. d05-design-tidy: the review notes of the slices above and of the batch's integration review.
7. d07-agent-catalog: decision 7.
8. d08-catalog-finish: the review of d07 (decision 7).
9. c00-writer-golden, c01-download-per-host, c02-setup-view-rules, c03-private-view-acts,
   c11-client-by-agent and c12-agent-lists: decision 8, before the release candidate. The installer
   restructure (c04 to c10 in the survey) follows the release.
10. e01-icon-cache, e02-icon-windows and e03-icon-polish: decision 7's icon order, the app's own
    icon first. The owner asked on 2026-10-17: "Didn't we talked about extracting icons? And a
    method that would simply publish the icons when available?" e01 is the catalog's icon sources,
    the resolver and cache, the published symbol and Linux; e02 reads Windows programs and Store
    packages; e03 is how the icons look and what the window and doctor say about them.
11. f01-icon-endpoint: decision 9.
12. t03-icon-tile: decision 10.

## RESUME HERE

d00 to d08, c00-writer-golden, c01-download-per-host, c02-setup-view-rules, c03-private-view-acts,
c11-client-by-agent, c12-agent-lists, e01-icon-cache, e02-icon-windows, e03-icon-polish,
f01-icon-endpoint and t03-icon-tile are on `dev` (not yet pushed). Next: the coordinator deploys, the owner looks (the
e02, e03, f01 and t03 owed checks name what Windows, doctor and the window should show), the coordinator
pushes and watches the browser lane's
`history-browser` journey in CI (d08 note 1), the Linux test job's writer golden (c00 note 4) and
agent list (c12 owed check), the Linux runs of the icon tests (e01 owed check) and the Linux build
of the service with e02's portable parts (e02 owed check).

## Landed

| Slice | What | Commits |
|---|---|---|
| d00-agent-names | The service names each session's agent as a person knows it. `install::agent_name` is one closed table beside the harness registry (`AGENT_NAMES`): each product name as the install summary says it, with its harness and product ids and the client names it is known to send (`codex-mcp-client`, `claude-ai`, `cursor-vscode`), matched ignoring case and surrounding space; a test keeps it in step with every registry row. `language::provenance::agent_name` adds the command line (`COMMAND_LINE_AGENT`, "Command line") and shows an unknown name as reported, trimmed. `SessionSummary.agent` and `ConnectionDetails.agent` carry it on sessions, operations and history items; `client_label` and `reported_application` keep the reported name for the trust note. Durable history names only the command line, from its channel. The setup view matches a session to an agent by that name, so a Codex session turns the Codex row Connected, and names the finishing agent by product name. The window's chips, activity client column, hero and search name the agent. | see `git log` |
| d01-glance-summary | At a glance opens on one summary pill instead of a chip per client and browser plus Connection details. The pill shows a face per agent (initials, the prompt glyph for the command line, teal while working), "N agents", an arrow, a dot and "N browsers", and Details; it is a `<details>` disclosure kept open across repaints like the page's other disclosures, remembered as a page disclosure apart from the activity feed's, so a feed rebuild (a queued call, Clear view, a stream reconnect) never closes it. Details opens two shelves: agents (spark or prompt glyph, the d00 name, a count past one session, a working agent's glyph breathing in teal and still under reduced motion) and browsers (window glyph, name, tab count, adapter version on hover). With no browser the shelf shows the service's line in a dashed chip: "Starts <browser> on request" only when `BrowserRecovery::starts_on_request` says a request would start one (`on_demand` and the remembered or only installed browser with a usable registration), otherwise "No browser connected" (`language::workbench::no_browser`). An agent chip opens its card: one line per session in words (tabs and browser, working or idle, "from the command line" for the command line), at most 4 and "and N more", each with Copy id for its connection ids, and the trust note once. A live MCP connection's explanation now names the claim (`"codex-mcp-client" is what the agent calls itself; Ghostlight does not verify it.`, `language::provenance::claimed_name_explanation`). The service adds `tab_count` to each browser and `no_browser` to the connections part, which a watcher is sent too. The per-connection list, its CSS and the chip-dot rules nothing used are gone. | see `git log` |
| d02-activity-names | Activity rows and the hero say what a person needs. `toolWords` in `ui/lib/words.js` is the one rule for a tool's displayed name: the shared `browser_` start left out and the rest in words ("navigate", "fill form", "press key", "policy explain"); the exact name is the row's and the hero title's hover and a fact in the detail card's meta line. The client column and the hero name the agent (d00), or "Unknown agent" with the workspace id only as a hover. The intake cell is empty for MCP and holds the prompt glyph for the command line, with "Command line" as its hover and screen-reader words; the hero no longer adds "via cli". A settled row names only the browser its record names and never borrows its session's. The record names the browser the call reached, not the session's pin: the one browser seam remembers the browser each invocation's commands were written to (`ApplicationExecutor::send`), and the receipt names only that browser. Whether a command was written comes from the port, not from its error: `BrowserDispatch.written` is set when the protocol-4 Link starts writing the command's first frame, so a read whose deadline runs out while the browser holds it, a cancel mid-read, or a page that changed under a read (all failures before any effect) still names the browser. A refusal before the call starts (invalid input, capacity, cancelled or past its deadline, an unknown workspace, an audit preflight refusal), a governance refusal before dispatch, `policy_explain`, and a call whose session's browser is gone record no browser; a flow step names the browser only when that step reached it. A session pinned to a browser that is gone carries none (`workbench/live.rs`). Both guessed "Chromium" before; `BrowserSummary::display_name` keeps the one family fallback for a connected browser that reports no name. docs/MEMORY.md's "Activity lists lead with tool identity" says the window drops the prefix. | see `git log` |
| d03-call-help | `ghostlight call --help` and `-h` print the call usage (its forms, `--json`, `--stdin`, `--catalog`, the exit codes, `GHOSTLIGHT_SESSION` and a pointer to docs/guides/scripting.md) and exit 0 without contacting the service (decision 3): `launch_mode` reads a help flag anywhere among the call's arguments as `LaunchMode::CallHelp`, which runs in the inspect role and installs nothing. The usage text lives in `cli/parse.rs` beside the `ghostlight --help` text, not in `language/`, and its exit-code lines are built from the constants `run` returns. | d71725a6 |
| d04-integrations | The Integrations tab is the approved master and detail page (decision 5). One header row: the kicker, the computed headline (`agentHeadline` in `ui/lib/words.js`: "Ghostlight is in all 7 of your agents.", "2 of your 5", "none of your 3 agents yet", one agent by name, "Add Ghostlight to your agent."), and on the right Re-check, Set up everything with a caption that says beforehand what it will do (`agentPlan`: "Will connect Claude Code and update Cursor. Junie is left for you."), and one Copy MCP command with the command shown small. A rail grouped Needs a look, Connected, Found, not connected and Not on this computer, with a state dot per agent and a second line only for what is the agent's own (sessions and tabs, Needs an update, Needs your attention, Copy setup only); the agents not on this computer fold into one row ("12 more agents Ghostlight can join") that opens in place, scrolls into view, and that the rail's search reaches (`/` focuses the search). A pane for the picked agent: its icon joined to the mascot by a wire that shows the state, one sentence for it, the next step with each button's effect written beside it, the settings file with Locate, Ghostlight's entry shown whole (one block per line, the indent a class, no scroll of its own) with Copy setup, the service's evidence and numbered fix steps for an agent that needs attention and cannot be fixed, a live line with "See them in At a glance" (it opens that agent's card), and Remove last as a quiet link that asks first and names the live sessions. The page opens on the most relevant agent and keeps the pick by id. An agent in use pulses teal on its rail dot, still under reduced motion. Only the rail and the pane scroll. The service adds `HarnessSummary.copy_only` (Continue and goose). The card grid, its CSS, the category vocabulary and `openHarnessManual` are gone; a failed act shows that agent in the pane instead. ADR-0129 gets a marked amendment. A review fixup: a session that starts or ends after the first paint reaches the rail, the pane and Remove (the page's memo holds each agent's live use and no outer gate hides it); a copy-only agent counts as connected while one of its sessions is live and otherwise, when found, stands in its own group "Found, set up by hand", left out of the headline's count ("2 of the 6 agents it can check"), with a sentence that claims nothing about its file; an agent that needs attention with no evidence gets steps from what the service saw (correct the file yourself, then Re-check), never "what Ghostlight found". | see `git log` |
| d05-design-tidy | The batch's loose ends, one commit per item. (1) No blank or default agent names: `language::provenance::claimed_name` says a client reported no name when its name is empty after trimming or is Ghostlight's own label for a client that sent none (`intake::mcp::DEFAULT_CLIENT_LABEL`, "MCP client"); `agent_name` then gives none, `ConnectionEvidence::details` gives no agent, no reported application and no claim sentence, and `SessionSummary.agent` is optional. The page groups such sessions as one "Unknown agent" on its shelf, and the service's search names them `language::provenance::UNKNOWN_AGENT`, the page's word, which a test pins. The session's agent is built from the workspace's label, the bounded claim the card shows, so a chip keeps no character the card removed. (2) One roster: `install::ROSTER` holds one `AgentRow` per host with who the agent is (harness id, product id, product name, and the names its client is known to report, `aliases`: `codex-mcp-client` on Codex, `claude-ai` on Claude Desktop, `cursor-vscode` on Cursor) and where its file lives (`path`, a function of the computer). `definitions` builds the registry from it, and `install::agent_name` reads it: a row matches its harness id, its product name and its aliases, ignoring case, never its product id, so `windsurf` names Windsurf and only `devin` names Devin. `AGENT_NAMES`, the `row` builder and `packaged_as` are gone. (3) `SessionSummary.client_label` is gone from the snapshot, the preview server's fixtures and the page tests: nothing in the window read it, and the trust note reads `connections[].reported_application`. The workspace keeps its label for the browser's tab group and released-tab reuse. (4) One name for the page: the page-title vocabulary (`VIEWS` in `ui/lib/words.js`), the service's search hit, INTENT, ACCEPTANCE item 8, README, SUPPORT, ROADMAP, DEV-LOOP, llms-install and the installation and solo-developer guides say "Integrations"; a test pins the search hit to the page's word. (5) The npm launcher's `neverInstalls` answers `call` with `--help` or `-h` anywhere among its arguments without installing, the rule the service's `launch_mode` uses for `CallHelp`: with nothing installed it runs the verified staged service, which prints the usage. The launcher tests cover both, and the npm README names the help commands. The MCPB launcher takes no arguments (it always runs `ghostlight mcp`), so it has no such rule to align. (7) The no-browser line's read of the installed browsers (`BrowserRecovery::starts_on_request`, kept per stretch and posture) runs with the `starts` lock released: a look takes the kept answer or a copy of the recovery under the lock, reads outside it, and keeps the answer only when no browser connected and the installed browsers did not change meanwhile (`BrowserStarts.stretch`). A test holds one read open and shows the change goes through and the held answer is not kept. (6) This ledger gets the d03-call-help row and RESUME HERE says the batch has landed; docs/STATUS.md says the window design batch landed and awaits the owner's live check. | see `git log` |
| d06-live-look | The coordinator's fixes from looking at the deployed batch (2026-10-17). The summary pill's faces wear the same tile the Integrations rail shows for each agent (`integrations/<icon>`, by the agent's name from the Integrations part, kept across snapshots that lack it), so Claude Code (CC) and Codex (CX) no longer read alike; initials stay for an agent the page has no tile for, and a working agent with a tile gets a teal ring. The preview server's queued rows carry the intake and agent of the session that ran them, as the service's records do (they showed a command-line mark beside "Unknown agent"). The headline's "of the N agents it can check" when some agents are set up by hand is kept as d04 wrote it: it is the truthful form of the approved "of your N agents". | see `git log` |
| d07-agent-catalog | One catalog of agents and one value that says what to show (decision 7). `install::ROSTER` is the catalog: each `AgentRow` says who the agent is (harness id, product name, aliases), how it looks (`monogram`, its packaged tile under `ui/integrations/`) and where its file lives. The new domain module `crate::agent` holds the value and its closed vocabulary: `Agent { id, name, symbol }`, `Symbol` serialized as `{ "src": "integrations/<file>" }` or `{ "glyph": <word> }`, and `Glyph` (`command_line`, `unknown`). The command line (`COMMAND_LINE_ID`) and an unknown agent (`UNKNOWN_ID`) are the catalog's reserved entries, named by `language::provenance` ("Command line", "Unknown agent"); a test keeps them off every roster row. One function names an agent: `Agent::of(reported, channel)` (the command line for a `ghostlight call` session, the roster's agent for a name it knows through `install::agent_called`, a name it does not know kept as reported with the unknown glyph, and the unknown agent when no name was reported), with `Agent::of_evidence` for work and `Agent::listed` for a roster row. `SessionSummary.agent`, `OperationSummary.agent`, `HistoryItem.agent` (built when the item is projected, never persisted: the audit line is unchanged) and `HarnessSummary.agent` (in place of `icon`) carry it; `ConnectionDetails` is evidence only again. `language::provenance::agent_name` and `install::agent_name` are gone. The page draws every agent through `symbolMarkup(agent, size)` in `ui/lib/words.js` (sizes `face`, `chip`, `card`, `rail`, `pane`; one `.symbol` style family): the pill faces, the shelves chips, the card head, the Integrations rail entries and the pane hero. The page-side icons map and its keep-across-snapshots logic, the initials, the spark glyph, the page's `UNKNOWN_AGENT` word and its pin test, the command-line decision from the intake, the session fallback for an action's agent and the unused `generic.svg` tile are gone; the activity hover shows the workspace id for an agent with the unknown glyph. Tests: every roster row has a packaged symbol and every Integrations row is its own catalog entry; a session, its running work, its history item and its Integrations row carry the same agent; the command line and an unknown agent get their glyphs; the page draws the same symbol in the pill, the shelves, the card and the rail, and no initials or spark; a pin keeps the page's glyph words in step with `Glyph`. The preview server and the page tests use the new shape. | see `git log` |

| d08-catalog-finish | The review of d07, one commit per item. (2) No impersonation: `Agent::of` gives an MCP client whose claimed name is a reserved entry's name ("Command line" or "Unknown agent", ignoring case and space) the unknown agent, so it never wears the command line's glyph or joins its chip; the connection's explanation still quotes the claim. A test covers both names in several spellings. (3) One id per agent: each roster row names its agent's catalog id (`AgentRow.agent_id`), one per agent and shared by every host it runs on, never the product id: the four Cline rows say `cline`, every other row its harness id. A session, its work, its history item and each of its Integrations rows carry that id (`HarnessSummary.agent`), and each row keeps its harness id for its own acts. `agent_called` matches the catalog id too. An agent Ghostlight does not know is identified by the name it reported (`unknown:<name>`, `UNKNOWN_NAME_SEPARATOR`), and one nothing names stays `unknown`. The id replaces every name lookup: the glance groups sessions on `agent.id` and keeps the open card by id (`data-glance-agent`), the Integrations pane's live sessions (`useOf`) and its link to At a glance (`data-show-agent`) use the id, and `workbench/setup.rs` `is_agent_of` compares ids. Tests: rows share an agent id exactly when they share a product name, and no id is reserved or holds the separator; a Cline session, its work, its history item and all four Cline rows carry one agent; the page's shelves make one chip of two hosts and two of two unknown names. (4) `symbolMarkup` no longer adds the `tile` class, which no rule selected: a symbol is styled through `.symbol`, its size class and the `img` or `svg` inside it, and the markup test reads the class list without it. (5) The preview server names each record's agent from the record's own evidence, as `Agent::of_evidence` does, never from its session: a live claim names its agent, the command line is named by its channel, and evidence that keeps no claim is the unknown agent. A record keeps the evidence it carries; one with none takes its live session's connection when its session is in this run, and otherwise only what its channel says (the legacy sentence). The restored row now carries durable evidence (a connection id, no claim, the durable MCP sentence) and reads "Unknown agent". (1) The CI journey `tests/workbench-history-browser.mjs` (the browser lane's `history-browser`, through `tests/hardening-suite.mjs`) no longer drives the session-level Connection details d01 retired: its C1 part opens the At a glance summary and Codex's card and checks both across refreshes that change the session's connection evidence, the card's trust note, and the command-line card's own line. The per-action Connection details checks are kept. It was checked with `node --check` only. | see `git log` |
| c00-writer-golden | A characterization golden of everything the installer writes and says, with no product behaviour change. `install::writer_golden` sets up every roster host from fixed seed files on three computers (Windows, Linux, and Windows from inside the `OpenAI.Codex` app container) and records, per seed, what inspection says (state, detail, evidence, the acts it offers, the download page, copy only, private view, the config path, the connector command and the manual setup document), what Check, Install, Fix and Uninstall say, and every file each act wrote or removed (backups and the located-path state included), each act from a fresh copy of the seed. The seeds: absent, empty, JSONC with comments and another server, a foreign entry, the current entry, a stale entry, the 1.x relay entry, the retired connector, a malformed file and a located file; OpenCode adds its nested `mcp.servers` files, Codex its entry with the Linux desktop forwarding, and goose and Continue have absent, empty, other and located. Setting up everything at once is recorded per computer. The files are `tests/golden/agents/writes/<harness>.txt` and `setup-everything.txt`, ASCII, with `<root>` and `<edge>` placeholders; `tests/golden/agents/native/windows.txt` and `linux.txt` pin, per build system, how its own separators and their escapes are written (note 4); `GHOSTLIGHT_AGENTS_GOLDEN_UPDATE=1` rewrites them (AGENTS.md lists it). The one product change: the bytes no longer depend on the build OS. The JSON and Codex TOML writers read the computer from the context (`HarnessContext.windows`, and a new `HarnessContext.linux` for the Codex desktop forwarding) instead of `cfg!`; in production both are set from the build target, so nothing a person gets changes. The golden is new, so there is no earlier diff; it records today's behaviour, bugs included. | see `git log` |
| c01-download-per-host | Each host opens its own download page (decision 8a). `HarnessSummary.download_url` is a real field, set by inspection from the host's roster row and present exactly when `can_download` is, so every audience gets it with the summary; the splice that added it afterwards (`workbench::parts::add_download_url`, `DOWNLOAD_URL_KEY` and `routes.rs` `link_download` with its `harness_result` wrapper) is gone. `HarnessRegistry::download_url` and `WorkbenchFacade::open_harness_download` take the harness id: an id outside the roster is refused with `UnknownHarness`, a host with no page with `NoDownload`, and neither opens anything. `WorkbenchFacade::harness_download_url` is gone. `POST /api/open` takes `{"harness": <id>}` in place of `{"product": ...}` (a `product` key is now a bad request); the page's Install button carries only `data-harness`, `app.js` sends that id and `transport.js` names it. Tests: through the facade each of the four Cline hosts opens its own page from the catalog and the extension's page is not the CLI's; the route opens Cline (Visual Studio Code)'s page and refuses an unknown host and one with no page; the snapshot gives every host its own page, or none; every roster row's summary carries its row's page and the registry answers it by id; the page sends the harness id (`tests/workbench-surface.mjs`, `tests/workbench-transport.test.mjs`). Golden: no writer byte changed; the reviewed diff is the `download:` line of the inspections in `cline-vscode.txt` (now the Visual Studio Marketplace page), `cline-cursor.txt` and `cline-windsurf.txt` (now the installing-cline page), 23 lines each, and nothing else. | see `git log` |
| c02-setup-view-rules | The setup view stops re-deciding what the roster and the workbench say (decision 8a). In use is one rule, `workbench::parts::in_use`: a live session of the row's agent (`is_agent_of`, moved there from `workbench/setup.rs`) and the row's entry in place (`Installed`), or a copy-only row with a live session. `ConnectionsPart.harnesses_in_use` carries the ids of the rows in use, in roster order, computed from the cached roster at each look (`Gathered.harnesses_in_use`) and from a snapshot's own roster (`ConnectionsPart::of`), so it is published with the sessions it is decided from and changes when an install or removal does. The setup view reads the same rule through `AgentFact.in_use` (was `seen`), so one Cline CLI session no longer marks a Cline host without the entry Connected; it still marks every Cline host whose entry is Installed (note 7). `agent_row` offers Connect only when `AgentFact.can_install` (and not a private view), so a copy-only agent (goose, Continue) reads "Found, set up by hand" and offers Copy setup (`SetupAction::CopySetup`, kind `copy_setup`; the page copies the row's `manual_setup` as the Integrations page does and the service refuses it as a setup act). A new `AgentState::Updatable` reads "Needs an update" with Update (the same `connect_agent` write, pending "Updating"); the words are `language::setup` constants pinned against `ui/lib/words.js`. The page: `agentStanding` takes the service's in-use answer, `agentOf` reads `harnesses_in_use`, and the store and transport carry it. A new surface test pins `AGENT_STATE_WORDS` keys to the `HarnessState` wire values plus `by_hand`. Tests: the setup rows for Updatable, copy-only, no `can_install`, private view and the four Cline hosts; a facade test that a Cursor session is in use only after its entry is written, in the published connections and the snapshot; the page draws only the rows the service names (`workbench-settings`), the store keeps the list, the transport flattens it, and the setup surface's Update and Copy setup clicks. Docs: an ADR-0200 amendment of 2026-10-17 (Decision 9) and its index row, and ARCHITECTURE's setup paragraph. Golden: `install::writer_golden` passes unchanged, zero diff; no writer is touched. | see `git log` |
| c03-private-view-acts | Neither surface offers an act an app container refuses (decision 8a). `install::inspect` makes `can_install`, `can_fix` and `can_uninstall` false for a target whose configuration the container this process runs in keeps private (`HarnessContext::private_to`), and its detail says why: `language::deployment::client_kept_private` ("Windows keeps this configuration private to <app>, so a change made from here would not reach other apps. To change it, run Ghostlight's install from a regular terminal."). The refusal `HarnessError::PrivateView` says the same sentence, so the detail and the refusal cannot drift. A copy-only target keeps its Copy setup sentence; Locate (it writes only Ghostlight's own state) and Copy setup stay offered. `HarnessState::takes_install` is the one rule for which states an install writes. `HarnessRegistry::setup_detected` leaves a private target out instead of failing on it. `AgentFact.private_view` is gone: the setup view reads only `can_install`, whose doc says it is false while the service's own container keeps the agent's file private (the doc conflict). `ghostlight install` from inside a container still names the clients it leaves to a regular terminal: `select_harnesses` selects a private client an install would write (`left_private`), which the run then adds to the kept-private lines as before; a dry run or a removal selects a private client too (a removal one that has Ghostlight's entry), changes nothing and says, in `language::deployment::client_left_private`, that the entry is left and which command run from a regular terminal changes it, so a removal never says that no client configuration needs to change while it leaves an entry. HarnessSummary.private_view's doc says no change is offered. ARCHITECTURE's container paragraph says so. Tests: inside another app's container Claude Desktop offers no Install, Fix or Uninstall, keeps Locate, every act is refused with its detail, Set up everything reports no failure and writes nothing; the CLI selects the private clients an install would write and, for a removal, the private client that has the entry; `client_step` leaves a private entry in place for a removal and a dry run and names the client; the setup view tests read `can_install` alone. Golden: no writer byte changed (no `wrote` line moved). The reviewed diff is in the container sections only, of the eight private hosts (claude-desktop, cline-cursor, cline-vscode, cline-windsurf, devin, kilo-code, vscode, zed): per seed the `detail:` and the check's `after:` detail now say the private sentence (24 each), `offers:` loses install or uninstall (24), and every `refused:` line (78, goose's 6 included) ends ". To change it, run Ghostlight's install from a regular terminal." in place of "; run Ghostlight's install from a regular terminal". | see `git log` |
| c11-client-by-agent | `ghostlight install --client` and `ghostlight uninstall --client` take an agent's catalog id as well as a host's harness id (decision 8b). `HarnessSummary::named_by` says how a value names a host, ignoring case: `ClientName::Host` for its harness id, `ClientName::Agent` for its agent's id (`agent.id`) when that is not the host's own. `HarnessSummary::is_selected_by` is the one rule: a harness id selects exactly its own host in any state, as before, and an agent id selects the agent's hosts that are detected (state other than `NotDetected`), or all of them with `--all-clients` (`install::ALL_CLIENTS_OPTION`, which the parser now matches by name), so `--client cline` with only the Cline CLI present selects cline-cli alone and writes nothing into an editor the person does not have (fixup after review). `select_harnesses` keeps the order the values were given and each host once; a value that names no host is refused with every valid id (`language::deployment::unknown_client`: "unknown MCP client 'x'; expected an agent id (codex, ...) or a host id (cline-cli, ...)"), and an agent id none of whose hosts is detected is refused naming its host ids and `--all-clients` (`language::deployment::no_client_host_found`). `install::client_ids` builds the valid values from the roster: every agent id once, then the harness ids no agent id names (the four Cline hosts). `ghostlight --help` lists both under "Client ids", wrapped from that list at run time, and its `--client` line says "Select an agent's detected hosts, or one host (repeatable)". The bash, zsh and fish completions declare the same ids on one fixed-shape line each and complete them after `--client` for install and uninstall (zsh's `--client` is repeatable); the manual page says what `--client` takes and lists "Agent ids:" and "Host ids:", and its uninstall synopsis names `--client`. Tests: every catalog id names its agent's hosts in any case and every harness id its own host (`install`); in a test home with only the Cline CLI's file, `cline` selects cline-cli alone and with `--all-clients` all four, while a harness id selects its host whatever its state (`install`); the CLI selects by agent and by host in the order given, once each, selects only detected hosts for an agent id unless `--all-clients` is given, refuses an agent with no detected host naming its hosts, and refuses an unknown value naming every id; the completions, the manual page and help each list exactly `client_ids` (`cli::parse`). docs/guides/installation.md says it in one sentence. Golden: the writer golden shows zero diff (it never goes through CLI selection, and a selection by harness id names the same hosts as before). | see `git log` |
| c12-agent-lists | The public lists name every agent, from the catalog, and say Windows and Linux plainly (decision 8c and 8d). docs/guides/installation.md has a new section, "Agents Ghostlight sets up", whose table is generated from `install::ROSTER` by the test `install::agent_list` between `<!-- BEGIN agent list -->` and `<!-- END agent list -->`: the count ("20 agents on 23 hosts"), then one row per host with the agent (its catalog id beside the name when no host has that id: "Cline (`cline`)"), the host id, the target, the entry (the key path read from the setup document Ghostlight shows for the host's dialect: `mcpServers.ghostlight` (JSON), `mcp_servers.ghostlight` (TOML), both OpenCode forms, or "Copy setup (YAML)" for goose and Continue), and where its file lives on Windows and on Linux, written with the variables a person reads (`%USERPROFILE%`, `%APPDATA%`, `~`, `$XDG_CONFIG_HOME`): a variable that names the file first ("`$CRUSH_GLOBAL_CONFIG` when CRUSH_GLOBAL_CONFIG is set, else ...", the same for CODEX_HOME), then the files the agent reads in its order ("the first found of `kilo.jsonc`, `kilo.json`, `opencode.jsonc`, `opencode.json`; else a new `kilo.json`"), and one generated sentence with the roots' fallbacks. The test compares the whole guide and fails when the table differs from the roster; `GHOSTLIGHT_AGENT_LIST_UPDATE=1` rewrites it (AGENTS.md lists it). The hand "Configuration family" table (11 agents) is gone. To generate the candidates from the catalog they became roster data: `AgentRow.candidates` (`OPENCODE_FILES`, `KILO_FILES`) lists the files an agent reads beside its default `path`, `AgentRow::file_on` picks the first that exists, and `HarnessDefinition::accepts_config_path` reads the same list instead of restating the OpenCode and Kilo names by id literal (the survey's Locate duplicate); the path closures no longer touch the filesystem. Crush's `CRUSH_GLOBAL_CONFIG` is read once into `HarnessContext.crush_config` instead of inside its closure, so tests no longer see the developer's variable, and the writer golden's refusal to run while it is set is gone. The variables the harness roots read are named constants. README ("Your first five minutes" and the sentence that listed 9 agents) and llms-install.md section 1 keep one sentence and link to the guide's table, ACCEPTANCE item 8 refers to the catalog instead of naming 10 agents, and README, llms-install and the guide each say "Agent setup supports Windows and Linux; macOS is not supported yet." Tests: the guide's table equals the roster's; every host with candidates reads the first that exists, in order, otherwise its default, and Locate accepts exactly those names; Crush reads the file the context names. Golden: the writer golden shows zero diff (no file under `tests/golden/` changed). | see `git log` |
| e01-icon-cache | The catalog says where each agent's own icon may be found, the service caches it, and the agent's symbol publishes it (decision 7). Each roster row has `icons`, its sources in order (`install::icons::IconSource`: `WindowsExe` with program places, either a command on `PATH`, resolved through a `.cmd` or `.ps1` launcher to the program it starts, or a path below `%LOCALAPPDATA%` or `%ProgramFiles%`; `WindowsPackage` by package name; `LinuxDesktop` by desktop file ids), and `icon_family`, the agent whose icon stands for it when none of its own is found (Claude Code: Claude Desktop). Sources are named for Visual Studio Code, Cursor, Windsurf, Kiro, Antigravity, Zed, Claude Desktop and Codex. `install::icon_catalog` projects them per agent. One resolver, `install::icons::IconCache`, runs at the service's start (`ServiceHost::start_waiting`) and on Re-check (`refresh_harnesses`), on its own thread, bounded to 5 seconds, never inside an MCP call. It tries the sources in order and keeps the first icon in `state/icons/<agent id>.<png or ico>` with `<agent id>.json` beside it (the source path, its size and modification time, the copy's SHA-256), written with the home's private-file writer (`record::atomic_replace`); a source is read again only when its file changed, a copy whose source is gone is dropped, a copy whose bytes its manifest does not vouch for is not offered, and a source that cannot be read is skipped with an `agent_icon_skipped` diagnostics line (the agent and the reason, no path). Linux: the desktop entry is found in `XDG_DATA_HOME`, `XDG_DATA_DIRS`, the Flatpak exports and Snap's directory; its `Icon=` is an absolute PNG or a hicolor theme name (128, 96, 64 and 48 px, then `pixmaps`, then 256, 512, 32 and 24 px), PNG only, checked by its signature and at most 1 MiB. Windows: the program is found, its icon is not read yet (e02). The service publishes the cache once (`icons::publish`); `Agent::listed` gives `{ "src": "agent-icons/<id>?v=<first 12 of the SHA-256>" }` while a copy is cached, else the monogram tile, so the pill, the shelves, the card, the rail and the pane draw it unchanged, a watcher too. A changed icon looks at the roster again and tells every screen. The window serves `GET` and `HEAD /agent-icons/<id>` for catalog ids only, as `image/png` or `image/x-icon`, with `nosniff`, `default-src 'none'; sandbox`, an ETag and `no-cache`; anything else there is not found. data-flows (a flow row, a `state/icons` artifact, the Linux private files), PRIVACY (a bullet) and ARCHITECTURE (a paragraph and the state directory) say what is read, that a copy is kept, and that nothing is sent. Tests: the first source that gives an icon wins; the family stands in and nothing found leaves the monogram; a changed source refreshes its copy and a gone one drops it; the manifest's fields and a reopened cache; Linux resolution against a temp XDG tree (preferred size, pixmaps, an absolute PNG, never an SVG, the Desktop Entry group only); a non-PNG is skipped for the next source; the XDG directory order; launchers on `PATH` (VS Code's `code.cmd`, an npm shim naming Node, a PowerShell launcher, a plain `.exe`) and known folders; the route (served with its headers in every context, 304, HEAD, unknown and uncached ids refused, POST refused); the catalog's families. Golden: no file under `tests/golden/` changed. | see `git log` |
| e02-icon-windows | Windows sources give icons (decision 7). A program's icon: `ghostlight_win_peer::program_icon` (new `crates/win-peer/src/program_icon.rs`) maps the program with `LoadLibraryExW` as a data file and image resource, takes the name of its first `RT_GROUP_ICON` through `EnumResourceNamesW`, copies the group and its `RT_ICON` images (`FindResourceW`, `SizeofResource`, `LoadResource`, `LockResource`) and writes a standard `.ico` file in memory: one image per size, the deepest colour of each, largest first, at most 4 sizes, each a PNG or a bitmap, within the cache's 1 MiB bound. A program with no icon group gives none; a file that does not map as a program is an error the resolver skips with its diagnostics line. The module is freed by its guard, and every unsafe call has a SAFETY note. A Store app's install folder: `ghostlight_win_peer::package_install_folders` (new `crates/win-peer/src/package.rs`) asks `GetPackagesByPackageFamily` for the family's full names and `GetPackagePathByFullName` for each folder, two kernel32 calls, no WinRT; the full names are read as offsets into the buffer the call filled, never through its pointers. In `install/icons.rs` the resolver now runs both: `Located::Program` reads the program's icon as `IconKind::Ico`, and `IconSource::WindowsPackage` finds the families of its package name below `%LOCALAPPDATA%\Packages` (`<name>_<publisher id>`, ignoring the name's case; `container::PRIVATE_DATA_PARENT` and `PACKAGE_FIELD_SEPARATOR` are shared, not restated), asks Windows for their install folders (`IconPlaces.packages`, `PackageFolders`, set on Windows only), reads each `AppxManifest.xml` (at most 512 KiB), takes the first app's `VisualElements` `Square44x44Logo`, else `Square150x150Logo`, and picks the logo's best PNG variant beside it by its file-name qualifiers (`scale-N` of the attribute's size, `targetsize-N`; the smallest of at least 96 px, else the largest; `altform-unplated` before plain before other forms; never `contrast-*`). A logo path with `..`, a drive, an entity or an empty part names nothing. Shims are e01's: the launcher resolves to the program, and the icon is that program's. ARCHITECTURE's icon paragraph, data-flows' Agent icons row and PRIVACY's icon bullet say what Windows reads. Tests: notepad's icon is a valid `.ico` with its largest images first and within a byte limit, this test executable (no icon resource) gives none and a text file is an error, the group's selection and the file's layout on synthetic bytes, an uninstalled or malformed family has no folder, a name is read only inside its buffer (win-peer, Windows); the fixture manifest's logo and tile, logos outside an app ignored; the variant choice over a temp folder of qualified files; a logo path never leaves the install folder; family names; a Store app's logo cached from a fixture install folder through a test lookup; notepad on a test `PATH` cached as an icon file and not read again (icons). No test reads an agent's install. Golden: no file under `tests/golden/` changed. | see `git log` |
| e03-icon-polish | The icons look right everywhere and the catalog says what it found (decision 7). A cached icon's manifest also names `app`, the catalog id of the agent whose app gave it (the agent's own, or its family's), and `found_in`, the kind of place it was read from (`install::icons::IconSourceKind`: `program`, `store_app`, `desktop_entry`, from `IconSource::kind`); a manifest whose app is not in the catalog is not offered. `AgentIconSources` carries the agent's name. `IconCache::found` gives a `FoundIcon` (the version, the app's id and name, the kind), `IconCache::agents` lists every catalog agent with what is cached for it, and `icons::published` replaces `published_version`. The symbol has three forms: `Symbol::Tile` (`{ "src": "integrations/<file>" }`, the monogram, renamed from `Image`), `Symbol::Icon` (`{ "icon": "agent-icons/<id>?v=<version>", "about": "Icon from the installed <app> app." }`) and `Symbol::Glyph`; the sentence is `language::agent_icon::icon_from`, so Claude Code's says Claude Desktop. The page: `symbolMarkup` draws an icon as the tile's `img` with the class `symbol-icon`, and `symbolAbout` gives the pane its line. CSS (classes only): every symbol image is `object-fit: contain`; an icon sits on the neutral `--icon-tile` (`#e4e4e7`) with 1 to 5 px of room by size, inside the same rounded outline the monogram has at that place (a circle in the summary face, a rounded tile elsewhere). The Integrations pane shows `<p class="agent-icon-about">` under the state, muted, only for an icon. `ghostlight doctor` opens the home's icon cache read-only (it looks at no app) and prints one `Agent icon: <agent> -- from the installed <app> app (<Windows program, Microsoft Store app or desktop entry>)` line per found icon, then `Agent icons: none found for <names> -- they show their monograms`, or `Agent icons: none found -- every agent shows its monogram`; `--json` has `agent_icons`, one `{ agent, found, app, source }` per catalog agent (`app` and `source` only when found), no paths. The preview server serves `tests/fixtures/workbench/agent-icon.png` (48 px, synthetic) as Codex's icon at `agent-icons/codex`, so Codex wears it in the pill, its chip, its card, the rail and the pane beside the monograms. ARCHITECTURE's icon paragraph and data-flows' `state/icons` bullet name the new manifest fields, the symbol's two image forms and doctor. Tests: the manifest's `app` and `found_in`, a family's icon naming the family's app, `agents` in catalog order, a cached `.ico` found as a program and nothing for an id outside the catalog (icons); the symbol's icon form and a family's sentence (agent); the pane sentence and doctor's lines (language); doctor's JSON (doctor); the surface test draws Codex's icon in the face, chip and card beside glyphs, and Windsurf's on the rail and in the pane with its one `about` line, while Zed's monogram pane says nothing. Golden: no file under `tests/golden/` changed. | see `git log` |
| f01-icon-endpoint | One image address per agent; the service decides which image it serves there; the page asks and draws (decision 9). `crate::agent::image` holds the one resolution order: `AgentImage::best` gives an agent's app icon when the cache holds one (`AgentImage::AppIcon`), else its packaged monogram (`AgentImage::Monogram`); a curated vendor mark has its place between the two in the module's doc and none is packaged. `AgentImage::version` is the first 12 hex of the SHA-256 of the image served (`install::icons::version_of`, which the cache now uses too), and `about` is `language::agent_icon::icon_from` for an app icon only. `served` gives the window that image (`ServedImage`: media type, bytes, version) for a catalog id, and falls back to the monogram when a cached copy can no longer be read. The symbol has two forms: `Symbol::Icon { address, about }`, serialized `{ "icon": "agent-icons/<id>?v=<version>" }` with `"about"` only for an app icon, and `Symbol::Glyph`; `Symbol::Tile`, `TILE_DIRECTORY` and the `src` key are gone. `Agent::listed(id, name, monogram)` still builds every roster agent, reading the cache the service published (`install::icons::published()` now gives the cache itself). The monogram tiles moved from `ui/integrations/` to `crates/orchestrator/src/agent/monograms/` (with `PROVENANCE.md`); each roster row embeds its tile (`monogram!("codex.svg")`, a `Monogram` over `include_bytes!`), `install::monogram_of` finds an agent's tile by its catalog id, and the 19 hand-written `integrations/*.svg` rows and `AssetType::Svg` left `workbench/web/assets.rs`, so `/integrations/<file>` is not found. The route: `GET` and `HEAD /agent-icons/<id>` answer every catalog id through `WorkbenchFacade::agent_image` (the attached cache), as `image/png`, `image/x-icon` or `image/svg+xml`, with `nosniff`, `default-src 'none'; sandbox` (`IMAGE_POLICY`), `no-cache` and an ETag of the version; any other id is not found, and the Closed context answers `watching_off` for every id the same way. The page: `symbolMarkup` emits `<img data-icon="<address>">` with no source for every agent, and adds the class `symbol-app-icon` (`APP_ICON_CLASS`) only when the symbol carries `about`; `symbolAbout` reads `about`; `ui/lib/icons.js` fetches each address once with the page's key and fills every image naming it, and a failed load leaves the image on its tile with no notice (`NOT_LOADED`, `onFailure` and their wiring in `app.js` are gone). CSS: every agent image has its place's rounded outline; an image with no source yet (waiting or failed) shows `--icon-tile` (`.symbol img:not([src])`), an app's icon keeps the tile with e03's 1 to 5 px of room (`.symbol-app-icon img`), and a drawn monogram sits on nothing. The preview server serves `/agent-icons/<id>` for every fixture agent (Codex's synthetic PNG, the others' monograms from the new folder) with versions from their bytes, and no `integrations/` files. ARCHITECTURE describes the endpoint and the resolution order; data-flows and PRIVACY stay true unchanged. Tests: a monogram for an agent with no cached icon and the cached icon when present, the version following the served image, no image for an id outside the catalog, every tile an agent's and free of script, links and handlers (agent image); the symbol forms with `about` only for an app icon (agent); every roster row's symbol is its agent's address at the monogram's version (install); the route serves the monogram, then the cached icon with a new ETag, 304 on the current tag, HEAD with GET's headers, 404 for ids outside the catalog and for `/integrations/zed.svg`, and Closed refusing every id identically (web); the page draws an app icon and a monogram the same way, only the app icon marked for its tile, and a failed load is asked once and leaves no source (icons); the stylesheet paints the tile under a waiting or failed image and an app's icon and under no loaded monogram, and styles no symbol class the page no longer draws (settings); every part's keys classify `agent.symbol.about` for both audiences, from fixture agents that wear an app icon (parts); a monogram is drawn from `image/svg+xml` (transport); the surface draws every agent by address in the pill, shelf, card, rail and pane, with no `src`. Golden: no writer byte changed; the reviewed diff is the `agent:` line of each host file (69 lines, three in each of 23 files), from `{"src":"integrations/<file>"}` to `{"icon":"agent-icons/<agent id>?v=<version>"}`. | see `git log` |
| t01-test-temp-cleanup | Tests leave nothing in the temporary folder (owner request of 2026-10-17 to reclaim disk space; about 20,000 of the 23,700 entries in the owner's temp folder were left by Ghostlight's tests). Each crate has one test-only guard, `Scratch`: a fresh directory under the temporary folder, made with `create_dir` so it never reuses an earlier run's leftovers, and removed with everything in it when the guard drops, also during a panic unwind. A small guard per crate was chosen over the `tempfile` crate: it adds no dependency or `Cargo.lock` change, and the relay and edge manifests are locked to the published relay revision, so their guard lives in their root `tests.rs`, which the lock leaves out. The service's guard (`crate::scratch`, moved from `install::scratch`) also gives `Scratch::file(name)`, a file path alone in its own directory, so siblings a write leaves (an atomic-replace partner, a backup, a renamed copy) go with it; `Scratch` derefs to `Path`. Every test helper and test that named a temporary path uses it: the recovery policy, the diagnostics, harness, native host, governance, work, workbench, audit, install, service and relay tests, and `TestHome`, the writer golden's run and the web fixture hold one. Helpers that built a context return the guard beside it. Explicit removals at the end of a test are gone; a removal in the middle of a test became `drop(guard)` at the same moment. Spawn tests start their child in the crate directory instead of the temporary folder. Scratch names start `ghostlight-test-<crate>-`. Each guard has a unit test that the directory is gone after the drop and after a caught panic. Node: `createHome` removes a home whose executables cannot be installed, and the `cdp-pipe` and `home` helper tests remove their homes in `finally`. Counts, by listing the names under `%TEMP%` (which is `std::env::temp_dir()` and `os.tmpdir()` here) before and after each run and taking the names that are new: before, 387 entries start with `ghostlight-` (24,242 in all). After `cargo test --workspace` (all passed): 0 new `ghostlight-` entries. After `node --test tests/guards/`, `npm test` in `extension/`, the `tests/lib` helper tests, the workbench tests and the page-asset tests (all passed): 0 new. The one other new name in each run was a `<guid>.tmp` file of another program, one of 213 such names, which came and went on its own. No product behaviour or assertion changed; no golden changed. | see `git log` |
| t03-icon-tile | The icon endpoint serves each app icon as a finished tile, the icon on the ground the service chose for it, so every agent image the page draws is whole like a monogram and the page decides nothing about grounds (decision 10). `crate::agent::image::tile` (new) holds the rule and its closed vocabulary `Ground` (`Dark`, `Light`, `Plate`; not on the wire). It decodes a cached PNG with the `png` crate (the orchestrator gains that dependency edge; the crate was already in `Cargo.lock` for `crates/desktop`) or the largest image of a cached `.ico` by its directory (an embedded PNG, or a 32-bit BGRA bitmap read by hand, its one-bit mask giving the transparency when its alpha is zero everywhere). An icon whose outer ring (4 percent of its side, at least 1 px) is on average at least 90 percent opaque is its own plate and is served exactly as cached. Otherwise each pixel is compared, by WCAG contrast, with the dark ground `#3a3a41` and the light ground `#e7e5e4`, and the ground under which more of the icon's alpha-weighted ink reaches 3:1 wins; a tie goes to light. The tile is a square of the ground 10 percent of the icon's side wider on each edge (at least 1 px), with the icon's pixels copied at their own size into its middle with source-over blending in integer arithmetic, encoded as an opaque RGB PNG with fixed settings (`Compression::Balanced`, `Filter::Adaptive`), so the same icon always gives the same bytes. An icon that cannot be decoded, or is larger than 512 px on a side, is served as cached and never fails the request. `AgentImage::AppIcon` now carries the `FoundIcon` and the `ServedImage` made from that copy; the image is made once per cached copy, keyed by the copy's SHA-256 (`FoundIcon.sha256`, in place of its 12-character `version`), and kept in memory (at most 64, then emptied). `AgentImage::best` makes or finds it, so the symbol's version and the route's bytes and ETag come from the same value and cannot disagree; a copy that can no longer be read, or whose bytes no longer match its SHA-256, gives the monogram on both. A tile is served as `image/png`; its version is the hash of the tile. The page: `symbolMarkup` draws an app icon exactly as a monogram (`APP_ICON_CLASS` and the `symbol-app-icon` class are gone); the stylesheet's only ground is the placeholder under an image still loading or failed (`.symbol img:not([src])`, `--icon-placeholder: #3a3a41`, the service's dark ground, which replaces `--icon-tile`), and the app-icon tile and its 1 to 5 px padding rules are gone. `about` stays for the Integrations pane's sentence. The wire is unchanged. The preview server serves the service's tiles: `tests/fixtures/workbench/agent-icon-tile.png` (the colored fixture on the light ground) as Codex's icon and `agent-icon-white-tile.png` (a new synthetic all-white ring and bar on transparent, `agent-icon-white.png`, drawn by the test) as Claude Code's, so the preview shows one tile on each ground; the service test makes all three again and compares (`GHOSTLIGHT_TILE_FIXTURES_UPDATE=1` rewrites them; AGENTS.md lists it). ARCHITECTURE's icon paragraph says the endpoint serves an app icon as a tile on a chosen ground. Tests: a white glyph gets the dark ground and a black one the light; red, blue and violet glyphs the light ground and a yellow one the dark; an icon with no ink is a tie and gets light; an opaque full-bleed square and one with slightly rounded corners are plates served as cached, a disc is not; an unreadable PNG, an empty one, a non-icon file, a 513 px PNG, a palette bitmap and a truncated icon file are served as cached, while 512 px is tiled; an `.ico` with an embedded PNG, one with a 32-bit bitmap, and one whose bitmap takes its transparency from the mask all read their largest image; the tile's size (40 px gives 48), its corner and center pixels and the place of the icon's edge; a half-transparent pixel's blend; byte-identical output for the same input (tile); the route of a cached app icon serves the tile, the version follows the tile and not the copy, the symbol names the tile, and a copy that changed under the cache gives the monogram on both, before its tile was made and after, when it is replaced or deleted (image); the preview's tiles are the service's, one on each ground (image); the route serves `image/png` with the fixture tile's bytes and the tile's ETag (web); the page draws an app icon and a monogram the same way (icons, settings), and the stylesheet has no ground or padding rule for an agent image but the placeholder (settings). Golden: no file under `tests/golden/` changed. | see `git log` |

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
- d04-integrations: open Integrations in the window, at 1200 and at 800 px wide. The header reads
  "Ghostlight is in all N of your agents." (or "N of your M agents.") with Re-check, Set up
  everything and Copy MCP command in one row and the command shown small under them; with
  something to set up the caption says what (for example "Will connect Cursor."), and with nothing
  it says "Nothing to set up." and the button is off. The rail shows Needs a look only when
  something does, then Connected and Found, and one row "N more agents Ghostlight can join" that
  opens in place and scrolls into view; typing in the search (or pressing /) finds a folded agent.
  The page opens on the agent that needs attention, else a found one, else the one with tabs.
  Codex and Claude Code in use show "N sessions, M tabs" in teal with a breathing dot (still with
  reduced motion on), and their pane says how many sessions are connected with "See them in At a
  glance", which opens their card. Remove from Claude Code asks first and names the live sessions;
  Cancel changes nothing. Continue (folded) says "Copy setup only"; goose found sits under "Found,
  set up by hand" with a dashed dot, its pane says Ghostlight cannot tell whether goose has the
  entry, and the headline counts "of the N agents it can check". Both offer Copy setup, never
  Connect. Start a goose session: goose moves to Connected with the teal dot, without a reload.
  Start a Codex session while the page is open: its rail line, pane live line and Remove's
  question name it without a reload. Only the rail and the pane scroll, and Ghostlight's entry is shown whole. A watching
  browser tab shows the same page with Copy setup and Install as plain controls and the one
  Continue line.
- d05-design-tidy: the window's search finds the page as "Integrations". A session whose client
  sent no name shows one "Unknown agent" chip on the At a glance shelf, never a blank one.
- d08-catalog-finish: CI's browser lane passes `history-browser` (`tests/workbench-history-browser.mjs`),
  which was ported here and not run (note 1).
- d07-agent-catalog: with Codex, Claude Code and a `ghostlight call` connected, each agent wears the
  same tile in the summary pill, on its shelf chip, in its card's head and on the Integrations rail
  and pane (Codex's CX, Claude Code's CC, never initials or the spark); a working agent's face has
  a teal ring and its chip's tile breathes (still with reduced motion on). The command line shows
  the prompt glyph in the pill, on its chip and in its card. A session whose client sent no name
  shows the unknown glyph (a circled question mark) beside "Unknown agent", and a restored MCP row in
  Activity reads "Unknown agent" with the workspace id on hover. A watching browser tab shows the
  same faces.
- c00-writer-golden: CI's Linux test job passes `install::writer_golden` against the files recorded
  on Windows (`writes/` and `native/linux.txt`), and compiles the Linux-only Codex tests this slice
  edited (note 4). Nothing changes for
  a person, so the deployed window needs no look for this slice.
- c01-download-per-host: in the deployed window, Integrations, the "not on this computer" agents:
  Install on Cline (Visual Studio Code) opens the Visual Studio Marketplace page of the Cline
  extension, on Cline (Cursor) and Cline (Windsurf) the Cline installing page, and on Cline (CLI)
  the Cline CLI page. A watching browser tab's Install link goes to the same pages.
- c02-setup-view-rules: with setup incomplete and a browser connected, the setup view's agent step:
  goose or Continue, when found, reads "Found, set up by hand" with Copy setup, and the copy pastes
  the same entry as Integrations; an agent with an older entry reads "Needs an update" with Update;
  with only Cline CLI set up, a Cline CLI session turns Cline (CLI) Connected and leaves the other
  Cline hosts as they were, in the setup view and on Integrations alike; with Cline CLI and Cline
  (Cursor) both set up, one Cline CLI session turns both Connected (note 7), and the Cline hosts
  without the entry stay as they were. Integrations still pulses Codex and Claude Code while their
  sessions are live, and stops when they end.
- c03-private-view-acts: from a terminal inside the Codex app (its container), with Claude Desktop
  installed: the window's Integrations pane for Claude Desktop offers no Connect, Update, Fix or
  Remove, and still offers Locate and Copy setup; Set up everything does not name Claude Desktop in
  its caption and reports no failure for it; `ghostlight doctor --json` shows Claude Desktop's
  detail as the private sentence; `ghostlight install` there still names "Claude Desktop MCP client"
  among the registrations kept private, and `ghostlight uninstall --dry-run` there says Claude
  Desktop's entry stays in place and names `uninstall` from a regular terminal. From a regular
  terminal nothing changes.
- c11-client-by-agent: in a terminal, `ghostlight --help` shows "Client ids" with the agent and
  host ids; `ghostlight install --client cline --dry-run` says one "Would install" line for each Cline
  host installed on the machine and none for the others (`--client cline --all-clients --dry-run`
  says one for all four; on a machine with no Cline host, `--client cline` refuses, naming the four
  host ids and `--all-clients`), `--client CODEX --dry-run` the Codex line alone, and `--client nope` refuses, naming
  the ids. On Linux with the completions installed, `ghostlight install --client <Tab>`
  offers the ids in bash, zsh and fish (only the bash file was syntax-checked here; this machine
  has no zsh or fish), and `man ghostlight` shows the two id lines.
- e01-icon-cache: on a Linux desktop with Zed or Visual Studio Code installed from a package, the
  Snap or the Flatpak, the deployed window shows that app's own icon for the agent in the summary
  pill, on its shelf chip, in its card's head and on the Integrations rail and pane, and a watching
  tab shows the same; `~/.ghostlight/state/icons` holds `<agent id>.png` and `<agent id>.json`
  (closed to other users); an agent with no app keeps its monogram. Removing the app and pressing
  Re-check brings the monogram back without a reload. Windows has its own check under e02. CI's
  Linux test job runs the icon tests (they ran here on Windows only, with Linux sources looked at
  through a test computer).
- e02-icon-windows: on the owner's Windows machine, after the deploy, the window shows the apps'
  own icons in the summary pill, the shelf chips, the card heads and the Integrations rail and
  pane: Codex and Claude Desktop their Store logos, Claude Code the Claude Desktop logo (its
  family), Visual Studio Code the icon of `Code.exe` behind `code.cmd`, and Antigravity the icon of
  its `Antigravity.exe`; the npm CLIs with no app of their own keep their monograms.
  `~/.ghostlight/state/icons` holds `codex.png`, `claude-desktop.png`, `claude-code.png`,
  `vscode.ico` and `antigravity.ico` with their manifests (a pre-deploy look through the resolver on
  this machine found exactly these five, the two `.ico` files four images each, 256 px first). The
  `.ico` icons look sharp at the pane's size in the light and dark themes. A second Re-check
  changes nothing. CI's Windows test job passes the win-peer icon and package tests, and the Linux
  job builds the service and win-peer (only win-peer could be checked for Linux here; the service's
  Linux check needs a Linux C compiler for `ring`, which this host lacks).
- e03-icon-polish: in the deployed window on the owner's machine, each found icon (Codex, Claude
  Desktop, Claude Code, Visual Studio Code, Antigravity) sits whole on its light tile in the summary
  face, the shelf chip, the card head, the rail and the pane, the same size and outline as the
  monograms beside it, never stretched, with keyboard focus and reduced motion unchanged; the pane
  says "Icon from the installed Codex app.", Claude Code's says Claude Desktop, and an agent with a
  monogram says nothing there. `ghostlight doctor` lists those five as found (Claude Code from the
  Claude Desktop app, Microsoft Store app; Visual Studio Code and Antigravity Windows programs) and
  names the rest as showing monograms; `ghostlight doctor --json` has `agent_icons` with the same
  facts and no paths. The first look after the deploy rewrites the manifests e02 wrote without the
  new fields (they are not offered until then, so the monograms may show for up to one look).
- f01-icon-endpoint: in the deployed window on the owner's machine, every agent shows the endpoint's
  image in the summary face, the shelf chip, the card head, the Integrations rail and the pane: the
  apps' own icons where e02 found them (Codex, Claude Desktop, Claude Code, Visual Studio Code,
  Antigravity) whole on the rounded light tile with a little room, as e03 drew them, and monograms
  for the rest, in the same outline with clean corners on the dark window (no light flecks at the
  corners of a chip or the rail). No image is empty and no "An agent's icon could not be loaded"
  notice appears, also after Re-check and after turning "Others can watch" off and on. `http://127.0.0.1:<port>/agent-icons/zed` in a watching browser
  tab shows Zed's monogram (or its icon), and `/integrations/zed.svg` is not found.
- t03-icon-tile: in the deployed window on the owner's machine, Codex, Claude Code, Claude Desktop,
  Visual Studio Code and Antigravity each show their app icon on a ground that suits it (a white or
  light icon on the dark gray, a dark or colored one on the warm off-white) or on its own plate, in
  the summary face, the shelf chip, the card head, the Integrations rail and the pane. A white icon
  is readable, no icon sits on a muddy mid gray, and the corners of every tile are clean on the
  dark window. Monograms look as before. An image still loading shows the quiet dark placeholder.
  CI's `cargo deny check licenses bans sources` and `cargo audit` pass with the orchestrator's new
  `png` edge (neither tool is installed here; the crate was already locked for the desktop crate).
- c12-agent-lists: CI's Linux test job passes `install::agent_list` (the table is built from
  placeholder roots, so it is the same on both systems; it ran here on Windows only). On GitHub the
  installation guide's "Agents Ghostlight sets up" table renders with its six columns, and the
  README and llms-install links reach it. Nothing in the product changed for a person.

## Notes

- 2026-10-17: the owner's machine was forced off by a driver freeze while the design batch build ran. After the reboot one service and one shell started by themselves, the browser and the MCP session reconnected, and no state was lost. A forced power-off does not test the shutdown path; that check stays owed for the next ordinary restart (release bar).

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
- d04-integrations: differences from the spec and the mockup. (1) Live sessions per agent come
  from the service's own naming: each session's `agent` is the roster's product name (d00), the
  rule the setup view uses (`is_agent_of`), so the page counts the sessions whose agent is the
  row's name and their tabs and decides nothing; no new fact was added for it. A Cline session
  therefore counts on every Cline row, as d00 notes for the setup view. (2) Copy-only is a service
  fact, `HarnessSummary.copy_only` (true for the manual dialects, Continue and goose), carried in
  the integrations part; the page uses it for its words and for Set up everything's plan. (3) The
  pane's sentence is the page's own words by state. For an agent that needs attention it is the
  page's sentence when the service gives evidence (a foreign entry) and the service's `detail`
  otherwise (a malformed or unreadable file), so the cause is never guessed. The service's other
  details (the missing Ghostlight program, a located path that is gone, a retired connector) are
  not shown in the pane; `ghostlight doctor` and `ghostlight setup` still print them. (4) Locate is
  described as what it does here, choosing the agent's file or program, not the mockup's "Opens
  File Explorer". For an agent that needs attention and can be fixed, the next step is Fix
  (confirmed, backed up) instead of the numbered steps; the steps are Copy setup and Re-check, and
  Locate stays on the settings file row. (5) The rail's group heads are plain headings: only the
  fold opens and closes. The pick is not written to the address. The caption carries no "Checked
  just now", because the page has no scan time from the service, and Set up everything adds no
  dialog: the caption says beforehand what it will do and the result toast says what it did. The
  mockup's scope line ("For your Windows user") is left out, since the target words are the
  service's. (6) The window's minimum width is 720 px, so the mockup's stacked layout below 700 px
  is not carried. (7) Remove's and Fix's questions moved into `ui/lib/words.js`; Fix's names the
  agent and its file. (8) The preview server shows every state (Cursor needs an update, goose found
  and copy-only, Continue not on this computer and copy-only, Claude Code connected and in use),
  and `?agents=connected` shows the all-connected headline. (9) ADR-0129 Decision 1 (the card
  roster) gets a marked amendment for the 2.0 window and its index row says so; ACCEPTANCE no
  longer says Windsurf and Devin share a card; the installation guide says remove instead of
  Disconnect. (10) The review fixup adds a place the mockup did not have: Ghostlight never reads a
  copy-only agent's file, so a found one with no live session stands `by_hand` (`agentStanding` in
  `ui/lib/words.js`), in its own rail group "Found, set up by hand" after Found, with a dashed dot
  and wire. It is neither connected nor not connected, is left out of the headline's count (the
  headline then says "agents it can check"), and Set up everything leaves it to the person. A
  copy-only agent with a live session stands connected; the page offers it no Remove, since
  Ghostlight never edits its file. A needs-attention agent with no evidence (a file Ghostlight
  could not read or parse) gets the steps "Correct the file yourself, so Ghostlight can read it"
  (no button) and Re-check.
- d05-design-tidy: differences from the spec. (1) The bounded label was already the chip's source:
  a workspace takes its label from the connection's bounded claim, so the fix there is a test
  (a name with a bidi character reads the same on the chip and the card). A client that reported
  no name now also has no reported application on its card, because "MCP client" was Ghostlight's
  word, not the client's; the browser's tab group still says "Ghostlight - MCP client".
  (2) No product id is matched: the only product ids that named an agent before were also that
  agent's harness id, so nothing that matched stops matching; `cline` names Cline by its name.
  (4) INTENT's sentence that the narrow tab row abbreviates the label is gone, since the label is
  the short one now. CHANGELOG, release notes and docs/history keep "MCP integrations" as written.
  (7) Two looks that miss at once may each read the installed browsers; neither waits on the
  other. Item 7 landed before item 6, so that RESUME HERE could say the batch has landed.

- d07-agent-catalog: differences from the spec. (1) The agent sits on each item the window reads,
  not inside the connection evidence: `SessionSummary`, `OperationSummary`, `HistoryItem` and
  `HarnessSummary` carry it, and `ConnectionDetails` no longer does. A session's connections are
  that session's agent, and a record that kept no evidence at all (refused before its workspace
  was known) had no evidence to carry a name, so the page had to say "Unknown agent" itself; now
  the service names it there too and the page decides nothing. (2) A client that reports a name
  Ghostlight does not know keeps that name (the d00 rule), with the unknown glyph and the id
  `unknown`; only a client that reported no name is "Unknown agent". The glance groups agents by
  name as before, so two different unknown names stay two chips. (3) A product on several hosts
  names the roster's first row: a Cline session's agent is `cline-cli`, because the reported name
  does not say the host; the rail still counts it on every Cline row by name (d04 note 1).
  (4) The Activity rows' agent cell has no mark: the client column is 116 px of 11 px mono, so a
  14 px tile leaves about 14 characters, and the intake column beside it already carries the prompt
  for the one exception, the command line. The intake mark draws that glyph from the same
  vocabulary. (5) No initials are kept as a last resort: every tile is packaged and served beside
  the page, and the name is beside the symbol everywhere except the pill, whose faces name the agent
  on hover. (6) `generic.svg`, the page's fallback for a row without an icon, is gone with its
  served entry, since every row names its monogram. (7) A running operation no longer borrows its
  session's agent: the service names it from the operation's own evidence, which every request
  carries. (8) A working agent's face gets the teal ring and glow whether it shows a tile or a
  glyph (a glyph face was only filled teal before). (9) `ghostlight doctor --json` lists each MCP
  client with `agent` in place of `icon`, the same summary the window reads. (10) The fixture of
  `tests/workbench-history-browser.mjs` uses the new shape; that journey still looks for the
  per-connection list d01 retired and was not run here.
- d08-catalog-finish: differences from the spec. (3) Grouping on the id alone keeps two agents
  Ghostlight does not know apart only if their ids differ, so such an agent's id carries the name
  it reported (`unknown:<name>`); the page still decides nothing and never reads a name to group.
  This replaces d07 note 2's id `unknown` for a named stranger. A Cline session's agent is now
  `cline` rather than `cline-cli` (d07 note 3), and `ghostlight doctor --json` lists the four Cline
  rows with that agent. A Cline session still counts on every Cline row (d04 note 1), now by id.
  (5) The preview's history rows kept their own evidence only in name before: every row was given
  its session's connection, so the evidence written beside each row was never shown. Each row now
  shows its own; the rows of live sessions read the same agents as before.
  (1) What changed in `tests/workbench-history-browser.mjs`, for the coordinator to watch in CI.
  The C1 part used to click `.session-connections > summary`, change the first session's agent to
  "Later application", and check that disclosure's text, focus and open state. Now, after opening
  the c1-receipt's per-action Connection details as before, it clicks `details.glance > summary`
  and waits for it to open, clicks `[data-glance-agent="codex"]` and waits for `#glance-card`, and
  focuses that chip. A first refresh (the same `audit_health_changed` publish) replaces the first
  session's connections with two claimed ones, "Later application" and "Another application",
  each with the service's claim sentence; the session's agent stays Codex, since a later connection
  joins the workspace without renaming it. It checks: the summary is open, the card's label is
  "Codex", focus is on the Codex chip and its `aria-expanded` is true, the trust note quotes
  "Another application", the first line's Copy id holds both connection ids, and the card does not
  say "from the command line". It then focuses the summary, refreshes to one "Latest application"
  connection, and checks focus stays on the summary, it stays open, the card is still Codex's and
  no longer says "Later application". The per-action checks follow unchanged (the receipt's
  details stay open, say "MCP client", and name neither later claim; the hero names Codex; no
  horizontal scroll), a new screenshot `h4-glance-card.png` is taken, and clicking
  `[data-glance-agent="command_line"]` must show a card labelled "Command line" whose first
  `.glance-line` says "from the command line". The restored receipt now sets the durable MCP
  sentence as its explanation, since the connection it copies carries a claim sentence. The
  closing C1 log line says what is checked now.
- c00-writer-golden: differences from the spec and what the golden leaves out. (1) The Linux
  forwarding was tied to the build OS in two places, not one: `edit_toml_with` passed `cfg!(windows)`,
  and `missing_codex_desktop_environment` itself refused unless `cfg!(target_os = "linux")`. Reading
  `context.windows` alone would still have kept the Linux forwarding out of a Windows build's record
  of the Linux computer. The context now carries `linux` beside `windows`, set once in
  `HarnessContext::system` from the build target, and every Codex path (inspection, the writer, the
  manual document) reads it. `edit_json_with` also compared paths with `cfg!(windows)`; it reads
  `context.windows` now. The writers take the context (`edit_json`, `fix_json`, `edit_toml`,
  `fix_toml`, `inspect_toml`), and the unit tests pass one; the tests' contexts set `linux` from the
  build target, so every existing test asserts what it did before. (2) The update variable is its
  own, `GHOSTLIGHT_AGENTS_GOLDEN_UPDATE`, so regenerating the model catalog never rewrites the
  freeze proof by accident. The directory is `tests/golden/agents/writes/`, leaving
  `tests/golden/agents/` for c08's generated catalog. (3) The third computer, Windows from inside
  another app's container, records absent, other and current only; it is there so c03's change to
  the offered acts shows as a diff (today Claude Desktop offers Install there and every act is
  refused). (4) One file holds on both build systems: every path is built with forward slashes, the
  edge sits in a home with a space (`Pat Doe`) so the command line's quoting is recorded, and paths
  are shown with forward slashes. The path separator is the one byte still tied to the build OS:
  a person's edge is a native path, so on Windows every file holds backslashes, as a JSON escape
  (`\\`) or a TOML literal string (`'...'`). The portable files cannot see that, and the unit tests
  only round-trip a backslash edge, so a second golden pins it per build system:
  `tests/golden/agents/native/windows.txt`, checked only on Windows builds, and `linux.txt`, checked
  only on Linux builds. Each records, for the first host of each way a file is written (one per JSON
  dialect, OpenCode and Codex TOML), the other seed, inspection with the manual setup document, and
  what Install says and writes, with paths built by the build system's own separator. Only the
  run's directory is hidden, as it is and as a JSON or TOML string escapes it, so the edge's
  separators and escapes below it stay in the file. The Linux-only Codex tests edited here could
  not be compiled on this machine (no Linux C toolchain for `ring`), and every file was recorded on
  Windows: `linux.txt` was recorded by spelling the Linux computer's paths with forward slashes,
  as a Linux build does, and matches the Linux sections of the portable files. CI's Linux job is
  the first run of both on Linux. (5) Left out on purpose:
  detection through `PATH` (the contexts list no directories), a located executable (its check reads
  Unix permission bits, which ties it to the build OS), a missing edge, and the CLI's and the
  window's own words, which come from `language/` and the workbench, not the installer. c05 extends
  the golden first with the Crush variable, the OpenCode and Kilo candidate files and XDG, as its
  slice says. (6) Crush reads `CRUSH_GLOBAL_CONFIG` outside the context, so with it set the test
  would write that real file; the test refuses to run while it is set and says why. (7) Today's bugs
  the golden records, for the slices that fix them: every Cline host's download is the Cline CLI
  page (c01); inside another app's container Claude Desktop offers Install although every act is
  refused (c03). Also worth knowing: the manual JSON documents and every newly written JSON entry
  list their keys alphabetically (serde's sorted map), not in the order a person writes them. (8) The test records the 23 hosts on one thread
  each and takes about 8 seconds.
- c01-download-per-host: differences from the spec. (1) `can_download` stays beside the new field,
  as the design's projection keeps it; the page still reads it, and the summary JSON gains one
  field. A host with no page has no `download_url` key, as before. (2) The writer golden prints the
  download line from the summary's field and asserts it equals what `HarnessRegistry::download_url`
  answers by the host's id, so the page the window opens is pinned too. (3)
  `tests/workbench-preview-server.mjs` is untouched: the preview server still carries no
  `download_url` (so a watching tab of the preview offers no Install link) and never served
  `/api/open`; the preview's hand roster is c12's and c08's to replace.
- c02-setup-view-rules: differences from the spec. (1) "Computed once per row in the workbench
  join" is `parts::in_use`, and the page receives it as `ConnectionsPart.harnesses_in_use` rather
  than as a field of each integrations row: the integrations part is read on demand behind a hint
  published only when the roster changes, while the connections part is published at each look
  with the sessions in use is decided from, so the list is never staler than the sessions beside
  it. (2) A copy-only row with a live session is in use whatever its detected state, as the page
  already drew it (`agentStanding`). (3) The setup view keeps its `private_view` check beside
  `can_install` until c03 makes `can_install` false under a private view; c03 can then drop it.
  (4) Update uses the `connect_agent` step (both write the entry through the same install act)
  with its own label and pending words, so no second write kind was added. (5) The page still counts
  an agent's sessions and tabs by agent id for its words ("2 sessions, 3 tabs") and for Remove's
  question; only whether a row is in use is the service's. (6) `tests/workbench-preview-server.mjs`
  gains a fixed `harnesses_in_use` (Codex and Claude Code), as the service would compute for its
  fixture; its hand roster is still c12's and c08's to replace. (7) The spec says one Cline CLI
  session never marks the other three Cline hosts Connected. That holds only for hosts without the
  entry. A session names its agent, not its host, because the client name it reports does not say
  which host runs it, so a session of an agent puts in use every host of that agent whose entry is
  Installed. One Cline session therefore leaves only the Cline hosts without the entry not in use.
  Telling hosts apart needs host evidence that no session carries today.
- c03-private-view-acts: differences from the spec. (1) The spec asks to fix the
  `AgentFact.private_view` doc comment. Once `can_install` is false under a private view the field
  had no reader but the setup view's redundant check, so the field is gone and the doc fix moved to
  `AgentFact.can_install`, which now says whose container it is (the service's). (2) The detail
  words are a function in `language::deployment`, beside the other client words an install says,
  and the refusal reuses them, so the refusal gained the detail's ending (". To change it, ..."). (3)
  `ghostlight install` selected clients by `can_install`, so without a change it would have stopped
  naming the private clients it leaves; `select_harnesses` keeps them, by state, through
  `HarnessState::takes_install`, the rule inspection uses. (4) `setup_detected` decided its targets
  by state and never read the flags, so the window's Set up everything would still have tried a
  private target and reported its refusal; it now skips private targets. Its other rules (state,
  not copy-only) are unchanged, so a missing edge still reports failures there as before. (5) The
  Integrations pane writes its own sentence for a found agent and does not show `detail`, so a
  private agent's pane says it is found and offers no change without the reason; the detail reaches
  `doctor --json`, the golden and any surface that reads it. Saying the reason in the pane needs the
  page to receive `private_view`, which `parts::project_harness` keeps from it; that is left as it
  is.
- c11-client-by-agent: differences from the spec. (1) A value matches a harness id or an agent's
  catalog id, not a product name, a host label ("Cline (Cursor)") or a name a client reports; the
  owner's decision names the catalog id. (2) A host named twice (by its agent and by its own id, or
  the same id repeated) is set up once, at its first mention. Before, a repeated harness id was
  applied twice, the second time as a no-op. (3) `uninstall` shares the install options and
  already took `--client` (the Linux acceptance test uses it), so its completions and its manual
  synopsis now offer it too. (4) The host list names only the harness ids no agent id names, the
  four Cline hosts, because every other harness id is its agent's id; every harness id is still
  accepted, and the help and the refusal say so by listing agents and hosts apart. (5) The
  completions and the manual page stay static files with a test that fails, naming the file, when
  their list differs from `install::client_ids`, in the style of the existing subcommand check,
  rather than generated files with an update variable. (6) The manual page's date moves to
  2026-10-17. (7) Only `bash -n` checked the bash file; the zsh action (`compadd -a
  ghostlight_clients`) and the fish list were not run, so that is an owed live check. (8) Fixup
  after review: an agent id selects only the agent's detected hosts, or every host of it with
  `--all-clients`, the same rule the run without `--client` follows. The first cut selected every
  host whatever its state, so `--client cline` with only the CLI present created
  `cline_mcp_settings.json` under the Visual Studio Code, Cursor and Windsurf globalStorage
  folders; those hosts then read Installed, the install summary and the Integrations page listed
  them as connected, and a Cline CLI session put all four in use (the c02 defect again, through a
  write). The owner's "--client cline selects every Cline host" does not ask Ghostlight to write
  into hosts the person does not have. A harness id still selects its own host in any state, so
  the persisted vocabulary and the writer golden are unchanged. An agent id with no detected host
  is refused, naming the agent's host ids and `--all-clients`, rather than falling back to every
  host, because the fallback would write the same absent files. The rule applies to `uninstall`
  as well, where an undetected host has no file to remove. The person-facing refusals moved to
  `language::deployment`.
- c12-agent-lists: differences from the spec. (1) The catalog holds 20 agents on 23 hosts, not 19:
  the spec counted products, and Windsurf and Devin share a product but are separate agents
  (ADR-0148, ACCEPTANCE item 8). The table's count is generated. (2) Making the candidates data
  changed one spelling: on Windows, when `opencode.jsonc` exists, its path now ends
  `.config/opencode\opencode.jsonc` (built with `with_file_name` from the default) where the old
  closure wrote `.config/opencode/opencode.jsonc`. It is the same file; only the `config_path` text
  differs, and the writer golden shows paths with forward slashes, so it shows no diff. Kilo's
  spellings are unchanged. (3) The guide's hand "Configuration family" table was the guide's other
  partial agent list, so the generated table carries its facts in the Entry column, read from the
  writer's own setup document rather than restated; one hand sentence keeps OpenCode's choice of
  form (the one its file uses, or the newer one when only `opencode2` is installed). (4) The table
  describes the code as it is, survey findings included: OpenCode and Crush read `~/.config` on
  both systems whatever `XDG_CONFIG_HOME` says, and Claude Desktop has a Linux row under
  `$XDG_CONFIG_HOME`. Changing either is the post-release c09 work. (5) The roots' fallbacks
  (`~/.config`, `%USERPROFILE%\AppData\Roaming`) are one generated sentence under the table rather
  than an "else" in every cell; the variables that name one agent's own file or directory
  (CODEX_HOME, CRUSH_GLOBAL_CONFIG) are written in their cells. (6) `CRUSH_GLOBAL_CONFIG` moving into
  the context is behaviour-identical for a person (the service's environment does not change while
  it runs). `tests/lib/home.mjs` and `tests/installer-powershell.ps1` still do not clear it for the
  process journeys; that is outside this slice. (7) The catalog has no platform field: Windows and
  Linux are the two computers the table is generated for and the sentence the docs say. (8)
  llms-install.md is republished at sylin.org/ghostlight/install.md, where its new relative link
  to the guide, like its existing one to docs/1.0/LANGUAGE.md, does not resolve.
- e01-icon-cache: differences from the spec. (1) `WindowsPackage` carries the package name
  (`Claude`, `OpenAI.Codex`), not the family name: the tree knows package names (the container
  detection and `CLAUDE_DESKTOP_PACKAGE`) but not Codex's publisher id, and the family is the name,
  an underscore and that id. e02 finds the installed package from the name. (2) Codex has one
  roster row, because its CLI and its desktop app share `config.toml`, so the app's package is
  Codex's own source rather than a family. The family field is per row and one level deep;
  setting it to `None` switches it off for that agent. (3) Agents left without sources, keeping
  their monograms: Claude Code (it wears Claude Desktop's icon), Cline (an extension in other
  editors, and a CLI), GitHub Copilot CLI, OpenCode, Crush, Qwen Code, Junie, Kilo Code, goose,
  Continue and ZCode, and Devin, Windsurf's newer distribution, whose install folder and desktop
  id this survey could not confirm. (4) A launcher resolves to a program only when it names an
  `.exe` relative to its own directory (`%~dp0`, `%dp0%`, `$basedir`, `$PSScriptRoot`) or by an
  absolute path, and never to an interpreter: npm's shims name Node, so Claude Code's and Codex's
  npm commands give no program. (5) Linux takes the preferred sizes, then `pixmaps`, then 256, 512,
  32 and 24 px, so an app that ships only large theme icons still gets its own. (6) The cache
  mirrors what is installed: a look that finds no icon for an agent drops its copy, while a look
  that runs out of time leaves the agents it did not reach as they were. (7) The symbol reads the
  cache the service publishes once, in `ServiceHost::start_waiting`, the production start; test
  services and command-line runs (`ghostlight doctor`) publish none and show monograms, so no test
  reads this computer's apps. What doctor says about icons is e03's. (8) The icon route is served
  in every context, as the page's own files are, because an image request carries no bearer, so
  the window's own images would break while watching is off. It says only that an app is
  installed, and `Cross-Origin-Resource-Policy: same-origin` keeps other sites' pages from loading
  it. (9) Each Integrations row carries its agent's symbol from the cached roster, so a look that
  changes an icon checks the roster again (`icons_changed`), which also publishes the sessions'
  agents. (10) The route test serves a PNG; the `.ico` media type is tested on the cache, because
  the route test's background look drops a copy no Linux source gives. (11) The diagnostics
  vocabulary gains `agent_icon_skipped` in `crates/bridge`. (12) PRIVACY and data-flows name the
  Windows reads now, ahead of e02, so the disclosure is never behind the code.
  docs/release/store-disclosure-2.0.md has no sentence this makes false. Owner question: the public
  privacy page (`https://sylin.org/ghostlight/privacy/`) should carry PRIVACY's new icon bullet
  when it is next published.
- e02-icon-windows: differences from the spec and choices. (1) The package's family is found by
  its folder below `%LOCALAPPDATA%\Packages`, because the catalog knows package names, not
  publisher ids (e01 note 1), and Windows has no kernel32 call from a package name to its family;
  `GetPackagesByPackageFamily` then confirms the package is installed for the person. A package
  installed for the person but never started may have no such folder yet; it then keeps its
  monogram until it has run once. (2) Only file-name qualifiers are read; logos kept in qualifier
  folders (`Assets\scale-200\Logo.png`) or only inside `resources.pri` are not found, and the app
  keeps its monogram or the next source. The Store apps on this machine (Codex, Claude, Notepad)
  name theirs in file names. (3) "The largest images up to 256 px" is read as the four largest
  sizes the group has, one image per size at its deepest colour: an icon group's sizes cannot pass
  256 px, and the window draws at most 46 CSS px. (4) The 96 px preference for Store logos is the
  window's largest symbol (46 px) on a 2x screen; the Linux theme order is unchanged. (5) The
  manifest is read with a small tag scanner (comments skipped, quoted values honoured, prefixes
  ignored), not an XML library: no dependency was added, and only one attribute of one element is
  needed. (6) A Store app's logo is cached as a PNG with the logo file as its source, so a package
  update (a new install folder) is a changed source and is read again. (7) The Linux compile
  check: `cargo clippy -p ghostlight-win-peer --target x86_64-unknown-linux-gnu --all-targets` is
  clean; the service's Linux check stops at `ring`'s C build on this host, so the portable parts in
  `icons.rs` are left to CI's Linux job (owed check). (8) Before the commit the resolver was run once
  by hand against this machine's installs (not a test, and nothing kept in the tree): Codex and
  Claude Desktop gave their `Square44x44Logo.targetsize-96_altform-unplated.png`, Claude Code
  wore Claude Desktop's, and VS Code (through `C:\Tools\VSCode\bin\code.cmd`) and Antigravity
  gave `.ico` files of four images, 256 px PNG first.
- e03-icon-polish: differences from the spec and choices. (1) The pane's line travels in the
  symbol (`about`), not in the Integrations row, so the line and the icon cannot disagree, and the
  writer golden, which records the agent, is unchanged (no test publishes a cache). Every place
  that names an agent carries the sentence; only the pane shows it. (2) The symbol says it is an
  app's own icon (`icon`) rather than the page reading the address, so the page still decides
  nothing about an agent's looks. `Symbol::Image` became `Symbol::Tile`, its only other form being
  the icon now. (3) The summary face keeps its circle for both kinds: "the monogram's rounded tile
  shape" is read as the outline the monogram has in each place. (4) The backdrop is one light
  neutral in the dark theme (the window has no light theme), chosen so a dark or transparent logo
  reads; a colourful icon reads either way. The room around it is a fixed few pixels per size,
  because a percentage padding resolves against the span, not the image. (5) Doctor reads the
  cache the service keeps, so it says what the service last found, not what a fresh look would
  find, and looks at no app. Its found agents get one line each and the agents with monograms one
  line together, so the text stays short; the JSON has one entry per agent. (6) A manifest written
  by e02 lacks `app` and `found_in`, fails to read, and is not offered until the next look rewrites
  it; nothing deployed had one (the e02 look by hand kept nothing). (7) The preview's icon is
  Codex's, which is connected and in use, so the fixture shows it in every place at once; the
  surface test uses Windsurf for the rail and pane and Codex for At a glance.
- f01-icon-endpoint: differences from the spec and choices. (1) "Status" is read as the image's
  availability (which image the service has for the agent), never its connection state: the image
  is the same whether the agent is connected, working or absent, and the page's dot and words say
  the state. (2) The curated vendor mark is a sentence in `crate::agent::image`'s doc between the
  app icon and the monogram, not an empty variant, because the tree keeps no dead code (ADR-0197
  D24). (3) The image policy needs nothing beyond `default-src 'none'; sandbox`: the tiles are
  presentation attributes and a `<text>` in the system sans-serif, which no directive governs.
  (4) The ETag is the quoted version, the same 12 characters the address carries, for both kinds.
  (5) The symbol reads the cache the service published and the route the cache attached to the
  window; the service gives both the same cache, and tests give the route theirs. Both go through
  `AgentImage::best`, so the order is written once. (6) Changed in review: the light tile is no
  longer painted under every image. Painted under an opaque monogram, it showed as light flecks at
  the monogram's antialiased corners on the dark window, where the image and its clip both cover a
  corner pixel only in part. So the tile is painted under an image with no source yet (waiting or
  failed, spec decision 3) and under an app's own icon, which may have no ground of its own (e03),
  and a drawn monogram sits on nothing, as before f01. The page knows an app's icon by the
  symbol's `about`, which the service gives only for one (spec decision 2), and marks it with the
  class `symbol-app-icon`; no wire key was added. With that mark back, the app icon also keeps
  e03's 1 to 5 px of room, which the first f01 draft dropped only because the page could not tell
  an app's icon from a monogram. The owed check looks at both. (7) The tiles live beside `crate::agent` rather than under
  `ui/`, because the page ships none of them; `ui/` holds only page files again. (8) Test fixtures
  that need an agent but not its image use `Monogram::FIXTURE` (test only). (9) The writer golden
  records each host's agent, so its 69 `agent:` lines now show the image address; no line the
  installer writes or says changed. The slice spec froze `tests/golden/`, so the change waited for
  the coordinator to approve this one diff. (10) Review also made the projection test's fixture
  agents wear an app icon, built directly because no test publishes an icon cache, so
  `agent.symbol.about` is classified in the session, operation, history and Integrations keys. It
  is classified for both audiences: a watcher sees who is connected and the Integrations part
  (ADR-0199), and the window serves a watcher the image itself.
- t01-test-temp-cleanup: left on purpose. (1) Production code that uses the temporary folder stays: `workbench/setup.rs` starts a setup program in the temporary folder when the person's home is unknown. (2) `crates/desktop` tests (`link.rs`, `machine.rs`, `webview.rs`, `tests/loads.rs`) still use `std::env::temp_dir()` and remove their files only when they pass; the desktop crate is outside this lane, so a desktop slice owes them a guard. (3) Evidence that journeys keep on purpose stays: `tests/installer-shell.mjs` keeps its run under the repository's `.tmp/installer-shell` and prints it, the browser journeys write screenshots under `.tmp`, `tests/installed-journey.mjs` writes its summary JSON to the temporary folder and keeps its profile only with `--keep-profile`, and a journey home keeps its evidence only where `GHOSTLIGHT_JOURNEY_EVIDENCE` names. (4) The 387 `ghostlight-` entries already in the owner's temporary folder are earlier runs' leftovers; this slice removes none of them. (5) No journey ran here, so the journeys' cleanup was checked with `node --check` only.
- t03-icon-tile: choices made by eye and differences from the spec. (1) The grounds are the
  suggested ones: `#3a3a41` stands apart from the window's `#0f0e12` (about 1.7:1), enough to read
  as a tile without glowing, and `#e7e5e4` is a slightly warm off-white that is softer than paper
  white on the dark window. The two grounds sit almost symmetrically around a mid gray, so an ink
  of relative luminance near 0.23 reaches 3:1 on neither; such an icon ties and gets light. (2) The
  plate test averages the ring's alpha (4 percent of the side, at least 1 px) and needs 90 percent:
  a square tile with slightly rounded corners keeps its plate, a disc or a glyph does not. (3) The
  padding is 10 percent of the icon's side per edge, rounded, at least 1 px, so the tile is about
  1.2 times the icon; the face's circle still clips a square icon's own corners, as it did before.
  (4) `FoundIcon` carries the copy's full SHA-256 in place of its 12-character version, because the
  memory of made tiles is keyed by the copy's own hash; doctor, the language and the agent tests
  build it that way. (5) A behaviour f01 left open is closed: f01's symbol named a cached icon's
  version while the route served the monogram when the copy could no longer be read. Both now read
  `AgentImage::best`, which gives the monogram for a copy that cannot be read or whose bytes do not
  match its hash, also after its tile was served: the copy is read and hashed on every call, and
  memory spares only the decode and the composite. (6) Building the symbol now reads a cached copy
  from `state/icons`, Ghostlight's own cache and never an app: the tile's version is the symbol's,
  so it must be made before the symbol can name it, and the copy must still be there and match its
  hash before the kept tile is named. The copies are small files of Ghostlight's own; an app is
  read only by a look. Nothing reads an image for the Closed route, which refuses before. (7) The placeholder takes the dark ground's color and a new
  name, `--icon-placeholder`, since it is no longer a tile under any drawn image. (8) The preview
  serves checked-in tiles made by the service test rather than compositing in JavaScript, so the
  rule is written once. The white fixture is drawn by the same test (a ring and a bar, 48 px); Claude
  Code wears it in the preview, so the pill shows both grounds side by side with Codex. (9) A
  bitmap entry with fewer than 32 bits, or a compressed one, is served as cached: most current Windows
  programs carry 32-bit images, and an older icon still draws as before.
  (10) The tile's bytes depend on the `png` crate's encoder: an upgrade that changes its output
  gives each app icon a new version once, and the fixture test says so.
