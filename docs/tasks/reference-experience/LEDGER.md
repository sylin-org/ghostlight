# LEDGER: the reference Ghostlight experience

Durable progress for the epic defined by [BOOTSTRAP.md](BOOTSTRAP.md), with exact values in
[PINS.md](PINS.md). Update this file before starting a stage, after every material finding, after
every commit, and when closing or blocking a stage.

## RESUME HERE

- State: S1 through S7 and Linux verification V1 through V5 are COMPLETE. S8 is BLOCKED at its
  mandatory real-desktop boundary.
- Owner: no active S8 executor. A Windows source host completed the S7 corrective pass below; it
  did not run the clean installed-Windows candidate lane.
- Next action: run Ubuntu GNOME Wayland L1-L9 and the clean installed-Windows journey, then resume
  the remaining S8 migration, accessibility, public-feedback, and disposition rows from
  [the dated evaluation](../../testing/reference-experience-evaluation-2026-08-16.md).
- Blocking condition: the required Ubuntu GNOME Wayland and clean installed-Windows candidate
  environments have not run. S8 forbids substituting source-only or complementary-desktop proof.
- Source baseline when the epic was reworked: `dev` at `2f24943f`.
- Last green evidence: the S7 corrective Windows-source row below passed the full repository gate
  and real process journeys. It does not replace either installed-desktop S8 lane.

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
| S2 the second machine | COMPLETE | `a46a0db0` | A new computer explains itself | Extension only |
| S3 adaptive familiarity | COMPLETE | `25ddab80` | Local desktop language | Includes WSL |
| S4 terminal citizenship | COMPLETE | `61f18bd9`..`d282ede2` | PATH, man, completions, `--json` | Six substeps, S4a-S4f |
| S5 human runtime control | COMPLETE | `efc15783` | Pause, resume, stop | Language and state; ADR-0126 D4-D6 |
| S6 At a glance | COMPLETE | `124a5557` | One calm window | ADR-0126 D9 |
| S7 readiness recovery | COMPLETE | `fb9845b` plus corrective pass below | Safe repair, exact refusal | Three green substeps, S7a-S7c |
| S8 evaluation | BLOCKED | -- | Evidence on real desktops | Required Ubuntu GNOME and installed Windows lanes unavailable on this host |

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
| Readiness recovery | S7 | Per-platform posture, single flight, bounded waits, exact failures | COMPLETE | S7a: registered startup posture and workbench choice. S7b: one executor seam, executable-backed installation diagnosis, per-generation single flight with caller-local cancellation and deadlines, closed failures, injected manual-mode zero-launch coverage, and a process-level pinned-disconnect refusal. S7c: safe owned-registration repair, exact inventoried executable with no arguments, ADR-0082 graphical-session proof on Linux, bounded inbound-adapter wait, and cancellation cleanup |
| Evaluation | S8 | Ubuntu GNOME lifecycle, Windows lane, migration cases, dispositions | BLOCKED | [Dated S8 evaluation](../../testing/reference-experience-evaluation-2026-08-16.md): local evidence inventoried; required desktop lanes and owner disposition remain |

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
| Whether 1.0 publishes before this epic lands | owner | DECIDED 2026-08-16: no | 1.0 waits for this epic. The epic is now on the release critical path; S8 is a release gate, not a nice-to-have |
| Whether `doctor` may query a running authority for aggregate readiness | owner/S8 | CLOSED 2026-08-17 | Yes, through one authenticated read-only opening that never demand-starts, opens a session, or writes audit. ADR-0126 amendment |
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
| S7a | Register `browser.startup` with the closed values `on_demand` and `manual` in the policy manifest, per ADR-0126 Decision 7: typo-closed validation, per-platform defaults, organization ceiling behavior, the workbench editor's grouped toggles, and the policy grammar fixtures | Yes. **DONE** |
| S7b | The decision layer: find the one seam that learns readiness failed, choose a browser only from a verified ordinary executable, keep registration as a separate readiness fact, reuse the existing Snap and Flatpak diagnosis, refuse ambiguity by naming candidates, and return one useful outcome in `manual` mode. Per-generation single-flight guard included | Yes. **DONE** |
| S7c | The launch itself: spawn the chosen browser with the person's ordinary profile and no automation flags, wait bounded for the adapter, and name the exact failure on exhaustion. Windows first; Linux only if a session environment resolves through the ADR-0082 seam, and diagnosis-only is an acceptable Linux completion | **DONE.** Source and deterministic mechanisms are complete. Linux manual-mode diagnosis is live-proved; physical Windows launch remains S8 evidence |

