<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight MCP</h1>

<p align="center"><strong>Give your agent a visible place in the browser you already use.</strong></p>

<p align="center">
  <a href="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml"><img src="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml/badge.svg?branch=dev" alt="CI"></a>
  <a href="https://www.npmjs.com/package/ghostlight"><img src="https://img.shields.io/npm/v/ghostlight?color=38BDF8&label=npm" alt="npm"></a>
  <a href="https://github.com/sylin-org/ghostlight/releases/latest"><img src="https://img.shields.io/github/v/release/sylin-org/ghostlight?color=38BDF8&label=release" alt="release"></a>
  <a href="https://registry.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP_registry-org.sylin%2Fghostlight-38BDF8" alt="MCP registry"></a>
</p>

<p align="center"><img src="docs/assets/demo.gif" alt="Ghostlight reading and completing a launch brief in a real browser with visible page, field, and click feedback" width="838" height="766"></p>
<p align="center"><sub>A launch brief moves from empty form to ready for review, in full view.</sub></p>

<p align="center"><a href="#your-first-five-minutes"><strong>Install Ghostlight</strong></a> | <a href="https://sylin.org/ghostlight/decision-aid/">See where it fits</a> | <a href="docs/guides/installation.md">Installation guide</a> | <a href="docs/trust/README.md">Trust Center</a></p>

Your agent needs a page you are signed in to. The usual answer is a second, empty browser that
knows none of your sessions, driven by a model that has to learn Chrome internals to get anything
done.

Ghostlight gives it a tab group inside the Chromium you already have open. The work happens in
front of you: watch it, pause it, take the wheel, or end the session. The model says what it wants,
and Ghostlight does the browser part.

Ask an agent to read a page, complete a form, handle a file, follow a popup, or investigate a
failed web workflow. Ghostlight carries the task across tabs and browser changes while keeping the
browser work and its controls on your machine.

> A light left burning, so the halls stay safe.

## Where it stands today

The published release is 1.3. It is available as the GitHub release
[`v1.3.2`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.2), the npm package
`ghostlight@1.3.2`, the Chrome Web Store adapter v1.0.0 (adapter 1.1.0 is in review), and the
MCP Registry record `org.sylin/ghostlight 1.3.2`, all observed on 2026-09-02 and recorded in
[`docs/public-status.json`](docs/public-status.json).

## What you get

- **24 catalog tools**: 23 browser tools covering tabs, navigation, reading a page, screenshots,
  semantic clicks and hovers, form input, file upload, scripts, waits, short sequences, and
  dialogs, plus one policy tool that explains the authority in force. One call carries
  the intent; Ghostlight performs the browser steps behind it.
- **One truthful answer per call**: what happened, what changed in the browser, what is ready, and
  whether running it again is safe. Ghostlight writes that answer, never the page, and adds at most
  two recovery steps of its own. When an effect is uncertain it says so rather than guessing, and
  never proposes a replay that could submit a form twice.
- **A desktop workbench** in the tray that shows work as it happens.
- **Your machine, and only your machine.** Ghostlight runs as you, reaches your browser over local
  IPC, and keeps a payload-free local record. No account, no telemetry, no activation service, no
  update ping, no hosted control plane, and no second hidden browser. The only network traffic is
  the browsing you asked for.

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
click or write. Next time, it reuses that group and the nearest tab it already owns on that site,
so repeated work stops littering your tab strip.

If a step needs attention, run:

```sh
npx -y ghostlight doctor
```

`doctor` checks the client entry, local service, browser connection, and extension, then names the
next action. The [installation guide](docs/guides/installation.md) covers targeted clients, source
builds, updates, uninstall, and symptom-led recovery.

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
ask the policy tool to explain the authority in force at any moment.

## The browser stays a shared space

Ghostlight works in a dedicated sky-blue tab group inside the browser window you chose. Page
scans, clicks, typing, drags, and longer phases share one visual language, so the movement on screen
has an explanation.

Pause the workspace, take over for a delicate step, or stop it. Move its tabs where you want them;
Ghostlight follows the workspace instead of snapping it back. Ordinary tabs remain outside the
agent's owned set.

Closing a tab needs two independent yes votes: the orchestrator's authority, and the browser's own
preserve-tabs setting, which ships on. That keeps the evidence of what happened in front of you.
Closing a tab yourself always works.

Personal use is complete without a policy manifest. Start with the full browser engine and get
useful work done. When a workflow needs stronger boundaries, grant `read`, `action`, `write`, and
`execute` capabilities by MCP identity and domain. Add sacred domains, dry-run preflight, and
structured audit while the browser experience stays the same. The
[governance guide](docs/guides/governance-configuration.md) shows the operating model, and the
[Trust Center](docs/trust/README.md) carries the security, privacy, continuity, deployment, and
procurement evidence.

## What it will and will not do

With no policy configured, ordinary remote HTTP(S) browsing is allowed. Loopback addresses,
link-local metadata endpoints, non-HTTP schemes, credential fields, and stale handles stay
protected regardless. Optional local and managed policy layers can only take capability away, and
per-request restrictions narrow things further. Nothing hands access back.

Credential-class fields come to you. Ghostlight does not type secrets.

