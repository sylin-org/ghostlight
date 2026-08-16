# PINS: normative facts and exact values for the reference-experience epic

This file is the single source of pinned values for every stage prompt. A prompt cites a pin; it
never restates one. When a prompt and this file disagree, this file wins. When this file and the
live tree disagree, STOP and record a deviation in `LEDGER.md`.

Authored against `dev` at `2f24943fa125d952fce9e4f11086aada762e4cad` on 2026-08-16. Every fact in
the "verified tree facts" section was read from the tree at that revision. **Standing order: re-read
the file before you change it.** A pin tells you what was true and what must become true; it does
not excuse you from looking.

Only S1 may append to this file, and only to add values it decided. No stage may edit an existing
pin. If a pin is wrong, STOP and say so in the ledger.

## Gate commands

Run all of these before every commit. They are literal.

    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    npm test --prefix extension
    node --check <each changed extension JavaScript file>

Process journeys resolve executables from `.target-ghostlight-1.0/debug` unless `GHOSTLIGHT_BIN_DIR`
overrides it. If you build into another target directory, pass it explicitly or the journey passes
against stale binaries and proves nothing:

    cargo build --workspace --target-dir .target-ghostlight-1.0
    node tests/process-journey.mjs
    node tests/workbench-surface.mjs

Do not pin a test count as a success criterion. Counts move. Record the counts you observed in the
ledger's gate log instead.

## Verified tree facts (as of the authoring revision)

| Fact | Location |
| --- | --- |
| The extension opens a one-time walkthrough on first install, including a Chrome-sync arrival | `extension/service-worker.js:181-185` |
| That walkthrough is remote only: `https://sylin.org/ghostlight/chromium-extension/post-install/` | `extension/service-worker.js:6` |
| `site/install.html` in this repository is a redirect stub, not the walkthrough | `site/install.html` |
| The connection snapshot already carries `last_error`, raw under the diagnostics preference and otherwise the generic `The local Ghostlight service is unavailable.` | `extension/service-worker.js:103` |
| Chrome's disconnect reason is captured into that field | `extension/service-worker.js:158-160` |
| No extension surface renders `last_error`. `popup.js`, `options.js`, `content.js`, and `lib/` contain zero references | verified by search |
| The popup shows `Waiting for the Ghostlight service...` whenever not connected, with no further distinction | `extension/popup.js:62-64` |
| The popup contains no links at all except its stylesheet | `extension/popup.html` |
| The options page links only to the GitHub repository | `extension/options.html:74` |
| The options Connection card subtitle is `Checking the link to the Ghostlight service...` | `extension/options.html:23` |
| Runtime control is one process-global `AtomicU8`; a hold is a **denial** at the final boundary, not a wait | `crates/orchestrator/src/governance/mod.rs:810-879` |
| The live state vocabulary is `Active`, `Held`, `Attention`, `Ended` | same file, `RuntimeControls::state` |
| The live intent vocabulary is `ToggleHold`, `Hold`, `Resume`, `EndSession`, `StartSession` | same file, `apply_intent` |
| The workbench renders `Held` as the word `paused` | `crates/orchestrator/src/workbench/mod.rs:1416-1421` |
| The popup renders held and attention as `Agent browsing is PAUSED.` | `extension/popup.js:43-47` |
| Snap and Flatpak browser packages are detected with a remedy sentence | `crates/orchestrator/src/install/browser_package.rs:98-131` |
| No WSL detection or messaging exists anywhere in the product | verified by search; `is-wsl` appears only as a transitive lockfile entry |
| No man pages and no shell completions are built or installed | verified by search of `packaging/` and `crates/orchestrator/src/install/` |
| No autostart entry is written on any platform | verified by search |
| The Linux desktop entry is XDG-correct under `data_home/applications` with id `org.sylin.ghostlight` | `crates/orchestrator/src/install/desktop_entry.rs:12,99-107` |
| `doctor` is implemented in the orchestrator binary, not a submodule | `crates/orchestrator/src/main.rs` |
| The npm package declares a `ghostlight` bin, so a global npm install puts it on PATH; `npx -y ghostlight install` does not | `packaging/npm/package.json:7-9` |
| The Debian package provides `/usr/bin/ghostlight` | ADR-0123, recorded in `docs/STATUS.md` |
| Extension tests run as `node --test tests/*.test.js` with `node:test` and `node:assert/strict` | `extension/package.json:5-7`, `extension/tests/shared.test.js:1-12` |