S7a is a governance-surface change: it touches the manifest validator, the effective-authority
projection, the workbench policy editor, and the documented policy grammar. Start it fresh rather
than appended to a long session.

## Gate and evaluation log

| Date | Stage | Commit | Automated gates | Live evidence | Result |
| --- | --- | --- | --- | --- | --- |
| 2026-08-16 | S1 | `9b537a14` | `cargo fmt --check` and `npm test --prefix extension` passed. Clippy and `cargo test` not rerun: no source changed | None required | PASS, documentation only |
| 2026-08-16 | S2 | `a46a0db0` | `npm test --prefix extension` 115 pass 0 fail (was 106); `node --check` on all four changed files; `cargo fmt --check`. Clippy and `cargo test` not rerun: no Rust source changed | Not yet observed in a live browser profile; owed before S8 | PASS |
| 2026-08-16 | S3 | `25ddab80` | `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` 254 orchestrator library, 4 binary, 32 bridge, 6 MCP connector, 0 failed; `npm test --prefix extension` 115 pass; `node tests/process-journey.mjs`; `node tests/cli-journey.mjs` | Live `ghostlight doctor` on the Windows authoring host printed `Environment: Windows -- Ghostlight is in your notification area, and in the Start menu.` | PASS |
| 2026-08-16 | S4a | `61f18bd9` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 254/4/32/6, 0 failed; `npm test --prefix extension` 115 pass; `node tests/cli-journey.mjs` | `ghostlight doctor --json` parsed as JSON on the Windows host; `ghostlight status --json` byte shape unchanged | PASS |
| 2026-08-16 | S4b | `34e36d9d` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 261/4/32/6, 0 failed; `npm test --prefix extension` 115 pass; CLI and process journeys | Windows `doctor` reports the command entry as not applicable and still names the absolute executable. The four symlink tests compile here but return early: they prove behavior only in the Linux lane | PASS with a recorded limit |
| 2026-08-16 | S4c | `59ff9471` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 261/5/32/6, 0 failed; `npm test --prefix extension` 115 pass | Source audit found no ANSI escape or color dependency in any crate, the npm launcher, or the shell scripts | PASS |
| 2026-08-16 | S4d | `7a936f21` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/5/32/6, 0 failed; `npm test --prefix extension` 115 pass; PowerShell parse of the finalize script | Three man pages authored; per-user install tested; the Debian injection could not run here because `dpkg-deb` and `gzip` are absent on the Windows host | PASS with a recorded limit |
| 2026-08-16 | S4e | `8b383d1d` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/7/32/6, 0 failed; `npm test --prefix extension` 115 pass; `bash -n` on the bash completion; PowerShell parse of the finalize script | An unknown subcommand now names the eight available ones. Completion guard verified against a negative control | PASS |
| 2026-08-16 | S4f | `d282ede2` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 266/9/32/6, 0 failed; `npm test --prefix extension` 115 pass | Live `ghostlight doctor` reads in plain words: `registered, needs an update` where it used to print `Updatable` | PASS |
| 2026-08-16 | S5 | `efc15783` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 273/9/32/6, 0 failed; `npm test --prefix extension` 116 pass; `node --check` on popup.js; process and CLI journeys | The two directives are pinned character for character against `PINS.md` | PASS |
| 2026-08-16 | S6 | `124a5557` | `cargo fmt --check`; warnings-denied workspace clippy; `cargo test --workspace` 280/9/32/6, 0 failed; `npm test --prefix extension` 116 pass; `node --check` on view.js and words.js; workbench-surface and process journeys | The vocabulary guard was verified against a negative control: reintroducing one word literal in view.js fails two assertions | PASS |
| 2026-08-16 | S3 (WSL) | `7a084ae5` | None; observation only | Real WSL2 Debian on the Windows host reports `WSL_DISTRO_NAME=Debian` and `/proc/sys/kernel/osrelease=6.6.87.2-microsoft-standard-WSL2`. Both halves of the pinned WSL rule match independently, and `XDG_CURRENT_DESKTOP` is empty there, which the unknown-row test already covers | PASS, inputs only |
| 2026-08-16 | V1 Linux command entry | this commit | Seven focused command-entry tests; `cargo fmt --check`; warnings-denied workspace Clippy; `cargo test --workspace` 286/9/32/6, 0 failed; 116 extension tests; isolated workspace build; process, CLI, and workbench-surface journeys | CachyOS KDE Wayland, ordinary per-user install in a disposable home: a new bash process ran `ghostlight doctor --json`; repeat install was byte-identical; foreign file and symlink survived install and uninstall byte-for-byte; uninstall removed only the owned entry; the real bash, zsh, profile, and fish startup-file hashes did not change; the active installed diagnosis matched before and after. No `/usr/bin/ghostlight` package was available, so the already-unit-proved not-applicable package row remains part of V2 | PASS; deviation 3 resolved |
| 2026-08-16 | V2 Linux manual pages | this commit | Rootless network-disabled Ubuntu 22.04 build with Rust 1.95.0 and Tauri CLI 2.11.0; current-source optimized workspace and Debian bundle; finalizer; Debian 12 `lintian`; package and per-user `man` rendering; full repository gate | The finalized 1.0.0 package has SHA-256 `20d85aca6d1932f544b55711d8498af73117a453e4aa98f383c854d4448a6c29`. All three compressed pages are present under `usr/share/man/man1`, and all three are in `md5sums`. Debian 12 `lintian` no longer reports any missing manual page; its remaining findings are the six browser-mandated `/etc/opt` paths and an embedded-libyaml string-table finding. All three package pages rendered. A disposable per-user install rendered all three through the owned PATH entry with `MANPATH` unset | PASS; deviation 5 resolved |
| 2026-08-16 | V3 Linux shell completions | this commit | Current Debian payload and disposable per-user install; real bash 5.3, fish 4.8.1, and zsh 5.9 completion engines; focused zsh regression; full repository gate | The package contains all three conventional vendor paths and the per-user route contains all three XDG paths. Bash and fish offered exactly the eight accepted subcommands and their doctor flags. Zsh offered the commands but initially no options; tracing found that its first `_arguments -C` call shifted the subcommand context. The completion now captures and shifts the context explicitly, a guard protects that relationship, and real Tab completion offers `--fix`, `--json`, and `--verbose`. The documented `fpath=(~/.local/share/zsh/site-functions $fpath)` plus `compinit` instruction loaded the completer. The rebuilt package contains the byte-exact repaired zsh file and has SHA-256 `dbc2642a7d7f24042d806fd91766ea39a07b444991cd9696d7eb23132cff465b` | PASS; deviation 7 resolved; finding 14 fixed |
| 2026-08-16 | V4 second-machine extension handoff | this commit | Real Chromium 151 on KDE Wayland; unpacked extension in a throwaway home with no native host; live popup and options pages through the browser debugging protocol; online and emulated-offline setup clicks; ordinary per-user native-host registration and browser restart; active installation diagnosis before and after | Both surfaces showed the exact not-installed sentence and `Set up Ghostlight`, and neither showed the waiting sentence. Online setup opened the canonical walkthrough. With the target offline, setup opened bundled `setup.html`; it reached `complete`, rendered the full instructions, and loaded no network resource. After the ordinary installer registered Chromium and the disposable browser restarted, both surfaces said connected and hid setup. The machine's working registration and service remained current throughout | PASS; deviation 2 resolved; finding 15 resolved |
| 2026-08-16 | V5 Linux environment rows | this commit | Current debug `ghostlight doctor` on real KDE Plasma 6.7.4 Wayland; the same binary in a shell with unrecognized `XDG_CURRENT_DESKTOP=Hyprland`; full repository gate | The real desktop printed `Environment: Linux (KDE) -- Ghostlight is in your system tray and your Applications menu.` The unrecognized shell printed `Environment: Linux -- Ghostlight is in your Applications menu, and in your system tray if your desktop draws one.` GNOME is not installed on this host, so no live GNOME claim is made; the exhaustive unit guard still proves that its row contains no tray promise. The WSL row remains the Windows lane's evidence | PASS |
| 2026-08-16 | S7a browser startup setting | this commit | `cargo fmt --check`; warnings-denied workspace Clippy; `cargo test --workspace` 291/10/32/6, 0 failed; 116 extension tests; policy-grammar and workbench-surface journeys; isolated workspace build; process and CLI journeys | Schema 3 accepts only `on_demand` and `manual`; non-string and unknown values fail. Defaults are `on_demand` on Windows and `manual` on Linux. The effective projection composes organization and user values, with organization `manual` pinning the result. The workbench renders and authors one closed two-option select. The active guide and organization fixture carry the new grammar. No recovery or browser launch behavior was added in this substep | PASS |
| 2026-08-16 | S7b browser recovery decision | this commit | `cargo fmt --check`; warnings-denied workspace Clippy; `cargo test --workspace` 299/10/32/6, 0 failed; 116 extension tests; `node --check tests/process-journey.mjs`; isolated workspace build; process and CLI journeys | The executor's browser-target seam is the sole recovery caller. Installation inspection reuses the native package, Snap, and Flatpak diagnosis; ambiguity names candidates; simultaneous requests join one decision attempt per scope; cancellation leaves no active flight; and manual mode performs no launch. A real no-browser CLI call on CachyOS KDE Wayland returned `failed`, `effect: none`, reason `browser_startup_manual`, and one start-browser next step in 53 ms. The active installed service and browser registration were unchanged | PASS |
| 2026-08-16 | S7c bounded browser launch | this commit | `cargo fmt --check`; warnings-denied workspace Clippy; `cargo test --workspace` 301/10/34/6, 0 failed; 116 extension tests; isolated workspace build; process and CLI journeys | The selected native executable is launched directly with no arguments, preserving the ordinary profile and adding no automation flag. An owned stale registration is the only repairable state. Linux launch requires a graphical display plus a real, owner-private same-user runtime directory resolved at the shared ADR-0082 seam; otherwise it returns an exact no-effect launch diagnosis. Launch and adapter wait remain one per-scope flight inside the caller deadline. Cancellation clears it; exhaustion reports `browser_handshake_timeout`. The Linux default remains manual and its 53 ms live no-browser outcome from S7b is the platform completion allowed by ADR-0126 D7. No Windows host was available for a physical launch; that lane remains explicit S8 evidence | PASS on the Linux completion path; Windows live evidence owed in S8 |
| 2026-08-16 | S8 evaluation attempt | this commit | `cargo fmt --check`; warnings-denied workspace Clippy; `cargo test --workspace` 301/10/34/6, 0 failed; 116 extension tests; isolated workspace build; process, CLI, and 39-assertion workbench-surface journeys | Host inspection reports CachyOS, KDE, Wayland, with no `gnome-shell`; no Windows host is available. The existing Ubuntu L1-L9 record remains entirely `NOT RUN`, and the existing Windows record explicitly excludes installed lifecycle work. The dated evaluation dispositions every ADR-0126 measure at the evidence currently available and names the exact unblock sequence | BLOCKED by the S8 STOP condition; no platform substitution |
| 2026-08-16 | S7 corrective Windows-source pass | `47d26b0b` | `cargo fmt --all -- --check`; warnings-denied workspace Clippy; `cargo test --workspace` 307/10/33/6, 0 failed; 116 extension tests; 10 npm launcher tests; isolated workspace build; process, CLI, PowerShell CLI, workbench-surface, and policy-grammar journeys | Four current Windows registrations with one verified executable now select that browser; an absent executable is not a candidate; two installed browsers remain ambiguous. Manual mode diagnoses unusable registration, freshly revalidates owned stale state before repair, and never launches. Flight-creator cancellation and a short creator deadline no longer decide a live joiner's result. The process journey uses an already-pinned fake browser and cannot enter host repair or launch. No clean package or visible browser launch was run | PASS for source and process boundaries; installed Windows evidence remains owed in S8 |
| 2026-08-16 | S7 corrective Windows live-source verification | this commit | Exact-path release swap; live service and connector inspection; `ghostlight status --json`; real Chrome open/read/screenshot; isolated release CLI recovery against actual fixed-root inventory | The stale `target/release` authority was replaced with release SHA-256 `CC20AF4A1E6EBF3C120E9CBB30954B7F4B4103C0332FC7AE4E625A9B014EDF7B`; existing connectors converged on one new authority. Real Chrome completed open, read, and screenshot. Close was correctly blocked by `preserve-tabs`; the one test tab was closed directly without changing the setting. The isolated no-adapter call returned `failed`, `effect: none`, `repeat_safe: true`, reason `browser_recovery_ambiguous`, and exactly Google Chrome plus Microsoft Edge. Doctor reported all four native-host registrations missing; no registration changed and no browser launch occurred | PASS for live source deployment, adapter work, and real-inventory ambiguity; clean installed-Windows on-demand launch remains owed in S8 |

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

