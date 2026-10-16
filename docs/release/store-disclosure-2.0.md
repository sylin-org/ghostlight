# Store disclosure for the 2.0 extension (owner draft)

Drafted 2026-10-16 for slice x01-store-submission of the install journey
([ADR-0200](../adr/0200-the-install-journey.md) Decisions 15 and 16). Nothing here has been
submitted, uploaded or published. The owner submits. Recheck every dashboard field name, length
limit and policy wording on the day.

The copy the dashboard takes lives in [STORE_LISTING.md](../legal/STORE_LISTING.md),
[PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md) and
[PRIVACY.md](../legal/PRIVACY.md). This page records the answers that need a decision, the policy
reading behind them, the release order and what is still owed.

## Dashboard answers

**Single purpose.** Paste the block under "Single purpose" in
[STORE_LISTING.md](../legal/STORE_LISTING.md).

**Remotely hosted code: No.** This is the owner's ruling of 2026-10-16. The page code is local. It
is part of the open-source Ghostlight application the person installed, it reaches the extension
over native messaging from that application on the same computer, and it is never pulled from a
remote source. The answer is the same as the 1.x answer, and the 1.x listing is not corrected. The
listing and the permission justifications still say two things plainly:

- The extension works with the Ghostlight application installed on this computer, which supplies
  the page code the extension runs. The extension checks each script against the hash the
  application sends and installs it only through the documented Debugger API, in an isolated world
  of tabs Ghostlight controls. It changes only when the person updates the application.
- When the person's AI agent asks for `browser_execute`, the script text is written by that agent,
  which may run on a remote model service. It reaches the extension only through the local
  application, under the person's local policy, and is evaluated through the Debugger API only in
  the attached page. No code runs in the extension origin.

If a reviewer asks, the page code is published with the service in
`crates/orchestrator/src/browser/adapter/chromium/protocol4/assets/`, and its hash is pinned in
`tests/golden/protocol4/assets.sha256`. No file in the extension package holds page code, and the
package has no content scripts and no `scripting` permission.

**Data use.** Answer from the "Privacy" section of
[STORE_LISTING.md](../legal/STORE_LISTING.md): disclose on-device handling of website content and
user activity as the dashboard's current definitions require, describe client-supplied form values
as input for the requested edit, and select categories from the actual collection and use. The
extension requests no history, cookies, geolocation or sync-storage permission, keeps no browsing
history, and does not read the browser's saved-password store. The privacy policy URL is
`https://sylin.org/ghostlight/privacy/`; the Limited Use statement is in the listing.

**Permissions.** One justification per manifest permission, pasted from
[PERMISSION_JUSTIFICATIONS.md](../legal/PERMISSION_JUSTIFICATIONS.md): `alarms`, `debugger`,
`downloads`, `nativeMessaging`, `offscreen`, `storage`, `tabGroups`, `tabs`, `webNavigation`,
`windows`, the http and https host permissions, and the page-context JavaScript block. Host
permissions are kept in this package. JQ3 said to drop them if a live check showed nothing needs
them, and no result of that check is recorded yet (see "Owed" below).

## Policy reading (coordinator, 2026-10-16)

Read on 2026-10-16.

### Chrome Web Store

Source: https://developer.chrome.com/docs/webstore/program-policies/mv3-requirements (last
updated 2024-04-03 UTC).

- The full functionality of an extension must be easily discernible from its submitted code.
  External resources may supply data but must not contain logic.
- The Debugger API and the User Scripts API are the only listed APIs that permit executing
  remotely sourced logic, when used for their documented purpose; the exemption covers only the
  code those APIs run.
- Contexts cut off from extension APIs (iframes, sandboxed pages) are exempt from the
  remote-loading rule, but the extension's functionality must stay determinable.
- The page says nothing specific about code received from a native application or a local service.

