# STATUS -- Ghostlight 1.0 source candidate

Last updated: 2026-09-04 (ADR-0151 through ADR-0154 are implemented in source and are not
released).

## Explicit integration Fix (ADR-0154, 2026-09-04)

The owner asked for an actionable repair on MCP integration cards where a foreign command occupies
Ghostlight's registration key. [ADR-0154](adr/0154-explicit-foreign-harness-entry-fix.md) adds one
confirmed per-target `Fix` action. The projection advertises it only for a parseable foreign entry
that the existing lossless writer can isolate. The writer re-checks that state inside the harness
mutation lock, replaces only the `ghostlight` entry, preserves unrelated content, creates the
existing `.ghostlight-backup`, and atomically replaces the file. A stale or repeated Fix refuses
without changing bytes.

Malformed, unreadable, missing, current, and non-lossless entries do not offer Fix. Ordinary Set
up, Update, Remove, CLI setup, and `Set up everything` retain the no-foreign-overwrite rule. The
workbench uses the existing closed `manage_harness` command and confirms the backup before sending
`fix`; no connector, bridge, extension, browser, or MCP contract changed.

Gates green: formatting, warnings-denied workspace Clippy, all 436 Rust workspace tests, all 171
extension tests, JavaScript syntax checks, repository integrity, the fresh isolated-target process,
CLI, PowerShell, npm-launcher, MCPB-launcher, and workbench journeys. The orchestrator was deployed
through the development loop. Its read-only `doctor --json` projection reports the real Cline CLI
and Visual Studio Code entries as `needs_attention` with `can_fix: true`; absent Cline targets have
`can_fix: false`. Diagnostics remain off. No Fix was invoked and no client configuration was
changed during the live check.

## Composed full-page reading (ADR-0151, 2026-09-04)

The owner confirmed that the shortest `browser_read` call should match the visible page rather
than prefer an article heuristic. [ADR-0151](adr/0151-composed-full-page-reading.md) amends
ADR-0133 and ADR-0138: an omitted mode now selects composed visible reading across the top
document, open shadow roots, assigned slots, and injected http(s) frames in stable order under one
global character ceiling. Hidden and editable content stays absent, and closed roots stay closed.
Explicit `article` mode still probes a useful top-document article and falls back to the same
composed page read when none exists. Target reads use the same frame-local composed traversal.

The root failure crossed two independent seams: the orchestrator routed an omitted mode through
the legacy top-frame command, and the content adapter used `innerText`, which stops at a shadow
boundary. The default is now a closed `ReadMode`, all document reads use `read_document`, and the
adapter owns one composed-tree collector plus one cross-frame merger. The complete meaning is
negotiated as `semantic_document` revision 4, so an older extension refuses before dispatch rather
than returning a narrower result. The browser connector and wire shape remain unchanged.

Focused extension tests cover nested open roots, slots, hidden and editable exclusions, article
fallback, stable frame order, and one global ceiling. Gates green: formatting, warnings-denied
workspace Clippy, all 432 Rust workspace tests, all 171 extension tests, JavaScript syntax checks,
the fresh isolated-target process journey, the PowerShell journey, the npm launcher suite, and
whole-repo integrity. Extension bytes changed, so this source is newer than the published adapter
and no store or release action has been taken.

## Composed semantic observation and geometry (ADR-0152 and ADR-0153, 2026-09-04)

The owner asked that the read fix be applied to sibling tools. The audit found the same boundary in
text waits, find matching, accessible-name fallback, document trees, shadow-hosted iframe geometry,
point receipts, and coordinate image drops. [ADR-0152](adr/0152-composed-semantic-observation.md)
makes read, inspect, find, and wait share the composed semantic layer. Rootless document trees now
include frames under one global 400-node budget. [ADR-0153](adr/0153-composed-frame-geometry.md)
makes frame boxes shadow-aware and routes points recursively to the deepest observable frame and
subject, while CDP keeps top-viewport coordinates.

The stronger meanings advance `observation` to revision 2, `pointer_input` to revision 3,
`capture` to revision 2, and `files` to revision 3. Every semantic-document command now requires
revision 4. No public command schema, authority class, or audit field changed. Target-less browser
capture, URL/load/target waits, dialog handling, diagnostics, and script evaluation retain their
existing document or browser scope because the audit found no composed-page promise there.

The unpacked extension was reloaded and the real MCP journey passed against
`https://sylin.org/ghostlight/demo/iframe/`: default read returned 146 words spanning the outer
page and embedded form; document inspect, find, composed text wait, an iframe-target hover and
screenshot, seven-field semantic fill, submit, completion wait, and completion read all succeeded.
The live journey now keeps those checks. The sibling website source expands the fixture further by
placing the iframe in an open shadow root and the completion state in a nested open root; its local
build, 34-page site check, Browser-plugin structure check, and local form journey passed. Those
website changes remain local and unpublished.

## 1.3.2 published (2026-09-02)

