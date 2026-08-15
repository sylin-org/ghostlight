<p align="center">
  <img src="extension/icons/ghostlight-mascot.png" alt="Ghostlight mascot: a small sky-blue pixel-art ghost holding a glowing lantern" width="100" height="100">
</p>

<h1 align="center">Ghostlight MCP</h1>

<p align="center"><strong>Give your agent a visible place in the browser you already use.</strong></p>

Your agent needs a page you are signed in to. The usual answer is a second, empty browser that
knows none of your sessions, driven by a model that has to learn Chrome internals to get anything
done.

Ghostlight gives it a tab group inside the Chromium you already have open. The work happens in
front of you: watch it, pause it, take the wheel, or end the session. The model says what it
wants, and Ghostlight does the browser part.

> A light left burning, so the halls stay safe.

## Where it stands today

This branch is the 1.0 source candidate. It builds and runs, and the browser tool schemas are
stable -- I am holding those steady. What is still settling is the shape around them: packaging,
upgrade guarantees, and the installed-product path to 1.0.

The published release is 0.8, recorded in [`docs/public-status.json`](docs/public-status.json).
Keep a 0.8 package and a 1.0 source build apart; they are not interchangeable.

## What you get

- **24 browser tools** covering tabs, navigation, reading a page, screenshots, semantic clicks and
  hovers, form input, file upload, scripts, waits, short sequences, and dialogs. One call carries
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

## The workbench

Open the tray icon and you get three places:

- **Monitor** is where you land. The action Ghostlight is taking right now sits at the top in
  full, elapsed time running. When the next one starts, that action freezes and drops into the
  queue below, newest first, so you can watch a session unfold and scroll back through what
  already happened. While nothing is running, the last thing that finished stays on screen.
  Every row says what happened in Ghostlight's own words, never the page's: "Opened example.com.",
  "Read 1,240 words.", "Filled 3 fields and submitted the form.", "Stopped at step 3 of 5." The
  site an action landed on is named; nothing from inside the page is.
- **MCP integrations** connects Ghostlight to the coding clients you have installed: Claude Code,
  Claude Desktop, Codex, Crush, Cursor, OpenCode, Visual Studio Code, Windsurf, and Zed. It merges
  into their configuration with a backup, keeps your comments intact, and leaves any entry it does
  not own untouched.
- **Status** answers whether Ghostlight is healthy, shows which authority sources apply, and ends
  the runtime session when you want that.

Pause and resume sit in the header beside the lamp, the same control the tray offers. Closing the
window returns it to the tray and leaves the service running. If the desktop shell cannot start,
Ghostlight carries on headless so your clients and browser stay connected.

## Build it

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
demand-starts Ghostlight when it is not already running. The normal desktop authority always owns
the tray and begins with its workbench backgrounded: minimized on Windows and hidden on Linux.
The installed Applications entry runs `ghostlight open`, so a desktop without a visible tray still
has a one-click route to the workbench. `--headless` explicitly runs without a desktop.

For an end-user install, the 1.0 release keeps the 0.8 one-command journey:

```sh
npx -y ghostlight@1.0.0 install
```

The installer opens the one required browser-store confirmation and tells you when to reconnect
your MCP client. If anything needs attention later, run `npx -y ghostlight@1.0.0 doctor`.

Linux 1.0 supports x86_64 glibc-based desktops with a native Chrome, Edge, Brave, or Chromium
package. A native `.deb` is provided for Debian and Ubuntu; the verified per-user install is the
default elsewhere. `doctor` identifies unsupported Snap and Flatpak browser packages instead of
reporting them as an unexplained disconnect.

That coordinate is not public until the gates in [`docs/STATUS.md`](docs/STATUS.md) are met. Signed
native packages, portable archives, and a self-contained Claude Desktop MCPB are equivalent release
routes. Every route uses the matching store adapter and the same three native executables.

## Your first proof

Ask your connected client:

> Open https://example.com in a new Ghostlight tab, summarize the page, and tell me which tab you
> used. Do not click, type, submit, or change the page after it opens.

You should see a blue tab group named for your client, the exact tab it used, and a summary that
arrived without a single click. When no Ghostlight group exists yet, it opens a dedicated window
rather than dropping work into whatever you were in the middle of. Next time it reuses that group,
wherever you moved it.

