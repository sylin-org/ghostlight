# LEDGER: the reference Ghostlight experience

Durable progress for the epic defined by [BOOTSTRAP.md](BOOTSTRAP.md), with exact values in
[PINS.md](PINS.md). Update this file before starting a stage, after every material finding, after
every commit, and when closing or blocking a stage.

## RESUME HERE

- State: S1 through S6 COMPLETE.
- Current stage: S7, readiness recovery. Orchestrator.
- Next action: the Linux lane owns the rest. Its entry point is
  [START-HERE-LINUX.md](START-HERE-LINUX.md): five verification tasks for work already landed, then
  S7a, S7b, S7c, then S8. S7a is a governance-schema change and should start a fresh session.
- Owner: `linux-codex` from 2026-08-16. The Windows host is not implementing further stages. S4 does not land coherently in one commit; its ordered substeps are in
  the S4 section further down. Each substep is one commit and leaves a green tree.
- Blocking condition: none. S4b's central behavior cannot be verified on a Windows host; see the
  substep note.
- Source baseline: `dev` at `2f24943fa125d952fce9e4f11086aada762e4cad`.
- Last green evidence: the baseline's recorded Windows current-source pass
  (`docs/testing/windows-current-source-pass-2026-08-15.md`) and CI run `31920645118`. No
  implementation gate is claimed for the authoring commits themselves.

## Provenance

This epic was first authored on 2026-08-15 in `83ca2ba9` as seven outcome-outline stages. It was
reworked on 2026-08-16 after a review against the live tree and the prior Linux research. The
reasons are recorded here so they are not re-argued:

1. The original stages cited no ADRs. This tree has 125 of them and a standing rule to read the
   owning ADR before touching a subsystem.
2. The original S7 required a recruited user cohort. That process was already rejected as a release
   gate in `docs/testing/greenfield-first-success.md` and in `docs/business/FOUNDER-TODO.md`. The
   evaluation now uses the Ubuntu GNOME Wayland lifecycle that ADR-0123 already made
   release-blocking, a Windows lane, and consented public feedback.
3. The original S5 required an authentic full-vector mascot. No such asset exists in the tree; every
   mascot file is raster and the only vector is `extension/icons/ghost-mark.svg`. The in-page
   affordance is deferred to `docs/design/in-page-affordance-deferred-2026-08.md`.
4. The original S2 made automatic browser launch the default recovery. On Linux that inherits the
   session-environment problem ADR-0082 exists for, plus keyring prompts, profile locks, and
   sandboxed browser packages. Recovery is now platform-honest and late in the sequence.
5. The review found four verified gaps that no stage covered, all in the path of a person moving
   between machines. They are now S2 and S3.
6. `docs/MEMORY.md` had recorded the human-stop directive as a durable fact. It does not exist in
   the tree. That entry was corrected to name it as this epic's intent.

The 2026-08-15 stage files remain in Git history. Do not take a path or excerpt from them as current.

## Stage table

| Stage | Status | Closing commit | Checkpoint | Notes |
| --- | --- | --- | --- | --- |
| S1 experience contract | COMPLETE | `9b537a14` | Decisions and pins only | ADR-0126; no behavior change |
| S2 the second machine | COMPLETE | (this commit) | A new computer explains itself | Extension only |
| S3 adaptive familiarity | COMPLETE | (this commit) | Local desktop language | Includes WSL |
| S4 terminal citizenship | COMPLETE | (this commit) | PATH, man, completions, `--json` | Six substeps, S4a-S4f |
| S5 human runtime control | COMPLETE | (this commit) | Pause, resume, stop | Language and state; ADR-0126 D4-D6 |
| S6 At a glance | COMPLETE | (this commit) | One calm window | ADR-0126 D9 |
| S7 readiness recovery | READY | -- | Safe repair, exact refusal | Platform-asymmetric |
| S8 evaluation | NOT STARTED | -- | Evidence on real desktops | Depends on S1-S7 |

Allowed values: `NOT STARTED`, `READY`, `IN PROGRESS`, `BLOCKED`, `COMPLETE`. At most one stage is
`IN PROGRESS`.

## Completion evidence matrix

A prose assertion is not evidence. Link a commit, test, fixture, ADR, or dated record.