**2. RESOLVED by V4 on Linux.** Real Chromium loaded the unpacked extension from a throwaway home
with no native host. The popup and options page both showed the exact not-installed sentence and a
`Set up Ghostlight` control, never the waiting sentence. The online control opened the canonical
walkthrough. The same control with its target offline opened bundled `setup.html`, which reached a
complete document with the full instructions and no network resources. After an ordinary per-user
registration and browser restart, both surfaces said connected and hid setup. The machine's real
registration was never removed or changed, and its diagnosis matched afterward.

**3. RESOLVED by V1 on Linux.** All seven command-entry tests ran on CachyOS rather than returning
early. An ordinary per-user install in a disposable home created exactly
`~/.local/bin/ghostlight`; a new bash process resolved it and ran `doctor --json`; repeat install
changed no bytes; regular-file and foreign-symlink occupants stayed byte-identical through install
and uninstall; and uninstall removed the owned link without changing a sentinel beside it. Hashes
for the real `.bashrc`, `.zshrc`, `.profile`, and fish startup file were identical before and after.
The active installed Ghostlight diagnosis was also unchanged before and after the task. The
system-package not-applicable branch passed its focused test; payload proof against a real Debian
package belongs to V2.

**4. S4c had nothing to implement.** `NO_COLOR` exists to suppress styling, and Ghostlight emits
none: no ANSI escape, no color crate, and no styling in the npm launcher or shell scripts. Adding an
environment read would have been dead code. Delivered instead as a guard test that fails if styling
is ever introduced, so the next person has to honor `NO_COLOR` in the same change. Verified against
a negative control.

