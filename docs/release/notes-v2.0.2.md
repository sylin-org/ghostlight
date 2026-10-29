# Ghostlight 2.0.2

Ghostlight 2.0.2 makes the browser tools easier for agents to call, and stops three answers that
told an agent something untrue. Update with the same command you installed with:
`irm https://sylin.org/ghostlight/get.ps1 | iex` on Windows,
`curl -fsSL https://sylin.org/ghostlight/get.sh | sh` on Linux, or `npx -y ghostlight install`.
Ghostlight in Browser stays at 2.0.1; this release needs no extension update.

### Changed

- **Calls are read the way agents write them.** A call with one sensible reading runs with it,
  and the result says how it was read (ADR-0205). A scroll amount can be `600`, `"600px"`,
  `"50%"`, `top`, `bottom` or `end`, and a negative amount goes the other way. A duration can be
  `2000` or `"2s"`. A key can be named as people name it (`esc`, `pgdn`), as a chord (`ctrl+a`),
  or as a sequence (`"PageDown PageDown"`), and `repeat` works with a single key. Words match in
  any case, field names in camel or kebab case, and an address without a scheme gets `https://`.
- **Left-out values come from the call or the page.** `browser_tabs` with no action lists,
  `browser_wait` with nothing waits for the page to load and otherwise reads its condition from
  what it is given, and a scroll with no direction goes forward on whatever the page scrolls.
- **What does not apply is ignored and named.** A field the chosen action does not use is no
  longer refused. Results name what was left out in `ignored` and what was used instead in
  `adjusted`. An unknown field on a tool that acts is still refused, now with the closest field
  and every field the tool takes.
- **The catalog admits these forms too**, so a client that checks calls against the schema
  before sending them lets them through: `action` and `condition` may be left out where they
  have a default, and keys are free text.

Policy, Pause and Stop, credential authorization, typed text and form values are as strict as
before.

### Fixed

- **A page that changes is not a missing tab.** A screenshot or read taken while the page
  committed a new document, such as a sign-in redirect, said "That tab handle names no open tab"
  about a tab that was open. It now runs once more on the new page, and if the page keeps
  changing it says so.
- **A slow page still navigates.** A navigation the browser reported as committed, on a page that
  took longer than 8 seconds to finish loading, said it ran out of time before the browser
  confirmed anything. It now succeeds with `readiness: "loading"` at the address it reached.
- **A scroll says what it moved.** A scroll with no target moved only the window, so on
  applications that scroll an inner pane, such as SharePoint and ServiceNow, it said "Scrolled
  down" while nothing moved. It now moves what actually scrolls, reports the pixels moved and
  whether it reached the end, and says so when nothing moved.
