<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight MCP</h1>

<p align="center"><strong>Let your agent work in the browser you already use.</strong></p>

<p align="center">
  <a href="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml"><img src="https://github.com/sylin-org/ghostlight/actions/workflows/ci.yml/badge.svg?branch=dev" alt="CI"></a>
  <a href="https://www.npmjs.com/package/ghostlight"><img src="https://img.shields.io/npm/v/ghostlight?color=38BDF8&label=npm" alt="npm"></a>
  <a href="https://github.com/sylin-org/ghostlight/releases/latest"><img src="https://img.shields.io/github/v/release/sylin-org/ghostlight?color=38BDF8&label=release" alt="release"></a>
  <a href="https://registry.modelcontextprotocol.io"><img src="https://img.shields.io/badge/MCP_registry-org.sylin%2Fghostlight-38BDF8" alt="MCP registry"></a>
</p>

<p align="center"><img src="docs/assets/demo.gif" alt="Ghostlight reading and completing a launch brief in a real browser with visible page, field, and click feedback" width="838" height="766"></p>
<p align="center"><sub>Ghostlight reads, fills, and completes a brief while the person watches.</sub></p>

<p align="center"><a href="#try-one-useful-task"><strong>Install and try it</strong></a> | <a href="https://sylin.org/ghostlight/decision-aid/">See when it fits</a> | <a href="docs/guides/installation.md">Every install path</a> | <a href="docs/trust/README.md">Trust Center</a></p>

Ghostlight lets an AI agent work in the Chromium browser where you are already signed in. The
work happens in a dedicated visible tab group, you can interrupt or take over, and the runtime
stays on your machine. Personal use is complete without policy. Capability and domain grants plus
structured audit are there when a job needs stronger boundaries.

There is no Ghostlight account, hosted control plane, telemetry, activation service, or Node
service to keep running. Connect a supported client or another compatible local stdio MCP client,
then keep using the browser profile you chose.

## Is this a fit?

Ghostlight is worth trying when:

- the job needs a site where you are already signed in;
- you want to watch browser work and keep the wheel;
- the workflow crosses forms, tabs, files, or browser diagnostics;
- you want the same local browser capability from compatible MCP clients; or
- capability, domain, and audit boundaries may be useful now or later.

