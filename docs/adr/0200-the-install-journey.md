# ADR-0200: The install journey

Date: 2026-10-16. Status: Accepted by owner direction (journey and mockups 2026-10-14/15; this
re-plan 2026-10-16). Details marked agent-decided are reversible on their own.

Builds on ADR-0199 and its amendments of 2026-10-15 (the web apps over loopback; the development
channel; Settings and the tray; shell milestone 2) and 2026-10-16 (a start outlasts a service that
is leaving on idle), with Decision 12 (idle exit); on ADR-0197; and on ADR-0196 Decision 5. It
cites ADR-0199 Decision 23 for the extension's install-time page and does not amend that half of
ADR-0070 Decision 1 again.

Amends:

- ADR-0070 Decision 1, the service-first half: an install shows the Ghostlight window, not a
  remote page.
- ADR-0070 Decisions 2 to 4. Decision 5 stands.
- ADR-0091 Decision 3: the store link moves into the window's setup view.
- ADR-0126 Decision 10, measure 2: a second computer reads "isn't answering", never a guess that
  Ghostlight is not installed.
- ADR-0177 Decision 2: `cli/setup.rs` has no handoff. It asks the running service to show its
  window.
- ADR-0095 Decision 2: `--no-open` keeps the window closed.

## Context

A person installs Ghostlight in three steps that live in three places: the computer, the browser
and the agent. Today they meet a remote page after the install, a store page, an options page and
a terminal, each with its own look and words. The owner approved one journey and its mockups on
2026-10-14 and 2026-10-15.

What the tree gives this journey, as of 2026-10-16:

- The window loads the workbench over loopback HTTP (ADR-0199, amendment of 2026-10-15). Its files
  come from the closed table in `workbench/web/assets.rs`. Requests are `/api` routes in
  `workbench/web/routes.rs`, each answered per context (`workbench/context.rs`, `workbench/parts.rs`).
  Changes reach screens as part-changed events over `/api/events`. `ui/kit/kit.css` is the web
  apps' Settings kit.
- The frozen extension opens its bundled `setup.html` only when the service is absent
  (ADR-0199 Decision 23).
- A service-first install still opens `https://sylin.org/ghostlight/service/post-install/` once
  per home (`install/handoff.rs`). That contradicts "No vendor page opens" in
  `docs/trust/data-flows.md` until the install slice removes it.
- The extension has no level 3 any more (`1f5412d8` deleted it), so the sentence in
  `docs/design/adapter-protocol-4.md` section 7 that a new service meets an old extension through
  an unchanged level-3 path no longer holds. 2.0 is a hard two-sided cutover.

## Decision

1. **Three steps everywhere: This computer, Your browser, Your agent.** One progress indicator,
   the 01/02/03 stepper under the mascot. A disc fills when its step is done. A segment lights
   only between two done discs; there is no lead-in, tail or notch. Setup surfaces show the
   mascot alone, with a glow that fades inside its own bounds. The framed card is reserved for
   presenting the product (owner, 2026-10-14).

2. **Surfaces and owners.** The extension's setup page owns this computer's step (install, update
   and, per JQ6, start), the meeting, and the regular-terminal fallback. The popup owns the
   connection and the human controls. The window's setup view owns the browser step, the agent
   step and the finish. The extension renders only its own connection. Detecting browsers,
   profiles and agents, and every write and launch, belong to the service.

3. **When the extension opens a page** is ADR-0199 Decision 23 (built by f06e). Every other route
   to setup opens the same bundled `setup.html`. No product code opens a sylin.org post-install
   page any more. The setup page keeps one documentation link (Decision 23), with the label and
   URL from JQ4.

4. **The kit.** `extension/kit.css` holds sylin.org's tokens and components and the setup layout.
   Tokens and element rules are scoped to `.gl-page` only, never `:root`, so linking it beside
   another stylesheet changes nothing outside a `.gl-page` element. The service serves the same
   bytes from `workbench/web/assets.rs` at `/extension/kit.css`, with the mascot at
   `/extension/ghostlight-mascot.png` (100x100). There is no copy, generator or `build.rs` step.
   This kit is separate from `ui/kit/kit.css`, which keeps its name and role. After 2.0
   `extension/kit.css` is frozen; service surfaces override only in their own stylesheets.

5. **Words.** The extension's pages own their words, frozen at 2.0. The window and the CLI take
   theirs from `crates/orchestrator/src/language/setup.rs`. Delight is clarity.

