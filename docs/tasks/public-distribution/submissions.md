# External distribution submissions -- DRAFTS ONLY

Three drafts the public-distribution batch prepared. None of these has been sent, posted, or
filed; sending any of them is an owner action. Facts were verified on 2026-08-29 and should be
re-verified on the day anything is sent.

## 1. ZCode in-app feedback ticket (product suggestion)

Channel: ZCode in-app feedback (top-right menu), which tracks tickets with IDs and a status
pipeline. Alternative public channel: an issue on `zai-org/feedback`.

Suggested title: Public submission process for the official plugin marketplace

Draft body:

> ZCode's plugin system is excellent, and the official marketplace's own description says it
> is for "built-in and community plugins". Today every listed plugin is authored by Z.ai, and
> the plugin documentation describes no way for a third-party plugin to be considered for the
> Public catalog.
>
> Suggestion: publish a submission process for the official marketplace -- an intake form, a
> pull-request route, or a review checklist -- so community plugins can join the roster.
>
> As a concrete candidate: Ghostlight (github.com/sylin-org/ghostlight) is an open-source
> (Apache-2.0 OR MIT), local-first governed browser automation MCP server, already packaged as
> a ZCode-compatible plugin with a bundled agent skill at
> github.com/sylin-org/ghostlight (root `marketplace.json`). We would submit it through the
> public process the moment one exists.

## 2. Pull request to `zai-org/zai-coding-plugins`

Channel: GitHub pull request adding a `plugins/ghostlight/` directory (or the repository's
preferred layout; confirm from recently merged PRs before opening). The marketplace currently
uses the Claude Code plugin schema, which Ghostlight's plugin already speaks.

Suggested title: feat: add ghostlight plugin to the marketplace

Draft body:

> What: adds Ghostlight, an open-source (Apache-2.0 OR MIT) governed browser automation MCP
> server, as a marketplace plugin.
>
> - Manifest: `.claude-plugin/plugin.json`, MCP server `npx -y ghostlight` (the project's
>   checksum-verified npm launcher; installs binaries from the official GitHub release and
>   hands stdio to the local MCP connector).
> - Bundled skill: `control-browser`, teaching agents the observe-then-act handle model,
>   composition tools, and effect-truth rules.
> - Identity: local by construction; no telemetry and no hidden network dependency; the only
>   network traffic is the explicit, checksum-pinned binary download from the project's
>   release channel.
> - Works in Claude Code and in ZCode, which reads the Claude plugin schema.
>
> Happy to adjust layout, naming, or scope to the marketplace's conventions.

Before sending: check how the two existing entries are laid out and mirror them; confirm the
maintainers want general-productivity plugins (the current catalog is GLM-plan tooling); be
ready to explain the npm launcher's download-on-first-run model, which reviewers may ask
about.

## 3. Anthropic plugin-directory submission

Status: SUBMITTED for review (2026-08-29). The Console confirmation reads "Plugin submitted
for review. Your plugin submission has been received." from Leo's Individual Org.

Correction from attempting it live: the 2026-08-29 research note about a
`clau.de/plugin-directory-submission` form is outdated. The current documented mechanism
(code.claude.com docs, "Submit your plugin to the community marketplace") is:

- Third-party submissions land in **`anthropics/claude-plugins-community`** after review.
  The official `claude-plugins-official` marketplace is curated at Anthropic's discretion;
  there is no application process and the submission form does not feed it.
- Submission forms: `claude.ai/admin-settings/directory/submissions/plugins/new` (requires a
  Team or Enterprise organization) or `platform.claude.com/plugins/submit` (the Console form,
  for individual authors).
- The review pipeline runs `claude plugin validate` plus automated safety screening;
  approved plugins are pinned by commit SHA in the community catalog and the pin auto-bumps
  on new pushes.

Local status: `claude plugin validate ./packaging/plugin/ghostlight` prints
"Validation passed" (Claude Code CLI 2.1.250, 2026-08-29). The Console form is behind the
owner's Anthropic login, which the browser session does not have; filing it is an owner
action. Prepared answers (adapt to the form's actual fields):

- Plugin name (immutable slug): `ghostlight`
- Repository: `https://github.com/sylin-org/ghostlight`
- Source path in the marketplace: `./packaging/plugin/ghostlight`
- Description: Governed, semantic tools for driving the user's real browser: navigate,
  inspect, find, click, fill, type, screenshot, record. Local by construction, with optional
  policy and audit.
- MCP server: `npx -y ghostlight` -- the project's npm launcher; verifies sha256-pinned
  binaries from the official GitHub release (allow-listed hosts), installs to
  `~/.ghostlight/bin/v<version>/`, then hands its inherited stdio to the MCP connector, which
  demand-starts the sibling orchestrator from its own directory.
- Security notes for reviewers: no telemetry or update pings of any kind; downloads restricted
  to GitHub release hosts and checksum-pinned; the browser adapter is a separate,
  store-reviewed Chrome extension; plugin ships no hooks and no commands, one skill, one MCP
  server; license Apache-2.0 OR MIT with DCO.
- Why it fits the directory: it gives agents safe, semantic control of the user's signed-in
  browser with per-action capability classification and structured audit -- a common request
  for web-UI testing and verification journeys.

## Outcomes of the first send-day (2026-08-29)

The owner authorized sending the submissions through the browser this day:

1. Z.ai feedback: filed as `zai-org/feedback` issue **#419** ("Public intake process for the
   official plugin marketplace (follow-up to #66)"), category Tool use / MCP, after a real
   duplicate search that found #66 (shipped marketplace-adding, closed as launched) and a
   CONTRIBUTING.md read.
2. Z.ai community marketplace: pull request **`zai-org/zai-coding-plugins#30`**, branch
   `sylin-org:feat/ghostlight-plugin`, one commit adding `plugins/ghostlight/` (manifest,
   skill, README) and one catalog entry. The fork and clone live at
   `F:\Replica\NAS\Files\repo\github\zai-org\zai-coding-plugins` per the local convention.
3. Anthropic community marketplace: submitted through `platform.claude.com/plugins/submit`
   and confirmed received ("Plugin submitted for review"). The wizard required an
   organization (created as the individual org, billing skipped) and a terms
   acknowledgement on the introduction step; the first Submit click bounced to that
   acknowledgement and the resubmit carried identical content. Submitted values: platforms
   Claude Code and Claude Cowork, license "Apache-2.0 OR MIT", privacy policy
   `https://sylin.org/ghostlight/privacy/` (the live page behind `src/ghostlight/privacy.njk`
   in the website repository; the earlier `/trust/` guess was wrong), contact
   `hello@sylin.org` (owner-set), plus the step-2 fields recorded above. Per the docs,
   approval pins the plugin by commit SHA in `anthropics/claude-plugins-community` with the
   pin auto-bumping on new pushes.
