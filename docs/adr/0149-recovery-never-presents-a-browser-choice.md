# ADR-0149: Recovery never presents a browser choice

- Status: Accepted
- Date: 2026-08-30
- Amends: ADR-0147 Decision 2 and ADR-0126 Decision 7
- Builds on: ADR-0122 and ADR-0146

## Context

ADR-0147 taught the manual-startup refusal to speak to the model and to name every registered
browser, but it kept the older rule that several installed browsers are "ambiguous" whenever
none is connected. That rule answers with:

`More than one installed browser could handle this work, so Ghostlight did not choose one.`

The owner rejected this as bureaucracy rather than delight: a person who cares which browser is
used has one open already, or names one. By the time a model call finds no browser connected,
there is no real choice to protect -- only a refusal that makes the model translate a non-decision
into a question for the person. Continuous-integration runners, which carry two installed
browsers with no registration, turned the surviving rule into the common answer, and the CLI
journey could no longer pin a machine-independent sentence.

The rule also misdescribed the mechanism. Ghostlight does not need one browser to repair or
register: `ghostlight install` registers every detected browser, and stale Ghostlight-owned
registrations can be repaired one by one without choosing. The only act that ever needed a
single browser was the automatic launch.

## Decision

### 1. Plural evidence asks; unique evidence acts

When no browser is connected and more than one installed browser could serve, recovery returns
the model-directed ask naming every installed browser whose native-host registration is current,
in both postures. The person opens whichever they like; Ghostlight chose nothing. With exactly
one connectable browser, the configured posture decides as before: `manual` asks, `on_demand`
launches it. A launch happens only when the evidence is unique and the posture permits it.

### 2. Stale owned registrations are repaired silently first

Among several installed browsers with no current registration, recovery repairs every stale
Ghostlight-owned registration (the existing ownership-checked single-browser repair, applied in
stable order through the same flight) and then returns the ask naming the repaired browsers.
Repair is silent, local, and choice-free; it changes no foreign or malformed entry. Without it
the ask would name browsers that cannot connect, and the alternative remedy would make the
person run by hand what Ghostlight can do invisibly.

### 3. Nothing usable is an install story, not a choice

When several browsers are installed but none has a usable registration and none is repairable,
recovery returns the existing `native_host_unavailable` failure naming the browsers. The remedy
(`ghostlight doctor`, then install) requires no choice between them.

### 4. A simultaneous arrival binds the first adapter

If more than one adapter connects during the bounded post-launch wait, the workspace binds to
the first arrival. Placement then follows the ordinary pinned-session rules, which already make
the first binding stable for the session. The old answer -- a failure that said Ghostlight did
not choose -- misdescribed the state (the browsers were connected, not absent) and put a
round-trip in front of a question nobody has.

### 5. The ambiguity failure leaves the closed vocabularies

`RecoveryFailure::Ambiguous` (`browser_recovery_ambiguous`) and its language-owned reason are
removed. `browser_startup_manual` now means exactly what its sentence says: startup is left to
the person, whatever the configured posture. The connected-browser refusal ("More than one
browser is connected, so there is no single place to open this.") is a different, real question
about placement among live adapters and is unchanged.

## Consequences

Every machine -- including continuous-integration images with two unregistered browsers -- gets
one of three useful answers: an ask naming the browsers it may open, a silent repair followed by
that ask, or a named choice-free remedy. No refusal mentions choosing. The CLI journey pins this
closed language contract instead of the local browser inventory. Exact sentences and the repair
queue remain pinned by unit tests over a controlled inventory.

Acceptance evidence:

1. Unit tests pin the plural ask in both postures, the silent multi-repair, the named remedy
   with no usable registration, and the first-arrival binding.
2. The CLI journey accepts exactly the two honest no-browser refusals and their closed facts on
   any machine.
3. The removed reason key appears nowhere in the language or recovery modules.

## Amendment 2026-08-30: cross-tree adoption is an install action

Accepted the same evening, after Decision 2's silent repair leaked the machine's real
registration into a preflight scratch tree (STATUS, "Native-host registration isolation"): one
un-isolated no-browser call from a fresh build rewrote every browser registration toward that
build. Journeys now isolate the registration surface behind `GHOSTLIGHT_NATIVE_HOST_DIR`, and
this amendment closes the product-side rule.

Silent repair narrows to registrations whose connector already lives in the running
installation's own directory: stale details within one tree, the original self-healing case.
A Ghostlight-owned registration naming a connector in another installation is a new closed
state, `owned_elsewhere`:

- recovery reports it and never adopts it -- fact `native_host_owned_elsewhere`, summary
  `Another Ghostlight installation owns the browser registration.`, and one next step asking
  the user to run `ghostlight install` from the installation that should own the browsers;
- deliberate install still adopts it (InstallOrUpdate), and recovery's ownership-checked
  repair refuses it exactly as it refuses foreign state;
- `doctor` names the owning directory and says when that installation no longer exists, so a
  deleted scratch tree becomes a visible row instead of silent breakage.

The accepted consequence: on a machine whose registration still names an older installation's
directory after an upgrade, recovery now teaches the one install command instead of silently
rewiring the machine. Every delivery channel re-registers on upgrade (npm handoff, package
hooks, dev-loop), so cross-tree self-healing was redundant for real upgrades -- and it was the
mechanism by which a scratch build hijacked the machine.

Additional acceptance evidence:

1. Unit tests pin the same-tree/cross-tree classification, the alive and removed owning-tree
   diagnoses, deliberate adoption by install, refusal by owned repair, and the recovery ladder
   arms (single, mixed plural, all-elsewhere).
2. The exact refusal sentence and next step are pinned in the language module.
