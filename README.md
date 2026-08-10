<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight MCP</h1>

<p align="center"><strong>Give your agent a visible place in the browser you already use.</strong></p>

Ghostlight lets an MCP client perform browser work in a dedicated group inside the user's existing,
authenticated Chromium browser. The work stays visible. The user can pause it, take over, require
attention, or end the session without teaching the model about transports, Chrome internals, or
policy mechanics.

This branch is the planned Ghostlight 1.0 source candidate. The currently published 0.8 release
and store adapter remain recorded in [`docs/public-status.json`](docs/public-status.json); they
must not be mixed with a 1.0 source build.

## What 1.0 includes

- A 24-tool, typo-closed browser language for tabs, navigation, page understanding, screenshots,
  semantic actions, form input, file upload, script execution, waits, short sequences, and browser
  dialogs.
- One orchestrator-owned workspace aggregate, authority snapshot, executor, completion path, and
  payload-free audit record for every invocation.
- A durable MCP connector and browser connector that can remain running while the orchestrator
  restarts. Interrupted effects become truthful unknown outcomes and are never replayed.
- The established `Ghostlight in Browser` extension identity, artwork, tab grouping, take-the-wheel
  controls, preserve-tabs interlock, and visible action language.
- A Tauri 2 desktop workbench built into the orchestrator: at-a-glance home, plural activity,
  history, checkup, configuration, supported-harness installation, global search, tray lifecycle,
  and high-signal native notifications.
- Local operation only. There is no account, telemetry, activation service, update ping, hosted
  control plane, or hidden browser.

## The workbench

Opening the Ghostlight tray icon shows the human-facing control surface:

- **Home** answers what is running, what needs attention, and whether the system is healthy.
- **Activity** lists current MCP sessions, operations, and connected browser instances.
- **History** shows a bounded, newest-first, payload-free record of terminal outcomes.
- **Checkup** explains service, browser, authority, and notification health.
- **Configuration** provides explicit pause, resume, end-session, and start-session controls.
- **Installations** checks, installs, or removes Ghostlight's owned registration for Codex,
  Claude Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, and Crush.

Global search spans destinations and user-visible records. Closing the window returns it to the
tray; it does not stop the orchestrator. If the desktop shell cannot start, Ghostlight continues
headlessly so connected clients and browsers can recover.

## Build the 1.0 source candidate

Prerequisites are Rust 1.82 or newer and, for browser validation, Chromium 116 or newer.

```sh
cargo build --workspace
```

The build produces three sibling executables:

- `ghostlight` -- orchestrator plus the desktop workbench;
- `ghostlight-mcp-connector` -- generic local stdio MCP lifecycle; and
- `ghostlight-browser-connector` -- generic Chromium native-messaging relay.

Start the workbench visibly with:

```sh
target/debug/ghostlight --show
```

Use `target/debug/ghostlight --headless` for the service-only path. A release package will install
the sibling binaries and native-messaging registration together. Source-tree browser registration
and the complete validation loop are documented in [`docs/DEV-LOOP.md`](docs/DEV-LOOP.md).

After the three binaries are side by side, open **Installations** in the workbench and explicitly
install the desired MCP harness registration. Ghostlight performs an ownership-checked merge,
creates a backup, preserves JSONC and TOML comments, and never overwrites a foreign `ghostlight`
entry. Restart or reconnect that harness after the change.

For end-user 1.0 installation, use only the signed package and matching 1.0 store adapter once the
release gates in [`docs/STATUS.md`](docs/STATUS.md) are complete.

## First browser proof

Ask the connected MCP client:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

Ghostlight creates or reuses one blue group named for the client. When no matching group exists,
it creates a dedicated normal browser window instead of inserting work into the user's active
window. Later sessions reuse the same-name group wherever the user placed it.

## Safety and truthful outcomes

No policy means ordinary remote HTTP(S) browser work is allowed. Loopback, link-local metadata,
non-HTTP schemes, credential entry, and unsafe stale handles remain protected. Optional local and
managed policy layers can only remove capabilities, hosts, or tab-close authority. Per-request
restrictions intersect those layers; they never grant access.

Every call returns one terminal envelope: status, observed effect, readiness, replay safety,
canonical facts, and at most two Ghostlight-authored recovery steps. Page content never authors
the summary. Unknown or partial effects never recommend replay.

Model-driven tab close has two independent gates: orchestrator authority and the extension's
default-on preserve-tabs setting. This keeps the visible evidence of browser work available to the
user while leaving manual browser closure untouched.

See [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md) for the complete catalog and
[`docs/guides/governance-configuration.md`](docs/guides/governance-configuration.md) for the exact
policy schema.

## Architecture

```text
MCP client <--stdio--> MCP connector <--typed local IPC-->
                                                     Ghostlight orchestrator
Desktop WebView <--> typed WorkbenchFacade <---------/       \
Browser <--> extension <--native messaging--> browser connector <--typed local IPC-->
```

The orchestrator is the only product mutation point and owns all model-facing language. The MCP
connector owns protocol negotiation; the browser connector owns relay lifecycle; the extension
owns physical Chromium, page-local DOM, and content-free rendering mechanisms. Product evolution
normally changes only the orchestrator.

The desktop is a presentation adapter inside the modular monolith, not another service. It has no
GUI protocol, arbitrary command runner, generic filesystem access, or browser primitive access.
[`ADR-0102`](docs/adr/0102-integrated-desktop-workbench.md) records the decision.

## Canonical 1.0 documents

| Concern | Source |
| --- | --- |
| Product promise and journeys | [`docs/1.0/INTENT.md`](docs/1.0/INTENT.md) |
| Complete model-facing language | [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md) |
| Contexts, ports, and invariants | [`docs/1.0/ARCHITECTURE.md`](docs/1.0/ARCHITECTURE.md) |
| Acceptance and release gates | [`docs/1.0/ACCEPTANCE.md`](docs/1.0/ACCEPTANCE.md) |
| Mutable candidate status | [`docs/STATUS.md`](docs/STATUS.md) |
| Historical decisions | [`docs/adr/`](docs/adr/) |

Historical 0.8 release, trust, business, research, and distribution records remain part of the
project's evidence. They are not silently rewritten as 1.0 claims.

## License and continuity

The engine outside `crates/orchestrator/src/governance/` is Apache-2.0 OR MIT. The governance
module is source-available under the Ghostlight Commercial License and is free for individuals,
teams of up to five, evaluation, development, all-open operation, and qualifying noncommercial
use. [`LICENSING.md`](LICENSING.md) maps the exact boundary.

License state never changes runtime behavior. Ghostlight does not phone home, and an installed
copy does not depend on a Ghostlight-operated service. The
[`Continuity Promise`](docs/trust/continuity.md) and [`PRICING.md`](PRICING.md) carry the durable
terms.

## Questions and contributions

Use [GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects and
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions and ideas.
Use hello@sylin.org for security, licensing, or information that cannot be public.

[`CONTRIBUTING.md`](CONTRIBUTING.md) explains the current boundaries and validation gates.
