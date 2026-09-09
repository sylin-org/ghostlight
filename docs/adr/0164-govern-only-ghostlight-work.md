# ADR-0164: Govern only Ghostlight work

Date: 2026-09-09. Status: Accepted by owner direction.

## Context

A manual visit to `chrome://version` produced a red `browser_landing` action card. The
service applied landing policy to every document-commit event, including human navigation,
held the tab against future automation, and recorded an action that nobody had requested.
The extension also adopted, grouped, and attached a debugger to any child of a controlled
tab, and regrouped controlled tabs when a person moved them between windows.

The owner requires Ghostlight to manage only its own actions. Merely improving the card's
wording or changing it to an informational event would still violate that requirement.

## Decision

1. Passive document commits are browser state, never work. They produce no governance
   decision, action receipt, history card, notification, or tab hold. They only invalidate
   Ghostlight's cached targets and update the current destination for later agent requests.
2. The executor remains responsible for authority at every requested operation and returned
   landing. Human freedom does not grant an agent access to a prohibited page. A correlation
   marks an observation interval; it does not prove who initiated a navigation.
3. An opener relationship never grants ownership. Remove automatic child-tab adoption,
   grouping, debugger attachment, and presentation. Older adapters' child-open events are
   ignored by the service. Explicit executor work remains the acquisition route. Popups are
   not automatically controlled, including popups that happen during agent work.
4. Moving a tab between windows never triggers automatic regrouping. Existing opaque
   ownership survives the move; the browser placement belongs to the person.
5. Remove the passive landing outcome and its active-authority registry. Normal invocations
   retain their immutable authority snapshot. Historical `browser_landing` lines stay on disk
   but are omitted from workbench history and its action counts on restoration.

This supersedes the opener-adoption and regrouping behavior described by the earlier browser
core and topology contracts, and the passive landing account covered by ADR-0103. It does not
change the generic browser bridge, manual runtime controls, explicit agent navigation/reuse,
or compensation for a refused agent-requested open.

## Verification

The service regression covers protected, denied, and permitted manual landings, with and
without an in-flight correlation. It checks no browser effects, no audit additions, no
operations/history, no tab holds, no unsolicited child ownership, and refusal only when an
agent subsequently requests access. The history regression proves old passive records are
omitted without rewriting the file. Extension guards prevent implicit creation/move handlers
from reclaiming those user actions. Normal executor, adapter, and workbench suites still apply.

Live acceptance requires deploying the orchestrator and reloading the changed extension.
An old extension may still group/debug child tabs even when the service ignores its event.
