# sylin.org Ghostlight copy refresh -- 2026-08-25 (draft)

APPLIED 2026-08-26: sections 1 through 7 shipped in website commit `f45d3b7`
(sylin-org/website main) after this document's sequencing preconditions closed, and the live
site was observed serving the 1.0 story the same day. Section 3's diagram, section 4's four
personas, section 5's step 3, section 6's fifth recipe, and section 7's published block are all
live; the site checker pins them (`check-site.js` now requires the orchestrator chain and
exactly five proof recipes). The install-guide fallback was refreshed from `main`'s updated
`llms-install.md`, and `scripts/publish-website.ps1 -DryRun` reports the fallbacks in sync.

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

AMENDED same day: Ghostlight became fully free and open source
([ADR-0140](../adr/0140-fully-open-source-licensing.md)), so "open-core" is retired everywhere.
The website working tree now carries `stage: 'free and open source'` for the Ghostlight card,
which renders as `v0.8.0 - free and open source` until the 1.0 version replaces it at
publication:

```text
v1.0.0 - free and open source
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

## 9. Applied the same day: FOSS copy and decision-aid cost-layer removal

With owner direction, the website working tree (`sylin-org/website`, local edits only, not
deployed) was updated for ADR-0140 alongside this repository:

- `src/_data/portfolio.js`: Ghostlight card stage changed from `open-core / pre-1.0` to
  `free and open source`.
- `src/index.njk` Continuity card and `src/way.njk`: the "paid layer" and "open source at the
  core" framing in both places was replaced with whole-product free-and-open-source statements.
- `/ghostlight/decision-aid/`: the entire cost layer was removed (price tuner, currency,
  planning scale and inference mix, annual totals and formulas, cost assumptions table, share
  cost toggle, cost interest and insights, and their now-dead CSS). It remains a fit-comparison
  aid; Ghostlight's scenario card states governance is included free under Apache-2.0 OR MIT.
  `GHOSTLIGHT-DECISION-AID-REFRESH.md` carries a matching amendment so refresh work never
  reinstates pricing entries.
- Site checks updated and green: check-site.js pins the retired ids as absent, rejects retired
  licensing phrases ("commercial layer", "paid layer", "open-core", "source-available") across
  all built pages, and check-decision-aid.js pins that prices/inputDefaults/cost insights stay
  removed.

Nothing here is deployed. Publication still follows section 10's sequencing.

## 10. Publication sequencing reminder

This batch deploys only after, in order: G10 tag and GitHub release observable, npm tarball
observable, Chrome adapter published from staged state. The website must never describe a 1.0
that cannot yet be downloaded; `docs/public-status.json` is the source of truth the site's
status fallback mirrors.
