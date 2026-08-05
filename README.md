<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight MCP</h1>

<p align="center"><strong>Give your agent a place in the browser you already use.</strong></p>

<p align="center">
  <a href="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml"><img src="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml/badge.svg?branch=dev" alt="CI"></a>
  <a href="https://www.npmjs.com/package/ghostlight"><img src="https://img.shields.io/npm/v/ghostlight?color=38BDF8&label=npm" alt="npm"></a>
  <a href="https://github.com/sylin-org/ghostlight/releases/latest"><img src="https://img.shields.io/github/v/release/sylin-org/ghostlight?color=38BDF8&label=release" alt="release"></a>
  <a href="https://registry.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP_registry-org.sylin%2Fghostlight-38BDF8" alt="MCP registry"></a>
</p>

<p align="center"><img src="docs/assets/demo.gif" alt="Ghostlight reading and completing a launch brief in a real browser with visible page, field, and click feedback" width="838" height="766"></p>
<p align="center"><sub>A launch brief moves from empty form to ready for review, in full view.</sub></p>

<p align="center"><a href="#your-first-five-minutes"><strong>Install Ghostlight</strong></a> | <a href="https://sylin.org/ghostlight/decision-aid/">See where it fits</a> | <a href="docs/guides/installation.md">Installation guide</a> | <a href="docs/trust/README.md">Trust Center</a></p>

Ghostlight gives compatible AI agents a dedicated workspace inside the Chromium profile you
already use. Your signed-in sessions are there. The work stays visible. You can pause, take over,
or stop it at any time.

Ask an agent to read a page, complete a form, handle a file, follow a popup, or investigate a
failed web workflow. Ghostlight carries the task across tabs and browser changes while keeping the
browser work and its controls on your machine.

## Your first five minutes

You need Chrome, Edge, Brave, or Chromium 116+, an MCP client, and Node.js for the installer. The
service you run afterward is native Rust.

1. Install Ghostlight and register the MCP clients it finds:

   ```sh
   npx -y ghostlight install
   ```

