# ADR-0186: Quiet Browser Coexistence

Date: 2026-09-30. Status: Accepted for an authorized local engineering trial.

Amends ADR-0088's ordinary form-fill mechanism, ADR-0098's browser-local topology, and
ADR-0137's unconditional same-host adoption and duplicate-group repair. Builds on ADR-0102,
ADR-0126, ADR-0164, ADR-0168, ADR-0181, and ADR-0185.
The source trial is unpublished. It changes no release version or public availability claim.

## Context

The operator and several agents can share one authenticated browser. Avoiding one window-focus
call does not protect the person: opening an active tab changes their visible page, same-host
reuse can navigate an unowned human tab, and group repair can move tabs between windows.
An operator preference must apply to every invocation route and cannot be relaxed by a model.

The existing runtime domain already expresses Pause, Stop, and explicit Attention. ADR-0185
removed automatic denial and credential holds. Quiet coexistence permits admitted work to
continue, so it is a browser attention preference rather than another runtime hold.

## Decision

1. Register one operator-authored schema-3 setting, `browser.attention`, with the closed values
   `background` and `foreground`. Background is the default. Foreground opts into the earlier
   direct activation and unbound same-host reuse behavior. The ordinary persisted user policy
   owns the setting; it has no model-facing input or extension-local competing switch.
2. A mandatory organization background value cannot be weakened by a user value. Recommended
   organization values are defaults. A user background value can tighten organization
   foreground. The effective view reports the value, its deciding layer, and the mandatory
   organization ceiling. Invalid cold authority fails closed with background attention.
   Invalid replacement retains the existing last-valid policy behavior.
   With no existing user layer, the first preference change in an untouched empty Workbench
   draft adds one visible all-sites RAWX grant. This preserves the prior capability baseline
   under organization intersection without grant boilerplate. Explicit rule editing or removal
   prevents reseeding; an intentionally empty deny-all draft stays explicit.
3. The shared executor selects attention for every physical dispatch, including flow children
   and compensation. Session-release cleanup uses the same preference and physical guards.
   CLI and MCP use the same route. If queued foreground work encounters a
   newly tightened background preference at the final writer boundary, refuse before sending;
   do not silently reinterpret or replay it. A foreground request admitted before the change
   retains its actual effect facts.
4. Background requires the adapter's negotiated `browser_attention` revision 1 before physical
   dispatch. An older adapter returns a visible capability refusal before effect. Legacy wire
   requests lacking the attention field retain foreground semantics; they cannot establish
   background enforcement. The browser connector remains an opaque relay.
5. A background request never foregrounds a window or selects a different tab in the person's
   focused or shared window. Opening prefers an unfocused work window containing only owned
   tabs, ignoring matching groups or windows containing human tabs. If none is eligible, it
   creates a dedicated normal window with `focused:false`.
   Separate same-title groups may remain across windows; the old canonical merge invariant
   belongs to foreground, not background repair at the expense of human placement.
   Background opening does not adopt or navigate an unowned human tab even when `reuse:domain`
   is requested. It does not merge duplicate groups, move their tabs, or use group placement
   to move an existing tab. Ownership repair remains an opaque association, without regrouping.
   Resize refuses a focused window or one shared with unowned tabs. Active close refuses a
   focused window, an unowned neighbor, or the last tab's window. A moved target refuses before
   either effect. These return typed attention protection. Release cleanup drops custody first.
   Missing-browser recovery asks the person to launch even when startup is configured on-demand:
   an ordinary browser launch cannot guarantee preserving another application's foreground.
6. Model `browser_tabs` focus is refused under background attention even though it requires no
   RAWX capability. The result reports no effect and `repeat_safe:false`, and explains the
   operator preference. Every completion's `facts.browser_attention` carries the effective
   value, deciding layer, and mandatory organization ceiling. Other admitted work proceeds
   subject to ordinary policy, ownership, runtime controls, and effect truth.
7. At a glance offers one bounded Workbench `Show tab` action for a current owned tab. The
   facade resolves the exact live workspace, browser, and tab before issuing an explicit human
   foreground reveal. Missing or released ownership refuses without creating a replacement.
   The desktop adapter supplies intent, never a physical tab id or browser primitive. This
   control is new on the existing surface; previous Workbench reveal opened the Workbench only.
8. Show tab grants no browser authority, resumes no session, retries no action, and implies no
   rollback. Showing a paused or stopped session's retained tab does not permit further agent
   work. Page feedback stays passive and cannot intercept the person's input.
9. Preserve the runtime state and intent vocabulary for compatibility. Pause refuses future
   dispatch; Stop is terminal; Resume permits new requests and never replays earlier ones.
   Explicit Attention remains a human compatibility control, not an automatic response to
   denials. Denied committed landings retain their separate tab-hold semantics.
   Attention protects shell placement and input surfaces, not all changes to controlled content:
   ordinary admitted navigation in a controlled tab remains available. Use Pause for takeover.
10. Controlled-tab CDP focus emulation is unchanged. It changes page-reported focus under active
    controlled custody, not desktop attention. Its pause, attention, stop, disconnect, release,
    and failed-cleanup rules remain governed by ADR-0168.

## Evidence-driven input amendment (2026-09-30)

