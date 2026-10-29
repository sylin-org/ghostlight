# ADR-0206: A version check the person starts

Date: 2026-10-29. Status: Proposed, for the owner's review before any code. On 2026-10-29 the
owner asked for "a 'check version' button to the About page, and if a newer version is detected we
download/install it", made passive ("the user clicks to check for updates") and pointed at the
release hosts ("it could directly contact, say, npm or github"). The same day the owner clarified
that ADR-0028 Decision 9 forbids automated update checks, not one the person starts (ADR-0028
amendment of 2026-10-29).

Builds on ADR-0028 Decision 9 as amended, ADR-0093 (the major.minor block is the service-adapter
contract), ADR-0197 Decisions 10 and 14 (one home per OS user; the edge is replaced only when the
relay revision changes) and ADR-0200 (the install journey and its carriers).

## Context

Today a person updates Ghostlight by running the command they installed with again, or through a
carrier such as npm, WinGet or the Claude Desktop bundle. Nothing in the window says a newer release
exists, and the About page says "no update ping". Every carrier ends in the same step: the new
service's `deployment install`, which stops the running service, swaps the images in
`~/.ghostlight/bin` and starts the new one. The one-line installers and the npm launcher already
download a release's images from GitHub, from a fixed list of hosts, and check each against the
release's `SHA256SUMS`.

## Decision

1. **Only on request.** The window's About page gets a "Check for updates" button, and the CLI
   gets the same check as `ghostlight update` (check) and `ghostlight update --install`. Nothing
   checks at start, on a schedule, in the background or as a side effect of other work, and no
   badge refreshes itself. A result stays on screen until the page is left.
2. **One question to the release host.** A check is one HTTPS GET of GitHub's latest-release record
   for `sylin-org/ghostlight`. It sends a fixed `User-Agent: Ghostlight` with no version, no
   cookie, and nothing that identifies the person, the machine or the installation. No Ghostlight
   or Sylin server is involved. GitHub, not npm, because the images live in its releases; npm
   carries only the launcher.
3. **Release identity first.** The record must name a stable `vX.Y.Z` tag that is neither a draft
   nor a prerelease. Every later download is pinned to that one release (the lesson in
   docs/MEMORY.md, "Release identity precedes asset transport").
4. **What the check says.** The installed version and the latest. When the latest is newer: its
   version, its release notes as plain text, bounded, and an "Install 2.0.x" button. When it is the
   same or older: "Ghostlight 2.0.2 is the latest." When GitHub cannot be reached: "Couldn't reach
   GitHub." with no retry.
5. **Install is a second click.** Installing restarts the service, which interrupts any agent in
   the middle of a task, so the button says how many agents are connected and that installing
   restarts Ghostlight. Then it:
   - downloads `SHA256SUMS` and this platform's three images (the target the service was built
     for, musl included) from the release, only through the hosts the npm launcher allows
     (`github.com`, `objects.githubusercontent.com`, `release-assets.githubusercontent.com`,
     `github-releases.githubusercontent.com`);
   - checks each image against `SHA256SUMS` and stages them in a temporary folder of the home;
   - runs the staged service's `deployment install`, the step every carrier runs. In-flight calls
     get their 10 seconds, the edge is replaced only when the relay revision changed, the window
     reconnects, and agents reconnect through the wake endpoint.
   A failure before the swap leaves the installation as it was and says what failed. Build
   provenance is not verified in the app, as the one-line installers do not verify it;
   docs/trust/supply-chain.md keeps the manual check.
6. **When nothing is offered.**
   - A development build says it updates through the dev loop and offers no install.
   - A newer release whose major.minor block the connected extension does not cover (ADR-0093,
     read from that release's `compatibility.json`) says which extension it needs and that the
     browser installs it from the Chrome Web Store, and holds the install until the extension
     arrives.
   - An administrator who turned it off (Decision 7).
7. **The administrator's switch.** One configuration key, `updates.check.enabled`, default `true`.
   `false` hides the button and makes `ghostlight update` refuse with the policy reason. As
   `mandatory` it locks the choice, for organizations that control which version people run.
8. **What is kept.** A diagnostics line for each check and each install: the versions and the
   outcome. Nothing enters the audit record, which is about agent work, and nothing is sent
   anywhere.
9. **The words change with the code.** The change that adds the button updates, in the same
   commit: the About text ("no update ping" becomes "no automatic update check"), the FAQ's list
   of what Ghostlight starts on the network (it gains "an update check you start"),
   docs/trust/data-flows.md and docs/legal/PRIVACY.md where they describe network traffic.

## Considered and not taken

- **A scheduled check, even opt-in.** ADR-0028 Decision 9 forbids automated checks.
- **One click that checks and installs.** It would restart Ghostlight under running agents before
  the person saw what changes.
- **Asking npm.** The images are not there, so a check against npm would still need GitHub to
  install.
- **Leaving updates to the carriers.** WinGet lags (its 2.0.1 manifest is still in review), the
  Chrome Web Store updates only the extension, and most people install with the one-line command,
  which has no update of its own.

## Open questions for the build

- Whether the HTTPS transport the managed policy fetch uses honors a system or environment proxy
  on corporate networks, and what the check says when a proxy blocks it.
- Whether the Claude Desktop bundle's own reinstall on start can put an older version back after a
  button install. Its rule installs only over an absent or older set, so it should not, but that
  is checked live.

## Verification

- Tests: a record that is a draft, a prerelease or a non-stable tag is not offered; a download
  outside the allowed hosts is refused; an image whose hash differs from `SHA256SUMS` is refused
  and nothing is swapped; a development build offers nothing; `updates.check.enabled: false`
  hides the button and refuses the CLI; an uncovered extension holds the install.
- Live, on the owner's machine: from the window, check and install the next release over the
  current one, with an agent connected, and confirm it reconnects.