| Area | Stage | Required evidence | Status | Evidence |
| --- | --- | --- | --- | --- |
| Vocabulary and measures | S1 | Accepted ADR; reconciled 1.0 contracts; appended pins | COMPLETE | `9b537a14`: ADR-0126; `docs/1.0/INTENT.md` corrected; `PINS.md` S1 section |
| Host-absent state | S2 | Distinguished state, both surfaces, offline route, tests | COMPLETE | `shared.linkState`; `extension/setup.html`; `extension/tests/onboarding.test.js`, 9 tests |
| Platform and desktop table | S3 | One owner, closed set, WSL case, consumer parity, tests | COMPLETE | `crates/orchestrator/src/language/environment.rs`, 8 tests; install and `doctor` both consume it |
| Terminal citizenship | S4 | PATH ownership, man pages, completions, `--json`, doctor parity guard | COMPLETE | `command_path.rs`, `user_assets.rs`, `packaging/linux/{man,completions}`, four guard tests in `main.rs` |
| Runtime control | S5 | One state machine, effect truth, deadline interaction, plural scopes | COMPLETE | `HUMAN_PAUSE_DIRECTIVE`/`HUMAN_STOP_DIRECTIVE` in `outcome.rs`; three governance tests; four language oracles; popup separation |
| At a glance | S6 | All states, controls, keyboard, accessibility, redundant surface removed | COMPLETE | `language/readiness.rs` with 6 tests; `ReadinessSummary` on the snapshot; three surface-journey assertions |
| Readiness recovery | S7 | Per-platform posture, single flight, bounded waits, exact failures | NOT STARTED | -- |
| Evaluation | S8 | Ubuntu GNOME lifecycle, Windows lane, migration cases, dispositions | NOT STARTED | -- |

## Decision register

Open questions, owned by a stage. Not conclusions to assume early.

| Decision | Stage | State | Resolution |
| --- | --- | --- | --- |
| Does a hold keep the caller pending or keep refusing, and what happens at caller timeout | S1/S5 | CLOSED | Refuses, non-terminally, with a pinned directive. ADR-0126 D4 |
| How `Attention` and `StartSession` map onto pause, resume, and stop | S1/S5 | CLOSED | Kept distinct. ADR-0126 D6 |
| How a held operation interacts with the ADR-0113 deadline and quarantine | S1/S5 | CLOSED | Not applicable; nothing waits. ADR-0126 D4 |
| Whether a held state survives workbench close, reconnect, and restart | S1/S5 | CLOSED | Survives close and reconnect, not restart. ADR-0126 D6 |
| Owner and default of the browser-startup preference, and whether it joins registered policy settings | S1/S7 | CLOSED | `browser.startup`, registered setting, `on_demand` on Windows and `manual` on Linux. ADR-0126 D7 |
| Whether the per-user route owns `~/.local/bin/ghostlight` or only reports the path | S1/S4 | CLOSED | Owns it, ownership-checked, plus prints the absolute path. ADR-0126 D8 |
| Whether At a glance replaces Monitor or becomes a new destination | S1/S6 | CLOSED | Replaces the landing; no sixth destination. ADR-0126 D9 |
| Acceptance thresholds for first use, recovery, and comprehension | S1/S8 | CLOSED | ADR-0126 D10 |
| Whether 1.0 publishes before this epic lands | owner | PROVISIONAL: yes | -- |
| Whether the in-page affordance returns | owner | PROVISIONAL: deferred | -- |

## What this epic makes redundant

Recorded by S1 so later stages remove duplication rather than adding to it. Each entry names the
stage that owns its removal.

| Redundant thing | Where | Owning stage |
| --- | --- | --- |
| The undifferentiated `Waiting for the Ghostlight service...` state, once host-absence is a distinct classification | `extension/popup.js`, `extension/options.html` | S2 |
| Any per-call-site phrasing of where Ghostlight lives or how it starts | install and `doctor` output | S3 |
| Any second copy of a state's wording that `doctor` and the workbench both need | `doctor` and `crates/orchestrator/ui/` | S4, S6 |
| The Monitor landing as a separate concept from the readiness answer | `crates/orchestrator/ui/index.html`, `app.js` | S6 |
| Any surface-local inference about whether Ghostlight is paused | all surfaces | S5 |

