<p align="center">
  <img src="extension/icons/icon512.png" alt="Ghostlight mascot" width="128" height="128">
</p>

# Ghostlight

[![CI](https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml/badge.svg?branch=dev)](https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml)

**Governed access to your own browser, for AI agents.**

Ghostlight is a single Rust binary plus a thin Chromium (Manifest V3) extension that gives
an AI agent controlled access to your real, authenticated browser session. It drives the
browser you are already logged in to, so the agent can observe and act on the web apps you already
use, through any MCP client (Claude Code, Cursor, and others). A separable governance layer decides,
per call, what the agent is allowed to do.

## What it does

Two concerns, one binary: a full browser-automation engine, and a governance layer that decides,
per call, what the agent may do.

- **The full tool surface.** The 13 trained tools at byte-parity with the official Claude-in-Chrome
  schemas, plus four additive tools -- `wait_for`, `script`, `form_fill`, and `explain`: screenshots
  with coordinate mapping, an on-page agent cursor, accessibility-tree and text reads, form input
  (including shadow DOM), in-page JavaScript, console and network inspection, tab management,
  condition-and-settlement waiting, sequential multi-step scripts with inter-step data flow, and
  semantic form filling by label. Structured results (`structuredContent`) on the tools that carry
  one, `dry_run` pre-flight verdicts on `script`, `read_page` diff mode, and consequence digests on
  mutating actions round out the surface.
- **The governance layer.** Capability-based policy manifests (per-call `read` / `action` / `write`
  / `execute` classification), identity-bound domain grants with allow/deny host polarity, sacred
  never-touch domains, a take-the-wheel pause and a panic kill switch, `observe` and `enforce` modes
  (observe records shadow denials without blocking), and structured JSON-Lines audit to file,
  stderr, or RFC 5424 syslog. Layered configuration with organization policy locks, and live
  manifest hot-reload: edit the policy file and the running session re-resolves with no restart,
  failing closed on a bad edit.
- **All-open is first-class.** With no manifest, the engine runs unrestricted -- the agent has the
  full capability surface, with no access decisions beyond secret-field redaction. Governance is an
  overlay you opt into, not a stripped-down build.
- **Operability.** A single portable binary with a built-in installer, a `doctor` diagnostic, a
  layered `config` CLI, and a `policy` CLI (explain / simulate / init).

Windows, macOS, and Linux all build and pass the full test suite in CI; end-to-end browser use is
verified on Windows.

## What makes it different

- **Bring your own agent.** It speaks the Model Context Protocol, so it works with any MCP client
  against the browser you already use. You are not locked into one vendor's app or cloud.
- **It is your session, not a clean-room browser.** The value is your own authenticated context:
  real cookies, real SSO, real tabs. Your work is never relocated to a cloud or a freshly launched
  browser to gain a technical property.
- **Governance fused with the engine, not bolted on.** Access control, capability classification,
  and audit live at a single dispatch chokepoint in the binary. A governed client only sees the
  tools its grants permit, and every call is checked and recorded. All-open is a first-class
  supported mode.
- **Single portable binary, zero runtime dependencies.** No Node.js, no `npx`, no separate servers
  to babysit. The class of install failures that affects Node-based browser MCPs does not exist.

## Requirements

- A Chromium browser: Chrome, Edge, Brave, or Chromium, version 116 or newer.
- An MCP client: Claude Code, Claude Desktop, Cursor, or VS Code.
- A Rust toolchain (stable) to build the binary. Install from https://rustup.rs.

## Getting started

### Quick install (two minutes)

One command downloads the latest release, registers the browser connection, and adds Ghostlight
to every MCP client it finds (idempotent value-level merge; it never clobbers your config):

```sh
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.sh | sh
```

```powershell
# Windows (PowerShell)
irm https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/get.ps1 | iex
```