6. **The setup page's command** (Q2, owner-approved 2026-10-15): get-script one-liners, no
   Node.js. The URLs are JQ4's answer and are written into the page:
   `irm https://sylin.org/ghostlight/get.ps1 | iex` and
   `curl -fsSL https://sylin.org/ghostlight/get.sh | sh`. sylin.org serves both as redirects to
   the latest GitHub release assets, so the frozen package never changes if hosting does. The
   command and the documentation link freeze with the extension, so both must keep working for
   the life of 2.x. The exact frozen strings are run against a 2.0 test release, made in a fork,
   before submission (the x01 precondition).

7. **Open Ghostlight from the setup page.** Adapter protocol 4 gains one browser-level event,
   `show_requested`, with no fields. The service reveals its window; the window decides what it
   shows.

8. **The browser step: installed browsers only.** A connected browser reads Connected. A profile
   holding the extension reads "Added in your <profile> profile. Open <browser> to finish." and
   offers Open <browser>. A native supported browser without the extension offers Add from the
   Chrome Web Store (Chrome, Brave, Chromium and, per JQ9, Edge). Get Google Chrome appears only
   when no supported browser is native. Add and Open <browser> make Ghostlight start that browser
   (JQ8). The service reads each browser's `Local State` and tests for
   `Extensions/lejccfmoeogmhemakeknjjdhkfkgncdl` only while setup is incomplete. Nothing is
   stored, logged or sent.

9. **The agent step: "Ask your agent."** It shows the prompt "Open sylin.org/ghostlight/demo/desk
   and ring the bell." and then the agents found. An agent reads Connected only when the service
   has seen an MCP session from it. A written entry not yet seen reads "Restart <label> to
   finish." Connect is the Integrations tab's act (`POST /api/integrations/act`). With no agent
   found, the MCP server command is the action.

10. **Completion is the first real agent request:** a top-level MCP call that ran in a browser and
    succeeded, recorded once in `state/setup.json`. A `ghostlight call` never completes setup. The
    2.0 break migration records setup as finished for a 1.x home and retires the 1.x handoff
    marker.

11. **When the window shows setup.** Setup is a workbench part, projected for Control. A Watch
    tab during setup shows the ordinary workbench (JQ5). An interactive install asks the service to
    show the window at `#setup` while setup is incomplete (not for a dry run, `--no-open`, CI or an
    app container), and so does the extension's Open Ghostlight. The window's setup view carries a
    quiet "Skip for now"; the tray brings setup back until it is done (JQ11). The first "no
    browser" refusal to an agent opens no window; the agent's answer carries the store link (JQ10).
    The meeting plays in the window only if the window showed the browser stage for at least 2 s
    (agent-decided), so it never plays twice.

12. **The install says only what the person needs:** the version, where Ghostlight lives, which
    agents have it, and one Next line while setup is incomplete. Mechanism detail stays in
    `doctor` and `--dry-run`.

13. **Motion.** The meeting plays once. Completion has no second celebration. Reduced motion
    removes all motion, and every resting look still reads.

14. **The popup** shows Ready, Paused, Session ended, Not connected or Update needed, with Pause
    agents and its key cap. The existing End session control stays. The button label follows the
    state (JQ6). The toolbar icon stays as it is while Ghostlight does not answer (JQ1): the
    badge and the popup already say so, and a frozen package carries no extra runtime code.

15. **Trust and store: two separate truths.**
    - Page code is part of the Ghostlight application release. It is open source, hash-pinned in
      `tests/golden/protocol4/assets.sha256`, delivered from the application on the same computer
      over native messaging and never pulled from a remote source, installed only through the
      Debugger API into the isolated world of controlled tabs, and changed only when the person
      updates the application. The store dashboard's answer to remotely hosted code stays No
      (owner ruling, 2026-10-16, JQ2): no embedded fallback copy of the page assets ships in the
      package.
    - `browser_execute` script text is written by the person's AI agent, which may run on a
      remote model service. It reaches the extension only through the local application, under the
      person's local policy, and runs only in the attached page.
    - The listing and `docs/legal/PERMISSION_JUSTIFICATIONS.md` say plainly that the extension
      works with the locally installed Ghostlight application, which supplies the page code it
      runs (Edge policy 1.2.3 requires naming the dependency), and that `browser_execute` runs the
      script the person's agent wrote, through the Debugger API. The sentence "All extension logic
      ships in the reviewed package" and its copies are withdrawn for that reason; the 1.x listing
      is not corrected. Host permissions: http and https host permissions are dropped if the live
      check shows nothing needs them (JQ3), so the install prompt stops saying the extension can
      read and change all data on all websites when it does not need that grant.

