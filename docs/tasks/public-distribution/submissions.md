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

## 3. Anthropic `claude-plugins-official` directory submission

Channel: the plugin directory submission form (`clau.de/plugin-directory-submission`).
Third-party plugins land in `external_plugins/` after quality and security review.

Prepared answers (adapt to the form's actual fields):

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