## S4 substeps

S4 spans the CLI, the installer, and packaging, and its parts have different verification stories.
Ordered so every prefix is coherent and green.

| Substep | Scope | Verifiable on the Windows authoring host |
| --- | --- | --- |
| S4a | `--json` uniformity across every state-reporting subcommand, through the existing command seam | Yes. **DONE** |
| S4b | `~/.local/bin/ghostlight` ownership per ADR-0126 D8: create, idempotent repeat, ownership check, removal on uninstall | **DONE, with a limit.** Linux-only by definition; the creation path is `cfg(unix)` and only executes in the Linux CI lane and on a Linux host. Inspection and the not-applicable path are cross-platform and testable here |
| S4c | `NO_COLOR` honored wherever the CLI styles output | Yes. **DONE**, as a guard: nothing is styled |
| S4d | Man pages for the three siblings, authored, installed by the Debian package and available to the per-user route | Partly. **DONE**: content, per-user install, and deb wiring here; `man` rendering and the deb payload check are Linux |
| S4e | Shell completions for bash, zsh, and fish, plus the guard comparing their subcommand list against the parser | Yes for the guard. **DONE**; live shell completion is Linux |
| S4f | `doctor` parity guard: every reportable state has a line in the same words the workbench uses | Yes. **DONE**, Rust half; the workbench half is S6 |

S4f is the substep that couples forward to S6. Today's reportable set is what the workbench renders
now; when S6 reshapes the landing surface it must keep this guard passing rather than weaken it.

## S7 substeps

S7 begins with a policy-schema change and ends with spawning a browser on someone's desktop. Those
belong in different commits, and the last one cannot be proved by a unit test at all.

| Substep | Scope | Verifiable without a live desktop |
| --- | --- | --- |
| S7a | Register `browser.startup` with the closed values `on_demand` and `manual` in the policy manifest, per ADR-0126 Decision 7: typo-closed validation, per-platform defaults, organization ceiling behavior, the workbench editor's grouped toggles, and the policy grammar fixtures | Yes |
| S7b | The decision layer: find the one seam that learns readiness failed, choose a browser only from deterministic evidence, reuse the existing Snap and Flatpak diagnosis, refuse ambiguity by naming candidates, and return one useful outcome in `manual` mode. Single-flight guard included | Yes |
| S7c | The launch itself: spawn the chosen browser with the person's ordinary profile and no automation flags, wait bounded for the adapter, and name the exact failure on exhaustion. Windows first; Linux only if a session environment resolves through the ADR-0082 seam, and diagnosis-only is an acceptable Linux completion | **No.** Spawning a browser is not a unit test. Windows live proof is owner or windows-codex; Linux is the S8 lane |

S7a is a governance-surface change: it touches the manifest validator, the effective-authority
projection, the workbench policy editor, and the documented policy grammar. Start it fresh rather
than appended to a long session.

## Gate and evaluation log