Choose [Playwright MCP](https://github.com/microsoft/playwright-mcp) for deterministic browser
testing or Playwright-native automation. Choose
[Chrome DevTools MCP](https://github.com/ChromeDevTools/chrome-devtools-mcp) when performance and
DevTools diagnostics are the center of the job. A first-party browser integration may be simpler
when one supported assistant is your whole environment. A hosted or headless browser is a better
fit when work must run away from the person's visible local browser.

Ghostlight does not target Firefox, stealth automation, scraping farms, or shared multi-tenant
browser service. The [decision aid](https://sylin.org/ghostlight/decision-aid/) and
[comparison guide](docs/COMPARISON.md) explain those choices without a feature-count scorecard.

## Try one useful task

You need a Chromium browser (Chrome, Edge, Brave, or Chromium 116+), an MCP client, and Node.js for
the `npx` launcher. The running service is native Rust.

1. Install the local service, browser connection, and detected MCP-client entries:

   ```sh
   npx -y ghostlight install
   ```

2. Install
   [Ghostlight in Browser](https://chromewebstore.google.com/detail/ghostlight-in-browser/lejccfmoeogmhemakeknjjdhkfkgncdl)
   from the Chrome Web Store.

3. Restart the MCP client if it does not hot-reload tools.

4. Copy this first prompt:

   > Open https://example.com/ in a new Ghostlight tab, summarize the page, and tell me which tab
   > you used. Do not click, type, submit, or change the page.

You should see a dedicated Ghostlight tab group. The agent should name the exact tab and summarize
the page without clicking or writing. If anything looks wrong, run:

```sh
npx -y ghostlight doctor
```

`doctor` checks the client registration, local service, native browser connection, and extension,
then names the next action. The [installation guide](docs/guides/installation.md) covers targeted
clients, source builds, Homebrew, uninstall, and symptom-led recovery. The
[agent install guide](llms-install.md) gives an AI client the shortest safe setup path.

## What working with Ghostlight feels like

- **Use the signed-in browser you chose.** Real cookies and SSO remain in that Chromium profile.
  Ghostlight works in its own managed tabs instead of taking arbitrary control of ordinary tabs.
- **Watch and interrupt.** Page reads and actions have visible feedback. The extension provides
  pause, takeover, and kill controls, while the service remains the policy authority.
- **Complete coherent work.** The agent can read and navigate, fill forms, handle files and
  dialogs, compose bounded steps, wait for page state, record a GIF, and inspect page, console,
  and network evidence.
- **Recover explicitly.** `doctor` checks the installed service, client registration, browser
  connection, and extension, then names the next action instead of leaving a silent failure.
- **Add boundaries when useful.** All-open operation is first-class. Optional governance grants
  `read`, `action`, `write`, and `execute` capabilities by identity and host, with structured audit
  at the service chokepoint.

Ask the agent to call `explain` for the authoritative in-session action and capability directory.
Tool descriptions in the live registry say when to prefer semantic actions, form tools,
composition, or low-level `computer` work. The [solo-developer guide](docs/guides/solo-developer.md)
and [governance guide](docs/guides/governance-configuration.md) cover the two common operating
modes without turning this page into a second tool or policy reference.

## Current compatibility

**Platform state.** Windows and Linux are verified end to end against live browsers. macOS builds and passes the full test suite in CI; its live-browser verification is still owed.

**Extension state.** The Chrome Web Store serves Chrome adapter v0.7.1. Chrome adapter v0.7.1
covers Ghostlight service versions v0.7.1-v0.7.3. Chrome adapter v0.8.0 is pending review and must
become public before service v0.8.0. Install the extension from the public listing.

The service and Chrome adapter version independently. The
[compatibility map](compatibility.json) defines released compatibility, and the
[releases page](https://github.com/sylin-org/ghostlight/releases/latest) identifies the current
service.

## How it works

```text
MCP Client <--stdio--> Relay <--local IPC--> Service <--native messaging--> Relay
                                                                        |
                                                                     Extension <--CDP--> Browser
```

The persistent Rust service owns browser sessions, governance, and audit. MCP clients and Chromium
launch separate roles of the small `ghostlight-relay` executable. The extension owns Chrome
mechanism without policy. See [docs/SPEC.md](docs/SPEC.md) for the deeper governance model.

<details>
<summary><strong>Build from source and test locally</strong></summary>

This is the development path, not an alternate packaged extension install.

```sh
git clone https://github.com/sylin-org/ghostlight
cd ghostlight
cargo build --release -p ghostlight -p ghostlight-relay
```

Open `chrome://extensions`, enable Developer mode, choose `Load unpacked`, and select this
repository's `extension/` directory. Then run `./target/release/ghostlight install`, reload the
extension after JavaScript changes, and verify with `./target/release/ghostlight doctor`.
`docs/DEV-LOOP.md` explains how to swap a Rust service build without fighting live Windows
executable locks.

</details>

## Documentation

| Need | Go here |
| --- | --- |
| Install, verify, recover, or uninstall | [Installation guide](docs/guides/installation.md) |
| Let an AI agent perform setup safely | [Agent install guide](llms-install.md) |
| Decide between browser-control approaches | [Comparison guide](docs/COMPARISON.md) |
| Configure capability, domain, and audit boundaries | [Governance configuration](docs/guides/governance-configuration.md) |
| Review security, privacy, continuity, or procurement evidence | [Trust Center](docs/trust/README.md) |
| Understand the current release and changes | [Public status](docs/public-status.json) and [changelog](CHANGELOG.md) |
| Contribute or propose a change | [CONTRIBUTING.md](CONTRIBUTING.md) and [ADR index](docs/adr/) |
| Read the original deep design | [docs/SPEC.md](docs/SPEC.md); ADRs and the live tree supersede it where they differ |
| Use the vendor-neutral capability vocabulary | [RAWX open specification](open-spec/rawx-capability-model.md) |

## Questions and contributing

Use [GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects,
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions and ideas,
and hello@sylin.org for security, licensing, or other information that cannot be public.
[CONTRIBUTING.md](CONTRIBUTING.md) explains the project's boundaries and test expectations.

## License and continuity

Ghostlight is open-core. The engine outside `crates/core/src/governance/` is Apache-2.0 OR MIT.
The governance module is source-available under the Ghostlight Commercial License and is free for
individuals, teams of up to five, evaluation, development, all-open operation, and qualifying
noncommercial use. [LICENSING.md](LICENSING.md) is the exact plain-language map.

License state never changes runtime behavior. An installed copy does not phone home for telemetry,
activation, or updates and keeps working without the vendor. Read the
[Continuity Promise](docs/trust/continuity.md) and [PRICING.md](PRICING.md) for the durable terms.
