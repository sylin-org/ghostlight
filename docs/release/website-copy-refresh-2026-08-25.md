# sylin.org Ghostlight copy refresh -- 2026-08-25 (draft)

This is a drafting document, not a deployment. The website lives in `sylin-org/website`; the
owner carries approved copy there. Nothing here may claim 1.0 before independently observable
1.0 artifacts exist (G10), and the Chrome Web Store listing serves 0.8.0 until publication is
authorized.

Verdict on the current page: it is accurate for the published 0.8 product, so nothing below is
urgent. Every proposed change ships in one batch at 1.0 publication, except where marked
"safe now".

## 1. Project grid card

Current: badge `0.8`, blurb "Let AI agents work in your signed-in Chromium browser while you
watch and stay in control. Ghostlight MCP runs locally with compatible MCP clients."

Proposed at 1.0 publication: badge `1.0`; blurb unchanged (it ages well).

## 2. Flagship detail panel

Current: `v0.8.0 - open-core - pre-1.0`.

Proposed at 1.0 publication:

```text
v1.0.0 - open-core
```

Keep the tagline, CTAs, paste-install prompt, and motto exactly as they are.

## 3. "Local by construction" diagram

Current names `ghostlight service`, which is correct for 0.8 and wrong for 1.0 (the role is
removed; the orchestrator is the one desktop authority). Proposed at 1.0 publication:

```text
MCP client  <->  ghostlight-mcp-connector  <->  ghostlight orchestrator  <->  ghostlight-browser-connector  <->  extension  <->  Chromium
```

Keep the sentence beneath it, with "the orchestrator, connectors, and extension run locally as
the current user" replacing "the three executable roles and the extension".

## 4. "What it looks like from where you sit" -- add the missing persona

Current rows: For you / For the agent / For reviewers. The organization persona is the page's
biggest gap; it is also where Ghostlight has substance competitors do not. Proposed fourth row
(order: For you, For the agent, For your scripts, For the organization; the scripts row is new
too):

```text
For your scripts
One command installs; repeat installs change nothing; `doctor --json` and preserved exit
statuses make the CLI safe to automate against.

For the organization
Policy a person can read: one line per capability naming the layer that decided it, signed
bundles from your own source, stable denial ids, and a local audit record that never leaves
the machine.
```

Rationale: the four-persona story is the differentiator; the site currently tells three of the
four.

## 5. "A minute with Ghostlight" step 3

Current step 3 is "Recover and continue". Proposed sharpening at 1.0 publication:

```text
03
Pick up where the browser left off
Exact workspace identity and useful next steps help work survive ordinary page, tab, and
connection changes -- and repeat work reuses the tab group and tab it already owns instead of
littering the strip.
```

Rationale: ADR-0137's reuse behavior is the most felt fix in 1.0; the page should say it.

## 6. Recipes -- add one, keep four

Recipes 01 through 04 stay exactly as they are. Proposed fifth at 1.0 publication:

```text
05
Continue a task without littering the tab strip
Tab and group reuse

Prompt
Open https://example.com/ in a new Ghostlight tab and summarize the page. Then open it again
and summarize it once more. Report which tab handled each request. Do not close any tab or
change any site.

What you should see
The first open creates or adopts one Ghostlight tab; the second open reuses it. The summary
says the same tab handled both, and no duplicate example.com tabs appear.

Success boundary
Only the disposable example.com tab is touched. A tab you moved or pinned yourself is left
where you put it.
```

Rationale: the reuse beat demos in one prompt what used to be the product's most visible
annoyance.

## 7. "Where it stands today" block

Two versions. Ship the first only after G10's observable-artifact rows close:

At 1.0 publication:

```text
Ghostlight 1.0.0 is published: the orchestrator, MCP connector, browser connector, and the
reviewed 1.0.0 store adapter. Windows and Linux are the supported platforms; the Chrome Web
Store listing serves the 1.0.0 adapter, which covers Ghostlight 1.0.x. See the compatibility
map for adapter and service version pairings.
```

Safe now (optional, 0.8-honest): unchanged. The current block is accurate.

## 8. Do not change

- The motto, mascot, palette, and card artwork.
- Recipes 01 through 04 and both demo stages.
- The install prompt and `install.md` pipeline: the site fetches `llms-install.md` live from the
  repository's main branch at build time; `scripts/publish-website.ps1` (restored 2026-08-25)
  refreshes the committed fallbacks and triggers the rebuild. That flow is how the 1.0 install
  guide reaches the site at publication -- no hand-edited install copy.
- The Trust Center, decision aid, and comparison links.

## 9. Publication sequencing reminder

This batch deploys only after, in order: G10 tag and GitHub release observable, npm tarball
observable, Chrome adapter published from staged state. The website must never describe a 1.0
that cannot yet be downloaded; `docs/public-status.json` is the source of truth the site's
status fallback mirrors.