| Date | Stage | Commit | Automated gates | Live evidence | Result |
| --- | --- | --- | --- | --- | --- |
| 2026-08-16 | S1 | `9b537a14` | `cargo fmt --check` and `npm test --prefix extension` passed. Clippy and `cargo test` not rerun: no source changed | None required | PASS, documentation only |
| 2026-08-16 | S2 | `a46a0db0` | `npm test --prefix extension` 115 pass 0 fail (was 106); `node --check` on all four changed files; `cargo fmt --check`. Clippy and `cargo test` not rerun: no Rust source changed | Not yet observed in a live browser profile; owed before S8 | PASS |
| 2026-08-16 | S3 | (this commit) | `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` 254 orchestrator library, 4 binary, 32 bridge, 6 MCP connector, 0 failed; `npm test --prefix extension` 115 pass; `node tests/process-journey.mjs`; `node tests/cli-journey.mjs` | Live `ghostlight doctor` on the Windows authoring host printed `Environment: Windows -- Ghostlight is in your notification area, and in the Start menu.` | PASS |
| 2026-08-16 | S4a | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 254/4/32/6, 0 failed; `npm test --prefix extension` 115 pass; `node tests/cli-journey.mjs` | `ghostlight doctor --json` parsed as JSON on the Windows host; `ghostlight status --json` byte shape unchanged | PASS |
| 2026-08-16 | S4b | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 261/4/32/6, 0 failed; `npm test --prefix extension` 115 pass; CLI and process journeys | Windows `doctor` reports the command entry as not applicable and still names the absolute executable. The four symlink tests compile here but return early: they prove behavior only in the Linux lane | PASS with a recorded limit |
| 2026-08-16 | S4c | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 261/5/32/6, 0 failed; `npm test --prefix extension` 115 pass | Source audit found no ANSI escape or color dependency in any crate, the npm launcher, or the shell scripts | PASS |
| 2026-08-16 | S4d | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/5/32/6, 0 failed; `npm test --prefix extension` 115 pass; PowerShell parse of the finalize script | Three man pages authored; per-user install tested; the Debian injection could not run here because `dpkg-deb` and `gzip` are absent on the Windows host | PASS with a recorded limit |
| 2026-08-16 | S4e | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/7/32/6, 0 failed; `npm test --prefix extension` 115 pass; `bash -n` on the bash completion; PowerShell parse of the finalize script | An unknown subcommand now names the eight available ones. Completion guard verified against a negative control | PASS |
| 2026-08-16 | S4f | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/9/32/6, 0 failed; `npm test --prefix extension` 115 pass | Live `ghostlight doctor` reads in plain words: `registered, needs an update` where it used to print `Updatable` | PASS |
| 2026-08-16 | S5 | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 273/9/32/6, 0 failed; `npm test --prefix extension` 116 pass; `node --check` on popup.js; process and CLI journeys | The two directives are pinned character for character against `PINS.md` | PASS |
| 2026-08-16 | S6 | (this commit) | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 280/9/32/6, 0 failed; `npm test --prefix extension` 116 pass; `node --check` on view.js and words.js; workbench-surface and process journeys | The vocabulary guard was verified against a negative control: reintroducing one word literal in view.js fails two assertions | PASS |
| 2026-08-16 | S3 (WSL) | `7a084ae5` | None; observation only | Real WSL2 Debian on the Windows host reports `WSL_DISTRO_NAME=Debian` and `/proc/sys/kernel/osrelease=6.6.87.2-microsoft-standard-WSL2`. Both halves of the pinned WSL rule match independently, and `XDG_CURRENT_DESKTOP` is empty there, which the unknown-row test already covers | PASS, inputs only |

## Deviations and findings

Number every deviation. Record the owning seam, the disposition, and the evidence. Do not silently
widen a stage or work around a broken assumption.

**1. S2 touched a file outside the extension.** The stage was scoped to `extension/`, but
`extension/setup.html` is a new runtime file and `scripts/package-extension.ps1` builds the store ZIP
from an explicit allowlist. Without that one-line addition the page would work unpacked and be
absent from the published extension, which is the worst version of the feature. Owning seam:
packaging. Disposition: allowlist entry added, plus a test that fails if a future surface file is
added without packaging it. Evidence: `the bundled setup page is packaged for the store build` in
`extension/tests/onboarding.test.js`.

**2. S2's live observation is owed.** The prompt asked for an eyes-on check in a Chromium profile
with no native host registered. The authoring host has Ghostlight installed and registered, so that
check was not run. Disposition: carried into S8's migration cases, where it is already required.

**3. S4b's symlink behavior is not proved on this host.** The four tests that create, update,
refuse, and remove the entry now compile on Windows and return early there, so a type error cannot
hide until CI, but they assert nothing on this platform. Their real execution is the Linux CI lane
and S8's Ubuntu run. Recorded rather than implied by a green Windows gate.

**4. S4c had nothing to implement.** `NO_COLOR` exists to suppress styling, and Ghostlight emits
none: no ANSI escape, no color crate, and no styling in the npm launcher or shell scripts. Adding an
environment read would have been dead code. Delivered instead as a guard test that fails if styling
is ever introduced, so the next person has to honor `NO_COLOR` in the same change. Verified against
a negative control.

**5. The Debian man-page injection is unexecuted.** `scripts/finalize-debian-package.ps1` now
installs the three compressed pages before it recomputes `md5sums`, so they are checksummed like any
other payload file. That path needs `dpkg-deb` and `gzip` against a real artifact and could not run
on the Windows authoring host; only a PowerShell parse was performed. It is exercised by the Debian
build and the package lifecycle smokes, and is a required check in S8.

