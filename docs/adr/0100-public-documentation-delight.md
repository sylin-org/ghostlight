# ADR-0100: Public documentation as a delight-first product surface

- Status: Accepted
- Date: 2026-08-05
- Builds on: ADR-0021, ADR-0057, ADR-0069, ADR-0093, and ADR-0094
- Execution batch: `docs/tasks/public-delight-0.8/`

## Context

Ghostlight 0.8 is a natural public reset. The product has moved substantially since the public
website and several directory crawls last described it. It now has a protocol-versioned MCP edge,
exact support for `2025-11-25` and `2026-07-28`, three role-specific executables, browser-created
tab continuity, a same-minor browser-adapter compatibility contract, and a fuller agent-facing
tool surface. The public explanation has not moved as one system with those changes.

The public signals are uneven. Glama currently gives Ghostlight A grades for license, quality, and
maintenance, while one broad compatibility tool receives a B. npm reports sustained package
activity. Those are useful diagnostics, but independent reviews and user-authored examples remain
thin. Directory approvals and posts written by the project are distribution evidence, not user
reception. The project must not manufacture social proof from them.

The closest browser-agent products have also moved. Access to an existing signed-in browser is no
longer enough to distinguish Ghostlight by itself. Ghostlight's stronger product is the complete
local experience: visible and interruptible work, browser continuity, useful recovery, a stable
multi-client MCP surface, optional policy and audit, and no vendor service in the runtime path.

Earlier public-awareness and documentation reviews established the right practitioner-first
direction. This ADR makes that direction a durable product and release decision, attaches an
execution batch, and defines how external evidence and agent-facing metadata may be improved.

## Decision

### 1. Documentation is part of the product

Public documentation is held to a first-success contract:

- In roughly 15 seconds, the right reader can understand what Ghostlight is and why it exists.
- In roughly 2 minutes, the reader can decide whether it fits the job.
- In roughly 5 minutes, a supported user can install it and complete one safe browser task.
- When a step fails, the user and agent receive a concrete recovery action instead of protocol or
  process archaeology.

These are editorial acceptance criteria, not telemetry requirements or guaranteed timings.

### 2. Lead with the user's outcome

The canonical story is:

> Ghostlight lets your AI agent work in the Chromium browser where you are already signed in. You
> see the work, keep control, and can add local policy and audit boundaries when needed.

Public prose stays warm, plain, and inviting. It begins with useful work, then explains visibility
and control, then local ownership, and only then introduces organizational governance. It does not
make personal use sound incomplete or make governance sound like punishment.

The supporting outcome pillars are:

1. Work where the user is already signed in.
2. See and interrupt what the agent does.
3. Complete coherent multi-step browser work.
4. Recover when pages, tabs, windows, or connections change.
5. Use one browser capability from compatible MCP clients.
6. Keep runtime execution, policy, and records local.

MCP revision dates, executable roles, and internal architecture remain documented and prominent
where compatibility or trust is the reader's question. They do not become the hero message.

### 3. Give each public surface one primary job

- The website explains the product, qualifies the use case, demonstrates one useful task, and
  leads to installation.
- The README proves the main claims, gives a concise technical overview, and routes readers to the
  right deeper document.
- The installation guide gets a supported user from zero to a green `ghostlight doctor` result.
- Agent-facing installation and usage guidance helps an agent install, verify, choose tools, and
  recover without guessing.
- The Trust Center answers privacy, security, policy, continuity, and procurement questions with
  evidence.
- The changelog and release notes explain user-visible change and upgrade consequences.
- Package, store, registry, and directory copy earns discovery with short factual descriptions.
- Troubleshooting maps a recognizable symptom to an explanation and next action.

No one surface should reproduce all the others. In particular, the README does not become a copy
of the Trust Center, architecture specification, or complete tool reference.

### 4. Explain capabilities as useful work

The primary public capability story uses reproducible workflows rather than a long machinery or
tool inventory. The 0.8 pass should demonstrate at least:

1. a safe first form task;
2. work inside an authenticated application without copying credentials;
3. a popup or account flow that continues in a browser-created tab; and
4. diagnosis of a failed web workflow through page, console, or network evidence.

Each example states what the user will see, what success looks like, and which boundary applies.
The full tool inventory remains available as reference material.

### 5. Keep evidence categories honest

Public research and claims distinguish three categories:

- Product evidence: source, tests, conformance results, releases, live verification, and public
  trust artifacts.