**5. RESOLVED by V2 on Linux.** A network-disabled rootless Ubuntu 22.04 builder produced a real
current-source Debian package, and the finalizer injected all three compressed pages before
recomputing `md5sums`. Debian 12 found and rendered each page from the extracted package. `lintian`
reported no missing-manual-page finding; only the browser-mandated `/etc/opt` locations and its
embedded-libyaml string-table finding remained. A disposable per-user installation also rendered
all three pages with `MANPATH` unset once the owned `~/.local/bin` entry was present on `PATH`, which
is the exact relationship the manual and `user_assets.rs` describe.

**6. `man_pages` became `user_assets`.** Shell completions install under the same user data
directory with the same ownership rules, and a module named `man_pages` that also wrote completions
would have been a lie. The module was renamed and generalized to own one list of `(relative path,
contents)` entries. Same commit as the completions, so no intermediate commit carries the wrong
name.

**7. RESOLVED by V3 on Linux.** Bash 5.3, fish 4.8.1, and zsh 5.9 each offered the eight accepted
subcommands from a disposable per-user installation. Bash and fish offered their doctor flags.
Zsh offered the same after finding 14 was fixed. Its documented `fpath` plus `compinit` instruction
is accurate. The Debian package contains the three conventional vendor paths, and the rebuilt
package's zsh file is byte-identical to the repaired source.

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