## What it will and will not do

With no policy configured, ordinary remote HTTP(S) browsing is allowed. Loopback addresses,
link-local metadata endpoints, non-HTTP schemes, credential fields, and stale handles stay
protected regardless. Optional local and managed policy layers can only take capability away, and
per-request restrictions narrow things further. Nothing hands access back.

Credential-class fields come to you. Ghostlight does not type secrets.

Closing a tab needs two independent yes votes: the orchestrator's authority, and the browser's own
preserve-tabs setting, which ships on. That keeps the evidence of what happened in front of you.
Closing a tab yourself always works.

The audit record holds identifiers, decisions, and content-minimized measurements: which tool ran,
whether authority allowed it, how long it took, and what it did -- 3 fields, 1,240 words, 1280x720.
The site an action landed on is named, because that answers where your agent went and is already in
your own tab strip. Paths, queries, fragments, page text, field values, screenshots, selectors, and
dialog text never enter it. [`docs/guides/siem-integration.md`](docs/guides/siem-integration.md) is
the exact record shape.

The full catalog is in [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md), and the exact policy schema
is in [`docs/guides/governance-configuration.md`](docs/guides/governance-configuration.md).

## When to choose something else

- You need headless, stealth, bulk, or remote-cloud automation. Reach for Playwright.
- Nobody will be responsible for the browser session while it runs.
- Claude's first-party browser integration already covers the whole job.
- You need Firefox.

## Architecture

```text
MCP client <--stdio--> MCP connector <--typed local IPC-->
                                                     Ghostlight orchestrator
Desktop WebView <--> typed WorkbenchFacade <---------/       \
Browser <--> extension <--native messaging--> browser connector <--typed local IPC-->
```

The orchestrator makes every product decision and owns everything the model reads. The two
connectors carry protocol and relay lifecycle, nothing more. The extension owns Chromium, the
page, and the drawing, and never policy. Adding a feature normally means changing the orchestrator
alone; that is a contract the shores are held to, not a happy accident.

The desktop is a presentation adapter inside the same process, not a second service. It has no GUI
protocol, command runner, filesystem access, or browser primitives.
[`ADR-0102`](docs/adr/0102-integrated-desktop-workbench.md) records why.

## Canonical 1.0 documents

| Concern | Source |
| --- | --- |
| Product promise and journeys | [`docs/1.0/INTENT.md`](docs/1.0/INTENT.md) |
| Complete model-facing language | [`docs/1.0/LANGUAGE.md`](docs/1.0/LANGUAGE.md) |
| Contexts, ports, and invariants | [`docs/1.0/ARCHITECTURE.md`](docs/1.0/ARCHITECTURE.md) |
| Acceptance and release gates | [`docs/1.0/ACCEPTANCE.md`](docs/1.0/ACCEPTANCE.md) |
| Where the candidate stands | [`docs/STATUS.md`](docs/STATUS.md) |
| Every decision, and why | [`docs/adr/`](docs/adr/) |

The 0.8 release, trust, business, research, and distribution records stay where they are. They are
this project's evidence, and they do not get quietly restated as 1.0 claims.

## License and continuity

Everything outside `crates/orchestrator/src/governance/` is Apache-2.0 OR MIT. The governance
module is source-available under the Ghostlight Commercial License, free for individuals, teams of
five or fewer, evaluation, development, all-open operation, and qualifying noncommercial use.
[`LICENSING.md`](LICENSING.md) draws the exact line.

License state never reaches runtime. An installed copy keeps working on its own terms, with no
check-in and no expiry. The [Continuity Promise](docs/trust/continuity.md) and
[`PRICING.md`](PRICING.md) carry the durable version of that.

## Questions and contributions

[GitHub Issues](https://github.com/sylin-org/ghostlight/issues) for reproducible defects,
[GitHub Discussions](https://github.com/sylin-org/ghostlight/discussions) for questions and ideas,
and hello@sylin.org for security, licensing, or anything that should not be public.

I build Ghostlight in partnership with AI coding agents.
[`CONTRIBUTING.md`](CONTRIBUTING.md) explains the current boundaries and the gates every change
passes.
