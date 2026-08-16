# ADR-0126: Reference experience contract

- Status: Accepted
- Date: 2026-08-16
- Amends: ADR-0102 Decision 2 (the workbench landing destination), and the runtime-control wording
  in ADR-0122's Status surface description
- Builds on: ADR-0028, ADR-0070, ADR-0082, ADR-0103, ADR-0104, ADR-0112, ADR-0113, ADR-0114,
  ADR-0116, ADR-0118, ADR-0119, ADR-0122, ADR-0123, and Research 25

## Context

Ghostlight's 1.0 experience was assembled seam by seam: install, demand start, workbench, policy,
harness roster. Each is defensible alone. Together they leave three gaps that only appear when a
person, rather than a subsystem, is followed end to end.

First, the product has no owner for what it says about the machine it is running on. Install,
`doctor`, and the workbench each describe the local environment in their own words, and none of them
knows about a desktop without a tray, or about a harness running under WSL while the browser runs on
Windows.

Second, the moment a person moves between machines is unhandled after its first minute. ADR-0070
opens a walkthrough when the extension is installed, including when a browser profile syncs it onto
a new computer. After that tab closes, an absent native host is indistinguishable from a service
that is momentarily down, and no surface offers a route back.

Third, human runtime control is described in the language of waiting but implemented as refusal. A
hold denies at the final boundary through one process-global flag. The reference-experience epic as
first authored assumed the caller would instead be held pending. That assumption was never checked
against the MCP request lifecycle or against ADR-0113's deadline and quarantine behavior.

This ADR settles the vocabulary, defaults, and measures the epic needs so that its later stages
implement decisions rather than make them.

## Decision

### 1. One product across every machine

The product promise is that the same words, controls, and truth appear on every computer a person
uses, shaped to the desktop they are on. Five audiences are in scope and are named so that a change
helping one at another's expense is a recorded decision: the Windows developer with an MCP harness,
the Linux terminal-first developer, the person moving between the two, the privacy-driven user, and
the team or organization.

Platform-dependent behavior is expressed as a table with one row per platform and desktop, never as
a two-branch conditional. macOS is deferred for want of test hardware, not abandoned; when hardware
exists it becomes a row and a body of evidence, not a restructure.

### 2. Progressive reveal is ordered, and the order is not the window

Capability is revealed as the agent's returned sentence, then `ghostlight` in a terminal, then the
workbench. A person who never opens the window is never at a disadvantage: every state the workbench
can render has a `doctor` line in the same words, drawn from the same source rather than duplicated.

### 3. Adaptive familiarity

Where a platform offers a familiar shape, Ghostlight uses it: a tray where the desktop shell has
one, an Applications entry on every Linux install, the notification area on Windows. None of them is
ever the only route to anything, which preserves ADR-0123's rule and GNOME's own guidance.

The recognized Linux desktop set is closed: GNOME, KDE, XFCE, Cinnamon, MATE, and an honest unknown
row. Resolution reads `XDG_CURRENT_DESKTOP`. WSL is a row of the same table, recognized by
`WSL_DISTRO_NAME` or a `microsoft` marker in `/proc/sys/kernel/osrelease`.

A successful install states the background posture once, because a person arriving from a platform
where such tools run at login will otherwise assume a defect: Ghostlight starts when an agent or a
browser asks for it, and nothing runs in the background until then.

### 4. Pause refuses; it does not hold the caller

A pause prevents the next browser effect by refusing it at the existing final boundary. It does not
suspend the invocation, and it does not hold a client's request open.

The rejected alternative was to keep the caller pending for as long as its transport permits. It is
rejected for three reasons, recorded so it is not re-proposed:

- MCP clients carry their own request timeouts. A human-scale pause reliably outlives them, so the
  held call ends as a client-side timeout whose meaning the product cannot state.
- ADR-0113 quarantines an operation whose post-dispatch probe goes unanswered at its deadline. A
  suspended operation and a liveness deadline are two mechanisms competing over one operation's
  fate, and reconciling them requires a second scheduler.
- Refusal is already implemented, already truthful, and already leaves effect truth intact.

What changes is therefore language, not mechanism. A refusal under human hold is non-terminal and
says so, in the orchestrator's outcome language, beginning with exactly:

    The user paused Ghostlight. Wait for further instructions.

A caller timeout and a caller disconnect remain terminal for that invocation. Neither may leave work
to continue later.

### 5. Stop is terminal, and its directive is fixed

