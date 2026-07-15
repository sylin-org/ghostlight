<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight</h1>

<p align="center"><strong>Your real browser, for any MCP agent -- visible, local, and yours.</strong></p>

<p align="center">
  <a href="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml"><img src="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml/badge.svg?branch=dev" alt="CI"></a>
  <a href="https://www.npmjs.com/package/ghostlight"><img src="https://img.shields.io/npm/v/ghostlight?color=38BDF8&label=npm" alt="npm"></a>
  <a href="https://github.com/sylin-org/ghostlight/releases/latest"><img src="https://img.shields.io/github/v/release/sylin-org/ghostlight?color=38BDF8&label=release" alt="release"></a>
  <a href="https://registry.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP_registry-org.sylin%2Fghostlight-38BDF8" alt="MCP registry"></a>
  <a href="https://github.com/sylin-org/homebrew-tap"><img src="https://img.shields.io/badge/Homebrew-sylin--org%2Ftap-38BDF8" alt="Homebrew tap"></a>
</p>

Ghostlight lets an AI agent use the Chromium profile where you are already signed in, inside a
dedicated tab group that stays separate from your ordinary tabs. Actions happen in front of you. It
works wide open for personal use or under inspectable policy when stronger boundaries are useful.
Everything runs locally, and nothing phones home.