## Exact strings this epic introduces

These are authored values, not suggestions. Use them verbatim. If a string must change, the owner
changes it here and the stage transcribes it.

**Human stop directive** (orchestrator language, S5). The completed or interrupted invocation's
outcome begins with exactly:

    The user asked to interrupt the process. Wait for further instructions.

**Native host absent** (extension presentation, S2). When the browser reports that the native
messaging host is not registered on this computer:

    Ghostlight is not installed on this computer yet.

**Native host absent, secondary line** (extension presentation, S2):

    The extension came with your Chrome profile. Install Ghostlight here to connect it.

**Service present but unreachable** (extension presentation, S2). Unchanged in meaning from today,
kept distinct from the sentence above:

    Waiting for the Ghostlight service...

**Route back to setup** (extension presentation, S2). The control's accessible name:

    Set up Ghostlight

**Background posture** (orchestrator language, S3). Stated once at the end of a successful install:

    Ghostlight starts when your agent or your browser asks for it. Nothing runs in the background until then.

## Detection rules

**Native host absent.** Chrome reports a disconnect reason through `chrome.runtime.lastError`. Treat
the host as absent when that message contains the case-insensitive substring `native messaging host
not found`. Any other reason, and any absent reason, is the ordinary unreachable state. Chrome may
change this text: the match is a narrowing hint, never a load-bearing contract, and an unmatched
message must fall back to the existing unreachable behavior rather than to a wrong claim.

**WSL.** Treat the process as running under WSL when `WSL_DISTRO_NAME` is set, or when
`/proc/sys/kernel/osrelease` contains the case-insensitive substring `microsoft`. Either alone is
sufficient.

**Linux desktop shell.** Read `XDG_CURRENT_DESKTOP`, lowercase it, split on `:`, and map the first
recognized entry onto a closed set. Recognize `gnome`, `kde`, `xfce`, `cinnamon`, and `mate`.
Anything else, including an unset variable, is the unknown case and must have its own honest phrase.

## Appended by S1 (ADR-0126, 2026-08-16)

Every item previously listed as not pinned is decided below. ADR-0126 carries the reasoning; this
section carries the values.

**Pause is a refusal, not a held caller** (ADR-0126 Decision 4). A hold refuses the next effect at
the existing final boundary. No invocation is suspended and no client request is held open. The
refusal is non-terminal and begins with exactly:

    The user paused Ghostlight. Wait for further instructions.

A caller timeout and a caller disconnect are terminal for that invocation and may never leave work
to continue later. There is no deadline reconciliation to implement, because no operation waits.

**State vocabulary** (ADR-0126 Decision 6). The four domain states keep their names and render as:

| State | Word shown to a person |
| --- | --- |
| `Active` | working |
| `Held` | paused |
| `Attention` | needs attention |
| `Ended` | stopped |

`Attention` is never collapsed into the human pause. `StartSession` is presented as starting a new
session, never as resuming. A hold is process-lifetime state: it survives workbench close, browser
reconnect, and harness reconnect, and does not survive an orchestrator restart.

**Browser startup** (ADR-0126 Decision 7). Registered policy setting `browser.startup`, closed
values `on_demand` and `manual`, an operational control rather than a security boundary. Default is
`on_demand` on Windows and `manual` on Linux. A launch uses the person's ordinary profile with no
automation flags. A sandboxed package is diagnosed, never launched.

**PATH entry** (ADR-0126 Decision 8). The per-user route owns `~/.local/bin/ghostlight`, created
only when that path is absent or already Ghostlight-owned, byte-identical on repeat install, removed
on uninstall, never overwriting a foreign file. Shell startup files are never edited. Every
successful install also prints the absolute command path.

**Landing destination** (ADR-0126 Decision 9). At a glance replaces Monitor as the landing
destination. The window keeps five destinations; no sixth is added.

**Page presence.** No preference is pinned, because the in-page affordance is deferred out of this
epic to `docs/design/in-page-affordance-deferred-2026-08.md`.

**Acceptance measures** are ADR-0126 Decision 10. S8 dispositions each one.
