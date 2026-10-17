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

## RESUME HERE

d00 to d08, c00-writer-golden, c01-download-per-host and c02-setup-view-rules are on `dev` (not yet
pushed). Decision 8 adds six slices before the release candidate; c00, c01 and c02 have landed. Next:
c03, then c11 and c12. Then the coordinator deploys,
the owner looks, the coordinator pushes and watches the browser lane's `history-browser` journey in CI
(d08 note 1) and the Linux test job's writer golden (c00 note 4).

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