**No account and no subscription trap.** The local browser automation core is Apache-2.0 OR MIT
and runs without a Ghostlight login, activation server, telemetry, or subscription. Organizational
governance is a separately licensed source-available layer. Personal use, evaluation, development,
small teams, and unrestricted all-open operation remain free under the exact terms linked in
[License](#license).

Responsibility is part of the experience, not a brake on it. A good agent tool should be easy to
start, obvious while it acts, clear when it stops, and honest about what it can and cannot control.

<p align="center"><a href="#try-it"><strong>Install and try it</strong></a> | <a href="docs/guides/installation.md">Every install path</a> | <a href="docs/COMPARISON.md">Compare alternatives</a></p>

<!-- HERO DEMO SLOT: an annotated session GIF captured from Ghostlight's built-in live tour
     (sky-blue click rings, action labels, compact narration, watermark). Record `ghostlight demo`,
     export a GIF under ~5 MB, then uncomment:
<p align="center"><img src="docs/assets/demo.gif" alt="Ghostlight driving a real browser: sky-blue click ripples, action captions, and a governed session in its own tab group"></p>
<p align="center"><sub>Ghostlight driving its own live demo stage, including a visible policy guardrail.</sub></p>
-->

## Is this your problem?

Ghostlight is worth trying when:

- your agent needs a site where you are already signed in, not a fresh browser profile;
- you want the same browser tools from Codex, Claude, Cursor, VS Code, Windsurf, Zed, OpenCode,
  Crush, or another MCP client;
- you want to watch the work, take the wheel, and understand failures without decoding logs; or
- you need policy and evidence that a developer and a security reviewer can both inspect.

It is probably not the right tool for a headless scraping farm, stealth automation, an isolated
cloud browser, or a Claude-only setup already served by Anthropic's first-party integration. The
[comparison guide](docs/COMPARISON.md) is candid about those choices.

## What makes it feel different

- **Your session, not a clean-room.** Real cookies and real SSO, used only in Ghostlight-managed
  tabs. Nothing gets relocated to a cloud browser or a throwaway profile just to gain a technical
  property; the whole point is your authenticated context without opening your ordinary tabs.
- **The agent gets a tool surface shaped for models.** The trained schemas stay byte-stable;
  additive tools provide forms, files, multi-step composition, recording, and inspection. Results
  are compact, errors say how to recover, and a capable agent can begin without a Ghostlight lesson.
- **The person watching can follow along.** Every move is visible: sky-blue click ripples, a comet trail on
  drags, a shimmer as it types, captions that narrate each step. It runs in its own tab group, kept
  visually separate from your own tabs, and you can grab the wheel or hit the kill switch at any
  moment.
- **Responsibility scales with the job.** Personal use needs no policy. When boundaries matter,
  Ghostlight adds capability grants, sacred never-touch domains, dry-run preflight, and one
  structured record per call. The unrestricted engine remains a first-class product.
- **The software stays yours.** The Rust service and its thin relay have no runtime framework to
  maintain. The engine is open source, the governance code is readable, license state never changes
  behavior, and an installed copy keeps working offline.

## Try it

Needs a Chromium browser (116+), an MCP client, and Node for the `npx` install path. The running
service is native Rust; there is no Node service to keep alive and nothing to compile.

```text
[1 Install service] -> [2 Add extension] -> [3 Restart MCP client] -> [4 Ask a first task]
      automatic           visible step             once                useful proof
```

1. **Install the local service and register detected MCP clients:**

   ```sh
   npx -y ghostlight install
   ```

   The installer is idempotent and opens the extension walkthrough on the first run. It recognizes
   Claude Code, Claude Desktop, Cursor, VS Code, Codex, Windsurf, Zed, OpenCode, and Crush. Use
   `--client <id>` to target one client, or `--dry-run` to inspect every planned change first.

2. **Add the extension.** Until the Chrome Web Store listing is public, download
   `ghostlight-extension-v*.zip` from the
   [latest release](https://github.com/sylin-org/ghostlight/releases/latest) and load it unpacked
   at `chrome://extensions`. The walkthrough opened by the installer shows the same current path.

3. **Restart your MCP clients.** The browser tools appear. Try:

   > In my current browser, summarize the active page and tell me which tab you used. Do not click
   > or change anything.

If anything looks off, `npx ghostlight doctor` tells you exactly what. Prebuilt archives, building
from source, and every other path live in the
[installation guide](docs/guides/installation.md) and the manual steps below.

For an unsupported client, add `{ "command": "npx", "args": ["-y", "ghostlight"] }` as a stdio
MCP server, then run the installer for the browser side. If VS Code reports a manual client step,
use its native installer:

[![Add to VS Code](https://img.shields.io/badge/VS_Code-Add_MCP_server-38BDF8?style=flat-square)](vscode:mcp/install?%7B%22name%22%3A%22ghostlight%22%2C%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22ghostlight%22%5D%7D)

**Current platform state.** Windows and Linux are verified end to end against live browsers.
macOS builds and passes the full test suite in CI; its live-browser verification is still owed.
The Chrome Web Store listing is under review.

**Other ways to get it.** Homebrew: `brew install sylin-org/tap/ghostlight`. On the
[MCP registry](https://registry.modelcontextprotocol.io) as `org.sylin/ghostlight`. Every release
also ships prebuilt binaries and checksums on the
[Releases page](https://github.com/sylin-org/ghostlight/releases/latest).

<details>
<summary><strong>Manual install (inspect everything)</strong></summary>

1. **Get the binary.** Download a prebuilt archive from the
   [Releases page](https://github.com/sylin-org/ghostlight/releases/latest) (each carries a
   SHA-256 checksum and a signed build-provenance attestation:
   `gh attestation verify <archive> --repo sylin-org/ghostlight`), or build from source with a
   stable Rust toolchain: `cargo build --release`. The build produces two executables:
   `ghostlight` (the CLI) and `ghostlight-relay`, the thin pass-through that your MCP client
   and Chrome launch.
2. **Load the extension.** `chrome://extensions` -> Developer mode -> Load unpacked -> the
   `extension/` directory. The committed manifest key pins the extension ID, and the installer
   already allows it, so there is nothing to copy.
3. **Register.** `./target/release/ghostlight install`. Useful flags: `--dry-run` (print the
   plan, write nothing), `--browser <id>` / `--client <id>` (limit scope; repeatable),
   `--all-browsers` / `--all-clients`, `--no-open`, `--debug` (observability on), and `--system`
   (machine-wide).
   The installer is an idempotent value-level merge; it never clobbers your config and never
   duplicates entries.
4. **Restart the client, reload the extension, run `ghostlight doctor`.** A healthy result
   reports registration, a live endpoint, and a connected extension; anything off gets a
   specific, actionable finding.

</details>

## What the agent can do

A typical first request:

> Open a new browser tab, go to example.com, and tell me what the page says.

The tool surface preserves the schemas Claude was trained on, byte for byte, then adds more on
top, for 25 tools in five groups. (Everything behind those schemas is an original, clean-room
Rust implementation.)

- **See and act.** Navigate, click, type, scroll, hover, drag; screenshots with exact coordinate
  mapping and an on-page cursor; semantic one-call actions with bounded outcome receipts.
- **Forms and files.** Fill forms by element ref or semantically by label (shadow DOM included);
  upload file bytes or captured screenshots straight into page inputs and drop targets.
- **Compose.** Multi-step scripts with inter-step data flow and `dry_run` pre-flight; one-call
  action batches; wait-for-condition with page settlement; timed narration at meaningful workflow
  phases for the person watching.
- **Record.** Animated-GIF session recording with click cues, action labels, a truthful REC badge,
  and real per-frame timing.
- **Inspect.** Accessibility tree (with diff mode), page text, actionable element search, console
  and network activity, JavaScript dialogs, and explicit owned-tab lifecycle controls.

Ask the agent to call `explain` at any time for the authoritative, in-session directory of every
action and the capability it requires.

<details>
<summary><strong>The full tool table</strong></summary>

| Tool                    | What it does                                     | Capability                 |
| ----------------------- | ------------------------------------------------ | -------------------------- |
| `navigate`              | Go to a URL, or forward/back in history          | read                       |
| `computer`              | Mouse, keyboard, and screenshots (13 actions)    | read or action, per action |
| `read_page`             | Accessibility-tree view of the page              | read                       |
| `get_page_text`         | Visible text extraction                          | read                       |
| `find`                  | Locate elements on the page                      | read                       |
| `form_input`            | Fill form fields, including shadow DOM           | write                      |
| `javascript_tool`       | Run JavaScript in the page context               | execute                    |
| `tabs_context_mcp`      | List tabs in the MCP tab group                   | read                       |
| `tabs_create_mcp`       | Create a tab in the MCP tab group                | none                       |
| `read_console_messages` | Recent console output                            | read                       |
| `read_network_requests` | Recent network activity                          | read                       |
| `resize_window`         | Resize the browser window                        | none                       |
| `update_plan`           | Record the agent's working plan                  | none                       |
| `narrate`               | Show timed agent commentary without touching page content | none              |
| `wait_for`              | Wait for a page condition and settlement         | read                       |
| `script`                | Run a sequence of tool calls in one request (with optional `dry_run`) | none |
| `form_fill`             | Fill a form by field labels in one call          | read + write (or read + write + action when `submit: true`) |
| `act_on`                | Resolve, act on, and observe one semantic target | read, action, or write, per action |
| `dialog`                | Inspect or explicitly resolve a JavaScript dialog | read or action, per action |
| `tab_control`           | Focus, reload, or close one owned tab            | none or action, per action |
| `file_upload`           | Upload file bytes to a file `<input>` on the page | write                     |
| `browser_batch`         | Run a batch of browser actions in one call       | none                       |
| `upload_image`          | Place a captured screenshot into a file input or drop target | write          |
| `gif_creator`           | Record a session and export it as an animated GIF | read or write, per action |
| `explain`               | List every action and the capability it requires | none                       |

For `computer`, the read-only actions (`screenshot`, `scroll`, `zoom`, `scroll_to`, `hover`)
require `read`, the input actions (`left_click`, `right_click`, `type`, `key`,
`left_click_drag`, `double_click`, `triple_click`) require `action`, and `wait` requires none.

</details>

## Governed, honestly

Governance is off by default and switches on when a policy manifest is present. A manifest grants
capabilities (`read`, `action`, `write`, `execute`) to an identity on the hosts you name, with
`deny` carve-outs, and every call resolves against it at a single chokepoint:

```json
{
  "schema": 3,
  "name": "acme-dev",
  "version": "2026.07.0",
  "identity": { "resolved_by": "local_file", "principal": "dev@acme" },
  "grants": [
    { "id": "acme-apps",
      "hosts": { "allow": ["*.acme.com"], "deny": ["payroll.acme.com"] },
      "allowed": ["read", "action", "write"] }
  ],
  "config": [
    { "key": "content.security.sacred_domains", "value": ["*.mybank.com"], "level": "mandatory" }
  ]
}
```

(That exact file renders as plain English with `ghostlight policy explain`. Try it.)

- **Capabilities, not tool lists.** Every action carries an intrinsic classification. The
  vocabulary is published as an open, vendor-neutral spec: the
  [RAWX capability model](open-spec/rawx-capability-model.md) (`rwx` for agents).
- **Observe before you enforce.** `observe` mode dispatches everything and records what enforce
  *would have* denied; `enforce` blocks. Sacred never-touch domains always enforce.
- **Evidence built in.** Every call, whether permitted, denied, or shadow-denied, emits one
  structured JSON-Lines audit record: identity, host, capability, grant, decision, duration. The
  recorder is on by default even in all-open mode, so a session always leaves a trail. Stream it to
  a file, stderr, or syslog for your SIEM ([guide](docs/guides/siem-integration.md)).
- **Live and layered.** Manifests hot-reload without a restart (failing closed on a bad edit);
  configuration resolves through defaults, org policy, and user layers, with org locks.

A governed client only *sees* the tools its grants permit, plus `explain`. Start from a ready
manifest in [`examples/`](examples/), preview any file with `ghostlight policy explain <file>`,
and see the [governance configuration guide](docs/guides/governance-configuration.md) for the
mechanics, with the [solo-developer](docs/guides/solo-developer.md) and
[compliance-team](docs/guides/compliance-team.md) walkthroughs for the full journey.

**Reviewing Ghostlight for procurement or a security assessment?** The
[Trust Center](docs/trust/README.md) is public and ungated: it answers the questions reviewers
ask first, each with linked evidence, and ships a CAIQ-shaped questionnaire plus MSA and DPA
templates you can read and file before you ever reach out.

## How it works

```
MCP Client <--stdio--> Relay <--local IPC--> Service <--native messaging--> Relay
                                                                        |
                                                                     Extension <--CDP--> Browser
```

The persistent Rust service owns browser sessions, governance, and audit. The two roles that MCP
clients and Chromium spawn are handled by one small `ghostlight-relay` executable. The extension is
deliberately thin: it contains Chrome-API mechanism, not policy. All decisions and records stay in
the local service. The separation lets clients, the extension worker, and the service restart
without making the user rebuild their browser session.

<details>
<summary><strong>CLI and troubleshooting</strong></summary>

- No subcommand: prints a short hint and exits. The MCP server role now lives in `ghostlight-relay`
  (your client launches `ghostlight-relay --role agent`; you never run it by hand).
- `install` / `uninstall`: register or remove everything (both support `--dry-run`).
- `doctor [--verbose]`: read-only diagnosis of the whole chain with a truthful exit code.
- `status [--json]`: a running server's live inner state (requires `--debug` /
  `GHOSTLIGHT_DEBUG=1`).
- `config <list|get|set|schema|docs|preset>`: the layered configuration, with sources and locks.
- `policy <explain|simulate|init>`: render a manifest as plain sentences, replay an audit log
  against a candidate policy, or write a starter manifest.

**If something is off, start with `doctor`.** It pinpoints unregistered browsers or clients, a
missing server, a stale endpoint, or an extension that never connected. Extension shows
disconnected? Reload it at `chrome://extensions`. Developing on Windows? Use the isolated engine
swap in [docs/DEV-LOOP.md](docs/DEV-LOOP.md); live clients and the browser reconnect around it.

</details>

## Documentation

| Doc                                                                 | What it is                                                              |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| [Guides & how-tos](docs/guides/)                                    | Install, configure governance, roll it out to a team, ship audit to a SIEM, manage a license. |
| [docs/COMPARISON.md](docs/COMPARISON.md)                            | A candid comparison with the alternatives.                               |
| [ROADMAP.md](ROADMAP.md)                                            | What we are building next, and the direction behind it.                  |
| [PRICING.md](PRICING.md)                                            | Editions, the founding program, and the Continuity Promise.              |
| [CONTRIBUTING.md](CONTRIBUTING.md)                                  | How to ask questions, request features, and contribute code.             |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)                            | The standards for participating in this community.                       |
| [SECURITY.md](SECURITY.md)                                          | Vulnerability reporting and what to expect.                              |
| [MAINTENANCE.md](MAINTENANCE.md)                                    | Who maintains it, the Continuity Promise, and how to pick it up.         |
| [Trust Center](docs/trust/README.md)                                | Procurement and security review, all public: FAQ, security overview, a CAIQ-shaped questionnaire, and MSA/DPA templates. |
| [docs/SPEC.md](docs/SPEC.md)                                        | The original deep design specification; ADRs and the live tree supersede it where they differ. |
| [docs/adr/](docs/adr/)                                              | Authoritative architecture decisions and amendments.                     |
| [open-spec/](open-spec/)                                            | Open specs we publish for the ecosystem (starts with RAWX).              |

## Questions, requests, and contributing

[GitHub Issues](../../issues) for bugs, [GitHub Discussions](../../discussions) for questions and
ideas, and hello@sylin.org for anything that cannot be public. Every request gets a disposition
with reasoning: accepted, deferred, or declined against the project's recorded vision. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

**The Continuity Promise comes first:** license state never affects behavior, and the binary never
phones home. Ghostlight runs the same whether or not anyone ever pays.

Ghostlight is open-core. The engine (everything outside `crates/core/src/governance/`) is open
source under Apache-2.0 OR MIT, at your option. The governance module
(`crates/core/src/governance/`) is source-available under the Ghostlight Commercial License, and it
is free for almost everyone: individuals and solo developers, teams of up to five, evaluation and
development at any size, all-open operation at any size, and noncommercial nonprofit or open-source
use. Exactly one situation needs a paid subscription: an organization of more than five people
running the governance features operationally.

See [LICENSING.md](LICENSING.md) for the plain-language guide, and, when you want them,
[PRICING.md](PRICING.md) for editions, prices, and the founding program (12 months free for the
first ten organizations).

---

<p align="center"><sub>Ghostlight is the first of a planned family of governance-friendly MCP tools.<br>
The name is the theater's ghost light: the single lamp left burning so the stage is never fully dark.</sub></p>
