# ADR-0147: Manual browser startup returns a model-directed handoff

- Status: Accepted
- Date: 2026-08-30
- Amends: ADR-0126 Decision 7 and ADR-0103 Decision 1
- Builds on: ADR-0114, ADR-0115, and ADR-0122

## Context

ADR-0126 already defines `browser.startup` with the closed values `on_demand` and `manual`.
The workbench described those values as what happens when no browser is connected, but did not
name the control plainly. Most browser crossings returned a manual-startup refusal through the
language owner, while tab listing had a separate generic refusal after its extension wake window.

The refusal also spoke as if its reader were the person at the computer: `Start Chromium to
continue.` The actual reader is an MCP model. A useful refusal should tell that model what to ask
the person to do and should name every locally eligible browser instead of forcing Ghostlight to
pick one when startup is explicitly manual.

## Decision

### 1. Keep the setting key and make its workbench label literal

The policy setting remains `browser.startup`; its wire values, platform defaults, organization
ceiling, and ordinary-profile launch rules do not change. The workbench labels it `Auto-open
browser on request` and presents the two closed values as `On` and `Off`.

This is still a closed choice rather than an ungoverned boolean because `manual` can be an
organization ceiling and the per-platform default remains part of effective authority.

### 2. Manual mode returns every eligible installed browser

When startup is `manual`, recovery returns every supported browser for which inventory verifies:

- a usable native browser package and an ordinary executable; and
- a current Ghostlight native-host registration.

Multiple eligible browsers are not ambiguous in manual mode because Ghostlight performs no
selection or launch. The person can open any named browser. Automatic startup keeps the existing
unique-candidate rule because Ghostlight would otherwise choose where to direct attention.

Native-host and package inventory cannot prove that one particular browser profile contains the
extension while that profile is closed. The handoff therefore asks for a named browser window
`with the Ghostlight extension installed`; it does not claim profile-local extension presence as
an observed fact.

### 3. The refusal speaks to the model

The language-owned refusal says:

`No browser is connected. Ask the user to open a {browser choices} browser window with the
Ghostlight extension installed, then repeat the call.`

The structured facts carry `reason: browser_startup_manual` and a `browsers` array. For one
browser, the existing singular `browser` fact remains as a compatibility aid. The sentence is the
one recovery instruction, so `next_steps` stays empty rather than repeating it.

After its bounded idle-extension wake window, `browser_tabs` listing crosses the ordinary browser
recovery seam too. It no longer constructs a separate generic refusal.

## Consequences

Turning auto-open off never starts a browser. The model gets an actionable request addressed to
its role, and a machine with Chrome, Edge, Brave, or Chromium can name every currently eligible
choice without inventing a preferred browser.

Package absence, unusable sandboxed packages, missing or foreign native-host registrations,
pinned profiles, and automatic-start ambiguity retain their distinct closed failures. The MCP
connector and extension remain unchanged.

Focused tests must prove the exact singular and plural model sentences, stable facts, no duplicate
next step, no launch in manual mode, all current registered candidates returned, and the tab-list
journey using the same refusal.
