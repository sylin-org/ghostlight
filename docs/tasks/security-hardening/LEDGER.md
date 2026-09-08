# security-hardening LEDGER

Durable scope and progress for the [security-hardening epic](EPIC.md). This file is the
authority on what is agreed and what has actually happened. Recommendations remain labeled as
proposals until the discussion settles them.

Opened: 2026-09-06. Review baseline: `48ef29ece1ff0f1633daba62a03932a666f00ca5`.

## RESUME HERE

Latest discussion, 2026-09-07: an all-open Reddit session repeatedly supplied its own
`restrict_hosts` list. The owner rejected that model-facing capability and requested an audit of
other misaligned tools. The [tool surface review](../../design/tool-surface-review-2026-09-07.md)
records recommendations and two reproduced dry-run result defects. No removal is implemented;
candidate changes to submission and batching need decisions before implementation. Preserve
configured governance, accepted browser jobs, and historical request attribution.

Current work, 2026-09-07: the owner reported two open Ghostlight copies. Both were real responsive
native windows inside one service. The startup/Open construction race and a related short startup
wait are corrected and deployed. All 17 gates pass, including 526 Rust tests and 18 new native
window/profile checks. Installed verification finds one responsive window through concurrent Open,
with Ready service and successful fresh MCP/browser calls. STATUS owns the deployed hash and the
[suite record](regression-suite.md#native-workbench-regression-follow-up-2026-09-07) owns evidence.
The thread's cached MCP tool transport remains closed; fresh installed MCP validation passed.
No extension reload is required, no browser drafts were touched, and nothing was pushed/published.

Earlier full-session work, 2026-09-07: the owner requested the [full-session regression suite](regression-suite.md)
and reported wrong-page read animations plus a persistent script spinner. The suite and visual
corrections are complete: all 16 gates pass, including 525 Rust tests,
207 extension tests, six harness fault tests, 23 script cases, and 68 real MV3 cases. It also exposed
and repaired form-batch preflight and recording export requirements. The first complete run caught
stale PowerShell fixture negotiation and Windows Chrome lifecycle races; their corrected harnesses
still fail explicitly on persistent failure. STATUS records the final service deployment.
The owner confirmed the final adapter reload. Installed acceptance passes through the actual native
host, including retained drafts, document exclusion/masking, script/flow effects, and the live Sylin
form's local-only simulation and captures. Its eight groups and 50 invocation records are in
`.tmp/installed-hardening-evidence.json`; STATUS owns the final service hash. The acceptance fixture
also exposed and fixed implicit form ancestry in semantic fills/uploads and incomplete declared
selector/postcondition requirements. The 38-variant, 338-case authority matrix includes those inputs.
Preserve the earlier completed feature work and distinguish configured Windows/Linux CI jobs from
executed Linux evidence. No further extension reload is owed for this service-only follow-up.

Earlier incident evidence, 2026-09-07: orchestrator and MCP connector through `00b44646` were deployed
using the dev-loop; the browser connector stayed unchanged. The owner reloaded the extension.
The broader installed Sylin journey was interrupted by an urgent Reddit draft incident. Its fix
is implemented, deployed, and verified: 513 Rust tests, 195 extension tests, process/CLI journeys,
31 Chromium cases, a visibly retained unsent Reddit reply through the installed native host, and
fresh installed MCP proof of diagnostic availability during attention. See
[the incident record](editor-incident-2026-09-07.md) for its repair and installed evidence. Earlier
package paragraphs below retain the readiness recorded when each package completed; STATUS owns
the latest deployed state. Remaining C1 verification choices still require ideation.

Linux bridge validation ran natively in Debian WSL: 55 passed, 9 environment/cross-build failures;
the private runtime `0600` check and unsupported Linux peer-observation check passed. Full Linux
journeys remain outstanding. The interrupted broader installed Sylin journey is now complete,
with actual installed evidence recorded above separately from the isolated browser fixtures.

The owner requested a complete epic covering the assessment and all subsequent decisions, and
accepted H3 as the first implementation cycle. The [epic](EPIC.md), [bootstrap](BOOTSTRAP.md), and
[ideation agenda](IDEATION.md) now organize that work. Undecided packages require an ideation
session before their implementation cycle; do not silently adopt the earlier recommendations.

[H3 script effect truth](h3-script-effect-truth.md) is implemented and verified locally. Its
record accompanies `fix(script): select script form before page execution`; use Git for the
commit hash. Both SA-04 defects have regressions, all 183 extension tests and workspace gates
pass, and 19 Chromium/MCP cases prove effects and compatibility. It is not deployed or published.
The owner accepted I1 and directed implementation of [H1 readable bounded audit](h1-readable-audit.md).
H1, H2b, and [H4](h4-grouped-history.md) are implemented and verified locally. I1-I3 are accepted.
[H5](h5-runtime-controls.md) is implemented and verified locally under accepted I4 and ADR-0157.
It repairs the request-restriction gap, isolates attention, and checks controls at dispatch.
The owner then authorized local deployment on 2026-09-07. Orchestrator `93976733` is running
from `target/release`; H1, H2a/H2b, H4, and H5 are deployed locally. Doctor reports Ready and
the existing MCP connection passes authority and live tab-list calls. H3's unpacked extension
reload remains pending because browser security policy blocked `chrome://extensions` access.
See the [current deployment record](../../STATUS.md#local-deployment-2026-09-07).
H6 is implemented and verified locally under ADR-0158, including 29 Sylin MV3 browser checks.
See [H6 verification](h6-verification.md). It is not deployed or published.
H7 I6 is accepted and H7 is implemented and verified locally under ADR-0159.
See [H7 verification](h7-audit-health.md): 488 Rust/192 extension tests, real-process failure and
recovery, workbench checks, and 29 Sylin/MV3 regressions pass. H7 is not deployed or published.
H8 I7 is accepted; H8 is implemented and verified locally under ADR-0160.
See [H8 continuity](h8-local-continuity.md): 498 Rust/192 extension tests, process/CLI/UI checks,
Windows private runtime replacement, and 29 Sylin/MV3 regressions pass. H8 is not deployed.
C1 I8 option A is accepted: the reporting foundation is implemented and verified locally under ADR-0161. It preserves
immutable per-connection action attribution, transient claims, narrow durable observations, and
quiet plural connection/history details. See [C1 foundation](c1-reporting-foundation.md).
Signature/hash verification remains deferred to a later C1 cycle; full C1 is incomplete.
Installed native-host transport and cross-platform runtime lanes remain untested for H6.
See [H2b](h2b-aggregate-outcomes.md) for the accepted behavior and bounded work.
Do not repeat completed cycles or implement undecided remedies silently.

The owner identified flow stopping as a bug to fix. [H2a](h2a-flow-stop.md) now fixes the two
missing loop exits in the working tree, with regression evidence and 362 passing orchestrator
library tests. It is committed as `8103c69b`, with full workspace gates and the process journey
now passing; it is now deployed locally. H2b aggregate reporting is implemented and verified locally.

H1's local-audit correction and H2b's aggregate behavior are accepted. H3's
distinction from expected partial effects is accepted. No transactional rollback requirement has
been added. The orchestrator is deployed locally; nothing from this epic has been published.

Client provenance remains reporting-first, with signer/hash admission conditional on a concrete
integration and verifiable subject. ADR-0105 records that direction; ADR-0161 settles the accepted
reporting foundation. Remaining verification and C2/C3 integration decisions require ideation,
not an inferred upstream identity.

The owner then established the product focus: Ghostlight enables delight through integrated
tooling, and governance supplies dependable boundaries chosen by individuals and organizations.
The answers below apply that focus to the open questions. They remain recommendations except
where a later entry explicitly records agreement, including H6's policy choices.

The owner agreed that host authority applies to the document actually accessed (H6's subject
boundary), then accepted policy-selectable exclusion handling and separate human notice
preferences. The agreed choices and starting profile are recorded under H6 below and in the
[H6 design](h6-frame-coverage-ux.md). Existing grants decide access; coverage remains truthful.
ADR-0158 now records the accepted schema, human-only hosts, mask/capture handling, and script
refusal. H6 is implemented locally; the H6 verification record owns its evidence and limits.

## Cycle readiness

| Package | Current disposition | Before its implementation cycle |
| --- | --- | --- |
| H2a | Fixed and committed as `8103c69b`; deployed locally | Required gates and process journey pass. |
| H3 | IMPLEMENTED, deployed, and verified locally | Complete evidence is in its task record and the regression suite; no remaining H3 implementation task. |
| H1 | IMPLEMENTED and verified locally; deployed locally | I1 accepted; all 447 Rust/183 extension tests and the extended process journey pass. See H1 and ADR-0103. |
| H2b | IMPLEMENTED and verified locally; deployed locally | I2 accepted; 455 Rust/183 extension tests and actual MCP/JSONL progress checks pass. See H2b. |
| H4 | IMPLEMENTED and verified locally; deployed locally | I3 accepted; 463 Rust/183 extension tests, incremental JSONL process checks, and isolated Chromium history checks pass. ADR-0156 and H4. |
| H5 | IMPLEMENTED and verified locally; deployed locally | I4 accepted; scoped recovery, admission, timing, and history evidence in H5 and ADR-0157. |
| H6 | IMPLEMENTED, deployed, and verified locally | I5 accepted; ADR-0158, full regression suite, 68 MV3 cases, and installed Sylin/native-host acceptance. |
| H7 | IMPLEMENTED, deployed, and verified locally | I6 accepted; real-process failure/repair and history UI checks pass in the full suite. ADR-0159 and H7. |
| H8 | IMPLEMENTED, deployed, and verified locally on Windows | I7 accepted. Continuity, bounded queues/exchanges, private discovery, and quiet controls. Full Linux journeys remain outstanding. ADR-0160 and H8. |
| C1 | Option A foundation IMPLEMENTED, deployed, and verified locally; full C1 incomplete | ADR-0161 settles immutable attribution and quiet reporting. Signature/hash verification remains deferred to another C1 ideation. |
| C2/C3 | Admission direction CONDITIONAL; mechanism open | Ideation I9 chooses a concrete integration and connection proof. |

Ideation can proceed alongside independent agreed work. Inclusion in the epic does not imply
that all packages are ready for unattended implementation.

## Evidence at opening

The [assessment](../../design/security-assessment-2026-09-06.md) contains the full architecture
review, source links, probe inputs and outputs, and claim reconciliation inventory.

- Reproduced with real orchestrator APIs and an isolated fake browser: a flow continues after a
  reported stop; its child operations produce no individual audit records; a prior successful
  Read's synthetic page text appears in the failed flow's audit record; workspace A's repeated
  denials place workspace B under attention.
- Reproduced with the actual extension evaluator and a Node VM sender: an exception containing
  `Illegal return statement` causes two evaluations and two increments; a runtime-thrown
  `SyntaxError` after an increment is reported with false no-effect certainty.
- Source findings requiring further evidence: per-frame host coverage, ignored audit-write
  errors and their visible consequences, the observation-to-effect pause race, and local service
  resource bounds. These do not all have demonstrated live exploits.
- The earlier assessment ran 360 orchestrator library tests and 171 extension tests, all passing.
  It did not run the full workspace gates or a live Chromium security journey.

## Agreed boundary and client provenance (2026-09-06)

The owner accepted the following direction after discussing same-user host automation and
certificate-based client admission. This is product scope, not evidence of implementation.

1. Ghostlight governs its own invocation route. Independent host/desktop control, including
   mouse and keyboard automation, can bypass that route. Client admission does not claim to
   contain that attacker or provide endpoint-wide audit.
2. Preserve claimed application identity separately from the observed connecting executable,
   verified signer, executable digest, and verification result. Record unavailable evidence
   honestly. Newly observed certificates are not automatically trusted.
3. Permit optional policy admission by verified signer identity or exact executable SHA-256.
   Validate the executable's signature before trusting its certificate. The exact signer-key
   representation, policy spelling, and verification cache are still design work.
4. Preserve all-open defaults. Managed and local admission restrictions intersect and never raise
   browser capability ceilings. A required identity that cannot be verified refuses admission.
   Equivalent invocation routes must not bypass the restriction. Verification stays offline and
   states its revocation limitations.
5. Initially enforce against actual direct peers when a concrete integration can exercise that
   contract. In ordinary MCP usage the service peer is Ghostlight's connector; authenticating it
   does not authenticate the upstream application. A signed interpreter similarly does not prove
   the provenance of the script or instructions it runs.
6. If the desired rule names existing upstream MCP applications, first prove how that identity
   binds to the actual stdio connection. Reported names, supplied certificates, and parent-process
   observations are insufficient authorization. A direct signed integration or an enrolled
   integration credential are possible directions, not selected implementations. A credential
   proves possession, not executable provenance, and requires participating clients.
7. The signed-success verification lane needs a real signed subject. An unsigned artifact pinned
   by hash proves only the hash rule. This work does not require Ghostlight to procure a signing
   certificate as a blanket release-readiness gate.

| ID | Work package | State | Acceptance evidence |
| --- | --- | --- | --- |
| C1 | Accurate client-provenance reporting | Option A foundation IMPLEMENTED and verified locally; verification deferred | Original connection evidence survives queueing, composition, refusal, and shared sessions. Bounded claims remain transient; narrow observations persist. Signature stays Not checked. Full C1 remains incomplete. |
| C2 | Optional signer/hash admission for direct peers | AGREED, CONDITIONAL; not implemented | A concrete allowed peer and negative controls for a different signer/hash, tampering, unavailable evidence, and alternate invocation paths. Layer intersection and all-open remain intact. |
| C3 | Upstream MCP identity binding | AGREED PREREQUISITE if application-specific enforcement is pursued; mechanism open | A real supported client connection demonstrates an identity bound to that connection. The proof distinguishes application identity from connector, launcher, interpreter, and credential identity. |

C1 depends on a safe audit projection before extending durable records. C2 depends on C1 and its
concrete verification subject. C3 is a prerequisite for advertising upstream application admission,
not a prerequisite for honest reporting of a direct peer. No additional process, published service
protocol, client credential system, or product-wide certificate requirement is selected here.

### C1 foundation completion (2026-09-07)

The accepted option A foundation is implemented and verified locally under ADR-0161. The
[C1 record](c1-reporting-foundation.md) records the two root fixes (mutable workspace attribution
and reversed Windows endpoint observation), transient claims, durable evidence, and quiet details.
All gates pass: 508 Rust/192 extension tests, 66 UI checks, C1/process/CLI/continuity journeys,
Chromium history, and 29 Sylin/MV3 checks. No deployment or publication occurred. This record
accompanies `fix(provenance): preserve each action's connection evidence`; Git owns its hash.
Full C1 verification remains open and needs its next ideation before implementation.

### C1 reporting foundation accepted (2026-09-07)

The owner accepted option A and directed implementation. Fix the demonstrated shared-session
attribution overwrite, retain each action's originating connection evidence, and add quiet details
to sessions and action history. H1/H4/H7 already supply the safe audit projection prerequisite.

The accepted foundation separates the bounded live application claim from durable connection ID,
observation time, declared intake, observed basename or explicit unavailable state, and the
`not_checked` signature state. Raw claims stay in memory, including bounded live history, and are
omitted from durable audit and process diagnostics. No path, PID, command line, or certificate dump
is added. Windows corrects its socket observer's TCP endpoint direction; Linux reports unsupported
observation. Reconnect always captures new evidence. Older records cannot borrow a newer
connection's identity, and legacy `peer_image` values are shown as Not recorded because the former
observer selected the service-owned row. Original historical files remain untouched.

The workbench keeps familiar names and exposes plural connections, parent receipts, and recorded
children through collapsed details. Unchecked or unavailable provenance creates no routine popup,
trust badge, or admission consequence. [ADR-0161](../../adr/0161-connection-bound-provenance.md)
owns the decision; [C1 foundation](c1-reporting-foundation.md) owns evidence and remaining work.

Signature/hash verification is explicitly deferred to another C1 cycle. C2/C3 remain conditional
on a concrete subject and connection proof; full C1 is not complete when this foundation lands.

## Work packages and priority

P0 means a demonstrated violation of an existing effect or confidentiality promise. P1 restores
complete enforcement and evidence or resolves a source finding with substantial possible impact.
P2 is focused hardening or further assurance after the demonstrated defects. These are scheduling
priorities, not CVSS scores. H1-H8 are complete locally. C1's reporting foundation is accepted.
The remaining order is a recommendation to settle with
the relevant ideation session and dependencies.

| Order | ID | Priority | Work package | Evidence and reason |
| --- | --- | --- | --- | --- |
| 1 | H3 | P0 | Prevent script re-evaluation and false no-effect reports | Implemented; 20 evaluator tests and 19 real Chromium/MCP cases prove the correction and compatibility. Not deployed. |
| 2 | H1 | P0 | Keep payloads out of audit | SA-03 corrected after I1: typed audit projection; failure, legacy-read, and actual JSONL checks pass. Not deployed. |
| 3 | H2 | P0 | Make flow stopping and aggregate effects truthful | H2a and H2b implemented and verified locally; counts, partial effects, recovery, and actual MCP/JSONL agree. |
| 4 | H4 | P1 | Complete child operation receipts | SA-02: flow reproduction and sequence source evidence. Policy checks happen, but terminal child records are missing. |
| 5 | H5 | P1 | Correct workspace attention and prove human-control timing | Reproduced SA-05 cross-workspace attention; SA-08 pause race still needs a controlled test. |
| 6 | H6 | P1 | Enforce composed-frame policy subjects | Implemented and verified locally under ADR-0158; 29 Sylin MV3 checks and full workspace gates. |
| 7 | H7 | P1 | Expose audit failure and define recovery | SA-07 source finding. Completion ignores append errors and UI history can outlive durable recording success. |
| 8 | H8 | P2 | Verify local admission and resource bounds | Review file permissions, unauthenticated idle connections, framing, cancellation, and simultaneous callers with bounded process tests. No exhaustion exploit is established. |

### H1: Audit confidentiality

I1 accepted: readable history by default, details on demand, existing target-name policy retained.
Minimal/Descriptive profiles and richer capture are deferred. The [H1 task](h1-readable-audit.md)
records the bounded implementation and evidence; ADR-0103 records the decision.

Discussion clarification: partial input already received by the website is an expected effect of
a non-atomic browser operation. SA-03 concerns a different destination: Ghostlight copies an
earlier Read result into its local audit after a later child fails. The owner accepted its
correction after I1. The correction preserves prior website effects and promises no rollback.

Replace unrestricted `facts` copying with a closed audit projection at its owning seam. Review
summary text and raw browser exceptions as well as structured facts. Pin permitted fields and
use synthetic sentinels in page content, scripts, form values, file paths, and error text. Exercise
ordinary and composite success/failure and serialization. Keep the existing bounded target-name
exception and its monotonic privacy setting explicit.

Exit evidence: forbidden sentinels appear in intended tool results where applicable and are absent
from every corresponding serialized audit record. No whole-result JSON reaches new audit records.
Do not erase existing audit files as part of the fix; any historical-data cleanup is separate work.

### H2: Flow control and aggregate truth

H2a and H2b are implemented and verified locally. I2 was accepted on September 6; see
[H2b](h2b-aggregate-outcomes.md) and the ADR-0133 amendment. Completed means succeeded. One
accumulator now preserves status/effect counts and current-state recovery for flow and sequence.
Human directives and invocation limits override Continue. Safe progress metadata survives result
omission and enters H1's parent audit projection. H4 now supplies grouped child receipts; automatic resumption remains a separate decision.

Cover a governed refusal, a decoded-operation error, and a reference error followed by independent
work. Under stop, no later step dispatches. Under continue, the result still truthfully reports
which children failed and which effects happened. A denied child must remain denied; an already
applied effect must not be erased from the aggregate account.

Exit evidence: command logs, child statuses, completed counts, and aggregate status/effect agree
for all cases. The flag-only stop behavior and inconsistent aggregation rules have been replaced.

### H3: Script effect truth

Agreed next cycle (2026-09-06): after the earlier discussion of partial effects, the owner accepted
the proposed H3 correction and requested the complete epic. SA-04 concerns an extra evaluation
initiated by Ghostlight after an error and false no-effect certainty after a runtime exception.
The September 6 amendment to ADR-0133 records the accepted behavior. See the
[H3 task and execution record](h3-script-effect-truth.md); implementation and the required
isolated evaluator, workspace, process, and Chromium/MCP verification are now complete locally.

Choose the supported script interpretation without executing possibly effectful code twice.
Do not use exception text or a runtime exception class as proof of a parse-only failure. Preserve
uncertainty whenever evaluation may have begun.

Exit evidence: the two SA-04 scripts each evaluate once and never report a known no-effect result
after changing the sentinel. Genuine supported bare-return inputs still behave as documented.
Add an actual Chromium CDP lane after focused evaluator tests; label each lane separately.

### H4: Composition completion and receipts

I3 accepted; implemented and verified locally. [H4](h4-grouped-history.md) records the delivered
experience, evidence lanes, limits, and commit reference. ADR-0156 owns the decision.

Depends on H1 for safe projection and H2 for stable flow semantics. Route ordinary and composed
children through one completion seam under the parent's snapshot and lease. Capture child
identity/order, actual RAWX requirements, policy decision, and terminal effect. Define aggregate
receipts without losing refusals or implying that an allowed empty wrapper approved every child.
Review positive grant provenance alongside the existing denial attribution.

Exit evidence: equivalent direct and composed operations produce equivalent child decisions and
safe terminal receipts; each attempted child completes once; unattempted later steps are not
reported as executed. No recursive top-level execution reacquires the lease or changes authority.

### H5: Runtime control and workspace scope

I4 was approved on 2026-09-06. [H5](h5-runtime-controls.md) is implemented and verified locally
on 2026-09-07 under [ADR-0157](../../adr/0157-session-attention-and-dispatch-control.md).
H4 reproduced observe-mode admission skipping a stricter request restriction. The evaluator now
checks request limits before returning observed allowance; direct and composed negative controls
prove that denied work sends no browser command and retains the actual permission decision.

Automatic denial and credential attention live in the workspace. The first incident links to
its triggering history group, and an independent incident id prevents stale recovery from clearing
newer attention. Three matching enforced denials in 60 seconds or five in 120 seconds retain the
existing thresholds. Observed findings do not count. The triggering child ends its composition
even if human recovery races completion. Global Resume preserves scoped attention. Human scoped
resume permits new requests, changes no grant, and replays no work.

The browser port checks runtime controls after waiting for the writer and before transmission,
including bounded close compensation. Post-action observations retain the acknowledged effect
when a later control prevents their dispatch. Already dispatched uncertainty remains uncertain.
Workbench notices identify the affected session; Review history restores the current receipt even
after Clear view, and Resume this session passes the exact incident to the human-only facade.

The H5 task records the full test lanes and their limits. The orchestrator is now deployed
locally. Installed Tauri/MV3 control timing and Linux runtime proof remain open; no publication.

### H6: Frame/site coverage

Agreed scope (2026-09-06): apply the chosen boundary to the document actually accessed. The
September 6 amendments to ADR-0151 record that principle and the subsequently accepted policy
direction. The owner requested that these choices be added to the ledger:

- Exclusion handling: use permitted content; require complete access for the operation; or
  require complete access for the page.
- Human notices, managed separately: on demand; when work is affected; or whenever content is
  excluded. Quiet presentation retains truthful structured coverage for the client.
- Starting profile: use permitted content, with human notices when work is affected.
- Existing host/RAWX grants decide access, including readable fields with forbidden edits.
  Handling and notice settings do not override those grants. Lower tiers cannot weaken an
  organizational requirement; the effective setting and its source are visible.
- All-open remains straightforward. These choices neither prevent website network activity nor
  make multi-step browser operations atomic.

On September 7 the owner chose human-only host details, masked excluded screenshot regions,
recording stops at the boundary, and script refusal when excluded access cannot be bounded.
ADR-0158 records the implementation contract. `content.frames.handling` and
`content.frames.notice` are registered choices. The executor admits actual documents before
extraction; the adapter binds content access to Chrome document identities. Bounded coverage
crosses results/audit, while host details remain volatile and human-only.

The [verification record](h6-verification.md) covers fresh process boundaries, the actual Sylin
iframe demo, and its form content served on distinct local hosts. All three modes and notices,
mixed RAWX grants, batch preflight, stale locators, truthful negatives, image masking/cleanup,
recording stops and source-aware export, script refusal, and audit privacy pass. Restricted
recordings stop on any admitted document-set change; a new recording requires fresh admission.
Nothing prevents the website's own network activity or supplies semantic transaction authority.

H6 is complete locally with 478 Rust tests, 192 extension tests, and 29 Chrome/MV3 Sylin checks.
The test replaces native-port discovery with a loopback pipe to the real browser connector;
installed native-host registration and other platforms remain separate, untested lanes.

### H7: Durable audit health

I6 accepted on 2026-09-07. The owner directed implementation of Keep working by default, optional
Require audit, persistent health, storage truth separate from browser effects, and bounded recovery
without replay. [ADR-0159](../../adr/0159-audit-health-and-recovery.md) and [H7](h7-audit-health.md)
own the contract and verification.

Inject a failing sink and verify what the tool result, workbench, and durable file actually say.
Define visible health and recovery, including whether a configured strict policy affects later
admission. Failure after an effect cannot be reported as if that effect never occurred. Coordinate
with H1/H4 so both parent and child receipt failures have an honest account.

Exit evidence: durable failure is visible, prior effects stay truthful, and recovery behavior is
specified and tested. Local append-only JSONL is not described as tamper proof. No vendor audit
collector or local hash-chain feature is assumed necessary.

### H8: Bounded local service

I7 is accepted and H8 is implemented locally. The owner selected quiet burst absorption, useful
waiting status, containment of stalled connections, usable other sessions and human controls,
and automatic connection recovery without action replay. ADR-0160 owns that decision and the
fixed implementation bounds. The [H8 record](h8-local-continuity.md) owns verification and limits.

The Windows permission review found inherited broad-user rules on the installed discovery file.
New runtime publication now creates a protected current-user/SYSTEM file before writing the token;
Linux uses a new exclusive 0600 file. The installed file remains unchanged until deployment.
The duplicate-ID cancellation defect and repeated Pause/Stop guardrail popups also have regressions.

All required gates, 498 Rust/192 extension tests, process reconnect/audit, CLI, new isolated H8
resilience, workbench UI, and 29 Sylin/MV3 checks pass. Local commit:
`fix(service): absorb bursts and bound local exchanges`. No deployment or publication. Linux
runtime execution, another-user impersonation, and installed-browser transport remain untested.

## Cross-cutting acceptance

Each implemented package updates affected active trust claims with its evidence. The final proof
should connect a permitted operation, a refused operation, correct continuation/stop semantics,
and a content-minimized receipt for each actual attempt. A deterministic hostile-page request
trace is valid fixture evidence; call it a live prompt-injection result only if that was tested.

Every implementation task records its own commit and required checks under the bootstrap. Full
workspace or live-browser success is never inferred from the earlier assessment's partial suites.

## Proposed answers under the product focus (2026-09-06)

The owner asked how the product focus changes the answers to the earlier questions. The following
recommendations turn it into expected behavior. The existing accepted scope and H2a fix remain
as recorded above. H1/H2b/H3 corrections and H6's policy/notice choices were subsequently accepted;
other additional design choices remain proposed unless explicitly recorded otherwise.

| Open issue | Recommended answer | Intended user experience |
| --- | --- | --- |
| Non-atomic work and recovery | Preserve completed, failed, unattempted, and uncertain work distinctly. Retry only with evidence that repetition is safe; an error message is insufficient evidence. | A failure explains where work ended without undo claims or unexpected repeated effects. |
| Audit confidentiality | Keep a closed metadata record of operations, policy decisions, and effects. Preserve the existing governed target-name exception. Exclude full page results, entered values, and arbitrary exception text from durable audit. | Useful history without creating an unexpected content archive. |
| Composite receipts | Give each attempted child a safe terminal receipt and group those receipts under one user-level operation. Clearly mark unattempted steps; do not fabricate execution records for them. | The default history stays readable, with enough detail available to explain a failure. |
| Audit-write failure | Default operation continues with visible degraded history and bounded recovery. If durable audit is explicitly required by policy, a known recording failure refuses new browser work until recording recovers; status and human controls remain available. Never erase an already-dispatched effect from the account or claim perfect atomic coupling between effects and disk writes. | An individual gets an honest degradation; an organization gets the prerequisite it deliberately configured. |
| Mixed-host frames | Apply authored host/RAWX rules to each document. Make the response to exclusions selectable: useful permitted content, complete operation scope, or complete page scope. Choose human notice intensity separately; results retain truthful coverage. | People and organizations choose how partial access affects work and attention, with the effective choice and its source visible. |
| Screenshots of restricted content | Apply the same disclosure boundary. Omit restricted areas only where that exclusion can be enforced reliably; otherwise refuse the capture and name a permitted alternative where one exists. | A screenshot does not quietly bypass the restriction on reading the same content. |
| Workspace and human controls | Automatic denial attention stays with its workspace. Explicit human global controls retain global scope. Prevent subsequent dispatch while preserving truthful facts about already-dispatched work. | One troubled task does not interrupt unrelated tasks; a person's deliberate stop remains dependable. |
| Client identity and certificate lifecycle | Keep the agreed reporting-first approach. Use verified cryptographic identities rather than display names. Approved signers may cover ordinary updates; identity rotation needs a policy update; exact hashes remain an optional tighter pin. Required evidence that a platform cannot supply refuses the restricted admission. Cache verification only with sound binding to an unchanged artifact. | Familiar updates need little maintenance, and changes to the actual trust identity are legible. |
| Local resilience | Bound abandoned connections, requests, and recovery at existing seams, using demonstrated failure cases to choose limits. | A misbehaving client cannot consume the service indefinitely or make other work mysteriously stall. |

These frame recommendations cover Ghostlight's own observations and actions, not every network
request made by a website. All-open operation still covers the complete supported composed page.
Mechanism capabilities remain RAWX; Ghostlight does not infer whether a generic click means
buying, sending, or another business action. No general intent classifier or per-action approval
ritual is introduced by this focus.

H1-H8 are complete locally. The accepted C1 reporting foundation is implemented and verified locally using the repaired
receipt seam. Remaining verification and conditional signer admission follow a concrete integration.
The epic maps those dependencies.
Run the affected package's ideation session before implementing its undecided choices.

## Decisions still open

- The later C1 signature/hash verification subject, result vocabulary, offline limitations,
  additional platform support, and cache invalidation.
- C2 signer pin and exact-hash representation, rotation, and admission lifecycle.
- Which concrete integration justifies C2; whether C3 is needed and which participating client
  can prove it. No client-private-key or handshake design has been selected.

The [ideation agenda](IDEATION.md) expands these questions into package-specific sessions.
H1-H8 behavior, existing pause/stop semantics, and C1's reporting foundation remain accepted
context rather than unresolved questions.

## Activity record

| Date | Work | Result |
| --- | --- | --- |
| 2026-09-07 | Owner accepted C1 option A and directed implementation | Reporting foundation in progress under ADR-0161: immutable action attribution, transient claims, narrow durable evidence, and quiet details. Signature/hash verification deferred; full C1 incomplete. No deployment. |
| 2026-09-07 | Completed accepted C1 reporting foundation | Two attribution root fixes, quiet human details, and private claims verified by all gates, 508 Rust/192 extension/66 UI checks, C1/process/CLI/continuity journeys and 29 Sylin/MV3 checks. Source only; remaining C1 verification needs ideation. |
| 2026-09-07 | Owner settled H6 choices and directed full testing with Sylin content | ADR-0158 and H6 implemented; 478 Rust/192 extension tests, real process checks, and 29 Chrome/MV3 Sylin cases. Not deployed or published. See h6-verification.md. |
| 2026-09-06 | Preserved the architecture/security review independently of events | Dated assessment with isolated reproductions and source-only findings; no production change. |
| 2026-09-06 | Discussed host control and client provenance; owner agreed and requested ledger entry | C1-C3 scope recorded; ADR-0105 amended with accepted direction and explicit deferrals. |
| 2026-09-06 | Ranked remaining work and checked current human-control contract | H1-H8 proposed. Corrected stale MEMORY wording: pause/stop directives already exist under ADR-0126; H5 concerns enforcement timing and scope. |
| 2026-09-06 | Owner accepted flow stopping as a bug; challenged framing of partial effects | Clarified H1/H3 without assuming agreement. H2a implemented and verified locally; no rollback mechanism added. |
| 2026-09-06 | Owner framed delight in integrated tooling and dependable chosen boundaries as the product purpose | Proposed concrete answers for partial effects, private audit, logging failure, frame coverage, scoped controls, and provenance. No further implementation or acceptance inferred. |
| 2026-09-06 | Owner agreed to document-specific host authority and asked how denied embeds should affect UX and trust | ADR-0151 amended for the subject boundary. H6 proposes permitted scoped results, explicit exclusions, truthful negative answers, quiet human notices, and consistent capture restrictions; no implementation started. |
| 2026-09-06 | Owner challenged fixed partial-result behavior and proposed policy flags for delight | H6 revised around selectable exclusion handling and separate notice preferences, composed through existing authority. Defaults and schema remain open; no production change. |
| 2026-09-06 | Owner accepted the policy choices and requested a ledger entry and current inventory | H6 handling modes, separate notice choices, and the use-permitted-content/notice-when-affected starting profile recorded as agreed. Detailed contracts and implementation remain outstanding. |
| 2026-09-06 | Owner accepted H3 next and requested one epic containing all discussion and decisions, with ideation before undecided cycles | EPIC, IDEATION, and the H3 task brief created; bootstrap, decision index, and readiness states reconciled. ADR-0133 amended for script correctness. No new production implementation or test run in this planning turn. |
| 2026-09-06 | Owner accepted H1 ideation and directed implementation | Readable bounded history; concise recovery language; display and retention separate. Profiles and richer capture deferred. H1 completed locally: 447 Rust tests, 183 extension tests, and actual JSONL checks through the process journey pass. Not deployed. |
| 2026-09-06 | Owner directed implementation | H2a committed separately as 8103c69b; assessment and epic preserved as a0310637. H3 implemented with local parsing and one effectful evaluation; full gates, 183 extension tests, and 19 Chromium/MCP cases pass. No deployment. Next: H1 ideation. |
| 2026-09-06 | Owner accepted H2b ideation and directed implementation | Shared progress and recovery implemented; completed counts successes, Continue retains failures, known partial effects remain known, and metadata survives omission. 455 Rust/183 extension tests and fresh-build MCP/JSONL checks pass. No deployment. Next: H4 ideation I3. |
| 2026-09-06 | Owner accepted I3 and directed H4 implementation | Incremental bounded child receipts, grouped history, and actual permission explanations. 463 Rust/183 extension tests, fresh-build process/JSONL, surface, and isolated Chromium history checks pass. No deployment. Next: H5 ideation I4. |

## H5 execution record (2026-09-07)

After the implementation commit, the owner requested local deployment. The dev-loop swapped
only the orchestrator at `93976733`; the isolated build and live binary hashes match. Existing
connector processes and binaries were preserved. Doctor reports Ready, and the existing MCP
session passes authority and live tab-list calls. The [deployment record](../../STATUS.md#local-deployment-2026-09-07)
retains the hash and the pending H3 extension reload. The implementation evidence below predates
that deployment and remains separate from installed-browser control timing.

The prior working tree contained the approved H5 patch and ADR. Continued exploration completed
its runtime, recovery, and UI paths. Formatting, workspace Clippy, all 469 Rust tests (392
orchestrator library), and 183 extension tests pass. Six new Rust regressions cover request
restriction enforcement, session isolation/recovery, writer queuing, preparation timing,
postcondition effects, and already-dispatched uncertainty. The process journey proves actual MCP
error flags, three child JSONL receipts, an unaffected second session, global Resume preserving
attention, and Pause between the synthetic adapter's describe receipt and the input command.
The bundled surface and isolated Chromium history journey pass, including cleared-history review,
first-problem scrolling, scoped recovery, and the 720-pixel layout. See H5 for evidence limits.
Local commit: `fix(control): isolate session attention and guard browser dispatch`; use Git for
its hash. No deployment, push, publication, or machine-local notes access.

## H2a execution record (2026-09-06)

Status: implemented and committed as `8103c69b`; not deployed.

- Exploration used the current flow, sequence, decoder, result types, executor fixture, and fake
  browser. ADR-0133 Decision 9 already requires the stop behavior; no new decision was needed.
- Before the fix, both added regression tests failed with two OpenTab commands instead of one:
  after a denied Execute and after a Read argument resolved to an invalid numeric value.
- `crates/orchestrator/src/work/flow.rs` now exits immediately after recording a child-decode
  failure or non-success child when `on_error` is `stop`. The existing reference-error exits
  provide the same behavior. The production change is two loop exits.
- `crates/orchestrator/src/work/mod.rs` adds two executor regression tests, including a matrix
  of decode/reference failures under explicit stop and continue. They verify physical command
  counts, retained earlier effects, failure rows, partial effect on stop, and no repeat-safe claim.
- Validation: `cargo test -p ghostlight --lib --locked --target-dir .target-ghostlight-1.0`
  passed all 362 tests. `cargo fmt --all -- --check` and `git diff --check` passed.
- H2a changes no extension, connector, policy, audit-projection, or script-evaluator code. Before
  its separate commit, formatting, workspace Clippy, workspace tests, extension tests, changed
  JavaScript syntax, and the fresh-build process journey passed. The orchestrator-only fix uses
  the deterministic fake-browser regression rather than claiming a live flow experiment.
- H1, H3, H4, and the broader H2 aggregate account remain unresolved. This entry completes the
  specific continuation bug, not the full hardening package.


## H7 execution record (2026-09-07)

The owner accepted I6 and directed implementation. One recorder now owns safe audit persistence,
health, and recovery. Keep working remains the default; Require audit is monotonic and attributed
to the original policy layer. Strict failures stop subsequent browser work and compositions,
including under Continue and observe policy, without revising earlier effects. Each result and
history receipt reports storage independently. A saved parent does not upgrade unconfirmed children.
Human controls and policy explanation remain available; health uses existing surfaces without
repeated native notifications. Automatic storage recovery is bounded and never replays or backfills.

All required gates pass: 488 Rust tests, 192 extension tests, formatting, Clippy, changed JS syntax,
ASCII, and diff whitespace. Fresh real-process failure/repair/cold-start checks, bundled UI in
Chromium, and all 29 Sylin/MV3 regressions pass. [H7 verification](h7-audit-health.md) records
mechanisms, artifacts, lane boundaries, and limits. No deployment, push, or publication. Commit:
`feat(audit): expose storage health and enforce audit requirements`; use Git for its hash.
