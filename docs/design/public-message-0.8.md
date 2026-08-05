# Ghostlight 0.8 public message architecture

Status: Internal copy kit for ADR-0100. Not approved for publication by itself.

This document turns the [2026-08-05 public truth and reception baseline](../research/public-reception-2026-08.md)
into one story that later surfaces can adapt. Canonical product state still belongs to
`docs/public-status.json`, `CHANGELOG.md`, compatibility manifests, the tool registry, and the
Trust Center. This document owns message order and reusable copy, not live facts.

Claim ids such as `C1` point to the [claim-to-evidence matrix](#claim-to-evidence-matrix). Keep the
traceability when adapting copy, but omit the ids from published prose.

## Message order

Use this order unless a surface has a narrower job:

1. Useful work in the Chromium browser the person already uses.
2. Visible work and human control.
3. Coherent workflows, continuity, and recovery.
4. Local ownership and compatible MCP clients.
5. Optional policy and audit for work that needs boundaries.
6. Protocol, executable, compatibility, licensing, and procurement depth.

Do not lead with process topology, licensing, governance, a tool count, or a comparison. Those
details matter after the reader recognizes the job.

## Canonical story

### One-sentence product truth

> Ghostlight lets your AI agent work in the Chromium browser where you are already signed in. You
> see the work, keep control, and can add local policy and audit boundaries when needed.

Evidence: `C1`, `C2`, `C3`, and `C7`.

### Short description

Ghostlight MCP lets an AI agent do useful work in your signed-in Chromium browser while you watch
and stay in control. It runs locally, works with compatible MCP clients, and adds policy and audit
only when you need them. (`C1`, `C2`, `C3`, `C4`, `C7`, `C8`)

### Medium description

Ghostlight MCP connects compatible AI clients to the Chromium profile you already use. The agent
can read pages, navigate, fill forms, handle files and dialogs, and inspect browser evidence in a
dedicated visible workspace. Browser-created tab continuity and explicit recovery help longer
work survive ordinary page, tab, and connection changes. The runtime stays on your machine, with
no Ghostlight account or hosted control plane; optional capability and domain policy can add local
audit boundaries for higher-stakes work. (`C1` through `C8`)

### Long description

Ghostlight MCP gives an AI agent a useful workspace in the Chromium browser where you are already
signed in. Authenticated sites stay in the browser profile you chose, so you do not need to copy
credentials into a separate browser service. The work happens in a dedicated visible tab group
with on-page action feedback, and you can pause, interrupt, or take over. (`C1`, `C2`)

The agent can read and navigate pages, complete forms, handle files and dialogs, compose bounded
multi-step work, and inspect page, console, and network evidence. Ghostlight keeps exact workspace
identity, can continue into an unambiguous browser-created child tab, and gives the agent concrete
recovery actions when a tab, page, or MCP connection changes. (`C5`, `C6`, `C13`)

The MCP edge, persistent service, browser connector, and extension run locally as the current OS
user, without a Ghostlight login, activation service, or telemetry. Personal and all-open use is a
complete product. When a job needs more control, optional capability and domain grants plus
structured audit can narrow the same browser workflow without moving policy into the extension.
(`C3`, `C4`, `C7`, `C8`, `C10`)

## Six outcome pillars

| Pillar | Reader promise | Supporting proof | Avoid |
| --- | --- | --- | --- |
| Work where you are signed in | Use the Chromium profile that already has the session needed for the job. | `C1` | Claiming signed-in access is unique or implying access to every ordinary tab |
| See and interrupt the work | Watch actions in a dedicated visible workspace and take control when needed. | `C2` | Calling visibility a security boundary by itself |
| Complete coherent work | Read, act, wait, compose, upload, and diagnose without turning every workflow into low-level clicks. | `C5`, `C13` | Leading with a 25-tool inventory |
| Recover when the browser changes | Continue through supported child-tab transitions and use explicit recovery for stale tabs or closed MCP transport. | `C6` | Saying every popup or ambiguous tab is adopted automatically |
| Use compatible MCP clients | Connect supported and compatible local stdio clients to one protocol-neutral service. | `C4`, `C9`, `C10` | Saying every MCP client has been verified |
| Keep ownership local | Run without a Ghostlight-hosted control plane or telemetry, then add local policy and audit when useful. | `C3`, `C7`, `C8` | Saying the product never uses a network; the browser, managed policy, and configured audit destinations may |

## Audience routes

### Person trying to use Ghostlight

Start with the one-sentence truth, the fit block, one install path, and the read-only first task.
Then route to the installation guide, comparison, and troubleshooting. Do not require architecture
or governance reading before first success.

### Integrating agent or MCP client

Start with the exact stdio entry, supported protocol revisions, the visible extension step, and a
health check. Then provide compact tool-choice and recovery guidance from the live registry. Route
to `llms-install.md`, the installation guide, compatibility contract, and tool declarations.

### Contributor

Start with the three executable roles, clean-room and trained-signature boundaries, the fast test
gate, and the relevant ADR index. Route to `CONTRIBUTING.md`, `AGENTS.md`, `docs/DEV-LOOP.md`, and
the subsystem ADR before implementation detail.

### Trust or procurement reviewer

Start with data flow, runtime boundary, governance ownership, audit destinations, continuity, and
known assurance gaps. Route to the Trust Center. Do not make the README reproduce procurement
answers, and do not move reviewed trust claims into this copy kit.

## Fit and anti-fit

### A good fit

Ghostlight is worth trying when:

- the job needs a site where the person is already signed in;
- the person wants browser work to stay visible and interruptible;
- the workflow crosses several page states, tabs, forms, or evidence sources;
- more than one compatible MCP client should use the same local browser capability; or
- capability, domain, and audit boundaries may be useful now or later.

Evidence: `C1` through `C8`.

### Choose another approach when it fits better

- Choose Playwright MCP when the primary job is deterministic browser testing, isolated test
  contexts, or Playwright-native automation. Its extension can also connect to existing signed-in
  tabs, so that fact alone is not a reason to choose Ghostlight.
- Choose Chrome DevTools MCP when performance analysis, DevTools diagnostics, or Chrome-specific
  debugging is the center of the job.
- Choose a first-party browser integration when one supported assistant is the whole environment
  and its native browser workflow already meets the need.
- Choose a hosted or headless browser product when the work must run away from the person's local,
  visible browser.
- Do not choose Ghostlight for Firefox, stealth automation, scraping farms, or a shared
  multi-tenant browser service. Those are outside its supported boundary.

Evidence: [E1 alternatives and distinctions](../research/public-reception-2026-08.md#closest-alternatives-and-defensible-distinctions),
[comparison guide](../COMPARISON.md), and [project scope](../SPEC.md).

## Claim-to-evidence matrix

| Id | Reusable material claim | Evidence class | Canonical owner or evidence |
| --- | --- | --- | --- |
| C1 | Ghostlight works in a Chromium profile where the person is already signed in. | Product | [E1 capability map](../research/public-reception-2026-08.md#capability-map-by-user-outcome), [data flows](../trust/data-flows.md) |
| C2 | Work is visible in a dedicated workspace and the person can interrupt or take over. | Product | [E1 capability map](../research/public-reception-2026-08.md#capability-map-by-user-outcome), [solo-developer guide](../guides/solo-developer.md), [visual language](visual-language.md) |
| C3 | The runtime needs no Ghostlight account, hosted control plane, telemetry, or activation service. | Product | [Continuity Promise](../trust/continuity.md), [Trust Center](../trust/README.md), [E1 canonical truth](../research/public-reception-2026-08.md#canonical-public-truth) |
| C4 | Nine installer targets and other compatible local stdio MCP clients can connect through one protocol-neutral service. | Product | [installer definitions](../../crates/core/src/install/clients.rs), [installation guide](../guides/installation.md), [ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md) |
| C5 | The current tool surface supports reading, interaction, forms, files, composition, recording, and browser diagnosis. | Product | [tool registry](../../crates/core/src/browser/directory.rs), [schema fidelity test](../../tests/tool_schema_fidelity.rs) |
| C6 | Exact workspace identity, explicit stale-workspace recovery, supported browser-created child-tab continuity, and MCP transport recovery are implemented. | Product | [ADR-0090](../adr/0090-explicit-stale-workspace-recovery.md), [ADR-0099](../adr/0099-browser-created-tab-continuity.md), [project status](../STATUS.md) |
| C7 | Optional governance applies capability and domain policy plus structured audit outside the policy-free extension. | Product | [governance guide](../guides/governance-configuration.md), [security overview](../trust/security-overview.md), [ADR-0013](../adr/0013-governance-overlay-all-open.md) |
| C8 | Personal and unrestricted all-open use is complete without a policy manifest; the product is open-core. | Product | [ADR-0013](../adr/0013-governance-overlay-all-open.md), [licensing map](../../LICENSING.md) |
| C9 | The 0.8 source candidate implements exact local stdio MCP revisions `2025-11-25` and `2026-07-28`. | Product | [0.8 changelog](../../CHANGELOG.md), [MCP connector source](../../crates/mcp-connector/src), [E1 canonical truth](../research/public-reception-2026-08.md#canonical-public-truth) |
| C10 | The 0.8 runtime ships an MCP connector, persistent service, and browser connector beside the extension. | Product | [ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md), [0.8 changelog](../../CHANGELOG.md) |
| C11 | Public service, adapter, platform, and pending-review claims come only from dated public status. | Product and distribution | [`docs/public-status.json`](../public-status.json), [compatibility contract](../../compatibility.json) |
| C12 | Windows and Linux have live-browser proof; macOS has build and full-suite CI proof with live-browser verification owed. | Product | [`docs/public-status.json`](../public-status.json) |
| C13 | The source declares 25 additive registry tools while the original 13 trained identities remain structurally stable. | Product | [tool registry](../../crates/core/src/browser/directory.rs), [schema fidelity test](../../tests/tool_schema_fidelity.rs), [ADR-0094](../adr/0094-agent-readable-tool-definitions.md) |

Reception counters and directory grades do not appear in this matrix because they are dated
diagnostics, not durable product claims. Use the E1 baseline when a later release packet needs
their dated context.

## Public-surface jobs

| Surface | One primary job | It should not become | Canonical owner |
| --- | --- | --- | --- |
| Website | Help the right person recognize the product, decide fit, see one useful workflow, and begin installation. | The full architecture, trust packet, or tool reference | Website source, synchronized from product facts |
| README | Prove the main outcome claims, give a concise technical orientation, and route to owned depth. | A duplicate Trust Center, specification, install manual, and full tool catalog | [`README.md`](../../README.md) |
| Installation guide | Move a supported user from zero to a green `ghostlight doctor` result. | Product marketing or complete architecture history | [Installation guide](../guides/installation.md) |
| Agent guide | Help an agent install, verify, choose the next tool class, and recover without guessing. | A second hand-written tool registry | [`llms-install.md`](../../llms-install.md) plus the live tool registry |
| Trust Center | Answer security, privacy, policy, continuity, assurance, and procurement questions with reviewed evidence. | Hero copy or unreviewed marketing claims | [Trust Center](../trust/README.md) |
| Changelog and release notes | Explain user-visible changes, compatibility, and upgrade consequences for one release. | A timeless product overview | [`CHANGELOG.md`](../../CHANGELOG.md) |
| Troubleshooting | Map one recognizable symptom to its likely explanation and next action. | A narrative install guide or process archaeology | [Installation troubleshooting](../guides/installation.md#troubleshooting) |
| Package metadata | Earn a qualified discovery click and state the install/runtime shape within the field limit. | A compressed feature inventory or mutable reception claim | npm, MCPB, and [`server.json`](../../server.json) manifests |
| Store listing | Explain why browser permissions are needed, what stays local, and how to complete the browser side of setup. | A service release history or unsupported privacy absolute | [Store listing source](../legal/STORE_LISTING.md) |
| Directories | Qualify the operating model and point to the canonical install path in the directory's available fields. | Social proof, stale copied README text, or a custom story per directory | [Directory runbook](../business/DIRECTORY-SUBMISSIONS.md) |

## Recognition, fit, first success, and recovery copy

### Roughly 15-second recognition

**Heading:** Let your agent work in the browser you already use.

**Support:** Ghostlight MCP connects compatible AI clients to your signed-in Chromium browser.
The work stays visible, you keep the wheel, and the runtime stays local. (`C1` through `C4`)

**Primary action:** Install and try one read-only task.

**Secondary action:** See when Ghostlight fits.

### Roughly 2-minute fit

Use the [fit and anti-fit](#fit-and-anti-fit) block beside one visible workflow. Keep four details
within the first screenful after the hero:

- normal signed-in Chromium profile, not a hosted clean browser;
- visible local workspace with human pause and takeover;
- compatible local stdio MCP clients; and
- personal use is complete, with governance optional.

Then link to comparison, Trust Center, platform status, and architecture instead of expanding them
inline.

### Roughly 5-minute first success

1. Run `npx -y ghostlight install`.
2. Add Ghostlight in Browser from the Chrome Web Store.
3. Restart the MCP client if it does not hot-reload tools.
4. Copy this read-only prompt:

> Open https://example.com/ in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page.

Expected result: the person sees a dedicated Ghostlight tab group, the agent names the exact tab,
and the page is summarized without a write or click. (`C1`, `C2`, `C4`)

The installation surface must use current store and platform text from `docs/public-status.json`,
not version text copied from this document.

### Recovery

**No tools after install:** restart or reconnect from the current MCP client. Do not launch a
standalone connector as a workaround.

**Browser says disconnected:** run `npx -y ghostlight doctor`, enable the store extension if it is
disabled, and restart the browser only if the finding asks for it.

**Tab or workspace is stale:** call `tabs_context_mcp` to inspect current owned tabs. If there is
no usable workspace, call `tabs_create_mcp` once to create and pin one; other calls do not switch
workspaces automatically.

**Transport closed during an effectful call:** stop and reconnect through the current MCP client.
Inspect page and tab state before deciding whether to retry; do not blindly replay an action whose
outcome is unknown.

Evidence: `C6`, [installation troubleshooting](../guides/installation.md#troubleshooting), and
[project status](../STATUS.md).

## Proof recipes

These are copy recipes for later surfaces. A surface should use the one recipe that proves its
job, not all four.

### 1. Safe form: complete a simulated launch brief

**Prompt**

> Open https://sylin.org/ghostlight/demo/brief/ in a new Ghostlight tab. This is a simulated form.
> Set Project to Moonlight Notes, Owner to Maya Chen, and Summary to "Turn field observations into
> a shared release brief." Enable Include screenshots and Keep data local, then select Create
> brief. Stop when the page confirms the brief is ready for review.

**What the person should see:** a new tab in the Ghostlight group, a read scan, visible feedback on
each field, one deliberate submit action, and `Moonlight Notes is ready for review.`

**Success boundary:** only the synthetic public demo changes. The prompt authorizes that form
submission and nothing outside it. Evidence: [demo-brief implementation](../../src/demo_brief.rs)
and `C2`, `C5`.

### 2. Authenticated work: read without copying credentials

**Prompt**

> Open [SIGNED-IN APPLICATION URL] in a new Ghostlight tab using my current browser session.
> Confirm the account or workspace name visible on the page, summarize the current page, and list
> the next available actions. Do not click, type, submit, or copy credentials.

**What the person should see:** the chosen application opens in the Ghostlight group with the same
profile session, the agent reports only visible page context, and no credential handoff or page
mutation occurs.

**Success boundary:** the person chooses the application and confirms the account is appropriate.
The proof is read-only and must not be recorded or quoted without permission. Evidence: `C1`, `C2`,
and [data flows](../trust/data-flows.md).

### 3. Browser-created-tab continuity: follow an exact child

**Prompt**

> Open https://example.com/ in a new Ghostlight tab. Add a temporary link labeled Open child proof
> that points to https://example.org/ and opens in a new tab, then click that link. Follow the
> browser-created child and report the title and URL of both the original and child tabs. Do not
> close either tab or change either site.

**What the person should see:** the click opens one child tab, Ghostlight follows the exact opener
relationship, the child becomes directly usable without a manual context refresh, and the source
tab remains open.

**Success boundary:** the temporary DOM change is limited to the disposable example.com tab and
the two public example domains. This recipe requires execute plus action capability. It must fail
honestly rather than adopt an ambiguous popup. Evidence: `C6` and [ADR-0099](../adr/0099-browser-created-tab-continuity.md).

### 4. Workflow diagnosis: combine page, console, and network evidence

**Prompt**

> Open https://sylin.org/ghostlight/demo/foundry/ in a new Ghostlight tab. Start console and
> network tracking, reload once so page-load events are captured, then inspect the page, console,
> and network buffers. Report any failed request or console error, distinguish observed evidence
> from inference, and recommend one next check. Do not modify the page.

**What the person should see:** a visible page read followed by quiet console and network reads.
The answer separates page state, browser events, and inference. Finding no error is a valid result
when the evidence is clean.

**Success boundary:** this is read-only diagnosis on the synthetic Foundry page. Console tracking
begins when first requested, so the single reload is explicit. Evidence: [demo implementation](../../src/demo.rs),
the [tool registry](../../crates/core/src/browser/directory.rs), and `C5`.

## Discovery metadata

Use a qualified identity in search and card contexts. The product name in prose remains
Ghostlight.

- **Page title (59 characters):** `Ghostlight MCP - Browser automation you can see and control`
- **Search description (147 characters):** `Let AI agents work in your signed-in Chromium browser while you watch and stay in control. Ghostlight MCP runs locally with compatible MCP clients.`
- **Social-card title:** `Ghostlight MCP for your signed-in browser`
- **Social-card description:** `Visible local browser automation, useful recovery, and optional policy and audit for compatible MCP clients.`

Evidence: `C1` through `C7`. Recheck field lengths in the actual website and package templates.

## Directory copy drafts

These drafts are not authorized for external submission.

### Compact description, 230 characters

> Ghostlight MCP lets AI agents work in your signed-in Chromium browser while you watch and stay
> in control. It runs locally, supports compatible MCP clients, recovers across browser changes,
> and can add capability policy and audit.

Evidence: `C1` through `C7`.

### Fuller description

> Ghostlight MCP connects compatible AI clients to the Chromium profile you already use. Agents
> can read and navigate pages, fill forms, handle files and dialogs, compose multi-step work, and
> inspect page, console, and network evidence in a dedicated visible workspace. Browser-created
> tab continuity and explicit recovery help work survive ordinary browser changes. The runtime
> stays local with no Ghostlight account or telemetry. Personal use is complete without policy;
> optional capability and domain grants add structured audit when the job needs boundaries.

Evidence: `C1` through `C8` and `C13`.

## Exact compatibility and architecture wording

Use this below the product story when protocol or integration compatibility is the reader's
question:

> Ghostlight 0.8 is a local stdio MCP server with exact protocol state machines for revisions
> `2025-11-25` and `2026-07-28`. MCP clients launch `ghostlight-mcp-connector`, which connects to
> the persistent, protocol-neutral `ghostlight` service. Chromium independently launches
> `ghostlight-browser-connector` as its native-messaging host. Ghostlight does not expose a remote
> MCP endpoint.

Evidence: `C9`, `C10`, [ADR-0077](../adr/0077-local-only-ingress.md), and
[ADR-0096](../adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md).

This wording is for the 0.8 candidate. Do not put it on a surface describing the current public
release until `docs/public-status.json`, the release, and that surface all agree.

## Banned overclaims and stale phrases

Do not publish these phrases or implications:

- `the only MCP server for your signed-in browser` or any equivalent uniqueness claim;
- `works with every MCP client`; say compatible local stdio MCP clients and name verified targets
  where useful;
- `one portable binary`, `ghostlight-relay`, `agent relay`, or a two-relay topology;
- `13 tools` as the complete current surface, or a tool count as the hero message;
- `tool descriptions are byte-stable` or `description prose is frozen`; structural trained
  identity is stable while guidance can improve;
- `governed browser access` as the opening definition; personal all-open use is complete;
- `open source` for the whole product without explaining the open-core license boundary;
- `no network access` or `nothing ever leaves the machine`; browser traffic, optional managed
  policy, and configured audit destinations are real;
- `all platforms live verified`; macOS live-browser verification is still owed;
- a public adapter version, pending-review state, release version, directory grade, download
  count, star count, favorite count, or review count without a dated authoritative source;
- an active-user, adoption, popularity, certification, audit, or comparative-superlative claim
  inferred from distribution or coarse counters;
- a quote, named workflow, or proof-participant detail without explicit permission; or
- `no reviews exist`; say none were located in the named dated surface or bounded search.

Current stale public phrases and their owners are recorded in the
[E1 discovery map](../research/public-reception-2026-08.md#discovery-and-stale-public-surfaces).

## Adaptation rule

Later passes should copy the smallest block that serves the surface's primary job, then replace
version, platform, adapter, compatibility, or external-status text from its canonical owner.
Never make a shorter surface more impressive by dropping a material boundary. Never make a front
door comprehensive by duplicating every deeper document.