16. **Release order: a hard cutover.** A 2.0 service refuses a 1.x extension and a 2.0 extension
    refuses a 1.x service, and no order avoids a gap for 1.x users. So the extension is submitted
    early as a staged publish, and public publication, the `v2.0.0` latest release and the scripts
    the frozen command fetches go live together. A staged item must be published within 30 days of
    approval or it returns to draft, and while a submission is pending no 1.x extension hotfix can
    ship without cancelling it. 2.0.0 waits for the service half of this journey (JQ7): the person
    gets the whole journey or none of it. Service-side words for an incompatible extension tell
    the person how to update it now.

### Owner questions and answers

The owner delegated JQ1 to JQ11 to the coordinator with one test: "what would be delightful for
the user?" The owner ruled the store question (JQ2) directly. All were answered 2026-10-16.

| Question | Answer |
|---|---|
| JQ1 grey toolbar icon while Ghostlight does not answer | No. The badge and the popup already show it. |
| JQ2 embedded page-asset fallback (Chrome and Edge) | No. The page code is local, so the remote-code answer stays No (owner). |
| JQ3 host permissions | Drop http and https host permissions if the live check shows nothing needs them. |
| JQ4 frozen URLs and the pre-submission test release | The two get one-liners above; documentation link `https://sylin.org/ghostlight/`; the release attaches `get.ps1` and `get.sh`; the sylin.org redirects are the owner's to publish; the test release is made in a fork. |
| JQ5 a Watch tab during setup | It shows the ordinary workbench. |
| JQ6 installed but not running | The setup page and the popup say "Open Ghostlight on this computer." and where it is (Start menu or Applications). |
| JQ7 scope of 2.0.0 and the cutover | 2.0.0 waits for the service half. |
| JQ8 Add and Open <browser> | Ghostlight starts that browser. |
| JQ9 Edge | Add from the Chrome Web Store until an Edge listing exists. |
| JQ10 window on the first "no browser" refusal | None. The agent's answer carries the link. |
| JQ11 Skip for now | Kept, quiet; the tray brings setup back until done. |

## Consequences

- The extension's words, command, links and kit freeze with 2.0. The x01 freeze guard enforces it
  and closes the gap between ADR-0199 Decision 21 and `tests/guards/connectors.test.mjs`.
- The remote handoff and post-install pages become website content only. The service no longer
  opens a vendor page, so the trust pages' claim of no vendor-bound traffic holds for a
  service-first install once the install slice lands.
- Silent installs (bare `npx`, MCPB, the plugin) open nothing. The agent's no-browser answer
  carries the store link.
- The second-machine strings in `docs/tasks/reference-experience/S2-second-machine.md` are
  superseded. The record stays.
- Edge and Brave install from the Chrome listing until their own listings exist.
- The two separate trust statements of Decision 15 replace one blanket sentence. Store review sees
  what the extension runs and where it comes from.
- Until the journey lands, the 1.x handoff stays as it is. Documents that describe the new
  journey are written last, so no public page claims it early.

## Migration

In order:

1. The extension half: the kit, `show_requested`, the setup page, the popup, the options page and
   (if approved) the toolbar icon.
2. The carriers and the get scripts, so the frozen command can be proven.
3. The store submission, prepared and frozen.
4. The setup record, the profile read, the words, the agent outcomes, the setup part, its actions,
   the window view and the install output.
5. Documents last.

The ledger is [docs/tasks/break-2.0/journey.md](../tasks/break-2.0/journey.md).

## Amendment (2026-10-16): where the short commands point

The redirects behind `https://sylin.org/ghostlight/get.ps1` and `get.sh` point at the installers on
Ghostlight's `main` branch (`https://raw.githubusercontent.com/sylin-org/ghostlight/main/scripts/`),
not at release assets. `main` is the published product, so the short commands install the current
release from the day the redirects go live and follow `main` to 2.0 at publication, with no change on
the website and none in the frozen extension. The release still attaches the scripts for its own
custody. The redirects live in the website repository (`src/_redirects`, kept by its site check);
publishing them is the owner's push there. This replaces the redirect target named in Decision JQ4
above.