2. Add
   [Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
   from the Chrome Web Store.

3. Restart an MCP client if it does not hot-reload tools.

4. Give it one small, read-only task:

   > Open https://example.com/ in a new Ghostlight tab, summarize the page, and tell me which tab
   > you used. Do not click, type, submit, or change the page.

A sky-blue Ghostlight group should appear in your browser. The agent opens the page, reads it, and
names the exact tab it used. That one prompt proves the whole connection without authorizing a
click or write.

If a step needs attention, run:

```sh
npx -y ghostlight doctor
```

`doctor` checks the client entry, local service, browser connection, and extension, then names the
next action. The [installation guide](docs/guides/installation.md) covers targeted clients,
Homebrew, source builds, updates, uninstall, and symptom-led recovery.

## From one page to a whole workflow

Ghostlight is at its best when browser work has a thread to follow:

- **Pick up where you are signed in.** Open an application in the Chromium profile you chose and
  work with the session already there. Credentials stay with the browser.
- **Finish the interaction.** Navigate, fill forms, upload files, resolve dialogs, wait for page
  state, and carry results from one step into the next.
- **Follow the browser.** Keep working when a site opens a supported child tab or when a known
  workspace changes underneath the task.
- **See what failed.** Bring page state, console messages, and network requests together so the
  next debugging step comes from evidence instead of guesswork.

Use the same browser capability from Codex, Claude Code, Claude Desktop, Cursor, VS Code, Windsurf,
Zed, OpenCode, Crush, or another compatible stdio MCP client. Agents receive structured page
reads, exact element references, bounded action receipts, and specific recovery guidance. They can
call `explain` for the live action and capability directory whenever a task needs it.

## The browser stays a shared space

Ghostlight works in a dedicated sky-blue tab group inside the browser window you chose. Page
scans, clicks, typing, drags, and longer phases share one visual language, so the movement on screen
has an explanation.

Pause the workspace, take over for a delicate step, or stop it. Move its tabs where you want them;
Ghostlight follows the workspace instead of snapping it back. Ordinary tabs remain outside the
agent's owned set.

Personal use is complete without a policy manifest. Start with the full browser engine and get
useful work done. When a workflow needs stronger boundaries, grant `read`, `action`, `write`, and
`execute` capabilities by MCP identity and domain. Add sacred domains, dry-run preflight, and
structured audit while the browser experience stays the same.

The [governance guide](docs/guides/governance-configuration.md) shows the operating model. The
[Trust Center](docs/trust/README.md) carries the security, privacy, continuity, deployment, and
procurement evidence. The [decision aid](https://sylin.org/ghostlight/decision-aid/) covers other
browser operating models when that is the question.

<details>
<summary><strong>Current release and compatibility</strong></summary>

**Platform state.** Windows and Linux are verified end to end against live browsers. macOS builds and passes the full test suite in CI; its live-browser verification is still owed.

**Extension state.** The Chrome Web Store serves Chrome adapter v0.7.1. Chrome adapter v0.7.1 covers Ghostlight service versions v0.7.1-v0.7.3. Chrome adapter v0.8.0 is pending review. Chrome adapter v0.8.0 covers Ghostlight service versions v0.8.x. Install the extension from the public listing.

The service and Chrome adapter version independently. The
[compatibility map](compatibility.json) is authoritative, and the
[public status file](docs/public-status.json) owns current release, platform, and store state.

The 0.8 source candidate implements exact local stdio MCP revisions `2025-11-25` and
`2026-07-28`. See the [changelog](CHANGELOG.md) for release changes and upgrade consequences.

</details>

<details>
<summary><strong>How Ghostlight fits together</strong></summary>

```text
MCP Client <--stdio--> ghostlight-mcp-connector <--typed local IPC--> ghostlight service
    <--browser IPC--> ghostlight-browser-connector <--native messaging--> Extension <--CDP--> Browser
```

The connector owns MCP protocol state. The persistent service owns workspaces, browser
coordination, optional governance, and audit. The browser connector passes native messages, while
the extension owns Chrome mechanism. Each role can reconnect independently.

[ADR-0096](docs/adr/0096-protocol-versioned-mcp-edge-and-neutral-service.md) explains the boundary.
[docs/SPEC.md](docs/SPEC.md) gives the deeper governance model. The
[installation guide](docs/guides/installation.md) includes the source-development path.

</details>

## Choose your next step

| I want to... | Start here |
| --- | --- |
| Install, verify, update, recover, or uninstall | [Installation guide](docs/guides/installation.md) |
| Let an AI client perform setup | [Agent install guide](llms-install.md) |
| Try a complete visible workflow | [Launch brief demo](https://sylin.org/ghostlight/demo/brief/) |
| Build from source and test locally | [Source-development path](docs/guides/installation.md#path-b-build-from-source) |
| Understand which browser operating model fits | [Decision aid](https://sylin.org/ghostlight/decision-aid/) |
| Add boundaries or review trust evidence | [Governance guide](docs/guides/governance-configuration.md) and [Trust Center](docs/trust/README.md) |
| Contribute code, docs, testing, or ideas | [Contributing guide](CONTRIBUTING.md) |
| Read the architecture decisions | [ADR index](docs/adr/) |

## License and continuity

The browser automation engine outside `crates/core/src/governance/` is Apache-2.0 OR MIT. The
governance module is source-available under the Ghostlight Commercial License and is free for
individuals, teams of up to five, evaluation, development, all-open operation, and qualifying
noncommercial use. [LICENSING.md](LICENSING.md) maps the exact boundary.

License state never changes runtime behavior. An installed copy does not call a Ghostlight service
for telemetry, activation, or updates, and it keeps working without the vendor. The
[Continuity Promise](docs/trust/continuity.md) and [PRICING.md](PRICING.md) carry the durable terms.

## Questions and contributing

Use [GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects and
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions and ideas.
Use hello@sylin.org for security, licensing, or information that cannot be public.

[CONTRIBUTING.md](CONTRIBUTING.md) explains the project's boundaries, test expectations, and ways
to participate.