Then add the **"Ghostlight in Browser"** extension
([Chrome Web Store](https://chromewebstore.google.com/detail/lejccfmoeogmhemakeknjjdhkfkgncdl), or
load the release zip unpacked) and restart your MCP client. Verify with `ghostlight doctor`.

No install at all: any MCP client can launch Ghostlight via npx --

```json
{ "command": "npx", "args": ["-y", "ghostlight"] }
```

[![Add to Cursor](https://img.shields.io/badge/Cursor-Add_MCP_server-38BDF8?style=flat-square)](cursor://anysphere.cursor-deeplink/mcp/install?name=ghostlight&config=eyJjb21tYW5kIjoibnB4IiwiYXJncyI6WyIteSIsImdob3N0bGlnaHQiXX0=)
[![Add to VS Code](https://img.shields.io/badge/VS_Code-Add_MCP_server-38BDF8?style=flat-square)](vscode:mcp/install?%7B%22name%22%3A%22ghostlight%22%2C%22command%22%3A%22npx%22%2C%22args%22%3A%5B%22-y%22%2C%22ghostlight%22%5D%7D)

```sh
# Claude Code
claude mcp add ghostlight -- npx -y ghostlight
```

(after adding via npx, run `npx ghostlight install` once to connect the browser extension).
Rust users: `cargo binstall --git https://github.com/sylin-org/ghostlight ghostlight`. The
walkthrough with all paths: https://sylin-org.github.io/ghostlight/install.html

The manual, inspect-everything route:

### 1. Get the binary

Download a prebuilt archive for your platform from the
[Releases page](https://github.com/sylin-org/ghostlight/releases/latest) and extract it, or build
from source:

```sh
git clone https://github.com/sylin-org/ghostlight
cd ghostlight
cargo build --release
```

The binary is at `target/release/ghostlight` (`ghostlight.exe` on Windows). All commands below run
that binary. Every release archive carries a SHA-256 checksum and a signed build-provenance
attestation (`gh attestation verify <archive> --repo sylin-org/ghostlight`).

### 2. Load the extension in Chrome

1. Open `chrome://extensions`.
2. Turn on **Developer mode** (top right).
3. Click **Load unpacked** and select the `extension/` directory of this repo.
4. Note the extension ID that Chrome assigns. The committed manifest key pins it to a stable value:
   `cjcmhepmagomefjggkcohdbfemacojoa`. Confirm the ID shown matches; you will pass it to the
   installer.

### 3. Register the native host and your MCP client

Run the installer from the binary you just built. It registers the native-messaging host with your
detected browsers and adds the MCP server entry to your detected MCP clients:

```sh
./target/release/ghostlight install --extension-id cjcmhepmagomefjggkcohdbfemacojoa
```

Useful flags:

- `--dry-run` computes and prints the plan without writing anything. Run this first to see what will
  change.
- `--browser <id>` limits registration to one browser (`chrome`, `edge`, `brave`, `chromium`).
  Repeatable. `--all-browsers` registers every known browser, not just detected ones.
- `--client <id>` limits registration to one client (`claude-code`, `claude-desktop`, `cursor`,
  `vscode`). Repeatable. `--all-clients` adds to every known client.
- `--debug` registers the server to run with observability on (see Troubleshooting).
- `--system` registers machine-wide (HKLM on Windows) instead of per-user.

The installer is idempotent: re-running it will not create duplicate entries.

### 4. Restart the client and reload the extension

- Restart (or reload the MCP connection of) your MCP client so it picks up the new server entry.
- Reload the extension at `chrome://extensions` so it connects to the freshly registered host.

### 5. Verify

```sh
./target/release/ghostlight doctor
```

A healthy result reports that the browser and client are registered, the IPC endpoint accepts
connections, and the extension is connected. If anything is off, `doctor` prints a specific,
actionable finding for each problem.

## Using it

Once connected, the browser tools appear in your MCP client and the agent can drive your browser.
Ghostlight works inside its own tab group (labeled 👻Ghostlight), so agent activity is visually
separated from your own tabs. A typical first request to the agent:

> Open a new browser tab, go to example.com, and tell me what the page says.

The agent will create a tab in the MCP group, navigate, read the page, and report back. It can then
click, type, fill forms (by ref or by label), run JavaScript, take screenshots, inspect console and
network activity, wait for dynamic pages to settle, and compose multi-step scripts that chain
results -- all in your real logged-in session, subject to whatever governance policy is active.

### The tools

Each action carries a capability requirement. Under a governance manifest, the layer both filters
the advertised tool set to what your grants permit and checks the requirement on every call; with no
manifest (all-open) every action is allowed.

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
| `wait_for`              | Wait for a page condition and settlement         | read                       |
| `script`                | Run a sequence of tool calls in one request (with optional `dry_run`) | none |
| `form_fill`             | Fill a form by field labels in one call          | read + write (or read + write + action when `submit: true`) |
| `explain`               | List every action and the capability it requires | none                       |

For `computer`, the read-only actions (`screenshot`, `scroll`, `zoom`, `scroll_to`, `hover`) require
`read`, the input actions (`left_click`, `right_click`, `type`, `key`, `left_click_drag`,
`double_click`, `triple_click`) require `action`, and `wait` requires none. Ask the agent to call
`explain` at any time for the authoritative, in-session capability directory.

## Governance

Governance is off by default (all-open) and turns on when a policy manifest is present. A manifest is
a JSON document (schema 3) describing identity-bound **grants**: each grant names the hosts it covers
(`allow` patterns with optional `deny` carve-outs) and the capabilities it permits (`read`,
`action`, `write`, `execute`). The layer resolves each call against the active grants and either
dispatches it or returns a clear denial naming the capability, the host, and a stable denial id.

The model in brief:

- **Capabilities, not tool lists.** Every action is classified `read` / `action` / `write` /
  `execute` (some require none). Grants allow capabilities on hosts; the classification is intrinsic
  to each action. This vocabulary is published as an open, vendor-neutral spec -- the
  [RAWX capability model](open-spec/rawx-capability-model.md) (`rwx` for agents) -- so it can be
  adopted beyond Ghostlight.
- **Host polarity.** A grant's `allow` patterns can carry `deny` carve-outs, so "everywhere except
  this site" is expressible directly.
- **Sacred domains.** A never-touch list denies every tool on a matching tab, regardless of grants.
- **Modes.** `observe` dispatches every call and records what enforce would have denied (as
  `shadow_deny` audit records with the same stable denial ids); `enforce` blocks. Sacred and
  user-authored protections always enforce, in every mode.
- **Advertisement filtering.** A governed client sees only the tools its grants can use, plus
  `explain`.
- **Audit.** Every call produces one JSON-Lines record (permitted, denied, and shadow-denied alike):
  identity, host, capability, grant id, decision, denial id, duration, and manifest hash. Destinations:
  local file, stderr, or RFC 5424 syslog over UDP for SIEM ingestion (see the
  [SIEM guide](docs/guides/siem-integration.md)).
- **Layered configuration.** Settings resolve through built-in defaults, org policy, and a user
  layer, with organization policy able to lock keys. Inspect and edit with `ghostlight config`.
- **Hot-reload.** The org policy path and a `file://` user manifest are watched; edits re-resolve the
  running session with no restart, an advertised-set change re-advertises the tools, and an invalid
  edit keeps the last-good manifest (fail closed).

Manifest sources, in precedence order: a `--manifest file://...` flag (or `GHOSTLIGHT_MANIFEST`), then
the machine org policy path (`%ProgramData%\ghostlight\policy.json` on Windows;
`/Library/Application Support/ghostlight/policy.json` on macOS; `/etc/ghostlight/policy.json` on
Linux). No manifest means all-open. See `examples/` for ready-to-adapt manifests and preview any file
with `ghostlight policy explain <file>`.

## CLI reference

The binary has no-subcommand and subcommand modes:

- No subcommand: the MCP server role. Your MCP client launches this over stdio; you do not run it by
  hand.
- `install` / `uninstall`: register or remove the native host and the MCP client entries (see flags
  above; both support `--dry-run`).
- `doctor [--verbose]`: one-shot, read-only diagnosis of the whole chain (registration, IPC
  endpoint, extension link) with a truthful exit code. It never changes anything.
- `status [--json]`: print a running server's live inner state. Requires a server started with
  `--debug` (or `GHOSTLIGHT_DEBUG=1`).
- `config <list | get | set | schema | docs | preset>`: inspect and edit the layered configuration.
  `list` shows every key with its effective value, source layer, and lock state; `preset` selects a
  named bundle of defaults after previewing the change.
- `policy <explain | simulate | init>`: work with policy files without a browser. `explain` renders
  a manifest or config file as plain sentences; `simulate` replays a recorded audit log through a
  candidate manifest; `init` writes an embedded example manifest as a starting point.

## Troubleshooting

- **Start with `doctor`.** It pinpoints most problems: a browser or client that is not registered,
  no server running, a stale process holding the endpoint, or an extension that never connected.
- **Extension shows disconnected.** Reload it at `chrome://extensions`, make sure the browser is
  running, and confirm the extension ID matches what you passed to `install`.
- **Turn on observability.** Install with `--debug` (or set `GHOSTLIGHT_DEBUG=1` in the server's
  environment), then run `ghostlight status` to see live counters and per-session state.
- **Rebuilding the binary on Windows.** A running server locks `ghostlight.exe`. Stop the MCP
  client (and reload/close the extension) before `cargo build`, then restart both.

## Architecture

```
MCP Client  --stdio-->  Rust Binary  --native messaging-->  Extension  --CDP-->  Browser
 (agent)                (engine +      (4-byte framed)      (thin CDP           (your real
                         governance)                         executor)           session)
```

Three processes, two protocol boundaries. The binary is both the MCP server (over stdio) and the
browser's native-messaging host; the extension is a deliberately thin CDP executor. All capability
and all policy live in the binary, and the extension holds none. The governance layer attaches at a
single dispatch chokepoint inside the binary without touching any tool code.

## Roadmap

- Live browser verification on macOS and Linux (the binary already builds and ships for all four
  targets on the [Releases page](https://github.com/sylin-org/ghostlight/releases)).
- A Chrome Web Store listing, so the extension installs without developer mode.
- Offline license keys for organizations (see [PRICING.md](PRICING.md)), and an `http` audit
  destination alongside file, stderr, and syslog.
- `managed://` policy distribution for MDM and Group Policy fleets.
- More adapters on the same governance spine -- the browser is the first.

## Direction

The governance engine -- capability grants, host polarity, audit, layered configuration, and
licensing -- is domain-agnostic by design; the browser is its first adapter. The same
policy-and-audit spine is what any future adapter would reuse, which is why this is the first of a
planned family rather than a one-off. The vocabulary that engine speaks is published for the whole
ecosystem as the [RAWX capability model](open-spec/rawx-capability-model.md): the durable asset in
agent governance is the way you classify and grant capabilities, not the mechanism that carries
them, and mechanisms change.

## Documentation

| Doc                                                                | What it is                                                                                                |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| [docs/guides/solo-developer.md](docs/guides/solo-developer.md)     | Ten minutes from clone to a working agent, plus personal safety rails.                                    |
| [docs/guides/compliance-team.md](docs/guides/compliance-team.md)   | Taking a policy from blank page to org-wide enforcement, with evidence.                                   |
| [docs/guides/siem-integration.md](docs/guides/siem-integration.md) | Audit stream schema and Splunk / Sentinel / Elastic ingestion.                                            |
| [docs/COMPARISON.md](docs/COMPARISON.md)                           | How Ghostlight compares to the alternatives, honestly.                                                    |
| [PRICING.md](PRICING.md)                                           | Editions, the founding program, and the Continuity Promise.                                               |
| [CONTRIBUTING.md](CONTRIBUTING.md)                                 | How to ask questions, request features, and contribute code.                                              |
| [SECURITY.md](SECURITY.md)                                         | Vulnerability reporting and what to expect.                                                               |
| [docs/SPEC.md](docs/SPEC.md)                                       | The authoritative design specification.                                                                   |
| [docs/adr/](docs/adr/)                                             | Architecture Decision Records: the reasons behind the design and how it evolved.                          |
| [docs/design/](docs/design/)                                       | Forward-looking design discussions (family and service architecture).                                     |
| [open-spec/](open-spec/)                                           | Open, vendor-neutral specifications we publish for the ecosystem (starts with the RAWX capability model). |
| [docs/research/NORTH-STAR.md](docs/research/NORTH-STAR.md)         | Governing design principles.                                                                              |

## Positioning and prior art

This is a clean-room Rust rewrite informed by
[open-claude-in-chrome](https://github.com/noemica-io/open-claude-in-chrome), a Node.js
reimplementation of the Claude-in-Chrome extension. Prior art is studied as a concern surface (the
hazards and questions others hit), not as a feature catalog to copy. The tool schemas are preserved
verbatim so a trained agent behaves as expected; everything behind them is our own.

Anthropic now ships a first-party Claude Code + Chrome integration, and generic agent-governance
toolkits are emerging; we treat both as validation and meet them with alternatives and open
standards, not rivalry ([ADR-0041](docs/adr/0041-post-evaluation-response.md)).
[docs/COMPARISON.md](docs/COMPARISON.md) is the honest decision guide, including when the
first-party path is the better choice; [docs/research/14](docs/research/14-post-evaluation-2026-07.md)
carries the current landscape evidence.

## The name

Ghostlight is the brand for a planned family of governance-friendly MCP tools; this browser adapter
is the first. The theatrical ghost light metaphor sits alongside the publisher's register of
guardian-in-a-bounded-space names. See [docs/adr/0021-ghostlight-brand-and-family.md](docs/adr/0021-ghostlight-brand-and-family.md)
for the naming decision.

## Questions, requests, and contributing

Three lanes: [GitHub Issues](../../issues) for bugs, [GitHub Discussions](../../discussions) for
questions, ideas, and feature requests, and hello@sylin.org for anything that cannot be public
(security, licensing, or a compliance team that cannot post in the open). Every request gets a
disposition with reasoning -- accepted, deferred, or declined against the project's recorded
vision. See [CONTRIBUTING.md](CONTRIBUTING.md) for the details and the contribution terms.

## License

Ghostlight is open-core. The engine -- everything outside `src/governance/` -- is open source
under Apache-2.0 OR MIT, at your option. The governance module (`src/governance/`) is
source-available under the Ghostlight Commercial License: free for individuals and solo
developers, development, testing, evaluation, all-open operation, and noncommercial
nonprofit/open-source use; production use with governance configured by an organization
requires a commercial subscription. See [LICENSING.md](LICENSING.md) for the plain-language
guide and [docs/adr/0027-open-core-business-model-and-licensing.md](docs/adr/0027-open-core-business-model-and-licensing.md)
for the decision.

Editions, prices, the founding program (12 months free for the first ten organizations), and
the Continuity Promise -- license state never affects behavior, and the binary never phones
home -- are in [PRICING.md](PRICING.md).