The audit record holds identifiers, decisions, and content-minimized measurements: which tool ran,
whether authority allowed it, how long it took, and what it did -- 3 fields, 1,240 words, 1280x720.
The site an action landed on is named, because that answers where your agent went and is already in
your own tab strip. Paths, queries, fragments, page text, field values, screenshots, selectors, and
dialog text never enter it. [`docs/guides/siem-integration.md`](docs/guides/siem-integration.md) is
the exact record shape.

The full catalog is in [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md), and the exact policy schema
is in [`docs/guides/governance-configuration.md`](docs/guides/governance-configuration.md).

## The workbench

Open the tray icon and you land **At a glance**: the action running right now in full, finished
actions stacking below it, newest first, each in Ghostlight's own words -- "Opened example.com.",
"Read 1,240 words.", "Filled 3 fields and submitted the form." -- never the page's. Beside it sit
**MCP integrations**, which connects the coding clients you already have and merges into their
configuration with a backup; **Status**, which answers whether the stack is healthy; **Policy**,
which states what the current rules allow, one plain line per capability, naming the layer that
decided each one; and **About**, which carries the promise underneath it: it never phones home.

Pause and resume sit in the header beside the lamp, the same control the tray offers. Closing the
window returns it to the tray and leaves the authority running. If the desktop shell cannot start,
Ghostlight exits instead of leaving an invisible authority.

## Build it from source

Rust 1.82 or newer, plus Chromium 116 or newer for browser validation.

```sh
cargo build --workspace
```

Three executables land side by side:

- `ghostlight` -- the orchestrator and the desktop workbench;
- `ghostlight-mcp-connector` -- the MCP stdio edge;
- `ghostlight-browser-connector` -- the Chromium native-messaging relay.

```sh
target/debug/ghostlight open
```

That shows the workbench, or focuses the one already running. Then open **MCP integrations**,
connect the client you want, and restart or reconnect it. [`docs/DEV-LOOP.md`](docs/DEV-LOOP.md)
covers browser registration and the full validation loop.

After that first setup there is no startup ritual: launching a connected MCP client or Chromium
demand-starts Ghostlight when it is not already running. There is no service-only launch mode.

The one-command install is the primary journey:

```sh
npx -y ghostlight install
```

Signed-checksum native packages, portable archives, and a self-contained Claude Desktop MCPB are
equivalent release routes. Every route uses the matching store adapter and the same three native
executables.

<details>
<summary><strong>Current release and compatibility</strong></summary>

**Platform state.** Windows and Linux are the supported 1.0 platforms, verified against live
browsers on development hosts; the clean installed-product evidence lanes continue after
publication. macOS has no 1.0 artifact yet.

**Extension state.** The Chrome Web Store listing serves adapter v1.0.0, matching the published
1.0.0 service line.

The service and Chrome adapter version independently. The
[compatibility map](compatibility.json) is authoritative, and the
[public status file](docs/public-status.json) owns current release, platform, and store state.

The MCP edge negotiates a compatible stdio revision per client; the compatibility map records
them. See the [changelog](CHANGELOG.md) for release changes and upgrade consequences.

</details>

<details>
<summary><strong>How Ghostlight fits together</strong></summary>

```text
MCP Client <--stdio--> ghostlight-mcp-connector <--typed IPC--> ghostlight orchestrator
    <--browser IPC--> ghostlight-browser-connector <--native messaging--> Extension <--CDP--> Browser
```

The orchestrator makes every product decision and owns everything the model reads. The two
connectors carry protocol and relay lifecycle, nothing more. The extension owns Chromium, the
page, and the drawing, and never policy. Adding a feature normally means changing the orchestrator
alone; that is a contract the shores are held to, not a happy accident.

The desktop is a presentation adapter inside the same process, not a second service. It has no GUI
protocol, command runner, filesystem access, or browser primitives.
[`ADR-0102`](docs/adr/0102-integrated-desktop-workbench.md) records why.
[`docs/SPEC.md`](docs/SPEC.md) gives the deeper governance model.

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
| Read the product promise and journeys | [`docs/1.0/INTENT.md`](docs/1.0/INTENT.md) |
| Read the complete model-facing language | [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md) |
| Read the architecture and acceptance contracts | [`docs/1.0/ARCHITECTURE.md`](docs/1.0/ARCHITECTURE.md), [`docs/1.0/ACCEPTANCE.md`](docs/1.0/ACCEPTANCE.md) |
| See where the candidate stands | [`docs/STATUS.md`](docs/STATUS.md) |
| Contribute code, docs, testing, or ideas | [Contributing guide](CONTRIBUTING.md) |
| Read every decision, and why | [ADR index](docs/adr/) |

## License and continuity

Ghostlight is entirely free and open source: everything in this repository, including the
governance module, is Apache-2.0 OR MIT. [`LICENSING.md`](LICENSING.md) explains what that
covers. The former open-core split and its paid tiers were withdrawn by
[ADR-0140](docs/adr/0140-fully-open-source-licensing.md).

License state never reaches runtime -- there is no license check to reach it. An installed copy
keeps working on its own terms, with no check-in and no expiry. The
[Continuity Promise](docs/trust/continuity.md) carries the durable version of that.

## Questions and contributions

[GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects,
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions and ideas,
and hello@sylin.org for security, licensing, or anything that should not be public.

I build Ghostlight in partnership with AI coding agents.
[`CONTRIBUTING.md`](CONTRIBUTING.md) explains the current boundaries and the gates every change
passes.