Reading: page code delivered by the local Ghostlight service and run through the Debugger API, and
`browser_execute` text, fall under the Debugger API exemption, provided the listing and the
justification describe it plainly and the packaged extension's own logic is complete and readable.
Under the owner's ruling the answer to remotely hosted code stays No.

### Microsoft Edge Add-ons

Source: https://learn.microsoft.com/en-us/legal/microsoft-edge/extensions/developer-policies
(ms.date 2026-07-24, updated 2026-08-13).

- 1.1.2: all functionality must be clearly disclosed to the user.
- 1.1.7: obfuscated code is not allowed, including external code or resources fetched from the web;
  reviewers may ask for refactoring if code is not reviewable.
- 1.2.1: the extension should not download a remote script and run it in a manner that is not
  consistent with the described functionality.
- 1.2.3: an extension may depend on non-integrated software (another product or service) for its
  primary functionality, if the description says so.
- The policy page has no Debugger API exemption and no outright ban on remote code.

Microsoft's Manifest V3 developer guide states more strictly that remotely hosted code is no
longer allowed and an extension can only run JavaScript included in its package; that is a
platform description, not the store policy text.

Reading: Edge review is the higher risk. Disclose the Ghostlight application dependency (1.2.3),
keep the page assets unobfuscated and published with each service release so a reviewer can read
them (1.1.7), and describe the delivered page code as part of the described functionality (1.2.1).

### The answer for Chrome and Edge (JQ2)

No embedded copy of the page assets ships in the package, for either store. The page code is local,
the remote-code answer is No, and a fallback copy would be a second copy of the same code to keep
in step. The Edge listing is not requested; Edge installs the extension from the Chrome Web Store
until the owner asks for one (JQ9). If Edge or Chrome rejects the package on this point, record the
reviewer's answer here and bring it to the owner. The decision is the owner's.

## Release order (hard cutover)

A 2.0 service refuses a 1.x extension and a 2.0 extension refuses a 1.x service, so no order
avoids a gap for 1.x users (ADR-0200 Decision 16). The order:

1. Build the zip from the approved `dev` commit with `scripts/package-extension.ps1`. The package
   is frozen by `tests/guards/extension-freeze.test.mjs`.
2. Submit it as `STAGED_PUBLISH`. Do not publish.
3. Publish only when all three hold: `v2.0.0` is GitHub's latest release, the command URL
   (`https://sylin.org/ghostlight/get.ps1`) serves a 2.0 script, and the public privacy URL carries
   the new text.
4. A staged item must be published within 30 days of approval or it returns to draft. Then the
   same zip is resubmitted.
5. While a submission is pending, no 1.x extension hotfix can ship without cancelling it. A
   pending submission is not cancelled to ship one without the owner's decision.
6. Any change to a packaged file means withdraw and resubmit, and the freeze golden is regenerated
   in the same commit as the change.

## Assets to recapture

Capture externally, with safe demo content only, so the extension's screenshot suppression does
not hide its cursor, highlights and ribbons:

- The popup, in the Ready state and in the Not connected state.
- The options page.
- The setup page, if the dashboard takes another image.

The popup, options and setup pages use the Ghostlight kit (sylin.org night ground, #5eead4
accent). Any store image of the old sky-blue popup or options page misrepresents this package.

## Owed

Live checks the coordinator runs; none was run by this slice:

- An install of the zip itself as an unpacked folder, on Chrome and Edge.
- After submission, the staged package itself through trusted testers. A packed Chrome clamps
  `chrome.alarms` to 30 seconds, so measure the time to connect after an install.
- The JQ3 check: the unpacked extension with `host_permissions` removed, on an http and an https
  page, covering attach, `webNavigation` events, capture, navigate, read, click, screenshot,
  execute and recording. If nothing needs the grant, remove the block from
  `extension/manifest.json`, remove the host-permission section from the justifications and the
  listing, regenerate the freeze golden, and run the check again on the final zip.

For the owner: the dashboard submission (staged), the remote-code answer above, the assets, the
public privacy URL before publication, and the decision on an Edge Add-ons listing.
