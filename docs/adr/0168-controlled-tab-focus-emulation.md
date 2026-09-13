# ADR-0168: Focus emulation for controlled tabs

Date: 2026-09-13. Status: Accepted by owner direction.

Builds on ADR-0088, ADR-0126, ADR-0137, and ADR-0164.
Amends ADR-0098's membership synchronization for repair of the adapter's lost cache.

## Context

An installed-browser comparison established that Ghostlight and Codex both filled the actual
React Hook Form model. Both then lost the draft when the site's visibility/focus handlers
refreshed the account and reran profile initialization. With Chromium focus emulation enabled,
both tools retained all five fields through a tab switch longer than a minute and a field click.
Research 29 records the source trace, controlled comparison, and Playwright prior art.

The owner approved using the same browser mechanism in Ghostlight, with restoration on pause,
stop, and release. This is an automation compatibility measure. It does not repair the site's
unconditional reset or promise that unsaved values survive after control ends.

## Decision

1. The debugger lifecycle enables `Emulation.setFocusEmulationEnabled` for retained, controlled
   tabs while the connected, compatible service reports active runtime control. The override
   survives individual command leases and applies to newly attached controlled tabs before
   debugger work proceeds. Temporary debugger use on an unowned tab does not enable it.
2. Restored ownership alone does not enable focus. The worker starts with emulation disabled
   and waits for service negotiation. Pause, attention, backend loss, and native disconnect
   disable it. Resume enables it on retained attachments without reattaching tabs that the
   person detached. Stop and the existing local debugger-release control disable it and detach.
   Chrome clears `storage.session` when the extension reloads. On the next explicit tab-scoped
   service request, the worker restores the service-supplied workspace association and debugger
   retention. This repairs an opaque cache; it does not grant service ownership, infer it from
   browser activity, or regroup/move tabs. Inventory, document discovery, and close cleanup do
   not acquire custody. The orchestrator's validated request is the authority.
3. A workspace release removes debugger retention even when the local preserve-tabs interlock
   keeps the physical tab open. An in-flight command can retain its mechanical attachment until
   its lease ends, but it cannot retain the focus override after ownership is released.
4. A later explicit debugger operation on an owned tab may reacquire local debugger custody.
   Human navigation, tab movement, tab activation, and opener relationships do not acquire it.
   ADR-0164's authority and passive-browsing rules are unchanged.
5. Attach, focus, and detach commands are serialized per tab. Control and ownership flags change
   immediately; setup already in flight reconciles the latest state before completing. Failed
   focus commands trigger cleanup rather than leave an uncertain override. If both disabling
   focus and detachment fail, retain the cleanup state and report failure for a later retry.
6. The adapter implements browser mechanics using existing ownership and control messages. No
   connector contract, model-facing tool, policy decision, site-specific script, form-value
   cache, automatic submission, or timer that rewrites a person's edits is added.

## Lifetime and limitations

The scope is the controlled tab while runtime control is active, not one model message. An MCP
workspace can outlive a conversational turn. Returning a tool result or writing a final answer
does not release it. The person can pause, stop, or release debugger sessions through existing
controls; the workspace's eventual release also removes the override.

During that scope, a page reports focused/visible even when its physical tab is inactive. This
changes page focus and visibility behavior without moving the user's tabs or stealing desktop
focus. Normal behavior returns at the stated boundaries. A page may then refresh or reset its
form. Other refresh sources can also reset valid drafts while emulation is enabled. Retention
evidence must distinguish automation under emulation from ordinary browsing after release.

## Verification

Lifecycle tests cover plural tabs, unowned leases, retained focus between calls, pause/resume,
external detach and explicit reacquisition, release during setup, failed commands, and cleanup
retry. Worker tests execute the real control and preserved-tab release paths with the real
lifecycle module. Installed acceptance must fill without submitting, switch away for at least
45 seconds, return and click a field, then compare retained values independently of the receipt.
The pre-implementation comparison is mechanism evidence, not proof of the installed new source.
The first installed attempt exposed the cleared session-cache case: existing service-owned tabs
did not get a retained debugger. The follow-up regression starts with an empty adapter cache and
requires explicit service work to restore custody without moving tabs or affecting another owner.

After the owner reloaded the corrected installed source, Ghostlight filled all five profile fields
without submission. Every DOM/model comparison passed after 81.465 seconds in an inactive tab,
return, field click, and a further 29.159 seconds. No separate automation focus override or page
hook was added. The [installed record](../testing/controlled-tab-focus-installed-2026-09-13.json)
contains source hashes and bounded observations. Cleanup boundaries have lifecycle/worker
regression coverage; the final verified draft was left unsaved under active control.

## Prior art

- [Playwright Chromium initialization](https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/chromium/crPage.ts)
  enables this emulation for its pages.
- [Chrome focused-page documentation](https://developer.chrome.com/docs/devtools/rendering/apply-effects#emulate_a_focused_page)
  describes the visible/focused behavior.
- [Chromium's emulation agent](https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/core/inspector/inspector_emulation_agent.cc)
  explicitly clears focus emulation when the agent is disabled.
- [Chrome extension storage](https://developer.chrome.com/docs/extensions/reference/api/storage/)
  documents clearing session storage on extension reload, update, disable, or browser restart.
- [Research 29](../research/29-form-retention-focus-and-frameworks-2026-09.md) records the
  experiment and the site's reset chain.