**12. `doctor` originally had no readiness line; resolved 2026-08-17.** The S6 prompt asked that
every front-door state have a `doctor` line in the same words. The aggregate answer needs live
facts -- connected adapters, running operations, and the current control state -- that local file
inspection cannot supply. Inventing a line from partial facts would be the kind of confident wrong
answer this epic exists to remove. The owner approved a live query: `doctor` now reads the exact
orchestrator-owned `ReadinessSummary` through one authenticated read-only service opening. It does
not demand-start an absent authority, open a session, reveal the workbench, or write audit. Wire,
no-mutation, absent-service, six-state language, and real CLI process tests pass. Required installed
desktop observation remains S8 evidence.

**13. The WSL row's rendered line is still unproved.** The detection inputs are now verified
against a real WSL2 system: `WSL_DISTRO_NAME` is set and the kernel release contains the pinned
`microsoft` marker, which was the assumption most likely to be wrong. What is not proved is the
sentence `ghostlight doctor` actually prints there, because that needs a Linux build of the
orchestrator and its WebKitGTK stack inside WSL, which was not attempted. This stays with the
Windows host rather than moving to the Linux lane, since WSL is a Windows-side configuration.

**14. V3 found and fixed a zsh subcommand-context defect.** Top-level Tab completion worked, but
`ghostlight doctor --` offered nothing. The completion called `_arguments -C` before choosing the
subcommand, which shifted `words` and `CURRENT`; the later branch therefore depended on mutated
helper state. The function now chooses `words[2]` first, then shifts once into the subcommand
context before describing its options. A source guard fails if either relationship regresses, and
real zsh Tab completion now offers `--fix`, `--json`, and `--verbose`.