- Distribution evidence: accepted registries, directory listings, package availability, and
  project-authored showcase posts.
- Reception evidence: independent reviews, voluntary user reports, written store reviews,
  user-authored workflows, support outcomes, and coarse usage signals.

External grades, download counts, stars, favorites, and search results are dated snapshots. They
may guide work and support factual statements, but they are not durable product claims. Download
counts are treated as directional because automated installs, CI, bots, and mirrors may contribute.
Quotes require the speaker's explicit permission.

Ghostlight adds no telemetry, analytics, activation, review nag, or vendor-bound feedback channel.
Reception is measured through public counters, manually inspected dashboards, support threads,
and voluntary conversations.

### 6. Preserve trained signatures and improve all guidance

The compatibility boundary remains each trained tool's name, parameter names, parameter types,
enum values, ordering, and structural contract. The `computer` tool keeps its official
Claude-in-Chrome-compatible signature even if a directory gives it a lower static grade. No
tool is renamed, split, hidden, duplicated, or structurally reshaped to improve a score.

Descriptions and metadata are guidance, not signature. The execution pass may deliberately enrich
all 25 tools through:

- advertised tool descriptions;
- parameter descriptions without structural schema change;
- display titles and standard MCP annotations;
- examples and expected-result guidance;
- output-field descriptions without changing the output contract;
- workflow preambles and tool-choice guidance; and
- package, registry, website, store, and directory metadata.

Every description should help an agent understand purpose, preferred use, important side effects,
the closest alternative, and recovery where applicable. The exact callable name is used. Mixed
tools such as `computer` retain conservative whole-tool annotations while Ghostlight's per-action
governance remains authoritative.

Schema and metadata snapshots are updated intentionally when prose changes. External scores are
diagnostics, never acceptance gates. Runtime behavior changes require their own product reason and
are outside this documentation pass.

### 7. Keep one public truth without adding a new publishing system

The existing sources remain authoritative:

- `docs/public-status.json` owns public release, platform, and browser-adapter state.
- `CHANGELOG.md` owns release changes.
- the live tool registry owns agent-facing tool declarations.
- compatibility and package manifests own their respective contracts.
- the Trust Center owns reviewed trust claims.

The website consumes synchronized facts from this repository. Release and reconciliation scripts
remain the mechanism for checking those facts. This pass does not add a CMS, analytics platform,
parallel capability database, badge framework, or generated documentation architecture.

### 8. Make discovery specific

Page titles, descriptions, directory copy, and social metadata use a consistent qualified identity
such as `Ghostlight MCP` or `Ghostlight browser automation` where context permits. This reduces the
large search collision around the word `Ghostlight` while leaving the product name unchanged.

Comparison material stays candid and use-case based. It explains when Playwright MCP, Chrome
DevTools MCP, a first-party browser integration, or another approach is a better fit. It does not
turn into a feature-count attack.

### 9. Coordinate 0.8 as one public release

The 0.8 execution pass reconciles the website, README, install and agent guides, package metadata,
store copy, registry and directory descriptions, compatibility statements, changelog, and release
notes. Cached and crawled surfaces are checked again after publication rather than assumed current.

The owner authorizes work in the `sylin-org/website` repository. Work may be prepared and committed
on a non-publishing branch. A push or merge that deploys the website, store resubmission, directory
post, release tag, registry publication, or other outward publication still requires explicit
owner confirmation under the project's standing draft-then-confirm rule.

### 10. Execute through one resumable batch

`docs/tasks/public-delight-0.8/` is the implementation authority. It contains six meaningful
passes: truth and reception baseline, message architecture, core public surfaces, agent guidance
and metadata, website and directory surfaces, and release reconciliation plus the reception loop.
The ledger is the handoff point for new sessions.

## Consequences

- Ghostlight 0.8 gains one coherent public explanation instead of a collection of independently
  updated surfaces.
- A shorter front door can remain candid because technical, architectural, and trust depth stays
  available at the right destinations.
- User and agent delight become reviewable outcomes: recognition, fit, first success, and recovery.
- The project can improve directory and model comprehension without breaking the trained
  Claude-in-Browser-compatible surface.
- External validation can be discussed without inflating distribution into reception or noisy
  counters into active-user claims.
- The release process gains a public-surface reconciliation obligation, including cached copies.
- The work spans two repositories and several external systems, so the ledger and publication
  gates are load-bearing. They do not authorize a new publishing subsystem.
