# ADR-0137: Tab and group reuse

Date: 2026-08-25. Status: Accepted (implemented in this revision).

## Context

Running the sample scripts repeatedly produced ever-accumulating tabs, and the browser showed
two identical "Ghostlight - ghostlight call" groups. Two mechanisms, both real:

1. **Duplicate groups.** The extension already reuses one exact-title group per client label,
   but nothing healed pollution from service-worker restarts that landed between group creation
   and titling, or from pre-repair releases. An orphaned duplicate was adopted-around forever,
   never merged or emptied.
2. **Tab accumulation.** Each fresh caller process is a fresh workspace (ADR-0106, correct), its
   first open created a fresh tab, and released tabs from dead workspaces stayed visible under
   the preserve-tabs interlock (also correct). Nothing ever adopted an orphaned same-host tab,
   so one run equaled one new tab, forever.

The owner directed a proper architectural fix over release ceremony: "We publish when we're
done."

## Decision

1. **The canonical-group invariant is enforced, not merely assumed.** After resolving the
   canonical group for a title, the extension merges every other same-title group into it by
   moving their tabs; Chromium deletes an emptied group itself. Merging is best-effort per
   duplicate and retried on the next assignment. Existing pollution self-heals on first contact.
2. **A plain open adopts before it creates.** `browser_navigate` gains `reuse`: `domain` (the
   default) or `never`. When an open must produce a tab and `reuse` is `domain`, the extension
   adopts the nearest unbound same-host tab -- exact URL preferred, then lowest tab id, ordinary
   web pages only -- into the current workspace and navigates it. `new_tab:true` always creates
   fresh and cannot be combined with `reuse:"domain"`; a stale-handle recovery always creates
   fresh under its handle. `reuse:"never"` is the explicit escape hatch.
3. **Reuse is said, not silent.** An adopted open returns `reused: true` and the summary reads
   "Reused the example.com tab." beside the same governed facts; landing governance, close
   compensation, and audit are byte-identical to a created open.

### Rejected alternatives

- Merging or closing tabs from the orchestrator: tab close is the preserve-tabs interlock's
  domain, and group topology is browser-local knowledge. The extension owns both.
- Cross-workspace adoption: tabs are workspace-bound by design (ADR-0084 D4); only unbound
  (released-or-never-bound) tabs are candidates.
- Same-host reuse inside a workspace that already holds the host: the unambiguous controlled
  tab already handles that by navigating in place; this ADR only governs the open path.

## Consequences

- Repeated runs of any script or client converge on one tab per host per group instead of
  accumulating; the sample scripts drop their unconditional `new_tab`.
- The `reuse` field rides the OpenTab command with serde default, so older adapters ignore it
  and keep creating -- the pre-ADR behavior -- which stays correct for them.
- The foundry runners' "whole catalog rehearsed" claim now includes a live reuse demonstration
  whenever a prior run's tab is available to adopt.