**6. `man_pages` became `user_assets`.** Shell completions install under the same user data
directory with the same ownership rules, and a module named `man_pages` that also wrote completions
would have been a lie. The module was renamed and generalized to own one list of `(relative path,
contents)` entries. Same commit as the completions, so no intermediate commit carries the wrong
name.

**7. Live shell completion is unproved.** The guard proves the three files offer exactly the
command line's own subcommands, and `bash -n` parses the bash file, but no shell was driven to
completion on this host. zsh additionally needs `fpath` configuration for the per-user path, which
`ghostlight.1` now states. Carried into S8.

**8. `doctor` was speaking Rust.** It printed state enums through `{:?}`, so a person saw
`NeedsAttention` and `Updatable`. Five state enums gained a `label()` with pinned words, and doctor
renders those. A guard pins every label and a second guard fails if a doctor line reintroduces debug
formatting. The workbench still renders its own words from the serialized values; making it consume
these exact labels is S6's work, and the S4 substep table already recorded that coupling.

**9. Two S5 test names in the prompt no longer apply.** The stage prompt named
`a_dispatched_effect_settles_truthfully_after_pause` and `held_operation_and_liveness_deadline_agree`.
Both were written for the held-caller design ADR-0126 Decision 4 rejected. Nothing waits now: a
pause denies at the final boundary, so there is no held operation to reconcile with the ADR-0113
deadline, and an already-dispatched effect settles exactly as it did before because the pause never
touched it. The coverage that replaced them is
`pause_prevents_the_next_browser_effect_and_resume_restores_it`, `stop_is_terminal_and_idempotent`,
and `attention_stays_distinct_from_a_human_pause`.

**10. The popup was collapsing attention into the person's pause.** It rendered
`["held", "attention"].includes(...)` as `Agent browsing is PAUSED.`, which tells someone they
paused Ghostlight when policy stopped it. ADR-0126 Decision 6 keeps the two apart, so attention now
reads `Agent browsing is STOPPED and needs you.` A test fails if the two states are merged again.

**11. The window was authoring the readiness answer.** `view.js` had `bandClass` and `bandWord`,
which classified the aggregate state and chose its word in JavaScript. The Policy chip three lines
below already carried the opposite rule in a comment: a surface that invents its own words is a
second source of truth about the one thing that must have one. `language/readiness.rs` now owns the
closed state, the word, the sentence, and the tone; the snapshot carries them; the window renders
them. A guard fails if any readiness word reappears as a literal in `view.js`.

**12. `doctor` has no readiness line, deliberately.** The S6 prompt asked that every front-door
state have a `doctor` line in the same words. The aggregate answer needs live facts -- connected
adapters, running operations, the current control state -- that `doctor` does not have, because it
deliberately does not start or query the service. Inventing a line from partial facts would be the
kind of confident wrong answer this epic exists to remove. The parity that was achievable is
delivered instead: both surfaces draw every state word from Rust, and neither authors its own. If a
readiness line in `doctor` is wanted later, it needs a decision about whether `doctor` may query a
running service.

**13. The WSL row's rendered line is still unproved.** The detection inputs are now verified
against a real WSL2 system: `WSL_DISTRO_NAME` is set and the kernel release contains the pinned
`microsoft` marker, which was the assumption most likely to be wrong. What is not proved is the
sentence `ghostlight doctor` actually prints there, because that needs a Linux build of the
orchestrator and its WebKitGTK stack inside WSL, which was not attempted. This stays with the
Windows host rather than moving to the Linux lane, since WSL is a Windows-side configuration.

## Stage close checklist

- The objective is observable and linked to evidence.
- Every changed decision has an ADR or a marked amendment.
- The change sits at one owning seam inside the modular monolith.
- No redundant surface or duplicated rule remains without a named follow-up here.
- Every gate command in `PINS.md` passed, and the counts are in the gate log.
- `docs/STATUS.md` and the active 1.0 contracts match reality.
- `RESUME HERE`, the stage row, the evidence matrix, and this file's deviations are current.