Stop ends the session. Every affected invocation completes through the typed outcome path beginning
with exactly:

    The user asked to interrupt the process. Wait for further instructions.

Completed, partial, or uncertain effect facts follow that sentence where they exist. No automatic
retry is ever recommended after a stop, and no stop fabricates a rollback.

### 6. The state vocabulary keeps four members

`Active`, `Held`, `Attention`, and `Ended` remain the domain states. They render to people as
working, paused, needs attention, and stopped.

`Attention` is not collapsed into the human pause. It is reached by the repeated-denial path that
ADR-0122 feeds, and merging the two would make a policy outcome indistinguishable from a person's
choice. `StartSession` remains the transition out of `Ended` and is presented as starting a new
session, never as resuming.

A hold is process-lifetime state. It survives workbench close, browser reconnect, and harness
reconnect, and it does not survive an orchestrator restart. Persisting it would require durable
state, migration, and a staleness rule; and because sessions are caller-owned, the sessions a hold
applied to are gone when the authority is. A restart is visible; a permanently forgotten pause file
is not.

### 7. Browser startup is one registered setting with per-platform defaults

The setting is `browser.startup`, with the closed values `on_demand` and `manual`. It joins the
registered policy settings, following `policy.user.enabled` as the precedent for an operational
control that lives there and can carry an organization ceiling. It is an operational control, not a
security boundary.

Defaults differ by platform, deliberately. Windows defaults to `on_demand`. Linux defaults to
`manual` until a launch can be proved deterministic from a session environment resolved through the
ADR-0082 seam. Diagnosis without a launch is an acceptable terminal state on Linux.

A launch, where it happens, uses the person's ordinary browser profile. Never a fresh profile, never
a temporary profile, never automation flags. A sandboxed browser package is diagnosed with the
existing remedy, never launched.

### 8. The per-user install owns one PATH entry

The per-user route creates `~/.local/bin/ghostlight` when that path is absent or already owned by
Ghostlight. It is byte-identical on repeat install, removed on uninstall, and never overwrites a
foreign file. Shell startup files are never edited, per Research 25. Every successful install also
prints the exact absolute command path, so the terminal route works whether or not the entry was
created.

### 9. At a glance replaces the Monitor landing, and adds no destination

The workbench keeps five destinations. The landing destination becomes At a glance: it leads with
whether Ghostlight is ready, connected, working, paused, recovering, or in need of attention, and
the action queue continues beneath that answer. Integrations, Status, Policy, and About are
unchanged.

A sixth destination is rejected because it would create two status summaries, which is the exact
duplication this epic exists to remove.

### 10. Acceptance measures

- First use: on a clean machine, a person reaches one successful browser operation without opening
  the workbench and without reading source.
- Second machine: with the extension present and no native host, both extension surfaces state that
  Ghostlight is not installed here and offer a route back, online or offline.
- Recovery: every readiness failure ends either repaired or named as one of the closed failure
  reasons, with a next action.
- Control: pause prevents the next effect; stop is terminal and truthful; neither fabricates a
  rollback.
- Parity: every workbench state has a `doctor` line in the same words, checked by a guard test
  rather than by review.
- Accessibility: keyboard-only operation, accessible names, large text, high contrast, and reduced
  motion, none depending on color, animation, pointer, tray, or notifications.
- Evidence: the Ubuntu GNOME Wayland lifecycle ADR-0123 made release-blocking, plus a Windows lane,
  plus the migration cases.

### 11. No network behavior is added

ADR-0028 Decision 9 stands unchanged. This epic adds no telemetry, no update ping, no activation
call, and no new outbound request. Adoption signal comes from the Chrome Web Store, npm, the MCP
registry, and GitHub, which already count installs and downloads without the product reporting
anything.

## Consequences

`docs/1.0/INTENT.md` is corrected: the workbench has five destinations, not three, and the landing
destination is At a glance.

The runtime-control work becomes smaller than the epic first assumed. No second queue, no scheduler,
no persistence, and no deadline reconciliation are needed; the change is one directive sentence, one
non-terminal refusal path, scope definitions for plural work, and surfaces that delegate.

The readiness work becomes platform-asymmetric and may land on Linux as diagnosis only. That is a
completion, not a failure, and the setting makes the difference visible to the person rather than
hidden in the code.

Deferred by this ADR, and recorded so it is not treated as cancelled: presence on controlled pages,
in `docs/design/in-page-affordance-deferred-2026-08.md`.