Ghostlight 1.3.2 is public: GitHub release
[`v1.3.2`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.2) (20 files, re-downloaded
and hash-compared before publication), npm `ghostlight@1.3.2` (tarball SHA-256
`ecdbedbc4b1cb3907cfed97639830a91f82a5a2dccab609db9c128264509af97`), the MCP Registry record
`org.sylin/ghostlight 1.3.2`, and the refreshed website, from custody-verified candidate run
[33643387463](https://github.com/sylin-org/ghostlight/actions/runs/33643387463) at frozen
revision `45639541` (two verified local copies,
[candidate-custody-2026-09-02](testing/candidate-custody-2026-09-02.md)). `check-public-surfaces
-Online` reports GitHub, npm, the Chrome update feed, the MCP Registry, and the website in
agreement (source 1.3.2, public 1.3.2, public adapter 1.0.0). The public install smoke passed:
the public npm launcher checksum-verified all three binaries and the real 1.3.2 orchestrator
answered `doctor --json`, reporting this machine's browsers `owned_elsewhere` with the dev tree
named. The release carries one fix: ADR-0150, demand-start identity follows the runtime
override, so a floating launcher entry (Cline's `npx -y ghostlight`) now demand-starts the
elected authority instead of a second one. The adapter stays 1.1.0 with byte-identical
candidate bytes; no store action. Scoop and WinGet metadata ride the `package-manager-metadata`
artifact; their bucket submissions remain owner actions.

## ADR-0150: the runtime override elects the demand-start authority (2026-09-02)

The owner found two Ghostlight workbenches running at once: the browser's native-host
registration named the development tree while Cline's `npx -y ghostlight` entry resolved to the
installed 1.3.1, whose connector demand-started its own sibling -- one authority serving the
browser and its harnesses, one serving only Cline with no browser and no way to reach one.
[ADR-0150](adr/0150-runtime-override-elects-demand-start-authority.md) completes the existing
`GHOSTLIGHT_RUNTIME_FILE` seam instead of building a new affordance or reviving the retired
ADR-0048 auto-shadow: with the override set, its directory elects the authority for discovery,
lease, deploy lock, and demand-start identity, and an elected directory without an authority
fails loudly rather than letting a foreign binary spawn into the slot. Without the override,
per-installation election beside the executable is unchanged. Routing writes and adopts no
registration; `owned_elsewhere` stands.

Gates green: formatting, warnings-denied workspace Clippy, every Rust workspace suite, all 156
extension tests, and the process, CLI, and PowerShell journeys, whose runtime overrides now
point inside the build under test so the elected directory and the quiesce lock stay coherent.
Proven live on this machine: a fixed connector pointed at an elected directory holding only a
fixed orchestrator demand-started exactly that authority (one new process, from the elected
path, initialize answered), and the real `npx -y ghostlight` chain with the override converged
on the dev authority spawning nothing. Cline's server entry now carries the override to the dev
runtime file (original kept beside it as `cline_mcp_settings.json.bak-2026-09-02`), and the
stale installed 1.3.1 pair was stopped connector-first by exact path, so the machine runs one
authority again. The corrected demand-start reaches the launcher stage at the next release:
hand copies into the launcher's versioned cache are reverted by checksum verification on every
launch, so "deploy it to the npx stage" means "publish". Until then a release-era connector
with the override converges whenever the elected authority is up; only its demand-start half
needs the new build.

The 1.3.2 candidate is assembled and held: candidate run
[33643387463](https://github.com/sylin-org/ghostlight/actions/runs/33643387463) at frozen
revision `45639541` ([candidate-custody-2026-09-02](testing/candidate-custody-2026-09-02.md)),
preflight 19/19 green ([release-preflight-2026-09-02](testing/release-preflight-2026-09-02.md)),
two verified local copies, provenance green, and the extension ZIP byte-identical to the
approved store adapter 1.1.0 (no store action). The publication sequence
(GitHub draft/publish, npm, MCP Registry, website) is owner-gated and not started.

## npm distribution moved under the sylin-org org (2026-09-01)

The owner found the npm package released under the personal account and directed the fix.
The `sylin-org` npm organization exists, and its `developers` team now holds read-write
access to `ghostlight`, granted through the organization settings (observed persisting
across reload and in the org's package list), so the organization can manage and publish
the package. The maintainer-of-record transfer (`npm owner add sylin-org ghostlight`) is
owed, not done: npm demands a one-time code for it, the account is passkey-only (npm's
two-factor page offers security keys and recovery codes, no authenticator app), the
website has no organization-as-maintainer flow, and npm's August 2026 restriction bars
bypass-2FA tokens from account changes. The owner directed leaving it at the org-access
state for now; a recovery code or an npm support request finishes it later. When the flip
lands: simplify `glama.json` maintainers to `sylin-org` and add the registry-maintainers
assertion to `check-public-surfaces.ps1 -Online` (both deliberately absent now -- they
would claim ahead of live truth). The npm launcher package names `sylin-org` as its author
from the next release. The machine's original npm token was restored after a web-login
detour; the login token the detour created stays in the account's token list for the owner
to revoke.

## 1.3.1 published (2026-08-31)

Ghostlight 1.3.1 is public: GitHub release
[`v1.3.1`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.1) (20 files, re-downloaded
and hash-compared before publication), npm `ghostlight@1.3.1` (tarball SHA-256
`e291c27c229a1f266575d70bf2653ac7f1690733119e45fbfe95637699d81b4f`), the MCP Registry record
`org.sylin/ghostlight 1.3.1`, and the refreshed website, from custody-verified candidate run
[33355735166](https://github.com/sylin-org/ghostlight/actions/runs/33355735166) at frozen
revision `0d7b7759` (two verified local copies,
[candidate-custody-2026-08-31](testing/candidate-custody-2026-08-31.md)). `main` was
fast-forward promoted (`296b0209..2e4e1448`), so main and dev are current together. The release
carries the ADR-0149 amendment (cross-tree adoption requires a deliberate install, the
`owned_elsewhere` diagnosis with dead-tree visibility) and the `GHOSTLIGHT_NATIVE_HOST_DIR`
isolation seam with its journey and preflight machine-state guards. The adapter stays 1.1.0
with byte-identical candidate bytes; no store action. The public install smoke passed: the
public npm launcher checksum-verified all three binaries, and the real 1.3.1 orchestrator
answered `doctor --json` reporting this machine's browsers `owned_elsewhere` with the dev tree
named -- the amendment's honest answer, observed on the public channel. Scoop and WinGet
metadata ride the `package-manager-metadata` artifact; their bucket submissions remain owner
actions.


## Cross-tree registration adoption requires install (2026-08-30)

The owner accepted both follow-ups to the registration-leak incident. The
[ADR-0149 amendment](adr/0149-recovery-never-presents-a-browser-choice.md) narrows silent repair
to stale details within the running installation's own directory. A Ghostlight-owned
registration naming another installation's connector is the new closed state `owned_elsewhere`:
recovery reports it (fact `native_host_owned_elsewhere`, the summary `Another Ghostlight
installation owns the browser registration.`, one next step teaching `ghostlight install` from
the installation that should own the browsers) and never adopts it; deliberate install still
adopts it; recovery's owned repair refuses it exactly as it refuses foreign state. `doctor`
names the owning directory and marks it removed when that installation no longer exists, so a
deleted scratch tree becomes a visible row instead of silent breakage -- the dead-path
detection the owner asked for. Unit tests pin the classification, both diagnoses, adoption by
install, refusal by repair, the recovery ladder arms (single, mixed plural, all-elsewhere), and
the exact sentence and next step. An un-isolated live call from the scratch build against this
machine now returns the refusal with the machine's registration byte-identical. The upgrade
consequence is accepted and recorded in the amendment: every delivery channel re-registers on
upgrade, so cross-tree self-healing was redundant where it was safe and is what let a scratch
build hijack the machine where it was not.

This is the mutable implementation snapshot. Git history, the ADR index, and dated research carry
history; this file does not rewrite it. The 0.8 layer was retired on 2026-08-27 (ADR-0143).

## Native-host registration isolation (2026-08-30)

After the 1.3.0 publication, the owner found three orchestrators running at once. Two were
legitimate one-per-scope authorities (the dev tree and the installed 1.2.0; the runtime file
lives beside each executable, so each installation directory elects exactly one). The third ran
from the preflight scratch tree, and the machine's real native-host registration pointed there
too. Root cause: ADR-0149's silent repair adopts every stale Ghostlight-owned registration
toward whichever tree crosses the no-browser seam, and the CLI journey crossed it from the
preflight build with the real environment -- one `browser_tabs list` assertion rewrote all four
browser registrations into `.target-preflight-130` (reproduced on demand as the negative
control: a single un-isolated call rewrites the manifest within seconds). Chrome then spawned
the scratch connector, which demand-started the scratch authority. The machine was restored
(`target/release` owns the registration again, scratch processes stopped), and the recurrence
mechanism is three layers:

1. `GHOSTLIGHT_NATIVE_HOST_DIR` isolates the entire registration surface -- manifests and the
   Windows registry keys (below a Ghostlight-owned `Software\\Ghostlight\\Isolated` subkey no
   Chromium release reads) -- following the established env-seam pattern. Pinned by unit tests.
2. Every journey that spawns real executables (CLI, process, PowerShell) sets it, and the CLI
   journey snapshots the real registration before its first process and asserts it
   byte-identical after its last, so the journey itself fails red on any future leak.
3. The release preflight gained a machine registration snapshot stage before the journeys and a
   guard stage after: registration must be unchanged, and any ghostlight process left inside the
   isolated target is stopped and fails the gate. A pre-existing StrictMode crash in the
   `-SkipJourneys` restore path was fixed at the same seam.

Proven together: the un-isolated negative control leaks, the isolated journeys stay green with
the manifest untouched, and the preflight passes 18 stages including both new ones. Full gates
green (formatting, warnings-denied Clippy, 350 orchestrator tests, journeys, extension suite
unchanged). No product behavior changed outside the isolation seam; the published 1.3.0 bytes
are unaffected (the seam is additive and defaults to off).

## 1.3.0 published (2026-08-30)

Ghostlight 1.3.0 is public: GitHub release
[`v1.3.0`](https://github.com/sylin-org/ghostlight/releases/tag/v1.3.0) (20 files, re-downloaded
and hash-compared before publication), npm `ghostlight@1.3.0` (tarball SHA-256
`9c818de3569f5178b4b7d027a8c48f175c2ac955496148e709f1a6fdc1fbc576`), and the MCP Registry record
`org.sylin/ghostlight 1.3.0`, all from custody-verified candidate run
[33333813230](https://github.com/sylin-org/ghostlight/actions/runs/33333813230) at the frozen
revision `7b925625` (two verified local copies,
[candidate-custody-2026-08-30](testing/candidate-custody-2026-08-30.md)). `main` was
fast-forward promoted (`a12391e3..296b0209`). The public install smoke passed: `npx -y
ghostlight@1.3.0` downloaded and checksum-verified all three binaries from the public release,
and the real 1.3.0 orchestrator answered `doctor --json`, with the new Devin registry row live
on Windows. The Chrome adapter needs no action this line: the candidate's extension ZIP is
byte-identical to the approved store 1.1.0 revision (`ce59185e...`), whose staged review
remains the store path; the public listing serves adapter 1.0.0. Website fallback refreshed and
pushed; `check-public-surfaces.ps1 -Online` reports GitHub, npm, the Chrome update feed, the
MCP Registry, and the website in agreement (source 1.3.0, public 1.3.0, public adapter 1.0.0).
Scoop and WinGet metadata ride the `package-manager-metadata` artifact and remain owner
submissions as before. The line carries four features: one-step harness setup (ADR-0146), the
model-directed manual browser handoff (ADR-0147), Devin harness continuity (ADR-0148), and
choice-free recovery (ADR-0149).

## 1.3.0 service line prepared (2026-08-30)

The workspace version is 1.3.0 and the changelog carries the line's four features: one-step
harness setup (ADR-0146), Devin harness continuity (ADR-0148), the model-directed manual
browser handoff (ADR-0147), and choice-free recovery (ADR-0149). The Chrome adapter stays at
1.1.0, and compatibility.json covers the new line with both adapter 1.0.0 and 1.1.0. The
published-state surfaces (`server.json`, `docs/public-status.json`, README release language)
still say 1.2.0 and move only at publication, as the offline truth check requires. Next:
owner-gated candidate assembly, custody, and the publication sequence in
[RELEASE.md](RELEASE.md).

## Recovery never presents a browser choice (2026-08-30)

The owner rejected the recovery ambiguity refusal as bureaucracy rather than delight: a person
who cares which browser is used has one open already, or names one, so a choice is the one thing
Ghostlight must never present. [ADR-0149](adr/0149-recovery-never-presents-a-browser-choice.md)
retires it. With several installed browsers and none connected, both postures now return the
model-directed ask naming every browser whose native-host registration is current; stale
Ghostlight-owned registrations among them are repaired silently first through the same flight;
when nothing usable is registered the answer is the existing named, choice-free remedy; and if
two adapters arrive inside the same launch wait the workspace binds the first arrival under the
ordinary pinned-session rules. The `browser_recovery_ambiguous` failure and its person-facing
sentence are removed from the closed vocabularies, and a launch happens only for a unique
candidate under the `on_demand` posture.

The same change unblocks `dev` CI, which had gone red on both runner images: the CLI journey
pinned the manual sentence that only single-registered-browser machines produce, while CI
machines carry two unregistered browsers and honestly answered with the removed failure. The
journey now pins the closed language contract -- the ask or the named remedy, with exact facts
per reason -- instead of the local inventory. Gates on this Windows host: formatting,
warnings-denied workspace/all-target Clippy, every Rust workspace suite, the process, CLI,
PowerShell, policy-grammar, and workbench-surface journeys, and all 156 extension tests.

## Cursor and Devin harness continuity (2026-08-30)

[ADR-0148](adr/0148-windsurf-devin-harness-continuity.md) records the current Windsurf product
transition without breaking installed historical clients. The fixed registry retains `windsurf`
at `~/.codeium/windsurf/mcp_config.json` and adds `devin` at the effective platform config root's
`devin/mcp_config.json`, recognizing `devin`, `devin-desktop`, and `surf`. Both targets group under
one product card and keep independent detection and mutation evidence. The complete workbench
preview now pins the actual 23-target, 19-product roster; it also restores ZCode, which the fixture
had omitted despite claiming completeness.

Cursor 3.14.27 and the current official Windsurf distribution, Devin 3.8.20, are installed in this
Linux user's local application directories with command launchers and desktop entries. The live
Ghostlight installer registered its exact existing MCP connector in `~/.cursor/mcp.json` and
`~/.config/devin/mcp_config.json`. The deployed doctor reports both targets `installed`, and both
editors restart from their installed images. Devin's shipped CLI names `--add-mcp`, and its bundled
application resolves `mcp_config.json` from the current Devin config directory. Neither editor
started a new connector child before its visible onboarding or user-session boundary, so this pass
does not claim an authenticated model-path invocation. The exact evidence and boundary are in
[the Linux Cursor and Devin report](testing/linux-cursor-devin-2026-08-30.md).

Formatting, warnings-denied workspace/all-target Clippy, all Rust workspace tests, all 156
extension tests, changed-JavaScript syntax checks, shell syntax, and the workbench surface journey
pass. The orchestrator alone was deployed locally at SHA-256
`af7897063037e19b44492ffa07ae43def24d1c3ef9faa119e40203a5186e3e33`; both connectors stayed
running.

## Model-directed manual browser handoff (2026-08-30)

[ADR-0147](adr/0147-model-directed-manual-browser-handoff.md) keeps the existing governed
`browser.startup` setting and presents it as `Auto-open browser on request` with `On` and `Off`
choices. With it off, recovery launches nothing and returns every installed native browser whose
ordinary executable and current Ghostlight native-host registration are verified. Multiple manual
choices are useful rather than ambiguous; automatic startup keeps its unique-candidate rule.

The language-owned refusal now tells the MCP model to ask the user to open one of the named browser
windows with the Ghostlight extension installed, then repeat the call. Structured facts add the
plural `browsers` list and retain the singular `browser` field when exactly one choice exists. The
complete recovery instruction lives in the summary, so `next_steps` does not repeat it. After its
existing bounded extension-wake window, tab listing now crosses the same recovery seam instead of
constructing a generic refusal.

Formatting, warnings-denied workspace/all-target Clippy, all 422 Rust workspace tests, all 156
extension tests, changed-JavaScript syntax checks, policy grammar, the workbench surface journey,
the process journey, and the real CLI journey pass. The orchestrator alone was deployed locally at
SHA-256 `729b14c6695f70d1ea9f6311dd237ed9ce7a9cf5edb6132b8c4369796a05083a`; both connectors were
left running. A live `browser_tabs` call against that service returned failed/no-effect truth,
`browser_startup_manual`, `browsers: ["Chromium"]`, no duplicate next step, and the exact
model-directed summary.

## Aggregate detected-harness setup (2026-08-30)

[ADR-0146](adr/0146-aggregate-detected-harness-setup.md) adds the requested one-step workbench job:
`Set up everything` configures every detected `Available` target and updates every detected
Ghostlight-owned `Updatable` target through the existing serialized, backup-preserving writers. It
skips current and absent products, does not install third-party software, and leaves foreign or
malformed entries untouched while reporting their attention count. Independent environmental
failures are bounded and do not stop the remaining targets.

The implementation also closes a detector false positive exposed by the aggregate path: a generic
home, configuration, or roaming root is no longer product evidence for a config stored directly
under it. Focused Rust tests pin safe add, owned update, blocked-entry preservation, idempotence,
and generic-root detection. The workbench surface journey pins the real button-to-orchestrator
route. Formatting, all 421 Rust workspace tests, all 156 extension tests, JavaScript syntax checks,
the surface journey, and release-profile workspace/all-target warnings-denied Clippy pass. The
orchestrator was deployed locally at SHA-256
`a67209edadad9ed482f8119f5d5813a868c18ec2c844ef16571b61426f492359`.

The [live Linux evidence](testing/linux-harness-setup-everything-2026-08-30.md) exercised the actual
button under one-time person-approved KDE input control. Its visible outcome was 3 registrations
added, 13 owned registrations updated, and 1 blocked target preserved. The refreshed roster held
16 installed, 1 needs-attention, and 5 not-detected targets. Claude Code, OpenCode, Qwen Code, and
Kilo Code then reported the live connector connected through their own MCP health commands; Codex
and GitHub Copilot CLI read the exact registration through their native lists. The person then
completed ZCode login: its live chain reached `ZCode -> zcode-host -> zcode-cli ->
ghostlight-mcp-connector`, and a visible read-only task invoked `policy_explain` and rendered
Ghostlight's successful authority report. Kiro login and goose provider configuration remain
explicit boundaries, and this pass does not claim a model path for the other clients without a
non-interactive health command.

## 1.2.0 published (2026-08-30)

Ghostlight 1.2.0 is public: GitHub release
[`v1.2.0`](https://github.com/sylin-org/ghostlight/releases/tag/v1.2.0) (20 files, hash-compared
before publication), npm `ghostlight@1.2.0`, and the MCP Registry record
`org.sylin/ghostlight 1.2.0`, all from custody-verified candidate run
[33284810442](https://github.com/sylin-org/ghostlight/actions/runs/33284810442) at the frozen
revision `765df478` (two local custody copies). `main` was fast-forward promoted and the public
install smoke passed: `npx -y ghostlight@1.2.0` downloaded and checksum-verified all three
binaries and the real 1.2.0 orchestrator answered `doctor --json` with the new
`process_diagnostics` state. The Chrome Web Store adapter moved to 1.1.0 on 2026-08-30: the owner refreshed the expired
refresh token, and `publish-extension.ps1` uploaded the exact candidate ZIP
(SHA-256 `ce59185e...3bb5ce`) and submitted it STAGED_PUBLISH -- PENDING_REVIEW at submission;
the public listing serves 1.1.0 automatically when review approves.
Scoop and WinGet metadata ride the `package-manager-metadata` artifact and remain owner
submissions as before. Website fallback refreshed and pushed; `check-public-surfaces -Online`
reports GitHub, npm, the Chrome update feed, the MCP Registry, and the website in agreement
(source 1.2.0, public 1.2.0, public adapter 1.0.0).

## Process diagnostics implemented and deployed locally (2026-08-29)

Agents report errors that no surface records: the demand-started orchestrator's output is
nulled at spawn, the connectors log one line per disconnect streak, and the extension is
silent by design. [ADR-0145](adr/0145-shared-process-diagnostics-log.md) (amended four times
in place, pre-implementation, under the owner's fewest-moving-parts bar) accepts a shared
local diagnostics directory under layered activation -- `GHOSTLIGHT_DIAGNOSTICS_DIR`, or a
presence-only `diagnostics.on` marker beside the runtime file that even the Chrome-spawned
browser connector can see without environment propagation -- where all three executables
append bounded, content-free operational JSONL from process birth. The marker is applied live
by an OS watch with a 2-second safety-net tick, whichever fires first, so toggles need no
restart; surfaces actuate the same marker: the person's hand, `ghostlight diagnostics
on|off`, the extension popup, and the workbench Status card, which also opens the folder.
`show` merges everything into one chronological timeline; `doctor` names the layer, folder,
and size. Records carry no page content or payloads, so the folder is shareable as-is.

The [process-diagnostics batch](tasks/process-diagnostics/LEDGER.md) executed D1-D10 the same
day: the bridge sink (watch plus tick, closed event vocabulary, automatic retention, schema
pinned by test), orchestrator hub and emissions, both connectors, the CLI, the doctor row,
the extended process journey, the popup toggle, the workbench card, and the contract
reconciliation. The known-flaky bridge test was fixed at its root (wait on observable
effects, not resolution). The batch's local work is complete; the extension bytes ride the
next store submission.

## Public distribution arc opened (2026-08-29)

The owner directed a public distribution arc after distribution research (the
[batch ledger](tasks/public-distribution/LEDGER.md) carries the sources and the live proof).
[ADR-0144](adr/0144-public-plugin-distribution.md) adds the plugin as a fifth distribution
member with its own version space: twin manifests at `packaging/plugin/ghostlight/`
(Claude schema canonical, ZCode native twin), one-address marketplace catalogs at the
repository root for both ecosystems, and the bundled `control-browser` skill written against
the 1.0 language contract. The plugin's MCP server is `npx -y ghostlight`, the npm launcher's
zero-argument checksum-verified handoff, proven live the same day with a real MCP initialize
and the full 24-tool catalog. The repository-integrity gate now pins twin-manifest and catalog
agreement. On the same day the owner authorized the send: the Z.ai feedback request for a
public plugin intake process is live as `zai-org/feedback#419`, the `zai-coding-plugins`
proposal is live as `zai-org/zai-coding-plugins#30` from the org fork, the repository's
one-address catalogs are promoted to `main`, and the Anthropic community-marketplace
submission is confirmed received through the Console form ("Plugin submitted for review").
All three public submissions of the distribution arc are live; see the
[batch ledger](tasks/public-distribution/LEDGER.md) and `submissions.md` for the exact
outcomes.

## 1.0 is published (2026-08-26)

The owner directed publication on 2026-08-26 ("get the binaries out and guarantee publication, and
then we flip the extension to 1.0"). Everything shipped from candidate build run
[33020313866](https://github.com/sylin-org/ghostlight/actions/runs/33020313866) at revision
`b2c27993a223c220f8828736b125676ae6f9d027`, custody-verified in two local copies
([candidate-custody-2026-08-26](testing/candidate-custody-2026-08-26.md)):

- GitHub release [`v1.0.0`](https://github.com/sylin-org/ghostlight/releases/tag/v1.0.0), tag at
  the candidate revision, all 20 files, draft re-downloaded and hash-compared before publication.
- npm `ghostlight@1.0.0` (tarball SHA-256 `ca43a866f30e839d608596835c9120d7f35c54c2486de4f8859f56a2e176e49b`).
- Chrome Web Store adapter 1.0.0, published from the approved staged revision (extension ZIP
  SHA-256 `3570494faf580a2286d9f7a5f1cbb6f657864ee369b0f70b944b0c927e64770c`, byte-identical to
  the release bundle because the packager now pins its cross-OS fields); the public listing and
  the CRX feed both observed serving 1.0.0.
- MCP Registry record `org.sylin/ghostlight` 1.0.0.
- `main` promoted by fast-forward (`0116feca..4ca4e6a1`); `v1.0.0` is tagged.
- `docs/public-status.json`, README release language, `server.json`, and the changelog date
  recorded the observed public state; `check-public-surfaces.ps1 -Online` reports GitHub, npm,
  the Chrome update feed, the MCP Registry, and the website in agreement at 1.0.0.
- Public-channel install smoke on Windows in an isolated profile: `npx -y ghostlight@1.0.0`
  downloaded and checksum-verified all three binaries from the public release and the real 1.0.0
  orchestrator answered `doctor --json`.
- The website shipped its 1.0 story the same day: `llms-install.md` now speaks of 1.0 as the
  published release (`97508008`), and website commit `f45d3b7` applied the drafted copy refresh --
  v1.0.0 badge, the orchestrator chain in the local-by-construction diagram, four personas
  including scripts and organization, the tab-reuse "minute" step and fifth recipe, and the
  published "Where it stands" block -- with both fallback snapshots refreshed and the checker
  pins moved to the new truth. The live site and the live install guide were observed serving
  the 1.0 story on 2026-08-26
  ([website copy refresh](release/website-copy-refresh-2026-08-25.md)).

Publication went ahead with the G4 (Ubuntu GNOME Wayland), G5 (clean Windows), G6 (npm channel
upgrade), G7 (public harnesses), and G8 (reference-experience closure) lanes open at the owner's
direction; their evidence remains owed and the GO decision is recorded in the
[release checklist](RELEASE-CHECKLIST.md). Scoop and WinGet metadata are prepared from the
candidate under the machine-local `.target-pkg-metadata` directory; their external bucket
submissions are owner actions.

## Publication-day engineering record

Three release-pipeline defects were found and fixed at their seams on publication day, and the
  determinism repair that matters most is recorded here for the next release:

- The 0.8 artifact-relationship ledgers had drifted across the ADR-0140 relicensing (twelve
  files); ordinary CI had been red since. Regenerated at `89bed6c6`.
- A broken ADR index link (ADR-0137 row still pointing at ADR-0106's pre-rename filename) fixed
  at `ded44e2d`.
- **The extension ZIP was not cross-OS deterministic.** Two independent .NET/PowerShell behaviors
  made the Linux CI archive differ from the Windows-built, store-approved bytes: `ConvertTo-Json`
  writes the platform newline into the rewritten `manifest.json`, and .NET's `ZipArchive` stamps
  the central directory's host-system marker (0 on Windows, 3 on Unix) and Unix mode bits from
  the running platform. 33 records, 99 differing bytes, zero differing content. The packager now
  pins the manifest serialization to CRLF and rewrites both central-directory fields to the
  Windows shape after archiving (`bd3bffe4`, `9ee05666`), so every host reproduces the approved
  store revision byte for byte. Durable lesson recorded in [MEMORY.md](MEMORY.md).

## ZCode harness integration (2026-08-27)

A live contradiction was reported: the workbench showed Zed READY while ZCode showed "The MCP
process failed to start." Both were true. Zed's registration was correct and was verified with a
live MCP initialize handshake, and ZCode, which the harness roster has never covered, held a
hand-written entry pointing at the orchestrator executable, which has no MCP stdio mode. The
user-side config was corrected on the spot to the sibling MCP connector.

[ADR-0141](adr/0141-zcode-harness-integration.md) then added `zcode` to the fixed harness
registry: config `~/.zcode/cli/config.json`, a `ZCode` dialect for its `mcp.servers` document
with the observed string-command shape, no pinned download destination, and neutral monogram
artwork. Gates: fmt, warnings-denied workspace Clippy, the full workspace suite, and a live
debug-build `doctor` check of the new row on this machine. The deployed release build predates
the change, so the workbench card appears after the next release deployment through
`scripts/dev-loop.ps1`.

## 1.1.0 published (2026-08-27)

Publication used the custody-verified candidate at revision
`655e2078ef1631e1d64e50c777c0ac12398a1196`
([candidate-custody-2026-08-27](testing/candidate-custody-2026-08-27.md)), which took three runs
to reach: the version bump surfaced every hand-stamped 1.0.0 copy asserting against live output
(CLI banner pin, four Debian smoke literals, the npm identity guard correctly refusing the
missed packaging stamps), and then the owner caught the deeper disease -- the bump had
mechanically restamped the unmodified Chrome adapter. Per-member release version spaces are
ratified in [ADR-0142](adr/0142-per-member-release-version-spaces.md): the adapter stayed 1.0.0,
the two equality guards that forced the restamping were removed, and the assembler stopped
restamping the ZIP's name.

The publication itself:

- GitHub release [`v1.1.0`](https://github.com/sylin-org/ghostlight/releases/tag/v1.1.0), tagged
  at the candidate revision: the draft was created from the verified custody copy after per-file
  provenance checks, re-downloaded and hash-compared, then published (20 files).
- npm `ghostlight@1.1.0` (tarball SHA-256 `b4dee8b0...`, pinned against the candidate manifest).
- MCP Registry record `org.sylin/ghostlight` 1.1.0.
- The Chrome Web Store adapter stays 1.0.0 by design; no store action.
- The website's public-status fallback was refreshed and pushed (website commit `759e848`), and
  `check-public-surfaces.ps1 -Online` reports GitHub, npm, the Chrome update feed, the official
  MCP Registry, and the website in agreement at 1.1.0 / adapter 1.0.0.

The same day the owner directed the retirement of the 0.8 layer
([ADR-0143](adr/0143-retire-the-0-8-layer.md)): `docs/0.8/`, the harvest and recovery machinery
and its CI gates, the 0.8-named business, design, research, and task records, and the pre-1.0
compatibility rows are removed from the working tree. Git history and the `archive/0.9-pre-1.0`
tag preserve every byte; ADRs and dated records keep their mentions as history; the runtime
pre-1.0 supervisor migration stays because real upgrades depend on it.

## In flight: 1.0 release pipeline

Engineering batches are closed: the foundry sprint, the interface-truth polish, and the
nine-task handle-continuity batch are landed, gated, and proven live on Windows plus
verified on CachyOS (see tasks/demo-press-key-diagnosis/ and tasks/handle-continuity/).
The 1.0.0 extension was rebuilt deterministically from foundry-sprint source, resubmitted
to the Chrome Web Store (PENDING_REVIEW, STAGED_PUBLISH, sha256 f7b9a6ad...), and that
review replaced the stale one -- see
[extension-store-submission-2026-08-24](testing/extension-store-submission-2026-08-24.md).
That review was itself canceled and replaced by the custody candidate on 2026-08-25; the
current submission is described below.
Release tooling now exists: scripts/release-preflight.ps1 (one-command G1 gates plus
evidence skeleton), declare-freeze/assert-freeze (G0), verify-custody.ps1 (G2).

G0 and G1 closed on both hosts at frozen revision e7d8986b (Windows preflight, CachyOS verification, and runner-repair records under docs/testing/). The owner then reported real tab and group spam, directed an architectural fix over release ceremony, and dropped the freeze itself: we publish when we are done, and the freeze machinery now only pins whichever revision becomes the candidate. ADR-0137 landed the fix: duplicate same-title groups merge into the canonical group (self-healing existing pollution), and plain opens adopt the nearest unbound same-host tab -- new_tab and reuse never still create fresh -- with the summary saying Reused the example.com tab. when it happens. Found during live verification: workspace release never tells the extension, so reaped tabs stay bound in topology and the reuse ladder cannot adopt them -- the release path must notify the extension to forget the released tab ids (then reuse works end to end). That seam landed, the unpacked extension was reloaded, and a green foundry rerun demonstrated reuse live. The G2 candidate was built by release workflow run 32846030216 at revision 994b6c85 (product bytes identical to ADR-0137 commit 8779e11b; only CI tooling differs), and custody is held in two verified local copies -- see [candidate-custody-2026-08-25](testing/candidate-custody-2026-08-25.md). On 2026-08-25, with explicit owner authorization, the stale f7b9a6ad store review was canceled through publishers.items.cancelSubmission and replaced by the custody ZIP (sha256 9ae88e67...) submitted STAGED_PUBLISH; it is PENDING_REVIEW and the public listing still serves 0.8.0 -- see [extension-store-resubmission-2026-08-25](testing/extension-store-resubmission-2026-08-25.md). Next: the owner-run environment lanes -- G4 Ubuntu GNOME Wayland, G5 clean Windows, G7 public harnesses -- which install the reviewed adapter from the store once review completes, then the owner-authorization boundaries G8 through G10; lane runbooks and gate drafts are prepared in [gates-g4-g10-preparation-2026-08-25](release/gates-g4-g10-preparation-2026-08-25.md). Live authority swaps go through
scripts/dev-loop.ps1 only.

## Windows release trust (2026-09-04)

Windows release artifacts remain unsigned. Checksums and keyless GitHub build-provenance
attestations are the current trust mechanisms; the release workflow has no certificate-signing
integration.

## Frame-transparent semantic layer (ADR-0138)

The cross-origin frame deferral from ADR-0078 D8 is lifted. Content scripts run in every
frame (`all_frames: true`); locators are frame-scoped at minting and stay opaque beyond the
extension, so the bridge, orchestrator, tool schemas, and every Rust crate are untouched.
Document-wide reads (`inspect`, `find`, `query_semantic`, visible-mode reads, text waits)
aggregate across frames in stable order; `fill_form` groups fields by owning frame and
proves a contained submit before clicking. Pointer geometry over embedded targets composes
through parent-side embed boxes matched by embed URL -- no debugger attachment, and one
mechanism for same-origin and cross-origin frames at any depth. Perpetual visuals stay
top-frame only; target-anchored effects render inside the owning frame and are suppressed
when the target has no live box. The public stage
`https://sylin.org/ghostlight/demo/iframe/` (website commit `5693092`) hosts the practice
journey. Proven live on the daily-Chrome authority with the reloaded unpacked extension:
inspect listed all nine embedded controls, one `fill_form` call completed and submitted the
frame's form, the frame-rendered completion sentence satisfied a text wait, and hover landed
on the "Project Name*" field inside a genuine third-party cross-origin iframe --
the exact interaction that forced coordinate guessing before. Extension suite is 151 tests.
Because extension bytes changed, the pending store review is stale against this source; the
owner replaces it when ready (same procedure as the 2026-08-24 replacement).

## Shadow-complete interaction (ADR-0139)

Open shadow trees were already observed and acted on; this closes the two light-DOM-only
seams a model actually hits mid-task. Focused-control discovery walks the
`shadowRoot.activeElement` chain, so focused typing and clearing reach the real field
inside a web component instead of describing its host, and point-action subjects (with the
`[inert]` check) cross the shadow boundary through `getRootNode().host`. Closed shadow
roots stay closed: never pierced, never patched, honestly absent from inspection. No wire,
tool, or Rust changes. The public stage `https://sylin.org/ghostlight/demo/shadow/
(website commit `8af8eb4`) hosts an open-root component form beside a sealed closed-root
widget whose contents inspection truthfully does not list. Extension suite is 153 tests.
The stage's fill-then-focused-type journey exposed a latent orchestrator panic (from the
R2 precision-input restoration): focused typing discarded the describe step's observation
and then expected a fallback subject that did not exist, so a fully successful type landed
its effect and never settled. Fixed at the seam -- the described control is the receipt's
subject -- pinned by a test that panics the old code, deployed by exact-path swap, and
proven live (`Typed 5 characters into the "Ledger project" text field.`). With the owner's
authorization the stale store review was canceled and replaced the same day by the
current-source package (sha256 `3570494f...`, deterministic across two builds, permission
surface unchanged) -- see
[extension-store-resubmission-2026-08-25-frames-shadow](testing/extension-store-resubmission-2026-08-25-frames-shadow.md);
`scripts/publish-extension.ps1` gained first-class Cancel and Status actions so the remedy
no longer needs inline authenticated calls. The review is staged and publishes nothing.

## Published capability restoration

The owner accepted [ADR-0133](adr/0133-behavioral-capability-restoration.md) after a direct
behavioral comparison with the exact published 0.8 source. The checked recovery inventory had
correctly preserved and dispositioned historical evidence, but its behavior-group coverage did
not prove that every published browser job remained reachable through the current 1.0 catalog.

The accepted [capability-restoration batch](tasks/capability-restoration/) restores the genuine
contractions through current seams: REPL-grade execution; modified, repeated, focused, timed, and
view-point input; unambiguous semantic action and form loops; article and hierarchical document
reads with bounded diffs; inline and captured-image upload; guarded beforeunload navigation; and
one result-aware `browser_flow`. Old names, narration prose, destructive diagnostics, client plan
mutation, and UDP syslog do not return. The [ledger](tasks/capability-restoration/LEDGER.md) is the
authority on progress. R1 is complete: every browser command now declares a minimum capability
revision at one bridge seam, an old adapter refuses `browser_execute` before dispatch with the typed
`CapabilityVersion` error, and page scripts evaluate with REPL semantics (`replMode`, promise
waiting, user gesture, by-value return, and exactly one diagnosed bare-return retry). Parse failures
now refuse decisively without inventing an effect; runtime failures remain unknown. R2 is complete:
modified and triple clicks ride POINTER_INPUT revision 2, stroke sequences repeat with cancellation
observed between repetitions, focused typing describes the control and keeps credential handoff,
duration waits run executor-side, and coordinate wheel scrolling reuses the governed view transform.
R3 is complete: one typed semantic selector resolves through a single revision-gated adapter query
with zero-or-many failing without effect, selectors work as alternatives on click, type_text, and
per-field fill, form fields accept boolean and finite-number values rendered to canonical wire
strings so older adapters keep working, contained-form submit is verified before clicking, and
optional postconditions report an applied effect truthfully when the expectation fails
(Status::Failed, Effect::Applied, never repeat-safe). R4 is complete: article-first reading with a
visible-text mode and a 50,000-character ceiling falls back to visible text, document-scope inspect
returns a bounded structure-only tree (no editable values, hidden content excluded, shadow-aware)
with a generation-bound snapshot_ handle superseded per tab, and a current prior snapshot yields a
bounded structural diff. R5 is complete: uploads accept exactly one of absolute paths, bounded
inline base64 files, or one captured image_ handle; inline bytes decode only after authorization
and credential preflight; each capture holds one volatile reuse asset beside its view, refused
above the upload ceiling and erased by supersession or tab closure; captured images attach to
file inputs through target or selector and drop at view points through a revision-gated command.
R6 is complete: browser_flow joins the catalog as its twenty-third tool, composing one to twenty
uniquely named steps whose arguments may reference earlier canonical result envelopes through
bounded JSON Pointers; references resolve before the ordinary child decoder runs again, children
authorize normally under the immutable invocation ceiling, dry run dispatches nothing, stop or
continue governs failures, per-step envelopes are captured under a bounded budget, and aggregates
report applied, partial, or unknown effects truthfully. R7 is complete: browser_navigate accepts
beforeunload:discard, which accepts only that navigation's own beforeunload prompt through a
revision-gated mechanism and then follows the ordinary commit and landing-governance path; the
default still stops and reports a blocking prompt without accepting it, and unrelated dialogs
remain browser_dialog's domain. R8 is complete: a checked behavioral matrix in repository
integrity maps all 25 published behaviors to evidence or explicit supersession, the extended
process journey drives every restored family through the real executable graph, and live lanes on
the development-swapped daily-Chrome authority proved REPL execution, semantic selection with its
none-chosen refusal, article reading, tree snapshot with a real-mutation diff, coordinate wheel,
captured-image drop, guarded discard, and a referenced flow -- finding and fixing two genuine
defects (inspect_tree wire encoding; flow stop continuing past a failed step). R9 prepares the
replacement extension package. R9 is complete: the deterministic packager produced a
byte-identical candidate across two runs (SHA-256 97bd4816...49a6, 89,441 bytes, 32 entries,
v1.0.0 MV3, development key stripped) from source revision 3c820a98, with the permission diff
against published 0.8 documented in the batch ledger. The pending Store review is stale against
these bytes; replacing the draft is an explicit owner action and was not performed.

This batch is now on the 1.0 release path. Its extension changes will supersede the already stale
pending Chrome Store review. R9 may build and verify a replacement ZIP, but this batch carries no
authority to upload, resubmit, publish, push, tag, or otherwise mutate anything external.

## Live full-catalog integration test (post-R9)

After R9 closed, a live integration test exercised the catalog tool-by-tool on the daily-Chrome
dev authority. Most families were already proven during R8's live lanes; this pass added
navigate/open, find, inspect controls, fill_form, click, type_text, hover, drag, window zoom and
resize, full-page capture, sequence, press_key driving a real link navigation, history back,
forward, and reload with document generations advancing, region capture chained from a governed
view, and the dialog journey end to end. The pass closed with diagnose, a record start/save
returning a real GIF replay, and the truthful preserve-tabs close refusal. Every catalog tool
has now been exercised live on the daily-Chrome authority.

The pass found and fixed three guidance defects at the root, each committed separately:

- `dcabf582` -- invalid-input results now carry the specific validation expectation as their
  next step instead of the circular "Correct the call using the advertised tool schema.", and
  the screenshot-region validator teaches the remedy (take a screenshot, then pass its view).
- `5a56fc2e` -- guidance renders the bare expectation without the "invalid input:" diagnostic
  prefix, which stays in the facts detail where it belongs.
- `d5a8c5de` -- dialog handling no longer trusts event-only tracking. A dialog that opens while
  the debugger is detached was invisible forever, and the orchestrator pre-refused handling on
  that blind inspect. `browser_dialog` now attempts `Page.handleJavaScriptDialog` directly
  (CDP's "No dialog is showing" error is the one authoritative absent probe) and reports a new
  typed `dialog_absent` outcome; failures with unknown effect name the open-dialog hypothesis
  and point at `browser_dialog`, then at observing the page rather than replaying the call.

## Language delight pass

The [language-delight batch](tasks/language-delight/) completed on 2026-08-24 through `D1-D4`.
Every validation message, tool description, and refusal or result guidance sentence was revoiced
to teach: validators name the allowed set and received value; descriptions say when to reach for
a tool and what to do instead; refusals lead with the recovery action, including new next steps
for deadline, receipt, upload, capture, and the three previously silent workspace reasons. New
pins hold the teaching sentences in place. D4 deployed the release orchestrator by exact-path
swap, proved the taught failures live (region without a view, stale view click, unsatisfied wait,
preserve-tabs interlock), served every pinned phrase to a real MCP client over stdio, and
reconciled eleven drifted sections of [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md) with the current
schemas, including adding the missing `browser_flow` catalog section. The ledger carries two D4
deviations: one transient WrongProfile refusal during the swap, and two disposable Example Domain
tabs left open by the preserve-tabs interlock for direct closure.


## Where the branches stand

Distances below are measured against the local remote-tracking refs, which are only as fresh as the
last fetch.

The repository carries exactly two branches, `main` and `dev`. They diverged between 2026-08-13 and
2026-08-17, when `main` took one commit `dev` did not: `0116feca`, which paused dependency updates
while `main` still carries 0.8.

The owner resolved that on 2026-08-17 by merging `main` into `dev` with the `ours` strategy.
`main`'s history is now contained in `dev` and none of its 0.8-line content was applied, so `dev`
keeps its own Dependabot configuration rather than inheriting the 0.8 pause. The merge left the tree
byte-identical. `main` is an ancestor of `dev` again, so the G10 promotion is a fast-forward.

Check this rather than trusting the paragraph: `git merge-base --is-ancestor main dev` exits zero
only while the topology is linear.

## One invoked desktop authority

[ADR-0127](adr/0127-one-invoked-desktop-authority.md) removes both `ghostlight service` and
`ghostlight --headless`. MCP connector, browser connector, CLI, and direct-user starts now converge
on the same no-argument desktop authority. Desktop startup or event-loop failure ends the process
instead of leaving an invisible authority. Tray creation remains capability-aware: supported
desktop sessions with trays must show the Ghostlight icon, while sessions without a tray retain the
Applications entry and `ghostlight open` as explicit interaction routes.

Process and CLI journeys now invoke the production launch with no application arguments. Linux CI
and Debian-package smokes provide a virtual display rather than using a product-only test mode.
Visible KDE, GNOME, and Windows evidence remains responsible for proving the real tray and window.

Local implementation evidence on CachyOS KDE Wayland, 2026-08-16:

- Formatting, warnings-denied workspace Clippy, 351 Rust tests, 116 extension tests, 10 npm
  launcher tests, four MCPB launcher tests, the workbench surface journey, shell syntax, and changed
  JavaScript syntax all passed. The real process, CLI, and checksum-verified portable-PowerShell
  journeys passed against freshly rebuilt debug binaries.
- The release binary was installed at `/home/test/.ghostlight/bin/v1.0.0/ghostlight` with SHA-256
  `369822d0489b784dc20ae66f72734cff50f4ef2e0b7b8c63502094c6a585660d`. Both removed command
  forms exited 1 before publishing runtime discovery. The live browser connector then demand-started
  that exact installed executable with no arguments and reconnected without being restarted.
- KDE's status-notifier watcher registered the new process's `ghostlight` item as `Active`, backed
  by the rendered Ghostlight PNG. Its exported menu contains Open Ghostlight, Pause browser work,
  Resume browser work, and Quit Ghostlight. `ghostlight open` activated the running authority.
- A real unpacked-extension journey through ordinary Chromium navigated to example.com, read the
  page, and returned a real JPEG screenshot. Its final close assertion remained blocked by the
  extension's user-owned preserve-tabs interlock, with reason `browser_local_interlock`; that is an
  expected safety refusal, not a green close result, and the evidence tab remains visible.
- An isolated launch with no X11 display, no Wayland runtime, and no session bus exited 1 and
  removed runtime discovery. A session missing only a display variable was not accepted as a
  negative control because live KDE still supplied its D-Bus tray interaction route.

## Reference experience epic

The owner-approved product direction is the staged
[reference-experience task batch](tasks/reference-experience/). It was authored on 2026-08-15,
reworked on 2026-08-16, and is executing. The
[ledger](tasks/reference-experience/LEDGER.md) is the authority on progress and carries sixteen
numbered deviations; [ADR-0126](adr/0126-reference-experience-contract.md) carries the decisions.

The aim is that Ghostlight behaves as one product across every machine a person uses: the same
words, the same controls, and the same truth, shaped to the desktop they are on. Ghostlight stays at
the mechanism boundary; inferred user-task meaning does not belong here.

S1 through S6 are complete on the Windows development host:

- **S1** ratified the contract as ADR-0126. Its consequential decision is that a pause refuses
  rather than holding the caller: a human-scale pause outlives an MCP request timeout, and a
  suspended operation would compete with the ADR-0113 deadline over one operation's fate. The
  rejected alternative is recorded so it is not re-proposed.
- **S2** made the extension distinguish an absent native host from an unreachable service. A browser
  profile syncs the extension to a second computer and the native host does not travel with it, so
  both surfaces now say Ghostlight is not installed there and offer one route back, with a bundled
  page for when the walkthrough host is unreachable.
- **S3** gave the product one owner for what it says about the machine it runs on: a closed table
  with a row per platform and desktop, including WSL, consumed by both install and `doctor`.
- **S4** made the command line a first-class surface: `doctor --json`, an owned
  `~/.local/bin/ghostlight`, manual pages for the three executables, completions for bash, zsh, and
  fish, and plain words in place of Rust identifiers for every reported state.
- **S5** replaced the human-control refusals with the two pinned directives, and separated a policy
  attention hold from a person's pause in the extension popup.
- **S6** moved the front door's readiness answer out of JavaScript and into
  `language/readiness.rs`. The landing destination is now called At a glance, and no sixth
  destination was added.

S7 is complete in three substeps. S7a registered `browser.startup` with the closed values
`on_demand` and `manual`, per-platform defaults, monotonic organization ceiling behavior, and a
closed workbench choice. S7b added the decision layer at the executor's one no-browser seam:
deterministic installation diagnosis, Snap and Flatpak refusal, ambiguity reporting, per-scope
single flight, exact closed failures, and a useful no-effect result in manual mode. It launches
nothing. S7c added the bounded physical attempt: safe repair only for stale Ghostlight-owned
registration, direct launch of the chosen installed executable with no arguments, Linux graphical
session proof through the ADR-0082 seam, and a bounded wait for the inbound adapter. Cancellation
leaves no recovery flight behind. A corrective Windows-source pass now keeps all-four-family
pre-registration but selects recovery candidates only from verified ordinary executables. Its
per-generation flight state keeps cancellation and deadlines local to each caller, hands unfinished
phases to a live joiner, and rechecks before repair and launch. The Linux default remains manual; a
real no-browser call returned its exact useful outcome with no effect. S8 is split between the owed
Ubuntu GNOME Wayland lifecycle and a clean installed-Windows live-launch lane.

The corrective Windows source host also replaced its stale live orchestrator through the exact-path
development swap. The deployed release SHA-256 is
`CC20AF4A1E6EBF3C120E9CBB30954B7F4B4103C0332FC7AE4E625A9B014EDF7B`. Existing connectors
converged on one new authority. A real attached Chrome adapter completed open, read, and screenshot;
the final close was correctly blocked by the person's `preserve-tabs` interlock, and the test tab was
then closed directly without changing that setting. In a separate isolated authority, the real
Windows inventory returned `browser_recovery_ambiguous` with exactly Google Chrome and Microsoft
Edge, no effect, and no launch. All four native-host registrations were missing and were left
unchanged. This is useful source-host integration evidence, not the clean installed-Windows S8 lane.

A later orchestrator-only workbench swap deployed the compact status-sorted MCP integrations
surface at SHA-256
`D2D61F74AECAF82FB0935FA5EB4C8A75D9A8110884DACE20C2A4813265B46445`. The exact live
`Ghostlight` / `Tauri Window` was visible and responsive. Its 21 concrete registry rows rendered as
18 product cards: 6 Ready, 6 Available, 1 Needs Attention, and 5 Not Detected. The compact roster is
one flat grid in that status order, with names alphabetical inside each status. Every card pairs its
visible status label with its semantic color; there are no status headings or counts. The
already-running browser connector plus all nine observed MCP connectors survived the authority
replacement and reconnected. The old authority PID 34920 was gone and the replacement ran only
from the expected release path as PID 33708. This is live Windows source-host workbench evidence,
not the clean installed-Windows S8 lane.

S8 is `BLOCKED`, not complete. The required Ubuntu GNOME Wayland and clean installed-Windows
candidate environments have not run. A Windows source host proved the corrected deterministic
mechanisms and process boundary, but that is not a substitute for the installed-product journey.
The prompt forbids substituting source or complementary-desktop evidence for the release-blocking
Ubuntu GNOME Wayland L1-L9 or installed-Windows journeys. The
[dated evaluation](testing/reference-experience-evaluation-2026-08-16.md) inventories the passing
automated and KDE evidence, dispositions every ADR-0126 acceptance measure at its current level,
and lists the exact desktop, migration, accessibility, and owner decisions still required. It also
sorts, under "What is decidable without a new machine", the rows that never needed a real desktop
at all. Public first-use feedback is no longer among the required rows; G0 removed it from 1.0 on
2026-08-17.

**1.0 waits for this epic.** The owner decided on 2026-08-16 that the release does not go out ahead
of it, so the epic's completion is a release gate. Practically, that means S8's evidence is release
evidence, and a stage that cannot honestly close blocks a release rather than deferring a feature.
An accurate `BLOCKED` is therefore worth more than an optimistic pass.

The owner closed S8's aggregate-readiness decision on 2026-08-17. `ghostlight doctor` now reads the
exact orchestrator-owned `ReadinessSummary` from an already-running authority through one
authenticated read-only opening. It never demand-starts, reveals the workbench, admits a channel,
opens a session, or writes audit. The [dated evidence](testing/doctor-readiness-parity-2026-08-17.md)
records wire, no-mutation, absent-service, six-state language, and real text/JSON CLI process
proofs. Installed Windows and Ubuntu observation remains part of S8.

Verification boundary: every commit passed formatting, warnings-denied workspace Clippy, the full
workspace test suite, the extension suite, and the journeys its change touched. The Linux lane has
proved the owned command entry, Debian and per-user manual pages, packaged and per-user shell
completion, the extension's second-machine state, and the KDE plus unknown environment rows. GNOME
is not installed on that host, so the Ubuntu GNOME Wayland lifecycle remains S8 work.

This epic adds no network behavior of any kind. ADR-0028 Decision 9 stands, and the epic's NEVER
list has no exception for it.

## RAWX and managed-policy restoration

The 1.0 policy product is restored on current orchestrator seams through `44f84eae`.

- RAWX is an independent set, not a rank. The exact action directory drives enforcement, audit,
  catalog projection, explanation, simulation, and tests. Sequence steps are admitted separately.
- Strict schema-3 ordered grants, host polarity and specificity, observe/enforce, layer
  intersection, sacred destinations, stable denial ids, grant attribution, and managed publish
  sequence are live.
- Policy-aware catalog projection emits the standard MCP tool-list change notification while
  all-open remains the exact 24-tool catalog.
- Local and managed policy reload atomically for future snapshots. Bad replacements keep last
  valid; configured cold start without authority fails closed.
- Customer-hosted file or HTTPS delivery uses signed monotonic bundles, required Ed25519, optional
  mandatory-both ML-DSA-65, verified cache, ETag, CA pin, bearer option, bounded retry, rollback
  refusal, and local signing/publication commands. No bootstrap performs no network work.
- The workbench Policy Passport shows organization, rationale, contacts, verification, sequence,
  freshness, source class, and verification time without source addresses, credentials, keys, or
  rules.
- The persistent workbench lamp band now distinguishes all-open, applied policy, retained-policy
  reload warning, and fail-closed policy states, with one click through to Status.
- Repeated enforced denials enter the existing workspace-local attention and runtime-control path.
  The full workspace gate passes with 197 orchestrator library tests, 2 orchestrator binary tests,
  30 bridge tests, 4 MCP connector tests, warnings denied, and 101 extension tests.

## The policy a person can read, and author

[ADR-0122](adr/0122-readable-policy-destination-and-authored-user-layer.md) is implemented, from
research input [24-policy-surface-user-delight-2026-08.md](research/24-policy-surface-user-delight-2026-08.md).

- Policy is the window's fourth destination. The state chip moved into the tab row between Status
  and About and opens it; Status keeps diagnostics, the session control, and notifications.
- One orchestrator-owned projection compiles the answer: a situation sentence, one line per
  capability in plain verbs with the layer that decided it, the rules behind those lines, the
  permanent ceilings in every situation including all-open, and the exact document and path for
  every layer. The surface renders it and computes no policy words of its own.
- Schema 3 gained an optional additive `organization` block (name, statement, HTTPS url, contacts).
  A signed bundle's presentation block still wins on conflict. Manifests stay typo-closed, so a
  document using the block is refused by older builds.
- The workbench authors one user policy through two bounded commands over a product-owned path in
  the per-user state directory. `GHOSTLIGHT_POLICY_FILE` still wins when set, and that file is
  shown read-only. Applying validates before replacing and writes atomically; no window action can
  leave the product failing closed.
- `policy.user.enabled` is registered. It gates authoring only, never enforcement, and is recorded
  as an operational control rather than a security boundary.
- `browser.startup` is registered as the first closed string setting. Windows defaults to
  `on_demand`, Linux to `manual`, and an organization-authored `manual` value pins the effective
  result. Runtime recovery verifies an ordinary executable independently of native-host
  registration, freshly revalidates ownership before repairing stale registration, and launches
  only under the effective `on_demand` posture.
- Rules render as one list in evaluation order, organization first, each a single line that opens
  into detail: read-only for a rule this person cannot change, the editor for one they can.
- A capability line states polarity. Available, some sites blocked, some sites allowed, and not
  available are four distinct answers, and the middle two point opposite ways.
- The editor speaks sentences: host readback on every pattern, organization ceilings shown on the
  control itself, redundant and unreachable rules marked in place, watch-only as a plain switch,
  and a dry run against recorded audit before applying.
- The editor authors the registered settings as three grouped controls -- where agents may connect,
  in the browser, privacy. Boolean permissions are on by default and named by what they do, never
  by their registered key; browser startup is one closed two-option select. A permissive boolean
  value is still never authored;
  `policy.user.enabled` is still refused from a user document; an organization ceiling on a setting
  disables its switch and names who set it, the same as a capability ceiling does. The two channel
  toggles link to the Integrations destination and the scripting guide instead of restating a
  client list that would drift.
- A refused row in the monitor names the deciding layer, the rule, the denial handle, and the
  organization's contacts when it supplied them.
- Gate at implementation: 212 orchestrator library tests, 2 binary, 30 bridge, 4 MCP connector,
  warnings denied, 101 extension tests, plus `node tests/policy-grammar.mjs` and
  `node tests/workbench-surface.mjs` (27 assertions). The amendment recorded in ADR-0122 covers the
  single rule list, stated polarity, authored restrictions, and the A4 permission-toggle framing.

The active 1.0 guides, contracts, public RAWX specification, licensing language, and Trust Center now
describe this feature set rather than the removed flat policy. The invented cohort-based
`greenfield-first-success.md` process is explicitly rejected as a release gate. Historical SPEC,
ADRs, research, and 0.8 task evidence remain preserved as history.

## Fully open-source relicensing (ADR-0140)

On 2026-08-25 the owner withdrew the open-core business model: the entire repository, including
`crates/orchestrator/src/governance/`, is Apache-2.0 OR MIT, and every paid option (tiers,
prices, the founding program, the commercial license text, and the governance-module CLA) was
removed. `PRICING.md` was deleted outright (git history preserves it); LICENSING.md, the licensing guide, README,
CONTRIBUTING, GOVERNANCE, MAINTENANCE, the trust center (FAQ, support policy, continuity,
controls, OpenSSF rows), GitHub templates, security-insights.yml, the workbench About view, and
all packaging payloads were revised to match. The retired MSA/DPA/tiers pages carry dated
retirement notices; historical records (ADRs 0026-0030, CHANGELOG history, 0.8 material,
business planning) are preserved as history under the supersession banners or index notes.
The runtime never enforced licensing and still contains no gate of any kind.

Direct UDP syslog from 0.8 was not rebuilt. The 1.0 audit contract is append-only local JSONL,
collected by the endpoint's existing file agent. Trust Center claims now say that plainly instead
of promising a sink that is absent. HTTP audit upload remains absent.

## Windows 1.0 development-host and package result

The Windows lane passed through implementation `b292bb22` on 2026-08-14. This is a
development-host package result, not clean-machine, provenance, login/reboot, matching-store-adapter,
or public-release evidence.

- `b979a8af` fixed the Windows first-run handoff state-root return found by the native compiler.
  `b292bb22` then fixed two packaged desktop regressions: release launches no longer expose a
  console, and the disposable workbench is created when Tauri's native event loop is ready instead
  of being lost just after startup.
- Formatting, warnings-denied workspace Clippy, all 194 Rust tests, all 100 extension tests, all 10
  npm launcher tests, all 4 MCPB tests, and the process, CLI, PowerShell, and workbench-surface
  journeys passed. The locked Windows build retained the Linux-only Tao patch from ADR-0120.
- GitHub CI run `31809913114` passed all nine jobs at pushed head `de4392db`: Windows and Linux
  Rust, Windows and Linux process journeys, both extension platforms, supply chain, release truth,
  and formatting.
- The mandatory npm process model preserved ordered CLI output and exit status from the optimized
  Windows application binary. A real installed MCP connector negotiated revision `2025-11-25`
  and returned Ghostlight's catalog metadata.
- The one-time npm handoff passed its first usable install, repeat-install, dry-run, `--no-open`,
  and CI-suppression cases. The same disposable install round registered Chrome, Edge, Brave, and
  Chromium plus Codex, Claude Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed,
  OpenCode, and Crush with direct native connector paths. A second install changed zero bytes in
  the nine client configurations.
- The locked NSIS candidate is 3,292,239 bytes with SHA-256
  `100093627d781b1a4e0c8cc481d974e63fbce3939ad2383384c74f8915acb4d9`. Payload inspection found
  the exact three executables and four legal files. A silent install into an exact disposable
  directory ran its browser-registration hook, and doctor reported the full local chain current.
- Native HWND inspection found one visible minimized `Ghostlight` Tauri window and no visible
  console. A second launch restored and focused the same workbench, Close destroyed only that
  window while the authority remained alive, and a third launch rebuilt it. Exactly one authority
  process remained throughout.
- A first uninstall removed all four browser registrations and all nine owned MCP entries. A
  second uninstall changed zero config bytes and left no installed connector reference. The NSIS
  uninstaller removed every package-owned file and its uninstall record. Only test-created runtime
  and empty audit files remained; both exact disposable directories were then deleted. No
  Ghostlight process or default runtime file remained.
- The post-uninstall development checkout was restored on 2026-08-14 by rebuilding and deploying
  all three release binaries under `target/release`, registering the ownership-checked native host
  for Chrome, Edge, Brave, and Chromium, and starting the one local authority. The Foundry
  PowerShell runner now waits explicitly for the console-free Windows executable and validates
  stdout before reading result fields. Its real published-stage journey then completed every beat,
  including the off-domain refusal, replay delivery, and recording erasure.
- Chrome's extension-errors surface then exposed an unchecked `Native host has exited` report from
  a replaced port. The disconnect handler now consumes Chrome's callback-scoped `runtime.lastError`
  before its stale-port ownership guard, so an expected replacement cannot leave a false extension
  error while the current port is healthy. All 101 extension tests pass. The loaded unpacked
  extension still needs its one explicit reload and the historical error card cleared for final
  visible confirmation.

## Windows current-source and local-package pass

The [dated Windows record](testing/windows-current-source-pass-2026-08-15.md) covers revision
`72402a7d` without changing installed registrations or package state.

- Formatting, locked warnings-denied Clippy, a locked isolated build, all 288 Windows Rust tests,
  all 106 extension tests, all 10 npm tests, all 4 MCPB tests, every tracked JavaScript and shell
  syntax check, release truth, 0.8 recovery, repository integrity, dependency policy, and the
  advisory gate passed. The advisory result retains the same 17 documented allowed warnings.
- Fresh isolated process, CLI, PowerShell, workbench, and policy journeys passed against the exact
  build under test. The online public check found GitHub, npm, Chrome, the official MCP Registry,
  and the website in agreement about the public 0.8 state.
- An isolated optimized build produced the exact three siblings, a 4,528,043-byte unsigned NSIS
  package, a deterministic Windows portable archive, and a host-locally reproducible extension
  ZIP. Native package inspection found the complete sibling and legal payload.
- Exact HWND inspection proved minimized console-free startup, foreground activation, Close
  containment, and workbench recreation while one authority stayed alive. The exact test process
  and its runtime files were removed afterward.

This closes current-source, local-package construction, and non-installed native-window evidence
on Windows. It does not close clean install, public-0.8 upgrade, uninstall, login/reboot, tray,
notification, matching-store-adapter, public-harness, provenance-candidate installation, or
publication gates.

## Linux 1.0 development-host result

The native CachyOS lane was extended through the current 1.0 source candidate on 2026-08-14. This
is development-host and npm-candidate evidence, not a Debian package with verified provenance, store-adapter,
reboot, or public-release pass.

- A locally packed, checksum-bound `ghostlight@1.0.0` installed the current optimized Linux
  siblings into `~/.ghostlight/bin/v1.0.0`, updated the active Codex, Claude Code, Visual Studio
  Code, and four Chromium registrations, and handed the unpacked adapter from the older development
  candidate to that exact installed set. The orchestrator, MCP connector, and browser connector
  SHA-256 values are respectively `1cac38da4928dec72e8c6ceabdf92c4266142f76c1ba7bf2921a4cdb0a9e59ec`,
  `81058c3d41fb1815a46cca6e36bb01d5dc8fd864e457ed6f62b2e0a71bd14052`, and
  `a725e65a3a0ff9cfec760f064f876ebc28e1e946356b4a11875ef005095ed8b6`.
- A clean temporary npm consumer downloaded all three current Linux binaries through the launcher's
  injected release transport, printed one progress and one verification line per sibling, and
  rejected no expected checksum. A separately packed `npx --offline --package` install started
  from an empty home and config root, registered detected clients, opened the service-first
  walkthrough once, kept its marker mode `0600`, stayed non-interactive on reinstall, and reported
  the idle service as ready on demand.
- The active installed candidate passed `doctor` with its service and native adapter connected,
  then completed visible open, list, 19-word read, and 1248x615 screenshot against Example Domain.
  The default preserve-tabs interlock truthfully refused model-driven close. The service-first
  installer and extension-first adapter now lead to their opposite halves through the two stable
  pages; only a first real install opens a page, while CI, dry-run, `--no-open`, update, and repeat
  paths stay non-interactive.

- Rust 1.95.0 formatting, warnings-denied clippy, isolated build, all 191 workspace tests, all 99
  extension tests, all 10 npm tests, all 5 MCPB tests, 41 JavaScript syntax checks, both process
  journeys, the workbench surface journey, `cargo deny` bans/licenses/sources, and `cargo audit`
  passed. Audit still reports the 17 allowed GTK3/Tauri-chain warnings recorded in the dated
  readiness audit.
- The earlier three-sibling optimized user candidate at
  `~/.ghostlight/bin/v1.0.0-dev-6152636` remains as historical local evidence but is no longer the
  active browser stack. Its orchestrator, MCP connector, and browser connector
  SHA-256 values are respectively `97131236cdbb0be8367ce152182af8b8eaba8033f34c8d407a948ac5e20f58b3`,
  `73738e5d71ce6f20ad211c9b10082a5725ab32d05fe2fb9f447913c32662337d`, and
  `1631aed13e00aa0c22a8af8cdde259bcc56c1210dc88a4291441c8025c55cb50`.
- The installed workbench uses Tao's exact merged Wayland decoration fix. KDE reported a separate
  28-pixel server-side titlebar with closeable, minimizable, maximizable, and resizable state; the
  owner confirmed the controls accept pointer input. Native minimize/maximize/restore/close all
  landed while the authority stayed alive. The exact `Open Ghostlight` tray menu item rebuilt one
  active 1180x760 view after minimize and after close, and two concurrent Open events coalesced to
  one replacement without a duplicate label.
- A fresh offline consumer installed the packed npm launcher and proved bare MCP initialization,
  its 22-tool catalog, one safe browser call, native CLI routing, valid-cache reuse, tamper
  replacement, and refusal to execute incomplete or unverified bytes. No npm registry was
  contacted.
- Public attested 0.8.0 was installed through its documented portable path, then upgraded without
  removing the browser profile, settings, harness configuration, or any older version directory.
  The first run exposed a surviving Linux supervisor; the corrected migration now stops the
  positively identified unit before removing its unit and enablement. A final uninstall/reinstall
  removed only owned entries and preserved malformed or foreign configuration byte-for-byte.
- Ordinary-profile Chromium 151 under the active KDE Wayland session proved visible open, read,
  screenshot, presentation, single-authority activation, connector demand-start, and browser
  restart recovery. Local preserve-tabs correctly refused the attempted model-driven close.
  Closing/reopening the workbench, tray open/quit, login/reboot, a second live harness, and the full
  interactive form/drag/upload/dialog matrix remain owner-visible work.
- The portable archive was inspected and hashed. Tauri staged a complete AppDir, but its bundled
  `linuxdeploy` strip tool cannot parse CachyOS `.relr.dyn` sections, so no AppImage pass is
  claimed. The Ubuntu/Debian lifecycle table remains untouched and blocking.

## Lean Linux installation and visible activation

[ADR-0123](adr/0123-lean-linux-install-and-visible-activation.md) accepts and implements the
highest-value 1.0 findings from [Research 25](research/25-delightful-linux-experience-2026-08.md).

- `ghostlight open` composes the existing sibling demand-start and authenticated activation seams.
  Connector startup still passes no arguments, the authority still begins backgrounded, and no
  second service role, listener, wrapper, or resident supervisor was added.
- Linux user installation owns one XDG Applications entry and the existing 128-pixel icon. The
  entry names the exact installed executable plus `open`; updates rewrite only owned state,
  uninstall removes only owned bytes, and `/usr/bin/ghostlight` defers to the Debian package's
  system entry.
- Browser package provenance is a closed local fact separate from native-host registration.
  Default Linux setup selects detected native Chrome, Edge, Brave, or Chromium packages. Snap and
  Flatpak-only selections are refused with the native-package remedy; `--all-browsers` keeps the
  deliberate pre-registration route.
- The release Linux artifact builds on Ubuntu 22.04. Candidate assembly requires exact Debian
  package install/remove/reinstall/purge smokes in Debian 12 and Ubuntu 24.04. The local expanded
  matrix and GitHub candidate run `31920647296` both pass those gates. This does not replace the
  visible Ubuntu GNOME Wayland L1-L9 gate.
- Current-tree verification passed formatting, warnings-denied workspace Clippy, 274 Rust tests,
  103 extension tests, 10 npm tests, 4 MCPB tests, shell syntax, fresh isolated build, process, CLI,
  workbench, policy-grammar, Tauri-config build, dependency-policy, and advisory gates. The live CLI
  separately reported native browser provenance and missing Applications integration correctly.
  The rootless package lab below supersedes this source-only package limitation.

RPM remains the next rational native format only after a separate scope decision and a real
lifecycle host. AppImage, Snap, Flatpak, AUR, and Nix artifacts do not become 1.0 gates merely
because their packaging tools exist.

## Full local Linux release rehearsal

The current CachyOS user installation passed the fullest local release rehearsal at `51552025`.
The [dated record](testing/local-linux-release-rehearsal-2026-08-15.md) carries exact hashes,
commands, coverage, and limits.

- The supported uninstall removed only owned browser, client, and Applications state. The previous
  version directory was preserved on the second drive. Reinstall restored the exact three siblings,
  Chromium native messaging, Codex, Claude Code, Visual Studio Code, and the XDG entry. Repeat
  install changed nothing, and no product supervisor exists.
- Direct MCP negotiated `2025-11-25`, returned the exact 22-tool catalog, and completed a real
  call. CLI and PowerShell journeys passed. All 13 policy examples validated; explain, audit
  simulation, invalid-input refusal, Ed25519 authoring, live narrowed authority, fail-closed invalid
  authority, and all-open restoration passed.
- The published Foundry journey completed form, upload, recording, diagnostics, host refusal,
  replay, and byte erasure. Prompt handling and screenshot-bound coordinate input passed. Authority
  demand start, browser restart, stable browser identity, and post-restart work passed.
- The first Foundry run found that the clean-room drag path had retained only pointer packets and
  could wait forever for a held move receipt. `51552025` restores ADR-0088's bounded two-lane seam:
  explicit held-button packets, action-scoped native interception, opaque drag-data replay, and
  cleanup on every terminal path. The Foundry pointer lane and a native HTML drag/drop fixture both
  passed live after the fix. Extension coverage is now 106 tests.
- All 277 Rust, 106 extension, 10 npm, and 4 MCPB tests pass, as do formatting, strict Clippy,
  JavaScript and shell syntax, process/CLI/PowerShell/workbench/policy journeys, deterministic
  extension packaging, recovery and integrity checks, dependency policy, and the advisory gate.
- A fresh native Zed 1.15.0 installation exposed that CachyOS/Arch names its launcher `zeditor`.
  Zed's existing declarative harness descriptor now lists both `zed` and `zeditor`; the generic
  detector and single canonical config target remain unchanged. Ordinary unforced install created
  the exact `context_servers.ghostlight` entry, Zed showed it active, and the live process chain
  reached the exact installed connector. Repeat install changed zero config bytes. Removal stopped
  that connector while preserving other harness files byte-for-byte, and ordinary reinstall plus
  the documented client restart restored it. Zed's custom-server schema has no icon field, so its
  generated `G` and custom marker remain an observed host limitation. Ghostlight's own packaged
  visual identity and the roster expansion are now accepted by [ADR-0125](adr/0125-recognizable-plural-linux-harness-integrations.md).

This closes the broad development-host browser matrix previously listed as incomplete. It does not
close the provenance-bound Debian, matching-store-adapter, Ubuntu GNOME login/reboot, notification,
or three-public-harness gates.

## Recognizable plural Linux harness integrations

[ADR-0125](adr/0125-recognizable-plural-linux-harness-integrations.md) is implemented. The
[dated evidence](testing/linux-harness-roster-2026-08-15.md) records the complete development-host
roster pass and its release boundary.

- One fixed registry now has 18 products and 21 concrete targets. Added products are GitHub
  Copilot CLI, Cline, Kiro, Qwen Code, Junie, Kilo Code, goose, Continue, and Antigravity. Cline's
  CLI, Visual Studio Code, Cursor, and Windsurf targets remain independently owned below one card.
- Cards use packaged offline Ghostlight-owned visual marks and accessible product names. Missing
  products offer an official Install destination, Locate, Copy MCP command, and target-specific
  Copy setup. Detected targets offer Set up or Update; current targets offer Remove. Automatic
  setup failure opens the same manual route.
- The workbench renders one compact flat grid ordered Ready, Available, Needs Attention, and Not
  Detected, with names alphabetical inside each status. Updatable and malformed or foreign targets
  require attention. A ready target wins for a plural product; otherwise attention wins over an
  available sibling so the card cannot conceal a repair. Each card pairs its status label with a
  green, blue, amber, or neutral treatment. The roster has no status headings or counts, and it
  never relies on color alone.
- Locate is one bounded native picker. Download URLs and clipboard material resolve in Rust from
  closed ids. The WebView has no generic dialog, clipboard, opener, shell, or filesystem grant.
- JSON, JSONC, TOML, and the new shared YAML seam preserve unrelated configuration. YAML tests cover
  comments, ordering, file mode, exact no-op bytes, owned removal, and refusal of flow shapes that
  cannot be edited losslessly.
- All nine new products were installed and started the exact isolated connector through their real
  Linux MCP lifecycle. Repeat setup and repeat removal were byte-identical across every target,
  and the final re-add restored all registrations.
- That live matrix found three compatibility blockers and closed each at its seam. Junie now
  negotiates `2025-03-26`; Antigravity receives `2026-07-28` discovery and falls back to an honestly
  advertised initialized revision; and Kiro/Bedrock receives portable top-level object schemas
  while the typed decoder retains exact conditional validation. No full stateless 2026 support is
  claimed.
- Current gates pass: formatting; warnings-denied workspace/all-target Clippy; all 294 Rust tests
  (252 orchestrator library, 4 orchestrator binary, 32 bridge, 6 MCP connector); 106 extension,
  10 npm, and 4 MCPB tests; every tracked JavaScript syntax check; process, native CLI, PowerShell,
  workbench (34 assertions), and policy journeys; repository integrity; dependency license, ban,
  and source policy; and `cargo audit` with the same 17 documented GTK/Tauri-chain warnings.
- The status-sorted roster follow-up passed formatting, warnings-denied workspace/all-target
  Clippy, all 356 Rust tests, 116 extension tests, 10 npm launcher tests, 4 MCPB tests, all 43 tracked
  JavaScript syntax checks, the 42-assertion workbench surface journey, policy grammar, and fresh
  isolated process, native CLI, and PowerShell journeys. Its preview serves the complete 21-target,
  18-product roster and refuses to start if either the exact id set or product count drifts.

This closes current-source roster compatibility. The source-roster pass itself did not provide
package provenance; the build-only candidate below now does. Ubuntu GNOME Wayland,
matching-store-adapter, login/reboot, and publication remain open.

## The integration destination returned to cards

On 2026-08-16 the MCP integrations destination was redesigned five times in one session: product
cards, compact single-line rows, two-line rows, one switch per client, and a master-and-detail split.
Each iteration removed a defect the owner had named, and each result was rejected. The owner reverted
to the card roster it started from, so the compact status-sorted card roster described above is
current and accurate.

[ADR-0129](adr/0129-integration-roster-reverted-to-cards.md) records the revert and supersedes
ADR-0130 (integration switches and foreign-entry evidence) and ADR-0128 (master and detail) in full.
ADR-0125 Decision 2 governs the destination again. Both superseded records are retained as history
and neither governs. ADR-0129 Decision 3 keeps what the five attempts established, so the same shapes
are not rediscovered by the next person who opens the surface and sees repetition.

Two different decisions were both filed as `0127` on 2026-08-16. The owner resolved the collision on
2026-08-17 by renumbering the superseded one:
[`adr/0127-one-invoked-desktop-authority.md`](adr/0127-one-invoked-desktop-authority.md) keeps its
number and governs, and the switch roster became
[`adr/0130-integration-switches-and-evidence.md`](adr/0130-integration-switches-and-evidence.md),
which does not govern. No decision text was reopened. The renumber is marked in ADR-0130's header,
and the references to it in ADR-0128 and ADR-0129 carry the same note, so the edit is visible rather
than silent.

## Provenance-bound build-only candidate

The [dated candidate record](testing/release-candidate-2026-08-16.md) carries exact run links,
hashes, provenance checks, and remaining limits. Source revision
`fd8640336b11ed12cd47fe96deb7eb06adfbdcd1` passed ordinary CI run `31920645118` and manual
build-only candidate run `31920647296`.

- All nine cross-platform CI jobs passed, including Windows and Linux Rust and process journeys.
- The release quality gate, Ubuntu 22.04 Debian build, Windows 2025 NSIS build, deterministic
  extension build, Debian 12 and Ubuntu 24.04 package lifecycle smokes, and candidate assembly all
  passed.
- The candidate contains 17 checksum-bound artifacts and four CycloneDX SBOMs. All 17 hashes
  matched locally. GitHub provenance verified for every asset plus the manifest and checksum file,
  pinned to the exact repository, release workflow, source revision, and `dev` ref.
- The GitHub bundle has 14-day retention. No tag, release, submission, or publication was created.

This closes candidate construction, provenance, and the two accepted noninteractive Debian package
gates. It does not close visible Ubuntu GNOME Wayland, matching-store-adapter, clean Windows,
login/reboot, notification, public-harness, or publication gates.

## Rootless Linux package evidence

[The dated container record](testing/linux-container-evidence-2026-08-15.md) carries the exact
candidate, image digests, coverage boundary, and results. The source candidate
`a9bd73424198cb144154117ad4dcae682d18baf5` produced a 4,768,536-byte Debian package with SHA-256
`a6c898f9072ae50363b12e8d422f74a6718d2bce3a874bd82d6d25b9658338e9` in a rootless Ubuntu 22.04
builder on the second drive.

- Debian 12, Debian 13, Ubuntu 24.04, and Ubuntu 26.04 passed the same package journey. It checks
  control metadata, dependencies, package checksums, modes, missing libraries, RPATH, the GLIBC
  ceiling, four conffile-bound native manifests, desktop validity, ordinary-UID runtime mode,
  status, doctor, native-host reporting, MCP initialize, remove, reinstall, purge, owned cleanup,
  and retained user state. Every binary's maximum required GLIBC symbol is 2.34.
- Ubuntu 24.04 passed that complete journey with its network namespace disconnected. The portable
  archive separately passed XDG install, exact idempotency, version-path update, runtime startup,
  and ownership-safe uninstall. Its SHA-256 is
  `7bf2994067c148191d797c572abd1a3604b487497c4bef8e2a44fb04548f8d10`.
- The attested public 0.8.0 archive installed its real user supervisor and browser manifests. The
  packaged 1.0 command retired the unit and enablement, rewrote all four owned manifests, changed
  zero bytes on repeat, preserved unrelated state and all old binaries, started as the ordinary
  user, and left user state after package purge.
- Advisory `lintian` now reports only browser-mandated `/etc/opt` paths, absent manpages for the
  three sibling executables, Rust-inapplicable C fortify notices, intentional duplicate legal
  resources, and binary string-table false positives. Placeholder metadata, libc dependency,
  conffile, changelog, copyright, strip, archive compression, and package-name path defects found
  by the first run are fixed and guarded.
- The release workflow and local guests now share one package lifecycle script. Extra local
  distributions remain advisory rather than expanding the accepted two-row release gate.

This local record supplied strong noninteractive package evidence but not provenance. The build-only
candidate above now supplies matching GitHub provenance and the accepted two-row package smokes.
Ubuntu GNOME Wayland L1-L9, the matching store adapter, login/reboot, tray, notifications, and the
full visible browser matrix remain owed.

- `dev` is the working branch and the release source line. Workspace version `1.1.0`, the ZCode
  harness line, whose candidate is held in custody. It absorbed `ghostlight-1.0`, which was a
  fast-forward and has been retired; `main` was promoted to the published 1.0.0 on 2026-08-26.
- `main` carries the published 1.0 line, promoted by fast-forward on 2026-08-26. Promoting `dev`
  again for 1.1.0 is a deliberate release decision, not routine sync. The line carries Windows and
  Linux source, extension, process, and supply-chain CI; a manual Pages deployment; and bounded
  monthly dependency updates targeting `dev`.
- No pull requests are open. Thirteen Dependabot bumps against the 0.8 line were closed as obsolete
  on 2026-08-13: the 1.0 tree either already carried the proposed version or had dropped the
  package outright (`clap`, `rustls`, `webpki-roots`, `color_quant`). Dependency updates are paused
  on `main` with `open-pull-requests-limit: 0` rather than by deleting the configuration. The 1.0
  config targets `dev`, runs monthly, groups non-major updates, and caps open work per ecosystem.
- The pre-1.0 worktree snapshot is preserved as the annotated tag `archive/0.9-pre-1.0`
  (`f5d43768`), pushed to the remote. It replaced a local-only branch that existed on one machine.
  It is history, never implementation authority for the 1.0 tree.
- Release safeguards are active again on `dev`: Rust, extension, and process CI cover Windows and
  Linux; dependency licenses, sources, wildcards, and
  advisories are gated; source and observed-public versions are checked separately; and the store
  extension package is built from an explicit runtime allowlist. The online public check passed
  against GitHub, npm, Chrome, the official MCP Registry, and sylin.org on 2026-08-13. The combined
  CI workflow passed all nine jobs at pushed Windows-lane head `de4392db` on 2026-08-14.
- Packaged native-host lifecycle is restored without restoring the 0.8 resident supervisors
  (ADR-0115). The orchestrator now checks, installs, updates, and safely removes Chrome, Edge,
  Brave, and Chromium registrations; packages carry both connector sidecars; and narrow migration
  retires recognized pre-1.0 Windows and Linux supervisor artifacts. The Windows NSIS
  candidate passed payload, install, doctor, idempotency, workbench-lifecycle, and uninstall checks
  on this development host. Clean-machine, provenance-verified, login/reboot, 0.8 package-upgrade, and Linux
  native-package journeys remain required evidence.
- Release construction is one checked 17-artifact unit: two native packages, two portable
  archives, six raw binaries, the deterministic extension, four component SBOMs, the npm launcher,
  and the Claude Desktop MCPB. Exact byte length and SHA-256 bind every item to one version and full
  source revision. The workflow adds GitHub build provenance but remains build-only. GitHub, npm,
  Chrome, and MCP Registry adapters each default to a non-mutating plan and require an explicit
  named action plus owner-approved execution; there is no master conductor.
- The published user entry points are restored on current seams. `npx -y ghostlight install` remains the
  primary journey; a bare npm launch remains MCP stdio; the launcher verifies all three cached
  binaries on every run. `ghostlight install`, `uninstall`, `doctor`, `doctor --fix`, `status
  --json`, `service`, dry-run, repeated client selection, and repeated browser selection are live.
  One-line installers, deterministic portable archives, the self-contained MCPB, and
  candidate-derived Scoop and WinGet 1.12 metadata are present and tested.
- Release access was recovered without exposing values. GitHub and npm authentication work, and
  the MCP DNS key and official publisher binary are present. Chrome API V2 access now validates the
  exact existing item after a PKCE refresh-token renewal and a non-secret publisher-id override.
  Ghostlight has no Windows code-signing certificate; checksums plus keyless GitHub provenance are
  the trust model instead of a signing gate.

## Implemented

- One Rust 2021 workspace builds four roles: the shared typed bridge, `ghostlight` orchestrator,
  generic MCP connector, and opaque browser connector.
- The orchestrator owns the 24-tool model-facing catalog, workspace aggregate, one executor and
  completion path, immutable authority snapshots, runtime controls, content-minimized audit,
  browser port, and content-free presentation decisions.
- The page-context JavaScript tool is `browser_execute`, not `browser_evaluate`. The execute name
  states that it may read, mutate, or navigate. The unreleased old name has no alias. Internal
  `RunScript` and `EvaluateScript` mechanism names remain behind the language boundary.
- The stable browser fringe includes a policy-free Manifest V3 extension, durable native relay,
  operation-disposition recovery, one browser-wide exact-title group per client label, dedicated
  Ghostlight window placement, and the established visual language and product identity.
- Adapter 1.0.0 advertises end-to-end liveness (ADR-0113). The service sends a content-free
  heartbeat every 20 seconds and follows every physical dispatch with its own probe. Forty-five
  seconds without an acknowledgement makes an attached relay unavailable; an operation deadline
  with no post-dispatch acknowledgement quarantines it immediately, so the next call stops before
  dispatch. A healthy silent operation stays connected when the extension answers independently.
  Older adapters retain their capability-gated attachment behavior, and the opaque browser
  connector is unchanged.
- Browsers are plural (ADR-0114). The service keeps one adapter connection per persistent browser
  identity, so Chrome and Edge, or two profiles, are connected and worked in at once. A hello
  carrying an identity that is already registered replaces that entry and **closes the replaced
  stream**, which is what makes a duplicate connection collapse instead of lingering as a silent
  sink. Each workspace binds to one browser for its life; physical tab ids resolve as
  `(browser, physical_id)`, so one browser's tab 5 can never be governed as another's. A crossing
  with no binding uses an explicit `browser`, then reported attention, then the sole connected
  browser, and otherwise refuses while naming the candidates. Runtime control publishes to every
  connected browser.
- Extension native-host startup is single-flight. Concurrent bootstrap, installation, startup,
  and reconnect signals share one attempt, and ownership is rechecked after local-state
  initialization. One worker epoch therefore cannot strand multiple attached relays with only one
  active extension listener.
- Recording now has one owner (ADR-0108, extended by ADR-0109). The extension keeps a plural,
  workspace-namespaced, memory-only registry; owns capture ids, frames, fixed bounds, autonomous
  stop, five-minute retention, erase, and the GIF encode itself; and exposes only
  start/status/stop/export/discard physical requests. It folds byte-identical successive JPEGs into
  one retained visual span with an accumulated duration, so capture time and compressed bytes are
  the ordinary limits. During recording, presentation disables only the perpetual controlled-scope
  glow and keeps transient action feedback available. The old service coordinator, renewal loop,
  unsolicited frame events, and duplicate deadlines are gone.
- Recording frames no longer cross a process boundary (ADR-0109). `gif_output.rs`, the
  frame-returning `read` command, and `PhysicalRecordingFrame` are deleted. The orchestrator
  governs the save, names one of three destinations, and states an output budget; the extension
  encodes in an offscreen document (pinned MIT `gifenc` under `extension/vendor/`) and delivers.
  A page attach and a browser download finish inside Chromium; only a client return carries bytes,
  and the shape of `RecordingDelivery` is what makes that structural rather than a rule. Thinning
  exists once, in `extension/lib/recording.js`, and folds each dropped frame's time into the frame
  before it, so a thinned replay still plays for as long as the work took. A saved replay's
  sentence reports how long it plays; counts and bytes stay in the facts. The manifest gained
  `offscreen` and `downloads`, which is a published-surface change.
- Model-driven tab close is admitted by service authority and then checked by the extension's
  default-on preserve-tabs interlock. A refusal stays visible and returns a blocked no-effect
  result.
- The `ghostlight` executable hosts a Tauri 2 workbench inside the modular monolith, with a tray
  lifecycle, bounded global search, and content-free native notifications. Its tab row carries five
  destinations:
  - **At a glance**, the landing surface. The current action stands in full with its elapsed time,
    then settles and drops into a newest-first queue as the next one rises. Connected sessions and
    browser instances sit alongside it, and the last completed action stays on screen while
    nothing is running.
  - **MCP integrations**, which checks, connects, and disconnects Ghostlight's owned registration.
    The narrow tab row abbreviates its label to Integrations; the destination's name everywhere
    else, including global search, is MCP integrations.
  - **Status**, which carries diagnostics, authority sources, and the end-session intent.
  - **Policy**, opened by the state chip that sits between Status and About, described under the
    readable-policy section above.
  - **About**.

  The landing destination was renamed from Monitor to At a glance in reference-experience S6. Its
  internal view id is still `monitor`, so the source name and the product name differ here by
  design.

  Pause and resume live in the persistent header beside the connection state and match the tray.
- The workbench capability grants the notification plugin only its automatic permission-state
  bootstrap probe. Notification delivery remains a Rust-owned presentation port; the WebView has
  no permission to request, send, cancel, or otherwise manage native notifications.
- The orchestrator publishes a closed sequenced change vocabulary (`OperationStarted`,
  `OperationChanged`, `OperationSettled`, `RuntimeChanged`) through a best-effort
  `WorkbenchEventSink`. Snapshots carry the sequence they reflect; a surface that receives a gap
  resynchronizes from a fresh snapshot rather than trusting its cache. The WebView may listen and
  is not granted permission to emit. A projection with no sink attached publishes nothing, so
  domain tests with no presentation sink stay free of desktop dependencies.
- `OperationSummary` carries the governed capability, so live work is classified as plainly as
  completed history.
- At a glance has a presentation-only Clear view control. It hides completed actions for the current
  desktop surface, keeps running work visible, and never mutates or deletes the durable audit.
- The workbench follows the published sylin.org palette: Ghostlight's teal accent carried as
  `--a`/`--al`/`--argb`, the night-garden ground, and the five-step ink ramp. The in-page renderer
  deliberately keeps its trained sky signal. The two surfaces still share the spring curve and the
  ADR-0083 medallion vocabulary.
- Supported MCP client registrations cover 18 products and 21 concrete targets: Codex, Claude
  Code, Claude Desktop, Cursor, Visual Studio Code, Windsurf, Zed, OpenCode, Crush, GitHub Copilot
  CLI, four Cline targets, Kiro, Qwen Code, Junie, Kilo Code, goose, Continue, and Antigravity.
  Re-check is read-only. Set up, Update, and Remove are explicit, serialized, ownership-checked,
  backed up, and preserve unrelated JSONC, TOML, and YAML configuration. Harness paths follow the
  effective Windows or Linux environment, including `CODEX_HOME`; exact owned pre-1.0 agent relays
  are migrated while other relay entries remain untouched and visible as attention-required state.
- There is no `service` command or `--headless` flag. Connector demand-start, CLI demand-start, and
  direct execution all invoke the same no-argument desktop authority. Desktop startup or event-loop
  failure ends it instead of leaving an invisible process.
- The shared bridge owns one demand-start seam used by both connectors after a failed service
  connection. It starts only the exact sibling `ghostlight` with no application arguments, honors
  a fresh deploy lock, and preserves each connector's established reconnect behavior.
- The orchestrator holds an operating-system lifetime lease before publishing runtime discovery or
  initializing Tauri. Concurrent launch attempts therefore converge on one authority and one tray.
- There is one desktop-authority launch. It creates a tray where the desktop session provides one
  and backgrounds its workbench: minimized on Windows and hidden on Linux. A second direct launch
  opens and focuses the running authority's authenticated workbench. Sessions without a tray retain
  the Applications entry and `ghostlight open`. Windows restores its existing view. Linux
  reconstructs its disposable view because Wayland cannot report or unset minimization.
- The tray and authority outlive their disposable workbench. Native close destroys the window,
  native minimize remains compositor-owned, and Open uses one serialized lifecycle seam. Linux
  coalesces Open requests, destroys any existing view, and reconstructs only after Tauri reports
  that exact window destroyed. Windows focuses or restores its existing view and constructs one
  when absent. Linux observes abnormal WebKit renderer loss, discards only that exact window after
  the callback, and recreates on the next explicit Open. The proprietary NVIDIA renderer policy is
  selected before WebKit starts and preserves user override.
- `ghostlight call` is a second intake for scripts and programs (ADR-0105). It invokes one tool, or
  a batch of them over one session with `--stdin`, prints the outcome sentence or `--json`, and maps
  the terminal status to distinct exit codes where an uncertain effect is never zero. It demand-
  starts the authority like any connector, and it crosses the same executor, governance facade, and
  completion path, so there is no scripting bypass.
- Every session records the intake it arrived on, and every audit record carries it. `ghostlight
  call` work is attributed to the `cli` channel and grouped under its own browser tab-group name.
  The channel is attribution and is never an input to an authority decision.
- `ghostlight_bridge::client::ServiceClient` is the one place the service handshake lives, so a
  second edge does not grow a second copy of it.
- `--output <file>` writes bounded content, so a scripted capture lands as an image rather than as
  base64 in a terminal. Later captures in one session gain an index instead of overwriting.
- A policy layer may close an intake: `{"channels":{"cli":{}}}` refuses it, `{"enabled":true}`
  admits it, and an absent map restricts nothing, so all-open is untouched. Layers intersect, so a
  managed refusal cannot be undone locally, and an unknown channel name is a typo that fails closed.
  The refusal lands at admission with the stable `channel_denied` reason, before a workspace exists,
  so nothing is invoked and nothing is audited (ADR-0105 amendment).
- A command-line session is its caller, not its connection (ADR-0106). Every `ghostlight call` from
  one terminal, or from one program that shells out repeatedly, reaches the same workspace and the
  same tabs. Identity is the caller's process id plus start time, so a recycled pid running the same
  program does not inherit a dead session; the executable name rides along for attribution only.
  `GHOSTLIGHT_SESSION` pins a session explicitly for a caller whose own children are ephemeral, and
  is a claim rather than an observation, so it never reaches an authority decision.
- An owned workspace outlives its connection and is released when its owner is gone, handing back
  the tabs it held. The close it then asks for goes through the same interlock a model's close does,
  so with the default-on preserve-tabs setting those tabs are released but stay visible. Liveness is observed rather than guessed at, sweeping on admission so the cost follows
  use. Work in flight is never reaped, and a connection that sends no marker keeps the previous
  connection-bound behavior, which is what the MCP edge does.
- [`scripts/demo-brief.ps1`](../scripts/demo-brief.ps1) and
  [`scripts/demo-brief.sh`](../scripts/demo-brief.sh) drive the ADR-0069-era launch-brief demo story
  entirely through the command line: open, scan, inventory controls once, three separately paced
  field writes, two checkbox clicks, submit, and a wait for the exact completion sentence.
  Verified live against the published Sylin stage: ten steps, one session, read/write/action
  capabilities classified per tool, and no typed value in the audit. `docs/design/demo-brief.md`
  specified this as a Rust subcommand; it does not need to be one, and the note now says so.
- [`scripts/browser-journey.ps1`](../scripts/browser-journey.ps1) and
  [`scripts/browser-journey.sh`](../scripts/browser-journey.sh) are complete PowerShell and POSIX
  shell journeys over the CLI: open, list, read, capture to a file, close, with Ghostlight's own
  exit code preserved. Each call remains a direct child of its long-lived shell, so each step uses
  the handle the previous one returned. [`scripts/demo-foundry.sh`](../scripts/demo-foundry.sh)
  gives the full Card Foundry story the same Linux-native entry point. All three shell scripts are
  syntax-gated and were verified against visible Chromium.
- At a glance rows carry the intake between the tool and the description, resolved from the record when
  settled and from the still-connected session while running. A guard derives the row's grid track
  count and each width's hidden cells from the stylesheet and compares them to the cells the surface
  renders, so a new column cannot silently shift the ones after it.

## Verified in this workspace

Re-run through 2026-08-15 against the current tree:

The complete dated evidence and explicit NOT RUN release gates are in
[`testing/release-readiness-2026-08-13.md`](testing/release-readiness-2026-08-13.md). The result is a
source-gate pass, not release approval.

- The follow-up gate repair made the managed-policy environment import Windows-only, moved the CLI
  refusal journey onto the maintained schema-3 policy example, scoped the reviewed
  `CDLA-Permissive-2.0` data-license exception to `webpki-roots`, and removed the unmaintained
  direct `rustls-pemfile` dependency in favor of the parser already exposed by `rustls`.
- `cargo fmt --all -- --check` and locked workspace/all-target warnings-denied Clippy passed.
- `cargo test --workspace --locked --no-fail-fast`: 264 Rust tests -- 227 in the orchestrator
  library, 2 in its launch-mode binary, 31 in the shared bridge, and 4 in the MCP connector.
- `npm test --prefix extension`: 103 extension tests.
- `npm test --prefix packaging/npm`: 10 launcher tests. The MCPB launcher has 4 Node tests.
- Fresh isolated debug binaries passed the Linux process and CLI journeys, including the schema-3
  CLI channel refusal. The workbench surface passed all 30 assertions, all 42 tracked JavaScript
  and module files parsed, and a fresh locked optimized workspace build completed. Dependency
  license/source/bans checks passed, and `cargo audit` returned to the documented 17 allowed
  Tauri/GTK-chain warnings after the direct PEM dependency was removed.
- The PowerShell-specific CLI journey was not rerun because PowerShell is absent on this host. The
  CachyOS host also cannot satisfy the exact pinned Ubuntu/Debian candidate and package inspection
  gate. The existing Windows and older development-host evidence remains unchanged, and the Debian
  L1-L9 lifecycle remains NOT RUN.
- The earlier cross-platform gate passed all four executable process/workbench journeys and all 41
  JavaScript and module files then tracked. This Linux follow-up reran the three journeys available
  without PowerShell and all 42 files now tracked. The 1,388-entry recovery matrix passed, every
  tracked file was readable with all local documentation links valid, and the artifact guard covers
  809 in-scope mature 0.8 paths.
- An isolated Linux command journey seeded an exact pre-1.0 Codex relay under a non-default
  `CODEX_HOME`, ran the built `ghostlight install --client codex`, and proved the Codex binary read
  the replacement MCP connector with empty arguments. The fixture used separate home and XDG roots
  and did not touch the active user configuration.
- The optimized local candidate then migrated the active Codex and Visual Studio Code legacy
  relays to the sibling MCP connector, reported both installed through doctor, and proved a second
  install was idempotent. The installed MCP edge negotiated revision `2025-11-25` with all 22
  tools. A fresh visible Chromium profile completed open and read against Example Domain; close
  stopped truthfully at the default-on preserve-tabs interlock.
- The restored development loop found the running repository service only by its exact
  `target/release/ghostlight.exe` path in plan mode. Its isolated smoke then built one selected
  package, enclosed the swap in `deploy.lock`, copied into a disposable repository-local live
  directory, removed the lock, and performed no launch under `-NoStart`. The real stack was not
  disturbed.
- Four real CycloneDX component SBOMs were generated with pinned `cargo-cyclonedx` 0.5.9. Synthetic
  Windows and Linux input exercised the exact 17-artifact assembly, npm hash embedding, deterministic
  MCPB, portable packaging, and package-manager metadata paths. That proves the construction code,
  not the missing native Linux artifacts or their provenance.
- Chrome, GitHub, and MCP publication plans made no mutation. That rehearsal exposed two 1.0
  release-adapter regressions from 0.8: optional Chrome API automation was described as required,
  and GitHub refused a candidate for lacking nonexistent platform signing. Both gates were removed;
  checksums and GitHub provenance remain mandatory. MCP Registry planning found the recovered key
  and publisher but refused the current 0.8 `server.json` and npm coordinate for a 1.0 candidate.
- The deterministic 1.0 extension ZIP has SHA-256
  `47a7cb7b715d14de991266f3602ecf6f166fd967623c4e7980f58a2afc3c47c3` and contains the exact
  Apache and MIT texts. The rebuilt local unsigned Windows NSIS candidate contains the exact three
  sibling executables and all four exact legal files; its SHA-256 is
  `100093627d781b1a4e0c8cc481d974e63fbce3939ad2383384c74f8915acb4d9`. Neither artifact is
  published or release-approved.
- `cargo audit` exited zero but reported 17 residual transitive warnings in the Linux Tauri/GTK3
  and Tauri URL-pattern graphs, including the `glib` iterator unsoundness advisory. The dated audit
  records the dependency paths and requires a recheck before Linux publication; do not call this scan
  warning-free.
- Plurality contracts prove two browser identities stay connected at once and each answers its own
  request, a second connection from one identity collapses onto the first with the replaced stream
  reaching end-of-stream rather than hanging open, attention moves to front without duplicates and
  never routes to an absent browser, and resolution prefers selection, then binding, then attention,
  then a sole browser. Executor contracts prove work follows the attended browser once and then
  stays there when attention moves, an ambiguous bootstrap names both candidates with no dispatch
  and no binding, a named stranger is refused rather than substituted, and listing tabs answers
  truthfully with no browser connected at all.
- Browser-port contracts prove an attached socket without heartbeat acknowledgements becomes
  unavailable, an unanswered post-dispatch probe quarantines at the operation deadline, a legacy
  adapter keeps compatible attachment semantics, and a silent operation can outlast the liveness
  timeout while independent acknowledgements keep it available.
- Lifecycle tests prove demand-start supplies no application arguments and the executable has one
  desktop-authority mode beside scripted intake. The real process journey still passes across
  authority restart and connector renegotiation.
- Action-subject tests prove the Chrome receipt carries the physical role and name without a
  describe round trip, the role cannot author language, editable values cannot become names, names
  are normalized and bounded, and either authority layer can remove them monotonically.
- `node tests/cli-powershell-journey.mjs`: the shipped PowerShell script drives a real service and a
  scripted browser adapter through open/list/read/capture/close, exits zero, writes real JPEG bytes,
  and every step is audited as `cli` with the landing host and no page text. It then proves the
  session marker across processes: one `ghostlight call` opens a tab and a separate one lists it.
- `node tests/cli-journey.mjs`: the real executable's command line reaches a real service, returns a
  governed result, exits non-zero on refusal, is attributed to the `cli` channel in the audit file
  the service wrote, and keeps one workspace across a `--stdin` batch while separate processes get
  separate workspaces. A second service started with `{"channels":{"cli":{}}}` refuses the intake
  with `channel_denied`, exits non-zero, and writes no audit record.
- `node tests/process-journey.mjs`: stable MCP and browser relays reconnect through a service
  restart without replaying an interrupted effect, then complete open/read, an extension-owned
  recording start/save/discard with a real GIF content block, a second save to the browser's
  download mechanism that returns no bytes at all, and close. Its adapter advertises liveness and
  acknowledges every dispatch probe through the unchanged opaque browser connector. The journey uses
  a fresh deployment lock to isolate explicit restart recovery from demand-start. It also reads
  the audit file the real executable wrote and checks that the read records a host and a word
  count, and no page text.
- `cargo build --workspace --target-dir .target-ghostlight-1.0`.
- `node --check` on both journeys, the bundled workbench script, and the preview server.
- A live 35-second static Example Domain recording with the scope glow suppressed retained 15
  frames and 121,293 JPEG bytes. Its 211,458-byte GIF was valid GIF89a, carried 35,320 milliseconds
  of playback with a 33,720-millisecond folded static span, and repeated save was byte-identical.
- A live Foundry hover, click, and type sequence retained six distinct frames across 670
  milliseconds. Its 595,861-byte GIF saved twice with the same SHA-256 digest, after which the demo
  state and recording bytes were cleared.
- Live isolated demand-start proofs began with no service. The MCP connector started one exact
  sibling authority and completed MCP initialization. In a separate run, the browser connector
  reported `backend_unavailable`, started one exact sibling authority, and completed its adapter
  hello. Each run found exactly one service at the isolated executable path and removed only that
  test-owned process afterward.
- The repository's live Windows `target/release` stack was replaced from an isolated release
  build under the deploy lock. Stopping its one service authority caused the already-running
  browser connector to demand-start one replacement with a fresh runtime token. A direct launch
  then revealed that workbench and exited while the authority count stayed one.
- The workbench renders against the repository preview server, which now drives the real sequenced
  event path, and uses the byte-identical original Ghostlight artwork.
- Guard tests keep the surface and the orchestrator in step: every publishable change has a
  handler, every capability class has a visual treatment, every runtime intent stays reachable
  (guarded by an exhaustive match), the surface reads the one observed fact no sentence states and
  renders the sentence for the rest, every observed fact is documented where collectors read it,
  every outcome measurement agrees with its sentence, every readiness has a note, the published
  palette
  is present with the accent defined once, the workbench capability grants listen without emit,
  and every catalog tool has a medallion. Each of these was checked against a negative control:
  breaking the thing it guards makes it fail.
- Outcome-language oracles cover every success sentence, every refusal sentence, workspace reason
  mapping, number grouping, safe next steps, sentence/measurement agreement, and the unchanged
  `Observed` JSON round trip. Executor tests prove the browser seam records landing facts without
  counts and the completion path still combines host/readiness with the outcome measurement.
- The complete desktop-workbench change, from its starting revision through the live-monitor
  rebuild, has an empty diff under `crates/mcp-connector`, `crates/browser-connector`,
  `crates/bridge`, and `extension`. The later demand-start lifecycle intentionally changes the
  bridge and both connectors at their connection-lifetime seam; the extension remains unchanged.

## Visual language and monitor content

- Both surfaces share one motion vocabulary. The workbench names its beats as `--beat-*` tokens
  taken from the renderer's frozen `visualIdentity`, so a treatment meaning the same thing in the
  page and the window keeps the same tempo.
- The in-page effect registry (`TRANSIENT_EFFECTS`) owns both reduced-motion enrollment and each
  treatment's beat, and teardown derives from the beat. No effect lifetime is hand-picked.
- The renderer stylesheet is static CSS: identity arrives once as custom properties, leaving only
  the token block and the generated reduced-motion selector interpolated.
- A click describes itself end to end. `ClickShape { clicks, button }` rides on
  `PresentationSignal`, and the renderer draws one ring per click, dashed for a secondary button.
- Audit records and workbench history carry the Ghostlight-authored `summary` and a measured
  `duration_ms`, so every row states what happened and how long it took.
- Outcome language now leads with the action and names the governed place. Browser action receipts
  return the role and accessible name of the physical element in the same effect response, without
  a describe round trip. The orchestrator narrows raw roles to a closed noun, normalizes and bounds
  names to 80 characters, and produces sentences such as `Clicked the "Save" button on
  example.com`. `preserve_target_names` defaults to true; false in either authority layer removes
  names monotonically and leaves `Clicked a button on example.com`. Editable values never supply a
  name. A refused explicit navigation adds only its normalized host to the existing observation
  shape, never its path, query, fragment, or value. Rendered label whitespace is normalized before
  the name is retained, so visually separate label fragments cannot collapse in the audit sentence.
- Per-action observation is built, at the seam it was designed for. See
  [`design/action-observations.md`](design/action-observations.md).
  - `language/outcome.rs` owns `Outcome`, `Refusal`, `WorkspaceReason`, and
    `Observed { host, readiness, count, width, height }`. Every successful completion requires an
    `Outcome`, so its Ghostlight-authored sentence, safe next steps, and named measurements cannot
    drift into separate call-site strings.
  - `Executor::dispatch` remains exhaustive over browser outcomes and gathers host/readiness keyed
    by invocation. `Outcome::observed` supplies counts and capture sizes from the same value that
    authored their sentence. The one completion path merges the outcome over the seam and clears
    the registry.
  - The host and optional governed action-target name are the deliberate line. Never the path,
    query, fragment, selector, target handle, entered value, or arbitrary page text. A capture reports its
    pixel size, a wait reports how long it waited and which condition it waited on, and a read
    reports how many words it read.
  - A count is recorded only where the Ghostlight-authored sentence beside it names what was
    counted, so the count needs no per-tool wording table on the surface. Those summaries now state
    their measurement: "Read 1,240 words from example.com.", "Filled 3 fields on example.com and
    submitted the form.", "Found 7 matches on example.com.", "Captured the viewport at 1280x720."
  - Rows always render the outcome sentence and add a readiness note where a document never
    settled. They no longer guess between host and measurement, because the orchestrator already
    chose which register the sentence uses. The hero renders the same sentence and carries no host
    chip: the sentence names the host, so a chip would say it twice. Readiness is the one observed
    fact no sentence states, and it is the only one the surface reads structurally. The host is
    guarded where it is collected, in
    [`guides/siem-integration.md`](guides/siem-integration.md), because that guide is what a
    person configuring a collector reads.
  - The audit stays content-minimized. `InvocationResult::facts` still carries page text and full
    URLs to the model; the observation is a separate closed type so there is no shortcut between
    them. The bounded action-target name exists only inside Ghostlight's terminal summary and may
    be removed by governance.
    [`guides/siem-integration.md`](guides/siem-integration.md) now documents `summary`,
    `duration_ms`, and `observed`, and states the host exception where it used to claim that no
    host is ever recorded.

- A service session used to outlive its connection whenever that connection ended badly. The
  request loop propagated read errors, oversized frames, and malformed lines out of the handler
  before the release ran, and an unowned workspace has no owning process for the reaper to check,
  so nothing could collect it afterwards: the workspace and every tab it held survived until the
  service restarted. A live workbench showed 17 sessions against 5 connectors. The teardown now
  runs on every exit path, guarded by a test that fails when the old early return is put back.
- The workbench connections bar groups its chips by client label, with a tally when one client
  holds more than one session. The sessions array itself is untouched, because history attribution
  resolves a single workspace to its client by id.

- The workbench surface is hardened and, for the first time, actually executed by a test.
  `node tests/workbench-surface.mjs` runs `app.js` against a minimal DOM with one panel broken on
  purpose and asserts the window still comes up, the failure is visible, the rest of the pass
  continues, and the broken panel is retried rather than memoised as done. Every other guard over
  this window reads its source as text, and none of them could tell that the window never started.
- Four fragilities behind that failure are fixed: the element table is derived from the document
  instead of hand-listed; boot is one ordered sequence that installs its own recovery first;
  wiring is an isolated step rather than loose statements ahead of boot; and a render failure is
  reported as itself instead of as a lost connection.

- The workbench surface is rebuilt around its seams. It was one 1045-line file where vocabulary,
  cache, rendering, transport and wiring were the same thing; it is now `ui/lib/words.js` and
  `ui/lib/entries.js` (pure), `ui/lib/transport.js` (the only caller of the orchestrator),
  `ui/lib/store.js` (the cache and its only writer, announcing a closed set of seven change
  kinds), `ui/lib/view.js` (the only thing that touches the document), and `ui/app.js` as a
  296-line composition root. Data flows one way: transport brings a snapshot, the store folds it
  in and announces, the view draws what it is handed. A view that cannot fetch cannot fail on a
  missing snapshot; a store that never sees the document cannot be corrupted by a paint.
  Guards hold the seam: words, entries, store and transport fail the build if they contain
  `document.`, `window.` or `el[`, with the view as the negative control.

## Executor decomposition

The executor-split batch is complete through `4d633fbc`.

- The operation families now live in `work/reading.rs`, `work/navigation.rs`,
  `work/recording.rs`, `work/pointer.rs`, `work/forms.rs`, and `work/sequence.rs`.
- `work/mod.rs` now contains the dispatch spine, shared execution infrastructure, free helpers,
  private shared types, and the unchanged test module. It fell from 5,824 to 3,255 total lines,
  with its production portion falling from roughly 4,200 to 1,633 lines.
- Each family landed as one pure-move commit. Every task independently passed formatting,
  warnings-denied Ghostlight Clippy, and all 226 orchestrator library tests without test edits.
  The durable task record is [docs/tasks/executor-split/LEDGER.md](tasks/executor-split/LEDGER.md).

## Owed

Post-publication debt and evidence are organized as the
[1.0-plus batch](tasks/1.0-plus/LEDGER.md); its ledger is the authority on what remains there.

On 2026-08-24 the owner promoted four items below into the pre-freeze window
([tasks/pre-freeze-debt](tasks/pre-freeze-debt/LEDGER.md)): the ServiceClient adoption (landed
the same day -- the bridge is now every edge's handshake home), the unsettled-row color
treatment (landed the same day), the model-facing policy explain operation (landed the same day
as [ADR-0136](adr/0136-model-facing-policy-explain.md) and `policy_explain`), and ADR-0105 stages
2 and 3. That last one landed as its stage-2 half only -- observed socket-peer attribution in
audit through the new audited `ghostlight-win-peer` crate ([ADR-0105](adr/0105-scripted-intake-channels.md)
amendment) -- with stage 3 deliberately re-deferred on evidence; see that amendment and the batch
ledger for the reasoning. Everything else in this section stays outside 1.0.

- ADR-0105 Decision 3's signer-gated admission stays deferred. The audited FFI crate exists and
  stage 2 observes the socket peer; what remains missing is any signed Ghostlight artifact to
  verify against, so no positive verification path can be exercised. Revisit when the first
  signed release artifact exists.
- GIF quality and ADR-0084 attention routing continue as the 1.0-plus batch's D2 and D3; the
  extension-stylesheet item was closed there by D1 (`f8bff79a`), and the former origin/main
  promotion item closed with the 2026-08-26 publication.

## Blocked-target evidence

[ADR-0135](adr/0135-blocked-target-evidence.md) closes ADR-0129 Decision 4 through the
[evidence-1 batch](tasks/evidence-1/), completed on 2026-08-24. A blocked integration card now
shows what Ghostlight found instead of only asserting that something is wrong:
`RegistrationState::Foreign` carries the bounded command line it saw, `inspect()` names the
actual cause (foreign entry, malformed configuration, or unreadable file) instead of one
conflated sentence, and an optional orchestrator-authored `evidence` field travels on
`HarnessSummary`, rendered verbatim by the card the destination already has. Ownership behavior
is untouched: foreign entries are still never overwritten or removed, and blocked targets still
offer no automatic repair. Proven by unit pins across JSON, TOML, and YAML dialects, a surface
journey assertion, and -- after the release orchestrator swap -- `ghostlight doctor --json`
against a seeded foreign configuration under redirected user roots reading the exact evidence
sentence back from the deployed binary. The paragraph was then verified with human eyes in the
real workbench: a foreign command was temporarily swapped into the owner's real Claude Desktop
configuration under a hash-verified backup, the card showed the cause sentence and evidence, and
the original bytes were restored and hash-confirmed.


## Release gates still requiring an owner or release environment

The status-bearing route through these gates is the
[1.0 release checklist](RELEASE-CHECKLIST.md). Its candidate-bound boxes reset when the source
revision changes; the prose below records the current evidence boundary.

Local pre-freeze evidence on Windows, 2026-08-17, is recorded in
[the dated release preflight](testing/release-preflight-2026-08-17.md). Formatting,
warnings-denied Clippy, 356 Rust tests, 116 extension tests, 10 npm launcher tests, 4 MCPB tests,
the fresh isolated build, process and both CLI journeys, 42 workbench assertions, policy grammar,
dependency policy, the 17-warning advisory allowance, script syntax, repository integrity, 0.8
recovery, and offline public truth passed. G0 has not frozen a revision, so this is a source
preflight rather than a checked candidate gate.

Extension-specific release preparation was repeated on Windows on 2026-08-22 after an exact
comparison with the published 0.8 ZIP. Product identity, both extension identities, all inherited
artwork, the visible product surfaces, installer-owned native-host generation, and historical
behavior dispositions remain present. The comparison found and fixed three real release gaps: the
missing `downloads` and `offscreen` justifications, the omitted 1.0 recording and diagnostics
privacy disclosure, and an overbroad store-package icon copy that included 1.34 MB of unreferenced
source artwork. Store instructions no longer treat unchanged screenshots as a blocker. The current
local 1.0 ZIP is a reproducible 86,729-byte, 31-entry package with SHA-256
`46507cede88b590f8e029b6cb5603e3103a6e3237ef11357d5fa786f64d307fa`; it has no development key,
carries the four exact inherited icons, and adds only the bounded screenshot geometry library to
the previous package surface. This is local source evidence, not the final
provenance-bound G2 artifact. The exact comparison, fixes, checks, and remaining public-policy
handoff are in
[the dated extension release preparation](testing/extension-release-preparation-2026-08-22.md).
The owner subsequently authorized store and public-policy work. API V2 accepted that exact local
ZIP as the existing item's `1.0.0` draft with upload state `SUCCEEDED`. The public privacy page now
serves the current browser-local recording and diagnostic disclosures with a greenfield date-only
header, and the owner manually reconciled and saved the dashboard-only permission, remote-code,
data-use, certification, and privacy URL fields. API V2 then submitted that earlier package, hash
`ccb48577a93995b1eaaf9b13fab75313a347483553782d178187e1ea8ceb0923`, with state `PENDING_REVIEW`
and publish type `STAGED_PUBLISH`. It is not public and approval will not publish it automatically.
ADR-0131 made that review stale, and on 2026-08-24 it was replaced: the foundry-sprint source was
repackaged (byte-identical across two runs, sha256
`f7b9a6adbf94bf5b1dcc158a3548501ff230ad4d39e72a5c878bde8d2d284d68`), uploaded, and resubmitted
staged -- see [the dated submission record](testing/extension-store-submission-2026-08-24.md).
The review now carries the reply-before-dispatch and fill submit-leg fixes. No store mutation was
made beyond that replacement, the public listing still serves 0.8.0, and this pre-freeze submission
does not close G3 until the frozen provenance-bound candidate matches its bytes and the reviewed
store installation is verified. If the extension source changes before G0 freeze, the submission
is replaced the same way.

The ADR-0131 implementation passes formatting, warnings-denied workspace Clippy, all 361 Rust
tests, all 119 extension tests, JavaScript and shell syntax, the 42-assertion workbench surface,
and a fresh isolated process journey. The process journey exercises viewport capture, region
magnification, and a second region through the real MCP, service, browser-relay, and receipt
boundaries. The exact repository release orchestrator and browser connector were replaced for the
live Windows lane. After the unpacked extension's explicit reload, real attached Chrome advertised
the exact 22-tool catalog and completed open, read, viewport JPEG, region JPEG, and chained-region
JPEG work. The final model-driven close was correctly blocked by the person's preserve-tabs
setting; both disposable Example Domain tabs were then closed directly without changing it.

ADR-0132 fixes a client-compatibility failure discovered during that live browser check. The
orchestrator already returned complete bounded `browser_execute` values and `browser_find` matches,
but the MCP edge exposed them only through `structuredContent`; clients that displayed ordinary
content showed only the authored summary. The generic edge now appends the complete compact opaque
result envelope to that text while preserving identical `structuredContent`, `isError`, and image
blocks. The process journey covers both reported tools using only ordinary content. This is a
connector change, not an extension change: the 86,729-byte extension ZIP and its SHA-256 remain
unchanged, though the pending store review remains stale for the earlier ADR-0131 package reason.
Formatting, warnings-denied Clippy, all 362 Rust tests, all 119 extension tests, all 10 npm launcher
tests, all 4 MCPB tests, JavaScript syntax, the 42-assertion workbench surface, offline public truth,
complete 0.8 recovery, and a fresh isolated process journey pass. The exact repository release MCP
connector was replaced and hash-matched. A direct invocation through that deployed binary against
the live service and attached Chrome returned the complete browser inventory in ordinary text and
preserved the structured envelope. The replaced harness transports must reconnect once; the
orchestrator, browser connector, extension, and Chrome were not restarted.
After VS Code restarted its harness, the refreshed catalog exposed the region screenshot branch.
A live disposable Example Domain journey returned the exact JavaScript value and one semantic find
match in ordinary text, captured a viewport JPEG, and captured a 2400 by 1600 magnified region JPEG.
Model-driven cleanup was correctly blocked by the person's preserve-tabs setting, leaving that one
test tab for direct closure.

- Build-only candidate `fd86403` is historical evidence, not the publishable 1.0 candidate. It is an
  ancestor of the current head, so the next candidate is built from the revision G0 freezes. CI
  `31920645118` and candidate workflow `31920647296` are green; all 19 candidate files have
  repository-, workflow-, source-, and ref-bound provenance. Its GitHub bundle expires on the
  workflow's 14-day retention, so anything still wanted from it has to be held locally.
- Use that provenance-attested bundle to verify clean install, public-0.8 upgrade, and uninstall on
  clean Windows and Linux machines. The Windows development-host package lifecycle and
  virtual-display Debian/Ubuntu smokes do not replace those release-environment rows.
- Complete interactive native-window, tray, and notification smoke tests on each platform. The
  automated environment verifies native build and failure containment but does not expose its GUI
  desktop to the test runner.
- Verify demand-start, direct workbench activation, and deploy quiesce from each clean
  platform installation.
- Repeat the now-passing development-host browser-job matrix against the provenance-bound Debian
  candidate and matching store adapter on Ubuntu GNOME Wayland. The unpacked-adapter CachyOS run
  does not replace that release-environment gate.
- Reconcile release metadata, public status, store submission, compatibility, distribution, and
  the final public documentation only when the 1.0 artifacts exist.
- Chrome API V2 access is restored, and the public policy and dashboard disclosures are current.
  The pending staged review contains the earlier package and must be replaced by the current local
  ZIP, then verified against the exact G2 candidate before claiming G3. Obtain separate owner
  authorization for Store mutation and later public publication.
- Re-run the Linux lifecycle and visible-browser policy matrix on the current policy-restoration
  revision using the `test-01` development host.
- The MCP Registry record must point at the published npm coordinate of its own release, never an
  older one.

## Canonical 1.0 sources

- Product intent: [`1.0/INTENT.md`](1.0/INTENT.md)
- Model-facing language: [`1.0/LANGUAGE.md`](1.0/LANGUAGE.md)
- Architecture: [`1.0/ARCHITECTURE.md`](1.0/ARCHITECTURE.md)
- Acceptance: [`1.0/ACCEPTANCE.md`](1.0/ACCEPTANCE.md)
- Desktop decision: [`adr/0102-integrated-desktop-workbench.md`](adr/0102-integrated-desktop-workbench.md),
  including its 2026-08-11 amendment for the live monitor and the published palette; the tab row
  now carries five destinations (At a glance, Integrations, Status, Policy, About).
- Outcome language decision:
  [`adr/0103-language-owned-outcome-voice.md`](adr/0103-language-owned-outcome-voice.md).
- Demand-start and single-engine decision:
  [`adr/0104-demand-start-single-engine-and-workbench-activation.md`](adr/0104-demand-start-single-engine-and-workbench-activation.md).
- One backgrounded desktop-startup decision:
  [`adr/0112-one-minimized-desktop-startup.md`](adr/0112-one-minimized-desktop-startup.md), amended
  for Linux by [`adr/0118-recoverable-linux-workbench-startup.md`](adr/0118-recoverable-linux-workbench-startup.md).
- End-to-end browser availability decision:
  [`adr/0113-end-to-end-browser-adapter-liveness.md`](adr/0113-end-to-end-browser-adapter-liveness.md).
- Plural browser adapters and routing decision:
  [`adr/0114-plural-browser-adapters.md`](adr/0114-plural-browser-adapters.md).
- Packaged native-host lifecycle decision:
  [`adr/0115-packaged-native-host-lifecycle.md`](adr/0115-packaged-native-host-lifecycle.md).
- Supported operating-system scope:
  [`adr/0116-windows-and-linux-platform-scope.md`](adr/0116-windows-and-linux-platform-scope.md).
- Effective harness configuration resolution:
  [`adr/0117-effective-harness-config-resolution.md`](adr/0117-effective-harness-config-resolution.md).