**15. V4's first disposable profile used a nonstandard Chromium data directory.** Chromium resolves
native-host manifests under an explicit `--user-data-dir`, while the installer correctly registers
the browser's ordinary per-user directory. A file-access trace proved the two paths differed. The
final run used a throwaway `HOME` with Chromium's normal profile location, so the ordinary installer
and browser shared the production path shape. The no-host state, registration, restart, and
connected state then passed without changing the machine's working registration.

**16. S8 has not run its two required installed desktops.** The dated evaluation host was CachyOS
KDE Wayland with no GNOME installation and could not execute the installed Windows lane. A later
Windows source host repaired and proved the deterministic recovery mechanisms, but did not use a
clean package candidate or matching store extension. The S8 STOP condition and ADR-0123 forbid
treating either source-only or complementary-desktop evidence as the missing installed journeys.
Disposition: S8 remains `BLOCKED`; the exact missing journeys and acceptance rows are recorded in
`docs/testing/reference-experience-evaluation-2026-08-16.md`.

**17. RESOLVED: Windows registration was mistaken for browser installation.** The package
deliberately pre-registers Chrome, Edge, Brave, and Chromium, while the recovery decision treated
each current registration as an installed candidate. A normal Windows install therefore became
ambiguous before checking which executable existed. Recovery now carries the exact fixed-root
ordinary executable in its inventory snapshot, selects only executable-backed candidates, and
launches that same path. The same pass replaced shared caller results with exact flight generations:
cancellation and deadlines are participant-local, a live joiner can continue the phase, and repair
or launch is rechecked at its pre-effect boundary. Pure tests guard both relationships. The Windows
process journey uses an already-pinned disconnected fake browser, so it exercises the pre-effect
refusal without entering host repair or launch or changing real HKCU state.

A later live-source check replaced the stale development authority, proved that the unchanged
browser connector renegotiates with it, and exercised the corrected inventory against the host's
real fixed roots. The no-adapter result named only installed Chrome and Edge, not the absent Brave
and Chromium families. The machine had no native-host registrations, so no successful on-demand
launch claim follows from that result.

## Stage close checklist

- The objective is observable and linked to evidence.
- Every changed decision has an ADR or a marked amendment.
- The change sits at one owning seam inside the modular monolith.
- No redundant surface or duplicated rule remains without a named follow-up here.
- Every gate command in `PINS.md` passed, and the counts are in the gate log.
- `docs/STATUS.md` and the active 1.0 contracts match reality.
- `RESUME HERE`, the stage row, the evidence matrix, and this file's deviations are current.