The integrated browser journey also found that creating a group with only `tabIds` lets Chromium
use the currently focused window, moving a newly created work tab into the human window. A new
group must name the actual tab window in `createProperties.windowId`. The adapter checks current
placement and eligibility before grouping, observes movement/focus during the effect, and
refreshes the tab afterward for the receipt. A pre-group change can skip decorative grouping;
an uncertain post-group placement never claims the original window or retries the effect.
This is an implicit Chrome API default that a source-only topology mock did not expose.

Chromium acknowledged native key and mouse packets on never-visible inactive tabs without
applying the requested effects. An active controlled tab in an unfocused work window received
trusted input while the human window retained focus. A transport acknowledgement alone cannot
support an input-success claim.

Native keyboard and pointer work is coordinated per physical window. The adapter may select an
owned inactive tab only inside an unfocused window whose tabs are all owned. It rechecks target,
placement, ownership, selection, and window focus before actual input packets; observed human
focus, tab addition, or movement fences that mechanical scope. It never reselects a tab to recover
after an input effect may have landed. Different work windows remain independent; no global hold
or new scheduler is introduced.

A keyboard/pointer surface in a focused or shared window returns typed `native_input` protection
before input, even if
the requested tab is already active. After any possibly applied input, a later fence returns
uncertainty or partial effects instead of claiming no effect or replaying packets. Show tab is
for human review, not permission to continue background native input while that window is focused.
Return focus elsewhere, or open a fresh background tab in an eligible work window, then reobserve
before further work. Model FocusTab remains refused under background attention.

Background ordinary textual form fill prepares and verifies the exact control's DOM selection,
performs one native `Input.insertText` replacement including empty or multiline text, and commits
by blurring the validated control. It retains the existing all-field preflight, budget, and
complete-batch retention verification. It does not fall back to prototype value setters,
fabricate input/change events, or replay key packets after an ambiguous acknowledgement.
Foreground form fill and the explicit keyboard tool's semantics remain unchanged. This amends
ADR-0088's form editing mechanism, not its promise for explicitly requested keyboard actions.
A page that requires every keyboard packet can still reject whole-value native insertion;
the receipt must follow observed retention rather than claim every key was delivered.
Whole-value text editing uses the same window coordinator without selecting the tab. It can
edit an inactive owned target without taking the visible surface from a shared or focused window;
it refuses when that target itself is active in the focused window. Packet checks and event
fences still apply. Key-up, mouse-release, and drag-cancel cleanup may finish after takeover to
release already held input; that cleanup does not admit new input or replay prior effects.

An attention-incompatible adapter is retired independently. The service sends its legacy Ended
control to disable retained debugger custody, closes that connection, and excludes repeated
legacy reconnects from Active state until a capable adapter connects. It sends no physical tab
close and does not Stop other adapters or the global runtime. This is bounded delivery, not a
correlated cleanup receipt; do not claim confirmed detachment or forgotten adapter topology.

A modern adapter receiving a different service epoch disables focus emulation, settles its
volatile diagnostic/recording state, detaches retained debugger custody, and clears cached tab
ownership before it accepts Active for the new epoch. Physical tabs and group presentation hints
remain. Same-epoch reconnect retains ordinary ownership continuity. Legacy Ended delivery does
not establish that this modern epoch cleanup occurred.

## Limits

This is enforcement of Ghostlight's direct browser mechanisms, not OS or page-code containment.
Explicit scripts and page actions can cause browser-originated popups, dialogs, or focus behavior.
Background work may create a visible unfocused window. It does not promise no windows, silence
from every website, exactly-once effects, or preservation of unsaved data after control release.
Chrome's separate query and effect APIs do not offer an atomic OS focus lock. Window-scoped
serialization and packet-time checks contain observed changes; they are not host containment.

Background opens can leave additional preserved tabs across released sessions. That is preferable
to treating any human same-host tab as expendable. The existing preserve-tabs interlock remains
independent; foreground remains an explicit operator choice for the old reuse behavior.

## Rejected alternatives

- Another attention service, workflow engine, scheduler, or control surface: the shared executor,
  operator policy, and Workbench already own the necessary boundaries.
- A model `quiet` flag: a caller could remove it, so it cannot protect the operator's preference.
- Suppressing only `windows.update`: active-tab navigation, adoption, and grouping still disrupt
  the person's work.
- Mapping quiet to Pause or removing the compatibility Attention state: quiet admits background
  work, while those runtime states refuse future work.
- A successful no-op focus receipt: it would say the requested tab was shown when it was not.

## Verification

Required proofs cover typed setting validation and persistence; mandatory and recommended
composition; both invocation edges and flows; old-adapter refusal; background opening and
navigation; owned versus unowned tabs; group movement; blocked model focus; explicit human
reveal; Pause/Stop/Resume; and acknowledged versus uncertain effects.
Native-input acceptance compares trusted effects on the active unfocused work surface with
refusal on focused/shared keyboard or pointer surfaces, including already-active targets.
Check inactive whole-value edits separately from active-focused edit refusal. Background fill
checks replacement, clear,
multiline text, blur-committed values, and later framework retention independently of receipts.

Compare actual tab/window focus and retained human input with receipts in visible Chromium using
local synthetic fixtures. The trial authorizes disposable isolated local installations and
browser profiles, including real native-messaging verification. It does not authorize production,
public, or remote rollout. Component browser fixtures, isolated installed native-messaging/Tauri
journeys, the person's selected everyday installation, and Linux runtime evidence remain separate
verification scopes. Check results belong in the trial status record; this ADR specifies
acceptance and does not assert pending checks passed.
